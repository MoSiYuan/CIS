//! # Configuration Commands
//!
//! View and manage CIS configuration.

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use cis_core::storage::paths::Paths;
use std::fs;

/// Configuration subcommands
#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Show current configuration
    Show {
        /// Show specific section (e.g., 'node', 'p2p', 'agent')
        #[arg(short, long)]
        section: Option<String>,
        /// Show in JSON format
        #[arg(long)]
        json: bool,
        /// Show all values including sensitive ones
        #[arg(long)]
        all: bool,
    },

    /// Set a configuration value
    Set {
        /// Configuration key (e.g., 'node.name', 'p2p.enabled')
        key: String,
        /// Configuration value
        value: String,
    },

    /// Unset a configuration value
    Unset {
        /// Configuration key to unset
        key: String,
    },

    /// List all configuration keys
    List {
        /// Filter by key prefix
        #[arg(short, long)]
        prefix: Option<String>,
    },

    /// Validate configuration
    Validate {
        /// Show detailed validation output
        #[arg(long)]
        verbose: bool,
    },
}

/// Configuration command arguments
#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

/// Handle configuration commands
pub async fn handle_config(args: ConfigArgs) -> Result<()> {
    match args.action {
        ConfigAction::Show { section, json, all } => {
            show_config(section.as_deref(), json, all).await
        }
        ConfigAction::Set { key, value } => {
            set_config(&key, &value).await
        }
        ConfigAction::Unset { key } => {
            unset_config(&key).await
        }
        ConfigAction::List { prefix } => {
            list_config(prefix.as_deref()).await
        }
        ConfigAction::Validate { verbose } => {
            validate_config(verbose).await
        }
    }
}

/// Show configuration
async fn show_config(section: Option<&str>, json_output: bool, show_all: bool) -> Result<()> {
    let config_path = Paths::config_file();

    if !config_path.exists() {
        println!("❌ Configuration file not found");
        println!("   Run 'cis init' to create a configuration.");
        return Ok(());
    }

    let content = fs::read_to_string(&config_path)
        .context("Failed to read configuration file")?;

    let config: toml::Value = toml::from_str(&content)
        .context("Failed to parse configuration")?;

    if json_output {
        // Output as JSON
        let json_output = if let Some(sec) = section {
            if let Some(value) = config.get(&sec) {
                serde_json::json!(value)
            } else {
                serde_json::json!({"error": "Section not found"})
            }
        } else {
            serde_json::json!(config)
        };
        println!("{}", serde_json::to_string_pretty(&json_output)?);
        return Ok(());
    }

    // Pretty print
    println!("╔════════════════════════════════════════╗");
    println!("║          CIS Configuration             ║");
    println!("╚════════════════════════════════════════╝");
    println!();

    if let Some(sec) = section {
        // Show specific section
        if let Some(value) = config.get(&sec) {
            print_toml_value(&sec, value, 0, show_all);
        } else {
            println!("❌ Section '{}' not found in configuration", sec);
            println!("   Available sections: node, p2p, agent, memory, skill, matrix");
        }
    } else {
        // Show all sections
        if let Some(table) = config.as_table() {
            for (key, value) in table {
                print_toml_value(key, value, 0, show_all);
                println!();
            }
        }
    }

    println!();
    println!("Configuration file: {}", config_path.display());
    println!("Use 'cis config set <key> <value>' to modify values");

    Ok(())
}

