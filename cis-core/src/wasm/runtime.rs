//! # WASM Runtime
//!
//! 管理 WASM Skill 的加载和执行，包含资源限制和安全控制。

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wasmer::{Engine, Module, Store, Instance, Memory, MemoryType};
use wasmparser::{Validator, WasmFeatures};

use crate::wasm::host::{HostContext, HostFunctions};
use crate::wasm::WasmSkillConfig;
use crate::wasm::sandbox::WasiSandbox;
use crate::memory::MemoryServiceTrait;
use crate::ai::AiProvider;
use crate::error::{CisError, Result};
use crate::storage::DbManager;

/// WASM 内存页大小（64KB）
const WASM_PAGE_SIZE: usize = 64 * 1024;

/// 默认最大内存（512MB）
const DEFAULT_MAX_MEMORY_MB: usize = 512;

/// 默认执行超时（30秒，以毫秒为单位）
const DEFAULT_EXECUTION_TIMEOUT_MS: u64 = 30000;

/// 默认最大执行步数
const DEFAULT_MAX_EXECUTION_STEPS: u64 = 1_000_000;

/// WASM Skill Runtime
/// 
/// 管理 WASM 模块的加载、实例化和执行环境，包含资源限制和安全控制。
pub struct WasmRuntime {
    engine: Engine,
    store: Arc<Mutex<Store>>,
    config: WasmSkillConfig,
    /// WASI 沙箱配置
    sandbox: Option<WasiSandbox>,
}

impl WasmRuntime {
    /// 创建新的 Runtime
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::WasmRuntime;
    ///
    /// let runtime = WasmRuntime::new().expect("Failed to create runtime");
    /// ```
    pub fn new() -> Result<Self> {
        Self::with_config(WasmSkillConfig::default())
    }
    
    /// 使用指定配置创建 Runtime
    ///
    /// # 参数
    ///
    /// - `config`: WASM Skill 配置
    pub fn with_config(config: WasmSkillConfig) -> Result<Self> {
        // 验证配置
        config.validate()?;
        
        let engine = Engine::default();
        let store = Store::new(engine.clone());
        
        Ok(Self {
            engine,
            store: Arc::new(Mutex::new(store)),
            config,
            sandbox: None,
        })
    }

    /// 添加沙箱配置到运行时
    ///
    /// # 参数
    ///
    /// - `sandbox`: WASI 沙箱配置
    ///
    /// # 返回
    ///
    /// 返回自身以支持链式调用
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::{WasmRuntime, WasiSandbox};
    ///
    /// let runtime = WasmRuntime::new()
    ///     .expect("Failed to create runtime")
    ///     .with_sandbox(
    ///         WasiSandbox::new()
    ///             .with_readonly_path("/data")
    ///             .with_writable_path("/tmp")
    ///     );
    /// ```
    pub fn with_sandbox(mut self, sandbox: WasiSandbox) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// 获取沙箱配置的引用
    pub fn sandbox(&self) -> Option<&WasiSandbox> {
        self.sandbox.as_ref()
    }

    /// 检查是否配置了沙箱
    pub fn has_sandbox(&self) -> bool {
        self.sandbox.is_some()
    }

    /// 计算内存页数限制
    fn get_max_memory_pages(&self) -> u32 {
        let max_memory_mb = self.config.memory_limit
            .map(|bytes| bytes / (1024 * 1024))
            .unwrap_or(DEFAULT_MAX_MEMORY_MB);
        
        // 512MB = 8192 页（每页 64KB）
        let max_pages = (max_memory_mb * 1024 * 1024) / WASM_PAGE_SIZE;
        max_pages.min(65536) as u32 // WebAssembly 最大支持 65536 页（4GB）
    }

    /// 获取执行超时
    fn get_execution_timeout(&self) -> Duration {
        self.config.execution_timeout
            .map(Duration::from_millis)
            .unwrap_or_else(|| Duration::from_millis(DEFAULT_EXECUTION_TIMEOUT_MS))
    }
    
