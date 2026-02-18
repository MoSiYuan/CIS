//! GLM Cloud Node API - 云端节点为 GLM 暴露的可信接口
//! 
//! 架构: GLM → HTTP API (云端) → Matrix Room → 本地 CIS 节点 → 执行

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// GLM API 配置
#[derive(Debug, Clone)]
pub struct GlmApiConfig {
    /// 监听地址
    pub bind_addr: SocketAddr,
    /// 允许的 DID 列表（格式: did:cis:{node_id}:{pub_key_short}）
    /// 与 CIS 其他节点间通信使用的 DID 格式保持一致
    pub allowed_dids: Vec<String>,
    /// 默认目标 Room ID
    pub default_room_id: String,
    /// 任务超时时间（秒）
    pub task_timeout_secs: u64,
}

impl Default for GlmApiConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:6767".parse().unwrap(),
            // 默认允许示例 DID，实际部署时替换为 GLM 云端节点的 DID
            allowed_dids: vec![
                "did:cis:glm-cloud:abc123".to_string(),
            ],
            default_room_id: "!default:matrix.org".to_string(),
            task_timeout_secs: 300,
        }
    }
}

/// GLM API 服务端
pub struct GlmApiServer {
    config: GlmApiConfig,
    state: Arc<GlmApiState>,
}

/// 共享状态
pub struct GlmApiState {
    /// 任务存储
    tasks: RwLock<HashMap<String, GlmTask>>,
    /// 等待确认的任务
    pending_confirmations: RwLock<HashMap<String, PendingDag>>,
    /// 默认 Room ID
    default_room_id: String,
    /// Skill 管理器（用于调用 dag-executor）
    skill_manager: Option<Arc<crate::skill::SkillManager>>,
    /// Matrix HTTP Client（用于发送消息到 Matrix Room）
    matrix_client: Option<MatrixHttpClient>,
}

/// Matrix HTTP Client 配置
#[derive(Debug, Clone)]
pub struct MatrixHttpClient {
    server_url: String,
    access_token: String,
    user_id: String,
}

impl MatrixHttpClient {
    /// 创建新的 Matrix HTTP Client
    pub fn new(server_url: String, access_token: String, user_id: String) -> Self {
        Self {
            server_url,
            access_token,
            user_id,
        }
    }
    
    /// 发送消息到 Room
    pub async fn send_message(
        &self,
        room_id: &str,
        msgtype: &str,
        body: &str,
    ) -> anyhow::Result<String> {
        let txn_id = format!("{}-{}", uuid::Uuid::new_v4(), chrono::Utc::now().timestamp_millis());
        let url = format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
            self.server_url, room_id, txn_id
        );
        
        let content = serde_json::json!({
            "msgtype": msgtype,
            "body": body,
        });
        
        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&content)
            .send()
            .await?;
        
        if response.status().is_success() {
            let result: serde_json::Value = response.json().await?;
            Ok(result["event_id"].as_str().unwrap_or("unknown").to_string())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Failed to send message: {} - {}", status, body))
        }
    }
}

/// 目标节点 HTTP Client（Task 4.1）
#[derive(Debug, Clone)]
pub struct TargetNodeClient {
    /// 节点地址列表（用于负载均衡/故障转移）
    node_endpoints: HashMap<String, String>, // node_id -> base_url
    /// 默认超时
    timeout_secs: u64,
}

impl Default for TargetNodeClient {
    fn default() -> Self {
        Self {
            node_endpoints: HashMap::new(),
            timeout_secs: 30,
        }
    }
}

impl TargetNodeClient {
    /// 创建新的目标节点客户端
    pub fn new() -> Self {
        Self::default()
    }
    
    /// 注册节点地址
    pub fn register_node(&mut self, node_id: String, base_url: String) {
        self.node_endpoints.insert(node_id, base_url);
    }
    
