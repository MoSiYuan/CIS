//! # Command Groups
//!
//! Organized command groups for better CLI structure.

pub mod core;
pub mod memory;
pub mod skill;
pub mod agent;
pub mod workflow;
pub mod network;
pub mod system;
pub mod advanced;
pub mod task;
pub mod session;
pub mod engine;
pub mod migrate;

pub use core::CoreGroup;
pub use memory::MemoryGroup;
pub use skill::SkillGroup;
pub use agent::AgentGroup;
pub use workflow::WorkflowGroup;
pub use network::NetworkGroup;
pub use system::SystemGroup;
pub use advanced::AdvancedGroup;
pub use task::TaskGroup;
pub use session::SessionGroup;
pub use engine::EngineGroup;
pub use migrate::MigrateGroup;
