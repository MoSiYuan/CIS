//! # Conflict Resolution 实现逻辑 (P1.7.0 任务组 0.2)
//!
//! 🔥 **冲突检测和解决**
//!
//! # 核心机制
//!
//! - **Vector Clock 比较**：检测因果关系
//! - **LWW 决胜策略**：基于时间戳
//! - **冲突解决**：用户选择或自动解决
//! - **AI 合并**：智能合并冲突版本

use crate::error::{CisError, Result};
use crate::memory::guard::conflict_guard::{
    ConflictNotification, ConflictVersion, ConflictResolutionChoice,
};
use crate::memory::guard::vector_clock::{VectorClock, VectorClockRelation};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 🔥 LWW (Last-Write-Wins) 决胜策略
///
/// # 核心逻辑
///
/// 基于时间戳选择最新版本：
/// - 比较 `timestamp` 字段
/// - 选择时间戳最新的版本
///
/// # 参数
///
/// - `versions`: 所有冲突版本
///
/// # 返回
///
/// 返回时间戳最新的 `ConflictVersion`。
///
/// # 示例
///
/// ```rust
/// let winner = resolve_by_lww(&versions);
/// ```
pub fn resolve_by_lww(versions: &[ConflictVersion]) -> Result<&ConflictVersion> {
    if versions.is_empty() {
        return Err(CisError::memory_not_found("No versions to resolve"));
    }

    let winner = versions
        .iter()
        .max_by_key(|v| v.timestamp)
        .ok_or_else(|| CisError::memory_not_found("Failed to find max timestamp"))?;

    Ok(winner)
}

/// 🔥 基于 Vector Clock 检测冲突
///
/// # 核心逻辑
///
/// 1. 解析 Vector Clock
/// 2. 比较版本关系
/// 3. 如果并发则判定为冲突
///
/// # 参数
///
/// - `local_version`: 本地版本
/// - `remote_versions`: 远程版本列表
///
/// # 返回
///
/// 返回 `true` 如果检测到冲突（并发版本）。
///
/// # 示例
///
/// ```rust
/// let has_conflict = detect_conflict_by_vector_clock(&local, &remotes)?;
/// ```
pub fn detect_conflict_by_vector_clock(
    local_version: &ConflictVersion,
    remote_versions: &[ConflictVersion],
) -> Result<bool> {
    // 解析本地 Vector Clock
    let local_vc = deserialize_vector_clock(&local_version.vector_clock)?;

    // 检查每个远程版本
    for remote_version in remote_versions {
        let remote_vc = deserialize_vector_clock(&remote_version.vector_clock)?;

        // 比较关系
        match local_vc.compare(&remote_vc) {
            VectorClockRelation::Concurrent => {
                // 并发 = 冲突
                return Ok(true);
            }
            _ => {
                // Happens-Before, Happens-After, Equal = 无冲突
                continue;
            }
        }
    }

    Ok(false)
}

/// 🔥 KeepBoth 策略的解决结果
///
/// # 说明
///
/// 当选择 KeepBoth 时，需要：
/// 1. 保留本地版本（原 key）
/// 2. 保存远程版本到新 key（key_remote）
/// 3. 返回本地版本值和新 key 的信息
#[derive(Debug, Clone)]
pub struct KeepBothResult {
    /// 本地版本的值（保留在原 key）
    pub local_value: Vec<u8>,

    /// 远程版本应该保存的新 key
    pub remote_new_key: String,

    /// 远程版本的值
    pub remote_value: Vec<u8>,
}

