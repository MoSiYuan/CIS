//! # Skill Command
//!
//! Manage CIS skills - list, load, unload, call, etc.

use anyhow::{Context, Result};
use cis_core::skill::types::LoadOptions;
use cis_core::skill::SkillManager;
use cis_core::storage::db::DbManager;
use std::sync::Arc;

/// List all registered skills
pub fn list_skills() -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    let skills = manager.list_all()?;
    
    if skills.is_empty() {
        println!("No skills registered.");
        return Ok(());
    }
    
    println!("Registered Skills:");
    println!("{:<20} {:<12} {:<10} {}", "Name", "Version", "State", "Description");
    println!("{}", "-".repeat(80));
    
    for skill in skills {
        let state_str = if manager.is_active(&skill.meta.name)? {
            "active"
        } else if manager.is_loaded(&skill.meta.name)? {
            "loaded"
        } else {
            "inactive"
        };
        
        println!(
            "{:<20} {:<12} {:<10} {}",
            skill.meta.name,
            skill.meta.version,
            state_str,
            &skill.meta.description
        );
    }
    
    Ok(())
}

/// Load a skill by name
pub fn load_skill(name: &str, activate: bool) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Loading skill '{}'...", name);
    
    let options = LoadOptions {
        auto_activate: activate,
        force_reload: false,
        config: None,
    };
    
    manager.load(name, options)
        .with_context(|| format!("Failed to load skill '{}'", name))?;
    
    if activate {
        println!("✅ Skill '{}' loaded and activated.", name);
    } else {
        println!("✅ Skill '{}' loaded. Use 'cis skill activate {}' to activate.", name, name);
    }
    
    Ok(())
}

/// Unload a skill by name
pub fn unload_skill(name: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Unloading skill '{}'...", name);
    
    manager.unload(name)
        .with_context(|| format!("Failed to unload skill '{}'", name))?;
    
    println!("✅ Skill '{}' unloaded.", name);
    
    Ok(())
}

/// Activate a skill
pub fn activate_skill(name: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Activating skill '{}'...", name);
    
    manager.activate(name)
        .with_context(|| format!("Failed to activate skill '{}'", name))?;
    
    println!("✅ Skill '{}' activated.", name);
    
    Ok(())
}

/// Deactivate a skill
pub fn deactivate_skill(name: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Deactivating skill '{}'...", name);
    
    manager.deactivate(name)
        .with_context(|| format!("Failed to deactivate skill '{}'", name))?;
    
    println!("✅ Skill '{}' deactivated.", name);
    
    Ok(())
}

/// Get skill info
pub fn skill_info(name: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    let info = manager.get_info(name)?
        .with_context(|| format!("Skill '{}' not found", name))?;
    
    println!("Skill Information:");
    println!("  Name:        {}", info.meta.name);
    println!("  Version:     {}", info.meta.version);
    println!("  Type:        {:?}", info.meta.skill_type);
    println!("  Author:      {}", info.meta.author);
    println!("  Description: {}", &info.meta.description);
    println!("  Path:        {}", info.meta.path);
    
    let state_str = if manager.is_active(&info.meta.name)? {
        "active"
    } else if manager.is_loaded(&info.meta.name)? {
        "loaded"
    } else {
        "inactive"
    };
    println!("  State:       {}", state_str);
    
    if !info.meta.permissions.is_empty() {
        println!("  Permissions: {}", info.meta.permissions.join(", "));
    }
    
    if !info.meta.subscriptions.is_empty() {
        println!("  Subscriptions: {}", info.meta.subscriptions.join(", "));
    }
    
    Ok(())
}

/// Call a skill method
pub fn call_skill(name: &str, method: &str, args: Option<&str>) -> Result<()> {
    println!("Calling skill '{}' method '{}'...", name, method);
    
    if let Some(args) = args {
        println!("Arguments: {}", args);
    }
    
    // TODO: Implement actual skill method invocation
    // This would require the skill to expose callable methods via a defined interface
    println!("⚠️  Skill method calling is not yet fully implemented.");
    
    Ok(())
}

/// Install a skill from path
pub fn install_skill(path: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    let path = std::path::Path::new(path);
    
    println!("Installing skill from '{}'...", path.display());
    
    // Detect skill type from path
    let skill_type = if path.extension().map_or(false, |ext| ext == "wasm") {
        cis_core::skill::types::SkillType::Wasm
    } else {
        cis_core::skill::types::SkillType::Native
    };
    
    let meta = manager.install(path, skill_type)
        .with_context(|| format!("Failed to install skill from '{}'", path.display()))?;
    
    println!("✅ Skill '{}' installed successfully.", meta.name);
    println!("   Use 'cis skill load {}' to load it.", meta.name);
    
    Ok(())
}

/// Remove a skill
pub fn remove_skill(name: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Removing skill '{}'...", name);
    
    manager.remove(name)
        .with_context(|| format!("Failed to remove skill '{}'", name))?;
    
    println!("✅ Skill '{}' removed.", name);
    
    Ok(())
}
