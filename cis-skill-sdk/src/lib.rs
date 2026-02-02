//! # CIS Skill SDK
//!
//! 统一的 Skill 开发框架，支持 Native 和 WASM 双模式。
//!
//! ## 快速开始
//!
//! ```rust
//! use cis_skill_sdk::{Skill, SkillContext, Event, Result};
//!
//! pub struct HelloSkill;
//!
//! #[cis_skill_sdk::skill]
//! impl Skill for HelloSkill {
//!     fn name(&self) -> &str {
//!         "hello"
//!     }
//!     
//!     async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
//!         if let Event::Custom { name, data } = event {
//!             if name == "greet" {
//!                 ctx.log_info(&format!("Hello, {:?}!", data));
//!             }
//!         }
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ## 功能模块
//!
//! - [`trait`] - Skill 核心 trait 定义
//! - [`types`] - 通用类型定义
//! - [`host`] - Host API 接口
//! - [`im`] - IM 即时通讯专用接口
//! - [`ai`] - AI 调用接口
//!
//! ## 模式选择
//!
//! - **Native** (默认): 本地编译，完整功能
//! - **WASM**: `no_std` 兼容，沙箱安全

#![cfg_attr(feature = "wasm", no_std)]

pub mod ai;
pub mod error;
pub mod host;
pub mod im;
pub mod skill;
pub mod types;

// 导出 derive 宏
pub use cis_skill_sdk_derive::skill;

// 重导出核心类型
pub use error::{Error, Result};
pub use skill::{Skill, SkillContext};
pub use types::*;

// Native 模式额外导出
#[cfg(feature = "native")]
pub use skill::NativeSkill;

// WASM 模式导出
#[cfg(feature = "wasm")]
pub mod wasm {
    //! WASM 特定导出
    pub use crate::skill::WasmSkill;
}
