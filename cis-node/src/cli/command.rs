//! # Command Trait
//!
//! Unified interface for all CLI commands.

use clap::Subcommand;
use std::fmt;

/// Command execution context
pub struct CommandContext {
    pub global_opts: GlobalOptions,
    pub config: std::sync::Arc<cis_core::config::GlobalConfig>,
    pub runtime: Option<std::sync::Arc<tokio::runtime::Runtime>>,
}

/// Global command options
#[derive(Debug, Clone)]
pub struct GlobalOptions {
    pub json: bool,
    pub verbose: bool,
    pub quiet: bool,
}

/// Command output
pub enum CommandOutput {
    Success,
    Message(String),
    Data(serde_json::Value),
    Table(Vec<Vec<String>>),
    Multi(Vec<CommandOutput>),
}

impl fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandOutput::Success => write!(f, "Success"),
            CommandOutput::Message(msg) => write!(f, "{}", msg),
            CommandOutput::Data(data) => write!(f, "{}", serde_json::to_string_pretty(data).unwrap()),
            CommandOutput::Table(rows) => {
                for row in rows {
                    writeln!(f, "{}", row.join("\t"))?;
                }
                Ok(())
            }
            CommandOutput::Multi(outputs) => {
                for output in outputs {
                    writeln!(f, "{}", output)?;
                }
                Ok(())
            }
        }
    }
}

/// Command error with suggestions
pub struct CommandError {
    pub message: String,
    pub suggestions: Vec<String>,
    pub exit_code: i32,
    pub source: Option<anyhow::Error>,
}

impl CommandError {
    /// Create a new command error
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            suggestions: Vec::new(),
            exit_code: 1,
            source: None,
        }
    }

    /// Add a suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// Set exit code
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }

    /// Attach source error
    pub fn with_source(mut self, source: anyhow::Error) -> Self {
        self.source = Some(source);
        self
    }

    /// Format error with suggestions
    pub fn format(&self) -> String {
        let mut output = format!("❌ Error: {}", self.message);

        if !self.suggestions.is_empty() {
            output.push_str("\n\nSuggestions:\n");
            for (i, suggestion) in self.suggestions.iter().enumerate() {
                output.push_str(&format!("  {}. {}\n", i + 1, suggestion));
            }
        }

        if let Some(source) = &self.source {
            output.push_str(&format!("\nDetails: {}", source));
        }

        output
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format())
    }
}

impl std::error::Error for CommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &dyn std::error::Error)
    }
}

/// Conversion from anyhow::Error
impl From<anyhow::Error> for CommandError {
    fn from(err: anyhow::Error) -> Self {
        Self::new(err.to_string()).with_source(err)
    }
}

/// Usage example
#[derive(Debug, Clone)]
pub struct Example {
    pub command: String,
    pub description: String,
}

/// Command trait - must be implemented by all commands
pub trait Command: Subcommand {
    /// Command name
    fn name(&self) -> &'static str;

    /// Short description for help text
    fn about(&self) -> &'static str;

    /// Execute the command
    fn run(&self, ctx: &CommandContext) -> Result<CommandOutput, CommandError>;

    /// Usage examples (optional)
    fn examples(&self) -> Vec<Example> {
        Vec::new()
    }

    /// Command category for help grouping
    fn category(&self) -> CommandCategory {
        CommandCategory::Other
    }

    /// Whether this command requires CIS to be initialized
    fn requires_init(&self) -> bool {
        true
    }

    /// Whether this command can run in non-interactive mode
    fn supports_non_interactive(&self) -> bool {
        true
    }
}

/// Command category for grouping in help text
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    Core,
    Memory,
    Skill,
    Agent,
    Workflow,
    Network,
    System,
    Advanced,
    Other,
}

impl CommandCategory {
    /// Display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            CommandCategory::Core => "Core",
            CommandCategory::Memory => "Memory",
            CommandCategory::Skill => "Skill",
            CommandCategory::Agent => "Agent",
            CommandCategory::Workflow => "Workflow",
            CommandCategory::Network => "Network",
            CommandCategory::System => "System",
            CommandCategory::Advanced => "Advanced",
            CommandCategory::Other => "Other",
        }
    }

    /// Description of the category
    pub fn description(&self) -> &'static str {
        match self {
            CommandCategory::Core => "Core functionality (init, config, status)",
            CommandCategory::Memory => "Memory storage and retrieval",
            CommandCategory::Skill => "Skill management and execution",
            CommandCategory::Agent => "AI agent interaction",
            CommandCategory::Workflow => "DAG and task workflow management",
            CommandCategory::Network => "P2P and network management",
            CommandCategory::System => "System utilities and maintenance",
            CommandCategory::Advanced => "Advanced and experimental features",
            CommandCategory::Other => "Other commands",
        }
    }
}
