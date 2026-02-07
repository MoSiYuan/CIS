//! # Schema Command
//!
//! CLI Schema self-description for AI integration.
//! Provides structured command metadata for AI agents to discover CLI capabilities.

use cis_core::cli::schema::build_cis_schema;

/// Handle schema command
pub async fn handle(format: String, compositions: bool) -> anyhow::Result<()> {
    let registry = build_cis_schema();

    match format.as_str() {
        "json" => {
            if compositions {
                // 输出包含组合模式的完整 schema
                let schema = registry.to_json_schema();
                println!("{}", serde_json::to_string_pretty(&schema)?);
            } else {
                // 仅输出命令列表
                let commands: Vec<_> = registry
                    .commands
                    .iter()
                    .map(|cmd| {
                        serde_json::json!({
                            "name": cmd.name,
                            "description": cmd.description,
                            "parameters": cmd.parameters,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&commands)?);
            }
        }
        "yaml" | "yml" => {
            if compositions {
                let schema = registry.to_json_schema();
                println!("{}", serde_yaml::to_string(&schema)?);
            } else {
                let commands: Vec<_> = registry
                    .commands
                    .iter()
                    .map(|cmd| {
                        serde_json::json!({
                            "name": cmd.name,
                            "description": cmd.description,
                            "parameters": cmd.parameters,
                        })
                    })
                    .collect();
                println!("{}", serde_yaml::to_string(&commands)?);
            }
        }
        _ => {
            anyhow::bail!("Unsupported format: {}. Use 'json' or 'yaml'", format);
        }
    }

    Ok(())
}

/// Print schema in human-readable format (for --help)
pub fn print_schema_help() {
    println!("CIS CLI Schema Commands:");
    println!();
    println!("  cis schema                    # List all commands as JSON");
    println!("  cis schema --format json      # Same as above");
    println!("  cis schema --format yaml      # List all commands as YAML");
    println!("  cis schema --compositions     # Include pipeline composition patterns");
    println!();
    println!("Exit Codes:");
    println!("  0  Success");
    println!("  1  General error");
    println!("  2  Need confirmation");
    println!("  3  Config missing");
    println!();
    println!("AI Integration:");
    println!("  The schema command provides structured metadata for AI agents to:");
    println!("  - Discover available CLI commands");
    println!("  - Understand parameter requirements");
    println!("  - Compose commands into pipelines");
    println!("  - Handle errors appropriately");
}
