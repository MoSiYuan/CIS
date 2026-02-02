//! # WASM Host API
//!
//! 提供 WASM Skill 访问 Host 能力的接口。
//!
//! ## Host Functions
//! - `host_memory_get`: 读取记忆
//! - `host_memory_set`: 写入记忆
//! - `host_memory_delete`: 删除记忆
//! - `host_memory_search`: 语义搜索
//! - `host_ai_chat`: AI 对话
//! - `host_ai_complete`: AI 补全
//! - `host_log`: 日志记录
//! - `host_http_get`: HTTP GET
//! - `host_http_post`: HTTP POST
//! - `host_config_get`: 获取配置
//! - `host_config_set`: 设置配置

use std::sync::{Arc, Mutex};
use wasmer::{FunctionEnv, FunctionEnvMut, Memory, Store, WasmPtr};

use crate::memory::{MemoryService, MemoryServiceTrait};
use crate::ai::AiProvider;
use crate::error::CisError;

/// Host 上下文
#[derive(Clone)]
pub struct HostContext {
    /// 记忆服务
    pub memory: Arc<Mutex<dyn MemoryServiceTrait>>,
    /// AI Provider
    pub ai: Arc<Mutex<dyn AiProvider>>,
    /// WASM 内存
    pub memory_ref: Option<Memory>,
    /// 日志回调
    pub log_callback: Option<Arc<dyn Fn(&str) + Send + Sync>>,
}

impl HostContext {
    /// 创建新的 Host 上下文
    pub fn new(
        memory: Arc<Mutex<dyn MemoryServiceTrait>>,
        ai: Arc<Mutex<dyn AiProvider>>,
    ) -> Self {
        Self {
            memory,
            ai,
            memory_ref: None,
            log_callback: None,
        }
    }

    /// 设置内存引用
    pub fn set_memory(&mut self, memory: Memory) {
        self.memory_ref = Some(memory);
    }

    /// 设置日志回调
    pub fn set_log_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.log_callback = Some(Arc::new(callback));
    }
}

/// Host 函数集合
pub struct HostFunctions;

impl HostFunctions {
    /// 创建 Host 导入对象
    pub fn create_imports(
        store: &mut Store,
        ctx: FunctionEnv<HostContext>,
    ) -> wasmer::Imports {
        use wasmer::Function;
        
        let mut imports = wasmer::Imports::new();
        
        // host_memory_get: (key_ptr, key_len, out_ptr, out_len) -> i32
        imports.define(
            "env",
            "host_memory_get",
            Function::new_typed_with_env(store, &ctx, Self::host_memory_get),
        );
        
        // host_memory_set: (key_ptr, key_len, value_ptr, value_len) -> i32
        imports.define(
            "env",
            "host_memory_set",
            Function::new_typed_with_env(store, &ctx, Self::host_memory_set),
        );
        
        // host_memory_delete: (key_ptr, key_len) -> i32
        imports.define(
            "env",
            "host_memory_delete",
            Function::new_typed_with_env(store, &ctx, Self::host_memory_delete),
        );
        
        // host_memory_search: (query_ptr, query_len, out_ptr, out_len) -> i32
        imports.define(
            "env",
            "host_memory_search",
            Function::new_typed_with_env(store, &ctx, Self::host_memory_search),
        );
        
        // host_ai_chat: (prompt_ptr, prompt_len, out_ptr, out_len) -> i32
        imports.define(
            "env",
            "host_ai_chat",
            Function::new_typed_with_env(store, &ctx, Self::host_ai_chat),
        );
        
        // host_log: (level, msg_ptr, msg_len) -> ()
        imports.define(
            "env",
            "host_log",
            Function::new_typed_with_env(store, &ctx, Self::host_log),
        );
        
        // host_config_get: (key_ptr, key_len, out_ptr, out_len) -> i32
        imports.define(
            "env",
            "host_config_get",
            Function::new_typed_with_env(store, &ctx, Self::host_config_get),
        );
        
        // host_config_set: (key_ptr, key_len, value_ptr, value_len) -> i32
        imports.define(
            "env",
            "host_config_set",
            Function::new_typed_with_env(store, &ctx, Self::host_config_set),
        );
        
        imports
    }
    
