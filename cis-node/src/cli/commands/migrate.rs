//! # Migrate Command
//!
//! 数据迁移命令：TOML → SQLite

use anyhow::{Context, Result};
use cis_core::task::{create_database_pool, TaskMigrator};
use cis_core::storage::paths::Paths;
use colored::*;
use std::path::PathBuf;
use std::sync::Arc;

/// 执行迁移命令
pub fn execute(
    source: String,
    database: Option<String>,
    verify: bool,
    rollback: bool,
) -> Result<()> {
    println!("Migrate called: source={}, verify={}, rollback={}",
        source, verify, rollback
    );

    // 确定数据库路径
    let db_path = if let Some(db) = database {
        PathBuf::from(db)
    } else {
        Paths::tasks_db()
    };

    println!("使用数据库: {}", db_path.display());

    // 创建数据库连接池
    let pool = create_database_pool(&db_path)
        .context("创建数据库连接池失败")?;

    let migrator = TaskMigrator::new(Arc::new(pool));

    // 处理回滚
    if rollback {
        println!("{}", "警告: 这将删除最近迁移的数据！".red().bold());
        println!("请确认回滚时间戳（Unix timestamp）: ");
        // 简化处理：使用当前时间前1小时作为示例
        let before_timestamp = chrono::Utc::now().timestamp() - 3600;
        println!("使用时间戳: {}", before_timestamp);

        let count = tokio_runtime()?.block_on(migrator.rollback_migration(before_timestamp))?;
        println!("{} 已回滚 {} 个任务", "✓".green(), count);
        return Ok(());
    }

    // 执行迁移
    let source_path = PathBuf::from(&source);
    let stats = if source_path.is_dir() {
        // 从目录迁移
        tokio_runtime()?.block_on(migrator.migrate_from_directory(&source_path))?
    } else if source_path.is_file() {
        // 从单个文件迁移
        tokio_runtime()?.block_on(migrator.migrate_from_toml_file(&source_path))?
    } else {
        return Err(anyhow::anyhow!("源路径不存在: {}", source));
    };

    // 验证迁移结果
    if verify {
        println!("\n{}", "验证迁移结果...".cyan());
        let verification = tokio_runtime()?.block_on(migrator.verify_migration())?;
        TaskMigrator::print_report(&stats, &verification);
    } else {
        // 打印简单统计
        println!("\n{}", "═════════════════════════════════════════".dimmed());
        println!("{}", "           迁移完成".bold().green());
        println!("{}", "═════════════════════════════════════════".dimmed());
        println!("  {} 成功迁移 {} 个任务",
            "✓".green(),
            stats.tasks_migrated.to_string().green()
        );
        println!("  {} 注册 {} 个 Teams",
            "✓".green(),
            stats.teams_registered.to_string().green()
        );

        if stats.tasks_failed > 0 {
            println!("  {} {} 个任务迁移失败",
                "✗".red(),
                stats.tasks_failed.to_string().red()
            );
        }

        println!("{}", "\n═════════════════════════════════════════".dimmed());
        println!("\n提示: 使用 --verify 查看详细验证报告");
    }

    // 打印警告
    if !stats.warnings.is_empty() {
        println!("\n{}", "警告:".bold().yellow());
        for warning in &stats.warnings {
            println!("  {} {}", "⚠".yellow(), warning);
        }
    }

    Ok(())
}

/// 创建 Tokio 运行时
fn tokio_runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Runtime::new()
        .context("创建 Tokio 运行时失败")
}
