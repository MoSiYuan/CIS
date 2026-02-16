# AIMerge 冲突解决策略实现报告

## 概述

本文档记录了 CIS 记忆系统中 AIMerge (AI 驱动的合并) 冲突解决策略的实现细节。

## 实现位置

- **主要实现**: `cis-core/src/memory/guard/ai_merge.rs`
- **集成代码**: `cis-core/src/memory/guard/conflict_resolution.rs`
- **测试文件**:
  - `cis-core/src/memory/guard/ai_merge.rs` (单元测试)
  - `cis-core/src/memory/guard/ai_merge_integration_test.rs` (集成测试)

## 核心组件

### 1. AIMerger

AI 合并器是核心组件，负责调用 AI 服务并处理合并逻辑。

```rust
pub struct AIMerger {
    ai_provider: Arc<RwLock<Option<Box<dyn AiProvider>>>>,
    config: AIMergeConfig,
}
```

**主要功能**:
- 检查 AI Provider 可用性
- 构建合并 prompt
- 调用 AI 服务（带重试机制）
- 解析 AI 响应
- 失败时自动回退到 KeepLocal

### 2. AIMergeConfig

配置 AI 合并行为：

```rust
pub struct AIMergeConfig {
    pub strategy: AIMergeStrategy,
    pub max_retries: usize,
    pub timeout_secs: u64,
}
```

**默认配置**:
- Strategy: `SmartMerge`
- Max retries: 2
- Timeout: 30 秒

### 3. AIMergeStrategy

定义三种合并策略：

#### SmartMerge (智能合并)
- 保留双方有效信息
- 智能解决冲突
- 维护数据一致性

**适用场景**: 需要保留完整信息的场景（如配置文件、文档）

#### ContentBased (基于内容的合并)
- 基于内容质量合并
- 优先保留更完整的信息

**适用场景**: 内容更新场景（如文章、笔记）

#### TimeBased (基于时间的合并)
- 优先保留较新的修改
- 保留不冲突的旧修改

**适用场景**: 时间敏感的数据（如日志、时间线）

## 使用示例

### 基本用法

```rust
use cis_core::memory::guard::ai_merge::{AIMerger, AIMergeConfig, AIMergeStrategy};
use cis_core::ai::ClaudeCliProvider;

// 1. 创建 AIMerger
let config = AIMergeConfig {
    strategy: AIMergeStrategy::SmartMerge,
    max_retries: 3,
    timeout_secs: 60,
};
let merger = AIMerger::new(config);

// 2. 设置 AI Provider
merger.set_ai_provider(Box::new(ClaudeCliProvider::default())).await;

// 3. 使用异步版本应用冲突解决
let resolved = apply_resolution_strategy_async(
    &ConflictResolutionChoice::AIMerge,
    &local_version,
    &remote_versions,
    "config/database",
    Some(&merger),
).await?;
```

### 同步版本（回退到 KeepLocal）

```rust
// 同步版本不支持 AIMerge，会自动回退到 KeepLocal
let resolved = apply_resolution_strategy(
    &ConflictResolutionChoice::AIMerge,
    &local_version,
    &remote_versions,
    "config/database",
    &[],
)?;
// 返回本地版本
```

### 错误处理

```rust
match apply_resolution_strategy_async(
    &ConflictResolutionChoice::AIMerge,
    &local,
    &remotes,
    key,
    Some(&merger),
).await {
    Ok(merged_value) => {
        // 成功合并
        println!("Merged: {:?}", String::from_utf8_lossy(&merged_value));
    }
    Err(e) => {
        // AI 失败，已经自动回退到 KeepLocal
        // 或者提供其他错误处理
        eprintln!("Merge failed: {}", e);
    }
}
```

## 实现细节

### Prompt 构建

系统为 AI 提供详细的上下文信息：

```
**Conflict Key:** config/database

**Local Version (Node: node-a, Timestamp: 1000):**
{host: "localhost", port: 5432}

**Remote Version 1 (Node: node-b, Timestamp: 2000):**
{host: "prod.db.com", port: 5432}

---
**Task:** Merge these versions into a single, coherent value...
```

### AI 调用流程

```text
1. 检查 AI Provider 可用性
   ├─ 无 Provider → 回退到 KeepLocal
   └─ Provider 不可用 → 回退到 KeepLocal

2. 构建合并 Prompt
   ├─ 包含冲突键
   ├─ 包含本地版本（节点 ID、时间戳、值）
   └─ 包含所有远程版本

3. 调用 AI 服务（带重试）
   ├─ 超时控制（默认 30 秒）
   ├─ 失败重试（默认 2 次）
   └─ 返回合并结果

4. 解析 AI 响应
   ├─ 去除 markdown 标记
   ├─ 去除 JSON 标记
   └─ 返回字节数组

5. 错误处理
   ├─ 任何步骤失败 → 回退到 KeepLocal
   └─ 记录详细日志
```

### 重试机制

