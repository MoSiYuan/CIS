//! # Memory Command
//!
//! Memory operations - get, set, search, etc.
//! Supports both keyword-based and semantic vector search.

use anyhow::{Context, Result};
use cis_core::memory::MemoryService;
use cis_core::types::{MemoryCategory, MemoryDomain};
use cis_core::vector::{VectorStorage, MemoryResult};
use cis_core::storage::paths::Paths;
use clap::{Args, ValueEnum};

/// Output format for search results
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Plain text output (default)
    Plain,
    /// JSON format
    Json,
    /// Table format
    Table,
}

/// Arguments for `cis memory search` command - semantic vector search
#[derive(Args, Debug)]
pub struct MemorySearchArgs {
    /// Search query
    pub query: String,
    
    /// Maximum number of results
    #[arg(short, long, default_value = "5")]
    pub limit: usize,
    
    /// Similarity threshold
    #[arg(short, long)]
    pub threshold: Option<f32>,
    
    /// Category filter
    #[arg(short, long)]
    pub category: Option<String>,
    
    /// Output format
    #[arg(short, long, value_enum, default_value = "plain")]
    pub format: OutputFormat,
}

/// Handle `cis memory search` command - semantic vector search
pub async fn handle_memory_search(args: MemorySearchArgs) -> Result<()> {
    let storage = VectorStorage::open(
        &Paths::vector_db(),
        None::<&cis_core::ai::embedding::EmbeddingConfig>,
    )?;
    
    let results = if let Some(category) = args.category {
        // Search by category
        storage.search_memory_by_category(&args.query, &category, args.limit).await
            .map_err(|e| anyhow::anyhow!("Search failed: {}", e))?
    } else {
        // General semantic search
        storage.search_memory(&args.query, args.limit, args.threshold).await
            .map_err(|e| anyhow::anyhow!("Search failed: {}", e))?
    };
    
    // Format and output results based on format argument
    match args.format {
        OutputFormat::Json => output_json(&results, &args.query),
        OutputFormat::Table => output_table(&results, &args.query).await,
        OutputFormat::Plain => output_plain(&results, &args.query).await,
    }
}

