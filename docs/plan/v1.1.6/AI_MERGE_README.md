# AIMerge 冲突解决策略实现

## 快速开始

AIMerge 允许使用 AI 智能合并冲突的记忆值。

```rust
use cis_core::memory::guard::ai_merge::{AIMerger, AIMergeStrategy};
use cis_core::memory::guard::conflict_resolution::apply_resolution_strategy_async;

// 创建合并器
let merger = AIMerger::default();
merger.set_ai_provider(Box::new(ClaudeCliProvider::default())).await;

// 执行合并
let merged = apply_resolution_strategy_async(
    &ConflictResolutionChoice::AIMerge,
    &local_version,
    &remote_versions,
    "config/database",
    Some(&merger),
).await?;
```

## 核心特性

### 1. 三种合并策略

- **SmartMerge**: 智能合并，保留双方有效信息
- **ContentBased**: 基于内容质量合并
- **TimeBased**: 基于时间戳优先合并

### 2. 错误回退

- AI Provider 不可用 → KeepLocal
- AI 调用超时 → 重试或 KeepLocal
- 所有重试失败 → KeepLocal

### 3. 可配置性

```rust
let config = AIMergeConfig {
    strategy: AIMergeStrategy::SmartMerge,
    max_retries: 3,
    timeout_secs: 60,
};
let merger = AIMerger::new(config);
```

## 文件结构

```
cis-core/src/memory/guard/
├── ai_merge.rs                    # AIMerge 核心实现
├── conflict_resolution.rs         # 冲突解决集成
├── ai_merge_integration_test.rs   # 集成测试
└── mod.rs                         # 模块导出

examples/
└── ai_merge_example.rs            # 使用示例

docs/plan/v1.1.6/
├── AI_MERGE_IMPLEMENTATION.md     # 详细实现文档
└── AI_MERGE_README.md             # 本文件
```

## 测试

运行测试：

```bash
# 单元测试
cargo test -p cis-core ai_merge

# 集成测试
cargo test -p cis-core ai_merge_integration

# 所有测试
cargo test -p cis-core memory::guard
```

## 示例

运行示例：

```bash
cargo run --example ai_merge_example
```

## 设计决策

### 为什么使用异步？

AI 调用是耗时操作，使用异步可以：
- 不阻塞其他操作
- 支持超时控制
- 提高并发性能

### 为什么提供同步版本？

同步版本的 `apply_resolution_strategy` 在 AIMerge 时回退到 KeepLocal，保证：
- API 兼容性
- 不强制异步
- 明确的行为

### 为什么有多种策略？

不同场景需要不同的合并逻辑：
- 配置文件 → SmartMerge
- 文档内容 → ContentBased
- 日志数据 → TimeBased

## 最佳实践

1. **总是设置 AI Provider**
   ```rust
   merger.set_ai_provider(...).await;
   ```

2. **选择合适的策略**
   ```rust
   // 配置 → SmartMerge
   // 内容 → ContentBased
   // 日志 → TimeBased
   ```

3. **处理错误**
   ```rust
   let result = apply_resolution_strategy_async(...)
       .await
       .unwrap_or_else(|_| local.value.clone());
   ```

4. **监控日志**
   ```rust
   tracing::info!("Merge completed");
   ```

## 相关文档

- [AI_MERGE_IMPLEMENTATION.md](./AI_MERGE_IMPLEMENTATION.md) - 详细实现文档
- [冲突检测与解决](../CONFLICT_DETECTION.md) - 冲突检测机制
- [Vector Clock](../VECTOR_CLOCK.md) - 版本控制

## 贡献

欢迎贡献！请确保：
- 添加测试
- 更新文档
- 遵循代码规范

## 许可

遵循 CIS 项目许可证。
