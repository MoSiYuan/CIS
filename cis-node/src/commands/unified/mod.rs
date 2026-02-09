//! # CIS 统一 CLI 命令
//!
//! 简化用户交互，提供一键式操作。

pub mod join;
pub mod setup;
pub mod status;

use clap::Subcommand;

/// 统一命令入口
#[derive(Debug, Subcommand)]
pub enum UnifiedCommands {
    /// 一键初始化 CIS（替代复杂的 init + 配置）
    #[command(name = "setup")]
    Setup {
        /// 全自动模式，无交互
        #[arg(long)]
        auto: bool,
        /// 指定节点角色
        #[arg(long, value_enum, default_value = "worker")]
        role: NodeRole,
    },
    
    /// 一键加入/创建网络（替代 pair + neighbor）
    #[command(name = "join")]
    Join {
        /// 指定目标地址（可选，默认自动发现）
        #[arg(long, short)]
        address: Option<String>,
        /// 使用配对码（可选，默认自动生成）
        #[arg(long, short)]
        code: Option<String>,
    },
    
    /// 统一状态查看（替代多个 status 命令）
    #[command(name = "status")]
    Status {
        /// 显示详细网络信息
        #[arg(long)]
        network: bool,
        /// 显示性能指标
        #[arg(long)]
        perf: bool,
    },
    
    /// 智能执行自然语言命令
    #[command(name = "do")]
    Do {
        /// 自然语言描述
        command: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum NodeRole {
    Coordinator,
    Worker,
    Edge,
}

impl UnifiedCommands {
    pub async fn execute(&self) -> anyhow::Result<()> {
        match self {
            UnifiedCommands::Setup { auto, role } => {
                setup::execute(*auto, *role).await
            }
            UnifiedCommands::Join { address, code } => {
                join::execute(address.clone(), code.clone()).await
            }
            UnifiedCommands::Status { network, perf } => {
                status::execute(*network, *perf).await
            }
            UnifiedCommands::Do { command } => {
                let cmd = command.join(" ");
                do_natural_language(&cmd).await
            }
        }
    }
}

/// 命令处理入口
pub async fn handle(command: UnifiedCommands) -> anyhow::Result<()> {
    command.execute().await
}

/// 自然语言命令解析
async fn do_natural_language(command: &str) -> anyhow::Result<()> {
    let cmd = command.to_lowercase();
    
    // 简单的意图识别
    if cmd.contains("组网") || cmd.contains("join") || cmd.contains("连接") {
        println!("🤖 理解为: 加入网络");
        join::execute(None, None).await
    } else if cmd.contains("初始化") || cmd.contains("setup") || cmd.contains("安装") {
        println!("🤖 理解为: 初始化 CIS");
        setup::execute(true, NodeRole::Worker).await
    } else if cmd.contains("状态") || cmd.contains("status") || cmd.contains("查看") {
        println!("🤖 理解为: 查看状态");
        status::execute(true, false).await
    } else {
        println!("❓ 未能理解的命令: {}", command);
        println!("💡 尝试使用: cis join / cis setup / cis status");
        Ok(())
    }
}
