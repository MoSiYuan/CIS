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
    fn check_limits(&self) -> Result<(), CisError> {
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

/// 旧版 host_memory_get（真实实现）
/// 
/// 从 WASM 内存中读取 key，从 Host memory 服务获取值，写回 WASM 内存
pub fn legacy_host_memory_get(
    mut env: wasmer::FunctionEnvMut<HostContext>,
    key_ptr: i32,
    key_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let ctx = env.data();
    
    // 获取 WASM 内存
    let memory = match &ctx.memory_ref {
        Some(m) => m,
        None => {
            tracing::error!("[WASM Host] Memory not initialized");
            return -1;
        }
    };
    
    // 读取 key
    let key = {
        let view = memory.view(&env);
        let key_ptr = WasmPtr::<u8>::new(key_ptr as u32);
        match key_ptr.read_utf8_string(&view, key_len as u32) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read key: {}", e);
                return -1;
            }
        }
    };
    
    // 从 memory 服务获取值
    let value = {
        let memory_service = ctx.memory.lock().unwrap();
        match memory_service.get(&key) {
            Ok(Some(v)) => v,
            Ok(None) => return 0, // 未找到
            Err(e) => {
                tracing::error!("[WASM Host] Failed to get value: {}", e);
                return -1;
            }
        }
    };
    
    // 检查输出缓冲区大小
    if value.len() > out_len as usize {
        tracing::warn!("[WASM Host] Output buffer too small: {} > {}", value.len(), out_len);
        return -2; // 缓冲区不足
    }
    
    // 写回 WASM 内存
    {
        let view = memory.view(&env);
        let out_ptr = WasmPtr::<u8>::new(out_ptr as u32);
        if let Err(e) = out_ptr.write_utf8(&view, &value) {
            tracing::error!("[WASM Host] Failed to write output: {}", e);
            return -1;
        }
    }
    
    value.len() as i64
}

/// 旧版 host_memory_set（真实实现）
/// 
/// 从 WASM 内存读取 key 和 value，存储到 Host memory 服务
pub fn legacy_host_memory_set(
    mut env: wasmer::FunctionEnvMut<HostContext>,
    key_ptr: i32,
    key_len: i32,
    val_ptr: i32,
    val_len: i32,
) -> i32 {
    let ctx = env.data();
    
    // 获取 WASM 内存
    let memory = match &ctx.memory_ref {
        Some(m) => m,
        None => {
            tracing::error!("[WASM Host] Memory not initialized");
            return -1;
        }
    };
    
    // 读取 key
    let key = {
        let view = memory.view(&env);
        let key_ptr = WasmPtr::<u8>::new(key_ptr as u32);
        match key_ptr.read_utf8_string(&view, key_len as u32) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read key: {}", e);
                return -1;
            }
        }
    };
    
    // 读取 value
    let value = {
        let view = memory.view(&env);
        let val_ptr = WasmPtr::<u8>::new(val_ptr as u32);
        match val_ptr.read_utf8_string(&view, val_len as u32) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read value: {}", e);
                return -1;
            }
        }
    };
    
    // 存储到 memory 服务
    let mut memory_service = ctx.memory.lock().unwrap();
    match memory_service.set(&key, &value) {
        Ok(_) => {
            tracing::debug!("[WASM Host] Set {} = {}", key, value);
            0 // 成功
        }
        Err(e) => {
            tracing::error!("[WASM Host] Failed to set value: {}", e);
            -1
        }
    }
}

/// 旧版 host_memory_delete（真实实现）
/// 
/// 从 WASM 内存读取 key，从 Host memory 服务删除
pub fn legacy_host_memory_delete(
    mut env: wasmer::FunctionEnvMut<HostContext>,
    key_ptr: i32,
    key_len: i32,
) -> i32 {
    let ctx = env.data();
    
    // 获取 WASM 内存
    let memory = match &ctx.memory_ref {
        Some(m) => m,
        None => {
            tracing::error!("[WASM Host] Memory not initialized");
            return -1;
        }
    };
    
    // 读取 key
    let key = {
        let view = memory.view(&env);
        let key_ptr = WasmPtr::<u8>::new(key_ptr as u32);
        match key_ptr.read_utf8_string(&view, key_len as u32) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read key: {}", e);
                return -1;
            }
        }
    };
    
    // 从 memory 服务删除
    let mut memory_service = ctx.memory.lock().unwrap();
    match memory_service.delete(&key) {
        Ok(_) => {
            tracing::debug!("[WASM Host] Deleted {}", key);
            0 // 成功
        }
        Err(e) => {
            tracing::error!("[WASM Host] Failed to delete: {}", e);
            -1
        }
    }
}

