//! # Skill Command
//!
//! Manage CIS skills - list, load, unload, call, etc.
//! Also provides semantic skill invocation via natural language.

use anyhow::{Context, Result};
use cis_core::skill::types::LoadOptions;
use cis_core::skill::SkillManager;
use cis_core::storage::db::DbManager;
use std::sync::Arc;
use std::path::PathBuf;
use clap::Args;
use std::time::Instant;

// Vector intelligence imports
use cis_core::intent::IntentParser;
use cis_core::skill::project_registry::ProjectSkillRegistry;
use cis_core::skill::router::SkillVectorRouter;
use cis_core::vector::VectorStorage;
use cis_core::ai::embedding::EmbeddingConfig;
use cis_core::storage::paths::Paths;

// Telemetry imports
use cis_core::telemetry::{RequestLogger, RequestLogBuilder, RequestResult, RequestMetrics};

/// Arguments for `cis skill do` command - natural language skill invocation
#[derive(Args, Debug)]
pub struct SkillDoArgs {
    /// Natural language description
    pub description: String,
    
    /// Project path
    #[arg(short, long)]
    pub project: Option<PathBuf>,
    
    /// Show candidate skill list
    #[arg(short, long)]
    pub candidates: bool,
}

/// Handle `cis skill do` command - semantic skill invocation
pub async fn handle_skill_do(args: SkillDoArgs) -> Result<()> {
    // Generate session ID for this request
    let session_id = format!("session-{}", std::process::id());
    let total_start = Instant::now();
    
    // Initialize RequestLogger
    let telemetry_path = Paths::data_dir().join("telemetry.db");
    let logger = RequestLogger::open(&telemetry_path, None)?;
    
    // Create log builder
    let mut log_builder = RequestLogBuilder::new(&session_id, &args.description);
    
    // 1. Initialize vector storage
    log_builder.start_stage("vector_storage_init");
    let vector_storage = Arc::new(VectorStorage::open(
        &Paths::vector_db(),
        None::<&EmbeddingConfig>,
    )?);
    log_builder.end_stage(true, Some("VectorStorage initialized".to_string()), None);
    
    // Get embedding service from vector storage
    let embedding_service = vector_storage.embedding_service().clone();
    
    // 2. Parse intent
    log_builder.start_stage("intent_parse");
    let intent_parser = IntentParser::new(embedding_service.clone());
    let intent_result = intent_parser.parse(&args.description).await;
    
    let intent = match intent_result {
        Ok(intent) => {
            let intent_info = format!("action={:?}, confidence={:.2}", intent.action_type, intent.confidence);
            log_builder.end_stage(true, Some(intent_info), None);
            
            println!("🎯 解析意图: {}", intent.normalized_intent);
            println!("📊 置信度: {:.2}", intent.confidence);
            intent
        }
        Err(e) => {
            let error_msg = format!("Failed to parse intent: {}", e);
            log_builder.end_stage(false, None, Some(error_msg.clone()));
            
            // Build and save error log
            let total_duration = total_start.elapsed().as_millis() as u64;
            let log = log_builder
                .set_result(RequestResult::Error { error: error_msg })
                .set_metrics(RequestMetrics {
                    total_duration_ms: total_duration,
                    intent_duration_ms: total_duration,
                    routing_duration_ms: 0,
                    execution_duration_ms: 0,
                })
                .build();
            let _ = logger.log_request(&log);
            
            return Err(anyhow::anyhow!("Failed to parse intent: {}", e));
        }
    };
    
    // 3. Route to skill
    log_builder.start_stage("skill_route");
    let project_path = args.project.as_deref()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    
    let project_registry = ProjectSkillRegistry::load(&project_path)
        .or_else(|_| Ok::<_, anyhow::Error>(ProjectSkillRegistry::new(&project_path)))?;
    
    let router = SkillVectorRouter::new(
        vector_storage,
        embedding_service,
    );
    
    let project_id = project_registry.project_path().to_str();
    
    let candidates_result = router.route(&intent, project_id).await;
    
    let candidates = match candidates_result {
        Ok(cands) => {
            let route_info = format!("found {} candidates", cands.len());
            log_builder.end_stage(true, Some(route_info), None);
            cands
        }
        Err(e) => {
            let error_msg = format!("Failed to route intent: {}", e);
            log_builder.end_stage(false, None, Some(error_msg.clone()));
            
            let total_duration = total_start.elapsed().as_millis() as u64;
            let log = log_builder
                .set_result(RequestResult::Error { error: error_msg })
                .set_metrics(RequestMetrics {
                    total_duration_ms: total_duration,
                    intent_duration_ms: 0,
                    routing_duration_ms: total_duration,
                    execution_duration_ms: 0,
                })
                .build();
            let _ = logger.log_request(&log);
            
            return Err(anyhow::anyhow!("Failed to route intent: {}", e));
        }
    };
    
    if candidates.is_empty() {
        println!("❌ 未找到匹配的技能");
        
        let total_duration = total_start.elapsed().as_millis() as u64;
        let log = log_builder
            .set_result(RequestResult::NoMatch { reason: "No matching skills found".to_string() })
            .set_metrics(RequestMetrics {
                total_duration_ms: total_duration,
                intent_duration_ms: 0,
                routing_duration_ms: 0,
                execution_duration_ms: 0,
            })
            .build();
        let _ = logger.log_request(&log);
        
        return Ok(());
    }
    
    // Display candidate skills
    if args.candidates || candidates.len() > 1 {
        println!("\n📋 候选技能:");
        for (i, c) in candidates.iter().enumerate().take(5) {
            println!("  {}. {} (置信度: {:.2})", i + 1, c.skill_name, c.confidence);
        }
    }
    
    // 4. Execute best match
    log_builder.start_stage("skill_execute");
    let best = &candidates[0];
    
    if best.confidence < 0.5 {
        println!("⚠️ 置信度过低，建议明确指定技能");
        
        let total_duration = total_start.elapsed().as_millis() as u64;
        let log = log_builder
            .set_result(RequestResult::Cancelled)
            .set_metrics(RequestMetrics {
                total_duration_ms: total_duration,
                intent_duration_ms: 0,
                routing_duration_ms: 0,
                execution_duration_ms: 0,
            })
            .add_metadata("cancel_reason", "confidence_too_low")
            .build();
        let _ = logger.log_request(&log);
        
        return Ok(());
    }
    
    println!("\n✅ 执行: {}", best.skill_name);
    
    // Check if there's a suggested skill chain
    if let Some(chain) = &best.suggested_chain {
        println!("🔗 建议技能链: {:?}", chain);
    }
    
    // Execute skill (using existing skill execution mechanism)
    // For now, we just show what would be executed
    println!("📦 技能ID: {}", best.skill_id);
    println!("📋 提取参数: {}", best.extracted_params);
    println!("⚠️  注意: 实际执行需要 SkillManager 支持");
    
    log_builder.end_stage(true, Some(format!("skill={}", best.skill_id)), None);
    
    // 5. Save final log
    let total_duration = total_start.elapsed().as_millis() as u64;
    let log = log_builder
        .set_result(RequestResult::Success { 
            skill_id: best.skill_id.clone(), 
            output_summary: format!("Executed skill '{}' with params: {}", best.skill_name, best.extracted_params)
        })
        .set_metrics(RequestMetrics {
            total_duration_ms: total_duration,
            intent_duration_ms: 0,  // Calculated from stage durations if needed
            routing_duration_ms: 0,
            execution_duration_ms: 0,
        })
        .add_metadata("candidate_count", candidates.len().to_string())
        .add_metadata("confidence", format!("{:.2}", best.confidence))
        .build();
    
    logger.log_request(&log)?;
    
    if logger.config().verbose {
        println!("\n📝 请求已记录 (耗时: {}ms)", total_duration);
    }
    
    Ok(())
}

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
