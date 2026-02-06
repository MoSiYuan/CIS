//! `cis im` 命令
//!
//! 提供 IM (即时通讯) 功能的命令行接口。

use clap::{Args, Subcommand};
use anyhow::Result;
use std::sync::Arc;
use cis_core::storage::db::DbManager;
use cis_core::skill::SkillManager;

/// IM 命令参数
#[derive(Args, Debug)]
pub struct ImArgs {
    #[command(subcommand)]
    pub action: ImAction,
}

/// IM 子命令
#[derive(Subcommand, Debug)]
pub enum ImAction {
    /// 发送消息
    Send(SendArgs),
    /// 列出会话
    List(ListArgs),
    /// 查看消息历史
    History(HistoryArgs),
    /// 搜索消息
    Search(SearchArgs),
    /// 创建会话
    Create(CreateArgs),
    /// 标记消息已读
    Read(ReadArgs),
    /// 获取会话信息
    Info(InfoArgs),
}

/// 发送消息参数
#[derive(Args, Debug)]
pub struct SendArgs {
    /// 会话 ID
    pub session_id: String,
    /// 消息内容
    pub message: String,
    /// 回复的消息 ID
    #[arg(short, long)]
    pub reply_to: Option<String>,
}

/// 列出会话参数
#[derive(Args, Debug)]
pub struct ListArgs {
    /// 最大返回数量
    #[arg(short, long, default_value = "20")]
    pub limit: usize,
    /// 用户 ID（默认当前用户）
    #[arg(short, long)]
    pub user: Option<String>,
}

/// 查看消息历史参数
#[derive(Args, Debug)]
pub struct HistoryArgs {
    /// 会话 ID
    pub session_id: String,
    /// 最大返回数量
    #[arg(short, long, default_value = "50")]
    pub limit: usize,
    /// 在指定时间之前
    #[arg(short, long)]
    pub before: Option<String>,
}

/// 搜索消息参数
#[derive(Args, Debug)]
pub struct SearchArgs {
    /// 搜索关键词
    pub query: String,
    /// 限定会话 ID
    #[arg(short, long)]
    pub session: Option<String>,
    /// 最大返回数量
    #[arg(short, long, default_value = "20")]
    pub limit: usize,
    /// 使用语义搜索
    #[arg(long)]
    pub semantic: bool,
}

/// 创建会话参数
#[derive(Args, Debug)]
pub struct CreateArgs {
    /// 会话类型
    #[arg(short, long, value_enum, default_value = "group")]
    pub r#type: SessionType,
    /// 会话标题
    pub title: String,
    /// 参与者用户 ID 列表
    #[arg(short, long, required = true)]
    pub participants: Vec<String>,
}

/// 会话类型
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum SessionType {
    /// 一对一私聊
    Direct,
    /// 群组聊天
    Group,
    /// 频道
    Channel,
}

/// 标记已读参数
#[derive(Args, Debug)]
pub struct ReadArgs {
    /// 会话 ID
    pub session_id: String,
    /// 特定消息 ID
    #[arg(short, long)]
    pub message: Option<String>,
    /// 标记所有消息已读
    #[arg(short, long)]
    pub all: bool,
}

/// 获取会话信息参数
#[derive(Args, Debug)]
pub struct InfoArgs {
    /// 会话 ID
    pub session_id: String,
}

/// 处理 IM 命令
pub async fn handle_im(args: ImArgs) -> Result<()> {
    match args.action {
        ImAction::Send(send_args) => {
            handle_send(send_args).await?;
        }
        ImAction::List(list_args) => {
            handle_list(list_args).await?;
        }
        ImAction::History(history_args) => {
            handle_history(history_args).await?;
        }
        ImAction::Search(search_args) => {
            handle_search(search_args).await?;
        }
        ImAction::Create(create_args) => {
            handle_create(create_args).await?;
        }
        ImAction::Read(read_args) => {
            handle_read(read_args).await?;
        }
        ImAction::Info(info_args) => {
            handle_info(info_args).await?;
        }
    }

    Ok(())
}

/// 处理发送消息
async fn handle_send(args: SendArgs) -> Result<()> {
    println!("📤 发送消息到会话 {}:", args.session_id);
    println!("   内容: {}", args.message);
    
    if let Some(reply_to) = &args.reply_to {
        println!("   回复: {}", reply_to);
    }

    // 通过 SkillManager 调用 IM Skill
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = SkillManager::new(db_manager)?;
    
    // 检查 IM Skill 是否已加载
    match skill_manager.is_loaded("im") {
        Ok(true) => {
            println!("   IM Skill 已加载");
            
            // 构建消息内容
            let content = serde_json::json!({
                "msgtype": "m.text",
                "body": args.message,
                "reply_to": args.reply_to,
            });
            
            // 发送事件到 IM Skill
            let event = cis_core::skill::Event::Custom {
                name: "im:send_message".to_string(),
                data: serde_json::json!({
                    "conversation_id": args.session_id,
                    "content": content,
                }),
            };
            
            match skill_manager.send_event("im", event).await {
                Ok(()) => {
                    println!("✅ 消息已发送");
                }
                Err(e) => {
                    eprintln!("❌ 发送失败: {}", e);
                }
            }
        }
        Ok(false) => {
            println!("⚠️  IM Skill 未加载，请先加载: cis skill load im");
        }
        Err(e) => {
            eprintln!("❌ 检查 IM Skill 状态失败: {}", e);
        }
    }
    
    Ok(())
}

