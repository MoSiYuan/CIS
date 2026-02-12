//! # Command Registry
//!
//! Central registry for all CLI commands with routing and help generation.

use std::collections::HashMap;
use std::fmt;

use crate::cli::command::{Command, CommandCategory};

/// Command group definition
#[derive(Debug, Clone)]
pub struct CommandGroup {
    pub name: &'static str,
    pub about: &'static str,
    pub category: CommandCategory,
}

impl CommandGroup {
    /// Create a new command group
    pub const fn new(name: &'static str, about: &'static str, category: CommandCategory) -> Self {
        Self {
            name,
            about,
            category,
        }
    }
}

/// Command registry
pub struct CommandRegistry {
    groups: Vec<CommandGroup>,
    commands: HashMap<String, Box<dyn Command>>,
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    /// Create a new command registry
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    /// Register a command group
    pub fn register_group(&mut self, group: CommandGroup) -> &mut Self {
        self.groups.push(group);
        self
    }

    /// Register a command
    pub fn register(&mut self, name: &'static str, command: Box<dyn Command>) -> &mut Self {
        self.commands.insert(name.to_string(), command);
        self
    }

    /// Register an alias for a command
    pub fn register_alias(&mut self, alias: &'static str, target: &'static str) -> &mut Self {
        self.aliases.insert(alias.to_string(), target.to_string());
        self
    }

    /// Resolve an alias to its target command
    pub fn resolve_alias(&self, name: &str) -> &str {
        self.aliases.get(name).map(|s| s.as_str()).unwrap_or(name)
    }

    /// Get a command by name (resolving aliases)
    pub fn get(&self, name: &str) -> Option<&dyn Command> {
        let resolved = self.resolve_alias(name);
        self.commands.get(resolved).map(|b| b.as_ref())
    }

    /// Check if a command exists
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Get all command names
    pub fn command_names(&self) -> Vec<&str> {
        self.commands.keys().map(|k| k.as_str()).collect()
    }

    /// Generate help text for all commands
    pub fn generate_help(&self) -> String {
        let mut help = String::from("CIS - Cluster of Independent Systems\n\n");
        help.push_str("Usage: cis [OPTIONS] <COMMAND>\n\n");

        // Command groups
        help.push_str("Command Groups:\n");
        for group in &self.groups {
            help.push_str(&format!("  {:<15} {}\n", group.name, group.about));
        }

        // All commands grouped by category
        help.push_str("\nCommands:\n");

        let mut categorized: HashMap<CommandCategory, Vec<&str>> = HashMap::new();
        for (name, cmd) in &self.commands {
            let category = cmd.category();
            categorized.entry(category).or_default().push(name);
        }

        let categories_order = vec![
            CommandCategory::Core,
            CommandCategory::Memory,
            CommandCategory::Skill,
            CommandCategory::Agent,
            CommandCategory::Workflow,
            CommandCategory::Network,
            CommandCategory::System,
            CommandCategory::Advanced,
            CommandCategory::Other,
        ];

        for category in categories_order {
            if let Some(commands) = categorized.get(&category) {
                help.push_str(&format!("\n  {}:\n", category.display_name()));
                for cmd in commands {
                    if let Some(command_impl) = self.get(cmd) {
                        help.push_str(&format!("    {:<30} {}\n", cmd, command_impl.about()));
                    }
                }
            }
        }

        help.push_str("\nOptions:\n");
        help.push_str("  -h, --help       Show help\n");
        help.push_str("  -V, --version    Show version\n");
        help.push_str("  -v, --verbose    Verbose output\n");
        help.push_str("  --json           Output in JSON format\n");

        help
    }

    /// Generate examples for a command
    pub fn generate_examples(&self, name: &str) -> String {
        if let Some(command) = self.get(name) {
            let examples = command.examples();
            if examples.is_empty() {
                return format!("No examples available for '{}'", name);
            }

            let mut output = format!("Examples for '{}':\n\n", name);
            for example in &examples {
                output.push_str(&format!("  {}\n", example.command));
                output.push_str(&format!("    {}\n\n", example.description));
            }
            output
        } else {
            format!("Unknown command: '{}'", name)
        }
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CommandRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.generate_help())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::command::{CommandCategory, CommandContext, CommandOutput, CommandError};

    struct MockCommand;

    impl Command for MockCommand {
        fn name(&self) -> &'static str {
            "mock"
        }

        fn about(&self) -> &'static str {
            "Mock command for testing"
        }

        fn run(&self, _ctx: &CommandContext) -> Result<CommandOutput, CommandError> {
            Ok(CommandOutput::Success)
        }

        fn category(&self) -> CommandCategory {
            CommandCategory::Core
        }
    }

    #[test]
    fn test_registry() {
        let mut registry = CommandRegistry::new();

        // Register group
        registry.register_group(CommandGroup::new(
            "core",
            "Core commands",
            CommandCategory::Core
        ));

        // Register command
        registry.register("mock", Box::new(MockCommand));

        // Test get
        assert!(registry.contains("mock"));
        let cmd = registry.get("mock");
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().about(), "Mock command for testing");

        // Test names
        let names = registry.command_names();
        assert!(names.contains(&"mock"));
    }

    #[test]
    fn test_alias() {
        let mut registry = CommandRegistry::new();
        registry.register("mock", Box::new(MockCommand));
        registry.register_alias("m", "mock");

        // Test alias resolution
        assert!(registry.contains("m"));
        let cmd = registry.get("m");
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().name(), "mock");
    }

    #[test]
    fn test_generate_help() {
        let mut registry = CommandRegistry::new();

        registry.register_group(CommandGroup::new(
            "core",
            "Core commands",
            CommandCategory::Core
        ));

        registry.register("mock", Box::new(MockCommand));

        let help = registry.generate_help();
        assert!(help.contains("CIS - Cluster of Independent Systems"));
        assert!(help.contains("Core commands"));
        assert!(help.contains("mock"));
        assert!(help.contains("Mock command for testing"));
    }
}
