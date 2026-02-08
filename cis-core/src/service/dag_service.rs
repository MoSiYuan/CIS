//! # DAG Service
//!
//! DAG 管理服务，提供 DAG 的创建、执行、查询等功能。

use super::{ListOptions, PaginatedResult};
use crate::error::{CisError, Result};
use crate::storage::{DbManager};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// DAG 摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagSummary {
    pub id: String,
    pub name: String,
    pub version: String,
    pub status: DagStatus,
    pub scope: DagScope,
    pub tasks_count: usize,
    pub created_at: DateTime<Utc>,
    pub last_run: Option<DateTime<Utc>>,
}

impl From<crate::storage::DagRecord> for DagSummary {
    fn from(record: crate::storage::DagRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            version: record.version,
            status: record.status.parse().unwrap_or(DagStatus::Draft),
            scope: record.scope.parse().unwrap_or(DagScope::User),
            tasks_count: record.tasks_count,
            created_at: Utc.timestamp_opt(record.created_at, 0).single().unwrap_or_else(Utc::now),
            last_run: record.last_run_at.and_then(|t| Utc.timestamp_opt(t, 0).single()),
        }
    }
}

/// DAG 详情
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagInfo {
    #[serde(flatten)]
    pub summary: DagSummary,
    pub description: String,
    pub definition: serde_json::Value,
    pub owner: String,
    pub permissions: Vec<String>,
    pub config: HashMap<String, serde_json::Value>,
}

impl TryFrom<crate::storage::DagDetail> for DagInfo {
    type Error = CisError;

    fn try_from(detail: crate::storage::DagDetail) -> Result<Self> {
        let definition: serde_json::Value = serde_json::from_str(&detail.definition)
            .map_err(|e| CisError::serialization(format!("Invalid DAG definition JSON: {}", e)))?;
        let permissions: Vec<String> = serde_json::from_str(&detail.permissions)
            .unwrap_or_default();
        let config: HashMap<String, serde_json::Value> = serde_json::from_str(&detail.config)
            .unwrap_or_default();

        Ok(Self {
            summary: DagSummary {
                id: detail.id,
                name: detail.name,
                version: detail.version,
                status: detail.status.parse().unwrap_or(DagStatus::Draft),
                scope: detail.scope.parse().unwrap_or(DagScope::User),
                tasks_count: detail.tasks_count,
                created_at: Utc.timestamp_opt(detail.created_at, 0).single().unwrap_or_else(Utc::now),
                last_run: detail.last_run_at.and_then(|t| Utc.timestamp_opt(t, 0).single()),
            },
            description: detail.description.unwrap_or_default(),
            definition,
            owner: detail.owner,
            permissions,
            config,
        })
    }
}

/// DAG 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DagStatus {
    Draft,
    Active,
    Paused,
    Deprecated,
}

impl std::fmt::Display for DagStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagStatus::Draft => write!(f, "draft"),
            DagStatus::Active => write!(f, "active"),
            DagStatus::Paused => write!(f, "paused"),
            DagStatus::Deprecated => write!(f, "deprecated"),
        }
    }
}

impl std::str::FromStr for DagStatus {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(DagStatus::Draft),
            "active" => Ok(DagStatus::Active),
            "paused" => Ok(DagStatus::Paused),
            "deprecated" => Ok(DagStatus::Deprecated),
            _ => Err(()),
        }
    }
}

/// DAG 作用域
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DagScope {
    System,
    Global,
    Project,
    User,
    Node,
}

impl std::fmt::Display for DagScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DagScope::System => write!(f, "system"),
            DagScope::Global => write!(f, "global"),
            DagScope::Project => write!(f, "project"),
            DagScope::User => write!(f, "user"),
            DagScope::Node => write!(f, "node"),
        }
    }
}

impl std::str::FromStr for DagScope {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "system" => Ok(DagScope::System),
            "global" => Ok(DagScope::Global),
            "project" => Ok(DagScope::Project),
            "user" => Ok(DagScope::User),
            "node" => Ok(DagScope::Node),
            _ => Err(()),
        }
    }
}

/// DAG 运行信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagRun {
    pub run_id: String,
    pub dag_id: String,
    pub status: RunStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub tasks_completed: usize,
    pub tasks_failed: usize,
    pub tasks_total: usize,
}

