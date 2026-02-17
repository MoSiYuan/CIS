//! # Federated Agent
//!
//! 支持跨节点通信的联邦 Agent 实现。
//!
//! ## 功能特性
//! - 包装本地 Agent，支持联邦通信
//! - 远程 Agent 代理（无本地实例）
//! - 通过 Matrix Federation 发送/接收事件
//! - 心跳和状态管理
//!
//! ## 使用示例
//! ```rust,ignore
//! use cis_core::agent::federation::{FederatedAgent, FederatedRuntime};
//! use cis_core::agent::persistent::{AgentConfig, AgentRuntime};
//!
//! // 创建联邦 Agent（包装本地 Agent）
//! let local_agent = runtime.create_agent(config).await?;
//! let federated = FederatedAgent::wrap_local(
//!     local_agent,
//!     matrix_client,
//!     room_id,
//! ).await?;
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, RwLock};
use tracing::{debug, error, info, warn};

use crate::agent::federation::protocol::{
    AgentAddress, AgentFederationEvent, TaskRequestPayload, TaskResultPayload,
};
use crate::agent::persistent::{
    AgentConfig, AgentInfo, AgentRuntime, AgentStatus, PersistentAgent, RuntimeType, TaskRequest,
    TaskResult,
};
use crate::error::{CisError, Result};
use crate::matrix::federation::{CisMatrixEvent, FederationClient, PeerInfo};

/// 联邦 Agent
///
/// 包装本地 Agent，支持跨节点通信。可以作为本地 Agent 的联邦包装器，
/// 也可以作为远程 Agent 的代理（无本地实例）。
pub struct FederatedAgent {
    /// 本地 Agent 实例（如果存在）
    local_agent: Option<Box<dyn PersistentAgent>>,

    /// Agent 地址
    address: AgentAddress,

    /// Matrix 客户端（用于跨节点通信）
    matrix_client: Arc<FederationClient>,

    /// Room ID（用于联邦通信）
    room_id: String,

    /// 状态
    state: Arc<RwLock<FederatedAgentState>>,

    /// 待处理请求表
    pending_requests: Arc<RwLock<HashMap<String, PendingRequest>>>,
    
    /// 已知对等节点列表
    peers: Arc<RwLock<Vec<PeerInfo>>>,
}

/// 联邦 Agent 内部状态
#[derive(Debug, Clone)]
struct FederatedAgentState {
    /// 当前状态
    status: AgentStatus,
    /// 最后心跳时间
    last_heartbeat: DateTime<Utc>,
    /// 远程 Agent 表（agent_id -> node_id）
    remote_agents: HashMap<String, String>,
}

/// 待处理请求
struct PendingRequest {
    /// 请求 ID
    request_id: String,
    /// 发送时间
    sent_at: DateTime<Utc>,
    /// 响应发送通道
    response_tx: oneshot::Sender<TaskResultPayload>,
}

impl FederatedAgent {
    /// 创建联邦 Agent（包装本地 Agent）
    ///
    /// # Arguments
    /// * `local_agent` - 本地 Agent 实例
    /// * `matrix_client` - Matrix 联邦客户端
    /// * `room_id` - 联邦通信 Room ID
    ///
    /// # Returns
    /// 新创建的 FederatedAgent 实例
    pub async fn wrap_local(
        local_agent: Box<dyn PersistentAgent>,
        matrix_client: Arc<FederationClient>,
        room_id: String,
    ) -> Result<Self> {
        // 从环境变量或 hostname 获取节点 ID
        let agent_id = local_agent.agent_id().to_string();
        let node_id = std::env::var("CIS_NODE_ID")
            .or_else(|_| gethostname::gethostname().into_string())
            .unwrap_or_else(|_| "local".to_string());
        let address = AgentAddress::new(agent_id, node_id);

        let agent = Self {
            local_agent: Some(local_agent),
            address,
            matrix_client,
            room_id,
            state: Arc::new(RwLock::new(FederatedAgentState {
                status: AgentStatus::Running,
                last_heartbeat: Utc::now(),
                remote_agents: HashMap::new(),
            })),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            peers: Arc::new(RwLock::new(Vec::new())),
        };

        // 发送注册事件
        agent.send_registration().await?;

        // 启动心跳
        agent.start_heartbeat();

        // 启动事件监听
        agent.start_event_listener();

        Ok(agent)
    }

