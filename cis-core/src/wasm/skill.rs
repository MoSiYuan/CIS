//! # WASM Skill 实现
//!
//! 将 WASM 模块包装为 Skill 接口的实现。

use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use wasmer::{Instance, Memory, MemoryType};

use crate::error::{CisError, Result};
use crate::memory::MemoryServiceTrait;
use crate::skill::{Event, Skill, SkillConfig, SkillContext};

use super::host::{HostEnv, create_host_imports};
use super::WasmInstance;

/// WASM Skill 实现
pub struct WasmSkill {
    /// Skill 名称
    name: String,
    /// Skill 版本
    version: String,
    /// Skill 描述
    description: String,
    /// WASM 实例
    instance: Arc<Mutex<WasmInstance>>,
    /// Host 环境
    host_env: HostEnv,
}

impl WasmSkill {
    /// 创建新的 WASM Skill
    ///
    /// # 参数
    ///
    /// - `name`: Skill 名称
    /// - `version`: Skill 版本
    /// - `description`: Skill 描述
    /// - `wasm_instance`: WASM 实例
    /// - `memory_service`: 记忆服务
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
        wasm_instance: WasmInstance,
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
    ) -> Result<Self> {
        let name = name.into();
        let version = version.into();
        let description = description.into();

        // 创建 AI 回调（简化实现）
        let ai_callback: Arc<Mutex<dyn Fn(&str) -> String + Send + 'static>> = 
            Arc::new(Mutex::new(|prompt: &str| {
                format!("AI response to: {}", prompt)
            }));

        let host_env = HostEnv::new(memory_service, ai_callback);

        Ok(Self {
            name,
            version,
            description,
            instance: Arc::new(Mutex::new(wasm_instance)),
            host_env,
        })
    }

    /// 实例化 WASM 模块
    ///
    /// 创建 WASM 实例并链接 Host 函数。
    pub fn instantiate(&mut self) -> Result<()> {
        let mut instance_guard = self.instance.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        
        // 获取 store Arc，保留它以避免临时值被释放
        let binding = instance_guard.store();
        let store_arc = binding.clone();
        
        // 获取 store 的锁
        let mut store = store_arc.lock()
            .map_err(|e| CisError::skill(format!("Store lock failed: {}", e)))?;

        // 创建线性内存
        let memory_type = MemoryType::new(1, Some(10), false);
        let memory = Memory::new(&mut *store, memory_type)
            .map_err(|e| CisError::skill(format!("Failed to create memory: {}", e)))?;

        // 设置 Host 环境内存
        self.host_env.set_memory(memory.clone());

        // 创建 FunctionEnv
        let function_env = wasmer::FunctionEnv::new(&mut *store, self.host_env.clone());

        // 创建 Host 函数导入
        let imports = create_host_imports(&mut *store, &function_env);

        // 实例化模块
        let instance = Instance::new(&mut *store, &instance_guard.module(), &imports)
            .map_err(|e| CisError::skill(format!("Failed to instantiate WASM module: {}", e)))?;

        // 设置实例
        instance_guard.set_instance(instance);

        tracing::info!("WASM Skill '{}' instantiated successfully", self.name);
        Ok(())
    }

    /// 调用 WASM 导出函数
    pub fn call_export(&self, func_name: &str, args: &[wasmer::Value]) -> Result<Vec<wasmer::Value>> {
        let instance_guard = self.instance.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        
        let instance = instance_guard.instance()
            .ok_or_else(|| CisError::skill("WASM instance not instantiated"))?;

        // 获取 store Arc，保留它以避免临时值被释放
        let binding = instance_guard.store();
        let store_arc = binding.clone();
        
        let mut store = store_arc.lock()
            .map_err(|e| CisError::skill(format!("Store lock failed: {}", e)))?;

        let func = instance.exports.get_function(func_name)
            .map_err(|e| CisError::skill(format!("Function '{}' not found: {}", func_name, e)))?;

        let result = func.call(&mut *store, args)
            .map_err(|e| CisError::skill(format!("Failed to call function '{}': {}", func_name, e)))?;

        Ok(result.to_vec())
    }

    /// 调用 Skill 初始化函数
    pub fn call_init(&self, config: &SkillConfig) -> Result<()> {
        // 将配置序列化为 JSON
        let config_json = serde_json::to_string(&config.values)
            .map_err(|e| CisError::skill(format!("Failed to serialize config: {}", e)))?;

        // 注意：实际实现需要将 config_json 写入 WASM 内存
        // 这里简化处理
        tracing::info!("Calling init with config: {}", config_json);

        // 尝试调用 init 函数（如果存在）
        match self.call_export("skill_init", &[]) {
            Ok(_) => Ok(()),
            Err(e) => {
                // 如果函数不存在，记录警告但继续
                tracing::warn!("Skill init function not found or failed: {}", e);
                Ok(())
            }
        }
    }

    /// 调用 Skill 事件处理函数
    pub fn call_handle_event(&self, event: &Event) -> Result<()> {
        // 将事件序列化为 JSON
        let event_json = serde_json::to_string(event)
            .map_err(|e| CisError::skill(format!("Failed to serialize event: {}", e)))?;

        // 注意：实际实现需要将 event_json 写入 WASM 内存
        tracing::info!("Calling handle_event with: {}", event_json);

        // 尝试调用 handle_event 函数（如果存在）
        match self.call_export("skill_handle_event", &[]) {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::warn!("Skill handle_event function not found or failed: {}", e);
                Ok(())
            }
        }
    }
}