impl From<crate::storage::DagRunRecord> for DagRun {
    fn from(record: crate::storage::DagRunRecord) -> Self {
        Self {
            run_id: record.run_id,
            dag_id: record.dag_id,
            status: record.status.parse().unwrap_or(RunStatus::Pending),
            started_at: Utc.timestamp_opt(record.started_at, 0).single().unwrap_or_else(Utc::now),
            finished_at: record.finished_at.and_then(|t| Utc.timestamp_opt(t, 0).single()),
            tasks_completed: record.tasks_completed,
            tasks_failed: record.tasks_failed,
            tasks_total: record.tasks_total,
        }
    }
}

/// 运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunStatus::Pending => write!(f, "pending"),
            RunStatus::Running => write!(f, "running"),
            RunStatus::Success => write!(f, "success"),
            RunStatus::Failed => write!(f, "failed"),
            RunStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for RunStatus {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(RunStatus::Pending),
            "running" => Ok(RunStatus::Running),
            "success" => Ok(RunStatus::Success),
            "failed" => Ok(RunStatus::Failed),
            "cancelled" => Ok(RunStatus::Cancelled),
            _ => Err(()),
        }
    }
}

/// DAG 服务
#[derive(Debug, Clone)]
pub struct DagService {
    db_manager: Arc<DbManager>,
}

impl DagService {
    pub fn new() -> Result<Self> {
        let db_manager = Arc::new(DbManager::new()?);
        Ok(Self { db_manager })
    }

    pub fn with_db_manager(db_manager: Arc<DbManager>) -> Self {
        Self { db_manager }
    }

    /// 列出 DAG
    pub async fn list(&self, options: ListOptions) -> Result<PaginatedResult<DagSummary>> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        let records = core.list_dags(options.all, options.limit)?;
        let total = records.len();
        let items: Vec<DagSummary> = records.into_iter().map(DagSummary::from).collect();

