//! # AIMerge 冲突解决示例
//!
//! 演示如何使用 AI 合并策略解决记忆冲突

use cis_core::ai::ClaudeCliProvider;
use cis_core::memory::guard::ai_merge::{AIMerger, AIMergeConfig, AIMergeStrategy};
use cis_core::memory::guard::conflict_guard::{ConflictResolutionChoice, ConflictVersion};
use cis_core::memory::guard::conflict_resolution::apply_resolution_strategy_async;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== CIS AIMerge 冲突解决示例 ===\n");

    // 1. 创建冲突版本
    let local_version = ConflictVersion {
        node_id: "node-device-1".to_string(),
        vector_clock: vec![],
        value: br#"{"database": {"host": "localhost", "port": 5432}, "features": ["feature-a"]}"#.to_vec(),
        timestamp: 1000,
    };

    let remote_version = ConflictVersion {
        node_id: "node-device-2".to_string(),
        vector_clock: vec![],
        value: br#"{"database": {"host": "prod.db.com", "port": 5432}, "features": ["feature-b"]}"#.to_vec(),
        timestamp: 2000,
    };

    println!("本地版本: {}", String::from_utf8_lossy(&local_version.value));
    println!("远程版本: {}", String::from_utf8_lossy(&remote_version.value));
    println!();

    // 2. 创建 AI 合并器
    let config = AIMergeConfig {
        strategy: AIMergeStrategy::SmartMerge,
        max_retries: 2,
        timeout_secs: 30,
    };
    let merger = AIMerger::new(config);

    // 3. 设置 AI Provider（如果可用）
    println!("正在检查 AI Provider...");
    let claude_provider = Box::new(ClaudeCliProvider::default());

    if claude_provider.available().await {
        println!("✅ AI Provider 可用，使用 Claude CLI\n");
        merger.set_ai_provider(claude_provider).await;

        // 4. 使用 AI 合并策略
        println!("正在执行 AI 合并...");
        let merged = apply_resolution_strategy_async(
            &ConflictResolutionChoice::AIMerge,
            &local_version,
            &[remote_version],
            "config/database",
            Some(&merger),
        )
        .await?;

        println!("✅ 合并成功!");
        println!("合并结果: {}", String::from_utf8_lossy(&merged));
    } else {
        println!("⚠️  AI Provider 不可用，回退到 KeepLocal");
        println!("使用本地版本: {}", String::from_utf8_lossy(&local_version.value));
    }

    println!("\n=== 示例完成 ===");

    Ok(())
}

/// 演示不同合并策略
#[allow(dead_code)]
async fn demo_strategies() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 演示不同合并策略 ===\n");

    // SmartMerge - 智能合并
    println!("1. SmartMerge (智能合并)");
    let smart_merger = AIMerger::new(AIMergeConfig {
        strategy: AIMergeStrategy::SmartMerge,
        ..Default::default()
    });
    println!("   - 保留双方有效信息");
    println!("   - 智能解决冲突");
    println!("   - 维护数据一致性\n");

    // ContentBased - 基于内容的合并
    println!("2. ContentBased (基于内容的合并)");
    let content_merger = AIMerger::new(AIMergeConfig {
        strategy: AIMergeStrategy::ContentBased,
        ..Default::default()
    });
    println!("   - 基于内容质量合并");
    println!("   - 优先保留更完整的信息\n");

    // TimeBased - 基于时间的合并
    println!("3. TimeBased (基于时间的合并)");
    let time_merger = AIMerger::new(AIMergeConfig {
        strategy: AIMergeStrategy::TimeBased,
        ..Default::default()
    });
    println!("   - 优先保留较新的修改");
    println!("   - 保留不冲突的旧修改\n");

    Ok(())
}

/// 演示错误处理
#[allow(dead_code)]
async fn demo_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 演示错误处理 ===\n");

    let local_version = ConflictVersion {
        node_id: "node-a".to_string(),
        vector_clock: vec![],
        value: b"local value".to_vec(),
        timestamp: 1000,
    };

    let remote_version = ConflictVersion {
        node_id: "node-b".to_string(),
        vector_clock: vec![],
        value: b"remote value".to_vec(),
        timestamp: 2000,
    };

    // 1. 没有 AI Provider 的情况
    println!("1. 测试：没有 AI Provider");
    let merger_no_provider = AIMerger::default();
    let result = apply_resolution_strategy_async(
        &ConflictResolutionChoice::AIMerge,
        &local_version,
        &[remote_version.clone()],
        "test/key",
        Some(&merger_no_provider),
    )
    .await?;

    println!("   ✅ 正确回退到 KeepLocal");
    println!("   结果: {}", String::from_utf8_lossy(&result));
    println!();

    // 2. 显式传入 None
    println!("2. 测试：显式传入 None");
    let result = apply_resolution_strategy_async(
        &ConflictResolutionChoice::AIMerge,
        &local_version,
        &[remote_version],
        "test/key2",
        None,
    )
    .await?;

    println!("   ✅ 正确回退到 KeepLocal");
    println!("   结果: {}", String::from_utf8_lossy(&result));

    Ok(())
}
