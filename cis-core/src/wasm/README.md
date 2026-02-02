# WASM Runtime 集成

CIS 项目的 WASM Runtime 集成，使用 wasmer 作为 WASM 引擎。

## 架构

```
┌─────────────────────────────────────────────────────────────┐
│                        CIS Core                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │ SkillManager │  │ WASM Runtime │  │  Native Skill│       │
│  └──────┬───────┘  └──────┬───────┘  └──────────────┘       │
│         │                 │                                  │
│         └─────────┬───────┘                                  │
│                   │                                          │
│         ┌─────────▼───────┐                                  │
│         │   WasmInstance  │                                  │
│         └─────────┬───────┘                                  │
│                   │                                          │
│         ┌─────────▼───────┐                                  │
│         │   WasmSkill     │                                  │
│         │  (implements    │                                  │
│         │    Skill trait) │                                  │
│         └─────────┬───────┘                                  │
│                   │                                          │
│         ┌─────────▼───────┐                                  │
│         │   Host API      │◄───── WASM Skill 调用           │
│         │  (host_memory_* │                                  │
│         │   host_ai_chat  │                                  │
│         │   host_log      │                                  │
│         │   host_http_*)  │                                  │
│         └─────────────────┘                                  │
└─────────────────────────────────────────────────────────────┘
```

## 模块结构

```
wasm/
├── mod.rs      # WASM Runtime 核心
├── host.rs     # Host API 实现
├── skill.rs    # WasmSkill 实现
├── tests.rs    # 测试用例
└── README.md   # 本文档
```

## 使用方法

### 1. 启用 WASM 特性

在 `Cargo.toml` 中启用 `wasm` 特性：

```toml
[dependencies]
cis-core = { path = "../cis-core", features = ["wasm"] }
```

### 2. 加载 WASM Skill

```rust
use cis_core::wasm::{WasmRuntime, WasmSkillBuilder};

// 创建运行时
let mut runtime = WasmRuntime::new()?;

// 从字节码加载 WASM
let wasm_bytes = std::fs::read("path/to/skill.wasm")?;
let instance = runtime.load_skill(&wasm_bytes)?;

// 或者使用 Builder 模式
let skill = WasmSkillBuilder::new()
    .name("my-skill")
    .version("1.0.0")
    .description("My WASM Skill")
    .wasm_bytes(wasm_bytes)
    .memory_service(memory_service)?;
```

### 3. 在 SkillManager 中使用

```rust
use cis_core::skill::{SkillManager, LoadOptions};

let manager = SkillManager::new(db_manager)?;

// 加载 WASM Skill
let wasm_bytes = std::fs::read("path/to/skill.wasm")?;
manager.load_wasm("my-wasm-skill", &wasm_bytes, LoadOptions::default())?;
```

## Host API

WASM Skill 可以通过以下 Host API 与 CIS Core 交互：

### 记忆操作

```c
// 读取记忆
int64_t host_memory_get(int32_t key_ptr, int32_t key_len);

// 写入记忆
int32_t host_memory_set(int32_t key_ptr, int32_t key_len, 
                        int32_t val_ptr, int32_t val_len);

// 删除记忆
int32_t host_memory_delete(int32_t key_ptr, int32_t key_len);
```

### AI 调用

```c
// AI 聊天
int64_t host_ai_chat(int32_t prompt_ptr, int32_t prompt_len);
```

### 日志

```c
// 记录日志（level: 0=Debug, 1=Info, 2=Warn, 3=Error）
void host_log(int32_t level, int32_t msg_ptr, int32_t msg_len);
```

### HTTP 请求

```c
// HTTP POST（需要 http 权限）
int64_t host_http_post(int32_t url_ptr, int32_t url_len,
                       int32_t body_ptr, int32_t body_len);
```

## 内存管理

Host API 使用 WASM 线性内存进行数据交换：

- **指针**: WASM 线性内存中的偏移量（i32）
- **长度**: 数据的字节数（i32）
- **返回值**: i64 编码的指针和长度（高32位为指针，低32位为长度）

### 返回值编码

```rust
fn encode_result(ptr: i32, len: i32) -> i64 {
    ((ptr as i64) << 32) | (len as i64 & 0xFFFFFFFF)
}
```

### C 语言使用示例

```c
#include <stdint.h>

// 声明 Host 函数
extern int64_t host_memory_get(int32_t key_ptr, int32_t key_len);
extern int32_t host_memory_set(int32_t key_ptr, int32_t key_len, 
                               int32_t val_ptr, int32_t val_len);
extern void host_log(int32_t level, int32_t msg_ptr, int32_t msg_len);

// 辅助函数：解码返回值
void decode_result(int64_t result, int32_t* ptr, int32_t* len) {
    *ptr = (int32_t)(result >> 32);
    *len = (int32_t)(result & 0xFFFFFFFF);
}

// 使用示例
void example() {
    // 设置记忆
    const char* key = "my_key";
    const char* value = "my_value";
    host_memory_set(
        (int32_t)key, strlen(key),
        (int32_t)value, strlen(value)
    );
    
    // 获取记忆
    int64_t result = host_memory_get((int32_t)key, strlen(key));
    int32_t ptr, len;
    decode_result(result, &ptr, &len);
    
    // 记录日志
    const char* msg = "Hello from WASM!";
    host_log(1, (int32_t)msg, strlen(msg));  // Info level
}
```

## 依赖

```toml
[dependencies]
wasmer = "4.0"
wasmer-compiler-cranelift = "4.0"
```

## 安全考虑

1. **内存隔离**: 每个 WASM Skill 在自己的线性内存中运行
2. **权限控制**: HTTP 等敏感操作需要显式权限
3. **资源限制**: 可配置内存限制和执行超时
4. **沙箱执行**: WASM 代码无法直接访问 Host 文件系统

## 开发计划

- [x] 基础 WASM Runtime 实现
- [x] Host API 定义
- [x] WasmSkill 实现
- [x] SkillManager 集成
- [ ] WASI 支持
- [ ] 更完善的内存管理
- [ ] 性能优化
- [ ] 更多 Host API（文件系统、网络等）

## 参考资料

- [Wasmer Docs](https://docs.wasmer.io/)
- [WebAssembly Spec](https://webassembly.github.io/spec/)
- [WASI Preview](https://wasi.dev/)