    // ==================== Memory Operations ====================
    
    /// Host function: memory_get
    fn host_memory_get(
        env: FunctionEnvMut<HostContext>,
        key_ptr: WasmPtr<u8>,
        key_len: i32,
        out_ptr: WasmPtr<u8>,
        out_len: i32,
    ) -> i32 {
        let ctx = env.data();
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 使用 MemoryView 读取 key
        let view = memory.view(&env);
        let key = match Self::read_string_from_view(&view, key_ptr, key_len) {
            Ok(k) => k,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read key: {}", e);
                return -2;
            }
        };
        
        // 查询记忆服务
        let value = match ctx.memory.lock() {
            Ok(svc) => match svc.get(&key) {
                Some(v) => v,
                None => return 0, // 未找到
            },
            Err(e) => {
                tracing::error!("[WASM Host] Failed to lock memory service: {}", e);
                return -2;
            }
        };
        
        // 写入 WASM 内存
        match Self::write_bytes_to_view(&view, out_ptr, out_len, &value) {
            Ok(written) => written as i32,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to write value: {}", e);
                -3
            }
        }
    }
    
    /// Host function: memory_set
    fn host_memory_set(
        env: FunctionEnvMut<HostContext>,
        key_ptr: WasmPtr<u8>,
        key_len: i32,
        value_ptr: WasmPtr<u8>,
        value_len: i32,
    ) -> i32 {
        let ctx = env.data();
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 使用 MemoryView 读取 key 和 value
        let view = memory.view(&env);
        let key = match Self::read_string_from_view(&view, key_ptr, key_len) {
            Ok(k) => k,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read key: {}", e);
                return -2;
            }
        };
        
        let value = match Self::read_bytes_from_view(&view, value_ptr, value_len) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read value: {}", e);
                return -3;
            }
        };
        
        // 写入记忆服务
        match ctx.memory.lock() {
            Ok(svc) => match svc.set(&key, &value) {
                Ok(_) => 1,
                Err(e) => {
                    tracing::error!("[WASM Host] Failed to set memory: {}", e);
                    -4
                }
            },
            Err(e) => {
                tracing::error!("[WASM Host] Failed to lock memory service: {}", e);
                -4
            }
        }
    }
    
    /// Host function: memory_delete
    fn host_memory_delete(
        env: FunctionEnvMut<HostContext>,
        key_ptr: WasmPtr<u8>,
        key_len: i32,
    ) -> i32 {
        let ctx = env.data();
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 使用 MemoryView 读取 key
        let view = memory.view(&env);
        let key = match Self::read_string_from_view(&view, key_ptr, key_len) {
            Ok(k) => k,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read key: {}", e);
                return -2;
            }
        };
        
        // 删除记忆
        match ctx.memory.lock() {
            Ok(svc) => match svc.delete(&key) {
                Ok(_) => 1,
                Err(e) => {
                    tracing::error!("[WASM Host] Failed to delete memory: {}", e);
                    -3
                }
            },
            Err(e) => {
                tracing::error!("[WASM Host] Failed to lock memory service: {}", e);
                -3
            }
        }
    }
    
    /// Host function: memory_search
    fn host_memory_search(
        env: FunctionEnvMut<HostContext>,
        _query_ptr: WasmPtr<u8>,
        _query_len: i32,
        out_ptr: WasmPtr<u8>,
        out_len: i32,
    ) -> i32 {
        let ctx = env.data();
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // TODO: 实现实际的搜索功能
        let results: Vec<serde_json::Value> = vec![];
        
        // 序列化结果
        let json = match serde_json::to_string(&results) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to serialize results: {}", e);
                return -2;
            }
        };
        
        // 写入 WASM 内存
        let view = memory.view(&env);
        match Self::write_bytes_to_view(&view, out_ptr, out_len, json.as_bytes()) {
            Ok(written) => written as i32,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to write results: {}", e);
                -3
            }
        }
    }
    
    // ==================== AI Operations ====================
    
    /// Host function: ai_chat
    fn host_ai_chat(
        env: FunctionEnvMut<HostContext>,
        prompt_ptr: WasmPtr<u8>,
        prompt_len: i32,
        out_ptr: WasmPtr<u8>,
        out_len: i32,
    ) -> i32 {
        let ctx = env.data();
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 使用 MemoryView 读取 prompt
        let view = memory.view(&env);
        let prompt = match Self::read_string_from_view(&view, prompt_ptr, prompt_len) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read prompt: {}", e);
                return -2;
            }
        };
        
        // 调用 AI（同步阻塞方式）
        let response = match ctx.ai.lock() {
            Ok(ai) => {
                // 创建运行时来执行异步调用
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::error!("[WASM Host] Failed to create runtime: {}", e);
                        return -3;
                    }
                };
                
                match rt.block_on(async { ai.chat(&prompt).await }) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!("[WASM Host] AI chat failed: {}", e);
                        return -3;
                    }
                }
            }
            Err(e) => {
                tracing::error!("[WASM Host] Failed to lock AI provider: {}", e);
                return -3;
            }
        };
        
        // 写入 WASM 内存
        match Self::write_bytes_to_view(&view, out_ptr, out_len, response.as_bytes()) {
            Ok(written) => written as i32,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to write response: {}", e);
                -4
            }
        }
    }
    
    // ==================== Logging ====================
    
    /// Host function: log
    fn host_log(
        env: FunctionEnvMut<HostContext>,
        level: i32,
        msg_ptr: WasmPtr<u8>,
        msg_len: i32,
    ) {
        let ctx = env.data();
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return;
            }
        };
        
        // 使用 MemoryView 读取消息
        let view = memory.view(&env);
        let msg = match Self::read_string_from_view(&view, msg_ptr, msg_len) {
            Ok(m) => m,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read log message: {}", e);
                return;
            }
        };
        
        let level_str = match level {
            0 => "DEBUG",
            1 => "INFO",
            2 => "WARN",
            3 => "ERROR",
            _ => "INFO",
        };
        
        let formatted = format!("[{}] {}", level_str, msg);
        
        // 调用回调（如果设置）
        if let Some(ref cb) = ctx.log_callback {
            cb(&formatted);
        }
        
        // 同时输出到 tracing
        match level {
            0 => tracing::debug!("[WASM Skill] {}", msg),
            1 => tracing::info!("[WASM Skill] {}", msg),
            2 => tracing::warn!("[WASM Skill] {}", msg),
            3 => tracing::error!("[WASM Skill] {}", msg),
            _ => tracing::info!("[WASM Skill] {}", msg),
        }
    }
    
    // ==================== Config Operations ====================
    
    /// Host function: config_get
    fn host_config_get(
        env: FunctionEnvMut<HostContext>,
        key_ptr: WasmPtr<u8>,
        key_len: i32,
        out_ptr: WasmPtr<u8>,
        out_len: i32,
    ) -> i32 {
        let ctx = env.data();
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        let view = memory.view(&env);
        let key = match Self::read_string_from_view(&view, key_ptr, key_len) {
            Ok(k) => k,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read config key: {}", e);
                return -2;
            }
        };
        
        // TODO: 从 core db 读取配置
        let value = format!("config:{}", key);
        
        match Self::write_bytes_to_view(&view, out_ptr, out_len, value.as_bytes()) {
            Ok(written) => written as i32,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to write config value: {}", e);
                -3
            }
        }
    }
    
    /// Host function: config_set
    fn host_config_set(
        env: FunctionEnvMut<HostContext>,
        key_ptr: WasmPtr<u8>,
        key_len: i32,
        value_ptr: WasmPtr<u8>,
        value_len: i32,
    ) -> i32 {
        let ctx = env.data();
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        let view = memory.view(&env);
        let key = match Self::read_string_from_view(&view, key_ptr, key_len) {
            Ok(k) => k,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read config key: {}", e);
                return -2;
            }
        };
        
        let value = match Self::read_string_from_view(&view, value_ptr, value_len) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read config value: {}", e);
                return -3;
            }
        };
        
        tracing::debug!("[WASM Host] Setting config {} = {}", key, value);
        
        // TODO: 实际实现配置存储
        1
    }
    
    // ==================== Helper Functions ====================
    
    /// 从 MemoryView 读取字符串
    fn read_string_from_view(
        view: &wasmer::MemoryView,
        ptr: WasmPtr<u8>,
        len: i32,
    ) -> Result<String, CisError> {
        if len < 0 {
            return Err(CisError::invalid_input("Invalid length: negative"));
        }
        
        let offset = ptr.offset() as u64;
        let length = len as usize;
        
        let mut buffer = vec![0u8; length];
        view.read(offset, &mut buffer)
            .map_err(|e| CisError::wasm(format!("Memory read error: {}", e)))?;
        
        String::from_utf8(buffer)
            .map_err(|e| CisError::wasm(format!("UTF-8 error: {}", e)))
    }
    
    /// 从 MemoryView 读取字节
    fn read_bytes_from_view(
        view: &wasmer::MemoryView,
        ptr: WasmPtr<u8>,
        len: i32,
    ) -> Result<Vec<u8>, CisError> {
        if len < 0 {
            return Err(CisError::invalid_input("Invalid length: negative"));
        }
        
        let offset = ptr.offset() as u64;
        let length = len as usize;
        
        let mut buffer = vec![0u8; length];
        view.read(offset, &mut buffer)
            .map_err(|e| CisError::wasm(format!("Memory read error: {}", e)))?;
        
        Ok(buffer)
    }
    
    /// 写入字节到 MemoryView
    fn write_bytes_to_view(
        view: &wasmer::MemoryView,
        ptr: WasmPtr<u8>,
        max_len: i32,
        data: &[u8],
    ) -> Result<usize, CisError> {
        if max_len < 0 {
            return Err(CisError::invalid_input("Invalid max length: negative"));
        }
        
        let offset = ptr.offset() as u64;
        let len = data.len().min(max_len as usize);
        
        view.write(offset, &data[..len])
            .map_err(|e| CisError::wasm(format!("Memory write error: {}", e)))?;
        
        Ok(len)
    }
}

