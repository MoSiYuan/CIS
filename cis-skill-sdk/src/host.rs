//! Host API 接口
//!
//! Host API 是 CIS 核心提供给 Skill 的系统调用接口。
//! 在 Native 模式下，直接调用 Rust 函数；在 WASM 模式下，通过 FFI 导入。

use crate::error::Result;
use crate::types::{HttpRequest, HttpResponse, LogLevel};

#[cfg(all(feature = "wasm", not(feature = "native")))]
use alloc::string::String;

#[cfg(all(feature = "wasm", not(feature = "native")))]
use alloc::vec::Vec;

// ==================== Native Host API ====================

#[cfg(feature = "native")]
pub mod native {
    use super::*;

    // 线程安全 Host API
    pub mod thread_safe;
    use super::*;


    /// Native Host API 提供者
    ///
    /// CIS 核心实现此 trait 并注入到 Skill 中
    pub trait HostApi: Send + Sync {
        /// 记录日志
        fn log(&self, level: LogLevel, message: &str);
        
        /// 读取记忆
        fn memory_get(&self, key: &str) -> Option<Vec<u8>>;
        
        /// 写入记忆
        fn memory_set(&self, key: &str, value: &[u8]) -> Result<()>;
        
        /// 删除记忆
        fn memory_delete(&self, key: &str) -> Result<()>;
        
        /// 列出记忆键
        fn memory_list(&self, prefix: &str) -> Vec<String>;
        
        /// 调用 AI
        fn ai_chat(&self, prompt: &str) -> Result<String>;
        
        /// 调用 AI 生成 JSON
        fn ai_generate_json(&self, prompt: &str, schema: &str) -> Result<serde_json::Value>;
        
        /// 发送 HTTP 请求
        fn http_request(&self, request: HttpRequest) -> Result<HttpResponse>;
        
        /// 获取当前时间戳
        fn now_timestamp(&self) -> u64;
        
        /// 生成 UUID
        fn generate_uuid(&self) -> String;
        
        /// 获取环境变量
        fn env_var(&self, key: &str) -> Option<String>;
        
        /// 读取文件
        fn read_file(&self, path: &str) -> Result<Vec<u8>>;
        
        /// 写入文件
        fn write_file(&self, path: &str, content: &[u8]) -> Result<()>;
        
        /// 调用其他 Skill
        fn call_skill(&self, skill_name: &str, method: &str, params: &[u8]) -> Result<Vec<u8>>;
        
        /// 发送事件给其他 Skill
        fn emit_event(&self, target_skill: &str, event: &str, data: &[u8]) -> Result<()>;
    }

    /// Host API 句柄（全局访问）
    static mut HOST_API: Option<&dyn HostApi> = None;

    /// 初始化 Host API
    ///
    /// # Safety
    /// 必须在 Skill 初始化前调用，且只能调用一次
    pub unsafe fn init_host_api(api: &'static dyn HostApi) {
        HOST_API = Some(api);
    }

    /// 获取 Host API
    ///
    /// # Safety
    /// 必须在 init_host_api 之后调用
    pub unsafe fn host_api() -> &'static dyn HostApi {
        HOST_API.expect("Host API not initialized")
    }
}

// ==================== WASM Host API ====================

#[cfg(feature = "wasm")]
pub mod wasm {
    //! WASM Host API 通过 FFI 导入
    //!
    //! 这些函数由 CIS WASM Runtime 提供

    // 记录日志
    #[link(wasm_import_module = "cis")]
    extern "C" {
        pub fn host_log(level: i32, ptr: *const u8, len: usize);
    }

    // 读取记忆
    #[link(wasm_import_module = "cis")]
    extern "C" {
        pub fn host_memory_get(
            key_ptr: *const u8,
            key_len: usize,
            out_ptr: *mut u8,
            out_len: usize,
        ) -> i32;
    }

    // 写入记忆
    #[link(wasm_import_module = "cis")]
    extern "C" {
        pub fn host_memory_set(
            key_ptr: *const u8,
            key_len: usize,
            value_ptr: *const u8,
            value_len: usize,
        ) -> i32;
    }

    // 删除记忆
    #[link(wasm_import_module = "cis")]
    extern "C" {
        pub fn host_memory_delete(key_ptr: *const u8, key_len: usize) -> i32;
    }

    // 调用 AI
    #[link(wasm_import_module = "cis")]
    extern "C" {
        pub fn host_ai_chat(
            prompt_ptr: *const u8,
            prompt_len: usize,
            out_ptr: *mut u8,
            out_len: usize,
        ) -> i32;
    }

    // 发送 HTTP POST
    #[link(wasm_import_module = "cis")]
    extern "C" {
        pub fn host_http_post(
            url_ptr: *const u8,
            url_len: usize,
            body_ptr: *const u8,
            body_len: usize,
            out_ptr: *mut u8,
            out_len: usize,
        ) -> i32;
    }

    // 获取时间戳
    #[link(wasm_import_module = "cis")]
    extern "C" {
        pub fn host_now() -> u64;
    }

    // 分配内存（返回指针）
    #[link(wasm_import_module = "cis")]
    extern "C" {
        pub fn host_alloc(size: usize) -> *mut u8;
    }

    // 释放内存
    #[link(wasm_import_module = "cis")]
    extern "C" {
        pub fn host_free(ptr: *mut u8, size: usize);
    }
}

// ==================== 统一 API 封装 ====================

/// 统一 Host API 封装
///
/// 根据编译目标自动选择 Native 或 WASM 实现
pub struct Host;

impl Host {
    /// 记录日志
    #[cfg(feature = "native")]
    pub fn log(level: LogLevel, message: &str) {
        unsafe {
            native::host_api().log(level, message);
        }
    }
    
    #[cfg(all(feature = "wasm", not(feature = "native")))]
    pub fn log(level: LogLevel, message: &str) {
        use wasm::host_log;
        let level_i32 = match level {
            LogLevel::Trace => 0,
            LogLevel::Debug => 1,
            LogLevel::Info => 2,
            LogLevel::Warn => 3,
            LogLevel::Error => 4,
        };
        let bytes = message.as_bytes();
        unsafe {
            host_log(level_i32, bytes.as_ptr(), bytes.len());
        }
    }
    
    /// 读取记忆
    #[cfg(feature = "native")]
    pub fn memory_get(key: &str) -> Option<Vec<u8>> {
        unsafe { native::host_api().memory_get(key) }
    }
    
    /// 写入记忆
    #[cfg(feature = "native")]
    pub fn memory_set(key: &str, value: &[u8]) -> Result<()> {
        unsafe { native::host_api().memory_set(key, value) }
    }
    
    /// 调用 AI
    #[cfg(feature = "native")]
    pub fn ai_chat(prompt: &str) -> crate::error::Result<String> {
        unsafe { native::host_api().ai_chat(prompt) }
    }
    
    /// 调用 AI 生成 JSON
    #[cfg(feature = "native")]
    pub fn ai_generate_json(prompt: &str, schema: &str) -> crate::error::Result<serde_json::Value> {
        unsafe { native::host_api().ai_generate_json(prompt, schema) }
    }
    
    /// 发送 HTTP 请求
    #[cfg(feature = "native")]
    pub fn http_request(request: HttpRequest) -> Result<HttpResponse> {
        unsafe { native::host_api().http_request(request) }
    }
}
