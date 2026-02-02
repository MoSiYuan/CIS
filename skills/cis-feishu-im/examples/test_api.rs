//! 测试飞书 API 连接

use cis_feishu_im::FeishuImConfig;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 测试飞书 API 连接");
    println!();

    // 读取配置
    let config_path = PathBuf::from(std::env::var("HOME").unwrap())
        .join(".cis/config/feishu_im.toml");

    let config_content = std::fs::read_to_string(&config_path)?;
    let config: FeishuImConfig = toml::from_str(&config_content)?;

    println!("📋 配置信息:");
    println!("  App ID: {}", config.app_id);
    println!("  App Secret: {}****", &config.app_secret[..8]);
    println!();

    // 创建 API 客户端
    let api_client = cis_feishu_im::FeishuApiClient::new(
        config.app_id.clone(),
        config.app_secret.clone(),
    );

    println!("🔑 获取访问令牌...");
    match api_client.get_access_token().await {
        Ok(token) => {
            println!("✅ 访问令牌获取成功: {}****", &token[..20]);
            println!();
        }
        Err(e) => {
            println!("❌ 访问令牌获取失败: {:?}", e);
            return Err(e.into());
        }
    }

    println!("📋 获取会话列表...");
    match api_client.list_conversations().await {
        Ok(conversations) => {
            println!("✅ 会话列表获取成功: {} 个会话", conversations.len());
            println!();
            for (i, conv) in conversations.iter().enumerate() {
                println!("  {}. {}", i + 1, conv.name);
                println!("     ID: {}", conv.chat_id);
                println!("     类型: {}", conv.chat_type);
                println!();
            }

            if conversations.is_empty() {
                println!("⚠️  当前没有任何会话");
                println!();
                println!("💡 提示:");
                println!("  1. 请在飞书中添加机器人到群聊或发起私聊");
                println!("  2. 确保应用已发布并激活");
                println!("  3. 检查权限配置（im:chat）");
                println!();
            } else {
                // 测试获取第一个会话的消息
                if let Some(conv) = conversations.first() {
                    println!("📨 获取第一个会话的消息...");
                    match api_client.list_messages(&conv.chat_id, None, 5).await {
                        Ok(messages) => {
                            println!("✅ 消息获取成功: {} 条消息", messages.len());
                            for msg in messages {
                                println!("  - {}: {}", msg.sender.sender_id, msg.content);
                            }
                        }
                        Err(e) => {
                            println!("❌ 消息获取失败: {:?}", e);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ 会话列表获取失败: {:?}", e);
            println!();
            println!("💡 可能的原因:");
            println!("  1. 应用权限未开通（需要 im:chat 权限）");
            println!("  2. 应用未发布或未激活");
            println!("  3. App ID 或 App Secret 配置错误");
            println!();
        }
    }

    Ok(())
}
