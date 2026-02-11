# D04: WASM 基础执行设计

> 任务: P0-3 WASM 基础执行  
> 负责人: 开发 A  
> 工期: Week 5-6 (10天)  
> 状态: 设计中  
> 依赖: D01-D03 架构重构

---

## 目标

实现 WASM Skill 的基础执行能力：加载 → 执行 → 清理。

---

## 当前问题

```rust
// ❌ 未实现 - matrix/bridge.rs:688-693
return Err(CisError::skill(
    "WASM skill execution not fully implemented".to_string()
))

// ❌ 简化 AI 回调 - wasm/skill.rs:67-72
let ai_callback = Arc::new(Mutex::new(|prompt: &str| {
    format!("AI response to: {}", prompt)  // 假数据!
}));
```

---

## 设计方案

### 架构图

```
┌─────────────────────────────────────────────────────────────┐
│                    WASM Skill 执行流程                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. 加载阶段                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │   Skill 注册  │───▶│  加载 WASM   │───▶│  验证模块    │  │
│  │   (manifest) │    │  (wasmer)    │    │  (checksum)  │  │
│  └──────────────┘    └──────────────┘    └──────┬───────┘  │
│                                                 │          │
│  2. 实例化阶段                                                 │
│                                                 ▼          │
│                                        ┌──────────────┐   │
│                                        │  创建 Store  │   │
│                                        │  + Memory    │   │
│                                        └──────┬───────┘   │
│                                               │           │
│  3. Host 函数注入                                              │
│                                               ▼           │
│                                        ┌──────────────┐   │
│                                        │  注入 Host   │   │
│                                        │  - cis_ai    │   │
│                                        │  - cis_memory│   │
│                                        │  - cis_http  │   │
│                                        └──────┬───────┘   │
│                                               │           │
│  4. 执行阶段                                                   │
│                                               ▼           │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │  调用 export │───▶│  执行 WASM   │───▶│  获取结果    │  │
│  │  函数        │    │  (带超时)    │    │              │  │
│  └──────────────┘    └──────────────┘    └──────┬───────┘  │
│                                                 │          │
│  5. 清理阶段                                                   │
│                                                 ▼          │
│                                        ┌──────────────┐   │
│                                        │  释放 Store  │   │
│                                        │  清理 Memory │   │
│                                        └──────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

### WASM 运行时封装

```rust
// wasm/runtime.rs

use wasmer::{Engine, Module, Store, Instance, Memory, MemoryType};
use wasmer_wasi::WasiEnv;

/// WASM 运行时配置
pub struct WasmRuntimeConfig {
    /// 最大内存限制 (默认 512MB)
    pub max_memory: usize,
    /// 最大执行时间 (毫秒，默认 30s)
    pub max_execution_time: u64,
    /// 是否启用 WASI
    pub enable_wasi: bool,
}

impl Default for WasmRuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory: 512 * 1024 * 1024,
            max_execution_time: 30000,
            enable_wasi: true,
        }
    }
}

/// WASM 运行时
pub struct WasmRuntime {
    engine: Engine,
    config: WasmRuntimeConfig,
    host_functions: HostFunctionRegistry,
}

impl WasmRuntime {
    pub fn new(config: WasmRuntimeConfig) -> Result<Self> {
        let engine = Engine::default();
        
        Ok(Self {
            engine,
            config,
            host_functions: HostFunctionRegistry::new(),
        })
    }
    
    /// 加载 WASM 模块
    pub fn load_module(&self, wasm_bytes: &[u8]) -> Result<WasmModule> {
        // 验证 WASM
        self.validate_wasm(wasm_bytes)?;
        
        // 编译模块
        let module = Module::new(&self.engine, wasm_bytes)?;
        
        Ok(WasmModule {
            module,
            engine: self.engine.clone(),
        })
    }
    
    /// 验证 WASM
    fn validate_wasm(&self, wasm_bytes: &[u8]) -> Result<()> {
        // 1. 检查 magic number
        if &wasm_bytes[0..4] != b"\0asm" {
            return Err(Error::invalid_wasm("invalid magic number"));
        }
        
        // 2. 检查版本
        let version = u32::from_le_bytes([wasm_bytes[4], wasm_bytes[5], wasm_bytes[6], wasm_bytes[7]]);
        if version != 1 {
            return Err(Error::invalid_wasm(format!("unsupported version: {}", version)));
        }
        
        // 3. 解析并检查限制
        let config = wasmparser::Validator::new_with_features(
            wasmparser::WasmFeatures {
                memory64: false,
                exceptions: false,
                ..Default::default()
            }
        );
        config.validate_all(wasm_bytes)
            .map_err(|e| Error::invalid_wasm(e.to_string()))?;
        
        Ok(())
    }
}

