//! AgentDeck command-line interface. Mutating operations require an explicit --yes.
use std::path::PathBuf;

use aisync_core::{config, deploy, git, registry, sync};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "aisync", about = "AgentDeck CLI", version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, ValueEnum)]
enum DeployAction {
    Install,
    Uninstall,
    Verify,
}

#[derive(Subcommand)]
enum Commands {
    /// List supported agents and installation capabilities (does not run installers).
    Agents,
    /// Sync personalization and opted-in memory to explicitly selected agents.
    Sync {
        #[arg(short, long, value_delimiter = ',', required = true, num_args = 1..)]
        tools: Vec<String>,
        /// Confirm configuration writes.
        #[arg(long)]
        yes: bool,
    },
    /// Install, uninstall or verify one registered agent.
    Deploy {
        #[arg(short, long)]
        agent: String,
        #[arg(short = 'x', long, value_enum)]
        action: DeployAction,
        /// Confirm execution of the registry installation/uninstallation command.
        #[arg(long)]
        yes: bool,
    },
    /// Create an encrypted configuration backup.
    Backup {
        #[arg(long, default_value = "CLI manual backup")]
        reason: String,
    },
    /// List encrypted backup history.
    Backups,
    /// Restore a selected backup; unmanaged files and encryption keys are preserved.
    Restore {
        timestamp: String,
        #[arg(long)]
        full: bool,
        #[arg(long)]
        yes: bool,
    },
    /// Validate the registry and report platform support without modifying data.
    Health,
    /// Publisher tools. Signing keys stay in the platform-protected key store.
    Release {
        #[command(subcommand)]
        action: ReleaseAction,
    },
}

#[derive(Subcommand)]
enum ReleaseAction {
    Init,
    Manifest {
        #[arg(long)]
        version: String,
        #[arg(long)]
        min_version: String,
        #[arg(long)]
        base: String,
        #[arg(long)]
        out: PathBuf,
        /// Release date (YYYY-MM-DD); defaults to today's UTC date.
        #[arg(long)]
        released: Option<String>,
        #[arg(long = "file", value_name = "NAME=PATH", required = true)]
        files: Vec<String>,
    },
}

fn require_confirmation(yes: bool) -> anyhow::Result<()> {
    anyhow::ensure!(yes, "此操作将修改本机配置或软件；检查目标后添加 --yes 确认");
    Ok(())
}