/// Set a configuration value
async fn set_config(key: &str, value: &str) -> Result<()> {
    let config_path = Paths::config_file();

    if !config_path.exists() {
        println!("❌ Configuration file not found");
        println!("   Run 'cis init' to create a configuration.");
        return Ok(());
    }

    let content = fs::read_to_string(&config_path)
        .context("Failed to read configuration file")?;

    let mut config: toml::Value = toml::from_str(&content)
        .context("Failed to parse configuration")?;

    // Parse key path (e.g., 'node.name' -> ['node', 'name'])
    let key_parts: Vec<&str> = key.split('.').collect();

    // Try to parse the value
    let parsed_value: toml::Value = if let Ok(b) = value.parse::<bool>() {
        toml::Value::Boolean(b)
    } else if let Ok(n) = value.parse::<i64>() {
        toml::Value::Integer(n)
    } else if let Ok(f) = value.parse::<f64>() {
        toml::Value::Float(f)
    } else {
        toml::Value::String(value.to_string())
    };

    // Set the value
    let mut current = &mut config;
    for (i, part) in key_parts.iter().enumerate() {
        if i == key_parts.len() - 1 {
            // Last part - set the value
            match current {
                toml::Value::Table(table) => {
                    table.insert(part.to_string(), parsed_value);
                }
                _ => {
                    anyhow::bail!("Cannot set value at '{}': parent is not a table", key);
                }
            }
        } else {
            // Navigate to next level
            match current {
                toml::Value::Table(table) => {
                    if !table.contains_key(*part) {
                        table.insert(part.to_string(), toml::Value::Table(toml::value::Table::new()));
                    }
                    current = table.get_mut(*part).unwrap();
                }
                _ => {
                    anyhow::bail!("Cannot navigate to '{}': parent is not a table", key);
                }
            }
        }
    }

    // Write back
    let toml_string = toml::to_string_pretty(&config)
        .context("Failed to serialize configuration")?;

    fs::write(&config_path, toml_string)
        .context("Failed to write configuration file")?;

    println!("✅ Configuration updated:");
    println!("   {} = {}", key, value);

    Ok(())
}

/// Unset a configuration value
async fn unset_config(key: &str) -> Result<()> {
    let config_path = Paths::config_file();

    if !config_path.exists() {
        println!("❌ Configuration file not found");
        return Ok(());
    }

    let content = fs::read_to_string(&config_path)
        .context("Failed to read configuration file")?;

    let mut config: toml::Value = toml::from_str(&content)
        .context("Failed to parse configuration")?;

    // Parse key path
    let key_parts: Vec<&str> = key.split('.').collect();

    // Navigate and remove
    let mut current = &mut config;
    for (i, part) in key_parts.iter().enumerate() {
        if i == key_parts.len() - 1 {
            // Last part - remove the value
            match current {
                toml::Value::Table(table) => {
                    if table.remove(*part).is_some() {
                        println!("✅ Configuration key removed:");
                        println!("   {}", key);
                    } else {
                        println!("⚠️  Key '{}' not found in configuration", key);
                    }
                }
                _ => {
                    anyhow::bail!("Cannot unset '{}': parent is not a table", key);
                }
            }
        } else {
            match current {
                toml::Value::Table(table) => {
                    if let Some(next) = table.get_mut(*part) {
                        current = next;
                    } else {
                        println!("⚠️  Key '{}' not found in configuration", key);
                        return Ok(());
                    }
                }
                _ => {
                    anyhow::bail!("Cannot navigate to '{}': parent is not a table", key);
                }
            }
        }
    }

    // Write back
    let toml_string = toml::to_string_pretty(&config)
        .context("Failed to serialize configuration")?;

    fs::write(&config_path, toml_string)
        .context("Failed to write configuration file")?;

    Ok(())
}

/// List all configuration keys
async fn list_config(prefix: Option<&str>) -> Result<()> {
    let config_path = Paths::config_file();

    if !config_path.exists() {
        println!("❌ Configuration file not found");
        return Ok(());
    }

    let content = fs::read_to_string(&config_path)
        .context("Failed to read configuration file")?;

    let config: toml::Value = toml::from_str(&content)
        .context("Failed to parse configuration")?;

    println!("📋 Configuration Keys");
    println!();

    let mut keys = vec![];

    // Collect all keys
    if let Some(table) = config.as_table() {
        collect_keys(table, String::new(), &mut keys);
    }

    // Filter by prefix
    let filtered: Vec<&String> = if let Some(p) = prefix {
        keys.iter().filter(|k| k.starts_with(p)).collect()
    } else {
        keys.iter().collect()
    };

    if filtered.is_empty() {
        if prefix.is_some() {
            println!("No keys found with prefix: {}", prefix.unwrap());
        } else {
            println!("No configuration keys found.");
        }
        return Ok(());
    }

    // Sort and display
    filtered.sort();
    for key in filtered {
        println!("  {}", key);
    }

    println!();
    println!("Use 'cis config show <key>' to view values");
    println!("Use 'cis config set <key> <value>' to modify values");

    Ok(())
}