        Ok(PaginatedResult::new(items, total))
    }

    /// 查看 DAG 详情
    pub async fn inspect(&self, id: &str) -> Result<DagInfo> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        match core.get_dag(id)? {
            Some(detail) => DagInfo::try_from(detail),
            None => Err(CisError::not_found(format!("DAG '{}' not found", id))),
        }
    }

    /// 创建 DAG
    pub async fn create(&self, name: &str, definition: serde_json::Value) -> Result<DagInfo> {
        let definition_str = serde_json::to_string(&definition)
            .map_err(|e| CisError::serialization(format!("Failed to serialize definition: {}", e)))?;

        // 生成唯一 ID
        let id = format!("dag_{}_{}", 
            name.replace(" ", "_").replace("-", "_"),
            Utc::now().timestamp_millis()
        );

        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        // 检查名称是否已存在
        if core.get_dag_by_name(name)?.is_some() {
            return Err(CisError::already_exists(format!("DAG with name '{}' already exists", name)));
        }

        core.create_dag(&id, name, &definition_str, None)?;
        
        // 添加创建日志
        core.add_dag_log(&id, None, "info", &format!("DAG '{}' created", name))?;

        // 返回创建的 DAG
        match core.get_dag(&id)? {
            Some(detail) => DagInfo::try_from(detail),
            None => Err(CisError::storage("Failed to retrieve created DAG".to_string())),
        }
    }

    /// 运行 DAG
    pub async fn run(&self, id: &str, params: HashMap<String, String>) -> Result<DagRun> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        // 检查 DAG 是否存在
        let dag = match core.get_dag(id)? {
            Some(d) => d,
            None => return Err(CisError::not_found(format!("DAG '{}' not found", id))),
        };

        // 检查 DAG 状态
        if dag.status == "paused" {
            return Err(CisError::invalid_input(format!("DAG '{}' is paused", id)));
        }
        if dag.status == "deprecated" {
            return Err(CisError::invalid_input(format!("DAG '{}' is deprecated", id)));
        }

        // 生成运行 ID
        let run_id = format!("run_{}_{}", id, Utc::now().timestamp_millis());

        // 序列化参数
        let params_json = serde_json::to_string(&params)
            .map_err(|e| CisError::serialization(format!("Failed to serialize params: {}", e)))?;

        // 创建运行记录
        core.create_dag_run(&run_id, id, Some(&params_json))?;

        // 更新 DAG 最后运行时间
        core.update_dag_last_run(id)?;

        // 添加运行日志
        core.add_dag_log(id, Some(&run_id), "info", "DAG run started")?;

        // 返回运行信息
        Ok(DagRun {
            run_id,
            dag_id: id.to_string(),
            status: RunStatus::Pending,
            started_at: Utc::now(),
            finished_at: None,
            tasks_completed: 0,
            tasks_failed: 0,
            tasks_total: dag.tasks_count,
        })
    }

    /// 列出 DAG 运行历史
    pub async fn runs(&self, id: &str, limit: usize) -> Result<Vec<DagRun>> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        // 检查 DAG 是否存在
        if core.get_dag(id)?.is_none() {
            return Err(CisError::not_found(format!("DAG '{}' not found", id)));
        }

        let records = core.list_dag_runs(id, limit)?;
        let runs: Vec<DagRun> = records.into_iter().map(DagRun::from).collect();
        Ok(runs)
    }

    /// 查看运行详情
    pub async fn run_inspect(&self, run_id: &str) -> Result<DagRun> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        match core.get_dag_run(run_id)? {
            Some(record) => Ok(DagRun::from(record)),
            None => Err(CisError::not_found(format!("Run '{}' not found", run_id))),
        }
    }

    /// 取消运行
    pub async fn run_cancel(&self, run_id: &str) -> Result<()> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        // 获取运行记录
        let run = match core.get_dag_run(run_id)? {
            Some(r) => r,
            None => return Err(CisError::not_found(format!("Run '{}' not found", run_id))),
        };

        // 检查是否可以取消
        if run.status == "success" || run.status == "failed" || run.status == "cancelled" {
            return Err(CisError::invalid_input(format!(
                "Cannot cancel run '{}' with status '{}'", 
                run_id, run.status
            )));
        }

        // 更新状态为取消
        core.update_dag_run_status(run_id, "cancelled")?;

        // 添加日志
        core.add_dag_log(&run.dag_id, Some(run_id), "info", "DAG run cancelled by user")?;

        Ok(())
    }

    /// 删除 DAG
    pub async fn remove(&self, id: &str, force: bool) -> Result<()> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        // 检查 DAG 是否存在
        if core.get_dag(id)?.is_none() {
            return Err(CisError::not_found(format!("DAG '{}' not found", id)));
        }

        // 检查是否有正在运行的任务
        if !force {
            let runs = core.list_dag_runs(id, 100)?;
            let has_running = runs.iter().any(|r| r.status == "pending" || r.status == "running");
            if has_running {
                return Err(CisError::invalid_input(
                    format!("DAG '{}' has running instances. Use force=true to delete anyway.", id)
                ));
            }
        }

        // 删除 DAG（级联删除运行记录和日志）
        core.delete_dag(id)?;

        Ok(())
    }

    /// 获取 DAG 日志
    pub async fn logs(&self, id: &str, run_id: Option<&str>, tail: usize) -> Result<Vec<String>> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        // 检查 DAG 是否存在
        if core.get_dag(id)?.is_none() {
            return Err(CisError::not_found(format!("DAG '{}' not found", id)));
        }

        let records = core.get_dag_logs(id, run_id, tail)?;
        let logs: Vec<String> = records.into_iter()
            .map(|log| {
                let time = Utc.timestamp_opt(log.timestamp, 0)
                    .single()
                    .unwrap_or_else(Utc::now)
                    .format("%Y-%m-%d %H:%M:%S");
                format!("[{}] [{}] {}", time, log.level.to_uppercase(), log.message)
            })
            .collect();

        Ok(logs)
    }

    /// 暂停 DAG
    pub async fn pause(&self, id: &str) -> Result<()> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        // 检查 DAG 是否存在
        if core.get_dag(id)?.is_none() {
            return Err(CisError::not_found(format!("DAG '{}' not found", id)));
        }

        // 更新状态为暂停
        let updated = core.update_dag_status(id, "paused")?;
        if !updated {
            return Err(CisError::storage(format!("Failed to pause DAG '{}'", id)));
        }

        // 添加日志
        core.add_dag_log(id, None, "info", "DAG paused")?;

        Ok(())
    }

    /// 恢复 DAG
    pub async fn unpause(&self, id: &str) -> Result<()> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        // 检查 DAG 是否存在
        if core.get_dag(id)?.is_none() {
            return Err(CisError::not_found(format!("DAG '{}' not found", id)));
        }

        // 更新状态为活跃
        let updated = core.update_dag_status(id, "active")?;
        if !updated {
            return Err(CisError::storage(format!("Failed to unpause DAG '{}'", id)));
        }

        // 添加日志
        core.add_dag_log(id, None, "info", "DAG resumed")?;

        Ok(())
    }

    /// 清理旧的 DAG 运行记录
    pub async fn prune(&self, max_age_days: u32) -> Result<usize> {
        let core = self.db_manager.core();
        let core = core.lock()
            .map_err(|e| CisError::storage(format!("Failed to lock core db: {}", e)))?;

        let count = core.prune_dag_runs(max_age_days)?;
        Ok(count)
    }
}

impl Default for DagService {
    fn default() -> Self {
        Self::new().expect("Failed to create DagService")
    }
}
