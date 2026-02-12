//! # Project Commands
//!
//! Project initialization and management commands for CIS.

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::fs;
use std::path::Path;
use uuid::Uuid;

/// Project subcommands
#[derive(Subcommand, Debug)]
pub enum ProjectAction {
    /// Initialize a new CIS project in current directory
    Init {
        /// Project name
        #[arg(long, default_value = "")]
        name: String,
        /// Project ID (auto-generated if not provided)
        #[arg(long)]
        id: Option<String>,
        /// Force initialization even if .cis already exists
        #[arg(long)]
        force: bool,
    },

    /// Validate project configuration
    Validate {
        /// Project path (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
        /// Show detailed validation output
        #[arg(long)]
        verbose: bool,
    },

    /// Show project information
    Info {
        /// Project path (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
}

/// Project command arguments
#[derive(Args, Debug)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub action: ProjectAction,
}

/// Handle project commands
pub async fn handle_project(args: ProjectArgs) -> Result<()> {
    match args.action {
        ProjectAction::Init { name, id, force } => {
            init_project(&name, id.as_deref(), force).await
        }
        ProjectAction::Validate { path, verbose } => {
            validate_project(path.as_deref(), verbose).await
        }
        ProjectAction::Info { path } => {
            show_project_info(path.as_deref()).await
        }
    }
}

/// Initialize a new CIS project
async fn init_project(name: &str, project_id: Option<&str>, force: bool) -> Result<()> {
    let current_dir = std::env::current_dir()
        .context("Failed to get current directory")?;

    let cis_dir = current_dir.join(".cis");
    let project_file = cis_dir.join("project.toml");
    let skills_dir = cis_dir.join("skills");

    // Check if already initialized
    if project_file.exists() && !force {
        anyhow::bail!(
            "Project already initialized. Use --force to reinitialize."
        );
    }

    // Create directories
    fs::create_dir_all(&cis_dir)
        .with_context(|| format!("Failed to create .cis directory"))?;
    fs::create_dir_all(&skills_dir)
        .with_context(|| format!("Failed to create .cis/skills directory"))?;

    // Generate project name
    let project_name = if name.is_empty() {
        current_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown-project")
            .to_string()
    } else {
        name.to_string()
    };

    // Generate project ID
    let id = project_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("proj-{}", Uuid::new_v4().to_string()[..8].to_string()));

    // Create project.toml
    let project_config = format!(
        r#"[project]
name = "{}"
id = "{}"

[ai]
provider = "claude"
guide = """
You are working on the {} project.
Tech stack: Detect from project files
Coding standards: Follow project conventions

Check CIS memory for project context before making changes.
"""
model = "claude-sonnet-4-20250514"

# Local Skills
[[skills]]
name = "custom-linter"
path = "./skills/custom-linter"
auto_load = false

[[skills]]
name = "project-deploy"
path = "./skills/project-deploy"
auto_load = false

[memory]
# Memory namespace (default: project/{{project-name}})
namespace = "project/{}"
# Shared memory keys (cross-project access)
shared_keys = ["conventions", "architecture", "api-contracts"]
"#,
        project_name, id, project_name, project_name
    );

    fs::write(&project_file, project_config)
        .with_context(|| format!("Failed to write project.toml"))?;

    // Create README in skills directory
    let readme = skills_dir.join("README.md");
    let readme_content = format!(
        r#"# Local Skills for {project_name}

This directory contains project-specific skills for CIS.

## Creating a New Skill

1. Create a new directory: `mkdir your-skill-name`
2. Create `skill.toml`:
   ```toml
   [skill]
   name = "your-skill-name"
   version = "1.0.0"
   type = "native"
   description = "Your skill description"

   [permissions]
   filesystem = true
   command = true
   ```
3. Create executable script (e.g., `run.sh` or `run.py`)

## Project Skills

Add your project skills here and enable them in `../project.toml`.
"#,
        project_name = project_name
    );

    fs::write(&readme, readme_content)
        .with_context(|| format!("Failed to write README.md"))?;

    println!("✅ CIS project initialized successfully!");
    println!();
    println!("Project Details:");
    println!("  Name:      {}", project_name);
    println!("  ID:        {}", id);
    println!("  Config:    .cis/project.toml");
    println!("  Skills:    .cis/skills/");
    println!("  Namespace: project/{}", project_name);
    println!();
    println!("Next Steps:");
    println!("  1. Edit .cis/project.toml to configure your project");
    println!("  2. Create local skills in .cis/skills/");
    println!("  3. Store project memory:");
    println!("     cis memory set project/{}/tech-stack '...'", project_name);
    println!();
    println!("Useful Commands:");
    println!("  cis project validate  - Validate project configuration");
    println!("  cis project info      - Show project information");
    println!("  cis memory set       - Store project-specific memory");
    println!("  cis dag run          - Run project DAGs");

    Ok(())
}

