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

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum DeployAction {
    Install,
    Configure,
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
    /// Install software, configure a profile, or verify one registered agent.
    Deploy {
        #[arg(short, long)]
        agent: String,
        #[arg(short = 'x', long, value_enum)]
        action: DeployAction,
        /// Confirm the selected software or configuration change.
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

/// Keep profile preparation separate so pure software installation never needs
/// a writable profile, configuration backup, or platform-protected vault.
fn run_deploy_with(
    definition: &registry::AgentDefinition,
    action: DeployAction,
    yes: bool,
    inspect: impl FnOnce(&registry::AgentDefinition) -> deploy::InstallationDetails,
    install: impl FnOnce(&registry::AgentDefinition) -> aisync_core::Result<()>,
    prepare_profile: impl FnOnce() -> anyhow::Result<PathBuf>,
) -> anyhow::Result<()> {
    let agent = &definition.id;
    if action != DeployAction::Verify {
        require_confirmation(yes)?;
    }
    match action {
        DeployAction::Verify => {
            let details = inspect(definition);
            println!(
                "{}: installation={}, local_auth_evidence={}",
                agent,
                serde_json::to_string(&details)?,
                deploy::check_auth(definition)?
            );
            anyhow::ensure!(
                details.probe.status == deploy::InstallationStatus::Installed,
                "Installation was not verified for {agent}: {}",
                details
                    .probe
                    .message
                    .as_deref()
                    .unwrap_or("Unknown verification failure")
            );
        }
        DeployAction::Install => {
            install(definition)?;
            println!("{agent}: installed; profile configuration was not changed");
        }
        DeployAction::Configure => {
            anyhow::ensure!(
                deploy::can_configure(definition),
                "{agent} has no supported profile configuration adapter"
            );
            // Check before profile initialization or backup creation. The core
            // repeats verification immediately before applying configuration.
            let details = inspect(definition);
            anyhow::ensure!(
                !details.ambiguous,
                "Cannot configure {agent}: multiple or unresolved installation instances"
            );
            anyhow::ensure!(
                details.probe.status == deploy::InstallationStatus::Installed,
                "Cannot configure {agent}: {}",
                details
                    .probe
                    .message
                    .as_deref()
                    .unwrap_or("Installation was not verified")
            );
            let source = prepare_profile()?;
            let outcome = deploy::configure_agent(definition, &source)?;
            println!("{agent}: profile configuration applied");
            for warning in outcome.warnings {
                eprintln!("Warning: {warning}");
            }
        }
        DeployAction::Uninstall => {
            // The core must reject unknown provenance before any external
            // command or backup creation, even with --yes.
            deploy::uninstall_agent(definition)?;
            println!("{agent}: uninstalled");
        }
    }
    Ok(())
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
            run_deploy_with(
                &definition,
                action,
                yes,
                deploy::inspect_installation,
                deploy::install_agent,
                || {
                    // Profile initialization and encrypted backup belong only to
                    // explicit configuration, never to software installation.
                    git::backup(&format!("CLI configure {agent}"))?;
                    Ok(config::profile_source_dir()?)
                },
            )?;
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
    fn fake_agent() -> registry::AgentDefinition {
        serde_json::from_value(serde_json::json!({
            "id": "fake", "name": "Fake", "source": "open", "ui_type": "cli",
            "category": "test", "popular": false, "description": "Test fixture", "icon": "t",
            "install": { "method": "manual", "command": "exit 97", "verify_cmd": "echo verified" },
            "auth_type": "none", "config_inject": {}
        }))
        .unwrap()
    }

    fn configurable_agent() -> registry::AgentDefinition {
        let mut agent = fake_agent();
        agent.config_inject.personalization = Some(registry::PersonalizationInject {
            template: "synthetic".into(),
            target: "~/.synthetic-agent/AGENTS.md".into(),
        });
        agent
    }

    fn fake_details(
        status: deploy::InstallationStatus,
        ambiguous: bool,
    ) -> deploy::InstallationDetails {
        deploy::InstallationDetails {
            probe: deploy::InstallationProbe {
                status,
                message: None,
            },
            instances: Vec::new(),
            ambiguous,
            message: None,
        }
    }

    #[test]
    fn install_and_configure_are_distinct_explicit_actions() {
        for (name, expected) in [
            ("install", DeployAction::Install),
            ("configure", DeployAction::Configure),
        ] {
            let cli =
                Cli::try_parse_from(["aisync", "deploy", "--agent", "fake", "--action", name])
                    .unwrap();
            let Commands::Deploy { action, yes, .. } = cli.command else {
                panic!("wrong command")
            };
            assert_eq!(action, expected);
            assert!(!yes);
            assert!(
                run_deploy_with(
                    &fake_agent(),
                    action,
                    yes,
                    |_| panic!("No inspection before confirmation"),
                    |_| panic!("No install before confirmation"),
                    || panic!("No profile preparation before confirmation"),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn install_does_not_prepare_profile_or_require_backup() {
        let called = std::cell::Cell::new(false);
        run_deploy_with(
            &fake_agent(),
            DeployAction::Install,
            true,
            |_| panic!("The installer owns its own preflight"),
            |_| {
                called.set(true);
                Ok(())
            },
            || panic!("Install must not initialize profiles or access backup/vault"),
        )
        .unwrap();
        assert!(called.get());
    }

    #[test]
    fn configure_does_not_run_installer_and_preserves_preparation_failure() {
        let error = run_deploy_with(
            &configurable_agent(),
            DeployAction::Configure,
            true,
            |_| fake_details(deploy::InstallationStatus::Installed, false),
            |_| panic!("Configure cannot install software"),
            || Err(anyhow::anyhow!("Backup unavailable")),
        )
        .unwrap_err();
        assert!(error.to_string().contains("Backup unavailable"));
    }

    #[test]
    fn configure_without_adapter_does_not_prepare_profile() {
        let error = run_deploy_with(
            &fake_agent(),
            DeployAction::Configure,
            true,
            |_| panic!("Unsupported configuration must not inspect"),
            |_| panic!("Configure cannot install software"),
            || panic!("Unsupported configuration cannot prepare a profile or backup"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("no supported profile"));
    }

    #[test]
    fn configure_unverified_agent_and_uninstall_do_not_prepare_profile() {
        let mut agent = configurable_agent();
        agent.install.verify_cmd = None;
        for action in [DeployAction::Configure, DeployAction::Uninstall] {
            assert!(
                run_deploy_with(
                    &agent,
                    action,
                    true,
                    |_| fake_details(deploy::InstallationStatus::Unsupported, false),
                    |_| panic!("No installer may run"),
                    || panic!("No backup or profile preparation for blocked actions"),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn ambiguous_installation_cannot_prepare_profile_even_when_version_probe_succeeds() {
        let error = run_deploy_with(
            &configurable_agent(),
            DeployAction::Configure,
            true,
            |_| fake_details(deploy::InstallationStatus::Installed, true),
            |_| panic!("No installer may run"),
            || panic!("Ambiguous instances must not initialize profiles or backups"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("multiple or unresolved"));
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