/// 已加载的 WASM 模块
pub struct WasmModule {
    module: Module,
    engine: Engine,
}

impl WasmModule {
    /// 实例化模块
    pub fn instantiate(&self, host: &HostEnv) -> Result<WasmInstance> {
        let mut store = Store::new(self.engine.clone());
        
        // 创建内存
        let memory_type = MemoryType::new(1, Some(8192), false);  // 64KB - 512MB
        let memory = Memory::new(&mut store, memory_type)?;
        
        // 创建导入对象
        let mut imports = wasmer::Imports::new();
        imports.define("env", "memory", memory.clone());
        
        // 注入 Host 函数
        host.register_functions(&mut store, &mut imports)?;
        
        // 实例化
        let instance = Instance::new(&mut store, &self.module, &imports)?;
        
        Ok(WasmInstance {
            instance,
            store,
            memory,
        })
    }
}

/// WASM 实例
pub struct WasmInstance {
    instance: Instance,
    store: Store,
    memory: Memory,
}

impl WasmInstance {
    /// 调用导出函数
    pub fn call(&mut self, name: &str, args: &[Value]) -> Result<Box<[Value]>> {
        let func = self.instance
            .exports
            .get_function(name)
            .map_err(|_| Error::function_not_found(name))?;
        
        // 带超时执行
        let result = tokio::time::timeout(
            Duration::from_millis(30000),
            async { func.call(&mut self.store, args) }
        ).await
            .map_err(|_| Error::execution_timeout())?;
        
        result.map_err(|e| Error::wasm_execution(e.to_string()))
    }
    
    /// 读取内存
    pub fn read_memory(&self, offset: usize, len: usize) -> Result<Vec<u8>> {
        let memory_view = self.memory.view(&self.store);
        let mut buffer = vec![0u8; len];
        memory_view.read(offset as u64, &mut buffer)
            .map_err(|e| Error::memory_access(e.to_string()))?;
        Ok(buffer)
    }
    
    /// 写入内存
    pub fn write_memory(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        let memory_view = self.memory.view(&self.store);
        memory_view.write(offset as u64, data)
            .map_err(|e| Error::memory_access(e.to_string()))?;
        Ok(())
    }
}
```

---

### Host 函数实现

```rust
// wasm/host.rs

/// Host 环境 - 提供给 WASM 的宿主功能
pub struct HostEnv {
    ai_provider: Arc<dyn AiProvider>,
    memory_service: Arc<dyn MemoryService>,
    http_client: Arc<dyn HttpClient>,
}

impl HostEnv {
    pub fn register_functions(&self, store: &mut Store, imports: &mut Imports) -> Result<()> {
        // cis_ai_prompt(prompt_ptr, prompt_len, result_ptr, result_cap) -> result_len
        imports.define(
            "cis",
            "ai_prompt",
            Function::new_typed_with_env(
                store,
                self,
                |env: &HostEnv, prompt_ptr: i32, prompt_len: i32, result_ptr: i32, result_cap: i32| -> i32 {
                    env.ai_prompt(prompt_ptr, prompt_len, result_ptr, result_cap)
                }
            )
        );
        
        // cis_memory_get(key_ptr, key_len, value_ptr, value_cap) -> value_len
        imports.define(
            "cis",
            "memory_get",
            Function::new_typed_with_env(
                store,
                self,
                |env: &HostEnv, key_ptr: i32, key_len: i32, value_ptr: i32, value_cap: i32| -> i32 {
                    env.memory_get(key_ptr, key_len, value_ptr, value_cap)
                }
            )
        );
        
        // cis_memory_set(key_ptr, key_len, value_ptr, value_len)
        imports.define(
            "cis",
            "memory_set",
            Function::new_typed_with_env(
                store,
                self,
                |env: &HostEnv, key_ptr: i32, key_len: i32, value_ptr: i32, value_len: i32| {
                    env.memory_set(key_ptr, key_len, value_ptr, value_len)
                }
            )
        );
        
        // cis_http_request(url_ptr, url_len, body_ptr, body_len, result_ptr, result_cap) -> result_len
        imports.define(
            "cis",
            "http_request",
            Function::new_typed_with_env(
                store,
                self,
                |env: &HostEnv, url_ptr: i32, url_len: i32, body_ptr: i32, body_len: i32, result_ptr: i32, result_cap: i32| -> i32 {
                    env.http_request(url_ptr, url_len, body_ptr, body_len, result_ptr, result_cap)
                }
            )
        );
        
        Ok(())
    }
    
