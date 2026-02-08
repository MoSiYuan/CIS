//! # Update Command
//!
//! Check for updates and upgrade CIS to the latest version.

use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};
use std::process::Command;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "MoSiYuan/CIS";
const GITHUB_API_URL: &str = "https://api.github.com/repos";

/// Update/Upgrade commands
#[derive(Subcommand, Debug)]
pub enum UpdateCommands {
    /// Check for available updates
    Check,
    /// Upgrade to the latest version
    Upgrade {
        /// Force upgrade even if already on latest version
        #[arg(long)]
        force: bool,
        /// Upgrade to a specific version
        #[arg(short, long)]
        version: Option<String>,
    },
    /// Show current version information
    Version,
}

/// Update CLI arguments
#[derive(Args, Debug)]
pub struct UpdateArgs {
    #[command(subcommand)]
    pub command: UpdateCommands,
}

/// Handle update commands
pub async fn handle(args: UpdateArgs) -> Result<()> {
    match args.command {
        UpdateCommands::Check => check_update().await,
        UpdateCommands::Upgrade { force, version } => upgrade(force, version).await,
        UpdateCommands::Version => show_version(),
    }
}

/// Check for available updates
async fn check_update() -> Result<()> {
    println!("🔍 Checking for updates...");
    println!("   Current version: {}", CURRENT_VERSION);
    
    match fetch_latest_version().await {
        Ok(latest) => {
            if latest == CURRENT_VERSION {
                println!("\n✅ You are on the latest version ({})", latest);
            } else {
                println!("\n📦 New version available: {}", latest);
                println!("   Current: {}", CURRENT_VERSION);
                println!("\n💡 Run `cis update upgrade` to upgrade");
                
                // Show release notes URL
                println!("\n📋 Release notes:");
                println!("   https://github.com/{}/releases/tag/v{}", 
                    GITHUB_REPO, latest);
            }
            Ok(())
        }
        Err(e) => {
            println!("\n⚠️  Failed to check for updates: {}", e);
            println!("   Please check your internet connection or try again later.");
            Err(e)
        }
    }
}

/// Upgrade to the latest version
async fn upgrade(force: bool, target_version: Option<String>) -> Result<()> {
    let version = match target_version {
        Some(v) => {
            println!("📦 Upgrading to specified version: {}", v);
            v
        }
        None => {
            println!("🔍 Checking for latest version...");
            match fetch_latest_version().await {
                Ok(latest) => {
                    if latest == CURRENT_VERSION && !force {
                        println!("\n✅ Already on latest version ({})", latest);
                        println!("   Use --force to reinstall");
                        return Ok(());
                    }
                    latest
                }
                Err(e) => {
                    return Err(anyhow!("Failed to fetch latest version: {}", e));
                }
            }
        }
    };
    
    println!("\n📥 Downloading CIS v{}...", version);
    
    // Determine installation method based on current installation
    if is_installed_via_cargo() {
        upgrade_via_cargo(&version).await
    } else if is_installed_via_homebrew() {
        upgrade_via_homebrew(&version).await
    } else {
        // Binary installation - provide manual instructions
        println!("\n⚠️  Automatic upgrade not available for binary installation.");
        println!("\n📋 Please download and install manually:");
        println!("   https://github.com/{}/releases/tag/v{}", GITHUB_REPO, version);
        println!("\n💡 Or install via package manager for automatic updates:");
        println!("   - Cargo: `cargo install cis`");
        println!("   - Homebrew: `brew install cis`");
        Ok(())
    }
}

/// Show current version information
fn show_version() -> Result<()> {
    println!("CIS {}", CURRENT_VERSION);
    println!();
    println!("Installation methods supported for updates:");
    
    if is_installed_via_cargo() {
        println!("  ✅ Cargo detected");
    } else {
        println!("  ⬜ Cargo");
    }
    
    if is_installed_via_homebrew() {
        println!("  ✅ Homebrew detected");
    } else {
        println!("  ⬜ Homebrew");
    }
    
    println!();
    println!("Repository: https://github.com/{}", GITHUB_REPO);
    println!("Documentation: https://docs.cis.dev");
    
    Ok(())
}

/// Fetch latest version from GitHub API
async fn fetch_latest_version() -> Result<String> {
    let url = format!("{}/{}/releases/latest", GITHUB_API_URL, GITHUB_REPO);
    
    let client = reqwest::Client::builder()
        .user_agent("cis-update-checker")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to connect to GitHub API: {}", e))?;
    
    if !response.status().is_success() {
        return Err(anyhow!("GitHub API returned error: {}", response.status()));
    }
    
    let release: serde_json::Value = response
        .json()
        .await
        .map_err(|e| anyhow!("Failed to parse GitHub response: {}", e))?;
    
    let tag_name = release["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow!("Invalid GitHub response: missing tag_name"))?;
    
    // Remove 'v' prefix if present
    let version = tag_name.trim_start_matches('v').to_string();
    
    Ok(version)
}

/// Check if installed via Cargo
fn is_installed_via_cargo() -> bool {
    // Check if running from cargo target directory or if cargo knows about it
    let output = Command::new("cargo")
        .args(["install", "--list"])
        .output();
    
    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        return stdout.contains("cis");
    }
    
    false
}

/// Check if installed via Homebrew
fn is_installed_via_homebrew() -> bool {
    let output = Command::new("brew")
        .args(["list", "cis"])
        .output();
    
    matches!(output, Ok(ref o) if o.status.success())
}

/// Upgrade via Cargo
async fn upgrade_via_cargo(version: &str) -> Result<()> {
    println!("📦 Upgrading via Cargo...");
    
    if version == "latest" || version == CURRENT_VERSION {
        // Install latest from crates.io
        let status = Command::new("cargo")
            .args(["install", "cis", "--force"])
            .status()
            .map_err(|e| anyhow!("Failed to run cargo install: {}", e))?;
        
        if status.success() {
            println!("\n✅ Successfully upgraded to latest version");
            println!("   Run `cis --version` to verify");
            Ok(())
        } else {
            Err(anyhow!("Cargo install failed"))
        }
    } else {
        // Install specific version
        let status = Command::new("cargo")
            .args(["install", "cis", "--version", version, "--force"])
            .status()
            .map_err(|e| anyhow!("Failed to run cargo install: {}", e))?;
        
        if status.success() {
            println!("\n✅ Successfully upgraded to version {}", version);
            Ok(())
        } else {
            Err(anyhow!("Cargo install failed"))
        }
    }
}

/// Upgrade via Homebrew
async fn upgrade_via_homebrew(version: &str) -> Result<()> {
    println!("📦 Upgrading via Homebrew...");
    
    if version == "latest" || version == CURRENT_VERSION {
        // Upgrade to latest
        let status = Command::new("brew")
            .args(["upgrade", "cis"])
            .status()
            .map_err(|e| anyhow!("Failed to run brew upgrade: {}", e))?;
        
        if status.success() {
            println!("\n✅ Successfully upgraded to latest version");
            println!("   Run `cis --version` to verify");
            Ok(())
        } else {
            Err(anyhow!("Brew upgrade failed"))
        }
    } else {
        // Homebrew doesn't support specific versions easily
        println!("⚠️  Homebrew doesn't support installing specific versions.");
        println!("   To upgrade to {}:", version);
        println!("   1. Uninstall: `brew uninstall cis`");
        println!("   2. Download from: https://github.com/{}/releases/tag/v{}", 
            GITHUB_REPO, version);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        let version = "v1.2.3";
        assert_eq!(version.trim_start_matches('v'), "1.2.3");
    }
}
