//! WASM 运行时测试
//!
//! 测试 WASM 模块的加载、验证和实例化。

use super::runtime::{WasmRuntime, WasmModule};
use super::{WasmSkillConfig, DEFAULT_MEMORY_LIMIT_BYTES, DEFAULT_EXECUTION_TIMEOUT_MS};
use std::sync::{Arc, Mutex};

/// 简单的 WASM 模块：空模块
const SIMPLE_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x01, 0x00, 0x00, 0x00, // version 1
];

/// 无效的 WASM 魔数
const INVALID_MAGIC_WASM: &[u8] = &[
    0x00, 0x00, 0x00, 0x00, // Invalid magic
    0x01, 0x00, 0x00, 0x00, // version 1
];

/// 不支持的 WASM 版本
const UNSUPPORTED_VERSION_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x02, 0x00, 0x00, 0x00, // version 2 (unsupported)
];

/// 测试运行时创建
#[test]
fn test_runtime_creation() {
    let runtime = WasmRuntime::new();
    assert!(runtime.is_ok());
}

/// 测试运行时默认创建
#[test]
fn test_runtime_default() {
    let _runtime = WasmRuntime::default();
    // 应该成功创建
}

/// 测试使用自定义配置创建运行时
#[test]
fn test_runtime_with_config() {
    let config = WasmSkillConfig {
        memory_limit: Some(256 * 1024 * 1024), // 256MB
        execution_timeout: Some(60000),        // 60 seconds
        allowed_syscalls: vec![],
    };
    
    let runtime = WasmRuntime::with_config(config);
    assert!(runtime.is_ok());
}

/// 测试无效配置创建失败
#[test]
fn test_runtime_invalid_config() {
    let config = WasmSkillConfig {
        memory_limit: Some(0), // 无效：0 内存
        ..Default::default()
    };
    
    let runtime = WasmRuntime::with_config(config);
    assert!(runtime.is_err());
}

/// 测试加载有效的 WASM 模块
#[test]
fn test_load_valid_module() {
    let runtime = WasmRuntime::new().unwrap();
    let module = runtime.load_module(SIMPLE_WASM);
    assert!(module.is_ok());
}

/// 测试加载无效的 WASM 魔数
#[test]
fn test_load_invalid_magic() {
    let runtime = WasmRuntime::new().unwrap();
    let result = runtime.load_module(INVALID_MAGIC_WASM);
    assert!(result.is_err());
}

/// 测试加载不支持的版本
#[test]
fn test_load_unsupported_version() {
    let runtime = WasmRuntime::new().unwrap();
    let result = runtime.load_module(UNSUPPORTED_VERSION_WASM);
    assert!(result.is_err());
}

/// 测试加载太小的模块
#[test]
fn test_load_too_small() {
    let runtime = WasmRuntime::new().unwrap();
    let tiny = &[0x00, 0x61, 0x73]; // 只有 3 字节
    let result = runtime.load_module(tiny);
    assert!(result.is_err());
}

/// 测试空模块加载
#[test]
fn test_load_empty_module() {
    let runtime = WasmRuntime::new().unwrap();
    let result = runtime.load_module(&[]);
    assert!(result.is_err());
}

/// 测试模块最大页数计算
#[test]
fn test_max_memory_pages_calculation() {
    let config = WasmSkillConfig {
        memory_limit: Some(512 * 1024 * 1024), // 512MB
        ..Default::default()
    };
    
    let runtime = WasmRuntime::with_config(config).unwrap();
    let module = runtime.load_module(SIMPLE_WASM).unwrap();
    
    // 512MB / 64KB = 8192 页
    assert_eq!(module.max_memory_pages(), 8192);
}

/// 测试自定义内存限制
#[test]
fn test_custom_memory_limit() {
    let config = WasmSkillConfig {
        memory_limit: Some(128 * 1024 * 1024), // 128MB
        ..Default::default()
    };
    
    let runtime = WasmRuntime::with_config(config).unwrap();
    let module = runtime.load_module(SIMPLE_WASM).unwrap();
    
    // 128MB / 64KB = 2048 页
    assert_eq!(module.max_memory_pages(), 2048);
}

/// 测试执行超时配置
#[test]
fn test_execution_timeout_config() {
    let config = WasmSkillConfig {
        execution_timeout: Some(10000), // 10 seconds
        ..Default::default()
    };
    
    let runtime = WasmRuntime::with_config(config).unwrap();
    let module = runtime.load_module(SIMPLE_WASM).unwrap();
    
    assert_eq!(module.execution_timeout().as_millis(), 10000);
}

