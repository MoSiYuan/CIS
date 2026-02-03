# CIS Skill 框架设计（优化版）

## 现状分析

### 当前设计问题

| 问题 | 影响 | 解决方案 |
|------|------|----------|
| 重新编译主节点 | 新增 Skill 需重编 cis-node | 动态加载 |
| 版本耦合 | Skill 与 Core 版本绑定 | 稳定 ABI |
| 依赖冲突 | Skill A 依赖 tokio 1.0, B 依赖 2.0 | WASM 沙箱 |
| 安全风险 | Skill 崩溃导致节点崩溃 | 进程隔离 |

## 优化方案：混合加载模式

### 三种 Skill 类型

#### Type 1: Native Skill（内置）
适用场景：官方 Skill、最高性能需求

```rust
pub trait NativeSkill: Send + Sync {
    fn name(&self) -> &str;
    async fn init(&mut self, ctx: SkillContext) -> Result<()>;
    async fn handle_event(&self, event: Event) -> Result<()>;
}
```

#### Type 2: WASM Skill（推荐）
适用场景：第三方、安全隔离、跨平台

```rust
// WASI 接口
#[no_mangle]
pub extern "C" fn skill_init(config: *const u8, len: usize) -> i32 {
    // 同步初始化，无 async
}

#[no_mangle]
pub extern "C" fn skill_handle(event: *const u8, len: usize) -> i32 {
    // 事件处理
}
```

SDK 设计：
```rust
// cis-skill-sdk - 最小化依赖
pub fn memory_get(key: &str) -> Option<Vec<u8>>;
pub fn memory_set(key: &str, value: &[u8]) -> Result<()>;
pub fn log(level: LogLevel, msg: &str);
pub fn emit_event(event: &[u8]);
```

#### Type 3: External Process（隔离）
适用场景：非 Rust、实验性、特殊权限

通过 gRPC 通信，独立进程运行

## 挂载模式

### 1. 自动扫描
```
~/.cis/skills/
├── cpu-monitor/
│   ├── skill.wasm
│   └── skill.toml
└── memory-organizer/
    └── ...
```

### 2. 配置挂载
```toml
[[skills]]
name = "cpu-monitor"
path = "/opt/cis/skills/cpu-monitor.wasm"
permissions = ["memory:read", "network:local"]
```

### 3. 运行时 CLI
```bash
cis skill mount ./my-skill.wasm
cis skill unmount cpu-monitor
cis skill list
```

## 依赖管理策略

### 禁止依赖（避免环境地狱）
- tokio/async-trait（WASM 不支持）
- 系统级 crates（nix, libc）
- 网络客户端（reqwest, hyper）

### 允许的依赖
- serde（序列化）
- regex（文本处理）
- chrono（时间处理）
- 纯算法 crates

### SDK 提供的能力
```rust
pub mod sdk {
    // 存储
    pub fn memory_get(key: &str) -> Option<Vec<u8>>;
    pub fn memory_set(key: &str, value: &[u8]);
    
    // 日志
    pub fn log_info(msg: &str);
    pub fn log_error(msg: &str);
    
    // HTTP（通过 host 代理）
    pub fn http_request(req: HttpRequest) -> HttpResponse;
    
    // 时间
    pub fn now_timestamp() -> u64;
    pub fn sleep_ms(ms: u64);
}
```

## 实施计划

### Phase 1: WASM Runtime
- 集成 wasmer 或 wasmtime
- 实现 WASI 子集
- 定义 Host API

### Phase 2: SDK
- 发布 cis-skill-sdk crate
- 提供 WASM 构建模板
- 示例 Skill

### Phase 3: Registry
- 签名验证
- 权限系统
- 热加载

## 对比

| 模式 | 性能 | 安全 | 分发 | 适用 |
|------|------|------|------|------|
| Native | 100% | 低 | 难 | 官方 |
| WASM | 90% | 高 | 易 | 第三方 |
| External | 70% | 中 | 中 | 特殊 |