    /// 创建远程 Agent 代理（无本地实例）
    ///
    /// # Arguments
    /// * `agent_id` - 远程 Agent ID
    /// * `node_id` - 远程节点 ID
    /// * `matrix_client` - Matrix 联邦客户端
    /// * `room_id` - 联邦通信 Room ID
    ///
    /// # Returns
    /// 新创建的 FederatedAgent 实例（作为远程代理）
    pub async fn remote_proxy(
        agent_id: String,
        node_id: String,
        matrix_client: Arc<FederationClient>,
        room_id: String,
    ) -> Result<Self> {
        let address = AgentAddress::new(agent_id, node_id);

        Ok(Self {
            local_agent: None,
            address,
            matrix_client,
            room_id,
            state: Arc::new(RwLock::new(FederatedAgentState {
                status: AgentStatus::Idle,
                last_heartbeat: Utc::now(),
                remote_agents: HashMap::new(),
            })),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            peers: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// 获取 Agent 地址
    pub fn address(&self) -> &AgentAddress {
        &self.address
    }

    /// 检查是否为本地 Agent
    pub fn is_local(&self) -> bool {
        self.local_agent.is_some()
    }

    /// 发送联邦事件到所有已知对等节点
    async fn send_federation_event(&self, event: AgentFederationEvent) -> Result<()> {
        // 转换为 CisMatrixEvent
        let matrix_event = CisMatrixEvent::new(
            format!("${}", uuid::Uuid::new_v4()),
            self.room_id.clone(),
            format!("@{}:{}", self.address.agent_id, self.address.node_id),
            event.event_type_str().to_string(),
            serde_json::to_value(&event)?,
        );

        // 获取当前已知的对等节点
        let peers = self.peers.read().await.clone();
        
        if peers.is_empty() {
            debug!("No peers to send federation event to");
            return Ok(());
        }

        // 向所有对等节点发送事件
        let mut sent = 0;
        let mut failed = 0;
        
        for peer in peers {
            match self.matrix_client.send_event(&peer, &matrix_event).await {
                Ok(response) => {
                    if response.accepted {
                        debug!("Event sent to {}: accepted", peer.server_name);
                        sent += 1;
                    } else {
                        warn!("Event rejected by {}: {:?}", peer.server_name, response.error);
                        failed += 1;
                    }
                }
                Err(e) => {
                    warn!("Failed to send event to {}: {}", peer.server_name, e);
                    failed += 1;
                }
            }
        }
        
        debug!(
            "Federation event {:?} sent to {}/{} peers ({} failed)",
            event.event_type_str(),
            sent,
            sent + failed,
            failed
        );
        
        Ok(())
    }
    
    /// 添加对等节点
    pub async fn add_peer(&self, peer: PeerInfo) {
        let mut peers = self.peers.write().await;
        // 检查是否已存在
        if !peers.iter().any(|p| p.server_name == peer.server_name) {
            info!("Adding peer: {} at {}:{}", peer.server_name, peer.host, peer.port);
            peers.push(peer);
        }
    }
    
    /// 移除对等节点
    pub async fn remove_peer(&self, server_name: &str) {
        let mut peers = self.peers.write().await;
        peers.retain(|p| p.server_name != server_name);
    }
    
    /// 获取已知对等节点列表
    pub async fn list_peers(&self) -> Vec<PeerInfo> {
        self.peers.read().await.clone()
    }

    /// 发送注册事件
    async fn send_registration(&self) -> Result<()> {
        let runtime_type = self
            .local_agent
            .as_ref()
            .map(|a| a.runtime_type())
            .unwrap_or(RuntimeType::Claude);

        let event = AgentFederationEvent::AgentRegistered {
            agent_id: self.address.agent_id.clone(),
            node_id: self.address.node_id.clone(),
            runtime_type,
            capabilities: vec!["task_execution".to_string(), "messaging".to_string()],
            timestamp: Utc::now(),
        };

        self.send_federation_event(event).await
    }

    /// 发送注销事件
    async fn send_unregistration(&self) -> Result<()> {
        let event = AgentFederationEvent::AgentUnregistered {
            agent_id: self.address.agent_id.clone(),
            node_id: self.address.node_id.clone(),
            reason: Some("Shutdown requested".to_string()),
            timestamp: Utc::now(),
        };

        self.send_federation_event(event).await
    }

    /// 启动心跳
    fn start_heartbeat(&self) {
        let state = Arc::clone(&self.state);
        let matrix_client = Arc::clone(&self.matrix_client);
        let peers = Arc::clone(&self.peers);
        let room_id = self.room_id.clone();
        let address = self.address.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let status = state.read().await.status.clone();

                let event = AgentFederationEvent::Heartbeat {
                    agent_id: address.agent_id.clone(),
                    node_id: address.node_id.clone(),
                    status,
                    timestamp: Utc::now(),
                };

                // 创建 Matrix 事件
                let matrix_event = CisMatrixEvent::new(
                    format!("${}", uuid::Uuid::new_v4()),
                    room_id.clone(),
                    format!("@{}:{}", address.agent_id, address.node_id),
                    event.event_type_str().to_string(),
                    match serde_json::to_value(&event) {
                        Ok(v) => v,
                        Err(e) => {
                            error!("Failed to serialize heartbeat event: {}", e);
                            continue;
                        }
                    },
                );

                debug!(
                    "Sending heartbeat for agent {}@{}",
                    address.agent_id, address.node_id
                );

                // 通过 FederationClient 发送心跳到所有对等节点
                let peer_list = peers.read().await.clone();
                for peer in peer_list {
                    let matrix_event = CisMatrixEvent::new(
                        format!("${}", uuid::Uuid::new_v4()),
                        room_id.clone(),
                        format!("@{}:{}", address.agent_id, address.node_id),
                        matrix_event.event_type.clone(),
                        matrix_event.content.clone(),
                    );
                    
                    match matrix_client.send_event(&peer, &matrix_event).await {
                        Ok(response) => {
                            if response.accepted {
                                debug!("Heartbeat sent to {}: accepted", peer.server_name);
                            } else {
                                debug!("Heartbeat rejected by {}: {:?}", peer.server_name, response.error);
                            }
                        }
                        Err(e) => {
                            debug!("Failed to send heartbeat to {}: {}", peer.server_name, e);
                        }
                    }
                }

                // 更新最后心跳时间
                state.write().await.last_heartbeat = Utc::now();
            }
        });
    }