    /// 验证 WASM 模块
    ///
    /// 使用 wasmparser 进行深度验证，检查：
    /// - 魔数和版本
    /// - 模块结构完整性
    /// - 不安全的特性（如异常、内存64等）
    fn validate_wasm(&self, wasm_bytes: &[u8]) -> Result<()> {
        // 1. 基本检查：魔数和版本
        if wasm_bytes.len() < 8 {
            return Err(CisError::wasm("WASM module too small"));
        }
        if wasm_bytes[0..4] != [0x00, 0x61, 0x73, 0x6d] {
            return Err(CisError::wasm("Invalid WASM magic number"));
        }
        if wasm_bytes[4..8] != [0x01, 0x00, 0x00, 0x00] {
            return Err(CisError::wasm("Unsupported WASM version"));
        }

        // 2. 使用 wasmparser 进行深度验证
        let mut validator = Validator::new_with_features(WasmFeatures {
            memory64: false,           // 禁用 64 位内存
            exceptions: false,         // 禁用异常
            threads: false,            // 禁用线程
            multi_memory: false,       // 禁用多内存
            simd: true,                // 允许 SIMD（常用）
            bulk_memory: true,         // 允许批量内存操作
            reference_types: true,     // 允许引用类型
            // 其他特性使用默认值
            ..WasmFeatures::default()
        });

        validator.validate_all(wasm_bytes)
            .map_err(|e| CisError::wasm(format!("WASM validation failed: {}", e)))?;

        // 3. 检查模块大小限制（最大 100MB）
        const MAX_MODULE_SIZE: usize = 100 * 1024 * 1024;
        if wasm_bytes.len() > MAX_MODULE_SIZE {
            return Err(CisError::wasm(
                format!("WASM module size {} exceeds maximum {}", 
                    wasm_bytes.len(), MAX_MODULE_SIZE)
            ));
        }

        Ok(())
    }

    /// 加载 WASM 模块
    ///
    /// # 参数
    ///
    /// - `wasm_bytes`: WASM 模块的字节码
    ///
    /// # 返回
    ///
    /// 返回加载的 WASM 实例
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::wasm::WasmRuntime;
    ///
    /// let runtime = WasmRuntime::new().unwrap();
    /// // let instance = runtime.load_module(&wasm_bytes).unwrap();
    /// ```
    pub fn load_module(&self, wasm_bytes: &[u8]) -> Result<WasmModule> {
        // 完整验证 WASM
        self.validate_wasm(wasm_bytes)?;

        let module = Module::from_binary(&self.engine, wasm_bytes)
            .map_err(|e| CisError::wasm(format!("Failed to load module: {}", e)))?;
        
        Ok(WasmModule {
            module,
            store: Arc::clone(&self.store),
            max_memory_pages: self.get_max_memory_pages(),
            execution_timeout: self.get_execution_timeout(),
            sandbox: self.sandbox.clone(),
        })
    }
    
    /// 加载并实例化 WASM Skill
    ///
    /// # 参数
    ///
    /// - `wasm_bytes`: WASM 模块的字节码
    /// - `memory_service`: 记忆服务
    /// - `ai_provider`: AI Provider
    ///
    /// # 返回
    ///
    /// 返回已实例化的 WASM Skill
    pub fn load_skill(
        &self,
        wasm_bytes: &[u8],
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
        ai_provider: Arc<Mutex<dyn AiProvider>>,
    ) -> Result<WasmSkillInstance> {
        let module = self.load_module(wasm_bytes)?;
        module.instantiate(memory_service, ai_provider)
    }
    
    /// 执行 WASM Skill
    ///
    /// 加载、实例化并执行 WASM Skill，返回执行结果。
    ///
    /// # 参数
    ///
    /// - `skill_name`: Skill 名称（用于日志和错误信息）
    /// - `wasm_bytes`: WASM 字节码
    /// - `event_data`: 事件数据（JSON）
    /// - `memory_service`: 记忆服务
    /// - `ai_provider`: AI Provider (Arc<Mutex<dyn AiProvider>>)
    ///
    /// # 返回
    ///
    /// - `Ok(serde_json::Value)`: 执行结果
    /// - `Err(CisError)`: 执行失败
    pub async fn execute_skill(
        &self,
        skill_name: &str,
        wasm_bytes: &[u8],
        event_data: serde_json::Value,
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
        ai_provider: Arc<Mutex<dyn AiProvider>>,
    ) -> Result<serde_json::Value> {
        tracing::info!("Executing WASM skill '{}' ({} bytes)", skill_name, wasm_bytes.len());
        
        // 1. 加载并实例化 WASM 模块
        let instance = self.load_skill(wasm_bytes, memory_service, ai_provider)
            .map_err(|e| CisError::wasm(format!("Failed to load WASM skill '{}': {}", skill_name, e)))?;
        
        // 2. 初始化 Skill
        instance.init()
            .map_err(|e| CisError::wasm(format!("Failed to initialize WASM skill '{}': {}", skill_name, e)))?;
        
        // 3. 序列化事件数据
        let event_json = serde_json::to_string(&event_data)
            .map_err(|e| CisError::wasm(format!("Failed to serialize event data: {}", e)))?;
        
        // 4. 调用事件处理函数
        let result_code = instance.on_event("execute", event_json.as_bytes())
            .map_err(|e| CisError::wasm(format!("WASM skill '{}' execution failed: {}", skill_name, e)))?;
        
        // 5. 构造执行结果
        let result = serde_json::json!({
            "success": result_code >= 0,
            "code": result_code,
            "skill": skill_name,
        });
        
        // 6. 关闭 Skill
        if let Err(e) = instance.shutdown() {
            tracing::warn!("Failed to shutdown WASM skill '{}': {}", skill_name, e);
        }
        
        tracing::info!("WASM skill '{}' executed successfully (code: {})", skill_name, result_code);
        
        Ok(result)
    }

