//! # Memory Command
//!
//! Memory operations - get, set, search, etc.

use anyhow::{Context, Result};
use cis_core::memory::MemoryService;
use cis_core::storage::db::DbManager;
use cis_core::types::{MemoryCategory, MemoryDomain};


/// Get a memory entry by key
pub fn get_memory(key: &str) -> Result<()> {
    let db_manager = DbManager::new()?;
    let core_db = db_manager.core();
    let service = MemoryService::new(core_db);
    
    match service.get(key)? {
        Some(entry) => {
            println!("Memory Entry: {}", key);
            println!("  Domain:    {:?}", entry.domain);
            println!("  Category:  {:?}", entry.category);
            println!("  Created:   {}", chrono::DateTime::from_timestamp(entry.created_at, 0)
                .map_or_else(|| "Unknown".to_string(), |dt| dt.to_rfc3339()));
            println!("  Updated:   {}", chrono::DateTime::from_timestamp(entry.updated_at, 0)
                .map_or_else(|| "Unknown".to_string(), |dt| dt.to_rfc3339()));
            println!("  Version:   {}", entry.version);
            println!("  Encrypted: {}", entry.encrypted);
            
            // Try to display value as string
            match String::from_utf8(entry.value.clone()) {
                Ok(s) => println!("  Value:     {}", s),
                Err(_) => println!("  Value:     <binary data, {} bytes>", entry.value.len()),
            }
        }
        None => {
            println!("No memory found for key: {}", key);
        }
    }
    
    Ok(())
}

/// Set a memory entry
pub fn set_memory(key: &str, value: &str, domain: MemoryDomain, category: MemoryCategory) -> Result<()> {
    let db_manager = DbManager::new()?;
    let core_db = db_manager.core();
    let service = MemoryService::new(core_db);
    
    service.set(key, value.as_bytes(), domain, category)
        .with_context(|| format!("Failed to set memory for key '{}'", key))?;
    
    let domain_str = match domain {
        MemoryDomain::Private => "private",
        MemoryDomain::Public => "public",
    };
    
    println!("✅ Memory set: {} (domain: {})", key, domain_str);
    
    Ok(())
}

/// Delete a memory entry
pub fn delete_memory(key: &str) -> Result<()> {
    let db_manager = DbManager::new()?;
    let core_db = db_manager.core();
    let service = MemoryService::new(core_db);
    
    match service.delete(key)? {
        true => {
            println!("✅ Memory deleted: {}", key);
        }
        false => {
            println!("⚠️  No memory found for key: {}", key);
        }
    }
    
    Ok(())
}

/// Search memory entries
pub fn search_memory(query: &str, limit: Option<usize>) -> Result<()> {
    let db_manager = DbManager::new()?;
    let core_db = db_manager.core();
    let service = MemoryService::new(core_db);
    
    let options = cis_core::memory::SearchOptions {
        limit,
        ..Default::default()
    };
    
    let results = service.search(query, options)?;
    
    if results.is_empty() {
        println!("No memory entries found matching query: {}", query);
        return Ok(());
    }
    
    println!("Found {} memory entries matching '{}':", results.len(), query);
    println!("{:<30} {:<10} {:<12} {}", "Key", "Domain", "Category", "Updated");
    println!("{}", "-".repeat(90));
    
    for entry in results {
        let updated = chrono::DateTime::from_timestamp(entry.updated_at, 0)
            .map_or_else(|| "Unknown".to_string(), |dt| dt.format("%Y-%m-%d %H:%M").to_string());
        
        println!(
            "{:<30} {:<10} {:<12} {}",
            entry.key,
            format!("{:?}", entry.domain).to_lowercase(),
            format!("{:?}", entry.category).to_lowercase(),
            updated
        );
    }
    
    Ok(())
}

/// List memory keys with optional prefix
pub fn list_memory(prefix: Option<&str>, domain: Option<MemoryDomain>) -> Result<()> {
    let db_manager = DbManager::new()?;
    let core_db = db_manager.core();
    let service = MemoryService::new(core_db);
    
    let prefix = prefix.unwrap_or("");
    let keys = service.list_keys(prefix, domain)?;
    
    if keys.is_empty() {
        if prefix.is_empty() {
            println!("No memory entries found.");
        } else {
            println!("No memory entries found with prefix: {}", prefix);
        }
        return Ok(());
    }
    
    let domain_str = domain.map_or_else(
        || "all domains".to_string(),
        |d| format!("{:?} domain", d).to_lowercase()
    );
    
    if prefix.is_empty() {
        println!("Memory keys ({}):", domain_str);
    } else {
        println!("Memory keys with prefix '{}' ({}):", prefix, domain_str);
    }
    
    for key in keys {
        println!("  - {}", key);
    }
    
    Ok(())
}

/// Export public memory
pub fn export_memory(since: Option<i64>, output: Option<&str>) -> Result<()> {
    let db_manager = DbManager::new()?;
    let core_db = db_manager.core();
    let service = MemoryService::new(core_db);
    
    let since = since.unwrap_or(0);
    let entries = service.export_public(since)?;
    
    if entries.is_empty() {
        println!("No public memory entries to export since timestamp {}.", since);
        return Ok(());
    }
    
    let export_data: Vec<_> = entries.iter().map(|e| {
        serde_json::json!({
            "key": e.key,
            "value": String::from_utf8_lossy(&e.value),
            "category": format!("{:?}", e.category),
            "created_at": e.created_at,
            "updated_at": e.updated_at,
        })
    }).collect();
    
    let json = serde_json::to_string_pretty(&export_data)?;
    
    match output {
        Some(path) => {
            std::fs::write(path, json)
                .with_context(|| format!("Failed to write to {}", path))?;
            println!("✅ Exported {} memory entries to {}", entries.len(), path);
        }
        None => {
            println!("Exported {} memory entries:", entries.len());
            println!("{}", json);
        }
    }
    
    Ok(())
}