/// 🔥 生成唯一的远程 key
///
/// # 核心逻辑
///
/// 为 KeepBoth 策略生成不冲突的 key：
/// 1. 尝试 `key_remote`
/// 2. 如果存在，尝试 `key_remote_2`, `key_remote_3`, ...
///
/// # 参数
///
/// - `base_key`: 基础 key
/// - `existing_keys`: 已存在的 key 集合
///
/// # 返回
///
/// 返回唯一的新 key。
///
/// # 示例
///
/// ```rust
/// let existing = vec!["key1".to_string(), "key1_remote".to_string()];
/// let new_key = generate_unique_remote_key("key1", &existing);
/// assert_eq!(new_key, "key1_remote_2");
/// ```
pub fn generate_unique_remote_key(base_key: &str, existing_keys: &[String]) -> String {
    let remote_key = format!("{}_remote", base_key);

    // 检查基础 remote key 是否存在
    if !existing_keys.contains(&remote_key) {
        return remote_key;
    }

    // 添加数字后缀直到找到唯一 key
    for i in 2..100 {
        let key = format!("{}_remote_{}", base_key, i);
        if !existing_keys.contains(&key) {
            return key;
        }
    }

    // 如果 100 次都失败，使用时间戳
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}_remote_{}", base_key, timestamp)
}

/// 🔥 应用冲突解决策略
///
/// # 核心逻辑
///
/// 根据 `ConflictResolutionChoice` 选择版本：
/// - `KeepLocal`: 保留本地版本
/// - `KeepRemote`: 保留指定远程版本
/// - `KeepBoth`: 保留两个版本（本地重命名为 key_local）
/// - `AIMerge`: AI 合并（TODO: 实现）
///
/// # 参数
///
/// - `choice`: 解决方案选择
/// - `local_version`: 本地版本
/// - `remote_versions`: 远程版本列表
/// - `key`: 冲突的记忆键
/// - `existing_keys`: 已存在的 key 集合（用于 KeepBoth 重命名检测）
///
/// # 返回
///
/// 返回解决后的记忆值。
///
/// # KeepBoth 特殊说明
///
/// 当选择 KeepBoth 时，返回本地版本值，同时调用方需要：
/// - 将远程版本保存到 `remote_new_key`
///
/// # 示例
///
/// ```rust,ignore
/// let resolved_value = apply_resolution_strategy(
///     &ConflictResolutionChoice::KeepLocal,
///     &local,
///     &remotes,
///     "key1",
///     &[]
/// )?;
/// ```
pub fn apply_resolution_strategy(
    choice: &ConflictResolutionChoice,
    local_version: &ConflictVersion,
    remote_versions: &[ConflictVersion],
    key: &str,
    existing_keys: &[String],
) -> Result<Vec<u8>> {
    match choice {
        ConflictResolutionChoice::KeepLocal => {
            // 保留本地版本
            Ok(local_version.value.clone())
        }

        ConflictResolutionChoice::KeepRemote { node_id } => {
            // 保留指定远程版本
            let remote = remote_versions
                .iter()
                .find(|v| &v.node_id == node_id)
                .ok_or_else(|| {
                    CisError::memory_not_found(&format!("Remote node {} not found", node_id))
                })?;

            Ok(remote.value.clone())
        }

        ConflictResolutionChoice::KeepBoth => {
            // 保留两个版本
            // 1. 本地版本保留在原 key
            // 2. 远程版本保存到新 key（key_remote）

            // 选择第一个远程版本（或可以选择时间戳最新的）
            let remote = remote_versions
                .first()
                .ok_or_else(|| CisError::memory_not_found("No remote versions available"))?;

            // 生成唯一的远程 key
            let remote_new_key = generate_unique_remote_key(key, existing_keys);

            tracing::info!(
                "KeepBoth strategy: local version kept at '{}', remote version will be saved to '{}'",
                key,
                remote_new_key
            );

            // 返回本地版本值（调用方需要处理远程版本的保存）
            Ok(local_version.value.clone())
        }

        ConflictResolutionChoice::AIMerge => {
            // AI 合并
            // 注意：同步版本不支持 AI 合并，回退到 KeepLocal
            tracing::warn!(
                "[ConflictResolution] AIMerge not supported in sync mode, falling back to KeepLocal for key: {}",
                key
            );
            Ok(local_version.value.clone())
        }
    }
}