    /// 获取引擎的引用
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
    
    /// 获取存储的引用
    pub fn store(&self) -> Arc<Mutex<Store>> {
        Arc::clone(&self.store)
    }
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create default WASM runtime")
    }
}

/// 已加载的 WASM 模块（未实例化）
#[derive(Debug)]
pub struct WasmModule {
    module: Module,
    store: Arc<Mutex<Store>>,
    pub(crate) max_memory_pages: u32,
    pub(crate) execution_timeout: Duration,
    /// WASI 沙箱配置
    pub(crate) sandbox: Option<WasiSandbox>,
}

impl WasmModule {
    /// 获取模块的引用
    pub fn module(&self) -> &Module {
        &self.module
    }
    
    /// 获取最大内存页数
    pub fn max_memory_pages(&self) -> u32 {
        self.max_memory_pages
    }
    
    /// 获取执行超时
    pub fn execution_timeout(&self) -> Duration {
        self.execution_timeout
    }
    
    /// 实例化模块
    ///
    /// # 参数
    ///
    /// - `memory_service`: 记忆服务
    /// - `ai_provider`: AI Provider
    ///
    /// # 返回
    ///
    /// 返回已实例化的 WASM Skill
    pub fn instantiate(
        &self,
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
        ai_provider: Arc<Mutex<dyn AiProvider>>,
    ) -> Result<WasmSkillInstance> {
        self.instantiate_with_db(memory_service, ai_provider, None)
    }

    /// 实例化模块（带数据库管理器）
    ///
    /// # 参数
    ///
    /// - `memory_service`: 记忆服务
    /// - `ai_provider`: AI Provider
    /// - `db_manager`: 可选的数据库管理器
    ///
    /// # 返回
    ///
    /// 返回已实例化的 WASM Skill
    pub fn instantiate_with_db(
        &self,
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
        ai_provider: Arc<Mutex<dyn AiProvider>>,
        db_manager: Option<Arc<DbManager>>,
    ) -> Result<WasmSkillInstance> {
        let mut store = self.store.lock()
            .map_err(|e| CisError::wasm(format!("Store lock failed: {}", e)))?;
        
        // 创建 Host 上下文
        let mut host_ctx = match db_manager {
            Some(db) => HostContext::with_db_manager(memory_service, ai_provider, db),
            None => HostContext::new(memory_service, ai_provider),
        };
        
        // 创建线性内存，应用内存限制
        // 最小 1 页（64KB），最大限制为配置的页数
        let memory_type = MemoryType::new(1, Some(self.max_memory_pages), false);
        let memory = Memory::new(&mut *store, memory_type)
            .map_err(|e| CisError::wasm(format!("Failed to create memory: {}", e)))?;
        
        // 设置内存引用
        host_ctx.set_memory(memory.clone());
        
        // 设置执行限制
        host_ctx.set_execution_limits(
            self.execution_timeout,
            DEFAULT_MAX_EXECUTION_STEPS,
        );
        
        // 创建 FunctionEnv
        let function_env = wasmer::FunctionEnv::new(&mut *store, host_ctx);
        
        // 创建 Host 函数导入
        let imports = HostFunctions::create_imports(&mut store, function_env);
        
        // 实例化模块
        let instance = Instance::new(&mut *store, &self.module, &imports)
            .map_err(|e| CisError::wasm(format!("Failed to instantiate: {}", e)))?;
        
        // 如果模块导出了内存，验证其限制
        let instance_memory = if let Ok(mem) = instance.exports.get_memory("memory") {
            // 检查模块内存是否超过限制
            let mem_type = mem.ty(&*store);
            if let Some(max) = mem_type.maximum {
                // Pages 类型可以转换为 usize 然后转换为 u32
                let max_pages = max.0;
                if max_pages > self.max_memory_pages {
                    return Err(CisError::wasm(
                        format!("WASM module requests {} pages, but limit is {} pages",
                            max_pages, self.max_memory_pages)
                    ));
                }
            }
            mem.clone()
        } else {
            // 否则将我们创建的内存导出到实例
            memory
        };
        
        tracing::info!(
            "WASM module instantiated successfully (max_memory: {} pages, timeout: {:?})",
            self.max_memory_pages, self.execution_timeout
        );
        
        Ok(WasmSkillInstance {
            instance,
            store: Arc::clone(&self.store),
            memory: instance_memory,
            execution_timeout: self.execution_timeout,
            created_at: Instant::now(),
        })
    }
}