    /// AI 调用
    fn ai_prompt(&self, prompt_ptr: i32, prompt_len: i32, result_ptr: i32, result_cap: i32) -> i32 {
        // 从 WASM 内存读取 prompt
        let prompt = self.read_string(prompt_ptr, prompt_len);
        
        // 调用 AI Provider (真实实现!)
        let response = self.ai_provider.complete(&prompt);
        
        // 写入 WASM 内存
        self.write_string(result_ptr, result_cap, &response)
    }
    
    /// 记忆读取
    fn memory_get(&self, key_ptr: i32, key_len: i32, value_ptr: i32, value_cap: i32) -> i32 {
        let key = self.read_string(key_ptr, key_len);
        
        // 调用 Memory Service
        match self.memory_service.get(&key) {
            Ok(Some(value)) => self.write_bytes(value_ptr, value_cap, &value),
            _ => -1,  // 未找到
        }
    }
}
```

---

### Skill 集成

```rust
// skill/wasm_skill.rs

pub struct WasmSkillExecutor {
    runtime: Arc<WasmRuntime>,
    ai_provider: Arc<dyn AiProvider>,
    memory_service: Arc<dyn MemoryService>,
    module_cache: Arc<RwLock<HashMap<String, WasmModule>>>,
}

impl WasmSkillExecutor {
    pub async fn execute(&self, skill: &SkillManifest, input: SkillInput) -> Result<SkillOutput> {
        // 1. 获取或加载模块
        let module = self.get_or_load_module(skill).await?;
        
        // 2. 创建 Host 环境
        let host_env = HostEnv::new(
            self.ai_provider.clone(),
            self.memory_service.clone(),
        );
        
        // 3. 实例化
        let mut instance = module.instantiate(&host_env)?;
        
        // 4. 准备输入
        let input_json = serde_json::to_string(&input)?;
        let input_ptr = self.alloc_string(&mut instance, &input_json)?;
        
        // 5. 调用执行函数
        let result = instance.call("execute", &[Value::I32(input_ptr as i32)])?;
        let output_ptr = result[0].i32().ok_or(Error::invalid_return())?;
        
        // 6. 读取输出
        let output_json = self.read_string(&instance, output_ptr as usize)?;
        let output: SkillOutput = serde_json::from_str(&output_json)?;
        
        // 7. 清理
        instance.call("dealloc", &[Value::I32(input_ptr), Value::I32(input_json.len() as i32)])?;
        
        Ok(output)
    }
}
```

---

## 任务清单

- [ ] 创建 `wasm/runtime.rs`
- [ ] 实现 WASM 验证
- [ ] 实现模块加载
- [ ] 实现实例化
- [ ] 实现 Host 函数注册
- [ ] 实现 AI Provider 接入
- [ ] 实现 Memory Service 接入
- [ ] 实现 HTTP Client 接入
- [ ] 实现 Skill 集成
- [ ] 实现超时控制
- [ ] 实现内存限制
- [ ] 实现错误处理
- [ ] 编写测试

---

## 验收标准

```bash
# 测试 1: WASM 加载和执行
cis skill run test-wasm-skill
# 预期: 成功执行并返回结果

# 测试 2: AI 调用
# WASM 内调用 cis_ai_prompt
# 预期: 返回真实 AI 响应

# 测试 3: 超时控制
# 执行死循环 WASM
# 预期: 30秒后超时退出

# 测试 4: 内存限制
# 申请超过 512MB 内存
# 预期: 内存分配失败
```

---

## 依赖

- D01 配置抽象
- D02 全局状态消除
- D03 事件总线

---

*设计创建日期: 2026-02-10*