/// 🔥 应用冲突解决策略（异步版本，支持 AI Merge）
///
/// # 核心逻辑
///
/// 根据 `ConflictResolutionChoice` 选择版本：
/// - `KeepLocal`: 保留本地版本
/// - `KeepRemote`: 保留指定远程版本
/// - `KeepBoth`: 保留两个版本（本地重命名为 key_local）
/// - `AIMerge`: AI 合并（调用 AI 服务智能合并）
///
/// # 参数
///
/// - `choice`: 解决方案选择
/// - `local_version`: 本地版本
/// - `remote_versions`: 远程版本列表
/// - `key`: 冲突的记忆键
/// - `ai_merger`: 可选的 AI 合并器（用于 AIMerge 策略）
///
/// # 返回
///
/// 返回解决后的记忆值。
///
/// # 示例
///
/// ```rust,no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// use cis_core::memory::guard::conflict_resolution::{apply_resolution_strategy_async, AIMerger};
/// use cis_core::memory::guard::conflict_guard::ConflictResolutionChoice;
///
/// let merger = AIMerger::default();
/// let resolved = apply_resolution_strategy_async(
///     &ConflictResolutionChoice::AIMerge,
///     &local,
///     &remotes,
///     "key1",
///     Some(&merger)
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn apply_resolution_strategy_async(
    choice: &ConflictResolutionChoice,
    local_version: &ConflictVersion,
    remote_versions: &[ConflictVersion],
    key: &str,
    ai_merger: Option<&super::ai_merge::AIMerger>,
) -> Result<Vec<u8>> {
    match choice {
        ConflictResolutionChoice::KeepLocal => {
            tracing::info!("[ConflictResolution] Keeping local version for key: {}", key);
            Ok(local_version.value.clone())
        }

        ConflictResolutionChoice::KeepRemote { node_id } => {
            tracing::info!(
                "[ConflictResolution] Keeping remote version from {} for key: {}",
                node_id,
                key
            );
            let remote = remote_versions
                .iter()
                .find(|v| &v.node_id == node_id)
                .ok_or_else(|| {
                    CisError::memory_not_found(&format!("Remote node {} not found", node_id))
                })?;

            Ok(remote.value.clone())
        }

        ConflictResolutionChoice::KeepBoth => {
            tracing::info!(
                "[ConflictResolution] KeepBoth requested for key: {}",
                key
            );
            // 异步版本也返回本地版本，调用方需要处理远程版本
            Ok(local_version.value.clone())
        }

        ConflictResolutionChoice::AIMerge => {
            tracing::info!("[ConflictResolution] AI merge requested for key: {}", key);

            // 检查是否提供了 AI Merger
            if let Some(merger) = ai_merger {
                merger.merge(key, local_version, remote_versions).await
            } else {
                tracing::warn!(
                    "[ConflictResolution] No AI merger provided, falling back to KeepLocal for key: {}",
                    key
                );
                Ok(local_version.value.clone())
            }
        }
    }
}

