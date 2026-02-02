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
//! ## 使用示例
//!
//! ```rust
//! use cis_core::wasm::WasmRuntime;
//!
//! let mut runtime = WasmRuntime::new()?;
//! let instance = runtime.load_skill(&wasm_bytes)?;
//! ```

use std::sync::{Arc, Mutex};
use wasmer::{Engine, Module, Store};

use crate::error::{CisError, Result};

pub mod host;
pub mod skill;

#[cfg(test)]
mod tests;

pub use skill::{WasmSkill, WasmSkillBuilder};

/// WASM 运行时
///
/// 管理 WASM 模块的加载和执行环境。
pub struct WasmRuntime {
    /// WASM 引擎
    engine: Engine,
    /// WASM 存储
    store: Arc<Mutex<Store>>,
}

impl WasmRuntime {
    /// 创建新的 WASM 运行时
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::WasmRuntime;
    ///
    /// let runtime = WasmRuntime::new();
    /// ```
    pub fn new() -> Result<Self> {
        let engine = Engine::default();
        let store = Store::new(engine.clone());
        
        Ok(Self {
            engine,
            store: Arc::new(Mutex::new(store)),
        })
    }

    /// 从字节码加载 WASM Skill
    ///
    /// # 参数
    ///
    /// - `wasm_bytes`: WASM 模块的字节码
    ///
    /// # 返回
    ///
    /// 返回加载的 WASM 实例
    pub fn load_skill(&mut self, wasm_bytes: &[u8]) -> Result<WasmInstance> {
        let module = Module::from_binary(&self.engine, wasm_bytes)
            .map_err(|e| CisError::skill(format!("Failed to load WASM module: {}", e)))?;
        
        Ok(WasmInstance {
            module,
            store: Arc::clone(&self.store),
            instance: None,
        })
    }

    /// 获取引擎的引用
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create default WASM runtime")
    }
}

/// WASM 实例
///
/// 表示一个已加载的 WASM Skill 实例。
pub struct WasmInstance {
    /// WASM 模块
    module: Module,
    /// WASM 存储
    store: Arc<Mutex<Store>>,
    /// 实例（实例化后设置）
    instance: Option<wasmer::Instance>,
}

impl WasmInstance {
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
}

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

/// 从文件加载 WASM 模块
///
/// # 参数
///
/// - `path`: WASM 文件路径
///
/// # 返回
///
/// 返回文件内容作为字节向量
pub fn load_wasm_from_file(path: &std::path::Path) -> Result<Vec<u8>> {
    std::fs::read(path)
        .map_err(|e| CisError::skill(format!("Failed to read WASM file: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_runtime_new() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok(), "Failed to create WASM runtime");
    }

    #[test]
    fn test_wasm_runtime_default() {
        let runtime = WasmRuntime::default();
        // 验证运行时创建成功
        let _store = runtime.store.lock().unwrap();
    }

    #[test]
    fn test_wasm_skill_config_default() {
        let config = WasmSkillConfig::default();
        assert_eq!(config.memory_limit, Some(64 * 1024 * 1024));
        assert_eq!(config.execution_timeout, Some(30000));
        assert!(config.allowed_syscalls.is_empty());
    }
}
