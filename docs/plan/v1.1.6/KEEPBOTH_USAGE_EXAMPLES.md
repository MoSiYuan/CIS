// KeepBoth 冲突解决策略 - 使用示例

use cis_core::memory::guard::conflict_resolution::{
    apply_keep_both_strategy, apply_resolution_strategy,
    generate_unique_remote_key, KeepBothResult,
};
use cis_core::memory::guard::conflict_guard::{
    ConflictResolutionChoice, ConflictVersion,
};
use cis_core::memory::MemoryService;

/// 示例 1: 基础使用 - 生成唯一 key
fn example_generate_unique_key() {
    // 场景: 首次冲突
    let existing_keys = vec!["config".to_string()];
    let new_key = generate_unique_remote_key("config", &existing_keys);
    println!("新 key: {}", new_key);  // 输出: config_remote

    // 场景: 已存在 config_remote
    let existing_keys = vec![
        "config".to_string(),
        "config_remote".to_string(),
    ];
    let new_key = generate_unique_remote_key("config", &existing_keys);
    println!("新 key: {}", new_key);  // 输出: config_remote_2
}

/// 示例 2: 使用 apply_keep_both_strategy（推荐）
async fn example_keep_both_strategy(
    memory_service: &MemoryService,
) -> Result<(), Box<dyn std::error::Error>> {
    // 模拟冲突数据
    let local_version = ConflictVersion {
        node_id: "node-a".to_string(),
        vector_clock: vec![],
        value: b"local_config_value".to_vec(),
        timestamp: 1000,
    };

    let remote_version = ConflictVersion {
        node_id: "node-b".to_string(),
        vector_clock: vec![],
        value: b"remote_config_value".to_vec(),
        timestamp: 2000,
    };

    // 获取现有 keys
    let existing_keys = memory_service
        .list_keys(None)
        .await?;

    // 应用 KeepBoth 策略
    let result: KeepBothResult = apply_keep_both_strategy(
        &local_version,
        &[remote_version],
        "config",
        &existing_keys,
    )?;

    println!("本地值: {:?}", String::from_utf8_lossy(&result.local_value));
    println!("远程新 key: {}", result.remote_new_key);
    println!("远程值: {:?}", String::from_utf8_lossy(&result.remote_value));

    // 本地版本已保留在原 key（无需操作）

    // 保存远程版本到新 key
    use cis_core::types::{MemoryDomain, MemoryCategory};
    memory_service.set(
        &result.remote_new_key,
        &result.remote_value,
        MemoryDomain::Public,
        MemoryCategory::Context
    ).await?;

    println!("KeepBoth 策略应用成功！");
    Ok(())
}

/// 示例 3: 使用 apply_resolution_strategy
async fn example_apply_resolution_strategy(
    memory_service: &MemoryService,
) -> Result<(), Box<dyn std::error::Error>> {
    // 模拟冲突数据
    let local_version = ConflictVersion {
        node_id: "node-a".to_string(),
        vector_clock: vec![],
        value: b"local_value".to_vec(),
        timestamp: 1000,
    };

    let remote_version = ConflictVersion {
        node_id: "node-b".to_string(),
        vector_clock: vec![],
        value: b"remote_value".to_vec(),
        timestamp: 2000,
    };

    // 获取现有 keys
    let existing_keys = memory_service
        .list_keys(None)
        .await?;

    // 应用策略（返回本地值）
    let local_value = apply_resolution_strategy(
        &ConflictResolutionChoice::KeepBoth,
        &local_version,
        &[remote_version],
        "key1",
        &existing_keys,
    )?;

    println!("本地值: {:?}", String::from_utf8_lossy(&local_value));

    // 手动处理远程版本
    let remote_new_key = generate_unique_remote_key("key1", &existing_keys);
    println!("远程新 key: {}", remote_new_key);

    // 保存远程版本
    // memory_service.set(&remote_new_key, &remote_version.value, ...).await?;

    Ok(())
}

/// 示例 4: 处理多个连续冲突
async fn example_sequential_conflicts() {
    let mut existing_keys = vec!["config".to_string()];

    // 第一次冲突
    let key1 = generate_unique_remote_key("config", &existing_keys);
    println!("第一次冲突: {}", key1);  // config_remote
    existing_keys.push(key1);

    // 第二次冲突
    let key2 = generate_unique_remote_key("config", &existing_keys);
    println!("第二次冲突: {}", key2);  // config_remote_2
    existing_keys.push(key2);

    // 第三次冲突
    let key3 = generate_unique_remote_key("config", &existing_keys);
    println!("第三次冲突: {}", key3);  // config_remote_3
}

