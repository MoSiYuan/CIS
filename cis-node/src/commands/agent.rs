//! # Agent Command
//!
//! Interact with AI agent.

use anyhow::{Context, Result};
use cis_core::agent::{
    AgentContext, AgentProvider, AgentProviderFactory, AgentRequest,
};
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