/// Validate configuration
async fn validate_config(verbose: bool) -> Result<()> {
    let config_path = Paths::config_file();

    if !config_path.exists() {
        println!("❌ Configuration file not found");
        println!("   Run 'cis init' to create a configuration.");
        return Ok(());
    }

    println!("🔍 Validating configuration...");
    println!("   File: {}", config_path.display());
    println!();

    let content = fs::read_to_string(&config_path)
        .context("Failed to read configuration file")?;

    let mut valid = true;
    let mut checks = vec![];

    // Check 1: Valid TOML
    match toml::from_str::<toml::Value>(&content) {
        Ok(_) => {
            checks.push(("✅", "Valid TOML syntax", ""));
        }
        Err(e) => {
            checks.push(("❌", "Valid TOML syntax", &format!("Parse error: {}", e)));
            valid = false;
        }
    }

    // Check 2: Required sections
    if let Ok(config) = toml::from_str::<toml::Value>(&content) {
        if let Some(table) = config.as_table() {
            // Check for [node] section
            if table.contains_key("node") {
                checks.push(("✅", "[node] section exists", ""));
            } else {
                checks.push(("⚠️ ", "[node] section exists", "Missing (recommended)"));
            }

            // Check for required fields in [node]
            if let Some(node) = table.get("node").and_then(|v| v.as_table()) {
                if node.contains_key("id") {
                    checks.push(("✅", "node.id is set", ""));
                } else {
                    checks.push(("❌", "node.id is set", "Missing (required)"));
                    valid = false;
                }
            }
        }
    }

    // Print validation results
    for (icon, check, note) in &checks {
        if verbose || !note.is_empty() {
            println!("{} {:<40} {}", icon, check, note);
        } else {
            println!("{} {}", icon, check);
        }
    }

    println!();
    if valid {
        println!("✅ Configuration is valid!");
    } else {
        println!("❌ Validation failed. Please fix the errors above.");
        return Err(anyhow::anyhow!("Validation failed"));
    }

    Ok(())
}

/// Collect all keys from a TOML table recursively
fn collect_keys(table: &toml::value::Table, prefix: String, keys: &mut Vec<String>) {
    for (key, value) in table {
        let full_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        match value {
            toml::Value::Table(nested) => {
                collect_keys(nested, full_key.clone(), keys);
            }
            _ => {
                keys.push(full_key);
            }
        }
    }
}

/// Print a TOML value with indentation
fn print_toml_value(key: &str, value: &toml::Value, indent: usize, show_all: bool) {
    let indent_str = "   ".repeat(indent);

    match value {
        toml::Value::String(s) => {
            // Hide sensitive values unless --all is specified
            if is_sensitive_key(key) && !show_all {
                println!("{}{}: *** (hidden)", indent_str, key);
            } else {
                println!("{}{}: {}", indent_str, key, s);
            }
        }
        toml::Value::Integer(n) => {
            println!("{}{}: {}", indent_str, key, n);
        }
        toml::Value::Float(f) => {
            println!("{}{}: {}", indent_str, key, f);
        }
        toml::Value::Boolean(b) => {
            println!("{}{}: {}", indent_str, key, b);
        }
        toml::Value::Array(arr) => {
            println!("{}{}:", indent_str, key);
            for item in arr {
                print!("{}  - ", indent_str);
                match item {
                    toml::Value::String(s) => println!("{}", s),
                    toml::Value::Integer(n) => println!("{}", n),
                    toml::Value::Boolean(b) => println!("{}", b),
                    _ => println!("{:?}", item),
                }
            }
        }
        toml::Value::Table(table) => {
            println!("{}{}:", indent_str, key);
            for (k, v) in table {
                print_toml_value(k, v, indent + 1, show_all);
            }
        }
        _ => {
            println!("{}{}: {:?}", indent_str, key, value);
        }
    }
}

/// Check if a key contains sensitive information
fn is_sensitive_key(key: &str) -> bool {
    let key_lower = key.to_lowercase();
    key_lower.contains("password")
        || key_lower.contains("secret")
        || key_lower.contains("token")
        || key_lower.contains("key")
        || key_lower.contains("credential")
}
