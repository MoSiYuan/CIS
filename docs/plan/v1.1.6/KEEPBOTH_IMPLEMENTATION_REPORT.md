# KeepBoth 冲突解决策略实现报告

## 实现概述

成功实现了 KeepBoth 冲突解决策略的完整功能，包括重命名逻辑、冲突检测和全面的单元测试。

## 实现位置

- **文件**: `/Users/jiangxiaolong/work/project/CIS/cis-core/src/memory/guard/conflict_resolution.rs`
- **模块**: `cis_core::memory::guard::conflict_resolution`

## 核心功能实现

### 1. KeepBothResult 结构体

```rust
pub struct KeepBothResult {
    /// 本地版本的值（保留在原 key）
    pub local_value: Vec<u8>,

    /// 远程版本应该保存的新 key
    pub remote_new_key: String,

    /// 远程版本的值
    pub remote_value: Vec<u8>,
}
```

**说明**: 包含 KeepBoth 策略的所有结果信息，便于调用方处理。

### 2. generate_unique_remote_key 函数

```rust
pub fn generate_unique_remote_key(base_key: &str, existing_keys: &[String]) -> String
```

**功能**:
1. 首先尝试 `key_remote` 格式
2. 如果存在冲突，尝试 `key_remote_2`, `key_remote_3`, ... `key_remote_99`
3. 如果 100 次尝试都失败，使用时间戳生成唯一 key

**示例**:
- `key1` → `key1_remote`
- `key1_remote` 已存在 → `key1_remote_2`
- `key1_remote_2` 已存在 → `key1_remote_3`

### 3. apply_resolution_strategy 更新

**新增参数**:
- `existing_keys: &[String]` - 已存在的 key 集合，用于检测重命名冲突

**KeepBoth 分支实现**:
```rust
ConflictResolutionChoice::KeepBoth => {
    // 1. 选择第一个远程版本
    let remote = remote_versions.first()
        .ok_or_else(|| CisError::memory_not_found("No remote versions available"))?;

    // 2. 生成唯一的远程 key
    let remote_new_key = generate_unique_remote_key(key, existing_keys);

    // 3. 记录日志
    tracing::info!(
        "KeepBoth strategy: local version kept at '{}', remote version will be saved to '{}'",
        key,
        remote_new_key
    );

    // 4. 返回本地版本值（调用方需要处理远程版本的保存）
    Ok(local_version.value.clone())
}
```

### 4. apply_keep_both_strategy 函数

```rust
pub fn apply_keep_both_strategy(
    local_version: &ConflictVersion,
    remote_versions: &[ConflictVersion],
    key: &str,
    existing_keys: &[String],
) -> Result<KeepBothResult>
```

**功能**: 返回详细的 KeepBoth 结果，包含本地值、远程值和新 key。

**使用示例**:
```rust
let result = apply_keep_both_strategy(&local, &remotes, "key1", &existing_keys)?;

// 本地版本保留在原 key（无需操作）

// 保存远程版本到新 key
memory_service.set(
    &result.remote_new_key,
    &result.remote_value,
    domain,
    category
).await?;
```

## 单元测试

实现了 15 个全面的单元测试，覆盖所有场景：

### 基础功能测试

1. **test_generate_unique_remote_key_basic**: 测试基础 key 生成
2. **test_generate_unique_remote_key_with_conflict**: 测试冲突处理
3. **test_generate_unique_remote_key_multiple_conflicts**: 测试多个连续冲突
4. **test_generate_unique_remote_key_no_conflict**: 测试无冲突场景
5. **test_generate_unique_remote_key_empty_list**: 测试空列表

### 策略应用测试

6. **test_apply_keep_both_strategy**: 测试 KeepBoth 策略基本功能
7. **test_apply_keep_both_strategy_with_conflict**: 测试有冲突的情况
8. **test_apply_keep_both_strategy_no_remote**: 测试无远程版本的错误处理
9. **test_apply_resolution_keep_both**: 测试集成到 `apply_resolution_strategy`
10. **test_keep_both_result_struct**: 测试结果结构体

### 高级场景测试

11. **test_keep_both_sequential_conflicts**: 测试多次连续冲突的重命名
12. **test_keep_both_with_path_keys**: 测试带路径的 key（如 `project/my-project/config`）
13. **test_keep_both_preserves_local_data**: 测试本地数据完整性保护

### AIMerge 回退测试

14. **test_ai_merge_fallback_in_sync_mode**: 测试同步模式下 AIMerge 回退
15. **test_ai_merge_fallback_without_merger**: 测试异步模式下无 AI Merger 的回退

## 重命名策略详细说明

### 场景 1: 首次冲突

```rust
// 原始 key: "config"
// 已存在 keys: ["config"]

generate_unique_remote_key("config", &["config"])
// 返回: "config_remote"
```

### 场景 2: 已存在 _remote

```rust
// 原始 key: "config"
// 已存在 keys: ["config", "config_remote"]

generate_unique_remote_key("config", &["config", "config_remote"])
// 返回: "config_remote_2"
```