/// Output results in JSON format
fn output_json(results: &[MemoryResult], query: &str) -> Result<()> {
    let output = serde_json::json!({
        "query": query,
        "count": results.len(),
        "results": results.iter().map(|r| {
            serde_json::json!({
                "memory_id": r.memory_id,
                "key": r.key,
                "category": r.category,
                "similarity": r.similarity,
            })
        }).collect::<Vec<_>>()
    });
    
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// Output results in table format
async fn output_table(results: &[MemoryResult], query: &str) -> Result<()> {
    println!("🔍 搜索: {} (找到 {} 条结果)\n", query, results.len());
    
    if results.is_empty() {
        println!("❌ 未找到相关记忆");
        return Ok(());
    }
    
    // Print table header
    println!("{:<4} {:<30} {:<15} {:>10}", "No.", "Key", "Category", "Similarity");
    println!("{}", "-".repeat(65));
    
    // Print rows
    for (i, r) in results.iter().enumerate() {
        let key = if r.key.len() > 28 {
            format!("{}...", &r.key[..25])
        } else {
            r.key.clone()
        };
        
        let category = r.category.as_deref().unwrap_or("general");
        let category = if category.len() > 13 {
            format!("{}...", &category[..10])
        } else {
            category.to_string()
        };
        
        println!(
            "{:<4} {:<30} {:<15} {:>9.1}%",
            i + 1,
            key,
            category,
            r.similarity * 100.0
        );
    }
    
    println!();
    Ok(())
}

/// Output results in plain format (default)
async fn output_plain(results: &[MemoryResult], query: &str) -> Result<()> {
    println!("🔍 搜索: {}", query);
    
    if results.is_empty() {
        println!("❌ 未找到相关记忆");
        return Ok(());
    }
    
    println!("\n📊 找到 {} 条相关记忆:\n", results.len());
    
    // Get the actual memory values
    let node_id = format!("node-{}", uuid::Uuid::new_v4());
    let service = MemoryService::open_default(node_id)?;
    
    for (i, r) in results.iter().enumerate() {
        // Get the actual memory value
        let content = if let Ok(Some(entry)) = service.get(&r.key) {
            String::from_utf8_lossy(&entry.value).to_string()
        } else {
            format!("<key: {}>", r.key)
        };
        
        let preview = if content.len() > 100 {
            format!("{}...", &content[..100])
        } else {
            content.to_string()
        };
        
        println!(
            "{}. [{}] {:.2}%",
            i + 1,
            r.category.as_deref().unwrap_or("general"),
            r.similarity * 100.0
        );
        println!("   {}\n", preview);
    }
    
    Ok(())
}

/// Get a memory entry by key
pub fn get_memory(key: &str) -> Result<()> {
    let node_id = format!("node-{}", uuid::Uuid::new_v4());
    let service = MemoryService::open_default(node_id)?;
    
    match service.get(key)? {
        Some(entry) => {
            println!("Memory Entry: {}", key);
            println!("  Domain:    {:?}", entry.domain);
            println!("  Category:  {:?}", entry.category);
            println!("  Created:   {}", entry.created_at.to_rfc3339());
            println!("  Updated:   {}", entry.updated_at.to_rfc3339());
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
    let node_id = format!("node-{}", uuid::Uuid::new_v4());
    let service = MemoryService::open_default(node_id)?;
    
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
    let node_id = format!("node-{}", uuid::Uuid::new_v4());
    let service = MemoryService::open_default(node_id)?;
    
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

/// Search memory entries (keyword-based)
pub async fn search_memory(query: &str, limit: Option<usize>) -> Result<()> {
    let node_id = format!("node-{}", uuid::Uuid::new_v4());
    let service = MemoryService::open_default(node_id)?;
    
    let options = cis_core::memory::SearchOptions {
        limit: limit.unwrap_or(100),
        ..Default::default()
    };
    
    let results = service.search(query, options).await?;
    
    if results.is_empty() {
        println!("No memory entries found matching query: {}", query);
        return Ok(());
    }
    
    println!("Found {} memory entries matching '{}':", results.len(), query);
    println!("{:<30} {:<10} {:<12} {}", "Key", "Domain", "Category", "Updated");
    println!("{}", "-".repeat(90));
    
    for entry in results {
        let updated = entry.updated_at.format("%Y-%m-%d %H:%M").to_string();
        
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
    let node_id = format!("node-{}", uuid::Uuid::new_v4());
    let service = MemoryService::open_default(node_id)?;
    
    let keys = service.list_keys(domain)?;
    
    // Filter by prefix if provided
    let keys: Vec<String> = if let Some(prefix) = prefix {
        keys.into_iter().filter(|k| k.starts_with(prefix)).collect()
    } else {
        keys
    };
    
    if keys.is_empty() {
        if prefix.is_none() || prefix == Some("") {
            println!("No memory entries found.");
        } else {
            println!("No memory entries found with prefix: {}", prefix.unwrap_or(""));
        }
        return Ok(());
    }
    
    let domain_str = domain.map_or_else(
        || "all domains".to_string(),
        |d| format!("{:?} domain", d).to_lowercase()
    );
    
    if prefix.is_none() || prefix == Some("") {
        println!("Memory keys ({}):", domain_str);
    } else {
        println!("Memory keys with prefix '{}' ({}):", prefix.unwrap_or(""), domain_str);
    }
    
    for key in keys {
        println!("  - {}", key);
    }
    
    Ok(())
}

/// Export public memory
pub fn export_memory(since: Option<i64>, output: Option<&str>) -> Result<()> {
    let node_id = format!("node-{}", uuid::Uuid::new_v4());
    let service = MemoryService::open_default(node_id)?;
    
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