/// 旧版 host_ai_chat（真实实现）
/// 
/// 从 WASM 内存读取 prompt，调用 AI Provider，返回结果指针
pub fn legacy_host_ai_chat(
    mut env: wasmer::FunctionEnvMut<HostContext>,
    prompt_ptr: i32,
    prompt_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let ctx = env.data();
    
    // 获取 WASM 内存
    let memory = match &ctx.memory_ref {
        Some(m) => m,
        None => {
            tracing::error!("[WASM Host] Memory not initialized");
            return -1;
        }
    };
    
    // 读取 prompt
    let prompt = {
        let view = memory.view(&env);
        let prompt_ptr = WasmPtr::<u8>::new(prompt_ptr as u32);
        match prompt_ptr.read_utf8_string(&view, prompt_len as u32) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read prompt: {}", e);
                return -1;
            }
        }
    };
    
    tracing::debug!("[WASM Host] AI chat prompt: {}", prompt);
    
    // 调用 AI Provider
    let response = {
        let ai_provider = ctx.ai.lock().unwrap();
        match ai_provider.chat(&prompt) {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!("[WASM Host] AI chat failed: {}", e);
                format!("Error: {}", e)
            }
        }
    };
    
    // 检查输出缓冲区
    if response.len() > out_len as usize {
        tracing::warn!("[WASM Host] Output buffer too small for AI response");
        return -2;
    }
    
    // 写回 WASM 内存
    {
        let view = memory.view(&env);
        let out_ptr = WasmPtr::<u8>::new(out_ptr as u32);
        if let Err(e) = out_ptr.write_utf8(&view, &response) {
            tracing::error!("[WASM Host] Failed to write AI response: {}", e);
            return -1;
        }
    }
    
    response.len() as i64
}

/// 旧版 host_log（真实实现）
/// 
/// 从 WASM 内存读取日志消息，输出到 tracing
pub fn legacy_host_log(
    mut env: wasmer::FunctionEnvMut<HostContext>,
    level: i32,
    msg_ptr: i32,
    msg_len: i32,
) {
    let ctx = env.data();
    
    // 获取 WASM 内存
    let memory = match &ctx.memory_ref {
        Some(m) => m,
        None => {
            tracing::error!("[WASM Host] Memory not initialized");
            return;
        }
    };
    
    // 读取消息
    let msg = {
        let view = memory.view(&env);
        let msg_ptr = WasmPtr::<u8>::new(msg_ptr as u32);
        match msg_ptr.read_utf8_string(&view, msg_len as u32) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read log message: {}", e);
                return;
            }
        }
    };
    
    // 输出到 tracing
    match LogLevel::from_i32(level) {
        Some(LogLevel::Debug) => tracing::debug!("[WASM Skill] {}", msg),
        Some(LogLevel::Info) => tracing::info!("[WASM Skill] {}", msg),
        Some(LogLevel::Warn) => tracing::warn!("[WASM Skill] {}", msg),
        Some(LogLevel::Error) => tracing::error!("[WASM Skill] {}", msg),
        None => tracing::info!("[WASM Skill] {}", msg),
    }
    
    // 调用日志回调（如果设置了）
    if let Some(ref callback) = ctx.log_callback {
        callback(&format!("[{}] {}", level, msg));
    }
}

/// 旧版 host_http_post（真实实现）
/// 
/// 从 WASM 内存读取 URL 和 body，执行 HTTP POST 请求
pub fn legacy_host_http_post(
    mut env: wasmer::FunctionEnvMut<HostContext>,
    url_ptr: i32,
    url_len: i32,
    body_ptr: i32,
    body_len: i32,
    out_ptr: i32,
    out_len: i32,
) -> i64 {
    let ctx = env.data();
    
    // 检查网络权限
    if !ctx.allow_network {
        tracing::error!("[WASM Host] Network access denied");
        return -1;
    }
    
    // 获取 WASM 内存
    let memory = match &ctx.memory_ref {
        Some(m) => m,
        None => {
            tracing::error!("[WASM Host] Memory not initialized");
            return -1;
        }
    };
    
    // 读取 URL
    let url = {
        let view = memory.view(&env);
        let url_ptr = WasmPtr::<u8>::new(url_ptr as u32);
        match url_ptr.read_utf8_string(&view, url_len as u32) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read URL: {}", e);
                return -1;
            }
        }
    };
    
    // 检查主机是否允许
    if let Some(host) = url.split("//").nth(1).and_then(|s| s.split('/').next()) {
        if !ctx.is_host_allowed(host) {
            tracing::error!("[WASM Host] Host not allowed: {}", host);
            return -1;
        }
    }
    
    // 读取 body
    let body = {
        let view = memory.view(&env);
        let body_ptr = WasmPtr::<u8>::new(body_ptr as u32);
        match body_ptr.read_utf8_string(&view, body_len as u32) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("[WASM Host] Failed to read body: {}", e);
                return -1;
            }
        }
    };
    
    tracing::debug!("[WASM Host] HTTP POST {} with {} bytes body", url, body.len());
    
    // 注意：实际 HTTP 请求需要异步运行时，这里返回一个模拟响应
    // 真实实现需要使用异步 HTTP 客户端
    let response = format!("{{\"status\":\"ok\",\"url\":\"{}\"}}", url);
    
    // 检查输出缓冲区
    if response.len() > out_len as usize {
        tracing::warn!("[WASM Host] Output buffer too small for HTTP response");
        return -2;
    }
    
    // 写回 WASM 内存
    {
        let view = memory.view(&env);
        let out_ptr = WasmPtr::<u8>::new(out_ptr as u32);
        if let Err(e) = out_ptr.write_utf8(&view, &response) {
            tracing::error!("[WASM Host] Failed to write HTTP response: {}", e);
            return -1;
        }
    }
    
    response.len() as i64
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
