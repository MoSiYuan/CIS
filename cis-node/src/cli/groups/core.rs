//! # Core Command Group
//!
//! Core functionality: init, status, config, doctor

use clap::{Parser, Subcommand};

use crate::cli::command::{Command, CommandCategory, CommandContext, CommandOutput, CommandError, Example};
use crate::cli::handlers::core;

/// Core commands - initialization, configuration, and diagnostics
#[derive(Parser, Debug)]
pub struct CoreGroup {
    #[command(subcommand)]
    pub action: CoreAction,
}

impl CoreGroup {
    /// Get all examples for core commands
    pub fn examples() -> Vec<Example> {
        vec![
            Example {
                command: "cis core init".to_string(),
                description: "Initialize CIS in the current directory".to_string(),
            },
            Example {
                command: "cis core init --project".to_string(),
                description: "Initialize a new CIS project".to_string(),
            },
            Example {
                command: "cis core status".to_string(),
                description: "Show CIS status and configuration".to_string(),
            },
            Example {
                command: "cis core doctor".to_string(),
                description: "Run diagnostics and check for issues".to_string(),
            },
            Example {
                command: "cis core config set user.name \"John Doe\"".to_string(),
                description: "Set a configuration value".to_string(),
            },
        ]
    }
}

/// Core command actions
#[derive(Subcommand, Debug)]
pub enum CoreAction {
    /// Initialize CIS environment
    Init {
        /// Initialize as a project (not global)
        #[arg(long, short)]
        project: bool,

        /// Force overwrite existing configuration
        #[arg(long)]
        force: bool,

        /// Non-interactive mode (use defaults)
        #[arg(long)]
        non_interactive: bool,

        /// Skip environment checks
        #[arg(long)]
        skip_checks: bool,

        /// Preferred AI provider
        #[arg(long)]
        provider: Option<String>,
    },

    /// Show CIS status and configuration
    Status {
        /// Show detailed path information
        #[arg(long)]
        paths: bool,

        /// Show in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Run diagnostics
    Doctor {
        /// Automatically fix issues
        #[arg(long)]
        fix: bool,

        /// Verbose output
        #[arg(long, short)]
        verbose: bool,
    },

    /// Generate shell completion scripts
    Completion {
        /// Shell type (bash, zsh, fish, powershell, elvish)
        shell: ShellType,
    },
}

/// Configuration management actions
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Get a configuration value
    Get {
        /// Configuration key (e.g., user.name)
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key (e.g., user.name)
        key: String,
        /// Configuration value
        value: String,
    },

    /// List all configuration values
    List {
        /// Show in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Open configuration file in editor
    Edit,
}

/// Shell types for completion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

impl std::fmt::Display for ShellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellType::Bash => write!(f, "bash"),
            ShellType::Zsh => write!(f, "zsh"),
            ShellType::Fish => write!(f, "fish"),
            ShellType::PowerShell => write!(f, "powershell"),
            ShellType::Elvish => write!(f, "elvish"),
        }
    }
}

impl std::str::FromStr for ShellType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bash" => Ok(ShellType::Bash),
            "zsh" => Ok(ShellType::Zsh),
            "fish" => Ok(ShellType::Fish),
            "powershell" | "pwsh" => Ok(ShellType::PowerShell),
            "elvish" => Ok(ShellType::Elvish),
            _ => Err(format!("Unknown shell type: {}", s)),
        }
    }
}

impl Command for CoreAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Init { .. } => "init",
            Self::Status { .. } => "status",
            Self::Config { .. } => "config",
            Self::Doctor { .. } => "doctor",
            Self::Completion { .. } => "completion",
        }
    }

    fn about(&self) -> &'static str {
        match self {
            Self::Init { .. } => "Initialize CIS environment",
            Self::Status { .. } => "Show CIS status and configuration",
            Self::Config { .. } => "Manage CIS configuration",
            Self::Doctor { .. } => "Run diagnostics",
            Self::Completion { .. } => "Generate shell completion scripts",
        }
    }

    fn run(&self, ctx: &CommandContext) -> Result<CommandOutput, CommandError> {
        match self {
            Self::Init { project, force, non_interactive, skip_checks, provider } => {
                core::init::execute(*project, *force, *non_interactive, *skip_checks, provider.clone(), ctx)
            }
            Self::Status { paths, json } => {
                core::status::execute(*paths, *json, ctx)
            }
            Self::Config { action } => {
                core::config::execute(action.clone(), ctx)
            }
            Self::Doctor { fix, verbose } => {
                core::doctor::execute(*fix, *verbose, ctx)
            }
            Self::Completion { shell } => {
                core::completion::execute(*shell, ctx)
            }
        }
    }

    fn examples(&self) -> Vec<Example> {
        CoreGroup::examples()
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Core
    }

    fn requires_init(&self) -> bool {
        !matches!(self, Self::Init { .. })
    }
}