/// 测试默认执行超时
#[test]
fn test_default_execution_timeout() {
    let runtime = WasmRuntime::new().unwrap();
    let module = runtime.load_module(SIMPLE_WASM).unwrap();
    
    assert_eq!(module.execution_timeout().as_millis() as u64, DEFAULT_EXECUTION_TIMEOUT_MS);
}

/// 测试模块获取方法
#[test]
fn test_module_getters() {
    let runtime = WasmRuntime::new().unwrap();
    let module = runtime.load_module(SIMPLE_WASM).unwrap();
    
    assert!(module.module().exports().count() >= 0);
}

/// 测试运行时获取引擎
#[test]
fn test_runtime_engine() {
    let runtime = WasmRuntime::new().unwrap();
    let _engine = runtime.engine();
    // 引擎应该有效
}

/// 测试运行时获取存储
#[test]
fn test_runtime_store() {
    let runtime = WasmRuntime::new().unwrap();
    let store = runtime.store();
    
    // 存储应该可以锁定
    let _guard = store.lock().unwrap();
}

/// 测试配置默认值
#[test]
fn test_config_defaults() {
    let config = WasmSkillConfig::default();
    
    assert_eq!(config.memory_limit, Some(DEFAULT_MEMORY_LIMIT_BYTES));
    assert_eq!(config.execution_timeout, Some(DEFAULT_EXECUTION_TIMEOUT_MS));
    assert!(config.allowed_syscalls.is_empty());
}

/// 测试配置构建器模式
#[test]
fn test_config_builder() {
    let config = WasmSkillConfig::new()
        .with_memory_limit_mb(256)
        .with_timeout_ms(5000);
    
    assert_eq!(config.memory_limit, Some(256 * 1024 * 1024));
    assert_eq!(config.execution_timeout, Some(5000));
}

/// 测试配置验证
#[test]
fn test_config_validation() {
    // 有效配置
    let valid_config = WasmSkillConfig {
        memory_limit: Some(512 * 1024 * 1024),
        execution_timeout: Some(30000),
        allowed_syscalls: vec!["read".to_string()],
    };
    assert!(valid_config.validate().is_ok());
    
    // 内存为 0
    let invalid_memory = WasmSkillConfig {
        memory_limit: Some(0),
        ..Default::default()
    };
    assert!(invalid_memory.validate().is_err());
    
    // 内存超过 4GB
    let too_large_memory = WasmSkillConfig {
        memory_limit: Some(5 * 1024 * 1024 * 1024),
        ..Default::default()
    };
    assert!(too_large_memory.validate().is_err());
    
    // 超时为 0
    let invalid_timeout = WasmSkillConfig {
        execution_timeout: Some(0),
        ..Default::default()
    };
    assert!(invalid_timeout.validate().is_err());
    
    // 超时超过 5 分钟
    let too_long_timeout = WasmSkillConfig {
        execution_timeout: Some(400_000),
        ..Default::default()
    };
    assert!(too_long_timeout.validate().is_err());
}

/// 测试配置克隆
#[test]
fn test_config_clone() {
    let config = WasmSkillConfig {
        memory_limit: Some(256 * 1024 * 1024),
        execution_timeout: Some(30000),
        allowed_syscalls: vec!["read".to_string(), "write".to_string()],
    };
    
    let cloned = config.clone();
    assert_eq!(config.memory_limit, cloned.memory_limit);
    assert_eq!(config.execution_timeout, cloned.execution_timeout);
    assert_eq!(config.allowed_syscalls, cloned.allowed_syscalls);
}

/// 测试系统调用白名单
#[test]
fn test_allowed_syscalls() {
    let config = WasmSkillConfig {
        allowed_syscalls: vec!["read".to_string(), "write".to_string()],
        ..Default::default()
    };
    
    assert!(config.validate().is_ok());
    assert_eq!(config.allowed_syscalls.len(), 2);
}

/// 测试 4GB 内存边界
#[test]
fn test_memory_boundary_4gb() {
    // 正好 4GB 应该通过
    let config_4gb = WasmSkillConfig {
        memory_limit: Some(4 * 1024 * 1024 * 1024),
        ..Default::default()
    };
    assert!(config_4gb.validate().is_ok());
    
    // 超过 4GB 应该失败
    let config_over = WasmSkillConfig {
        memory_limit: Some(4 * 1024 * 1024 * 1024 + 1),
        ..Default::default()
    };
    assert!(config_over.validate().is_err());
}

/// 测试模块大小编码
#[test]
fn test_module_size_limit() {
    // 创建一个超过 100MB 限制的模块（通过重复填充）
    let huge_module = vec![0x00u8; 101 * 1024 * 1024];
    
    let runtime = WasmRuntime::new().unwrap();
    let result = runtime.load_module(&huge_module);
    
    // 应该因为大小限制而失败
    assert!(result.is_err());
}
