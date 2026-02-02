//! cis-feishu-im 示例程序
//!
//! 展示如何使用 FeishuImSkill

use cis_feishu_im::{FeishuImConfig, FeishuImSkill, TriggerMode};
use cis_skill_sdk::Skill;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    println!("🚀 CIS Feishu IM Skill 示例程序\n");

    // 创建配置
    let config = FeishuImConfig {
        app_id: "cli_example_app_id".to_string(),
        app_secret: "example_secret".to_string(),
        encrypt_key: "example_key".to_string(),
        verify_token: "example_token".to_string(),
        verify_signature: false, // 示例程序关闭签名验证
        trigger_mode: TriggerMode::All, // 示例程序响应所有消息
        im_db_path: PathBuf::from("/tmp/feishu_im_example.db"),
        memory_db_path: PathBuf::from("/tmp/memory_example.db"),
        ..Default::default()
    };

    println!("📋 配置信息:");
    println!("  - 触发模式: {:?}", config.trigger_mode);
    println!("  - 上下文持久化: {}", config.context_config.persist_context);
    println!("  - 最大对话轮次: {}", config.context_config.max_turns);
    println!();

    // 创建 Skill
    let mut skill = FeishuImSkill::with_config(config);

    // 初始化 Skill (同步方法，不需要 await)
    let skill_config = cis_skill_sdk::SkillConfig::default();
    skill.init(skill_config)?;

    println!("✅ FeishuImSkill 初始化成功");
    println!();

    println!("📝 Webhook 服务器信息:");
    println!("  - 地址: http://0.0.0.0:8080/webhook/feishu");
    println!();
    println!("💡 提示: 使用飞书发送消息到配置的 Webhook URL");
    println!("     (在生产环境请配置真实的飞书应用凭证)");
    println!();

    println!("✨ 示例程序运行完成");
    println!("   要实际使用 Webhook 功能，请调用 skill.start_webhook().await");

    Ok(())
}
