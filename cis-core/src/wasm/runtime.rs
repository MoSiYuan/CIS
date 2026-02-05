//! # WASM Runtime
//!
//! 管理 WASM Skill 的加载和执行。

use std::sync::{Arc, Mutex};
use wasmer::{Engine, Module, Store, Instance, TypedFunction, Memory, MemoryType};

use crate::wasm::host::{HostContext, HostFunctions};
use crate::memory::MemoryServiceTrait;
use crate::ai::AiProvider;
use crate::error::{CisError, Result};
use crate::storage::DbManager;

/// WASM Skill Runtime
/// 
/// 管理 WASM 模块的加载、实例化和执行环境。
pub struct WasmRuntime {
    engine: Engine,
    store: Arc<Mutex<Store>>,
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
        let engine = Engine::default();
        let store = Store::new(engine.clone());
        
        Ok(Self {
            engine,
            store: Arc::new(Mutex::new(store)),
        })
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
        let module = Module::from_binary(&self.engine, wasm_bytes)
            .map_err(|e| CisError::wasm(format!("Failed to load module: {}", e)))?;
        
        Ok(WasmModule {
            module,
            store: Arc::clone(&self.store),
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
pub struct WasmModule {
    module: Module,
    store: Arc<Mutex<Store>>,
}

impl WasmModule {
    /// 获取模块的引用
    pub fn module(&self) -> &Module {
        &self.module
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
        
        // 创建线性内存
        let memory_type = MemoryType::new(1, Some(100), false);
        let memory = Memory::new(&mut *store, memory_type)
            .map_err(|e| CisError::wasm(format!("Failed to create memory: {}", e)))?;
        
        // 设置内存引用
        host_ctx.set_memory(memory.clone());
        
        // 创建 FunctionEnv
        let function_env = wasmer::FunctionEnv::new(&mut *store, host_ctx);
        
        // 创建 Host 函数导入
        let imports = HostFunctions::create_imports(&mut *store, function_env);
        
        // 实例化模块
        let instance = Instance::new(&mut *store, &self.module, &imports)
            .map_err(|e| CisError::wasm(format!("Failed to instantiate: {}", e)))?;
        
        // 如果模块导出了内存，使用模块的内存
        let instance_memory = if let Ok(mem) = instance.exports.get_memory("memory") {
            mem.clone()
        } else {
            // 否则将我们创建的内存导出到实例
            memory
        };
        
        tracing::info!("WASM module instantiated successfully");
        
        Ok(WasmSkillInstance {
            instance,
            store: Arc::clone(&self.store),
            memory: instance_memory,
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
    
    /// 调用 Skill 初始化函数
    ///
    /// # 返回
    ///
    /// - `Ok(())`: 初始化成功
    /// - `Err(CisError)`: 初始化失败
    pub fn init(&self) -> Result<()> {
        let mut store = self.store.lock()
            .map_err(|e| CisError::wasm(format!("Store lock failed: {}", e)))?;
        
        // 尝试调用 skill_init 函数（如果存在）
        match self.instance.exports.get_function("skill_init") {
            Ok(func) => {
                func.call(&mut *store, &[])
                    .map_err(|e| CisError::wasm(format!("Init failed: {}", e)))?;
                tracing::info!("WASM Skill initialized");
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
        let mut store = self.store.lock()
            .map_err(|e| CisError::wasm(format!("Store lock failed: {}", e)))?;
        
        // 分配 WASM 内存
        let event_ptr = self.alloc(&mut *store, event_type.len())?;
        let data_ptr = self.alloc(&mut *store, data.len())?;
        
        // 写入数据
        self.write_memory(&*store, event_ptr, event_type.as_bytes())?;
        self.write_memory(&*store, data_ptr, data)?;
        
        // 调用函数
        let result = match self.instance.exports.get_function("skill_on_event") {
            Ok(func) => {
                let args = &[
                    wasmer::Value::I32(event_ptr.offset() as i32),
                    wasmer::Value::I32(event_type.len() as i32),
                    wasmer::Value::I32(data_ptr.offset() as i32),
                    wasmer::Value::I32(data.len() as i32),
                ];
                
                let res = func.call(&mut *store, args)
                    .map_err(|e| CisError::wasm(format!("Event handling failed: {}", e)))?;
                
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
        
        // 释放内存
        let _ = self.free(&mut *store, event_ptr);
        let _ = self.free(&mut *store, data_ptr);
        
        Ok(result)
    }
    
    /// 调用 Skill 关闭函数
    pub fn shutdown(&self) -> Result<()> {
        let mut store = self.store.lock()
            .map_err(|e| CisError::wasm(format!("Store lock failed: {}", e)))?;
        
        // 尝试调用 skill_shutdown 函数（如果存在）
        match self.instance.exports.get_function("skill_shutdown") {
            Ok(func) => {
                func.call(&mut *store, &[])
                    .map_err(|e| CisError::wasm(format!("Shutdown failed: {}", e)))?;
                tracing::info!("WASM Skill shutdown");
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
        // 尝试使用模块的 malloc 函数
        match self.instance.exports.get_function("malloc") {
            Ok(func) => {
                let result = func.call(store, &[wasmer::Value::I32(size as i32)])
                    .map_err(|e| CisError::wasm(format!("Allocation failed: {}", e)))?;
                
                let ptr = result.first()
                    .and_then(|v| v.i32())
                    .ok_or_else(|| CisError::wasm("malloc returned invalid value"))?;
                
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
}