/// 处理列出会话
async fn handle_list(args: ListArgs) -> Result<()> {
    let user_id = args.user.as_deref().unwrap_or("current_user");
    
    println!("📋 用户 {} 的会话列表（最近 {} 个）:", user_id, args.limit);
    println!();

    // 通过 SkillManager 调用 IM Skill
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = SkillManager::new(db_manager)?;
    
    match skill_manager.is_loaded("im") {
        Ok(true) => {
            // 发送事件获取会话列表
            let event = cis_core::skill::Event::Custom {
                name: "im:list_conversations".to_string(),
                data: serde_json::json!({
                    "user_id": user_id,
                    "limit": args.limit,
                }),
            };
            
            match skill_manager.send_event("im", event).await {
                Ok(()) => {
                    println!("✅ 已请求会话列表（异步处理）");
                }
                Err(e) => {
                    eprintln!("❌ 获取会话列表失败: {}", e);
                }
            }
        }
        Ok(false) => {
            println!("⚠️  IM Skill 未加载，请先加载: cis skill load im");
        }
        Err(e) => {
            eprintln!("❌ 检查 IM Skill 状态失败: {}", e);
        }
    }

    Ok(())
}

/// 处理查看消息历史
async fn handle_history(args: HistoryArgs) -> Result<()> {
    println!("📜 会话 {} 的消息历史（最近 {} 条）:", args.session_id, args.limit);
    println!();

    if let Some(before) = &args.before {
        println!("   在 {} 之前", before);
        println!();
    }

    // 通过 SkillManager 调用 IM Skill
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = SkillManager::new(db_manager)?;
    
    match skill_manager.is_loaded("im") {
        Ok(true) => {
            // 发送事件获取消息历史
            let event = cis_core::skill::Event::Custom {
                name: "im:get_history".to_string(),
                data: serde_json::json!({
                    "conversation_id": args.session_id,
                    "limit": args.limit,
                    "before": args.before,
                }),
            };
            
            match skill_manager.send_event("im", event).await {
                Ok(()) => {
                    println!("✅ 已请求消息历史（异步处理）");
                }
                Err(e) => {
                    eprintln!("❌ 获取消息历史失败: {}", e);
                }
            }
        }
        Ok(false) => {
            println!("⚠️  IM Skill 未加载，请先加载: cis skill load im");
        }
        Err(e) => {
            eprintln!("❌ 检查 IM Skill 状态失败: {}", e);
        }
    }

    Ok(())
}

/// 处理搜索消息
async fn handle_search(args: SearchArgs) -> Result<()> {
    println!("🔍 搜索消息: {}", args.query);
    
    if let Some(session_id) = &args.session {
        println!("   限定会话: {}", session_id);
    }
    
    if args.semantic {
        println!("   搜索模式: 语义搜索");
    } else {
        println!("   搜索模式: 关键词搜索");
    }
    println!();

    // 通过 SkillManager 调用 IM Skill
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = SkillManager::new(db_manager)?;
    
    match skill_manager.is_loaded("im") {
        Ok(true) => {
            // 发送事件搜索消息
            let event = cis_core::skill::Event::Custom {
                name: "im:search_messages".to_string(),
                data: serde_json::json!({
                    "query": args.query,
                    "session_id": args.session,
                    "limit": args.limit,
                    "semantic": args.semantic,
                }),
            };
            
            match skill_manager.send_event("im", event).await {
                Ok(()) => {
                    println!("✅ 已请求搜索消息（异步处理）");
                }
                Err(e) => {
                    eprintln!("❌ 搜索消息失败: {}", e);
                }
            }
        }
        Ok(false) => {
            println!("⚠️  IM Skill 未加载，请先加载: cis skill load im");
        }
        Err(e) => {
            eprintln!("❌ 检查 IM Skill 状态失败: {}", e);
        }
    }

    Ok(())
}