    /// 直接推送 DAG 到目标节点（Task 4.1）
    pub async fn push_dag(
        &self,
        target_node: &str,
        dag: &crate::scheduler::DagSpec,
    ) -> anyhow::Result<DagPushResponse> {
        let base_url = self.node_endpoints.get(target_node)
            .ok_or_else(|| anyhow::anyhow!("Unknown target node: {}", target_node))?;
        
        let url = format!("{}/api/v1/dag/execute", base_url);
        
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout_secs))
            .build()?;
        
        let response = client
            .post(&url)
            .json(dag)
            .send()
            .await?;
        
        if response.status().is_success() {
            let result: DagPushResponse = response.json().await?;
            info!("DAG pushed to node {}: run_id={}", target_node, result.run_id);
            Ok(result)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("Failed to push DAG: {} - {}", status, body))
        }
    }
}

/// DAG 推送响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagPushResponse {
    pub run_id: String,
    pub status: String,
    pub message: String,
}

/// Room 广播客户端（Task 4.2）
#[derive(Debug, Clone)]
pub struct RoomBroadcastClient {
    matrix_client: Option<MatrixHttpClient>,
    default_room_id: String,
}

impl RoomBroadcastClient {
    pub fn new(matrix_client: Option<MatrixHttpClient>, default_room_id: String) -> Self {
        Self {
            matrix_client,
            default_room_id,
        }
    }
    
    /// 广播 DAG 到 Room
    pub async fn broadcast_dag(
        &self,
        dag: &crate::scheduler::DagSpec,
        target_room: Option<String>,
    ) -> anyhow::Result<String> {
        let room_id = target_room.unwrap_or_else(|| self.default_room_id.clone());
        
        let event_content = serde_json::json!({
            "type": "io.cis.dag.execute",
            "content": {
                "dag_id": dag.dag_id,
                "tasks": dag.tasks,
                "scope": dag.scope,
                "target_node": dag.target_node,
                "priority": dag.priority,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }
        });
        
        if let Some(ref matrix) = self.matrix_client {
            let event_id = matrix.send_message(
                &room_id,
                "m.text",
                &event_content.to_string(),
            ).await?;
            
            info!("DAG {} broadcast to room {}, event_id: {}", dag.dag_id, room_id, event_id);
            Ok(event_id)
        } else {
            error!("Matrix client not available, DAG broadcast failed");
            Err(anyhow::anyhow!("Matrix client not available, cannot broadcast DAG"))
        }
    }
}