```rust
for attempt in 0..max_retries {
    match call_ai_merge(...).await {
        Ok(result) => return Ok(result),
        Err(e) => {
            tracing::error!("Attempt {} failed: {}", attempt + 1, e);
            last_error = Some(e);
        }
    }
}
// 所有重试都失败，返回错误
```

## 测试覆盖

### 单元测试 (`ai_merge.rs`)

1. **配置测试**
   - 默认配置验证
   - 自定义配置验证

2. **Prompt 构建测试**
   - 单个远程版本
   - 多个远程版本
   - 特殊字符处理

3. **系统提示词测试**
   - SmartMerge 提示词
   - ContentBased 提示词
   - TimeBased 提示词

4. **响应解析测试**
   - 普通文本
   - Markdown 代码块
   - JSON 标记
   - 空格处理

### 集成测试 (`ai_merge_integration_test.rs`)

1. **回退行为测试**
   - 同步版本回退到 KeepLocal
   - 异步版本无 AI Merger 回退

2. **策略集成测试**
   - KeepLocal 仍然工作
   - KeepRemote 仍然工作
   - AIMerge 正确回退

3. **端到端测试**
   - 完整的合并流程
   - 错误恢复

## 错误处理策略

### 1. AI Provider 不可用

**现象**: 没有设置 AI Provider 或 Provider 返回 `available() == false`

**处理**: 回退到 KeepLocal

```rust
tracing::warn!("[AIMerge] AI provider not available, falling back to KeepLocal");
return Ok(local_version.value.clone());
```

### 2. AI 调用超时

**现象**: AI 调用超过配置的超时时间（默认 30 秒）

**处理**: 记录错误，重试或回退

```rust
.map_err(|_| CisError::ai("AI merge timeout".to_string()))?
```

### 3. AI 返回错误

**现象**: AI 服务返回错误响应

**处理**: 记录错误，重试或回退

```rust
.map_err(|e| CisError::ai(format!("AI merge failed: {}", e)))?
```

### 4. 所有重试失败

**现象**: 达到最大重试次数仍未成功

**处理**: 返回最后一个错误

```rust
Err(last_error.unwrap_or_else(|| {
    CisError::ai("AI merge failed: unknown error".to_string())
}))
```

## 性能考虑

### 异步设计

整个 AIMerge 流程是异步的，不会阻塞其他操作：

```rust
pub async fn merge(...) -> Result<Vec<u8>>
```

### 超时控制

使用 Tokio 超时机制防止长时间等待：

```rust
tokio::time::timeout(
    Duration::from_secs(self.config.timeout_secs),
    ai_provider.chat_with_context(...)
).await
```

### 可配置性

所有关键参数都可配置：

- `max_retries`: 重试次数（默认 2）
- `timeout_secs`: 超时时间（默认 30 秒）
- `strategy`: 合并策略

## 日志记录

系统使用 `tracing` crate 记录详细日志：

```rust
tracing::info!("[AIMerge] Starting merge for key: {}, {} remote versions", key, remote_versions.len());
tracing::warn!("[AIMerge] No AI provider available, falling back to KeepLocal");
tracing::error!("[AIMerge] Attempt {} failed: {}", attempt + 1, e);
tracing::info!("[AIMerge] Successfully merged key: {}", key);
```

## 最佳实践

### 1. 总是提供 AI Provider

```rust
merger.set_ai_provider(Box::new(ClaudeCliProvider::default())).await;
```

### 2. 根据场景选择策略

```rust
// 配置文件 - 使用 SmartMerge
let config = AIMergeConfig {
    strategy: AIMergeStrategy::SmartMerge,
    ..Default::default()
};

// 文档内容 - 使用 ContentBased
let config = AIMergeConfig {
    strategy: AIMergeStrategy::ContentBased,
    ..Default::default()
};

// 日志数据 - 使用 TimeBased
let config = AIMergeConfig {
    strategy: AIMergeStrategy::TimeBased,
    ..Default::default()
};
```

### 3. 处理回退情况

```rust
let resolved = apply_resolution_strategy_async(
    &ConflictResolutionChoice::AIMerge,
    &local,
    &remotes,
    key,
    Some(&merger),
).await.unwrap_or_else(|_| local.value.clone());
```

### 4. 监控日志

```rust
// 启用 tracing
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .init();
```

## 未来改进

1. **更智能的合并策略**
   - 基于历史合并结果学习
   - 自动选择最佳策略

2. **冲突可视化**
   - 显示冲突的具体差异
   - 提供手动编辑界面

3. **合并审计**
   - 记录所有合并操作
   - 支持回滚到合并前的状态

4. **性能优化**
   - 缓存 AI Provider
   - 批量合并多个冲突

## 总结

AIMerge 实现提供了一个可靠、灵活的 AI 驱动冲突解决方案：

- ✅ **安全性**: 多层错误处理，总是能回退到 KeepLocal
- ✅ **灵活性**: 三种合并策略适应不同场景
- ✅ **可靠性**: 重试机制和超时控制
- ✅ **可测试性**: 完整的单元测试和集成测试
- ✅ **可观测性**: 详细的日志记录

实现符合 CIS 的设计原则，确保记忆系统的冲突解决既智能又可靠。