/// 处理创建会话
async fn handle_create(args: CreateArgs) -> Result<()> {
    let session_type = match args.r#type {
        SessionType::Direct => "direct",
        SessionType::Group => "group",
        SessionType::Channel => "channel",
    };

    println!("📢 创建新会话:");
    println!("   类型: {}", session_type);
    println!("   标题: {}", args.title);
    println!("   参与者: {}", args.participants.join(", "));

    // 通过 SkillManager 调用 IM Skill
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = SkillManager::new(db_manager)?;
    
    match skill_manager.is_loaded("im") {
        Ok(true) => {
            // 发送事件创建会话
            let event = cis_core::skill::Event::Custom {
                name: "im:create_conversation".to_string(),
                data: serde_json::json!({
                    "session_type": session_type,
                    "title": args.title,
                    "participants": args.participants,
                }),
            };
            
            match skill_manager.send_event("im", event).await {
                Ok(()) => {
                    println!("✅ 会话创建请求已发送");
                }
                Err(e) => {
                    eprintln!("❌ 创建会话失败: {}", e);
                }
            }
        }
        Ok(false) => {
            println!("⚠️  IM Skill 未加载，请先加载: cis skill load im");
        }
        Err(e) => {
            eprintln!("❌ 检查 IM Skill 状态失败: {}", e);
        }
    }
    
    Ok(())
}

/// 处理标记已读
async fn handle_read(args: ReadArgs) -> Result<()> {
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = SkillManager::new(db_manager)?;
    
    match skill_manager.is_loaded("im") {
        Ok(true) => {
            if args.all {
                println!("📖 标记会话 {} 的所有消息已读", args.session_id);
                // 发送事件批量标记已读
                let event = cis_core::skill::Event::Custom {
                    name: "im:mark_all_read".to_string(),
                    data: serde_json::json!({
                        "conversation_id": args.session_id,
                    }),
                };
                
                match skill_manager.send_event("im", event).await {
                    Ok(()) => {
                        println!("✅ 批量标记已读请求已发送");
                    }
                    Err(e) => {
                        eprintln!("❌ 标记已读失败: {}", e);
                    }
                }
            } else if let Some(message_id) = &args.message {
                println!("📖 标记消息 {} 已读", message_id);
                // 发送事件标记单条消息已读
                let event = cis_core::skill::Event::Custom {
                    name: "im:mark_read".to_string(),
                    data: serde_json::json!({
                        "conversation_id": args.session_id,
                        "message_id": message_id,
                    }),
                };
                
                match skill_manager.send_event("im", event).await {
                    Ok(()) => {
                        println!("✅ 标记消息已读请求已发送");
                    }
                    Err(e) => {
                        eprintln!("❌ 标记已读失败: {}", e);
                    }
                }
            } else {
                println!("⚠️ 请指定 --message 或 --all");
            }
        }
        Ok(false) => {
            println!("⚠️  IM Skill 未加载，请先加载: cis skill load im");
        }
        Err(e) => {
            eprintln!("❌ 检查 IM Skill 状态失败: {}", e);
        }
    }

    Ok(())
}

/// 处理获取会话信息
async fn handle_info(args: InfoArgs) -> Result<()> {
    println!("ℹ️  会话 {} 信息:", args.session_id);
    println!();

    // 通过 SkillManager 调用 IM Skill
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = SkillManager::new(db_manager)?;
    
    match skill_manager.is_loaded("im") {
        Ok(true) => {
            // 发送事件获取会话信息
            let event = cis_core::skill::Event::Custom {
                name: "im:get_conversation_info".to_string(),
                data: serde_json::json!({
                    "conversation_id": args.session_id,
                }),
            };
            
            match skill_manager.send_event("im", event).await {
                Ok(()) => {
                    println!("✅ 已请求会话信息（异步处理）");
                }
                Err(e) => {
                    eprintln!("❌ 获取会话信息失败: {}", e);
                }
            }
        }
        Ok(false) => {
            // 显示基本占位信息
            println!("  ID:          {}", args.session_id);
            println!("  类型:        group");
            println!("  名称:        开发团队");
            println!("  参与者:      5 人");
            println!("  创建时间:    2024-01-01 10:00:00");
            println!("  最后消息:    2024-01-15 16:30:00");
            println!("  未读消息:    3 条");
            println!();
            println!("⚠️  IM Skill 未加载，以上为模拟数据");
            println!("   请先加载: cis skill load im");
        }
        Err(e) => {
            eprintln!("❌ 检查 IM Skill 状态失败: {}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_type_enum() {
        assert_eq!(SessionType::Direct as i32, 0);
        assert_eq!(SessionType::Group as i32, 1);
        assert_eq!(SessionType::Channel as i32, 2);
    }

    #[test]
    fn test_send_args() {
        let args = SendArgs {
            session_id: "test-session".to_string(),
            message: "Hello".to_string(),
            reply_to: Some("msg-123".to_string()),
        };
        assert_eq!(args.session_id, "test-session");
        assert_eq!(args.message, "Hello");
        assert_eq!(args.reply_to, Some("msg-123".to_string()));
    }
}