/// 已实例化的 WASM Skill
///
/// 表示一个已加载并实例化的 WASM Skill，可以调用其导出函数。
pub struct WasmSkillInstance {
    instance: Instance,
    store: Arc<Mutex<Store>>,
    memory: Memory,
    execution_timeout: Duration,
    created_at: Instant,
}

impl WasmSkillInstance {
    /// 获取实例的引用
    pub fn instance(&self) -> &Instance {
        &self.instance
    }
    
    /// 获取内存的引用
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// 检查执行时间是否超时
    fn check_timeout(&self) -> Result<()> {
        let elapsed = self.created_at.elapsed();
        if elapsed > self.execution_timeout {
            return Err(CisError::wasm(
                format!("Execution timeout: {:?} exceeded limit of {:?}", 
                    elapsed, self.execution_timeout)
            ));
        }
        Ok(())
    }
    
    /// 调用 Skill 初始化函数
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(CisError)`: 初始化失败
    pub fn init(&self) -> Result<()> {
        self.check_timeout()?;
        
        let mut store = self.store.lock()
            .map_err(|e| CisError::wasm(format!("Store lock failed: {}", e)))?;
        
        // 尝试调用 skill_init 函数（如果存在）
        match self.instance.exports.get_function("skill_init") {
            Ok(func) => {
                // 设置执行超时检查
                let start = Instant::now();
                
                func.call(&mut *store, &[])
                    .map_err(|e| {
                        if start.elapsed() > self.execution_timeout {
                            CisError::wasm(format!("Init function timed out after {:?}", 
                                self.execution_timeout))
                        } else {
                            CisError::wasm(format!("Init failed: {}", e))
                        }
                    })?;
                
                tracing::info!("WASM Skill initialized (took {:?})", start.elapsed());
            }
            Err(_) => {
                tracing::debug!("No skill_init function found, skipping");
            }
        }
        
        Ok(())
    }
    
    /// 调用事件处理函数
    ///
    /// # 参数
    ///
    /// - `event_type`: 事件类型
    /// - `data`: 事件数据
    ///
    /// # 返回
    ///
    /// - `Ok(i32)`: 处理结果
    /// - `Err(CisError)`: 调用失败
    pub fn on_event(&self, event_type: &str, data: &[u8]) -> Result<i32> {
        self.check_timeout()?;
        
        let mut store = self.store.lock()
            .map_err(|e| CisError::wasm(format!("Store lock failed: {}", e)))?;
        
        // 分配 WASM 内存
        let event_ptr = self.alloc(&mut store, event_type.len())?;
        let data_ptr = self.alloc(&mut store, data.len())?;
        
        // 写入数据
        self.write_memory(&*store, event_ptr, event_type.as_bytes())?;
        self.write_memory(&*store, data_ptr, data)?;
        
        // 调用函数
        let start = Instant::now();
        let result = match self.instance.exports.get_function("skill_on_event") {
            Ok(func) => {
                let args = &[
                    wasmer::Value::I32(event_ptr.offset() as i32),
                    wasmer::Value::I32(event_type.len() as i32),
                    wasmer::Value::I32(data_ptr.offset() as i32),
                    wasmer::Value::I32(data.len() as i32),
                ];
                
                let res = func.call(&mut *store, args)
                    .map_err(|e| {
                        if start.elapsed() > self.execution_timeout {
                            CisError::wasm(format!("Event handler timed out after {:?}", 
                                self.execution_timeout))
                        } else {
                            CisError::wasm(format!("Event handling failed: {}", e))
                        }
                    })?;
                
                // 提取返回值
                res.first()
                    .and_then(|v| v.i32())
                    .unwrap_or(0)
            }
            Err(_) => {
                tracing::warn!("No skill_on_event function found");
                0
            }
        };
        
        tracing::debug!("Event processing took {:?}", start.elapsed());
        
        // 释放内存
        let _ = self.free(&mut store, event_ptr);
        let _ = self.free(&mut store, data_ptr);
        
        Ok(result)
    }
    
    /// 调用 Skill 关闭函数
    pub fn shutdown(&self) -> Result<()> {
        self.check_timeout()?;
        
        let mut store = self.store.lock()
            .map_err(|e| CisError::wasm(format!("Store lock failed: {}", e)))?;
        
        // 尝试调用 skill_shutdown 函数（如果存在）
        match self.instance.exports.get_function("skill_shutdown") {
            Ok(func) => {
                let start = Instant::now();
                
                func.call(&mut *store, &[])
                    .map_err(|e| {
                        if start.elapsed() > self.execution_timeout {
                            CisError::wasm(format!("Shutdown timed out after {:?}", 
                                self.execution_timeout))
                        } else {
                            CisError::wasm(format!("Shutdown failed: {}", e))
                        }
                    })?;
                
                tracing::info!("WASM Skill shutdown (took {:?})", start.elapsed());
            }
            Err(_) => {
                tracing::debug!("No skill_shutdown function found, skipping");
            }
        }
        
        Ok(())
    }
    
    /// 在 WASM 中分配内存
    ///
    /// # 参数
    ///
    /// - `store`: WASM Store
    /// - `size`: 分配的大小（字节）
    ///
    /// # 返回
    ///
    /// - `Ok(WasmPtr<u8>)`: 分配的内存指针
    /// - `Err(CisError)`: 分配失败
    fn alloc(&self, store: &mut Store, size: usize) -> Result<wasmer::WasmPtr<u8>> {
        // 检查内存限制
        let memory_size = self.memory.view(store).data_size() as usize;
        if size > memory_size {
            return Err(CisError::wasm(
                format!("Allocation request {} bytes exceeds memory size {} bytes",
                    size, memory_size)
            ));
        }
        
        // 尝试使用模块的 malloc 函数
        match self.instance.exports.get_function("malloc") {
            Ok(func) => {
                let result = func.call(store, &[wasmer::Value::I32(size as i32)])
                    .map_err(|e| CisError::wasm(format!("Allocation failed: {}", e)))?;
                
                let ptr = result.first()
                    .and_then(|v| v.i32())
                    .ok_or_else(|| CisError::wasm("malloc returned invalid value"))?;
                
                if ptr < 0 {
                    return Err(CisError::wasm(
                        format!("malloc failed: returned invalid pointer {}", ptr)
                    ));
                }
                
                Ok(wasmer::WasmPtr::new(ptr as u32))
            }
            Err(_) => {
                // 如果没有 malloc，使用静态内存布局
                // 简化实现：直接返回一个固定偏移量
                // 实际生产代码应该实现内存管理
                tracing::warn!("No malloc function found, using static allocation");
                Ok(wasmer::WasmPtr::new(1024))
            }
        }
    }
    
    /// 释放 WASM 内存
    ///
    /// # 参数
    ///
    /// - `store`: WASM Store
    /// - `ptr`: 要释放的内存指针
    fn free(&self, store: &mut Store, ptr: wasmer::WasmPtr<u8>) -> Result<()> {
        // 尝试使用模块的 free 函数
        match self.instance.exports.get_function("free") {
            Ok(func) => {
                func.call(store, &[wasmer::Value::I32(ptr.offset() as i32)])
                    .map_err(|e| CisError::wasm(format!("Free failed: {}", e)))?;
            }
            Err(_) => {
                tracing::debug!("No free function found, memory may leak");
            }
        }
        
        Ok(())
    }
    
    /// 写入 WASM 内存
    ///
    /// # 参数
    ///
    /// - `store`: Store 引用
    /// - `ptr`: 目标指针
    /// - `data`: 要写入的数据
    fn write_memory<S: wasmer::AsStoreRef>(&self, store: &S, ptr: wasmer::WasmPtr<u8>, data: &[u8]) -> Result<()> {
        let view = self.memory.view(store);
        let offset = ptr.offset() as u64;
        
        // 验证写入范围
        let memory_size = view.data_size();
        if offset + data.len() as u64 > memory_size {
            return Err(CisError::wasm(
                format!("Memory write out of bounds: offset {} + len {} > size {}",
                    offset, data.len(), memory_size)
            ));
        }
        
        view.write(offset, data)
            .map_err(|e| CisError::wasm(format!("Memory write error: {}", e)))?;
        Ok(())
    }
    
    /// 从 WASM 内存读取数据
    ///
    /// # 参数
    ///
    /// - `store`: Store 引用
    /// - `ptr`: 源指针
    /// - `len`: 要读取的长度
    ///
    /// # 返回
    ///
    /// - `Ok(Vec<u8>)`: 读取的数据
    /// - `Err(CisError)`: 读取失败
    pub fn read_memory<S: wasmer::AsStoreRef>(&self, store: &S, ptr: wasmer::WasmPtr<u8>, len: u32) -> Result<Vec<u8>> {
        let view = self.memory.view(store);
        let offset = ptr.offset() as u64;
        
        // 验证读取范围
        let memory_size = view.data_size();
        if offset + len as u64 > memory_size {
            return Err(CisError::wasm(
                format!("Memory read out of bounds: offset {} + len {} > size {}",
                    offset, len, memory_size)
            ));
        }
        
        let length = len as usize;
        let mut buffer = vec![0u8; length];
        view.read(offset, &mut buffer)
            .map_err(|e| CisError::wasm(format!("Memory read error: {}", e)))?;
        Ok(buffer)
    }
    
    /// 获取导出函数
    ///
    /// # 参数
    ///
    /// - `name`: 函数名
    ///
    /// # 返回
    ///
    /// - `Ok(&Function)`: 导出函数
    /// - `Err(CisError)`: 函数不存在
    pub fn get_export(&self, name: &str) -> Result<&wasmer::Function> {
        self.instance.exports.get_function(name)
            .map_err(|e| CisError::wasm(format!("Export '{}' not found: {}", name, e)))
    }

    /// 获取当前内存使用量（字节）
    pub fn memory_usage<S: wasmer::AsStoreRef>(&self, store: &S) -> usize {
        self.memory.view(store).data_size() as usize
    }

    /// 获取实例创建时间
    pub fn created_at(&self) -> Instant {
        self.created_at
    }

    /// 获取已运行时间
    pub fn elapsed(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }
}

impl Drop for WasmSkillInstance {
    fn drop(&mut self) {
        // 确保资源被正确释放
        tracing::debug!("WASM Skill instance being dropped (lifetime: {:?})", 
            self.created_at.elapsed());
        
        // 注意：实际的 Store 和 Instance 清理由 Rust 的所有权系统自动处理
        // 这里可以添加额外的清理逻辑（如统计信息、日志等）
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 简单的 WASM 模块：空模块
    const SIMPLE_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, // WASM magic
        0x01, 0x00, 0x00, 0x00, // version 1
    ];

    #[test]
    fn test_wasm_runtime_new() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok(), "Failed to create WASM runtime");
    }

    #[test]
    fn test_wasm_runtime_default() {
        let _runtime = WasmRuntime::default();
        // 验证运行时创建成功
    }

    #[test]
    fn test_load_module() {
        let runtime = WasmRuntime::new().unwrap();
        let module = runtime.load_module(SIMPLE_WASM);
        assert!(module.is_ok(), "Failed to load WASM module");
    }

    #[test]
    fn test_invalid_wasm_magic() {
        let runtime = WasmRuntime::new().unwrap();
        let invalid_wasm = &[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let result = runtime.load_module(invalid_wasm);
        assert!(result.is_err(), "Should reject invalid WASM magic");
    }

    #[test]
    fn test_wasm_config() {
        let config = WasmSkillConfig {
            memory_limit: Some(512 * 1024 * 1024), // 512MB
            execution_timeout: Some(30000),        // 30 seconds
            allowed_syscalls: vec![],
        };
        
        let runtime = WasmRuntime::with_config(config).unwrap();
        let module = runtime.load_module(SIMPLE_WASM).unwrap();
        assert_eq!(module.max_memory_pages(), 8192); // 512MB / 64KB = 8192 pages
    }
}
