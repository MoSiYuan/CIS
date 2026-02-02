//! # WASM Runtime 模块
//!
//! 提供对 WASM Skill 的沙箱执行环境。
//!
//! ## 功能
//!
//! - WASM 模块加载和执行
//! - Host API 提供给 WASM Skill 调用
//! - 内存隔离和安全沙箱
//!
//! ## 架构
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        Host (Rust)                          │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
//! │  │ MemoryService│  │  AiProvider  │  │  ConfigStore │       │
//! │  └──────────────┘  └──────────────┘  └──────────────┘       │
//! │         │                 │                 │               │
//! │         └─────────────────┼─────────────────┘               │
//! │                           ▼                                 │
//! │                  ┌─────────────────┐                        │
//! │                  │   HostContext   │                        │
//! │                  └─────────────────┘                        │
//! │                           │                                 │
//! │                  ┌─────────────────┐                        │
//! │                  │  HostFunctions  │                        │
//! │                  │  (WASM Imports) │                        │
//! │                  └─────────────────┘                        │
//! └───────────────────────────┬─────────────────────────────────┘
//!                             │ Host API Calls
//! ════════════════════════════╪══════════════════════════════════
//!                             │
//! ┌───────────────────────────▼─────────────────────────────────┐
//! │                    WASM Sandbox                             │
//! │  ┌──────────────────────────────────────────────────────┐   │
//! │  │                 WASM Module                          │   │
//! │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐           │   │
//! │  │  │skill_init│  │skill_on_ │  │skill_    │           │   │
//! │  │  │          │  │  event   │  │shutdown  │           │   │
//! │  │  └──────────┘  └──────────┘  └──────────┘           │   │
//! │  │  ┌──────────────────────────────────────────────┐   │   │
//! │  │  │           Linear Memory (WasmPtr)            │   │   │
//! │  │  └──────────────────────────────────────────────┘   │   │
//! │  └──────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Host API
//!
//! WASM Skill 可以通过 Host API 访问宿主能力：
//!
//! - `host_memory_get`: 读取记忆
//! - `host_memory_set`: 写入记忆  
//! - `host_memory_delete`: 删除记忆
//! - `host_memory_search`: 语义搜索
//! - `host_ai_chat`: AI 对话
//! - `host_log`: 日志记录
//! - `host_config_get`: 获取配置
//! - `host_config_set`: 设置配置
//!
//! ## 使用示例
//!
//! ### 基本使用
//!
//! ```rust,no_run
//! use cis_core::wasm::{WasmRuntime, WasmSkillBuilder};
//! use std::sync::{Arc, Mutex};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // 创建运行时
//! let runtime = WasmRuntime::new()?;
//!
//! // 创建记忆服务和 AI Provider
//! let memory_service = Arc::new(Mutex::new(/* MemoryService */));
//! let ai_provider = Arc::new(Mutex::new(/* AiProvider */));
//!
//! // 加载 WASM Skill
//! let wasm_bytes = std::fs::read("skill.wasm")?;
//! let skill = runtime.load_skill(&wasm_bytes, memory_service, ai_provider)?;
//!
//! // 初始化
//! skill.init()?;
//!
//! // 发送事件
//! skill.on_event("test", b"{}")?;
//!
//! // 关闭
//! skill.shutdown()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### 使用 Builder
//!
//! ```rust,no_run
//! use cis_core::wasm::WasmSkillBuilder;
//! use std::sync::{Arc, Mutex};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let skill = WasmSkillBuilder::new()
//!     .name("my-skill")
//!     .version("1.0.0")
//!     .description("My WASM Skill")
//!     .wasm_bytes(std::fs::read("skill.wasm")?)
//!     .memory_service(Arc::new(Mutex::new(/* MemoryService */)))
//!     .build()?;
//! # Ok(())
//! # }
//! ```

use std::sync::{Arc, Mutex};
use wasmer::{Engine, Module, Store};

use crate::error::{CisError, Result};

// 子模块
pub mod host;
pub mod runtime;
pub mod skill;

#[cfg(test)]
mod tests;

// 公开导出
pub use host::{HostContext, HostFunctions, HostEnv};
pub use runtime::{WasmRuntime, WasmModule, WasmSkillInstance};
pub use skill::{WasmSkill as WasmSkillImpl, WasmSkillBuilder};

/// WASM Skill 配置
#[derive(Debug, Clone)]
pub struct WasmSkillConfig {
    /// 内存限制（字节）
    pub memory_limit: Option<usize>,
    /// 执行超时（毫秒）
    pub execution_timeout: Option<u64>,
    /// 允许的系统调用
    pub allowed_syscalls: Vec<String>,
}

impl Default for WasmSkillConfig {
    fn default() -> Self {
        Self {
            memory_limit: Some(64 * 1024 * 1024), // 64MB 默认限制
            execution_timeout: Some(30000),       // 30秒默认超时
            allowed_syscalls: vec![],
        }
    }
}

/// WASM 运行时（向后兼容别名）
///
/// 这是 [`runtime::WasmRuntime`] 的别名，用于向后兼容。
pub type WasmRuntimeCompat = runtime::WasmRuntime;

/// WASM 实例（向后兼容）
///
/// 表示一个已加载的 WASM Skill 实例。
/// 这个结构体现在使用新的运行时实现，但保持旧的 API。
pub struct WasmInstance {
    /// WASM 模块
    module: Module,
    /// WASM 存储
    store: Arc<Mutex<Store>>,
    /// 实例（实例化后设置）
    instance: Option<wasmer::Instance>,
    /// 内部运行时实例（新版）
    runtime_instance: Option<runtime::WasmSkillInstance>,
}

impl WasmInstance {
    /// 从模块和存储创建 WasmInstance
    pub(crate) fn from_module(module: Module, store: Arc<Mutex<Store>>) -> Self {
        Self {
            module,
            store,
            instance: None,
            runtime_instance: None,
        }
    }

    /// 获取模块的引用
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// 获取实例的引用（如果已实例化）
    pub fn instance(&self) -> Option<&wasmer::Instance> {
        self.instance.as_ref()
    }

    /// 设置实例
    pub fn set_instance(&mut self, instance: wasmer::Instance) {
        self.instance = Some(instance);
    }

    /// 获取存储的引用
    pub fn store(&self) -> Arc<Mutex<Store>> {
        Arc::clone(&self.store)
    }

    /// 设置运行时实例（新版）
    pub(crate) fn set_runtime_instance(&mut self, instance: runtime::WasmSkillInstance) {
        self.runtime_instance = Some(instance);
    }

    /// 获取运行时实例（新版）
    pub(crate) fn runtime_instance(&self) -> Option<&runtime::WasmSkillInstance> {
        self.runtime_instance.as_ref()
    }
}

/// 从文件加载 WASM 模块
///
/// # 参数
///
/// - `path`: WASM 文件路径
///
/// # 返回
///
/// 返回文件内容作为字节向量
///
/// # 示例
///
/// ```rust,no_run
/// use cis_core::wasm::load_wasm_from_file;
/// use std::path::Path;
///
/// let wasm_bytes = load_wasm_from_file(Path::new("skill.wasm")).expect("Failed to load");
/// ```
pub fn load_wasm_from_file(path: &std::path::Path) -> Result<Vec<u8>> {
    std::fs::read(path)
        .map_err(|e| CisError::skill(format!("Failed to read WASM file: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_skill_config_default() {
        let config = WasmSkillConfig::default();
        assert_eq!(config.memory_limit, Some(64 * 1024 * 1024));
        assert_eq!(config.execution_timeout, Some(30000));
        assert!(config.allowed_syscalls.is_empty());
    }
}
