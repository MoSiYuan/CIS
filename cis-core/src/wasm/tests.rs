//! # WASM 模块测试
//!
//! 需要 WASM 文件来运行完整测试。

#[cfg(test)]
mod tests {
    use super::super::*;

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

    #[test]
    fn test_wasm_runtime_creation() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok(), "Failed to create WASM runtime");
    }

    #[test]
    fn test_load_empty_wasm() {
        let mut runtime = WasmRuntime::new().unwrap();
        let result = runtime.load_skill(EMPTY_WASM);
        assert!(result.is_ok(), "Failed to load empty WASM module");
    }

    #[test]
    fn test_load_add_wasm() {
        let mut runtime = WasmRuntime::new().unwrap();
        let result = runtime.load_skill(ADD_WASM);
        assert!(result.is_ok(), "Failed to load add WASM module");
    }

    #[test]
    fn test_wasm_instance_creation() {
        let mut runtime = WasmRuntime::new().unwrap();
        let instance = runtime.load_skill(EMPTY_WASM).unwrap();
        
        // 验证模块已加载
        assert!(instance.instance().is_none(), "Instance should not be instantiated yet");
    }

    /// 创建一个简单的测试用 WASM Skill
    /// 
    /// 这个测试需要完整的 Host 环境，所以用 `ignore` 标记。
    #[test]
    #[ignore = "Requires complete Host environment setup"]
    fn test_wasm_skill_builder() {
        use crate::memory::MemoryService;
        use std::sync::Mutex as StdMutex;
        use crate::storage::db::DbManager;
        use std::sync::Arc;

        let db_manager = Arc::new(DbManager::new().unwrap());
        let memory_service: Arc<StdMutex<dyn crate::memory::MemoryServiceTrait>> = 
            Arc::new(StdMutex::new(MemoryService::new(db_manager.core())));

        let result = WasmSkillBuilder::new()
            .name("test-skill")
            .version("1.0.0")
            .description("Test WASM Skill")
            .wasm_bytes(EMPTY_WASM.to_vec())
            .memory_service(memory_service);

        assert!(result.is_ok());
    }

    /// 测试 Host API 编码/解码
    #[test]
    fn test_host_api_encode_decode() {
        use super::super::host::{encode_result, decode_ptr, decode_len};

        let ptr = 0x1234i32;
        let len = 0x5678i32;
        
        let encoded = encode_result(ptr, len);
        assert_eq!(decode_ptr(encoded), ptr);
        assert_eq!(decode_len(encoded), len);
    }

    /// 测试日志级别转换
    #[test]
    fn test_log_level_conversion() {
        use super::super::host::LogLevel;

        assert_eq!(LogLevel::from_i32(0), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_i32(1), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_i32(2), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_i32(3), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_i32(99), None);
    }
}