/// GLM 任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlmTask {
    pub task_id: String,
    pub task_type: TaskType,
    pub target: String,
    pub args: String,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// 目标节点 ID（Matrix User ID）
    pub target_node: Option<String>,
    /// 来源 Room ID
    pub room_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskType {
    ShellCommand,
    OpenApp,
    FileSearch,
    SystemControl,
    DagPublish,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskStatus {
    Pending,
    Running,
    Success,
    Failed,
    WaitingConfirmation,
}

/// DAG 发布请求（等待确认）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingDag {
    pub dag_id: String,
    pub description: String,
    pub tasks: Vec<DagTaskDef>,
    pub schedule: Option<String>,
    pub created_at: String,
    pub expires_at: String,
    pub requested_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTaskDef {
    pub id: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub command: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// API 请求/响应类型

#[derive(Debug, Deserialize)]
pub struct ListTasksRequest {
    #[serde(default)]
    pub status: Option<TaskStatus>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Debug, Serialize)]
pub struct TaskListResponse {
    pub count: usize,
    pub tasks: Vec<TaskInfo>,
}

#[derive(Debug, Serialize)]
pub struct TaskInfo {
    pub task_id: String,
    pub task_type: TaskType,
    pub target: String,
    pub status: TaskStatus,
    pub result: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub target_node: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct IssueTaskRequest {
    pub task_type: TaskType,
    pub target: String,
    #[serde(default)]
    pub args: String,
    /// 目标节点（Matrix User ID，如 @user:example.com）
    pub target_node: Option<String>,
    /// 指定 Room ID（可选）
    pub room_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IssueTaskResponse {
    pub task_id: String,
    pub status: TaskStatus,
}

#[derive(Debug, Deserialize)]
pub struct QueryTaskRequest {
    pub task_id: String,
}

#[derive(Debug, Serialize)]
pub struct TaskStatusResponse {
    pub task_id: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PublishDagRequest {
    pub dag_id: String,
    pub description: String,
    pub tasks: Vec<DagTaskDef>,
    pub schedule: Option<String>,
    /// 目标节点
    pub target_node: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PublishDagResponse {
    pub dag_id: String,
    pub status: String,
    pub confirm_url: String,
    pub expire_sec: u64,
}

#[derive(Debug, Deserialize)]
pub struct QueryDagRequest {
    pub dag_id: String,
    #[serde(default)]
    pub query_scope: String,
}

#[derive(Debug, Serialize)]
pub struct DagStatusResponse {
    pub dag_id: String,
    pub status: String,
    pub last_run: Option<String>,
    pub completed_tasks: usize,
    pub total_tasks: usize,
    pub raw_data: Value,
}

#[derive(Debug, Deserialize)]
pub struct AnalyzeDagRequest {
    pub dag_id: String,
    #[serde(default)]
    pub stuck_task_hint: Option<String>,
}

impl GlmApiServer {
    /// 创建新的 GLM API 服务端
    pub fn new(config: GlmApiConfig) -> Self {
        let state = Arc::new(GlmApiState {
            tasks: RwLock::new(HashMap::new()),
            pending_confirmations: RwLock::new(HashMap::new()),
            default_room_id: config.default_room_id.clone(),
            skill_manager: None,
            matrix_client: None,
        });

        Self { config, state }
    }
    
    /// 创建新的 GLM API 服务端（带 SkillManager）
    pub fn new_with_skill_manager(
        config: GlmApiConfig, 
        skill_manager: Arc<crate::skill::SkillManager>
    ) -> Self {
        let state = Arc::new(GlmApiState {
            tasks: RwLock::new(HashMap::new()),
            pending_confirmations: RwLock::new(HashMap::new()),
            default_room_id: config.default_room_id.clone(),
            skill_manager: Some(skill_manager),
            matrix_client: None,
        });

        Self { config, state }
    }
    
    /// 创建新的 GLM API 服务端（带 Matrix Client）
    pub fn new_with_matrix(
        config: GlmApiConfig,
        matrix_server: String,
        matrix_token: String,
        matrix_user: String,
    ) -> Self {
        let matrix_client = Some(MatrixHttpClient::new(
            matrix_server,
            matrix_token,
            matrix_user,
        ));
        
        let state = Arc::new(GlmApiState {
            tasks: RwLock::new(HashMap::new()),
            pending_confirmations: RwLock::new(HashMap::new()),
            default_room_id: config.default_room_id.clone(),
            skill_manager: None,
            matrix_client,
        });

        Self { config, state }
    }
    
    /// 创建新的 GLM API 服务端（带 SkillManager 和 Matrix Client）
    pub fn new_full(
        config: GlmApiConfig,
        skill_manager: Arc<crate::skill::SkillManager>,
        matrix_server: String,
        matrix_token: String,
        matrix_user: String,
    ) -> Self {
        let matrix_client = Some(MatrixHttpClient::new(
            matrix_server,
            matrix_token,
            matrix_user,
        ));
        
        let state = Arc::new(GlmApiState {
            tasks: RwLock::new(HashMap::new()),
            pending_confirmations: RwLock::new(HashMap::new()),
            default_room_id: config.default_room_id.clone(),
            skill_manager: Some(skill_manager),
            matrix_client,
        });

        Self { config, state }
    }

    /// 启动 HTTP 服务器
    pub async fn start(&self) -> anyhow::Result<()> {
        let app = create_router(self.state.clone(), self.config.clone());
        
        info!("GLM API server starting on {}", self.config.bind_addr);
        
        let listener = tokio::net::TcpListener::bind(self.config.bind_addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }

    /// 获取共享状态（用于外部操作）
    pub fn state(&self) -> Arc<GlmApiState> {
        self.state.clone()
    }
}

impl GlmApiState {
    /// 执行 DAG 转发逻辑（Task 4.1 & 4.2）
    /// 
    /// 逻辑：
    /// 1. 如果指定了 target_node，直接 HTTP POST 到目标节点
    /// 2. 如果没有指定，通过 Room 广播
    pub async fn execute_dag_forwarding(
        &self,
        dag: &crate::scheduler::DagSpec,
    ) -> anyhow::Result<DagPushResponse> {
        // 检查是否有指定的 target_node
        if let Some(ref target_node) = dag.target_node {
            // Task 4.1: HTTP 直接推送
            info!("Target node specified ({}), pushing DAG directly", target_node);
            
            // 构建目标节点客户端
            let mut client = TargetNodeClient::new();
            
            // 从配置或发现服务获取节点地址
            let node_url = format!("http://{}:7676", target_node); // 节点间通信端口
            client.register_node(target_node.clone(), node_url);
            
            match client.push_dag(target_node, dag).await {
                Ok(resp) => {
                    info!("DAG {} pushed to {} successfully", dag.dag_id, target_node);
                    return Ok(resp);
                }
                Err(e) => {
                    warn!("Failed to push to {}, falling back to broadcast: {}", target_node, e);
                    // 失败时回退到广播
                }
            }
        }
        
        // Task 4.2: Room 广播
        info!("No target node specified or push failed, broadcasting to room");
        
        let broadcast_client = RoomBroadcastClient::new(
            self.matrix_client.clone(),
            self.default_room_id.clone(),
        );
        
        let event_id = broadcast_client.broadcast_dag(dag, None).await?;
        
        Ok(DagPushResponse {
            run_id: format!("broadcast-{}", event_id),
            status: "broadcasted".to_string(),
            message: "DAG broadcasted to room for node claiming".to_string(),
        })
    }

    /// 创建新任务
    pub async fn create_task(
        &self,
        req: IssueTaskRequest,
        _user_id: &str,
    ) -> anyhow::Result<GlmTask> {
        let task_id = format!("glm_{}", &Uuid::new_v4().to_string().replace("-", "")[..16]);
        let now = chrono::Local::now().to_rfc3339();

        let task = GlmTask {
            task_id: task_id.clone(),
            task_type: req.task_type,
            target: req.target,
            args: req.args,
            status: TaskStatus::Pending,
            result: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
            target_node: req.target_node,
            room_id: req.room_id.or_else(|| Some(self.default_room_id.clone())),
        };

        self.tasks.write().await.insert(task_id, task.clone());

        // 发送任务到 Room（这里简化处理，实际应通过 Matrix 发送）
        info!("Task {} created for room {:?}", task.task_id, task.room_id);
        debug!("Task details: {:?}", task);

        Ok(task)
    }

    /// 获取任务状态
    pub async fn get_task_status(&self, task_id: &str) -> Option<TaskStatusResponse> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).map(|task| TaskStatusResponse {
            task_id: task.task_id.clone(),
            status: task.status.clone(),
            progress: Some(format!("{:?}", task.status)),
            result_data: task.result.clone(),
            error_message: task.error.clone(),
        })
    }

    /// 更新任务状态
    pub async fn update_task_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        result: Option<String>,
        error: Option<String>,
    ) -> anyhow::Result<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            task.status = status;
            task.result = result;
            task.error = error;
            task.updated_at = chrono::Local::now().to_rfc3339();
        }
        Ok(())
    }

    /// 创建待确认的 DAG
    pub async fn create_pending_dag(
        &self,
        req: PublishDagRequest,
        _user_id: &str,
    ) -> anyhow::Result<PendingDag> {
        let now = chrono::Local::now();
        let expires = now + chrono::Duration::seconds(300);

        let pending = PendingDag {
            dag_id: req.dag_id.clone(),
            description: req.description,
            tasks: req.tasks,
            schedule: req.schedule,
            created_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
            requested_by: _user_id.to_string(),
        };

        self.pending_confirmations
            .write()
            .await
            .insert(req.dag_id.clone(), pending.clone());

        Ok(pending)
    }

    /// 获取待确认 DAG
    pub async fn get_pending_dag(&self, dag_id: &str) -> Option<PendingDag> {
        self.pending_confirmations.read().await.get(dag_id).cloned()
    }

    /// 确认 DAG
    pub async fn confirm_dag(&self, dag_id: &str) -> anyhow::Result<Option<(PendingDag, String)>> {
        let removed = self.pending_confirmations.write().await.remove(dag_id);
        
        if let Some(ref dag) = removed {
            info!("DAG {} confirmed, publishing to room", dag_id);
            // 发布到 Matrix Room
            self.publish_dag_to_room(dag).await?;
            
            // 调用 dag-executor skill 执行 DAG
            let run_id = self.execute_dag_with_skill(dag).await?;
            info!("DAG {} execution started with run_id: {}", dag_id, run_id);
            
            return Ok(Some((dag.clone(), run_id)));
        }

        Ok(None)
    }
    
    /// 调用 dag-executor skill 执行 DAG
    async fn execute_dag_with_skill(&self, dag: &PendingDag) -> anyhow::Result<String> {
        use crate::scheduler::{DagSpec, DagTaskSpec};
        use crate::skill::Event;
        use serde_json::json;
        
        // 构建 DagSpec
        let tasks: Vec<DagTaskSpec> = dag.tasks.iter().map(|t| DagTaskSpec {
            id: t.id.clone(),
            task_type: t.task_type.clone(),
            command: t.command.clone(),
            depends_on: t.depends_on.clone(),
            env: std::collections::HashMap::new(),
        }).collect();
        
        let spec = DagSpec::new(dag.dag_id.clone(), tasks);
        
        // 如果有 SkillManager，发送事件到 dag-executor
        if let Some(ref skill_manager) = self.skill_manager {
            let event = Event::Custom {
                name: "dag:execute".to_string(),
                data: json!(spec),
            };
            
            skill_manager.send_event("dag-executor", event).await
                .map_err(|e| anyhow::anyhow!("Failed to send event to dag-executor: {}", e))?;
            
            // 生成 run_id（实际的 run_id 应该从 dag-executor 返回，这里本地生成）
            let run_id = format!("dag-run-{}-{}", dag.dag_id, uuid::Uuid::new_v4());
            Ok(run_id)
        } else {
            // 如果没有 SkillManager，返回错误
            error!("SkillManager not available, cannot execute DAG");
            Err(anyhow::anyhow!("SkillManager not available, DAG execution failed"))
        }
    }

    /// 发布 DAG 到 Room
    async fn publish_dag_to_room(
        &self,
        dag: &PendingDag,
    ) -> anyhow::Result<()> {
        let skillmd = format!(
            "!dag publish\n---\nid: {}\ndescription: {}\n{}tasks:\n{}",
            dag.dag_id,
            dag.description,
            dag.schedule.as_ref().map(|s| format!("schedule: \"{}\"\n", s)).unwrap_or_default(),
            dag.tasks.iter().map(|t| format!(
                "  - id: {}\n    type: {}\n    command: \"{}\"\n{}",
                t.id, t.task_type, t.command,
                if t.depends_on.is_empty() { "".to_string() }
                else { format!("    depends_on: {:?}\n", t.depends_on) }
            )).collect::<String>()
        );

        info!("Publishing DAG {} to room", dag.dag_id);
        debug!("SkillMD: {}", skillmd);
        
        // 通过 Matrix Room 发送
        if let Some(ref matrix_client) = self.matrix_client {
            let room_id = &self.default_room_id;
            match matrix_client.send_message(room_id, "m.text", &skillmd).await {
                Ok(event_id) => {
                    info!("DAG {} published to room {}, event_id: {}", dag.dag_id, room_id, event_id);
                }
                Err(e) => {
                    error!("Failed to publish DAG {} to room: {}", dag.dag_id, e);
                    return Err(e);
                }
            }
        } else {
            warn!("Matrix client not configured, DAG {} not sent to room", dag.dag_id);
        }
        
        Ok(())
    }

    /// 获取 DAG 状态
    pub async fn get_dag_status(&self, dag_id: &str) -> Option<DagStatusResponse> {
        // 简化处理，实际应从 scheduler 获取
        Some(DagStatusResponse {
            dag_id: dag_id.to_string(),
            status: "unknown".to_string(),
            last_run: None,
            completed_tasks: 0,
            total_tasks: 0,
            raw_data: json!({"note": "DAG status integration pending"}),
        })
    }

    /// 获取所有待确认列表
    pub async fn list_pending(&self) -> Vec<PendingDag> {
        self.pending_confirmations
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    /// 获取任务列表
    pub async fn list_tasks(
        &self,
        status_filter: Option<TaskStatus>,
        limit: usize,
    ) -> TaskListResponse {
        let tasks = self.tasks.read().await;
        let filtered: Vec<TaskInfo> = tasks
            .values()
            .filter(|t| status_filter.as_ref().map_or(true, |s| *s == t.status))
            .take(limit)
            .map(|t| TaskInfo {
                task_id: t.task_id.clone(),
                task_type: t.task_type.clone(),
                target: t.target.clone(),
                status: t.status.clone(),
                result: t.result.clone(),
                error: t.error.clone(),
                created_at: t.created_at.clone(),
                updated_at: t.updated_at.clone(),
                target_node: t.target_node.clone(),
            })
            .collect();

        TaskListResponse {
            count: filtered.len(),
            tasks: filtered,
        }
    }
}

/// 创建 Axum 路由
fn create_router(state: Arc<GlmApiState>, config: GlmApiConfig) -> Router {
    Router::new()
        .route("/health", get(health_check))
        // 任务相关 API
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/task/issue", post(issue_task))
        .route("/api/v1/task/:task_id/status", get(query_task_status))
        // DAG 相关 API
        .route("/api/v1/dag/publish", post(publish_dag))
        .route("/api/v1/dag/:dag_id/confirm", post(confirm_dag))
        .route("/api/v1/dag/:dag_id/status", get(query_dag_status))
        .route("/api/v1/dag/:dag_id/analyze", post(analyze_dag))
        .route("/api/v1/pending", get(list_pending))
        .layer(axum::middleware::from_fn_with_state(
            config.clone(),
            auth_middleware,
        ))
        .with_state(state)
}

/// 健康检查
async fn health_check() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "glm-api" }))
}