    /// 启动事件监听
    fn start_event_listener(&self) {
        let state = Arc::clone(&self.state);
        let pending_requests = Arc::clone(&self.pending_requests);
        let local_agent = self.local_agent.is_some();
        let address = self.address.clone();

        tokio::spawn(async move {
            info!(
                "Starting event listener for federated agent {}@{}",
                address.agent_id, address.node_id
            );

            // 注意：实际的 Matrix Room 事件订阅需要在 FederationManager 或 Matrix Nucleus 中实现
            // 这里我们启动一个后台任务来处理超时检查

            loop {
                // 每秒检查一次超时
                tokio::time::sleep(Duration::from_secs(1)).await;

                // 处理待处理请求的超时
                let now = Utc::now();
                let mut pending = pending_requests.write().await;
                let timed_out: Vec<String> = pending
                    .iter()
                    .filter(|(_, req)| (now - req.sent_at).num_seconds() > 300)
                    .map(|(id, _)| id.clone())
                    .collect();

                for request_id in timed_out {
                    if let Some(req) = pending.remove(&request_id) {
                        let _ = req.response_tx.send(TaskResultPayload::error(
                            "Request timeout".to_string(),
                            -1,
                        ));
                    }
                }
                drop(pending); // 释放锁

                // 检查 Agent 状态
                let current_status = state.read().await.status.clone();
                if current_status == AgentStatus::Shutdown {
                    info!("Agent is shutdown, stopping event listener");
                    break;
                }
            }
        });
    }
    