fn find_agent(id: &str) -> anyhow::Result<registry::AgentDefinition> {
    registry::load_registry_runtime()?
        .agents
        .remove(id)
        .ok_or_else(|| anyhow::anyhow!("Agent 未找到: {id}"))
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Agents => {
            let doc = registry::load_registry_runtime()?;
            let mut agents: Vec<_> = doc.agents.into_values().collect();
            agents.sort_by(|a, b| a.id.cmp(&b.id));
            for agent in agents {
                println!(
                    "{}\t{}\t{}",
                    agent.id,
                    agent.name,
                    if agent
                        .install
                        .command
                        .as_deref()
                        .is_some_and(|c| !c.trim().is_empty())
                    {
                        "automatic installation"
                    } else {
                        "manual installation"
                    }
                );
            }
        }
        Commands::Sync { tools, yes } => {
            require_confirmation(yes)?;
            // Validate the whole selection before changing any selected agent.
            let agents = tools
                .iter()
                .map(|id| find_agent(id))
                .collect::<anyhow::Result<Vec<_>>>()?;
            for agent in &agents {
                anyhow::ensure!(
                    config::supports_personalization(agent) || agent.config_inject.memory,
                    "{} has no supported personalization or memory adapter",
                    agent.id
                );
            }
            git::backup("CLI sync configurations")?;
            for agent in agents {
                if config::supports_personalization(&agent) {
                    sync::sync_personalization(&agent.id)?;
                } else if agent.config_inject.personalization.is_some() {
                    eprintln!(
                        "Warning: {} personalization has no supported destination; skipped",
                        agent.id
                    );
                }
                if agent.config_inject.memory {
                    sync::sync_memory_rules(&agent.id)?;
                }
                println!("{}: synchronized", agent.id);
            }
        }
        Commands::Deploy { agent, action, yes } => {
            let definition = find_agent(&agent)?;
            match action {
                DeployAction::Verify => {
                    let installed = deploy::verify_installation(&definition)?;
                    println!(
                        "{}: installed={installed}, local_auth_evidence={}",
                        agent,
                        deploy::check_auth(&definition)?
                    );
                    anyhow::ensure!(installed, "Agent 未安装或验证失败: {agent}");
                }
                DeployAction::Install => {
                    require_confirmation(yes)?;
                    anyhow::ensure!(
                        definition.install.command.is_some(),
                        "此 Agent 仅支持手动安装"
                    );
                    let source = config::profile_source_dir()?;
                    git::backup(&format!("CLI install {agent}"))?;
                    let outcome = deploy::install_agent_with_inject(&definition, &source)?;
                    println!("{agent}: installed");
                    for warning in outcome.warnings {
                        eprintln!("Warning: {warning}");
                    }
                }
                DeployAction::Uninstall => {
                    require_confirmation(yes)?;
                    anyhow::ensure!(
                        definition.install.uninstall_cmd.is_some(),
                        "此 Agent 仅支持手动卸载"
                    );
                    git::backup(&format!("CLI uninstall {agent}"))?;
                    deploy::uninstall_agent(&definition)?;
                    println!("{agent}: uninstalled");
                }
            }
        }
        Commands::Backup { reason } => println!("{}", git::backup(&reason)?),
        Commands::Backups => println!("{}", serde_json::to_string_pretty(&git::list_backups()?)?),
        Commands::Restore {
            timestamp,
            full,
            yes,
        } => {
            require_confirmation(yes)?;
            let paths = git::restore(&timestamp, full)?;
            println!("{}", serde_json::to_string_pretty(&paths)?);
        }
        Commands::Health => {
            let doc = registry::load_registry_runtime()?;
            println!(
                "AgentDeck CLI v{} OK; registry_agents={}; protected_vault={}",
                env!("CARGO_PKG_VERSION"),
                doc.agents.len(),
                if cfg!(windows) {
                    "Windows DPAPI"
                } else {
                    "unsupported on this platform"
                }
            );
        }
        Commands::Release { action } => match action {
            ReleaseAction::Init => {
                let (pubkey, _) = aisync_core::release::load_or_create_signing_key()?;
                use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
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
                released,
                files,
            } => {
                anyhow::ensure!(base.starts_with("https://"), "发布下载 URL 必须使用 HTTPS");
                let released =
                    released.unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
                chrono::NaiveDate::parse_from_str(&released, "%Y-%m-%d")?;
                let mut entries: Vec<(String, String, PathBuf)> = Vec::new();
                let mut names = std::collections::HashSet::new();
                for file in files {
                    let (name, path) = file
                        .split_once('=')
                        .ok_or_else(|| anyhow::anyhow!("文件参数格式应为 name=path: {file}"))?;
                    anyhow::ensure!(
                        !name.is_empty()
                            && !name.contains(['/', '\\', ':'])
                            && name != "."
                            && name != "..",
                        "非法资产名称: {name}"
                    );
                    anyhow::ensure!(
                        name != "manifest.json"
                            && name != "manifest.json.sig"
                            && names.insert(name.to_string()),
                        "重复或保留资产名称: {name}"
                    );
                    anyhow::ensure!(std::path::Path::new(path).is_file(), "资产不存在: {path}");
                    entries.push((
                        name.into(),
                        format!("{}/{name}", base.trim_end_matches('/')),
                        PathBuf::from(path),
                    ));
                }
                let refs: Vec<_> = entries
                    .iter()
                    .map(|(n, u, p)| (n.as_str(), u.as_str(), p.as_path()))
                    .collect();
                let (manifest, sig) =
                    aisync_core::release::make_manifest(&version, &min_version, &released, &refs)?;
                // Never overwrite an existing publisher output directory accidentally.
                std::fs::create_dir(&out)?;
                use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
                for (name, _, path) in entries {
                    std::fs::copy(path, out.join(name))?;
                }
                std::fs::write(
                    out.join("manifest.json"),
                    aisync_core::release::canonical_manifest_json(&manifest)?,
                )?;
                std::fs::write(out.join("manifest.json.sig"), B64.encode(sig))?;
                println!("manifest 已生成: {}", out.join("manifest.json").display());
            }
        },
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    run(Cli::parse())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mutations_require_explicit_confirmation() {
        assert!(require_confirmation(false).is_err());
        assert!(require_confirmation(true).is_ok());
    }
    #[test]
    fn invalid_deploy_actions_and_missing_sync_selection_are_rejected() {
        assert!(
            Cli::try_parse_from(["aisync", "deploy", "--agent", "codex", "--action", "oops"])
                .is_err()
        );
        assert!(Cli::try_parse_from(["aisync", "sync", "--yes"]).is_err());
    }
    #[test]
    fn sync_accepts_comma_separated_targets() {
        let cli = Cli::try_parse_from(["aisync", "sync", "--tools", "codex,claude-code", "--yes"])
            .unwrap();
        let Commands::Sync { tools, yes } = cli.command else {
            panic!("wrong command")
        };
        assert_eq!(tools, ["codex", "claude-code"]);
        assert!(yes);
    }
}
