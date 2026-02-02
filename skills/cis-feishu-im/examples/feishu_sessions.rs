//! 飞书会话查询工具
//!
//! 用于查看和管理飞书对话会话

use cis_feishu_im::{
    FeishuImConfig, FeishuSessionManager,
    expand_path, ConversationContext,
};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📱 CIS 飞书会话查询工具");
    println!();

    // 读取配置
    let config_path = PathBuf::from(std::env::var("HOME").unwrap())
        .join(".cis/config/feishu_im.toml");

    let config_content = std::fs::read_to_string(&config_path)?;
    let config: FeishuImConfig = toml::from_str(&config_content)?;
    let config = FeishuImConfig {
        im_db_path: expand_path(&config.im_db_path),
        memory_db_path: expand_path(&config.memory_db_path),
        ..config
    };

    // 创建运行时
    let rt = tokio::runtime::Runtime::new()?;

    // 创建会话管理器
    let context = Arc::new(ConversationContext::new(config.context_config.clone()));
    let session_manager = Arc::new(FeishuSessionManager::new(
        config.im_db_path.clone(),
        context,
    ));

    // 加载历史会话
    rt.block_on(session_manager.load_sessions());

    println!("✅ 会话管理器已初始化");
    println!();

    // 启动 REPL
    run_repl(&session_manager, &rt);

    Ok(())
}

/// 运行交互式命令行
fn run_repl(session_manager: &Arc<FeishuSessionManager>, rt: &tokio::runtime::Runtime) {
    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📝 可用命令:");
    println!();
    println!("  list              - 列出所有会话");
    println!("  list-active       - 列出活跃会话");
    println!("  show <session_id>  - 显示会话详情");
    println!("  search <query>    - 搜索会话");
    println!("  archive <id>      - 归档会话");
    println!("  delete <id>       - 删除会话");
    println!("  help              - 显示帮助");
    println!("  exit / quit       - 退出");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();

    loop {
        print!("📱 feishu> ");
        io::stdout().flush().unwrap();

        line.clear();
        let bytes_read = reader.read_line(&mut line).unwrap();
        if bytes_read == 0 {
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let command = parts[0];

        match command {
            "list" => {
                rt.block_on(cmd_list(session_manager));
            }
            "list-active" => {
                rt.block_on(cmd_list_active(session_manager));
            }
            "show" => {
                if parts.len() < 2 {
                    println!("❌ 用法: show <session_id>");
                    continue;
                }
                rt.block_on(cmd_show(session_manager, parts[1]));
            }
            "search" => {
                if parts.len() < 2 {
                    println!("❌ 用法: search <query>");
                    continue;
                }
                rt.block_on(cmd_search(session_manager, parts[1]));
            }
            "archive" => {
                if parts.len() < 2 {
                    println!("❌ 用法: archive <session_id>");
                    continue;
                }
                rt.block_on(cmd_archive(session_manager, parts[1]));
            }
            "delete" => {
                if parts.len() < 2 {
                    println!("❌ 用法: delete <session_id>");
                    continue;
                }
                rt.block_on(cmd_delete(session_manager, parts[1]));
            }
            "help" => {
                print_help();
            }
            "exit" | "quit" => {
                println!("👋 再见！");
                break;
            }
            _ => {
                println!("❌ 未知命令: {} (输入 'help' 查看帮助)", command);
            }
        }

        println!();
    }
}

/// 列出所有会话
async fn cmd_list(session_manager: &Arc<FeishuSessionManager>) {
    let sessions = session_manager.list_sessions().await;

    if sessions.is_empty() {
        println!("📭 暂无会话");
        return;
    }

    println!("📋 所有会话 ({} 个):", sessions.len());
    println!();

    for session in sessions {
        println!("{}", FeishuSessionManager::format_session_summary(&session));
        println!();
    }
}

/// 列出活跃会话
async fn cmd_list_active(session_manager: &Arc<FeishuSessionManager>) {
    let sessions = session_manager.list_active_sessions().await;

    if sessions.is_empty() {
        println!("📭 暂无活跃会话");
        return;
    }

    println!("✅ 活跃会话 ({} 个):", sessions.len());
    println!();

    for session in sessions {
        println!("{}", FeishuSessionManager::format_session_summary(&session));
        println!();
    }
}

/// 显示会话详情
async fn cmd_show(session_manager: &Arc<FeishuSessionManager>, session_id: &str) {
    let session = match session_manager.get_session(session_id).await {
        Some(s) => s,
        None => {
            println!("❌ 会话不存在: {}", session_id);
            return;
        }
    };

    let history = session_manager.get_session_history(session_id).await;
    let detail = FeishuSessionManager::format_session_detail(&session, &history);

    println!("{}", detail);
}

/// 搜索会话
async fn cmd_search(session_manager: &Arc<FeishuSessionManager>, query: &str) {
    let sessions = session_manager.search_sessions(query).await;

    if sessions.is_empty() {
        println!("🔍 未找到匹配 '{}' 的会话", query);
        return;
    }

    println!("🔍 搜索结果 (匹配 '{}', {} 个):", query, sessions.len());
    println!();

    for session in sessions {
        println!("{}", FeishuSessionManager::format_session_summary(&session));
        println!();
    }
}

/// 归档会话
async fn cmd_archive(session_manager: &Arc<FeishuSessionManager>, session_id: &str) {
    if session_manager.archive_session(session_id).await {
        println!("✅ 会话已归档: {}", session_id);
    } else {
        println!("❌ 会话不存在: {}", session_id);
    }
}

/// 删除会话
async fn cmd_delete(session_manager: &Arc<FeishuSessionManager>, session_id: &str) {
    if session_manager.delete_session(session_id).await {
        println!("🗑️  会话已删除: {}", session_id);
    } else {
        println!("❌ 会话不存在: {}", session_id);
    }
}

/// 打印帮助信息
fn print_help() {
    println!("📖 命令帮助:");
    println!();
    println!("📋 列出会话:");
    println!("  list              - 列出所有会话（包括归档）");
    println!("  list-active       - 仅列出活跃会话");
    println!();
    println!("🔍 查看会话:");
    println!("  show <session_id>  - 显示会话详情和对话历史");
    println!("  search <query>    - 按名称或ID搜索会话");
    println!();
    println!("📁 管理会话:");
    println!("  archive <id>      - 归档会话");
    println!("  delete <id>       - 永久删除会话");
    println!();
    println!("💡 提示:");
    println!("  - 会话 ID 格式: feishu_oc_xxxxx");
    println!("  - 按 Tab 键可以自动补全命令");
    println!("  - 使用上下箭头查看历史命令");
    println!();
    println!("💼 示例:");
    println!("  feishu> list-active");
    println!("  feishu> show feishu_oc_a1b2c3d4");
    println!("  feishu> search 测试");
    println!("  feishu> archive feishu_oc_a1b2c3d4");
}