### 场景 3: 多个连续冲突

```rust
// 已存在 keys: ["config", "config_remote", "config_remote_2", "config_remote_3"]

generate_unique_remote_key("config", &[...])
// 返回: "config_remote_4"
```

### 场景 4: 极端情况（100 次冲突）

```rust
// 如果 key_remote_2 到 key_remote_99 都存在
// 使用时间戳保证唯一性

generate_unique_remote_key("config", &[...99 个冲突 keys...])
// 返回: "config_remote_1705312345"
```

### 场景 5: 带路径的 key

```rust
// 原始 key: "project/my-project/config"
// 已存在 keys: ["project/my-project/config"]

generate_unique_remote_key("project/my-project/config", &[...])
// 返回: "project/my-project/config_remote"
```

## 使用流程

### 方式 1: 使用 apply_resolution_strategy

```rust
// 1. 检测冲突
let conflict = guard.check_conflicts(key).await?;

// 2. 用户选择 KeepBoth
let choice = ConflictResolutionChoice::KeepBoth;

// 3. 获取现有 keys
let existing_keys = memory_service.list_keys(None).await?;

// 4. 应用策略（返回本地值）
let local_value = apply_resolution_strategy(
    &choice,
    &conflict.local_version,
    &conflict.remote_versions,
    key,
    &existing_keys,
)?;

// 5. 手动保存远程版本（调用方需要生成新 key）
let remote_new_key = generate_unique_remote_key(key, &existing_keys);
let remote = &conflict.remote_versions[0];
memory_service.set(
    &remote_new_key,
    &remote.value,
    domain,
    category
).await?;
```

### 方式 2: 使用 apply_keep_both_strategy（推荐）

```rust
// 1. 检测冲突
let conflict = guard.check_conflicts(key).await?;

// 2. 获取现有 keys
let existing_keys = memory_service.list_keys(None).await?;

// 3. 应用 KeepBoth 策略（返回完整结果）
let result = apply_keep_both_strategy(
    &conflict.local_version,
    &conflict.remote_versions,
    key,
    &existing_keys,
)?;

// 4. 本地版本已保留（无需操作）

// 5. 保存远程版本到新 key
memory_service.set(
    &result.remote_new_key,
    &result.remote_value,
    domain,
    category
).await?;
```

## 设计亮点

1. **自动冲突检测**: 自动检测 `key_remote` 是否已存在，避免覆盖
2. **递增后缀**: 使用 `_2`, `_3`, ... 递增后缀，清晰可读
3. **时间戳兜底**: 极端情况下使用时间戳保证唯一性
4. **完整结果返回**: `KeepBothResult` 提供所有必要信息
5. **日志记录**: 使用 `tracing::info!` 记录操作，便于调试
6. **错误处理**: 无远程版本时返回明确错误
7. **路径支持**: 支持带路径的 key（如 `project/my-project/config`）
8. **数据完整性**: 保证本地版本数据不被修改

## 测试覆盖

| 测试类别 | 测试数量 | 覆盖场景 |
|---------|---------|---------|
| 基础功能 | 5 | key 生成、冲突检测、边界情况 |
| 策略应用 | 5 | KeepBoth 策略、错误处理、集成 |
| 高级场景 | 3 | 连续冲突、路径 key、数据完整性 |
| AIMerge 回退 | 2 | 同步/异步回退逻辑 |
| **总计** | **15** | **全面覆盖** |

## 性能考虑

1. **O(n) 查找**: 使用 `Vec::contains` 查找现有 key，对于少量 keys 性能足够
2. **最多 100 次尝试**: 限制循环次数，避免无限循环
3. **提前返回**: 第一次成功立即返回，无需遍历所有可能性

## 未来优化建议

1. **使用 HashSet**: 如果 keys 数量很大，可以使用 `HashSet<String>` 提高查找性能
2. **批量重命名**: 支持批量处理多个冲突 key
3. **自定义后缀**: 允许用户指定自定义后缀（如 `_conflict`, `_backup`）
4. **重命名历史**: 记录重命名历史，便于追溯

## 实现验证

所有功能已实现并通过单元测试验证：

✅ 重命名策略（key_remote → key_remote_2 → ...）
✅ 冲突检测和处理
✅ 错误处理（无远程版本）
✅ 边界情况（空列表、路径 key）
✅ 数据完整性保证
✅ 日志记录
✅ 单元测试覆盖（15 个测试）

## 总结

成功实现了完整的 KeepBoth 冲突解决策略，包括：

1. ✅ 保留本地版本（原 key）
2. ✅ 将远程版本保存为 key_remote（重命名）
3. ✅ 处理重命名冲突（如果 key_remote 已存在，添加 _2, _3, ...）
4. ✅ 返回本地版本值
5. ✅ 添加了 15 个全面的单元测试

实现代码位于 `/Users/jiangxiaolong/work/project/CIS/cis-core/src/memory/guard/conflict_resolution.rs`，可以直接使用。
