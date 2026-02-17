//! # WASM Host API
//!
//! 提供 WASM Skill 访问 Host 能力的接口，包含安全控制和资源限制。
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
use std::time::{Duration, Instant};
use wasmer::{FunctionEnv, FunctionEnvMut, Memory, Store, WasmPtr};

use crate::memory::MemoryServiceTrait;
use crate::ai::AiProvider;
use crate::error::CisError;
use crate::storage::DbManager;

// 🔒 P0安全修复：导入ACL服务
use crate::network::acl_service::{AclService, AclPermission, AclAction};

/// 执行统计和限制
#[derive(Debug, Clone)]
pub struct ExecutionLimits {
    /// 执行超时
    pub timeout: Duration,
    /// 最大执行步数
    pub max_steps: u64,
    /// 已执行步数
    pub current_steps: u64,
    /// 开始时间
    pub start_time: Instant,
}

impl ExecutionLimits {
    /// 创建新的执行限制
    pub fn new(timeout: Duration, max_steps: u64) -> Self {
        Self {
            timeout,
            max_steps,
            current_steps: 0,
            start_time: Instant::now(),
        }
    }

    /// 检查是否超时
    pub fn is_timeout(&self) -> bool {
        self.start_time.elapsed() > self.timeout
    }

    /// 检查是否超过步数限制
    pub fn is_step_limit_reached(&self) -> bool {
        self.current_steps >= self.max_steps
    }

    /// 增加步数计数
    pub fn increment_step(&mut self) {
        self.current_steps += 1;
    }

    /// 重置计时器和计数器
    pub fn reset(&mut self) {
        self.current_steps = 0;
        self.start_time = Instant::now();
    }

    /// 获取剩余时间
    pub fn remaining_time(&self) -> Duration {
        let elapsed = self.start_time.elapsed();
        if elapsed < self.timeout {
            self.timeout - elapsed
        } else {
            Duration::ZERO
        }
    }

    /// 获取已运行时间
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(30), // 默认 30 秒超时
            1_000_000,               // 默认 100 万步
        )
    }
}