/// 🔥 应用 KeepBoth 策略并返回详细信息
///
/// # 核心逻辑
///
/// 与 `apply_resolution_strategy` 类似，但返回 KeepBothResult，
/// 包含本地值、远程值和新 key 信息。
///
/// # 参数
///
/// - `local_version`: 本地版本
/// - `remote_versions`: 远程版本列表
/// - `key`: 冲突的记忆键
/// - `existing_keys`: 已存在的 key 集合
///
/// # 返回
///
/// 返回 `KeepBothResult`，包含：
/// - `local_value`: 本地版本的值
/// - `remote_new_key`: 远程版本应该保存的新 key
/// - `remote_value`: 远程版本的值
///
/// # 示例
///
/// ```rust
/// let result = apply_keep_both_strategy(&local, &remotes, "key1", &[])?;
/// // 保存远程版本到新 key
/// memory_service.set(&result.remote_new_key, &result.remote_value, ...)?;
/// ```
pub fn apply_keep_both_strategy(
    local_version: &ConflictVersion,
    remote_versions: &[ConflictVersion],
    key: &str,
    existing_keys: &[String],
) -> Result<KeepBothResult> {
    // 选择第一个远程版本（或可以选择时间戳最新的）
    let remote = remote_versions
        .first()
        .ok_or_else(|| CisError::memory_not_found("No remote versions available"))?;

    // 生成唯一的远程 key
    let remote_new_key = generate_unique_remote_key(key, existing_keys);

    tracing::info!(
        "KeepBoth strategy: local version kept at '{}', remote version will be saved to '{}'",
        key,
        remote_new_key
    );

    Ok(KeepBothResult {
        local_value: local_version.value.clone(),
        remote_new_key,
        remote_value: remote.value.clone(),
    })
}

/// 🔥 反序列化 Vector Clock
///
/// # 参数
///
/// - `bytes`: 序列化的 Vector Clock 字节
///
/// # 返回
///
/// 返回解析后的 `VectorClock`。
fn deserialize_vector_clock(bytes: &[u8]) -> Result<VectorClock> {
    // TODO: 实现反序列化逻辑
    // 当前返回空 Vector Clock（临时实现）
    if bytes.is_empty() {
        Ok(VectorClock::new())
    } else {
        // 临时：假设 bytes 是 JSON 格式
        match serde_json::from_slice::<HashMap<String, u64>>(bytes) {
            Ok(counters) => {
                let mut vc = VectorClock::new();
                for (node_id, counter) in counters {
                    // 直接设置计数器（通过 increment 实现）
                    for _ in 0..counter {
                        vc.increment(&node_id);
                    }
                }
                Ok(vc)
            }
            Err(_) => {
                // 如果不是 JSON，返回空 Vector Clock
                Ok(VectorClock::new())
            }
        }
    }
}

/// 🔥 序列化 Vector Clock
///
/// # 参数
///
/// - `vc`: Vector Clock
///
/// # 返回
///
/// 返回序列化后的字节。
pub fn serialize_vector_clock(vc: &VectorClock) -> Result<Vec<u8>> {
    // 序列化为 JSON
    serde_json::to_vec(vc.counters())
        .map_err(|e| CisError::memory_not_found(&format!("Failed to serialize Vector Clock: {}", e)))
}