/// 示例 5: 带路径的 key
fn example_path_keys() {
    let existing_keys = vec!["project/my-project/config".to_string()];
    let new_key = generate_unique_remote_key("project/my-project/config", &existing_keys);
    println!("路径 key: {}", new_key);  // project/my-project/config_remote
}

/// 示例 6: 完整的冲突解决流程
async fn example_full_conflict_resolution(
    memory_service: &MemoryService,
    key: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use cis_core::memory::guard::ConflictGuard;

    // 1. 创建 ConflictGuard
    let guard = ConflictGuard::new(memory_service.clone());

    // 2. 检查冲突
    let check_result = guard
        .check_conflicts_before_delivery(&[key.to_string()])
        .await?;

    match check_result {
        cis_core::memory::guard::conflict_guard::ConflictCheckResult::NoConflicts => {
            println!("无冲突，可以直接使用");
        }

        cis_core::memory::guard::conflict_guard::ConflictCheckResult::HasConflicts { conflicts } => {
            // 3. 获取冲突通知
            if let Some(notification) = conflicts.get(key) {
                println!("检测到冲突！");
                println!("本地版本: {:?}", notification.local_version);
                println!("远程版本: {:?}", notification.remote_versions);

                // 4. 用户选择 KeepBoth
                let choice = ConflictResolutionChoice::KeepBoth;

                // 5. 获取现有 keys
                let existing_keys = memory_service.list_keys(None).await?;

                // 6. 应用策略
                let result = apply_keep_both_strategy(
                    &notification.local_version,
                    &notification.remote_versions,
                    key,
                    &existing_keys,
                )?;

                // 7. 保存远程版本到新 key
                use cis_core::types::{MemoryDomain, MemoryCategory};
                memory_service.set(
                    &result.remote_new_key,
                    &result.remote_value,
                    MemoryDomain::Public,
                    MemoryCategory::Context
                ).await?;

                println!("冲突已解决：");
                println!("- 本地版本保留在: {}", key);
                println!("- 远程版本保存到: {}", result.remote_new_key);
            }
        }
    }

    Ok(())
}

/// 示例 7: 错误处理
async fn example_error_handling() {
    let local_version = ConflictVersion {
        node_id: "node-a".to_string(),
        vector_clock: vec![],
        value: b"local".to_vec(),
        timestamp: 1000,
    };

    let existing_keys = vec!["key1".to_string()];

    // 尝试应用 KeepBoth 策略，但没有远程版本
    let result = apply_keep_both_strategy(
        &local_version,
        &[].clone(),  // 空的远程版本列表
        "key1",
        &existing_keys,
    );

    match result {
        Ok(_) => println!("不应该成功"),
        Err(e) => println!("预期的错误: {}", e),
        // 输出: 预期的错误: No remote versions available
    }
}

/// 示例 8: 与其他策略比较
async fn example_strategy_comparison() {
    use cis_core::memory::guard::conflict_guard::ConflictResolutionChoice;

    let local_version = ConflictVersion {
        node_id: "node-a".to_string(),
        vector_clock: vec![],
        value: b"local".to_vec(),
        timestamp: 1000,
    };

    let remote_version = ConflictVersion {
        node_id: "node-b".to_string(),
        vector_clock: vec![],
        value: b"remote".to_vec(),
        timestamp: 2000,
    };

    let existing_keys = vec!["key1".to_string()];

    // 策略 1: KeepLocal - 只保留本地版本
    // 策略 2: KeepRemote - 只保留远程版本
    // 策略 3: KeepBoth - 保留两个版本（需要重命名）
    // 策略 4: AIMerge - AI 合并（需要 AI 服务）

    let result = apply_keep_both_strategy(
        &local_version,
        &[remote_version],
        "key1",
        &existing_keys,
    ).unwrap();

    println!("KeepBoth 结果:");
    println!("  本地值: {:?}", result.local_value);
    println!("  远程值: {:?}", result.remote_value);
    println!("  远程新 key: {}", result.remote_new_key);
}

fn main() {
    println!("KeepBoth 冲突解决策略 - 使用示例\n");

    println!("=== 示例 1: 生成唯一 key ===");
    example_generate_unique_key();
    println!();

    println!("=== 示例 4: 处理多个连续冲突 ===");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(example_sequential_conflicts());
    println!();

    println!("=== 示例 5: 带路径的 key ===");
    example_path_keys();
    println!();

    println!("=== 示例 7: 错误处理 ===");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(example_error_handling());
    println!();

    println!("=== 示例 8: 与其他策略比较 ===");
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(example_strategy_comparison());
}
