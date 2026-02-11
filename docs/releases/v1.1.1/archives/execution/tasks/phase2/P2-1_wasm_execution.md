# P2-1: WASM Skill 完整执行

**优先级**: P0 (阻塞)  
**阶段**: Phase 2 - 核心功能完善  
**负责人**: Agent-H (Rust-Core)  
**预估**: 4 天  
**依赖**: P1-1 (内存安全修复)  

---

## 当前状态

```rust
// scheduler/skill_executor.rs
async fn execute_wasm(&self, wasm_bytes: &[u8], inputs: Value) -> Result<Value> {
    todo!("WASM execution not yet implemented")
}
```

---

## 原子任务

### [ ] P2-1.1: 选择 WASM 运行时

**方案对比**:
- **wasmtime** (推荐): Cranelift 编译器，性能优秀，生态活跃
- **wasmer**: 已部分集成，但性能略逊

**决策**: 使用 `wasmtime`

**实现**:
```toml
# Cargo.toml
[dependencies]
wasmtime = "15.0"
wasmtime-wasi = "15.0"
```

---

### [ ] P2-1.2: 实现 WASM 模块加载

**文件**: `cis-core/src/wasm/loader.rs` (新建)

**功能**:
1. 验证 WASM 模块 (magic number, version)
2. 编译为机器码 (缓存结果)
3. 模块实例化管理

**代码框架**:
```rust
use wasmtime::{Engine, Module, Store, Instance};

pub struct WasmLoader {
    engine: Engine,
    cache: HashMap<String, Module>,
}

impl WasmLoader {
    pub fn new() -> Self {
        let engine = Engine::default();
        Self { engine, cache: HashMap::new() }
    }
    
    pub fn load(&mut self, name: &str, bytes: &[u8]) -> Result<&Module> {
        if !self.cache.contains_key(name) {
            let module = Module::new(&self.engine, bytes)?;
            self.cache.insert(name.to_string(), module);
        }
        Ok(self.cache.get(name).unwrap())
    }
}
```

---

### [ ] P2-1.3: 实现 Host API

**文件**: `cis-core/src/wasm/host.rs` (新建)

**API 列表**:

| API | 功能 | 签名 |
|-----|------|------|
| `memory_get` | 读取存储 | `fn(key: &str) -> Option<String>` |
| `memory_set` | 写入存储 | `fn(key: &str, value: &str)` |
| `ai_chat` | AI 对话 | `fn(prompt: &str) -> String` |
| `log` | 日志输出 | `fn(level: &str, message: &str)` |
| `http_post` | HTTP 请求 | `fn(url: &str, body: &str) -> Result<String>` |

**实现示例**:
```rust
use wasmtime::{Func, FuncType, ValType};

pub fn create_host_functions(store: &mut Store<HostState>) -> Vec<Func> {
    let memory_get = Func::wrap(store, |mut caller: Caller<'_, HostState>, key_ptr: i32, key_len: i32, out_ptr: i32| -> i32 {
        // 从 WASM 内存读取 key
        // 查询存储
        // 写入 WASM 内存
        // 返回状态码
    });
    vec![memory_get, /* ... */]
}
```

---

### [ ] P2-1.4: 实现输入输出序列化

**格式**: JSON

**处理边界**:
- 大输入 (> 1MB): 分块传输
- 复杂嵌套结构: 递归序列化

---

### [ ] P2-1.5: 实现资源限制

| 资源 | 限制 | 实现方式 |
|------|------|----------|
| CPU | < 30s | `tokio::time::timeout` |
| 内存 | < 128MB | `wasmtime::Config::max_memory_size` |
| 网络 | 可选禁用 | Host API 层控制 |

---

### [ ] P2-1.6: 添加 WASM 测试套件

**测试用例**:
1. 简单计算 (1+1=2)
2. 内存操作 (get/set)
3. AI 调用 (模拟)
4. 超时测试 (无限循环)

**验收标准**:
```bash
cargo test -p cis-core --lib wasm
# 100% 测试通过
```

---

## 集成与验收

### 替换 execute_wasm

```rust
// scheduler/skill_executor.rs
use crate::wasm::{WasmLoader, HostState};

async fn execute_wasm(&self, wasm_bytes: &[u8], inputs: Value) -> Result<Value> {
    let mut loader = WasmLoader::new();
    let module = loader.load("skill", wasm_bytes)?;
    
    let mut store = Store::new(&loader.engine, HostState::new(inputs));
    let instance = Instance::new(&mut store, module, &[])?;
    
    // 调用入口函数
    let main = instance.get_typed_func::<(), (i32,)>(&mut store, "main")?;
    let (result_ptr,) = main.call(&mut store, ())?;
    
    // 读取结果
    let result = read_result_from_memory(&store, result_ptr)?;
    Ok(result)
}
```

### 最终验收
```bash
# 1. 测试 Skill 编译为 WASM
cd skills/init-wizard
cargo build --target wasm32-unknown-unknown --release

# 2. 运行 WASM Skill
cis skill run --wasm target/wasm32-unknown-unknown/release/init-wizard.wasm

# 3. 验证 Host API 可用
# - 日志输出正常
# - 存储读写正常
```

---

## 输出物清单

- [ ] `feat/wasm/loader.rs` - 模块加载器
- [ ] `feat/wasm/host.rs` - Host API 实现
- [ ] `feat/wasm/mod.rs` - 模块导出
- [ ] `scheduler/skill_executor.rs` 更新
- [ ] WASM 测试套件
- [ ] `reports/P2-1-completion.md`

---

## 并行提示

**依赖上游**: P1-1 (内存安全修复)
**可并行**: P2-2 (四级决策)、P2-4 (P2P组网)
**依赖下游**: P2-9 (Skill生态)、P3-1 (内存优化)
