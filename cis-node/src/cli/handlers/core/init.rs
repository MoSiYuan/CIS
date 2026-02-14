//! # Init Command Handler
//!
//! Initialize CIS environment

use crate::cli::{CommandContext, CommandOutput, CommandError};
use cis_core::storage::paths::Paths;
use colored::*;

/// Execute init command
pub fn execute(
    project: bool,
    force: bool,
    _non_interactive: bool,
    _skip_checks: bool,
    _provider: Option<String>,
    ctx: &CommandContext,
) -> Result<CommandOutput, CommandError> {
    ctx.verbose(&format!("Init called: project={}, force={}", project, force));

    // Check if already initialized
    if Paths::config_file().exists() && !force {
        return Err(CommandError::new("CIS already initialized")
            .with_suggestion("Use --force to reinitialize")
            .with_suggestion("Or run 'cis core status' to check current state"));
    }

    // Initialize
    if let Err(e) = init_cis(project, ctx) {
        return Err(CommandError::new(format!("Initialization failed: {}", e))
            .with_suggestion("Run 'cis core doctor' to diagnose issues")
            .with_suggestion("Check file permissions"));
    }

    Ok(CommandOutput::Message(
        format!("{} CIS initialized successfully", "✓".green())
    ))
}

/// Initialize CIS
fn init_cis(project: bool, ctx: &CommandContext) -> anyhow::Result<()> {
    use std::fs;

    // Create directories
    let base_dir = if project {
        std::env::current_dir()?
    } else {
        Paths::data_dir()
    };

    let cis_dir = base_dir.join(".cis");
    fs::create_dir_all(&cis_dir)?;

    // Create default config
    let config_file = if project {
        cis_dir.join("project.toml")
    } else {
        Paths::config_file()
    };

    if !config_file.exists() {
        let config = if project {
            generate_project_config()
        } else {
            generate_global_config()
        };
        fs::write(&config_file, config)?;
    }

    Ok(())
}

/// Generate project configuration
fn generate_project_config() -> String {
    r#"[project]
name = "my-project"
id = "proj-xxx"

[ai]
provider = "claude"
guide = "You are working on my-project"

[memory]
namespace = "project/my-project"
"#.to_string()
}

/// Generate global configuration
fn generate_global_config() -> String {
    r#"[node]
name = "my-cis-node"

[ai]
provider = "claude"
model = "claude-3-sonnet"

[p2p]
enabled = true
listen_port = 7677
"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_project_config() {
        let config = generate_project_config();
        assert!(config.contains("[project]"));
        assert!(config.contains("name = \"my-project\""));
    }

    #[test]
    fn test_generate_global_config() {
        let config = generate_global_config();
        assert!(config.contains("[node]"));
        assert!(config.contains("[p2p]"));
    }
}
