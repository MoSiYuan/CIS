//! # WASM Runtime 模块
//!
//! 提供对 WASM Skill 的沙箱执行环境。
//!
//! ## 功能
//!
//! - WASM 模块加载和执行
//! - Host API 提供给 WASM Skill 调用
//! - 内存隔离和安全沙箱
//! - 资源限制（内存、超时、执行步数）
//! - 网络权限控制
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
//! ## 资源限制
//!
//! 默认配置：
//! - 内存限制: 512 MB
//! - 执行超时: 30 秒
//! - 最大执行步数: 1,000,000
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
//! ```ignore
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
//! ```ignore
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
//!
//! ### 使用自定义配置
//!
//! ```ignore
//! use cis_core::wasm::{WasmRuntime, WasmSkillConfig};
//! use std::sync::{Arc, Mutex};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // 创建自定义配置
//! let config = WasmSkillConfig {
//!     memory_limit: Some(512 * 1024 * 1024),  // 512 MB
//!     execution_timeout: Some(30000),          // 30 seconds
//!     allowed_syscalls: vec!["read".to_string(), "write".to_string()],
//! };
//!
//! // 使用配置创建运行时
//! let runtime = WasmRuntime::with_config(config)?;
//! # Ok(())
//! # }
//! ```

use std::sync::{Arc, Mutex};
use wasmer::{Module, Store};

use crate::error::{CisError, Result};

// 子模块
pub mod host;
pub mod runtime;
pub mod sandbox;
pub mod skill;
pub mod validator;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod runtime_tests;

#[cfg(test)]
mod host_tests;

#[cfg(test)]
mod skill_tests;

// 公开导出
pub use host::{HostContext, HostFunctions, HostEnv, ExecutionLimits};
pub use runtime::{WasmRuntime, WasmModule, WasmSkillInstance};
pub use sandbox::{WasiSandbox, AccessType, WasiSandboxSummary};
pub use skill::{WasmSkill as WasmSkillImpl, WasmSkillBuilder, WasmSkillExecutor};

/// 默认 WASM 内存限制（512MB，以字节为单位）
pub const DEFAULT_MEMORY_LIMIT_BYTES: usize = 512 * 1024 * 1024;

/// 默认执行超时（30秒，以毫秒为单位）
pub const DEFAULT_EXECUTION_TIMEOUT_MS: u64 = 30000;

/// 默认最大执行步数
pub const DEFAULT_MAX_EXECUTION_STEPS: u64 = 1_000_000;

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
            memory_limit: Some(DEFAULT_MEMORY_LIMIT_BYTES), // 512MB 默认限制
            execution_timeout: Some(DEFAULT_EXECUTION_TIMEOUT_MS), // 30秒默认超时
            allowed_syscalls: vec![],
        }
    }
}

impl WasmSkillConfig {
    /// 创建新的配置，使用默认值
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置内存限制（MB）
    pub fn with_memory_limit_mb(mut self, mb: usize) -> Self {
        self.memory_limit = Some(mb * 1024 * 1024);
        self
    }

    /// 设置执行超时（毫秒）
    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.execution_timeout = Some(ms);
        self
    }

    /// 设置允许的系统调用
    pub fn with_allowed_syscalls(mut self, syscalls: Vec<String>) -> Self {
        self.allowed_syscalls = syscalls;
        self
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 验证内存限制
        if let Some(limit) = self.memory_limit {
            if limit == 0 {
                return Err(CisError::configuration("Memory limit cannot be zero"));
            }
            if limit > 4 * 1024 * 1024 * 1024 { // 4GB WebAssembly 最大限制
                return Err(CisError::configuration(
                    format!("Memory limit {} exceeds WebAssembly maximum (4GB)", limit)
                ));
            }
        }

        // 验证执行超时
        if let Some(timeout) = self.execution_timeout {
            if timeout == 0 {
                return Err(CisError::configuration("Execution timeout cannot be zero"));
            }
            if timeout > 300_000 { // 5 分钟上限
                return Err(CisError::configuration(
                    format!("Execution timeout {}ms exceeds maximum (300000ms)", timeout)
                ));
            }
        }

        Ok(())
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
    #[allow(dead_code)]
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
/// ```ignore
/// use cis_core::wasm::load_wasm_from_file;
/// use std::path::Path;
///
/// let wasm_bytes = load_wasm_from_file(Path::new("skill.wasm")).expect("Failed to load");
/// ```
pub fn load_wasm_from_file(path: &std::path::Path) -> Result<Vec<u8>> {
    std::fs::read(path)
        .map_err(|e| CisError::skill(format!("Failed to read WASM file: {}", e)))
}

