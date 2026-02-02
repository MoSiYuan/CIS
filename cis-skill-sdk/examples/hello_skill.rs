//! Hello Skill 示例
//!
//! 展示如何使用 CIS Skill SDK 开发一个简单的 Skill

use cis_skill_sdk::{
    Event, HttpMethod, HttpRequest, LogLevel, MemoryOp, MessageContent, Result, Skill, SkillConfig,
    SkillContext, SkillMeta, Permission,
};
use cis_skill_sdk::im::{ImContextExt, ImMessage, ImMessageBuilder, MessageType};

/// 简单的问候 Skill
pub struct HelloSkill {
    greeting: String,
    counter: std::sync::atomic::AtomicU32,
}

impl HelloSkill {
    pub fn new() -> Self {
        Self {
            greeting: "Hello".to_string(),
            counter: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

impl Skill for HelloSkill {
    fn name(&self) -> &str {
        "hello"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn description(&self) -> &str {
        "A simple greeting skill"
    }
    
    fn meta(&self) -> SkillMeta {
        SkillMeta {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: self.description().to_string(),
            author: "CIS Team".to_string(),
            permissions: vec![
                Permission::MemoryRead,
                Permission::MemoryWrite,
                Permission::AiCall,
            ],
            subscriptions: vec![
                "custom:greet".to_string(),
                "im:message".to_string(),
            ],
            config_schema: None,
        }
    }
    
    fn init(&mut self, config: SkillConfig) -> Result<()> {
        if let Some(greeting) = config.get::<String>("greeting") {
            self.greeting = greeting;
        }
        Ok(())
    }
    
    fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        match event {
            Event::Custom { name, data } => {
                match name.as_str() {
                    "greet" => {
                        let target = data.as_str().unwrap_or("World");
                        let msg = format!("{}, {}!", self.greeting, target);
                        ctx.log_info(&msg);
                        
                        // 记录到记忆
                        let count = self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        ctx.memory_set(
                            &format!("hello/greet_count/{}", target),
                            &count.to_le_bytes(),
                        )?;
                    }
                    _ => {}
                }
            }
            Event::MemoryChange { key, value, operation } => {
                ctx.log_debug(&format!(
                    "Memory changed: {} -> {:?} ({:?})",
                    key, operation, value
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

/// IM 回复 Skill 示例
pub struct ImEchoSkill;

impl Skill for ImEchoSkill {
    fn name(&self) -> &str {
        "im-echo"
    }
    
    fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        if let Event::Custom { name, data } = event {
            if name == "im:message" {
                // 解析 IM 消息
                let msg: ImMessage = serde_json::from_value(data)
                    .map_err(|e| cis_skill_sdk::Error::SerializationError(e.to_string()))?;
                
                // 只处理文本消息
                if let MessageContent::Text { text } = &msg.content {
                    ctx.log_info(&format!("收到消息: {}", text));
                    
                    // 如果是特定命令，回复
                    if text.starts_with("/echo ") {
                        let reply_text = text.trim_start_matches("/echo ").to_string();
                        
                        // 构建回复
                        let reply = ImMessageBuilder::text(&reply_text)
                            .to(&msg.from)
                            .from("im-echo")
                            .reply_to(&msg.id)
                            .build();
                        
                        // 发送回复
                        ctx.im_send(&reply)?;
                    }
                }
            }
        }
        Ok(())
    }
}

fn main() {
    println!("Hello Skill Example");
    println!("==================");
    
    // 创建 Skill
    let skill = HelloSkill::new();
    
    // 打印元数据
    let meta = skill.meta();
    println!("Skill: {} v{}", meta.name, meta.version);
    println!("Description: {}", meta.description);
    println!("Permissions: {:?}", meta.permissions);
    println!("Subscriptions: {:?}", meta.subscriptions);
    
    println!("\n示例完成！");
}
