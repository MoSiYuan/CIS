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

pub mod manager;
pub mod registry;
pub mod types;

pub use manager::SkillManager;
pub use registry::{SkillRegistry, SkillRegistration};
pub use types::{SkillMeta, SkillState, SkillType};
