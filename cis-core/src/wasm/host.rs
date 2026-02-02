//! # Host API 模块
//!
//! 提供给 WASM Skill 调用的 Host 函数。
//!
//! ## API 列表
//!
//! - `host_memory_get`: 读取记忆
//! - `host_memory_set`: 写入记忆
//! - `host_memory_delete`: 删除记忆
//! - `host_ai_chat`: AI 聊天调用
//! - `host_log`: 记录日志
//! - `host_http_post`: HTTP POST 请求
//!
//! ## 内存管理
//!
//! Host API 使用指针和长度来传递数据：
//! - 指针: WASM 线性内存中的偏移量
//! - 长度: 数据的字节数
//! - 返回值: i64 编码的指针和长度（高32位为指针，低32位为长度）

use std::sync::{Arc, Mutex};
use wasmer::{Memory, Store};

use crate::memory::MemoryServiceTrait;

/// Host 环境
///
/// 存储 Host API 需要的上下文信息。
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
    /// 调试级别
    Debug = 0,
    /// 信息级别
    Info = 1,
    /// 警告级别
    Warn = 2,
    /// 错误级别
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

/// 创建 Host 函数导入对象
///
/// 返回一个包含所有 Host API 的导入对象，用于实例化 WASM 模块。
pub fn create_host_imports(
    store: &mut Store,
    env: &wasmer::FunctionEnv<HostEnv>,
) -> wasmer::Imports {
    use wasmer::Function;
    
    let mut imports = wasmer::Imports::new();
    
    // 创建 Host 函数
    let memory_get_fn = Function::new_typed_with_env(store, env, host_memory_get);
    let memory_set_fn = Function::new_typed_with_env(store, env, host_memory_set);
    let memory_delete_fn = Function::new_typed_with_env(store, env, host_memory_delete);
    let ai_chat_fn = Function::new_typed_with_env(store, env, host_ai_chat);
    let log_fn = Function::new_typed_with_env(store, env, host_log);
    let http_post_fn = Function::new_typed_with_env(store, env, host_http_post);
    
    // 添加到导入表
    imports.define("env", "host_memory_get", memory_get_fn);
    imports.define("env", "host_memory_set", memory_set_fn);
    imports.define("env", "host_memory_delete", memory_delete_fn);
    imports.define("env", "host_ai_chat", ai_chat_fn);
    imports.define("env", "host_log", log_fn);
    imports.define("env", "host_http_post", http_post_fn);
    
    imports
}

/// # Host API: 读取记忆
///
/// 从记忆服务读取指定键的值。
///
/// ## 参数
///
/// - `key_ptr`: 键字符串的指针
/// - `key_len`: 键字符串的长度
///
/// ## 返回
///
/// 返回编码的 i64 值：
/// - 高32位: 值数据的指针（如果找不到则为0）
/// - 低32位: 值数据的长度（如果找不到则为0）
pub fn host_memory_get(
    mut env: wasmer::FunctionEnvMut<HostEnv>,
    key_ptr: i32,
    key_len: i32,
) -> i64 {
    // 简化实现：实际应该读取 WASM 内存
    let _ = (key_ptr, key_len);
    
    // 获取环境数据
    let env_data = env.data();
    let _memory = match &env_data.memory {
        Some(m) => m,
        None => {
            tracing::error!("Memory not initialized");
            return 0;
        }
    };
    
    // TODO: 实现从 WASM 内存读取键
    // TODO: 查询记忆服务
    // TODO: 将结果写回 WASM 内存
    
    tracing::debug!("host_memory_get called (stub implementation)");
    0 // 返回0表示未找到
}

/// # Host API: 写入记忆
///
/// 向记忆服务写入键值对。
///
/// ## 参数
///
/// - `key_ptr`: 键字符串的指针
/// - `key_len`: 键字符串的长度
/// - `val_ptr`: 值数据的指针
/// - `val_len`: 值数据的长度
///
/// ## 返回
///
/// - 0: 成功
/// - -1: 失败
pub fn host_memory_set(
    env: wasmer::FunctionEnvMut<HostEnv>,
    key_ptr: i32,
    key_len: i32,
    val_ptr: i32,
    val_len: i32,
) -> i32 {
    // 简化实现
    let _ = (key_ptr, key_len, val_ptr, val_len);
    
    let env_data = env.data();
    let _memory = match &env_data.memory {
        Some(m) => m,
        None => {
            tracing::error!("Memory not initialized");
            return -1;
        }
    };
    
    // TODO: 实现从 WASM 内存读取键和值
    // TODO: 写入记忆服务
    
    tracing::debug!("host_memory_set called (stub implementation)");
    0 // 成功
}