/// Validate project configuration
async fn validate_project(path: Option<&str>, verbose: bool) -> Result<()> {
    let project_path = if let Some(p) = path {
        Path::new(p).to_path_buf()
    } else {
        std::env::current_dir()?
    };

    let cis_dir = project_path.join(".cis");
    let project_file = cis_dir.join("project.toml");
    let skills_dir = cis_dir.join("skills");

    println!("🔍 Validating CIS project...");
    println!("   Path: {}", project_path.display());
    println!();

    let mut valid = true;
    let mut checks = vec![];

    // Check 1: .cis directory exists
    if cis_dir.exists() {
        checks.push(("✅", ".cis directory exists", ""));
    } else {
        checks.push(("❌", ".cis directory exists", "Not found"));
        valid = false;
    }

    // Check 2: project.toml exists
    if project_file.exists() {
        checks.push(("✅", "project.toml exists", ""));
    } else {
        checks.push(("❌", "project.toml exists", "Not found"));
        valid = false;
    }

    // Check 3: project.toml is valid TOML
    if project_file.exists() {
        match fs::read_to_string(&project_file) {
            Ok(content) => {
                match toml::from_str::<toml::Value>(&content) {
                    Ok(_) => {
                        checks.push(("✅", "project.toml is valid TOML", ""));
                    }
                    Err(e) => {
                        checks.push(("❌", "project.toml is valid TOML", &format!("Parse error: {}", e)));
                        valid = false;
                    }
                }
            }
            Err(e) => {
                checks.push(("❌", "project.toml is readable", &format!("Read error: {}", e)));
                valid = false;
            }
        }
    }

    // Check 4: skills directory exists
    if skills_dir.exists() {
        checks.push(("✅", ".cis/skills directory exists", ""));
    } else {
        checks.push(("⚠️ ", ".cis/skills directory exists", "Not found (optional)"));
    }

    // Check 5: project.toml structure
    if project_file.exists() {
        if let Ok(content) = fs::read_to_string(&project_file) {
            if let Ok(config) = toml::from_str::<toml::Value>(&content) {
                // Check for [project] section
                if config.get("project").is_some() {
                    checks.push(("✅", "[project] section exists", ""));
                } else {
                    checks.push(("❌", "[project] section exists", "Missing"));
                    valid = false;
                }

                // Check for [ai] section
                if config.get("ai").is_some() {
                    checks.push(("✅", "[ai] section exists", ""));
                } else {
                    checks.push(("⚠️ ", "[ai] section exists", "Missing (optional)"));
                }

                // Check for [memory] section
                if config.get("memory").is_some() {
                    checks.push(("✅", "[memory] section exists", ""));
                } else {
                    checks.push(("⚠️ ", "[memory] section exists", "Missing (optional)"));
                }

                // Check for [[skills]] section
                if config.get("skills").is_some() {
                    checks.push(("✅", "[[skills]] section exists", ""));
                } else {
                    checks.push(("⚠️ ", "[[skills]] section exists", "Missing (optional)"));
                }
            }
        }
    }

    // Print validation results
    println!("Validation Results:");
    println!();
    for (icon, check, note) in &checks {
        if verbose || !note.is_empty() {
            println!("{} {:<40} {}", icon, check, note);
        } else {
            println!("{} {}", icon, check);
        }
    }

    println!();
    if valid {
        println!("✅ Project configuration is valid!");
    } else {
        println!("❌ Project validation failed. Please fix the errors above.");
        return Err(anyhow::anyhow!("Validation failed"));
    }

    Ok(())
}