/// 获取任务列表
async fn list_tasks(
    State(state): State<Arc<GlmApiState>>,
    axum::extract::Query(req): axum::extract::Query<ListTasksRequest>,
) -> Json<TaskListResponse> {
    Json(state.list_tasks(req.status, req.limit).await)
}

/// 认证中间件 - Bearer DID
/// 
/// 使用 DID 作为 Bearer Token，与 CIS 节点间认证保持一致
/// Header: Authorization: Bearer did:cis:{node_id}:{pub_key_short}
async fn auth_middleware(
    State(config): State<GlmApiConfig>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // 从 Authorization header 提取 Bearer Token
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(auth) if auth.starts_with("Bearer ") => {
            let did = &auth[7..]; // 去掉 "Bearer " 前缀
            
            // 验证 DID 格式和是否在允许列表中
            if is_valid_did_format(did) && config.allowed_dids.contains(&did.to_string()) {
                Ok(next.run(req).await)
            } else {
                warn!("Invalid or unauthorized DID: {}", did);
                Err(StatusCode::UNAUTHORIZED)
            }
        }
        _ => {
            warn!("Missing or invalid Authorization header");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// 验证 DID 格式（did:cis:{node_id}:{pub_key_short}）
fn is_valid_did_format(did: &str) -> bool {
    // 基本格式检查
    if !did.starts_with("did:cis:") {
        return false;
    }
    
    let parts: Vec<&str> = did.split(':').collect();
    // 应为 did:cis:node_id:pub_key_short (4 部分)
    if parts.len() != 4 {
        return false;
    }
    
    // node_id 和 pub_key_short 不应为空
    if parts[2].is_empty() || parts[3].is_empty() {
        return false;
    }
    
    true
}

/// 发布任务
async fn issue_task(
    State(state): State<Arc<GlmApiState>>,
    axum::extract::Json(req): axum::extract::Json<IssueTaskRequest>,
) -> Result<Json<IssueTaskResponse>, StatusCode> {
    // 从 header 获取 user_id（已由中间件验证）
    let user_id = "glm_cloud_user";

    match state.create_task(req, user_id).await {
        Ok(task) => Ok(Json(IssueTaskResponse {
            task_id: task.task_id,
            status: task.status,
        })),
        Err(e) => {
            error!("Failed to create task: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 查询任务状态
async fn query_task_status(
    State(state): State<Arc<GlmApiState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskStatusResponse>, StatusCode> {
    match state.get_task_status(&task_id).await {
        Some(resp) => Ok(Json(resp)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// 发布 DAG（等待确认）
async fn publish_dag(
    State(state): State<Arc<GlmApiState>>,
    axum::extract::Json(req): axum::extract::Json<PublishDagRequest>,
) -> Result<Json<PublishDagResponse>, StatusCode> {
    let user_id = "glm_cloud_user";

    match state.create_pending_dag(req, user_id).await {
        Ok(pending) => Ok(Json(PublishDagResponse {
            dag_id: pending.dag_id.clone(),
            status: "waiting_confirmation".to_string(),
            confirm_url: format!("/api/v1/dag/{}/confirm", pending.dag_id),
            expire_sec: 300,
        })),
        Err(e) => {
            error!("Failed to create pending DAG: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 确认 DAG
async fn confirm_dag(
    State(state): State<Arc<GlmApiState>>,
    Path(dag_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match state.confirm_dag(&dag_id).await {
        Ok(Some((pending, run_id))) => Ok(Json(json!({
            "success": true,
            "dag_id": dag_id,
            "message": "DAG confirmed and published",
            "tasks_count": pending.tasks.len(),
            "run_id": run_id,
        }))),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to confirm DAG: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// 查询 DAG 状态
async fn query_dag_status(
    State(state): State<Arc<GlmApiState>>,
    Path(dag_id): Path<String>,
) -> Result<Json<DagStatusResponse>, StatusCode> {
    match state.get_dag_status(&dag_id).await {
        Some(resp) => Ok(Json(resp)),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// 分析 DAG 卡点
async fn analyze_dag(
    Path(dag_id): Path<String>,
    axum::extract::Json(req): axum::extract::Json<AnalyzeDagRequest>,
) -> Result<Json<Value>, StatusCode> {
    // 简化实现：返回诊断模板
    let diagnostic = json!({
        "dag_id": dag_id,
        "stuck_task_hint": req.stuck_task_hint,
        "analysis": "DAG analysis integration pending",
        "suggestions": [
            "Check task logs in ~/.cis/logs/",
            "Verify task dependencies",
            "Check resource availability"
        ],
    });

    Ok(Json(diagnostic))
}

/// 列出待确认任务
async fn list_pending(
    State(state): State<Arc<GlmApiState>>,
) -> Json<Value> {
    let pending = state.list_pending().await;
    Json(json!({
        "count": pending.len(),
        "pending": pending,
    }))
}

/// 启动 GLM API 服务的便捷函数
pub async fn start_glm_api_service(
    config: GlmApiConfig,
) -> anyhow::Result<()> {
    let server = GlmApiServer::new(config);
    server.start().await
}