    /// 处理接收到的联邦事件
    /// 
    /// 此方法由外部事件源（如 FederationManager）调用，将事件传递给 Agent 处理
    pub async fn process_incoming_event(&self, event: AgentFederationEvent) -> Result<()> {
        debug!("Processing incoming event: {:?}", event.event_type_str());
        
        match &event {
            AgentFederationEvent::Heartbeat { agent_id, node_id, status, .. } => {
                debug!("Received heartbeat from {}@{} (status: {:?})", agent_id, node_id, status);
                // 更新远程 Agent 状态
                let mut state = self.state.write().await;
                state.remote_agents.insert(agent_id.clone(), node_id.clone());
            }
            AgentFederationEvent::AgentRegistered { agent_id, node_id, .. } => {
                info!("Agent {}@{} registered", agent_id, node_id);
                let mut state = self.state.write().await;
                state.remote_agents.insert(agent_id.clone(), node_id.clone());
            }
            AgentFederationEvent::AgentUnregistered { agent_id, node_id, .. } => {
                info!("Agent {}@{} unregistered", agent_id, node_id);
                let mut state = self.state.write().await;
                state.remote_agents.remove(agent_id);
            }
            AgentFederationEvent::TaskRequest { .. } => {
                // 处理任务请求（仅本地 Agent 模式）
                if self.local_agent.is_some() {
                    if let Err(e) = self.handle_task_request(event).await {
                        error!("Failed to handle task request: {}", e);
                    }
                }
            }
            AgentFederationEvent::TaskResponse { request_id, result, .. } => {
                // 处理任务响应
                let mut pending = self.pending_requests.write().await;
                if let Some(req) = pending.remove(request_id) {
                    let _ = req.response_tx.send(result.clone());
                }
            }
            AgentFederationEvent::Message { from_agent, payload, .. } => {
                info!("Received message from {}: {:?}", from_agent, payload);
                // 可以在这里添加消息处理逻辑
            }
            AgentFederationEvent::StatusUpdate { agent_id, node_id, status, .. } => {
                debug!("Status update from {}@{}: {:?}", agent_id, node_id, status);
                if agent_id == &self.address.agent_id && node_id == &self.address.node_id {
                    // 忽略自己的状态更新
                } else {
                    let mut state = self.state.write().await;
                    state.remote_agents.insert(agent_id.clone(), node_id.clone());
                }
            }
        }
        
        Ok(())
    }

