//! # Doctor Command
//!
//! Check CIS environment and diagnose issues.

use anyhow::Result;
use cis_core::storage::paths::Paths;
use cis_core::wizard::checks::EnvironmentChecker;

/// Run all environment checks
pub fn doctor() -> Result<()> {
    println!("🔍 CIS Environment Check\n");
    
    let checker = EnvironmentChecker::new();
    let result = checker.run_all_checks()?;
    
    // Print check results
    println!("Environment Checks:");
    println!("{}", "-".repeat(50));
    
    // Display results by category
    println!("\n📁 Storage:");
    if Paths::data_dir().exists() {
        println!("  ✅ Data directory exists: {}", Paths::data_dir().display());
    } else {
        println!("  ❌ Data directory missing: {}", Paths::data_dir().display());
    }
    
    if Paths::config_file().exists() {
        println!("  ✅ Config file exists: {}", Paths::config_file().display());
    } else {
        println!("  ⚠️  Config file missing: {}", Paths::config_file().display());
    }
    
    // Check AI agents
    println!("\n🤖 AI Agents:");
    
    // Note: We can't call async here, so we just check if binaries exist
    let has_claude = which::which("claude").is_ok();
    let has_kimi = which::which("kimi").is_ok() || which::which("kimi-code").is_ok();
    let has_aider = which::which("aider").is_ok();
    
    if has_claude {
        println!("  ✅ Claude Code found");
    } else {
        println!("  ❌ Claude Code not found");
    }
    
    if has_kimi {
        println!("  ✅ Kimi found");
    } else {
        println!("  ❌ Kimi not found");
    }
    
    if has_aider {
        println!("  ✅ Aider found");
    } else {
        println!("  ❌ Aider not found");
    }
    
    if !has_claude && !has_kimi && !has_aider {
        println!("\n  ⚠️  Warning: No AI agent found. Install one to use AI features.");
    }
    
    // Check database
    println!("\n💾 Database:");
    match check_database() {
        Ok(_) => println!("  ✅ Database accessible"),
        Err(e) => println!("  ❌ Database error: {}", e),
    }
    
    // Display warnings and recommendations
    if !result.warnings.is_empty() {
        println!("\n⚠️  Warnings:");
        for warning in &result.warnings {
            println!("  - {}", warning);
        }
    }
    
    if !result.recommendations.is_empty() {
        println!("\n💡 Recommendations:");
        for rec in &result.recommendations {
            println!("  - {}", rec);
        }
    }
    
    // Final summary
    println!("\n{}", "-".repeat(50));
    if result.can_proceed {
        println!("✅ Environment check passed!");
    } else {
        println!("❌ Environment check failed. Please fix the issues above.");
    }
    
    Ok(())
}

/// Check database connectivity
fn check_database() -> Result<()> {
    let db_manager = cis_core::storage::db::DbManager::new()?;
    let _core_db = db_manager.core();
    Ok(())
}

/// Check if CIS is initialized
pub fn check_initialized() -> bool {
    Paths::config_file().exists()
}

/// Quick fix for common issues
pub fn quick_fix() -> Result<()> {
    println!("🔧 Running quick fixes...\n");
    
    // Ensure directories exist
    Paths::ensure_dirs()?;
    println!("✅ Ensured data directories exist");
    
    // Check if we need to initialize
    if !Paths::config_file().exists() {
        println!("⚠️  CIS not initialized. Run 'cis init' to initialize.");
    } else {
        println!("✅ CIS configuration found");
    }
    
    println!("\nQuick fixes completed.");
    
    Ok(())
}
