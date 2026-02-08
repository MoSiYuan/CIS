//! # Init Command
//!
//! Initialize CIS environment or project with interactive wizard.

use anyhow::Result;
use cis_core::init::{InitWizard, WizardResult};
use tracing::info;

/// Initialize global CIS environment with full wizard
pub async fn init_global() -> Result<()> {
    info!("Initializing CIS global environment...");

    let wizard = InitWizard::new();
    let result = wizard.run(false).await?;

    display_result(&result);

    Ok(())
}

/// Initialize CIS project in current directory with full wizard
pub async fn init_project() -> Result<()> {
    info!("Initializing CIS project...");

    let current_dir = std::env::current_dir()?;
    println!("Initializing project in: {}\n", current_dir.display());

    let wizard = InitWizard::new();
    let result = wizard.run(true).await?;

    display_result(&result);

    Ok(())
}

/// Initialize with custom options
pub async fn init_with_options(options: InitOptions) -> Result<()> {
    let mut wizard = InitWizard::new();

    if options.non_interactive {
        wizard = InitWizard::non_interactive();
    }

    if options.skip_checks {
        wizard = wizard.skip_checks();
    }

    if options.force {
        wizard = wizard.with_force();
    }

    if let Some(provider) = options.preferred_provider {
        wizard = wizard.with_provider(provider);
    }

    let result = wizard.run(options.project_mode).await?;

    display_result(&result);

    Ok(())
}

/// Display wizard result
fn display_result(result: &WizardResult) {
    if result.config_created || result.project_initialized {
        println!("\n✅ 初始化完成！");

        if !result.messages.is_empty() {
            println!("\n📁 生成的文件:");
            for msg in &result.messages {
                println!("   • {}", msg);
            }
        }

        if result.tests_passed {
            println!("\n✅ 所有验证测试通过！");
        } else {
            println!("\n⚠️  部分验证测试未通过。");
        }
    } else {
        println!("\n⚠️  初始化未完成。");
    }
}

/// Initialize command options
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct InitOptions {
    /// Initialize project instead of global
    pub project_mode: bool,
    /// Project directory (only valid in project_mode)
    pub project_dir: Option<std::path::PathBuf>,
    /// Skip environment checks
    pub skip_checks: bool,
    /// Force overwrite existing configuration
    pub force: bool,
    /// Selected AI Provider
    pub preferred_provider: Option<String>,
    /// Non-interactive mode
    pub non_interactive: bool,
}


/// Quick initialization with default settings (non-interactive)
pub async fn quick_init(project_mode: bool) -> Result<()> {
    use cis_core::init::init_non_interactive;

    let result = init_non_interactive(project_mode, false).await?;
    display_result(&result);

    Ok(())
}
