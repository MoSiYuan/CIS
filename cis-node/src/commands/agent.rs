//! # Agent Command
//!
//! Interact with AI agent.

use anyhow::{Context, Result};
use cis_core::agent::{
    AgentContext, AgentProvider, AgentProviderFactory, AgentRequest,
};
use cis_core::storage::paths::Paths;
use std::path::PathBuf;
use tracing::info;

/// Execute a prompt with the AI agent
pub async fn execute_prompt(prompt: &str) -> Result<()> {
    info!("Executing prompt with AI agent...");
    
    // Get default provider
    let provider = AgentProviderFactory::default_provider().await
        .context("No AI agent available. Please install Claude Code, Kimi, or Aider.")?;
    
    println!("🤖 Using AI agent: {}", provider.name());
    println!("⏳ Processing your request...\n");
    
    // Build request
    let request = AgentRequest {
        prompt: prompt.to_string(),
        context: AgentContext::new()
            .with_work_dir(std::env::current_dir()?),
        skills: vec![],
        system_prompt: None,
        history: vec![],
    };
    
    // Execute
    match provider.execute(request).await {
        Ok(response) => {
            println!("\n📝 Response:\n{}", response.content);
            
            if let Some(usage) = response.token_usage {
                println!("\n📊 Token usage: {} total ({} prompt, {} completion)",
                    usage.total, usage.prompt, usage.completion);
            }
        }
        Err(e) => {
            eprintln!("\n❌ Error: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

/// Execute a prompt with specific skills enabled
pub async fn execute_with_skills(prompt: &str, skills: Vec<String>) -> Result<()> {
    info!("Executing prompt with skills: {:?}", skills);
    
    let provider = AgentProviderFactory::default_provider().await
        .context("No AI agent available.")?;
    
    println!("🤖 Using AI agent: {}", provider.name());
    println!("🔧 Enabled skills: {}", skills.join(", "));
    println!("⏳ Processing your request...\n");
    
    let request = AgentRequest {
        prompt: prompt.to_string(),
        context: AgentContext::new()
            .with_work_dir(std::env::current_dir()?),
        skills,
        system_prompt: None,
        history: vec![],
    };
    
    match provider.execute(request).await {
        Ok(response) => {
            println!("\n📝 Response:\n{}", response.content);
        }
        Err(e) => {
            eprintln!("\n❌ Error: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

/// List available AI agents
pub async fn list_agents() -> Result<()> {
    use cis_core::agent::providers::{ClaudeProvider, KimiProvider, AiderProvider};
    
    println!("Available AI Agents:");
    println!("{}", "-".repeat(50));
    
    let claude = ClaudeProvider::default();
    let kimi = KimiProvider::default();
    let aider = AiderProvider::default();
    
    // Check Claude
    print!("  Claude Code  ");
    if claude.available().await {
        println!("✅ Available");
    } else {
        println!("❌ Not found");
    }
    
    // Check Kimi
    print!("  Kimi         ");
    if kimi.available().await {
        println!("✅ Available");
    } else {
        println!("❌ Not found");
    }
    
    // Check Aider
    print!("  Aider        ");
    if aider.available().await {
        println!("✅ Available");
    } else {
        println!("❌ Not found");
    }
    
    println!("\nNote: At least one AI agent must be installed to use 'cis agent' command.");
    
    Ok(())
}

/// Start an interactive chat session
pub async fn interactive_chat() -> Result<()> {
    use std::io::{self, Write};
    
    let provider = AgentProviderFactory::default_provider().await
        .context("No AI agent available.")?;
    
    println!("🤖 CIS Interactive Chat");
    println!("Using AI agent: {}", provider.name());
    println!("Type 'exit' or 'quit' to end the session.\n");
    
    let mut history = vec![];
    
    loop {
        print!("You: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("Goodbye! 👋");
            break;
        }
        
        let request = AgentRequest {
            prompt: input.to_string(),
            context: AgentContext::new()
                .with_work_dir(std::env::current_dir()?),
            skills: vec![],
            system_prompt: None,
            history: history.clone(),
        };
        
        match provider.execute(request).await {
            Ok(response) => {
                println!("\nAgent: {}\n", response.content);
                
                // Add to history
                history.push(cis_core::agent::AgentMessage {
                    role: cis_core::agent::MessageRole::User,
                    content: input.to_string(),
                });
                history.push(cis_core::agent::AgentMessage {
                    role: cis_core::agent::MessageRole::Assistant,
                    content: response.content.clone(),
                });
                
                // Limit history size
                if history.len() > 20 {
                    history.drain(0..2);
                }
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Arguments for `cis agent context` command
#[derive(Debug, clap::Args)]
pub struct AgentContextArgs {
    /// The prompt to send to the agent
    pub prompt: String,
    
    /// Session ID for conversation context
    #[arg(short, long)]
    pub session: Option<String>,
    
    /// Project path
    #[arg(short, long)]
    pub project: Option<PathBuf>,
}

/// Handle `cis agent context` command - AI conversation with context
pub async fn handle_agent_context(args: AgentContextArgs) -> Result<()> {
    info!("Executing agent context command...");
    
    // Generate or use provided session ID
    let session_id = args.session.unwrap_or_else(|| {
        format!("session-{}", std::process::id())
    });
    
    let project_path = args.project.as_deref()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    
    println!("🤖 使用会话上下文: {}", session_id);
    println!("📁 项目路径: {}", project_path.display());
    println!("⏳ 处理请求...\n");
    
    // Initialize conversation database
    use cis_core::storage::conversation_db::ConversationDb;
    let conv_db = ConversationDb::open(&Paths::data_dir().join("conversations.db"))
        .map_err(|e| anyhow::anyhow!("Failed to open conversation database: {}", e))?;
    
    // Try to find existing conversation by session or project
    let conversation: cis_core::storage::conversation_db::Conversation = 
        match conv_db.list_conversations_by_session(&session_id, 1) {
            Ok(mut conversations) if !conversations.is_empty() => {
                let conv = conversations.remove(0);
                println!("💬 恢复历史对话 (创建于 {})\n", conv.created_at.format("%Y-%m-%d %H:%M"));
                conv
            }
            _ => {
                // Start new conversation
                let conv = cis_core::storage::conversation_db::Conversation {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: session_id.clone(),
                    project_path: Some(project_path.to_string_lossy().to_string()),
                    summary: None,
                    topics: vec![],
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                };
                conv_db.save_conversation(&conv)
                    .map_err(|e| anyhow::anyhow!("Failed to save conversation: {}", e))?;
                println!("🆕 开始新对话\n");
                conv
            }
        };
    
    // Add user message
    let user_msg = cis_core::storage::conversation_db::ConversationMessage {
        id: uuid::Uuid::new_v4().to_string(),
        conversation_id: conversation.id.clone(),
        role: "user".to_string(),
        content: args.prompt.clone(),
        timestamp: chrono::Utc::now(),
    };
    conv_db.save_message(&user_msg)
        .map_err(|e| anyhow::anyhow!("Failed to add message: {}", e))?;
    
    // Get AI provider
    let provider = AgentProviderFactory::default_provider().await
        .context("No AI agent available. Please install Claude Code, Kimi, or Aider.")?;
    
    // Prepare enhanced prompt with context - get recent messages
    let history = conv_db.get_messages(&conversation.id)
        .map_err(|e| anyhow::anyhow!("Failed to get messages: {}", e))?;
    
    let mut agent_history = vec![];
    // Take last 10 messages for context
    let start_idx = history.len().saturating_sub(10);
    for msg in &history[start_idx..] {
        agent_history.push(cis_core::agent::AgentMessage {
            role: match msg.role.as_str() {
                "user" => cis_core::agent::MessageRole::User,
                _ => cis_core::agent::MessageRole::Assistant,
            },
            content: msg.content.clone(),
        });
    }
    
    // Execute with context
    let request = AgentRequest {
        prompt: args.prompt.clone(),
        context: AgentContext::new()
            .with_work_dir(project_path),
        skills: vec![],
        system_prompt: None,
        history: agent_history,
    };
    
    match provider.execute(request).await {
        Ok(response) => {
            println!("📝 回复:\n{}\n", response.content);
            
            // Add assistant response to context
            let assistant_msg = cis_core::storage::conversation_db::ConversationMessage {
                id: uuid::Uuid::new_v4().to_string(),
                conversation_id: conversation.id.clone(),
                role: "assistant".to_string(),
                content: response.content.clone(),
                timestamp: chrono::Utc::now(),
            };
            conv_db.save_message(&assistant_msg)
                .map_err(|e| anyhow::anyhow!("Failed to save response: {}", e))?;
            
            if let Some(usage) = response.token_usage {
                println!("📊 Token 使用: {} 总计 ({} 提示, {} 完成)",
                    usage.total, usage.prompt, usage.completion);
            }
        }
        Err(e) => {
            eprintln!("\n❌ 错误: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}