/// Show project information
async fn show_project_info(path: Option<&str>) -> Result<()> {
    let project_path = if let Some(p) = path {
        Path::new(p).to_path_buf()
    } else {
        std::env::current_dir()?
    };

    let cis_dir = project_path.join(".cis");
    let project_file = cis_dir.join("project.toml");

    if !project_file.exists() {
        println!("❌ Not a CIS project (no .cis/project.toml found)");
        println!("   Run 'cis project init' to initialize a project.");
        return Ok(());
    }

    let content = fs::read_to_string(&project_file)
        .context("Failed to read project.toml")?;

    let config: toml::Value = toml::from_str(&content)
        .context("Failed to parse project.toml")?;

    println!("╔════════════════════════════════════════╗");
    println!("║          CIS Project Information         ║");
    println!("╚════════════════════════════════════════╝");
    println!();

    // Project info
    if let Some(project) = config.get("project") {
        println!("📦 Project:");
        if let Some(name) = project.get("name").and_then(|v| v.as_str()) {
            println!("   Name: {}", name);
        }
        if let Some(id) = project.get("id").and_then(|v| v.as_str()) {
            println!("   ID:   {}", id);
        }
    }

    // AI config
    if let Some(ai) = config.get("ai") {
        println!();
        println!("🤖 AI Configuration:");
        if let Some(provider) = ai.get("provider").and_then(|v| v.as_str()) {
            println!("   Provider: {}", provider);
        }
        if let Some(model) = ai.get("model").and_then(|v| v.as_str()) {
            println!("   Model:   {}", model);
        }
        if let Some(guide) = ai.get("guide").and_then(|v| v.as_str()) {
            let guide_preview = if guide.len() > 60 {
                format!("{}...", &guide[..60])
            } else {
                guide.to_string()
            };
            println!("   Guide:   {}", guide_preview);
        }
    }

    // Memory config
    if let Some(memory) = config.get("memory") {
        println!();
        println!("💾 Memory:");
        if let Some(namespace) = memory.get("namespace").and_then(|v| v.as_str()) {
            println!("   Namespace: {}", namespace);
        }
        if let Some(keys) = memory.get("shared_keys").and_then(|v| v.as_array()) {
            let keys: Vec<&str> = keys
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            println!("   Shared Keys:");
            for key in keys {
                println!("     - {}", key);
            }
        }
    }

    // Skills
    if let Some(skills) = config.get("skills").and_then(|v| v.as_array()) {
        println!();
        println!("🔧 Local Skills ({}):", skills.len());
        for skill in skills {
            if let Some(table) = skill.as_table() {
                if let Some(name) = table.get("name").and_then(|v| v.as_str()) {
                    let auto_load = table
                        .get("auto_load")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let path = table
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    println!("   • {}", name);
                    println!("     Path:      {}", path);
                    println!("     Auto Load: {}", if auto_load { "✅" } else { "❌" });
                }
            }
        }
    }

    // Paths
    println!();
    println!("📁 Paths:");
    println!("   Project:   {}", project_path.display());
    println!("   Config:    {}", project_file.display());
    println!("   Skills:    {}", cis_dir.join("skills").display());

    println!();
    println!("Commands:");
    println!("   cis project validate - Validate project configuration");
    println!("   cis memory set       - Store project memory");
    println!("   cis dag run          - Run project DAGs");

    Ok(())
}