    /// 处理收到的任务请求（本地 Agent 模式）
    async fn handle_task_request(&self, event: AgentFederationEvent) -> Result<()> {
        match event {
            AgentFederationEvent::TaskRequest {
                request_id,
                from_agent,
                to_agent,
                task,
                timeout_secs,
                ..
            } => {
                // 检查是否是发给本 Agent 的
                let target_addr = format!("{}@{}", self.address.agent_id, self.address.node_id);
                if to_agent != target_addr {
                    return Ok(());
                }

                info!(
                    "Received task request {} from {} to {}",
                    request_id, from_agent, to_agent
                );

                // 转换为 TaskRequest
                let task_request = TaskRequest {
                    task_id: task.task_id,
                    prompt: task.prompt,
                    work_dir: None,
                    files: Vec::new(),
                    context: {
                        let mut ctx = HashMap::new();
                        if !task.context.is_empty() {
                            ctx.insert("context".to_string(), serde_json::json!(task.context));
                        }
                        ctx
                    },
                    timeout_secs,
                };

                // 如果有本地 Agent，执行任务
                if let Some(ref local) = self.local_agent {
                    match local.execute(task_request).await {
                        Ok(result) => {
                            // 发送任务响应
                            let response = AgentFederationEvent::TaskResponse {
                                request_id: request_id.clone(),
                                from_agent: target_addr.clone(),
                                to_agent: from_agent,
                                result: TaskResultPayload {
                                    success: result.success,
                                    output: result.output.unwrap_or_default(),
                                    exit_code: if result.success { 0 } else { 1 },
                                    metadata: result.metadata,
                                },
                                timestamp: Utc::now(),
                            };
                            self.send_federation_event(response).await?;
                        }
                        Err(e) => {
                            // 发送错误响应
                            let response = AgentFederationEvent::TaskResponse {
                                request_id: request_id.clone(),
                                from_agent: target_addr,
                                to_agent: from_agent,
                                result: TaskResultPayload::error(e.to_string(), -1),
                                timestamp: Utc::now(),
                            };
                            self.send_federation_event(response).await?;
                        }
                    }
                } else {
                    warn!("Received task request but no local agent available");
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// 处理收到的任务响应（远程代理模式）
    async fn handle_task_response(&self, event: AgentFederationEvent) -> Result<()> {
        match event {
            AgentFederationEvent::TaskResponse {
                request_id,
                result,
                ..
            } => {
                let mut pending = self.pending_requests.write().await;
                if let Some(req) = pending.remove(&request_id) {
                    let _ = req.response_tx.send(result);
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// 更新远程 Agent 注册信息
    async fn update_remote_agent(&self, agent_id: String, node_id: String) {
        let mut state = self.state.write().await;
        state.remote_agents.insert(agent_id, node_id);
    }

    /// 移除远程 Agent 注册信息
    async fn remove_remote_agent(&self, agent_id: &str) {
        let mut state = self.state.write().await;
        state.remote_agents.remove(agent_id);
    }
}

#[async_trait]
impl PersistentAgent for FederatedAgent {
    fn agent_id(&self) -> &str {
        &self.address.agent_id
    }

    fn runtime_type(&self) -> RuntimeType {
        // 联邦 Agent 使用 Claude 作为基础类型（如果有本地 Agent）
        self.local_agent
            .as_ref()
            .map(|a| a.runtime_type())
            .unwrap_or(RuntimeType::Claude)
    }

    async fn execute(&self, task: TaskRequest) -> Result<TaskResult> {
        // 如果是本地 Agent，直接执行
        if let Some(ref local) = self.local_agent {
            return local.execute(task).await;
        }

        // 否则，通过 Matrix 发送任务请求
        let request_id = format!("req-{}", uuid::Uuid::new_v4());

        // 创建响应通道
        let (tx, rx) = oneshot::channel();

        // 注册待处理请求
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(
                request_id.clone(),
                PendingRequest {
                    request_id: request_id.clone(),
                    sent_at: Utc::now(),
                    response_tx: tx,
                },
            );
        }

        // 构建上下文字符串
        let context_str = task
            .context
            .get("context")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 发送任务请求事件
        let event = AgentFederationEvent::TaskRequest {
            request_id: request_id.clone(),
            from_agent: self.address.to_string(),
            to_agent: format!("{}@{}", self.address.agent_id, self.address.node_id),
            task: TaskRequestPayload {
                task_id: task.task_id.clone(),
                prompt: task.prompt,
                context: context_str,
                system_prompt: None, // 从 task.context 或单独字段获取
                model: None,
                metadata: task.context,
            },
            timeout_secs: task.timeout_secs,
            timestamp: Utc::now(),
        };

        self.send_federation_event(event).await?;

        // 等待响应（带超时），并计算执行时间
        let timeout_duration = Duration::from_secs(task.timeout_secs.unwrap_or(300));
        let start_time = std::time::Instant::now();
        
        match tokio::time::timeout(timeout_duration, rx).await {
            Ok(Ok(result)) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                Ok(TaskResult {
                    task_id: task.task_id,
                    success: result.success,
                    output: Some(result.output),
                    error: None,
                    duration_ms,
                    completed_at: Utc::now(),
                    metadata: result.metadata,
                })
            }
            Ok(Err(_)) => Err(CisError::execution("Response channel closed")),
            Err(_) => {
                // 超时，清理待处理请求
                self.pending_requests.write().await.remove(&request_id);
                Err(CisError::execution("Task execution timeout"))
            }
        }
    }

    async fn status(&self) -> AgentStatus {
        // 优先使用本地 Agent 状态
        if let Some(ref local) = self.local_agent {
            return local.status().await;
        }

        // 否则使用缓存的远程状态
        let state = self.state.read().await;
        state.status.clone()
    }

    async fn attach(&self) -> Result<()> {
        if let Some(ref local) = self.local_agent {
            return local.attach().await;
        }

        Err(CisError::execution("Cannot attach to remote agent"))
    }

    async fn detach(&self) -> Result<()> {
        if let Some(ref local) = self.local_agent {
            return local.detach().await;
        }

        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        // 发送注销事件
        self.send_unregistration().await?;

        // 关闭本地 Agent
        if let Some(ref local) = self.local_agent {
            return local.shutdown().await;
        }

        Ok(())
    }
}

/// 联邦 Runtime
///
/// 负责管理和创建联邦 Agent，支持本地 Agent 的联邦包装和远程 Agent 代理。
pub struct FederatedRuntime {
    matrix_client: Arc<FederationClient>,
    room_id: String,
    local_agents: Arc<RwLock<HashMap<String, FederatedAgent>>>,
}

impl FederatedRuntime {
    /// 创建新的 FederatedRuntime
    ///
    /// # Arguments
    /// * `matrix_client` - Matrix 联邦客户端
    /// * `room_id` - 联邦通信 Room ID
    pub fn new(matrix_client: Arc<FederationClient>, room_id: impl Into<String>) -> Self {
        Self {
            matrix_client,
            room_id: room_id.into(),
            local_agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取 Matrix 客户端
    pub fn matrix_client(&self) -> &FederationClient {
        &self.matrix_client
    }

    /// 获取 Room ID
    pub fn room_id(&self) -> &str {
        &self.room_id
    }

    /// 创建远程 Agent 代理
    ///
    /// # Arguments
    /// * `agent_id` - 远程 Agent ID
    /// * `node_id` - 远程节点 ID
    ///
    /// # Returns
    /// 远程 Agent 的联邦代理
    pub async fn create_remote_proxy(
        &self,
        agent_id: impl Into<String>,
        node_id: impl Into<String>,
    ) -> Result<FederatedAgent> {
        FederatedAgent::remote_proxy(
            agent_id.into(),
            node_id.into(),
            Arc::clone(&self.matrix_client),
            self.room_id.clone(),
        )
        .await
    }
}

#[async_trait]
impl AgentRuntime for FederatedRuntime {
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Claude // 联邦 Runtime 以 Claude 为基础类型
    }

    async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn PersistentAgent>> {
        use crate::agent::persistent::{ClaudePersistentAgent, OpenCodePersistentAgent};

        // 1. 创建本地 Agent
        let local_agent: Box<dyn PersistentAgent> = match self.runtime_type() {
            RuntimeType::Claude => {
                // 注意：这里需要一个静态的 SessionManager 引用
                // 实际实现中应该通过依赖注入获取
                Box::new(
                    ClaudePersistentAgent::start(
                        crate::agent::cluster::SessionManager::global(),
                        config,
                    )
                    .await?,
                )
            }
            RuntimeType::OpenCode => Box::new(OpenCodePersistentAgent::start(config).await?),
            _ => {
                return Err(CisError::invalid_input(
                    "Unsupported runtime for federation",
                ))
            }
        };

        // 2. 包装为 FederatedAgent
        let federated = FederatedAgent::wrap_local(
            local_agent,
            Arc::clone(&self.matrix_client),
            self.room_id.clone(),
        )
        .await?;

        // 3. 保存到本地表
        let agent_id = federated.agent_id().to_string();
        let agent_id_for_proxy = agent_id.clone();
        self.local_agents
            .write()
            .await
            .insert(agent_id, federated);

        // 返回包装后的 Agent
        // 注意：由于 FederatedAgent 包含 RwLock，我们需要返回一个 Arc 包装
        // 但 PersistentAgent trait 要求返回 Box，这里需要特殊处理
        // 暂时返回第一个创建的 Agent 的克隆引用
        let agents = self.local_agents.read().await;
        let agent = agents
            .get(&agent_id_for_proxy)
            .ok_or_else(|| CisError::execution("Failed to retrieve created agent"))?;

        // 由于无法直接克隆 FederatedAgent，我们创建一个新的远程代理
        // 实际使用时应该通过 Arc 共享
        let proxy = FederatedAgent::remote_proxy(
            agent.agent_id().to_string(),
            agent.address.node_id.clone(),
            Arc::clone(&self.matrix_client),
            self.room_id.clone(),
        )
        .await?;

        Ok(Box::new(proxy))
    }

    async fn list_agents(&self) -> Vec<AgentInfo> {
        // 返回本地联邦 Agent + 已发现的远程 Agent
        let agents = self.local_agents.read().await;
        let mut result = Vec::new();

        for (id, agent) in agents.iter() {
            let status = agent.status().await;
            result.push(
                AgentInfo::new(id.clone(), id.clone(), std::env::temp_dir())
                    .with_runtime_type(agent.runtime_type())
                    .with_status(status),
            );
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_federated_agent_address() {
        let address = AgentAddress::new("agent-1", "node-a");
        assert_eq!(address.agent_id, "agent-1");
        assert_eq!(address.node_id, "node-a");
        assert_eq!(address.to_string(), "agent-1@node-a");
    }

    #[test]
    fn test_federated_agent_state() {
        let state = FederatedAgentState {
            status: AgentStatus::Running,
            last_heartbeat: Utc::now(),
            remote_agents: HashMap::new(),
        };
        assert_eq!(state.status, AgentStatus::Running);
    }

    // 注意：FederatedAgent 的完整测试需要模拟 Matrix 客户端
    // 这些测试应该在集成测试环境中进行
}
