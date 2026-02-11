//! # CIS Service Layer
//!
//! 统一的数据服务层，为 CLI、GUI、API 提供一致的接口。
//!
//! ## 设计原则
//!
//! - **统一接口**: 所有访问形式使用相同的服务接口
//! - **资源抽象**: 每个资源类型（Worker、Node、DAG、Task）有独立服务
//! - **异步支持**: 所有操作都是异步的，支持并发
//! - **错误统一**: 使用 CisError 统一错误处理
//!
//! ## 服务列表
//!
//! - `WorkerService` - Worker 进程管理
//! - `NodeService` - 节点管理
//! - `DagService` - DAG 管理
//! - `TaskService` - 任务管理
//! - `SkillService` - Skill 管理
//! - `NetworkService` - 网络/联邦管理
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use cis_core::service::{WorkerService, ListOptions};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let service = WorkerService::new()?;
//! let workers = service.list(ListOptions::default()).await?;
//! # Ok(())
//! # }
//! ```

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod worker_service;
pub mod node_service;
pub mod dag_service;
pub mod task_service;
pub mod skill_executor_impl;

pub use worker_service::WorkerService;
pub use node_service::NodeService;
pub use dag_service::DagService;
pub use task_service::TaskService;
pub use skill_executor_impl::SkillExecutorImpl;

/// 通用列表选项
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// 显示所有（包括已停止/已删除）
    pub all: bool,
    /// 过滤器
    pub filters: HashMap<String, String>,
    /// 限制数量
    pub limit: Option<usize>,
    /// 排序字段
    pub sort_by: Option<String>,
    /// 排序方向
    pub sort_desc: bool,
}

impl ListOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_all(mut self) -> Self {
        self.all = true;
        self
    }

    pub fn with_filter(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.filters.insert(key.into(), value.into());
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_sort(mut self, field: impl Into<String>, desc: bool) -> Self {
        self.sort_by = Some(field.into());
        self.sort_desc = desc;
        self
    }
}

/// 通用资源状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceStatus {
    Running,
    Stopped,
    Paused,
    Error,
    Creating,
    Removing,
}

impl std::fmt::Display for ResourceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceStatus::Running => write!(f, "running"),
            ResourceStatus::Stopped => write!(f, "stopped"),
            ResourceStatus::Paused => write!(f, "paused"),
            ResourceStatus::Error => write!(f, "error"),
            ResourceStatus::Creating => write!(f, "creating"),
            ResourceStatus::Removing => write!(f, "removing"),
        }
    }
}

/// 资源统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceStats {
    pub cpu_percent: f64,
    pub memory_usage: u64,
    pub memory_limit: u64,
    pub memory_percent: f64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub pids: u32,
}

/// 分页结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

impl<T> PaginatedResult<T> {
    pub fn new(items: Vec<T>, total: usize) -> Self {
        Self {
            items,
            total,
            page: 1,
            per_page: total,
        }
    }

    pub fn with_pagination(mut self, page: usize, per_page: usize) -> Self {
        self.page = page;
        self.per_page = per_page;
        self
    }
}

/// 批量操作结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub success: Vec<String>,
    pub failed: Vec<(String, String)>,
}

impl BatchResult {
    pub fn new() -> Self {
        Self {
            success: Vec::new(),
            failed: Vec::new(),
        }
    }

    pub fn add_success(&mut self, id: impl Into<String>) {
        self.success.push(id.into());
    }

    pub fn add_failure(&mut self, id: impl Into<String>, error: impl Into<String>) {
        self.failed.push((id.into(), error.into()));
    }

    pub fn is_empty(&self) -> bool {
        self.success.is_empty() && self.failed.is_empty()
    }

    pub fn has_failures(&self) -> bool {
        !self.failed.is_empty()
    }
}

impl Default for BatchResult {
    fn default() -> Self {
        Self::new()
    }
}

/// 服务特征 - 定义资源的通用操作
#[async_trait]
pub trait ResourceService: Send + Sync {
    type Resource;
    type ResourceSummary;
    type ResourceInfo;

    /// 列出资源
    async fn list(&self, options: ListOptions) -> Result<PaginatedResult<Self::ResourceSummary>>;

    /// 获取资源详情
    async fn inspect(&self, id: &str) -> Result<Self::ResourceInfo>;

    /// 检查资源是否存在
    async fn exists(&self, id: &str) -> Result<bool>;

    /// 删除资源
    async fn remove(&self, id: &str, force: bool) -> Result<()>;

    /// 获取资源统计
    async fn stats(&self, id: &str) -> Result<ResourceStats>;

    /// 清理已停止/已删除的资源
    async fn prune(&self) -> Result<Vec<String>>;
}
