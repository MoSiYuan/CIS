//! cis-feishu-im Webhook 服务器启动程序
//!
//! 从配置文件读取配置并启动 Webhook 服务器

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

    println!("🚀 CIS 飞书 IM Skill - Webhook 服务器");
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
    println!("  - Webhook 地址: http://{}:{}{}",
        config.webhook.bind_address,
        config.webhook.port,
        config.webhook.path
    );
    println!();

    // 检查必要配置
    if config.app_id.is_empty() || config.app_secret.is_empty() {
        eprintln!("❌ 配置不完整: app_id 或 app_secret 未填写");
        std::process::exit(1);
    }

    if config.encrypt_key.is_empty() || config.verify_token.is_empty() {
        println!("⚠️  警告: encrypt_key 或 verify_token 未填写");
        println!("   Webhook 签名验证可能无法工作");
        println!("   请在飞书开放平台配置事件订阅后填写这些值");
        println!();
    }

    // 保存配置信息供后续使用
    let webhook_port = config.webhook.port;
    let webhook_path = config.webhook.path.clone();
    let bind_address = config.webhook.bind_address.clone();

    // 创建 Skill
    println!("🔧 初始化 FeishuImSkill...");
    let mut skill = FeishuImSkill::with_config(config);

    // 初始化 Skill
    let skill_config = cis_skill_sdk::SkillConfig::default();
    skill.init(skill_config)?;

    println!("✅ FeishuImSkill 初始化成功");
    println!();

    // 启动 Webhook 服务器
    println!("🌐 启动 Webhook 服务器...");
    println!("   监听地址: http://{}:{}{}", bind_address, webhook_port, webhook_path);
    println!();

    // 在单独的任务中启动 Webhook 服务器
    let webhook_handle = tokio::spawn(async move {
        if let Err(e) = skill.start_webhook().await {
            eprintln!("❌ Webhook 服务器启动失败: {}", e);
            std::process::exit(1);
        }
    });

    println!("✅ Webhook 服务器已启动");
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📝 下一步操作:");
    println!();
    println!("1. 使用 ngrok 或类似工具暴露本地端口:");
    println!("   ngrok http {}", webhook_port);
    println!();
    println!("2. 在飞书开放平台配置事件订阅:");
    println!("   URL: https://xxxx.ngrok-free.app{}", webhook_path);
    println!("   事件: im.message.receive_v1");
    println!();
    println!("3. 复制生成的 Encrypt Key 和 Verification Token");
    println!("   填写到配置文件中:");
    println!("   nano ~/.cis/config/feishu_im.toml");
    println!();
    println!("4. 重启此服务");
    println!();
    println!("按 Ctrl+C 停止服务");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    // 等待 Ctrl+C 信号
    match signal::ctrl_c().await {
        Ok(()) => {
            println!();
            println!("🛑 收到停止信号，正在关闭...");
            webhook_handle.abort();
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
