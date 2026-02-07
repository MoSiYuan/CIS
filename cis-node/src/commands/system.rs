//! # System Management Commands
//!
//! 系统级管理命令：
//! - `cis system status` - 查看系统状态
//! - `cis system init` - 初始化系统（含目录结构）
//! - `cis system migrate` - 迁移旧配置
//! - `cis system clean` - 清理缓存/日志
//! - `cis system purge` - 完全卸载（危险）

use anyhow::Result;
use clap::Subcommand;
use tracing::{info, warn};

use cis_core::storage::unified_paths::{Cleanup, UnifiedPaths};

/// System management commands
#[derive(Debug, Subcommand)]
pub enum SystemCommands {
    /// Show system status
    Status {
        /// Output format
        #[arg(short, long, default_value = "human")]
        format: String,
    },

    /// Initialize CIS system directories
    Init {
        /// Force reinitialize even if already initialized
        #[arg(short, long)]
        force: bool,

        /// Non-interactive mode
        #[arg(long)]
        non_interactive: bool,
    },

    /// Migrate from legacy directory structure
    Migrate {
        /// Show what would be migrated without doing it
        #[arg(short, long)]
        dry_run: bool,
    },

    /// Clean up cache and old logs
    Clean {
        /// Clean cache
        #[arg(short, long)]
        cache: bool,

        /// Clean logs older than N days
        #[arg(short, long, value_name = "DAYS")]
        logs: Option<u32>,

        /// Clean all (cache + logs)
        #[arg(long)]
        all: bool,
    },

    /// Purge all CIS data (DANGEROUS)
    Purge {
        /// Confirm purge without interactive prompt
        #[arg(short, long)]
        force: bool,

        /// Also remove backup
        #[arg(long)]
        include_backup: bool,
    },

    /// Check system health and configuration
    Check {
        /// Output format
        #[arg(short, long, default_value = "human")]
        format: String,

        /// Try to auto-fix issues
        #[arg(short, long)]
        fix: bool,
    },
}

/// Handle system commands
pub async fn handle(cmd: SystemCommands) -> Result<()> {
    match cmd {
        SystemCommands::Status { format } => show_status(&format).await?,
        SystemCommands::Init { force, non_interactive } => init_system(force, non_interactive).await?,
        SystemCommands::Migrate { dry_run } => migrate_system(dry_run).await?,
        SystemCommands::Clean { cache, logs, all } => clean_system(cache, logs, all).await?,
        SystemCommands::Purge { force, include_backup } => purge_system(force, include_backup).await?,
        SystemCommands::Check { format, fix } => check_system(&format, fix).await?,
    }

    Ok(())
}

/// Show system status
async fn show_status(format: &str) -> Result<()> {
    let base_dir = UnifiedPaths::base_dir();
    let legacy_dir = UnifiedPaths::legacy_config_dir();

    let status = serde_json::json!({
        "initialized": base_dir.exists(),
        "directories": {
            "base": base_dir.display().to_string(),
            "config": UnifiedPaths::config_file().display().to_string(),
            "data": UnifiedPaths::data_dir().display().to_string(),
            "models": UnifiedPaths::models_dir().display().to_string(),
            "logs": UnifiedPaths::logs_dir().display().to_string(),
        },
        "legacy": {
            "exists": legacy_dir.exists(),
            "path": legacy_dir.display().to_string(),
            "needs_migration": UnifiedPaths::needs_migration(),
        },
        "health": check_health(),
    });

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("╔════════════════════════════════════════╗");
        println!("║          CIS System Status             ║");
        println!("╚════════════════════════════════════════╝");
        println!();
        println!("Base Directory: {}", status["directories"]["base"].as_str().unwrap());
        println!();
        println!("Directories:");
        println!("  Config: {}", status["directories"]["config"].as_str().unwrap());
        println!("  Data:   {}", status["directories"]["data"].as_str().unwrap());
        println!("  Models: {}", status["directories"]["models"].as_str().unwrap());
        println!("  Logs:   {}", status["directories"]["logs"].as_str().unwrap());
        println!();

        if status["legacy"]["exists"].as_bool().unwrap() {
            if status["legacy"]["needs_migration"].as_bool().unwrap() {
                println!("⚠️  Legacy directory found, migration needed:");
                println!("   {}", status["legacy"]["path"].as_str().unwrap());
                println!("   Run: cis system migrate");
            } else {
                println!("✓ Legacy directory migrated");
            }
        }
        println!();
        println!("Health: {}", status["health"]["status"].as_str().unwrap());
    }

    Ok(())
}

