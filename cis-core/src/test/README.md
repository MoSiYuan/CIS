# CIS Test Framework

CIS v1.1.4 测试框架 - 完整的 Mock 实现、CI/CD 集成和覆盖率工具支持。

## 目录结构

```
cis-core/src/test/
├── mod.rs              # 测试模块入口和工具函数
├── README.md           # 本文档
├── examples.rs         # Mock 使用示例（5个示例测试）
├── SHAME_TAGS.md       # 耻辱标签文件
└── mocks/
    ├── mod.rs          # Mock 基础组件（CallTracker, ArgMatcher）
    ├── network_service.rs  # MockNetworkService
    ├── storage_service.rs  # MockStorageService
    ├── event_bus.rs        # MockEventBus
    └── ai_provider.rs      # MockAiProvider
```

## Mock 实现

所有 Mock 都实现了：
- ✅ 调用追踪（自动记录所有方法调用）
- ✅ 参数验证（验证调用时的参数）
- ✅ 行为配置（预设返回值或错误）
- ✅ 断言方法（验证调用次数、参数等）
- ✅ 并发安全（使用 tokio::sync::RwLock）

### MockNetworkService

```rust
use cis_core::test::mocks::MockNetworkService;

let mock = MockNetworkService::new();

// 配置行为
mock.preset_connect("ws://localhost:8080", Ok(())).await;

// 执行操作
mock.connect("ws://localhost:8080").await.unwrap();

// 验证
mock.assert_connected("ws://localhost:8080").await;
mock.assert_call_count("connect", 1);
```

### MockStorageService

```rust
use cis_core::test::mocks::MockStorageService;

let mock = MockStorageService::new();

// 预填充数据
mock.seed("key", "value").await;

// 配置行为
mock.preset_get("key", Some("value".to_string())).await;

// 验证
mock.assert_key_accessed("key");
```

### MockEventBus

```rust
use cis_core::test::mocks::MockEventBus;
use serde_json::json;

let mock = MockEventBus::new();

// 订阅
mock.subscribe("event.name", |event| {
    println!("Received: {:?}", event);
}).await;

// 发布
mock.publish("event.name", json!({"data": "value"})).await.unwrap();

// 验证
mock.assert_event_published("event.name").await;
```

### MockAiProvider

```rust
use cis_core::ai::AiProvider;
use cis_core::test::mocks::MockAiProvider;

let mock = MockAiProvider::new();

// 配置响应
mock.preset_chat("Hello", "Hi!").await;

// 使用
let response = mock.chat("Hello").await.unwrap();

// 验证
mock.assert_prompt_received("Hello").await;
```

## 覆盖率工具

### cargo-tarpaulin

```bash
# 安装
cargo install cargo-tarpaulin

# 运行测试并生成覆盖率报告
cargo tarpaulin

# 生成 HTML 报告
cargo tarpaulin --out Html --output-dir target/coverage
```

配置: `tarpaulin.toml`

### cargo-llvm-cov

```bash
# 安装
cargo install cargo-llvm-cov

# 生成 LCOV 报告
cargo llvm-cov --lcov --output-path target/lcov.info

# 生成 HTML 报告
cargo llvm-cov --html
```

## CI/CD 集成

GitHub Actions 配置 (`.github/workflows/ci.yml`)：

1. **check**: 格式化检查、Clippy、单元测试
2. **mock-tests**: 专门验证 Mock 实现
3. **coverage**: cargo-tarpaulin 覆盖率报告
4. **coverage-llvm**: cargo-llvm-cov 备用方案
5. **build**: 跨平台构建测试
6. **security**: cargo audit 安全审计
7. **doc-tests**: 文档示例测试

## 运行测试

```bash
# Mock 单元测试 (22个测试)
cargo test --package cis-core --lib test::mocks

# 示例测试 (5个测试)
cargo test --package cis-core --lib test::examples

# 所有测试框架测试 (27个测试)
cargo test --package cis-core --lib "test::"
```

## 禁止事项（耻辱标签规则）

- ❌ 禁止使用简化/mock 实现
- ❌ 禁止使用未测试的代码路径
- ✅ 每个 Mock 必须可验证行为
- ✅ 每个测试必须有断言

## 验收标准

| 标准 | 状态 |
|------|------|
| Mock 可以验证调用次数和参数 | ✅ |
| CI 自动运行测试并生成覆盖率报告 | ✅ |
| 覆盖率工具可用 | ✅ |
| 所有 Mock 都有文档 | ✅ |

## 测试统计

- Mock 单元测试: 22个
- 示例测试: 5个
- 总测试数: 27个
- 通过率: 100%

## 版本信息

- 框架版本: v1.1.4
- 任务: P0-6 测试框架搭建
- 完成时间: 2026-02-10
