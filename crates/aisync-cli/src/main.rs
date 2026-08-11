//! AgentDeck CLI —— 命令行入口。
//!
//! 子命令：sync / deploy / backup / health / release
use std::path::PathBuf;

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
    /// 发布工具（M6）：签名密钥与 manifest 生成
    Release {
        #[command(subcommand)]
        action: ReleaseAction,
    },
}

#[derive(Subcommand)]
enum ReleaseAction {
    /// 生成/显示 ed25519 签名密钥（私钥 DPAPI 加密存 ~/.agentdeck/release/）
    Init,
    /// 生成 manifest.json + manifest.json.sig（files: 名称=路径，URL 用 --base 拼接）
    Manifest {
        #[arg(long)]
        version: String,
        #[arg(long)]
        min_version: String,
        /// 资产 URL 前缀（如 https://github.com/MOSHOULI/agentdeck/releases/download/v1.0.0）
        #[arg(long)]
        base: String,
        /// 输出目录
        #[arg(long)]
        out: PathBuf,
        /// 待发布文件：name=path
        #[arg(long = "file", value_name = "NAME=PATH")]
        files: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Sync { .. } => {
            println!("sync 子命令暂未实现（M6 后）");
        }
        Commands::Deploy { .. } => {
            println!("deploy 子命令暂未实现");
        }
        Commands::Backup => {
            println!("backup 子命令暂未实现");
        }
        Commands::Health => {
            println!("AgentDeck CLI v{} OK", env!("CARGO_PKG_VERSION"));
        }
        Commands::Release { action } => match action {
            ReleaseAction::Init => {
                let (pubkey, _) = aisync_core::release::load_or_create_signing_key()?;
                use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
                println!("签名密钥已就绪（私钥 DPAPI 加密存 ~/.agentdeck/release/）");
                println!("公钥 base64: {}", B64.encode(&pubkey));
                println!(
                    "公钥指纹: {}",
                    aisync_core::release::pubkey_fingerprint(&pubkey)
                );
            }
            ReleaseAction::Manifest {
                version,
                min_version,
                base,
                out,
                files,
            } => {
                let mut entries: Vec<(String, String, PathBuf)> = Vec::new();
                for f in &files {
                    let (name, path) = f
                        .split_once('=')
                        .ok_or_else(|| anyhow::anyhow!("文件参数格式应为 name=path: {}", f))?;
                    let url = format!("{}/{}", base.trim_end_matches('/'), name);
                    entries.push((name.to_string(), url, PathBuf::from(path)));
                }
                let refs: Vec<(&str, &str, &std::path::Path)> = entries
                    .iter()
                    .map(|(n, u, p)| (n.as_str(), u.as_str(), p.as_path()))
                    .collect();
                let (manifest, sig) = aisync_core::release::make_manifest(
                    &version,
                    &min_version,
                    "2026-08-11",
                    &refs,
                )?;
                std::fs::create_dir_all(&out)?;
                let manifest_json = serde_json::to_string_pretty(&manifest)?;
                use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
                std::fs::write(out.join("manifest.json"), manifest_json)?;
                std::fs::write(out.join("manifest.json.sig"), B64.encode(sig))?;
                for (name, _, path) in &entries {
                    std::fs::copy(path, out.join(name))?;
                }
                println!("manifest 已生成: {}", out.join("manifest.json").display());
                println!("pubkey_fp: {}", manifest.pubkey_fp);
            }
        },
    }
    Ok(())
}