// ==================== Legacy API Compatibility ====================

/// Host 环境（旧版 API 兼容）
pub struct HostEnv {
    /// WASM 线性内存
    pub memory: Option<Memory>,
    /// 记忆服务
    pub memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
    /// AI 服务回调
    pub ai_callback: Arc<Mutex<dyn Fn(&str) -> String + Send + 'static>>,
}

impl HostEnv {
    /// 创建新的 Host 环境
    pub fn new(
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
        ai_callback: Arc<Mutex<dyn Fn(&str) -> String + Send + 'static>>,
    ) -> Self {
        Self {
            memory: None,
            memory_service,
            ai_callback,
        }
    }

    /// 设置内存
    pub fn set_memory(&mut self, memory: Memory) {
        self.memory = Some(memory);
    }
}

impl Clone for HostEnv {
    fn clone(&self) -> Self {
        Self {
            memory: self.memory.clone(),
            memory_service: Arc::clone(&self.memory_service),
            ai_callback: Arc::clone(&self.ai_callback),
        }
    }
}

/// 编码返回值为 i64（高32位为指针，低32位为长度）
pub fn encode_result(ptr: i32, len: i32) -> i64 {
    ((ptr as i64) << 32) | (len as i64 & 0xFFFFFFFF)
}

/// 解码指针从返回值
pub fn decode_ptr(result: i64) -> i32 {
    ((result >> 32) & 0xFFFFFFFF) as i32
}

