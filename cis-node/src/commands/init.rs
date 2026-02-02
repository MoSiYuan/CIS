//! # Init Command
//!
//! Initialize CIS environment or project.

use anyhow::Result;
use cis_core::wizard::{quick_init, InitOptions, InitWizard};
use tracing::info;

/// Initialize global CIS environment
pub fn init_global() -> Result<()> {
    info!("Initializing CIS global environment...");
    
    let result = quick_init(false)?;
    
    if result.success {
        println!("✅ CIS global environment initialized successfully!");
        println!("\nGenerated files:");
        for path in &result.config_paths {
            println!("  - {}", path.display());
        }
        
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
    }
    
    Ok(())
}

/// Initialize CIS project in current directory
pub fn init_project() -> Result<()> {
    info!("Initializing CIS project...");
    
    let current_dir = std::env::current_dir()?;
    println!("Initializing project in: {}", current_dir.display());
    
    let result = quick_init(true)?;
    
    if result.success {
        println!("✅ CIS project initialized successfully!");
        println!("\nGenerated files:");
        for path in &result.config_paths {
            println!("  - {}", path.display());
        }
        
        if !result.warnings.is_empty() {
            println!("\n⚠️  Warnings:");
            for warning in &result.warnings {
                println!("  - {}", warning);
            }
        }
    }
    
    Ok(())
}

/// Initialize with custom options
pub fn init_with_options(options: InitOptions) -> Result<()> {
    let wizard = InitWizard::new(options);
    let result = wizard.run()?;
    
    if result.success {
        println!("✅ Initialization completed successfully!");
        println!("\nGenerated files:");
        for path in &result.config_paths {
            println!("  - {}", path.display());
        }
    }
    
    Ok(())
}
