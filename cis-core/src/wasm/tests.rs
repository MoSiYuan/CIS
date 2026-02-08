//! # WASM 模块测试
//!
//! WASM Runtime 的完整测试套件。

use super::*;

/// 最小的有效 WASM 模块（空模块）
/// 
/// 这个模块只包含 WASM 魔数和版本号。
const EMPTY_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic: \0asm
    0x01, 0x00, 0x00, 0x00, // version 1
];

/// 简单的 WASM 模块，导出一个 `add` 函数
/// 
/// 这个模块是用 WAT (WebAssembly Text Format) 编写的：
/// ```wat
/// (module
///   (func $add (param i32 i32) (result i32)
///     local.get 0
///     local.get 1
///     i32.add)
///   (export "add" (func $add))
/// )
/// ```
const ADD_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x07, 0x01,                               // type section
    0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,             // type: (i32, i32) -> i32
    0x03, 0x02, 0x01, 0x00,                         // func section
    0x07, 0x07, 0x01,                               // export section
    0x03, 0x61, 0x64, 0x64, 0x00, 0x00,             // export "add" as func 0
    0x0a, 0x09, 0x01,                               // code section
    0x07, 0x00,                                     // body size 7, 0 locals
    0x20, 0x00,                                     // local.get 0
    0x20, 0x01,                                     // local.get 1
    0x6a,                                           // i32.add
    0x0b,                                           // end
];

/// 无效 WASM 魔数
const INVALID_MAGIC: &[u8] = &[
    0x00, 0x00, 0x00, 0x00, // Invalid magic
    0x01, 0x00, 0x00, 0x00, // version 1
];

/// 无效 WASM 版本
const INVALID_VERSION: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x02, 0x00, 0x00, 0x00, // Invalid version 2
];

/// 太小的 WASM 模块
const TOO_SMALL: &[u8] = &[0x00, 0x61, 0x73, 0x6d];

#[test]
fn test_wasm_runtime_creation() {
    let runtime = WasmRuntime::new();
    assert!(runtime.is_ok(), "Failed to create WASM runtime");
}

#[test]
fn test_load_empty_wasm() {
    let runtime = WasmRuntime::new().unwrap();
    let result = runtime.load_module(EMPTY_WASM);
    assert!(result.is_ok(), "Failed to load empty WASM module");
}

#[test]
fn test_load_add_wasm() {
    let runtime = WasmRuntime::new().unwrap();
    let result = runtime.load_module(ADD_WASM);
    assert!(result.is_ok(), "Failed to load add WASM module");
}

#[test]
fn test_invalid_wasm_rejection() {
    let runtime = WasmRuntime::new().unwrap();
    
    // 测试无效魔数
    let result = runtime.load_module(INVALID_MAGIC);
    assert!(result.is_err(), "Should reject invalid WASM magic");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("magic"), "Error should mention magic number");
    
    // 测试无效版本
    let result = runtime.load_module(INVALID_VERSION);
    assert!(result.is_err(), "Should reject invalid WASM version");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("version"), "Error should mention version");
    
    // 测试太小的模块
    let result = runtime.load_module(TOO_SMALL);
    assert!(result.is_err(), "Should reject too small WASM module");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("too small"), "Error should mention size");
}

#[test]
fn test_wasm_instance_creation() {
    let runtime = WasmRuntime::new().unwrap();
    let module = runtime.load_module(EMPTY_WASM).unwrap();
    
    // 验证模块已加载 - module() 返回 &Module
    let _module_ref = module.module();
}

#[test]
fn test_host_api_encode_decode() {
    use super::host::{encode_result, decode_ptr, decode_len};

    let ptr = 0x1234i32;
    let len = 0x5678i32;
    
    let encoded = encode_result(ptr, len);
    assert_eq!(decode_ptr(encoded), ptr);
    assert_eq!(decode_len(encoded), len);
}

#[test]
fn test_log_level_conversion() {
    use super::host::LogLevel;

    assert_eq!(LogLevel::from_i32(0), Some(LogLevel::Debug));
    assert_eq!(LogLevel::from_i32(1), Some(LogLevel::Info));
    assert_eq!(LogLevel::from_i32(2), Some(LogLevel::Warn));
    assert_eq!(LogLevel::from_i32(3), Some(LogLevel::Error));
    assert_eq!(LogLevel::from_i32(99), None);
}

#[test]
fn test_skill_builder() {
    let _builder = WasmSkillBuilder::new()
        .name("test-skill")
        .version("1.0.0")
        .description("Test WASM Skill")
        .wasm_bytes(EMPTY_WASM.to_vec());

    // Builder created successfully
}

/// 内存使用测试
#[test]
fn test_memory_page_calculation() {
    // 64KB 每页
    const PAGE_SIZE: usize = 64 * 1024;
    
    // 512MB = 8192 页
    let mb_512 = 512 * 1024 * 1024;
    assert_eq!(mb_512 / PAGE_SIZE, 8192);
    
    // 1GB = 16384 页
    let gb_1 = 1024 * 1024 * 1024;
    assert_eq!(gb_1 / PAGE_SIZE, 16384);
    
    // 4GB = 65536 页（WebAssembly 最大）
    let gb_4 = 4 * 1024 * 1024 * 1024;
    assert_eq!(gb_4 / PAGE_SIZE, 65536);
}