/// # Host API: 删除记忆
///
/// 从记忆服务删除指定键。
///
/// ## 参数
///
/// - `key_ptr`: 键字符串的指针
/// - `key_len`: 键字符串的长度
///
/// ## 返回
///
/// - 0: 成功
/// - -1: 失败
pub fn host_memory_delete(
    env: wasmer::FunctionEnvMut<HostEnv>,
    key_ptr: i32,
    key_len: i32,
) -> i32 {
    // 简化实现
    let _ = (key_ptr, key_len);
    
    let env_data = env.data();
    let _memory = match &env_data.memory {
        Some(m) => m,
        None => {
            tracing::error!("Memory not initialized");
            return -1;
        }
    };
    
    // TODO: 实现从 WASM 内存读取键
    // TODO: 删除记忆服务中的键
    
    tracing::debug!("host_memory_delete called (stub implementation)");
    0 // 成功
}

/// # Host API: AI 聊天
///
/// 调用 AI 服务进行聊天。
///
/// ## 参数
///
/// - `prompt_ptr`: 提示词的指针
/// - `prompt_len`: 提示词的长度
///
/// ## 返回
///
/// 返回编码的 i64 值：
/// - 高32位: 响应数据的指针
/// - 低32位: 响应数据的长度
/// - 0: 失败
pub fn host_ai_chat(
    mut env: wasmer::FunctionEnvMut<HostEnv>,
    prompt_ptr: i32,
    prompt_len: i32,
) -> i64 {
    // 简化实现
    let _ = (prompt_ptr, prompt_len);
    
    let env_data = env.data();
    let _memory = match &env_data.memory {
        Some(m) => m,
        None => {
            tracing::error!("Memory not initialized");
            return 0;
        }
    };
    
    // TODO: 从 WASM 内存读取提示词
    // TODO: 调用 AI 服务
    // TODO: 将响应写回 WASM 内存
    
    tracing::debug!("host_ai_chat called (stub implementation)");
    0 // 返回0表示失败
}

/// # Host API: 记录日志
///
/// 记录日志消息到 Host 日志系统。
///
/// ## 参数
///
/// - `level`: 日志级别（0=Debug, 1=Info, 2=Warn, 3=Error）
/// - `msg_ptr`: 消息字符串的指针
/// - `msg_len`: 消息字符串的长度
pub fn host_log(
    env: wasmer::FunctionEnvMut<HostEnv>,
    level: i32,
    msg_ptr: i32,
    msg_len: i32,
) {
    // 简化实现
    let _ = (msg_ptr, msg_len);
    
    let env_data = env.data();
    let _memory = match &env_data.memory {
        Some(m) => m,
        None => {
            tracing::error!("Memory not initialized");
            return;
        }
    };
    
    // TODO: 从 WASM 内存读取消息
    
    match LogLevel::from_i32(level) {
        Some(LogLevel::Debug) => tracing::debug!("[WASM Skill] log called (stub)"),
        Some(LogLevel::Info) => tracing::info!("[WASM Skill] log called (stub)"),
        Some(LogLevel::Warn) => tracing::warn!("[WASM Skill] log called (stub)"),
        Some(LogLevel::Error) => tracing::error!("[WASM Skill] log called (stub)"),
        None => tracing::info!("[WASM Skill] log called (stub)"),
    }
}

/// # Host API: HTTP POST 请求
///
/// 发送 HTTP POST 请求。
///
/// ## 参数
///
/// - `url_ptr`: URL 字符串的指针
/// - `url_len`: URL 字符串的长度
/// - `body_ptr`: 请求体的指针
/// - `body_len`: 请求体的长度
///
/// ## 返回
///
/// 返回编码的 i64 值：
/// - 高32位: 响应数据的指针
/// - 低32位: 响应数据的长度
/// - 0: 失败
///
/// ## 安全考虑
///
/// 此 API 应该受到权限控制，只有获得 `http` 权限的 Skill 才能调用。
pub fn host_http_post(
    mut env: wasmer::FunctionEnvMut<HostEnv>,
    url_ptr: i32,
    url_len: i32,
    body_ptr: i32,
    body_len: i32,
) -> i64 {
    // 简化实现
    let _ = (url_ptr, url_len, body_ptr, body_len);
    
    let env_data = env.data();
    let _memory = match &env_data.memory {
        Some(m) => m,
        None => {
            tracing::error!("Memory not initialized");
            return 0;
        }
    };
    
    // TODO: 从 WASM 内存读取 URL 和 body
    // TODO: 执行 HTTP 请求
    // TODO: 将响应写回 WASM 内存
    
    tracing::debug!("host_http_post called (stub implementation)");
    0 // 返回0表示失败
}

#[cfg(test)]
mod tests {
    use super::*;

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
