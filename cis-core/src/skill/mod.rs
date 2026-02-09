//! # Skill 管理模块
//!
//! 支持热插拔的 Skill 生命周期管理。
//!
//! ## 生命周期状态
//!
//! ```text
//! Installed → Registered → Loaded → Active → Unloading → Unloaded → Removed
//!                 ↑_________|___________|       |
//!                          Pause      Resume     |
//!                                       ↑_________|
//! ```
//!
//! ## Skill = Matrix Room 视图
//!
//! 每个 Skill 对应一个 Matrix Room，支持联邦标记控制是否广播：
//! - `room_id()`: 返回 Skill 对应的 Matrix Room ID
//! - `federate()`: 控制是否联邦同步（默认 false）
//! - `init()`: 初始化时创建/加入 Room
//! - `on_matrix_event()`: 处理 Matrix 事件

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod builtin;
pub mod chain;
pub mod cis_admin;
pub mod compatibility_db;
pub mod dag;
pub mod manager;
pub mod manifest;
pub mod project_registry;
pub mod registry;
pub mod router;
pub mod semantics;
pub mod types;

pub use builtin::{BuiltinSkill, BuiltinSkillInstaller, BUILTIN_SKILLS, ensure_required_skills};
pub use chain::{ChainBuilder, ChainContext, ChainDiscoveryResult, ChainMetadata, ChainOrchestrator,
                ChainStep, ChainStepResult, ChainTemplates, SkillChain, SkillCompatibilityRecord, StepResult};
pub use cis_admin::{CisAdminSkill, CisAnalyzeSkill, CisCommitSkill, CisFileSkill, CisReadSkill, register_cis_local_skills};
pub use compatibility_db::SkillCompatibilityDb;
pub use manager::SkillManager;
pub use manifest::{SkillManifest, SkillPermissions, ManifestValidator};
pub use dag::{SkillDagBuilder, SkillDagContext, SkillDagConverter, SkillDagStats};
pub use project_registry::{ProjectSkillRegistry, ProjectSkillConfig, ProjectSkillEntry, ProjectSkillDiscovery};
pub use registry::{SkillRegistry, SkillRegistration};
pub use router::{ChainExecutionResult, ResolvedParameters, RouteResult, 
                SkillCompatibility, SkillRoutingResult, SkillVectorRouter};
pub use semantics::{SkillIoSignature, SkillScope, SkillSemanticDescription, SkillSemanticMatcher, SkillSemanticRegistry, SkillSemanticsExt};
pub use types::{LoadOptions, SkillConfig, SkillInfo, SkillMeta, SkillRoomInfo, SkillState, SkillType};

// Re-export Matrix types for Skill integration
pub use crate::matrix::nucleus::{MatrixNucleus, RoomOptions};

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

    /// Skill 对应的 Matrix Room ID
    /// 默认格式: !{skill_name}:cis.local
    fn room_id(&self) -> Option<String> {
        Some(format!("!{}:cis.local", self.name()))
    }

    /// 是否联邦同步（默认 false）
    fn federate(&self) -> bool {
        false
    }

    /// 初始化
    ///
    /// 默认实现：创建 Room，注册 Matrix 事件处理器
    async fn init(&mut self, config: SkillConfig) -> crate::error::Result<()> {
        let _ = config;
        Ok(())
    }

    /// 初始化 Matrix Room（由核心在加载 Skill 后调用）
    ///
    /// 默认实现：
    /// - 创建/加入 Room
    /// - 通过 channel 传递 Matrix 事件给 Skill
    async fn init_room(&self, nucleus: Arc<MatrixNucleus>) -> crate::error::Result<()> {
        if let Some(room_id_str) = self.room_id() {
            let room_id = crate::matrix::nucleus::RoomId::parse(&room_id_str)
                .map_err(|e| crate::error::CisError::Other(format!("Invalid room ID: {}", e)))?;
            
            // 创建 RoomOptions
            let opts = RoomOptions::new(self.name().to_string())
                .with_federate(self.federate());
            
            // 创建 Room
            nucleus.create_room(opts).await.map_err(|e| 
                crate::error::CisError::Other(format!("Failed to create room: {}", e))
            )?;

            // 创建 channel 用于传递 Matrix 事件
            let (event_tx, mut event_rx) = mpsc::channel::<crate::matrix::nucleus::MatrixEvent>(100);
            
            // 注册事件处理器，将事件发送到 channel
            let _handler_id = nucleus.register_handler("*", move |event| {
                if let Err(e) = event_tx.try_send(event) {
                    tracing::warn!("Failed to send event to skill channel: {}", e);
                }
                Ok(())
            }).await;

            // 启动后台任务处理 channel 中的事件
            let skill_name = self.name().to_string();
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    tracing::debug!(
                        "Skill '{}' received matrix event: {:?}",
                        skill_name,
                        &event.event_type
                    );
                    // 注意：这里需要将事件传递给 Skill 实例
                    // 由于 trait 对象限制，实际的 on_matrix_event 调用
                    // 需要在具体的 Skill 实现中处理
                }
            });

            tracing::info!("Skill '{}' initialized room: {}", self.name(), room_id);
        }
        Ok(())
    }

    /// 处理 Matrix Event（可选实现）
    async fn on_matrix_event(&self, event: crate::matrix::nucleus::MatrixEvent) -> crate::error::Result<()> {
        // 默认空实现
        let _ = event;
        Ok(())
    }

    /// 处理 CIS 事件
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