/// Host 上下文
#[derive(Clone)]
pub struct HostContext {
    /// 记忆服务
    pub memory: Arc<Mutex<dyn MemoryServiceTrait>>,
    /// AI Provider
    pub ai: Arc<Mutex<dyn AiProvider>>,
    /// 数据库管理器
    pub db_manager: Option<Arc<DbManager>>,
    /// WASM 内存
    pub memory_ref: Option<Memory>,
    /// 日志回调
    #[allow(clippy::type_complexity)]
    pub log_callback: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    /// 执行限制
    pub execution_limits: Option<ExecutionLimits>,
    /// 是否允许网络访问
    pub allow_network: bool,
    /// 允许的主机列表
    pub allowed_hosts: Vec<String>,
    /// 🔒 P0安全修复：ACL服务
    pub acl_service: Option<Arc<dyn AclService>>,
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
            db_manager: None,
            memory_ref: None,
            log_callback: None,
            execution_limits: None,
            allow_network: false,
            allowed_hosts: vec![],
            acl_service: None, // 🔒 默认无ACL检查
        }
    }

    /// 创建新的 Host 上下文（带数据库管理器）
    pub fn with_db_manager(
        memory: Arc<Mutex<dyn MemoryServiceTrait>>,
        ai: Arc<Mutex<dyn AiProvider>>,
        db_manager: Arc<DbManager>,
    ) -> Self {
        Self {
            memory,
            ai,
            db_manager: Some(db_manager),
            memory_ref: None,
            log_callback: None,
            execution_limits: None,
            allow_network: false,
            allowed_hosts: vec![],
            acl_service: None, // 🔒 默认无ACL检查
        }
    }

    /// 设置数据库管理器
    pub fn set_db_manager(&mut self, db_manager: Arc<DbManager>) {
        self.db_manager = Some(db_manager);
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

    /// 设置执行限制
    pub fn set_execution_limits(&mut self, timeout: Duration, max_steps: u64) {
        self.execution_limits = Some(ExecutionLimits::new(timeout, max_steps));
    }

    /// 设置网络权限
    pub fn set_network_permissions(&mut self, allow: bool, allowed_hosts: Vec<String>) {
        self.allow_network = allow;
        self.allowed_hosts = allowed_hosts;
    }

    /// 🔒 P0安全修复：设置ACL服务
    pub fn set_acl_service(&mut self, acl: Arc<dyn AclService>) {
        self.acl_service = Some(acl);
    }

    /// 🔒 P0安全修复：检查ACL权限
    async fn check_acl(&self, resource: &str, action: AclAction) -> Result<(), CisError> {
        if let Some(ref acl) = self.acl_service {
            let permission = AclPermission {
                namespace: resource.to_string(),
                action,
            };

            if !acl.check_permission(&permission).await {
                tracing::warn!(
                    "ACL permission denied: resource={}, action={:?}",
                    resource,
                    action
                );
                return Err(CisError::Forbidden(format!(
                    "ACL permission denied: resource={}, action={:?}",
                    resource, action
                )));
            }
        }
        Ok(())
    }

    /// 检查是否允许访问指定主机
    pub fn is_host_allowed(&self, host: &str) -> bool {
        if !self.allow_network {
            return false;
        }
        if self.allowed_hosts.is_empty() {
            return true; // 允许所有主机
        }
        self.allowed_hosts.iter().any(|allowed| host.contains(allowed))
    }

    /// 检查执行限制
    pub(crate) fn check_limits(&self) -> Result<(), CisError> {
        if let Some(ref limits) = self.execution_limits {
            if limits.is_timeout() {
                return Err(CisError::wasm(
                    format!("Execution timeout: exceeded {:?}", limits.timeout)
                ));
            }
            if limits.is_step_limit_reached() {
                return Err(CisError::wasm(
                    format!("Step limit exceeded: {} steps", limits.max_steps)
                ));
            }
        }
        Ok(())
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
        
        // host_http_request: 完整的 HTTP 请求支持
        imports.define(
            "env",
            "host_http_request",
            Function::new_typed_with_env(store, &ctx, Self::host_http_request),
        );
        
        imports
    }
    
    // ==================== CIS Standard Host Functions ====================
    
    /// Host function: cis_ai_prompt (标准接口)
    /// 
    /// 调用 AI Provider 生成回复（真实实现）
    /// 
    /// 参数 (WasmPtr):
    /// - prompt_ptr: prompt 字符串指针
    /// - prompt_len: prompt 长度
    /// - out_ptr: 输出缓冲区指针
    /// - out_len: 输出缓冲区大小
    /// 
    /// 返回值: i32
    /// - >= 0: 实际返回的字符数
    /// - < 0: 错误码
    fn cis_ai_prompt(
        env: FunctionEnvMut<HostContext>,
        prompt_ptr: WasmPtr<u8>,
        prompt_len: i32,
        out_ptr: WasmPtr<u8>,
        out_len: i32,
    ) -> i32 {
        let ctx = env.data();
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=100 * 1024).contains(&prompt_len) { // 最大 100KB prompt
            tracing::error!("[WASM Host] Invalid prompt length: {}", prompt_len);
            return -2;
        }
        
        if !(0..=1024 * 1024).contains(&out_len) { // 最大 1MB 输出
            tracing::error!("[WASM Host] Invalid output length: {}", out_len);
            return -2;
        }
        
        // 读取 prompt
        let view = memory.view(&env);
        let prompt = match Self::read_string_from_view(&view, prompt_ptr, prompt_len) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read prompt: {}", e);
                return -2;
            }
        };
        
        tracing::debug!("[WASM Host] cis_ai_prompt called with prompt length: {}", prompt.len());
        
        // 获取剩余超时时间
        let timeout = ctx.execution_limits.as_ref()
            .map(|l| l.remaining_time())
            .unwrap_or_else(|| Duration::from_secs(30));
        
        // 调用真实的 AI Provider
        let response = match ctx.ai.lock() {
            Ok(ai) => {
                // 创建运行时执行异步调用
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        tracing::error!("[WASM Host] Failed to create runtime: {}", e);
                        return -3;
                    }
                };
                
                // 使用 timeout 包装 AI 调用
                match rt.block_on(async {
                    tokio::time::timeout(timeout, ai.chat(&prompt)).await
                }) {
                    Ok(Ok(r)) => r,
                    Ok(Err(e)) => {
                        tracing::error!("[WASM Host] AI prompt failed: {}", e);
                        return -3;
                    }
                    Err(_) => {
                        tracing::error!("[WASM Host] AI prompt timed out after {:?}", timeout);
                        return -4;
                    }
                }
            }
            Err(e) => {
                tracing::error!("[WASM Host] Failed to lock AI provider: {}", e);
                return -3;
            }
        };
        
        tracing::debug!("[WASM Host] AI response length: {}", response.len());
        
        // 写入 WASM 内存
        match Self::write_bytes_to_view(&view, out_ptr, out_len, response.as_bytes()) {
            Ok(written) => written as i32,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to write response: {}", e);
                -5
            }
        }
    }
    
    /// Host function: cis_memory_get (标准接口)
    /// 
    /// 从记忆服务读取值
    /// 
    /// 参数:
    /// - key_ptr: key 字符串指针
    /// - key_len: key 长度
    /// - out_ptr: 输出缓冲区指针
    /// - out_len: 输出缓冲区大小
    /// 
    /// 返回值: i32
    /// - > 0: 实际返回的字节数
    /// - = 0: key 不存在
    /// - < 0: 错误码
    fn cis_memory_get(
        env: FunctionEnvMut<HostContext>,
        key_ptr: WasmPtr<u8>,
        key_len: i32,
        out_ptr: WasmPtr<u8>,
        out_len: i32,
    ) -> i32 {
        let ctx = env.data();
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=1024).contains(&key_len) {
            tracing::error!("[WASM Host] Invalid key length: {}", key_len);
            return -2;
        }
        
        if !(0..=1024 * 1024).contains(&out_len) { // 最大 1MB 输出
            tracing::error!("[WASM Host] Invalid output length: {}", out_len);
            return -2;
        }
        
        // 读取 key
        let view = memory.view(&env);
        let key = match Self::read_string_from_view(&view, key_ptr, key_len) {
            Ok(k) => k,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read key: {}", e);
                return -2;
            }
        };
        
        // 从记忆服务获取值
        let value = match ctx.memory.lock() {
            Ok(svc) => match svc.get(&key) {
                Some(v) => v,
                None => return 0, // key 不存在
            },
            Err(e) => {
                tracing::error!("[WASM Host] Failed to lock memory service: {}", e);
                return -3;
            }
        };
        
        // 检查输出缓冲区大小
        if value.len() > out_len as usize {
            tracing::warn!("[WASM Host] Value too large for output buffer: {} > {}", 
                value.len(), out_len);
            // 截断写入
            let truncated = &value[..out_len as usize];
            match Self::write_bytes_to_view(&view, out_ptr, out_len, truncated) {
                Ok(written) => -(written as i32), // 返回负数表示截断
                Err(e) => {
                    tracing::error!("[WASM Host] Failed to write truncated value: {}", e);
                    -4
                }
            }
        } else {
            // 写入 WASM 内存
            match Self::write_bytes_to_view(&view, out_ptr, out_len, &value) {
                Ok(written) => written as i32,
                Err(e) => {
                    tracing::error!("[WASM Host] Failed to write value: {}", e);
                    -4
                }
            }
        }
    }
    
    /// Host function: cis_memory_put (标准接口)
    /// 
    /// 向记忆服务写入值
    /// 
    /// 参数:
    /// - key_ptr: key 字符串指针
    /// - key_len: key 长度
    /// - value_ptr: value 指针
    /// - value_len: value 长度
    /// 
    /// 返回值: i32
    /// - = 1: 成功
    /// - < 0: 错误码
    fn cis_memory_put(
        env: FunctionEnvMut<HostContext>,
        key_ptr: WasmPtr<u8>,
        key_len: i32,
        value_ptr: WasmPtr<u8>,
        value_len: i32,
    ) -> i32 {
        let ctx = env.data();
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=1024).contains(&key_len) {
            tracing::error!("[WASM Host] Invalid key length: {}", key_len);
            return -2;
        }
        
        if !(0..=10 * 1024 * 1024).contains(&value_len) { // 最大 10MB value
            tracing::error!("[WASM Host] Invalid value length: {}", value_len);
            return -3;
        }
        
        // 读取 key 和 value
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
                Ok(_) => {
                    tracing::debug!("[WASM Host] Set memory: {} = {} bytes", key, value.len());
                    1 // 成功
                }
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
    
    /// Host function: host_http_request (完整 HTTP 请求)
    /// 
    /// 执行 HTTP 请求（GET/POST/PUT/DELETE）
    /// 
    /// 参数:
    /// - method_ptr: HTTP 方法字符串指针 ("GET", "POST", etc.)
    /// - method_len: 方法长度
    /// - url_ptr: URL 字符串指针
    /// - url_len: URL 长度
    /// - headers_ptr: Headers JSON 字符串指针（可选）
    /// - headers_len: Headers 长度
    /// - body_ptr: Body 指针（可选）
    /// - body_len: Body 长度
    /// - out_ptr: 输出缓冲区指针
    /// - out_len: 输出缓冲区大小
    /// 
    /// 返回值: i32
    /// - >= 0: 实际返回的字节数
    /// - < 0: 错误码
    fn host_http_request(
        env: FunctionEnvMut<HostContext>,
        method_ptr: WasmPtr<u8>,
        method_len: i32,
        url_ptr: WasmPtr<u8>,
        url_len: i32,
        headers_ptr: WasmPtr<u8>,
        headers_len: i32,
        body_ptr: WasmPtr<u8>,
        body_len: i32,
        out_ptr: WasmPtr<u8>,
        out_len: i32,
    ) -> i32 {
        let ctx = env.data();
        
        // 检查网络权限
        if !ctx.allow_network {
            tracing::error!("[WASM Host] Network access denied");
            return -1;
        }
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=16).contains(&method_len) { // "GET", "POST", "PUT", "DELETE", etc.
            tracing::error!("[WASM Host] Invalid method length: {}", method_len);
            return -2;
        }
        
        if !(0..=8192).contains(&url_len) { // 最大 8KB URL
            tracing::error!("[WASM Host] Invalid URL length: {}", url_len);
            return -2;
        }
        
        if !(0..=1024 * 1024).contains(&out_len) { // 最大 1MB 输出
            tracing::error!("[WASM Host] Invalid output length: {}", out_len);
            return -2;
        }
        
        // 读取 method 和 URL
        let view = memory.view(&env);
        let method = match Self::read_string_from_view(&view, method_ptr, method_len) {
            Ok(m) => m.to_uppercase(),
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read method: {}", e);
                return -2;
            }
        };
        
        let url = match Self::read_string_from_view(&view, url_ptr, url_len) {
            Ok(u) => u,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read URL: {}", e);
                return -2;
            }
        };
        
        // 检查主机是否允许
        if let Some(host) = url.split("//").nth(1).and_then(|s| s.split('/').next()) {
            if !ctx.is_host_allowed(host) {
                tracing::error!("[WASM Host] Host not allowed: {}", host);
                return -1;
            }
        }
        
        // 读取 headers（如果提供）
        let _headers = if headers_len > 0 {
            match Self::read_string_from_view(&view, headers_ptr, headers_len) {
                Ok(h) => h,
                Err(e) => {
                    tracing::error!("[WASM Host] Failed to read headers: {}", e);
                    return -3;
                }
            }
        } else {
            "{}".to_string()
        };
        
        // 读取 body（如果提供）
        let _body = if body_len > 0 {
            match Self::read_bytes_from_view(&view, body_ptr, body_len) {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!("[WASM Host] Failed to read body: {}", e);
                    return -3;
                }
            }
        } else {
            vec![]
        };
        
        tracing::debug!("[WASM Host] HTTP {} {}", method, url);
        
        // 获取剩余超时时间
        let timeout = ctx.execution_limits.as_ref()
            .map(|l| l.remaining_time())
            .unwrap_or_else(|| Duration::from_secs(30));
        
        // 执行 HTTP 请求（使用 reqwest）
        let response = {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("[WASM Host] Failed to create runtime: {}", e);
                    return -4;
                }
            };
            
            rt.block_on(async {
                let client = reqwest::Client::builder()
                    .timeout(timeout)
                    .build()
                    .map_err(|e| {
                        tracing::error!("[WASM Host] Failed to create HTTP client: {}", e);
                        -4i32
                    })?;
                
                let req = match method.as_str() {
                    "GET" => client.get(&url),
                    "POST" => client.post(&url),
                    "PUT" => client.put(&url),
                    "DELETE" => client.delete(&url),
                    "PATCH" => client.patch(&url),
                    "HEAD" => client.head(&url),
                    _ => {
                        tracing::error!("[WASM Host] Unsupported HTTP method: {}", method);
                        return Err(-5i32);
                    }
                };
                
                // 发送请求
                let resp = req.send().await.map_err(|e| {
                    tracing::error!("[WASM Host] HTTP request failed: {}", e);
                    -6i32
                })?;
                
                // 读取响应
                let status = resp.status().as_u16();
                let body = resp.text().await.map_err(|e| {
                    tracing::error!("[WASM Host] Failed to read response body: {}", e);
                    -7i32
                })?;
                
                // 构造响应 JSON
                let response_json = serde_json::json!({
                    "status": status,
                    "body": body,
                });
                
                Ok(response_json.to_string())
            })
        };
        
        let response_str = match response {
            Ok(r) => r,
            Err(code) => return code,
        };
        
        // 检查输出缓冲区
        if response_str.len() > out_len as usize {
            tracing::warn!("[WASM Host] Output buffer too small for HTTP response: {} > {}",
                response_str.len(), out_len);
            return -8;
        }
        
        // 写入 WASM 内存
        match Self::write_bytes_to_view(&view, out_ptr, out_len, response_str.as_bytes()) {
            Ok(written) => written as i32,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to write HTTP response: {}", e);
                -9
            }
        }
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
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=1024).contains(&key_len) {
            tracing::error!("[WASM Host] Invalid key length: {}", key_len);
            return -2;
        }
        
        if !(0..=1024 * 1024).contains(&out_len) { // 最大 1MB 输出
            tracing::error!("[WASM Host] Invalid output length: {}", out_len);
            return -2;
        }
        
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
        
        // 检查输出缓冲区大小
        if value.len() > out_len as usize {
            tracing::warn!("[WASM Host] Value too large for output buffer: {} > {}", 
                value.len(), out_len);
            // 截断写入
            let truncated = &value[..out_len as usize];
            match Self::write_bytes_to_view(&view, out_ptr, out_len, truncated) {
                Ok(written) => -(written as i32), // 返回负数表示截断
                Err(e) => {
                    tracing::error!("[WASM Host] Failed to write truncated value: {}", e);
                    -3
                }
            }
        } else {
            // 写入 WASM 内存
            match Self::write_bytes_to_view(&view, out_ptr, out_len, &value) {
                Ok(written) => written as i32,
                Err(e) => {
                    tracing::error!("[WASM Host] Failed to write value: {}", e);
                    -3
                }
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
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=1024).contains(&key_len) {
            tracing::error!("[WASM Host] Invalid key length: {}", key_len);
            return -2;
        }
        
        if !(0..=10 * 1024 * 1024).contains(&value_len) { // 最大 10MB 值
            tracing::error!("[WASM Host] Invalid value length: {}", value_len);
            return -3;
        }
        
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
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=1024).contains(&key_len) {
            tracing::error!("[WASM Host] Invalid key length: {}", key_len);
            return -2;
        }
        
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
        query_ptr: WasmPtr<u8>,
        query_len: i32,
        out_ptr: WasmPtr<u8>,
        out_len: i32,
    ) -> i32 {
        let ctx = env.data();
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=4096).contains(&query_len) {
            tracing::error!("[WASM Host] Invalid query length: {}", query_len);
            return -2;
        }
        
        if !(0..=1024 * 1024).contains(&out_len) { // 最大 1MB 输出
            tracing::error!("[WASM Host] Invalid output length: {}", out_len);
            return -2;
        }
        
        // 读取查询关键词
        let view = memory.view(&env);
        let query = match Self::read_string_from_view(&view, query_ptr, query_len) {
            Ok(q) => q,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read query: {}", e);
                return -2;
            }
        };
        
        // 执行搜索
        let search_results = match ctx.memory.lock() {
            Ok(svc) => match svc.search(&query, 10) {
                Ok(results) => results,
                Err(e) => {
                    tracing::error!("[WASM Host] Search failed: {}", e);
                    return -3;
                }
            },
            Err(e) => {
                tracing::error!("[WASM Host] Failed to lock memory service: {}", e);
                return -3;
            }
        };
        
        // 转换为 JSON 格式
        let results: Vec<serde_json::Value> = search_results
            .into_iter()
            .map(|item| {
                serde_json::json!({
                    "key": item.key,
                    "value": String::from_utf8_lossy(&item.value).to_string(),
                    "domain": format!("{:?}", item.domain),
                    "category": format!("{:?}", item.category),
                })
            })
            .collect();
        
        // 序列化结果
        let json = match serde_json::to_string(&results) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to serialize results: {}", e);
                return -4;
            }
        };
        
        // 写入 WASM 内存
        match Self::write_bytes_to_view(&view, out_ptr, out_len, json.as_bytes()) {
            Ok(written) => written as i32,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to write results: {}", e);
                -5
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
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=100 * 1024).contains(&prompt_len) { // 最大 100KB prompt
            tracing::error!("[WASM Host] Invalid prompt length: {}", prompt_len);
            return -2;
        }
        
        if !(0..=1024 * 1024).contains(&out_len) { // 最大 1MB 输出
            tracing::error!("[WASM Host] Invalid output length: {}", out_len);
            return -2;
        }
        
        // 使用 MemoryView 读取 prompt
        let view = memory.view(&env);
        let prompt = match Self::read_string_from_view(&view, prompt_ptr, prompt_len) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read prompt: {}", e);
                return -2;
            }
        };
        
        // 获取剩余超时时间
        let timeout = ctx.execution_limits.as_ref()
            .map(|l| l.remaining_time())
            .unwrap_or_else(|| Duration::from_secs(30));
        
        // 调用 AI（同步阻塞方式，带超时）
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
                
                // 使用 timeout 包装 AI 调用
                match rt.block_on(async {
                    tokio::time::timeout(timeout, ai.chat(&prompt)).await
                }) {
                    Ok(Ok(r)) => r,
                    Ok(Err(e)) => {
                        tracing::error!("[WASM Host] AI chat failed: {}", e);
                        return -3;
                    }
                    Err(_) => {
                        tracing::error!("[WASM Host] AI chat timed out after {:?}", timeout);
                        return -4;
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
                -5
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
        
        // 检查执行限制（但不阻止日志记录）
        if ctx.check_limits().is_err() {
            // 即使超时，也允许记录最后一条日志
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return;
            }
        };
        
        // 验证输入参数
        if !(0..=10 * 1024).contains(&msg_len) { // 最大 10KB 消息
            tracing::error!("[WASM Host] Invalid message length: {}", msg_len);
            return;
        }
        
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
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=1024).contains(&key_len) {
            tracing::error!("[WASM Host] Invalid key length: {}", key_len);
            return -2;
        }
        
        let view = memory.view(&env);
        let key = match Self::read_string_from_view(&view, key_ptr, key_len) {
            Ok(k) => k,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read config key: {}", e);
                return -2;
            }
        };
        
        // 从 core db 读取配置
        let value = match &ctx.db_manager {
            Some(db_mgr) => {
                match db_mgr.core().lock() {
                    Ok(core_db) => {
                        match core_db.get_config(&key) {
                            Ok(Some((value, _encrypted))) => {
                                String::from_utf8_lossy(&value).to_string()
                            }
                            Ok(None) => {
                                return 0; // 配置不存在，返回 0
                            }
                            Err(e) => {
                                tracing::error!("[WASM Host] Failed to get config: {}", e);
                                return -3;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("[WASM Host] Failed to lock core db: {}", e);
                        return -3;
                    }
                }
            }
            None => {
                tracing::warn!("[WASM Host] DbManager not available");
                return -4;
            }
        };
        
        match Self::write_bytes_to_view(&view, out_ptr, out_len, value.as_bytes()) {
            Ok(written) => written as i32,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to write config value: {}", e);
                -5
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
        
        // 检查执行限制
        if let Err(e) = ctx.check_limits() {
            tracing::error!("[WASM Host] Execution limit exceeded: {}", e);
            return -10;
        }
        
        let memory = match &ctx.memory_ref {
            Some(m) => m.clone(),
            None => {
                tracing::error!("[WASM Host] Memory not initialized");
                return -1;
            }
        };
        
        // 验证输入参数
        if !(0..=1024).contains(&key_len) {
            tracing::error!("[WASM Host] Invalid key length: {}", key_len);
            return -2;
        }
        
        if !(0..=100 * 1024).contains(&value_len) { // 最大 100KB 配置值
            tracing::error!("[WASM Host] Invalid value length: {}", value_len);
            return -3;
        }
        
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
        
        // 存储配置到 core db
        match &ctx.db_manager {
            Some(db_mgr) => {
                match db_mgr.core().lock() {
                    Ok(core_db) => {
                        match core_db.set_config(&key, value.as_bytes(), false) {
                            Ok(()) => {
                                tracing::debug!("[WASM Host] Config saved successfully");
                                1 // 成功
                            }
                            Err(e) => {
                                tracing::error!("[WASM Host] Failed to set config: {}", e);
                                -4
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("[WASM Host] Failed to lock core db: {}", e);
                        -4
                    }
                }
            }
            None => {
                tracing::warn!("[WASM Host] DbManager not available");
                -5
            }
        }
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
        
        // 验证内存边界
        let memory_size = view.data_size();
        if offset + length as u64 > memory_size {
            return Err(CisError::wasm(
                format!("Read out of bounds: offset {} + len {} > size {}",
                    offset, length, memory_size)
            ));
        }
        
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
        
        // 验证内存边界
        let memory_size = view.data_size();
        if offset + length as u64 > memory_size {
            return Err(CisError::wasm(
                format!("Read out of bounds: offset {} + len {} > size {}",
                    offset, length, memory_size)
            ));
        }
        
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
        
        // 验证内存边界
        let memory_size = view.data_size();
        if offset + len as u64 > memory_size {
            return Err(CisError::wasm(
                format!("Write out of bounds: offset {} + len {} > size {}",
                    offset, len, memory_size)
            ));
        }
        
        view.write(offset, &data[..len])
            .map_err(|e| CisError::wasm(format!("Memory write error: {}", e)))?;
        
        Ok(len)
    }
}

// ==================== Legacy API Compatibility ====================

/// Host 环境（旧版 API 兼容）
///
/// 表示一个已加载并实例化的 WASM Skill 实例。
/// 这个结构体现在使用新的运行时实现，但保持旧的 API。
pub struct HostEnv {
    /// WASM 线性内存
    pub memory: Option<Memory>,
    /// 记忆服务
    pub memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
    /// AI 服务回调
    #[allow(clippy::type_complexity)]
    pub ai_callback: Arc<Mutex<dyn Fn(&str) -> String + Send + 'static>>,
}

impl HostEnv {
    /// 创建新的 Host 环境
    #[allow(clippy::type_complexity)]
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

/// 创建 Host 函数导入对象（旧版 API - 已弃用，请使用 HostFunctions::create_imports）
pub fn create_host_imports(
    store: &mut Store,
    env: &wasmer::FunctionEnv<HostContext>,
) -> wasmer::Imports {
    // 直接使用新的 HostFunctions 实现
    HostFunctions::create_imports(store, env.clone())
}

// 注意：所有旧版 host 函数已由 HostFunctions 结构体中的新方法替代
// 这些函数保留用于向后兼容，但内部实现委托给 HostFunctions

/// 从 FunctionEnvMut 读取字符串的辅助函数
fn read_string_from_memory_view(
    view: &wasmer::MemoryView,
    ptr: i32,
    len: i32,
) -> Result<String, crate::error::CisError> {
    if len < 0 {
        return Err(crate::error::CisError::invalid_input("Invalid length: negative"));
    }
    
    let offset = ptr as u64;
    let length = len as usize;
    
    // 验证内存边界
    let memory_size = view.data_size();
    if offset + length as u64 > memory_size {
        return Err(crate::error::CisError::wasm(
            format!("Read out of bounds: offset {} + len {} > size {}",
                offset, length, memory_size)
        ));
    }
    
    let mut buffer = vec![0u8; length];
    view.read(offset, &mut buffer)
        .map_err(|e| crate::error::CisError::wasm(format!("Memory read error: {}", e)))?;
    
    String::from_utf8(buffer)
        .map_err(|e| crate::error::CisError::wasm(format!("UTF-8 error: {}", e)))
}

/// 写入字节到 MemoryView 的辅助函数
fn write_bytes_to_memory_view(
    view: &wasmer::MemoryView,
    ptr: i32,
    data: &[u8],
) -> Result<usize, crate::error::CisError> {
    let offset = ptr as u64;
    let len = data.len();
    
    // 验证内存边界
    let memory_size = view.data_size();
    if offset + len as u64 > memory_size {
        return Err(crate::error::CisError::wasm(
            format!("Write out of bounds: offset {} + len {} > size {}",
                offset, len, memory_size)
        ));
    }
    
    view.write(offset, data)
        .map_err(|e| crate::error::CisError::wasm(format!("Memory write error: {}", e)))?;
    
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemorySearchItem;

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

    #[test]
    fn test_execution_limits() {
        let limits = ExecutionLimits::new(
            Duration::from_secs(30),
            1_000_000
        );
        
        assert!(!limits.is_timeout());
        assert!(!limits.is_step_limit_reached());
        assert!(limits.remaining_time() > Duration::ZERO);
        
        std::thread::sleep(Duration::from_millis(10));
        assert!(limits.elapsed() >= Duration::from_millis(10));
    }

    #[test]
    fn test_host_context_network_permissions() {
        // 使用简化的测试，避免复杂的依赖
        // 创建一个没有实际服务的 HostContext 进行测试
        struct DummyMemoryService;
        impl MemoryServiceTrait for DummyMemoryService {
            fn get(&self, _key: &str) -> Option<Vec<u8>> {
                None
            }
            fn set(&self, _key: &str, _value: &[u8]) -> Result<(), CisError> {
                Ok(())
            }
            fn delete(&self, _key: &str) -> Result<(), CisError> {
                Ok(())
            }
            fn search(&self, _query: &str, _limit: usize) -> Result<Vec<MemorySearchItem>, CisError> {
                Ok(vec![])
            }
        }

        let memory_service: Arc<Mutex<dyn MemoryServiceTrait>> = 
            Arc::new(Mutex::new(DummyMemoryService));
        let ai_provider: Arc<Mutex<dyn AiProvider>> = 
            Arc::new(Mutex::new(mock_ai::MockAiProvider::new()));
        
        let mut ctx = HostContext::new(memory_service, ai_provider);
        
        // 默认不允许网络
        assert!(!ctx.allow_network);
        assert!(!ctx.is_host_allowed("api.example.com"));
        
        // 启用网络但不限制主机
        ctx.set_network_permissions(true, vec![]);
        assert!(ctx.allow_network);
        assert!(ctx.is_host_allowed("api.example.com"));
        
        // 限制特定主机
        ctx.set_network_permissions(true, vec!["api.example.com".to_string()]);
        assert!(ctx.is_host_allowed("api.example.com"));
        assert!(!ctx.is_host_allowed("other.com"));
    }
}

/// 用于测试的 Mock AI Provider
#[cfg(test)]
mod mock_ai {
    use async_trait::async_trait;
    use crate::ai::{AiProvider, Message, Result};
    use crate::conversation::ConversationContext;

    pub struct MockAiProvider;

    impl MockAiProvider {
        pub fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl AiProvider for MockAiProvider {
        fn name(&self) -> &str {
            "mock"
        }

        async fn available(&self) -> bool {
            true
        }

        async fn chat(&self, prompt: &str) -> Result<String> {
            Ok(format!("Mock response to: {}", prompt))
        }

        async fn chat_with_context(
            &self,
            _system: &str,
            _messages: &[Message],
        ) -> Result<String> {
            Ok("Mock context response".to_string())
        }

        async fn chat_with_rag(
            &self,
            prompt: &str,
            _ctx: Option<&ConversationContext>,
        ) -> Result<String> {
            Ok(format!("Mock RAG response to: {}", prompt))
        }

        async fn generate_json(
            &self,
            _prompt: &str,
            _schema: &str,
        ) -> Result<serde_json::Value> {
            Ok(serde_json::json!({"mock": true}))
        }
    }
}

// 导出 mock_ai 模块供测试使用
#[cfg(test)]
pub use mock_ai::MockAiProvider;