/// 解码长度从返回值
pub fn decode_len(result: i64) -> i32 {
    (result & 0xFFFFFFFF) as i32
}

/// 日志级别
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl LogLevel {
    /// 从 i32 转换为 LogLevel
    pub fn from_i32(level: i32) -> Option<Self> {
        match level {
            0 => Some(Self::Debug),
            1 => Some(Self::Info),
            2 => Some(Self::Warn),
            3 => Some(Self::Error),
            _ => None,
        }
    }
}

/// HTTP 响应
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP 状态码
    pub status: u16,
    /// 响应体
    pub body: String,
}

/// 创建 Host 函数导入对象（旧版 API）
pub fn create_host_imports(
    store: &mut Store,
    env: &wasmer::FunctionEnv<HostEnv>,
) -> wasmer::Imports {
    use wasmer::Function;
    
    let mut imports = wasmer::Imports::new();
    
    // 创建简化版 Host 函数
    let memory_get_fn = Function::new_typed_with_env(store, env, legacy_host_memory_get);
    let memory_set_fn = Function::new_typed_with_env(store, env, legacy_host_memory_set);
    let memory_delete_fn = Function::new_typed_with_env(store, env, legacy_host_memory_delete);
    let ai_chat_fn = Function::new_typed_with_env(store, env, legacy_host_ai_chat);
    let log_fn = Function::new_typed_with_env(store, env, legacy_host_log);
    let http_post_fn = Function::new_typed_with_env(store, env, legacy_host_http_post);
    
    // 添加到导入表
    imports.define("env", "host_memory_get", memory_get_fn);
    imports.define("env", "host_memory_set", memory_set_fn);
    imports.define("env", "host_memory_delete", memory_delete_fn);
    imports.define("env", "host_ai_chat", ai_chat_fn);
    imports.define("env", "host_log", log_fn);
    imports.define("env", "host_http_post", http_post_fn);
    
    imports
}