/// 🔥 创建冲突通知
///
/// # 参数
///
/// - `key`: 记忆键
/// - `local_version`: 本地版本
/// - `remote_versions`: 远程版本列表
///
/// # 返回
///
/// 返回 `ConflictNotification`。
pub fn create_conflict_notification(
    key: String,
    local_version: ConflictVersion,
    remote_versions: Vec<ConflictVersion>,
) -> ConflictNotification {
    ConflictNotification {
        key,
        local_version,
        remote_versions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 LWW 决胜策略
    #[test]
    fn test_resolve_by_lww() {
        let version1 = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"value1".to_vec(),
            timestamp: 1000,
        };

        let version2 = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: vec![],
            value: b"value2".to_vec(),
            timestamp: 2000,
        };

        let versions = vec![version1, version2];
        let winner = resolve_by_lww(&versions).unwrap();

        assert_eq!(winner.timestamp, 2000);
        assert_eq!(winner.value, b"value2");
    }

    /// 测试 Vector Clock 冲突检测
    #[test]
    fn test_detect_conflict_by_vector_clock() {
        // 创建并发版本
        let mut vc1 = VectorClock::new();
        vc1.increment("node-a");

        let mut vc2 = VectorClock::new();
        vc2.increment("node-b");

        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: serialize_vector_clock(&vc1).unwrap(),
            value: b"local".to_vec(),
            timestamp: 1000,
        };

        let remote = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: serialize_vector_clock(&vc2).unwrap(),
            value: b"remote".to_vec(),
            timestamp: 1000,
        };

        let has_conflict = detect_conflict_by_vector_clock(&local, &[remote]).unwrap();
        assert!(has_conflict);  // 并发 = 冲突
    }

    /// 测试应用解决策略（KeepLocal）
    #[test]
    fn test_apply_resolution_keep_local() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let remotes = vec![];

        let resolved = apply_resolution_strategy(
            &ConflictResolutionChoice::KeepLocal,
            &local,
            &remotes,
            "key1",
        ).unwrap();

        assert_eq!(resolved, b"local_value");
    }

    /// 测试应用解决策略（KeepRemote）
    #[test]
    fn test_apply_resolution_keep_remote() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let remote = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: vec![],
            value: b"remote_value".to_vec(),
            timestamp: 2000,
        };

        let resolved = apply_resolution_strategy(
            &ConflictResolutionChoice::KeepRemote {
                node_id: "node-b".to_string(),
            },
            &local,
            &[remote],
            "key1",
        ).unwrap();

        assert_eq!(resolved, b"remote_value");
    }

    /// 测试序列化和反序列化
    #[test]
    fn test_vector_clock_serialize_deserialize() {
        let mut vc = VectorClock::new();
        vc.increment("node-a");
        vc.increment("node-b");

        let serialized = serialize_vector_clock(&vc).unwrap();
        let deserialized = deserialize_vector_clock(&serialized).unwrap();

        assert_eq!(deserialized.get("node-a"), Some(&1));
        assert_eq!(deserialized.get("node-b"), Some(&1));
    }

    // ========== KeepBoth 策略测试 ==========

    /// 测试生成唯一的远程 key（基础情况）
    #[test]
    fn test_generate_unique_remote_key_basic() {
        let existing = vec!["key1".to_string()];
        let new_key = generate_unique_remote_key("key1", &existing);

        assert_eq!(new_key, "key1_remote");
    }

    /// 测试生成唯一的远程 key（已存在 _remote）
    #[test]
    fn test_generate_unique_remote_key_with_conflict() {
        let existing = vec![
            "key1".to_string(),
            "key1_remote".to_string(),
        ];
        let new_key = generate_unique_remote_key("key1", &existing);

        assert_eq!(new_key, "key1_remote_2");
    }

    /// 测试生成唯一的远程 key（多个冲突）
    #[test]
    fn test_generate_unique_remote_key_multiple_conflicts() {
        let existing = vec![
            "key1".to_string(),
            "key1_remote".to_string(),
            "key1_remote_2".to_string(),
            "key1_remote_3".to_string(),
        ];
        let new_key = generate_unique_remote_key("key1", &existing);

        assert_eq!(new_key, "key1_remote_4");
    }

    /// 测试生成唯一的远程 key（无冲突）
    #[test]
    fn test_generate_unique_remote_key_no_conflict() {
        let existing = vec!["key2".to_string()];
        let new_key = generate_unique_remote_key("key1", &existing);

        assert_eq!(new_key, "key1_remote");
    }

    /// 测试生成唯一的远程 key（空列表）
    #[test]
    fn test_generate_unique_remote_key_empty_list() {
        let existing = vec![];
        let new_key = generate_unique_remote_key("key1", &existing);

        assert_eq!(new_key, "key1_remote");
    }

    /// 测试应用 KeepBoth 策略
    #[test]
    fn test_apply_keep_both_strategy() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let remote = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: vec![],
            value: b"remote_value".to_vec(),
            timestamp: 2000,
        };

        let existing_keys = vec!["key1".to_string()];
        let result = apply_keep_both_strategy(
            &local,
            &[remote],
            "key1",
            &existing_keys,
        ).unwrap();

        // 验证本地值
        assert_eq!(result.local_value, b"local_value");

        // 验证远程值
        assert_eq!(result.remote_value, b"remote_value");

        // 验证新 key
        assert_eq!(result.remote_new_key, "key1_remote");
    }

    /// 测试应用 KeepBoth 策略（有冲突）
    #[test]
    fn test_apply_keep_both_strategy_with_conflict() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let remote = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: vec![],
            value: b"remote_value".to_vec(),
            timestamp: 2000,
        };

        // 模拟已存在 key1_remote
        let existing_keys = vec![
            "key1".to_string(),
            "key1_remote".to_string(),
        ];
        let result = apply_keep_both_strategy(
            &local,
            &[remote],
            "key1",
            &existing_keys,
        ).unwrap();

        // 验证新 key 应该是 key1_remote_2
        assert_eq!(result.remote_new_key, "key1_remote_2");
    }

    /// 测试应用 KeepBoth 策略（无远程版本）
    #[test]
    fn test_apply_keep_both_strategy_no_remote() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let existing_keys = vec!["key1".to_string()];
        let result = apply_keep_both_strategy(
            &local,
            &[],
            "key1",
            &existing_keys,
        );

        // 应该返回错误：没有远程版本
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No remote versions"));
    }

    /// 测试应用解决策略（KeepBoth）
    #[test]
    fn test_apply_resolution_keep_both() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let remote = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: vec![],
            value: b"remote_value".to_vec(),
            timestamp: 2000,
        };

        let existing_keys = vec!["key1".to_string()];
        let resolved = apply_resolution_strategy(
            &ConflictResolutionChoice::KeepBoth,
            &local,
            &[remote],
            "key1",
            &existing_keys,
        ).unwrap();

        // KeepBoth 应该返回本地版本值
        assert_eq!(resolved, b"local_value");
    }

    /// 测试 KeepBothResult 结构体
    #[test]
    fn test_keep_both_result_struct() {
        let result = KeepBothResult {
            local_value: b"local".to_vec(),
            remote_new_key: "key1_remote".to_string(),
            remote_value: b"remote".to_vec(),
        };

        assert_eq!(result.local_value, b"local");
        assert_eq!(result.remote_new_key, "key1_remote");
        assert_eq!(result.remote_value, b"remote");
    }

    /// 测试多个连续冲突的重命名
    #[test]
    fn test_keep_both_sequential_conflicts() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_v1".to_vec(),
            timestamp: 1000,
        };

        // 第一次冲突：key1 -> key1_remote
        let existing1 = vec!["key1".to_string()];
        let result1 = apply_keep_both_strategy(
            &local,
            &[ConflictVersion {
                node_id: "node-b".to_string(),
                vector_clock: vec![],
                value: b"remote_v1".to_vec(),
                timestamp: 2000,
            }],
            "key1",
            &existing1,
        ).unwrap();
        assert_eq!(result1.remote_new_key, "key1_remote");

        // 第二次冲突：key1 -> key1_remote_2（因为 key1_remote 已存在）
        let existing2 = vec![
            "key1".to_string(),
            "key1_remote".to_string(),
        ];
        let result2 = apply_keep_both_strategy(
            &local,
            &[ConflictVersion {
                node_id: "node-c".to_string(),
                vector_clock: vec![],
                value: b"remote_v2".to_vec(),
                timestamp: 3000,
            }],
            "key1",
            &existing2,
        ).unwrap();
        assert_eq!(result2.remote_new_key, "key1_remote_2");

        // 第三次冲突：key1 -> key1_remote_3
        let existing3 = vec![
            "key1".to_string(),
            "key1_remote".to_string(),
            "key1_remote_2".to_string(),
        ];
        let result3 = apply_keep_both_strategy(
            &local,
            &[ConflictVersion {
                node_id: "node-d".to_string(),
                vector_clock: vec![],
                value: b"remote_v3".to_vec(),
                timestamp: 4000,
            }],
            "key1",
            &existing3,
        ).unwrap();
        assert_eq!(result3.remote_new_key, "key1_remote_3");
    }

    /// 测试带路径的 key 重命名
    #[test]
    fn test_keep_both_with_path_keys() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local".to_vec(),
            timestamp: 1000,
        };

        let existing_keys = vec![
            "project/my-project/config".to_string(),
        ];

        let result = apply_keep_both_strategy(
            &local,
            &[ConflictVersion {
                node_id: "node-b".to_string(),
                vector_clock: vec![],
                value: b"remote".to_vec(),
                timestamp: 2000,
            }],
            "project/my-project/config",
            &existing_keys,
        ).unwrap();

        assert_eq!(result.remote_new_key, "project/my-project/config_remote");
    }

    /// 测试 KeepBoth 策略保留本地版本数据完整性
    #[test]
    fn test_keep_both_preserves_local_data() {
        let local_value = b"important_local_data".to_vec();
        let remote_value = b"conflicting_remote_data".to_vec();

        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: local_value.clone(),
            timestamp: 1000,
        };

        let remote = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: vec![],
            value: remote_value,
            timestamp: 2000,
        };

        let existing_keys = vec!["key1".to_string()];
        let result = apply_keep_both_strategy(
            &local,
            &[remote],
            "key1",
            &existing_keys,
        ).unwrap();

        // 验证本地数据完整保留
        assert_eq!(result.local_value, local_value);
        assert_eq!(String::from_utf8_lossy(&result.local_value), "important_local_data");
    }

    // ========== AIMerge 策略测试 ==========

    /// 测试 AIMerge 在同步模式下回退到 KeepLocal
    #[test]
    fn test_ai_merge_fallback_in_sync_mode() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let remote = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: vec![],
            value: b"remote_value".to_vec(),
            timestamp: 2000,
        };

        // 同步版本的 AIMerge 应该回退到 KeepLocal
        let resolved = apply_resolution_strategy(
            &ConflictResolutionChoice::AIMerge,
            &local,
            &[remote],
            "key1",
            &[],
        ).unwrap();

        // 应该返回本地版本
        assert_eq!(resolved, b"local_value");
    }

    /// 测试 AIMerge 在没有 AI Merger 时回退到 KeepLocal
    #[tokio::test]
    async fn test_ai_merge_fallback_without_merger() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let remote = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: vec![],
            value: b"remote_value".to_vec(),
            timestamp: 2000,
        };

        // 异步版本，但没有提供 AI Merger
        let resolved = apply_resolution_strategy_async(
            &ConflictResolutionChoice::AIMerge,
            &local,
            &[remote],
            "key1",
            None,  // 没有 AI Merger
        ).await
        .unwrap();

        // 应该回退到本地版本
        assert_eq!(resolved, b"local_value");
    }

    /// 测试 AIMerge 保留其他策略的正确行为
    #[tokio::test]
    async fn test_apply_resolution_strategy_async_keep_local() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let resolved = apply_resolution_strategy_async(
            &ConflictResolutionChoice::KeepLocal,
            &local,
            &[],
            "key1",
            None,
        ).await
        .unwrap();

        assert_eq!(resolved, b"local_value");
    }

    /// 测试 AIMerge 保留远程策略的正确行为
    #[tokio::test]
    async fn test_apply_resolution_strategy_async_keep_remote() {
        let local = ConflictVersion {
            node_id: "node-a".to_string(),
            vector_clock: vec![],
            value: b"local_value".to_vec(),
            timestamp: 1000,
        };

        let remote = ConflictVersion {
            node_id: "node-b".to_string(),
            vector_clock: vec![],
            value: b"remote_value".to_vec(),
            timestamp: 2000,
        };

        let resolved = apply_resolution_strategy_async(
            &ConflictResolutionChoice::KeepRemote {
                node_id: "node-b".to_string(),
            },
            &local,
            &[remote],
            "key1",
            None,
        ).await
        .unwrap();

        assert_eq!(resolved, b"remote_value");
    }
}
