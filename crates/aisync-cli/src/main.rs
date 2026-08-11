/// AgentDeck CLI —— 命令行入口。
///
/// 子命令：sync / deploy / backup / health / version

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aisync", about = "AgentDeck CLI", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 同步 skills/MCP/个性化到指定 agent
    Sync {
        #[arg(short, long)]
        tools: Option<Vec<String>>,
    },
    /// 安装/卸载 agent
    Deploy {
        #[arg(short, long)]
        agent: String,
        #[arg(short, long)]
        action: String, // install | uninstall
    },
    /// 备份当前配置
    Backup,
    /// 健康检查
    Health,
}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    // M0: 骨架阶段，子命令均为 todo
    println!("AgentDeck CLI v{}", env!("CARGO_PKG_VERSION"));
    println!("当前为 M0 骨架版本，功能开发中...");
    Ok(())
}
