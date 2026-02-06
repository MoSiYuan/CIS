//! # Skill Command
//!
//! Manage CIS skills - list, load, unload, call, etc.
//! Also provides semantic skill invocation via natural language.

use anyhow::{Context, Result};
use cis_core::skill::types::LoadOptions;
use cis_core::skill::router::ResolvedParameters;
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

// Skill chain imports
use cis_core::skill::chain::SkillChain;

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

/// Arguments for `cis skill chain` command - skill chain preview and execution
#[derive(Args, Debug)]
pub struct SkillChainArgs {
    /// Natural language description of the task
    pub description: String,
    
    /// Preview mode - only show the chain without executing
    #[arg(long)]
    pub preview: bool,
    
    /// Show detailed matching information
    #[arg(short, long)]
    pub verbose: bool,
    
    /// Project path
    #[arg(short, long)]
    pub project: Option<PathBuf>,
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
    
    // Initialize SkillManager and DbManager (needed for router)
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = Arc::new(SkillManager::new(db_manager.clone())?);
    
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
    
    // Create router with SkillManager for execution
    let db_manager = Arc::new(DbManager::new()?);
    let router = SkillVectorRouter::new(
        vector_storage,
        embedding_service,
        skill_manager.clone(),
        db_manager,
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
    
    // Execute skill chain
    let params = ResolvedParameters::new(serde_json::json!({
        "prompt": args.description,
        "intent": intent.normalized_intent,
        "entities": intent.entities,
        "params": best.extracted_params,
    }));
    
    // Build and execute skill chain
    let mut chain = SkillChain::new(params.initial.clone());
    chain.add_step(best.skill_id.clone());
    
    if let Some(suggested) = &best.suggested_chain {
        for step in suggested {
            chain.add_step(step.clone());
        }
    }
    
    match router.execute_chain(&chain, &params).await {
        Ok(result) => {
            println!("\n✅ 执行完成 (耗时: {}ms)", result.execution_time_ms);
            
            if result.all_succeeded {
                println!("✓ 所有步骤执行成功");
                println!("\n📤 最终输出:");
                println!("{}", serde_json::to_string_pretty(&result.final_output)?);
                
                log_builder.end_stage(true, Some(format!("skill={}, success", best.skill_id)), None);
                
                // Save final log
                let total_duration = total_start.elapsed().as_millis() as u64;
                let log = log_builder
                    .set_result(RequestResult::Success { 
                        skill_id: best.skill_id.clone(), 
                        output_summary: format!("Executed skill '{}' successfully", best.skill_name)
                    })
                    .set_metrics(RequestMetrics {
                        total_duration_ms: total_duration,
                        intent_duration_ms: 0,
                        routing_duration_ms: 0,
                        execution_duration_ms: result.execution_time_ms,
                    })
                    .add_metadata("candidate_count", candidates.len().to_string())
                    .add_metadata("confidence", format!("{:.2}", best.confidence))
                    .add_metadata("steps", chain.len().to_string())
                    .build();
                let _ = logger.log_request(&log);
            } else {
                println!("⚠️ 部分步骤执行失败:");
                for step in &result.step_results {
                    let status = if step.success { "✓" } else { "✗" };
                    println!("  {} 步骤 {}: {}", status, step.step_index, step.skill_id);
                    if let Some(ref error) = step.error {
                        println!("    错误: {}", error);
                    }
                }
                
                log_builder.end_stage(false, None, Some("Some steps failed".to_string()));
                
                // Save final log
                let total_duration = total_start.elapsed().as_millis() as u64;
                let log = log_builder
                    .set_result(RequestResult::Error { 
                        error: "Some steps failed".to_string(),
                    })
                    .set_metrics(RequestMetrics {
                        total_duration_ms: total_duration,
                        intent_duration_ms: 0,
                        routing_duration_ms: 0,
                        execution_duration_ms: result.execution_time_ms,
                    })
                    .add_metadata("candidate_count", candidates.len().to_string())
                    .add_metadata("confidence", format!("{:.2}", best.confidence))
                    .add_metadata("skill_id", best.skill_id.clone())
                    .build();
                let _ = logger.log_request(&log);
            }
        }
        Err(e) => {
            println!("\n❌ 执行失败: {}", e);
            
            log_builder.end_stage(false, None, Some(e.to_string()));
            
            // Save final log
            let total_duration = total_start.elapsed().as_millis() as u64;
            let log = log_builder
                .set_result(RequestResult::Error { error: e.to_string() })
                .set_metrics(RequestMetrics {
                    total_duration_ms: total_duration,
                    intent_duration_ms: 0,
                    routing_duration_ms: 0,
                    execution_duration_ms: 0,
                })
                .build();
            let _ = logger.log_request(&log);
            
            return Err(e.into());
        }
    }
    
    if logger.config().verbose {
        println!("\n📝 请求已记录");
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
pub async fn load_skill(name: &str, activate: bool) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Loading skill '{}'...", name);
    
    let options = LoadOptions {
        auto_activate: activate,
        force_reload: false,
        config: None,
    };
    
    manager.load(name, options)
        .await
        .with_context(|| format!("Failed to load skill '{}'", name))?;
    
    // 如果启用了自动激活，调用 activate
    if activate {
        manager.activate(name)
            .await
            .with_context(|| format!("Failed to activate skill '{}'", name))?;
        println!("✅ Skill '{}' loaded and activated.", name);
    } else {
        println!("✅ Skill '{}' loaded. Use 'cis skill activate {}' to activate.", name, name);
    }
    
    Ok(())
}

/// Unload a skill by name
pub async fn unload_skill(name: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Unloading skill '{}'...", name);
    
    manager.unload(name)
        .await
        .with_context(|| format!("Failed to unload skill '{}'", name))?;
    
    println!("✅ Skill '{}' unloaded.", name);
    
    Ok(())
}

/// Activate a skill
pub async fn activate_skill(name: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Activating skill '{}'...", name);
    
    manager.activate(name)
        .await
        .with_context(|| format!("Failed to activate skill '{}'", name))?;
    
    println!("✅ Skill '{}' activated.", name);
    
    Ok(())
}

/// Deactivate a skill
pub async fn deactivate_skill(name: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Deactivating skill '{}'...", name);
    
    manager.deactivate(name)
        .await
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
pub async fn call_skill(name: &str, method: &str, args: Option<&str>) -> Result<()> {
    println!("Calling skill '{}' method '{}'...", name, method);
    
    // Parse arguments
    let args_json: serde_json::Value = if let Some(args_str) = args {
        println!("Arguments: {}", args_str);
        serde_json::from_str(args_str).unwrap_or_else(|_| {
            serde_json::json!({ "input": args_str })
        })
    } else {
        serde_json::json!({})
    };
    
    // 通过 SkillManager 调用 Skill 方法
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = SkillManager::new(db_manager)?;
    
    // 检查 skill 是否已加载
    match skill_manager.is_loaded(name) {
        Ok(true) => {
            // 创建调用事件
            let event = cis_core::skill::Event::Custom {
                name: format!("skill:call:{}", method),
                data: serde_json::json!({
                    "skill": name,
                    "method": method,
                    "args": args_json,
                }),
            };
            
            // 发送事件到 skill
            match skill_manager.send_event(name, event).await {
                Ok(()) => {
                    println!("✅ Skill method '{}' called successfully", method);
                }
                Err(e) => {
                    eprintln!("❌ Failed to call skill method: {}", e);
                    return Err(anyhow::anyhow!("Failed to call skill method: {}", e));
                }
            }
        }
        Ok(false) => {
            println!("⚠️  Skill '{}' is not loaded. Please load it first with:", name);
            println!("   cis skill load {}", name);
            return Err(anyhow::anyhow!("Skill '{}' is not loaded", name));
        }
        Err(e) => {
            eprintln!("❌ Failed to check skill status: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

/// Install a skill from path
pub fn install_skill(path: &str) -> Result<()> {
    use cis_core::skill::manifest::SkillManifest;
    
    let path = std::path::Path::new(path);
    
    println!("Installing skill from '{}'...", path.display());
    
    // 检查是否是 DAG 文件（.toml 或 .json）
    let is_dag_file = path.extension().map_or(false, |ext| {
        ext == "toml" || ext == "json"
    });
    
    if is_dag_file {
        // 尝试作为 DAG 文件加载
        match SkillManifest::from_dag_file(path) {
            Ok(manifest) => {
                println!("📦 Loading DAG skill: {}", manifest.skill.name);
                
                // 转换为 SkillMeta 并注册
                let skill_info = cis_core::skill::types::SkillInfo::from_manifest(&manifest);
                
                let db_manager = Arc::new(DbManager::new()?);
                let manager = SkillManager::new(db_manager)?;
                manager.register(skill_info.meta)?;
                
                println!("✅ DAG skill '{}' installed successfully!", manifest.skill.name);
                if let Some(ref dag) = manifest.dag {
                    println!("   Tasks: {}", dag.tasks.len());
                    println!("   Policy: {:?}", dag.policy);
                }
                
                return Ok(());
            }
            Err(e) => {
                tracing::debug!("Not a valid DAG file: {}", e);
                // 继续尝试其他加载方式
            }
        }
    }
    
    // 原有的加载逻辑（Native/WASM）
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
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
pub async fn remove_skill(name: &str) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let manager = SkillManager::new(db_manager)?;
    
    println!("Removing skill '{}'...", name);
    
    manager.remove(name)
        .await
        .with_context(|| format!("Failed to remove skill '{}'", name))?;
    
    println!("✅ Skill '{}' removed.", name);
    
    Ok(())
}

/// Handle `cis skill chain` command - discover and execute skill chains
pub async fn handle_skill_chain(args: SkillChainArgs) -> Result<()> {
    let start = std::time::Instant::now();
    
    // Initialize SkillManager and DbManager (needed for router)
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = Arc::new(SkillManager::new(db_manager.clone())?);
    
    // 1. Parse intent
    println!("🎯 解析意图: {}", args.description);
    
    let vector_storage = Arc::new(VectorStorage::open(
        &Paths::vector_db(),
        None::<&EmbeddingConfig>,
    )?);
    
    let embedding_service = vector_storage.embedding_service().clone();
    let intent_parser = IntentParser::new(embedding_service.clone());
    
    let intent = match intent_parser.parse(&args.description).await {
        Ok(intent) => {
            if args.verbose {
                println!("  动作类型: {:?}", intent.action_type);
                println!("  置信度: {:.2}", intent.confidence);
                println!("  标准化意图: {}", intent.normalized_intent);
            }
            intent
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to parse intent: {}", e));
        }
    };
    
    // 2. Route to skills
    let project_path = args.project.as_deref()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    
    let project_registry = ProjectSkillRegistry::load(&project_path)
        .or_else(|_| Ok::<_, anyhow::Error>(ProjectSkillRegistry::new(&project_path)))?;
    
    let db_manager = Arc::new(DbManager::new()?);
    let router = SkillVectorRouter::new(
        vector_storage.clone(),
        embedding_service.clone(),
        skill_manager.clone(),
        db_manager,
    );
    
    let project_id = project_registry.project_path().to_str();
    let candidates = router.route(&intent, project_id).await
        .map_err(|e| anyhow::anyhow!("Failed to route intent: {}", e))?;
    
    if candidates.is_empty() {
        println!("❌ 未找到匹配的技能");
        return Ok(());
    }
    
    // 3. Discover skill chain
    let primary = &candidates[0];
    println!("\n🔗 发现技能链:");
    println!("   主技能: {} (置信度: {:.2})", primary.skill_name, primary.confidence);
    
    // Show chain if suggested
    if let Some(chain) = &primary.suggested_chain {
        println!("   建议链: {:?}", chain);
    }
    
    // Show all candidates in verbose mode
    if args.verbose && candidates.len() > 1 {
        println!("\n   候选技能:");
        for (i, c) in candidates.iter().enumerate().take(5).skip(1) {
            println!("     {}. {} (置信度: {:.2})", i + 1, c.skill_name, c.confidence);
        }
    }
    
    // 4. Preview mode - only show chain
    if args.preview {
        println!("\n📋 预览模式 (不执行):");
        println!("   将执行以下步骤:");
        println!("   1. {} - 参数: {}", primary.skill_name, primary.extracted_params);
        
        if let Some(chain) = &primary.suggested_chain {
            for (i, step) in chain.iter().enumerate() {
                println!("   {}. {}", i + 2, step);
            }
        }
        
        println!("\n⏱️  耗时: {:?}", start.elapsed());
        return Ok(());
    }
    
    // 5. Execute chain
    println!("\n⚡ 执行技能链...");
    
    // Build and execute chain
    let initial_input = serde_json::json!({
        "intent": intent.normalized_intent,
        "params": primary.extracted_params,
        "entities": intent.entities,
    });
    
    let mut chain = SkillChain::new(initial_input.clone());
    chain.add_step(primary.skill_id.clone());
    
    if let Some(suggested) = &primary.suggested_chain {
        for step in suggested {
            chain.add_step(step.clone());
        }
    }
    
    // Execute chain using router
    let params = ResolvedParameters::new(initial_input);
    
    match router.execute_chain(&chain, &params).await {
        Ok(result) => {
            println!("\n✅ 执行完成 (耗时: {}ms)", result.execution_time_ms);
            
            if result.all_succeeded {
                println!("✓ 所有步骤执行成功");
                println!("\n📤 最终输出:");
                println!("{}", serde_json::to_string_pretty(&result.final_output)?);
            } else {
                println!("⚠️ 部分步骤执行失败:");
                for step in &result.step_results {
                    let status = if step.success { "✓" } else { "✗" };
                    println!("  {} 步骤 {}: {}", status, step.step_index, step.skill_id);
                    if let Some(ref error) = step.error {
                        println!("    错误: {}", error);
                    }
                }
            }
        }
        Err(e) => {
            println!("\n❌ 执行失败: {}", e);
            return Err(e.into());
        }
    }
    
    println!("\n   步骤数: {}", chain.len());
    println!("   总耗时: {:?}", start.elapsed());
    
    Ok(())
}