#[async_trait]
impl Skill for WasmSkill {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn init(&mut self, config: SkillConfig) -> Result<()> {
        // 实例化 WASM 模块
        self.instantiate()?;
        
        // 调用 WASM init 函数
        self.call_init(&config)?;
        
        tracing::info!("WASM Skill '{}' initialized", self.name);
        Ok(())
    }

    async fn handle_event(&self, _ctx: &dyn SkillContext, event: Event) -> Result<()> {
        self.call_handle_event(&event)?;
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        // 尝试调用 shutdown 函数（如果存在）
        match self.call_export("skill_shutdown", &[]) {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("Skill shutdown function not found or failed: {}", e);
            }
        }
        
        tracing::info!("WASM Skill '{}' shutdown", self.name);
        Ok(())
    }
}

/// WASM Skill 构建器
pub struct WasmSkillBuilder {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    wasm_bytes: Option<Vec<u8>>,
    memory_service: Option<Arc<Mutex<dyn MemoryServiceTrait>>>,
}

impl WasmSkillBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            name: None,
            version: None,
            description: None,
            wasm_bytes: None,
            memory_service: None,
        }
    }

    /// 设置名称
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 设置版本
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// 设置描述
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置 WASM 字节码
    pub fn wasm_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.wasm_bytes = Some(bytes);
        self
    }

    /// 设置记忆服务
    pub fn memory_service(mut self, service: Arc<Mutex<dyn MemoryServiceTrait>>) -> Self {
        self.memory_service = Some(service);
        self
    }

    /// 构建 WASM Skill
    pub fn build(self) -> Result<WasmSkill> {
        let name = self.name.ok_or_else(|| CisError::skill("Name is required"))?;
        let version = self.version.unwrap_or_else(|| "0.1.0".to_string());
        let description = self.description.unwrap_or_else(|| "WASM Skill".to_string());
        let wasm_bytes = self.wasm_bytes.ok_or_else(|| CisError::skill("WASM bytes are required"))?;
        let memory_service = self.memory_service.ok_or_else(|| CisError::skill("Memory service is required"))?;

        // 创建运行时和实例
        let mut runtime = super::WasmRuntime::new()?;
        let instance = runtime.load_skill(&wasm_bytes)?;

        WasmSkill::new(name, version, description, instance, memory_service)
    }
}

impl Default for WasmSkillBuilder {
    fn default() -> Self {
        Self::new()
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
    fn test_wasm_skill_builder() {
        // 这个测试需要一个有效的 WASM 文件和 MemoryService
        // 实际测试需要在有完整环境的情况下运行
        let builder = WasmSkillBuilder::new()
            .name("test-skill")
            .version("1.0.0")
            .description("Test WASM Skill")
            .wasm_bytes(SIMPLE_WASM.to_vec());

        assert_eq!(builder.name, Some("test-skill".to_string()));
        assert_eq!(builder.version, Some("1.0.0".to_string()));
    }
}