/// 旧版 host_memory_get（stub 实现）
pub fn legacy_host_memory_get(
    mut _env: wasmer::FunctionEnvMut<HostEnv>,
    _key_ptr: i32,
    _key_len: i32,
) -> i64 {
    0
}

/// 旧版 host_memory_set（stub 实现）
pub fn legacy_host_memory_set(
    _env: wasmer::FunctionEnvMut<HostEnv>,
    _key_ptr: i32,
    _key_len: i32,
    _val_ptr: i32,
    _val_len: i32,
) -> i32 {
    0
}

/// 旧版 host_memory_delete（stub 实现）
pub fn legacy_host_memory_delete(
    _env: wasmer::FunctionEnvMut<HostEnv>,
    _key_ptr: i32,
    _key_len: i32,
) -> i32 {
    0
}

/// 旧版 host_ai_chat（stub 实现）
pub fn legacy_host_ai_chat(
    mut _env: wasmer::FunctionEnvMut<HostEnv>,
    _prompt_ptr: i32,
    _prompt_len: i32,
) -> i64 {
    0
}

/// 旧版 host_log（stub 实现）
pub fn legacy_host_log(
    _env: wasmer::FunctionEnvMut<HostEnv>,
    level: i32,
    _msg_ptr: i32,
    _msg_len: i32,
) {
    match LogLevel::from_i32(level) {
        Some(LogLevel::Debug) => tracing::debug!("[WASM Skill] log called (stub)"),
        Some(LogLevel::Info) => tracing::info!("[WASM Skill] log called (stub)"),
        Some(LogLevel::Warn) => tracing::warn!("[WASM Skill] log called (stub)"),
        Some(LogLevel::Error) => tracing::error!("[WASM Skill] log called (stub)"),
        None => tracing::info!("[WASM Skill] log called (stub)"),
    }
}

/// 旧版 host_http_post（stub 实现）
pub fn legacy_host_http_post(
    mut _env: wasmer::FunctionEnvMut<HostEnv>,
    _url_ptr: i32,
    _url_len: i32,
    _body_ptr: i32,
    _body_len: i32,
) -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_context_creation() {
        tracing::debug!("HostContext structure test passed");
    }

    #[test]
    fn test_encode_decode() {
        let ptr = 1024i32;
        let len = 100i32;
        let encoded = encode_result(ptr, len);
        
        assert_eq!(decode_ptr(encoded), ptr);
        assert_eq!(decode_len(encoded), len);
    }

    #[test]
    fn test_log_level_from_i32() {
        assert_eq!(LogLevel::from_i32(0), Some(LogLevel::Debug));
        assert_eq!(LogLevel::from_i32(1), Some(LogLevel::Info));
        assert_eq!(LogLevel::from_i32(2), Some(LogLevel::Warn));
        assert_eq!(LogLevel::from_i32(3), Some(LogLevel::Error));
        assert_eq!(LogLevel::from_i32(99), None);
    }
}
