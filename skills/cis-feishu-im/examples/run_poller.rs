//! cis-feishu-im 长连接轮询启动程序
//!
//! 从配置文件读取配置并启动消息轮询

use cis_feishu_im::{FeishuImConfig, FeishuImSkill};
use cis_skill_sdk::Skill;
use std::path::PathBuf;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::level_filters::LevelFilter::INFO.into())
        )
        .init();

    println!("🚀 CIS 飞书 IM Skill - 长连接轮询模式");
    println!("");

    // 读取配置文件
    let config_path = PathBuf::from(std::env::var("HOME").unwrap())
        .join(".cis/config/feishu_im.toml");

    println!("📋 读取配置文件: {}", config_path.display());

    if !config_path.exists() {
        eprintln!("❌ 配置文件不存在: {}", config_path.display());
        eprintln!("   请先运行: bash scripts/init-config.sh");
        std::process::exit(1);
    }

    // 读取配置文件内容
    let config_content = std::fs::read_to_string(&config_path)?;
    let config: FeishuImConfig = toml::from_str(&config_content)
        .map_err(|e| format!("配置文件解析失败: {}", e))?;

    // 展开路径中的 ~
    let config = FeishuImConfig {
        im_db_path: expand_path(&config.im_db_path),
        memory_db_path: expand_path(&config.memory_db_path),
        ..config
    };

    println!("✅ 配置文件加载成功");
    println!();
    println!("📋 配置信息:");
    println!("  - App ID: {}", config.app_id);
    println!("  - 触发模式: {:?}", config.trigger_mode);
    println!("  - 上下文持久化: {}", config.context_config.persist_context);
    println!("  - 最大对话轮次: {}", config.context_config.max_turns);
    println!("  - IM 数据库: {}", config.im_db_path.display());
    println!("  - 记忆数据库: {}", config.memory_db_path.display());
    println!("  - 轮询间隔: {} 秒", config.polling.http_interval);
    println!("  - 批量大小: {} 条", config.polling.batch_size);
    println!();

    // 检查必要配置
    if config.app_id.is_empty() || config.app_secret.is_empty() {
        eprintln!("❌ 配置不完整: app_id 或 app_secret 未填写");
        std::process::exit(1);
    }

    // 创建 Skill
    println!("🔧 初始化 FeishuImSkill...");
    let mut skill = FeishuImSkill::with_config(config.clone());

    // 初始化 Skill（设置 AI Provider）
    let skill_config = cis_skill_sdk::SkillConfig::default();
    skill.init(skill_config)?;

    println!("✅ FeishuImSkill 初始化成功");
    println!();

    // 启动轮询
    println!("🔄 启动消息轮询...");
    println!("   模式: 冷冻模式（离线消息丢弃）");
    println!("   策略: 主动拉取 + 自动重连");
    println!();

    skill.start_polling().await
        .map_err(|e| format!("启动轮询失败: {}", e))?;

    println!("✅ 消息轮询已启动");
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📝 运行状态:");
    println!();
    println!("✅ 随时关机友好:");
    println!("   - 关机: 轮询自动停止，飞书标记离线");
    println!("   - 开机: 重新运行此脚本即可恢复");
    println!("   - 离线消息: 自动丢弃（冷冻模式）");
    println!();
    println!("📡 工作模式:");
    println!("   - 每 {} 秒拉取一次新消息", config.polling.http_interval);
    println!("   - 私聊消息自动响应");
    println!("   - 群聊 @ 机器人响应");
    println!();
    println!("按 Ctrl+C 停止服务");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // 等待 Ctrl+C 信号
    match signal::ctrl_c().await {
        Ok(()) => {
            println!();
            println!("🛑 收到停止信号，正在关闭...");
            skill.stop_polling().await?;
            println!("✅ 服务已停止");
        }
        Err(err) => {
            eprintln!("❌ 无法监听停止信号: {}", err);
            std::process::exit(1);
        }
    }

    Ok(())
}

/// 展开路径中的 ~
fn expand_path(path: &PathBuf) -> PathBuf {
    if path.starts_with("~") {
        if let Some(home) = std::env::var("HOME").ok() {
            return PathBuf::from(
                path.to_str()
                    .unwrap()
                    .replace("~", &home)
            );
        }
    }
    path.clone()
}