/// Initialize system directories
async fn init_system(force: bool, non_interactive: bool) -> Result<()> {
    if !force && UnifiedPaths::base_dir().exists() {
        if non_interactive {
            println!("System already initialized. Use --force to reinitialize.");
            return Ok(());
        }

        print!("System already initialized. Reinitialize? (y/N): ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() != "y" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    info!("Initializing CIS system directories...");

    UnifiedPaths::init()?;

    println!("✓ CIS system initialized successfully");
    println!();
    println!("Directories created:");
    println!("  {}", UnifiedPaths::base_dir().display());
    println!("  ├── config/  - Configuration files");
    println!("  ├── data/    - Runtime data (databases, sessions)");
    println!("  ├── models/  - AI models");
    println!("  ├── logs/    - Log files");
    println!("  └── cache/   - Cache files");
    println!();
    println!("Next steps:");
    println!("  cis init              # Complete initialization with AI provider");

    Ok(())
}

/// Migrate from legacy directory
async fn migrate_system(dry_run: bool) -> Result<()> {
    if !UnifiedPaths::legacy_config_dir().exists() {
        println!("No legacy directory found. Nothing to migrate.");
        return Ok(());
    }

    if !UnifiedPaths::needs_migration() {
        println!("Migration already completed or not needed.");
        return Ok(());
    }

    let legacy = UnifiedPaths::legacy_config_dir();
    let new_base = UnifiedPaths::base_dir();

    println!("Migration Plan:");
    println!();
    println!("From: {}", legacy.display());
    println!("To:   {}", new_base.display());
    println!();

    if dry_run {
        println!("(Dry run - no changes made)");
        return Ok(());
    }

    print!("Proceed with migration? (y/N): ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        println!("Cancelled.");
        return Ok(());
    }

    match UnifiedPaths::migrate() {
        Ok(report) => {
            if report.migrated {
                println!("✓ {}", report.message);
                println!();
                println!("Your old configuration has been backed up to:");
                println!("  {}.backup", legacy.display());
            } else {
                println!("⚠ {}", report.message);
            }
        }
        Err(e) => {
            eprintln!("✗ Migration failed: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

/// Clean up system
async fn clean_system(cache: bool, logs: Option<u32>, all: bool) -> Result<()> {
    let clean_cache = cache || all;
    let clean_logs = logs.is_some() || all;
    let log_days = logs.unwrap_or(30);

    if clean_cache {
        info!("Cleaning cache...");
        Cleanup::clean_cache()?;
        println!("✓ Cache cleaned");
    }

    if clean_logs {
        info!("Cleaning logs older than {} days...", log_days);
        Cleanup::clean_logs(log_days)?;
        println!("✓ Old logs cleaned (>{} days)", log_days);
    }

    if !clean_cache && !clean_logs {
        println!("Nothing to clean. Use --cache, --logs, or --all.");
    }

    Ok(())
}

/// Purge all data (DANGEROUS)
async fn purge_system(force: bool, include_backup: bool) -> Result<()> {
    println!("╔════════════════════════════════════════════════╗");
    println!("║  ⚠️  WARNING: This will DELETE ALL CIS DATA!   ║");
    println!("╚════════════════════════════════════════════════╝");
    println!();
    println!("This action will permanently delete:");
    println!("  - All configuration files");
    println!("  - All data (memories, sessions, DAG runs)");
    println!("  - All downloaded models");
    println!("  - All logs");
    println!();
    println!("Directories to be removed:");
    println!("  {}", UnifiedPaths::base_dir().display());
    if include_backup {
        println!("  {}.backup", UnifiedPaths::legacy_config_dir().display());
    }
    println!();

    if !force {
        print!("Type 'DELETE' to confirm: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim() != "DELETE" {
            println!("Cancelled.");
            return Ok(());
        }
    }

    info!("Purging all CIS data...");
    Cleanup::purge_all()?;

    println!("✓ All CIS data has been purged.");

    if include_backup {
        let backup = UnifiedPaths::legacy_config_dir().with_extension("backup");
        if backup.exists() {
            std::fs::remove_dir_all(&backup)?;
            println!("✓ Backup directory removed.");
        }
    }

    Ok(())
}

/// Check system health
async fn check_system(format: &str, fix: bool) -> Result<()> {
    let mut issues = vec![];
    let mut fixes = vec![];

    // Check 1: Directory structure
    if !UnifiedPaths::base_dir().exists() {
        issues.push("System directories not initialized");
        fixes.push("cis system init");
    }

    // Check 2: Legacy migration
    if UnifiedPaths::needs_migration() {
        issues.push("Legacy directory needs migration");
        fixes.push("cis system migrate");
    }

    // Check 3: Config file
    if !UnifiedPaths::config_file().exists() {
        issues.push("Configuration file not found");
        fixes.push("cis init");
    }

    // Check 4: Vector engine
    let vector_ready = !cis_core::ai::embedding_init::needs_init();
    if !vector_ready {
        issues.push("Vector engine not configured");
        fixes.push("cis config vector");
    }

    let health = serde_json::json!({
        "healthy": issues.is_empty(),
        "issues": issues,
        "fixes": fixes,
        "checks": {
            "directories": UnifiedPaths::base_dir().exists(),
            "config": UnifiedPaths::config_file().exists(),
            "vector_engine": vector_ready,
        },
    });

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&health)?);
    } else {
        println!("System Health Check");
        println!("===================");
        println!();

        if issues.is_empty() {
            println!("✓ All checks passed!");
        } else {
            println!("⚠️  Issues found:");
            for (i, issue) in issues.iter().enumerate() {
                println!("  {}. {}", i + 1, issue);
            }
            println!();
            println!("Suggested fixes:");
            for (i, fix_cmd) in fixes.iter().enumerate() {
                println!("  {}. Run: {}", i + 1, fix_cmd);
            }
        }

        if fix && !fixes.is_empty() {
            println!();
            println!("Auto-fixing issues...");
            // Implement auto-fix logic here
        }
    }

    Ok(())
}

/// Simple health check
fn check_health() -> serde_json::Value {
    let mut issues = vec![];

    if !UnifiedPaths::base_dir().exists() {
        issues.push("not_initialized");
    }

    if UnifiedPaths::needs_migration() {
        issues.push("needs_migration");
    }

    serde_json::json!({
        "status": if issues.is_empty() { "healthy" } else { "needs_attention" },
        "issues": issues,
    })
}
