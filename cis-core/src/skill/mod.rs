//! # Skill 管理模块
//!
//! 支持热插拔的 Skill 生命周期管理。
//!
//! ## 生命周期状态
//!
//! ```
//! Installed → Registered → Loaded → Active → Unloading → Unloaded → Removed
//!                 ↑_________|___________|       |
//!                          Pause      Resume     |
//!                                       ↑_________|
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod manager;
pub mod manifest;
pub mod registry;
pub mod types;

pub use manager::SkillManager;
pub use manifest::{SkillManifest, SkillPermissions, ManifestValidator};
pub use registry::{SkillRegistry, SkillRegistration};
pub use types::{LoadOptions, SkillConfig, SkillInfo, SkillMeta, SkillState, SkillType};

/// Skill 统一接口（CIS Core 内部使用）
#[async_trait]
pub trait Skill: Send + Sync {
    /// Skill 名称
    fn name(&self) -> &str;

    /// 版本号
    fn version(&self) -> &str {
        "0.1.0"
    }

    /// 描述
    fn description(&self) -> &str {
        ""
    }

    /// 初始化
    async fn init(&mut self, config: SkillConfig) -> crate::error::Result<()> {
        let _ = config;
        Ok(())
    }

    /// 处理事件
    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> crate::error::Result<()>;

    /// 关闭
    async fn shutdown(&self) -> crate::error::Result<()> {
        Ok(())
    }
}

/// Skill 上下文接口
pub trait SkillContext: Send + Sync {
    /// 记录日志
    fn log_info(&self, message: &str);
    fn log_debug(&self, message: &str);
    fn log_warn(&self, message: &str);
    fn log_error(&self, message: &str);

    /// 读取记忆
    fn memory_get(&self, key: &str) -> Option<Vec<u8>>;

    /// 写入记忆
    fn memory_set(&self, key: &str, value: &[u8]) -> crate::error::Result<()>;

    /// 删除记忆
    fn memory_delete(&self, key: &str) -> crate::error::Result<()>;

    /// 获取配置
    fn config(&self) -> &SkillConfig;
}

/// 事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    /// 初始化
    Init { config: serde_json::Value },
    /// 关闭
    Shutdown,
    /// 定时触发
    Tick,
    /// 记忆变更
    MemoryChange {
        key: String,
        value: Vec<u8>,
        operation: MemoryOp,
    },
    /// 自定义事件
    Custom {
        name: String,
        data: serde_json::Value,
    },
    /// Agent 调用
    AgentCall {
        prompt: String,
        callback: String, // channel identifier
    },
}

/// 记忆操作类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryOp {
    Create,
    Update,
    Delete,
}
