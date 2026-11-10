//! AgentDeck command-line interface. Mutating operations require an explicit --yes.
use std::path::{Path, PathBuf};

use aisync_core::{config, deploy, dsh, git, qoder, qwen, registry, sync};
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
    /// Inspect DSH manifests or fixed offline evidence without executing or changing it.
    Dsh {
        #[command(subcommand)]
        action: DshAction,
    },
    /// Inspect an explicit Qwen settings file using a narrowly reviewed package contract.
    Qwen {
        #[command(subcommand)]
        action: QwenAction,
    },
    /// Inspect explicitly selected Qoder CLI scopes without executing or modifying them.
    Qoder {
        #[command(subcommand)]
        action: QoderAction,
    },
    /// Sync personalization and opted-in memory to explicitly selected agents.
    #[command(
        long_about = "Sync personalization and opted-in memory to explicitly selected agents. Use --dry-run for a read-only JSON metadata plan without executing agents, creating backups/profiles, or reading configuration and memory contents. Exit 0 for a dry run means a report was produced, not that installation, paths, actual changes, or apply readiness were verified. --yes is required for writes and conflicts with --dry-run."
    )]
    Sync {
        #[arg(short, long, value_delimiter = ',', required = true, num_args = 1..)]
        tools: Vec<String>,
        /// Emit a read-only metadata plan as JSON; does not validate installation or file contents.
        #[arg(long, conflicts_with = "yes")]
        dry_run: bool,
        /// Confirm configuration writes (not available with --dry-run).
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
enum QoderAction {
    /// Read-only cache state and preservation plan, never deletion or repair authorization.
    #[command(
        long_about = "Linux x86_64 only. Observe an explicit Qoder 1.1.85 baseline staged or unpacked cache. Fixed record contracts and complete archive/payload hashes are rechecked. No locks, writes, cleanup, execution or PATH changes. Exit zero reports an observation, including conflict/unknown ownership, not readiness or permission to delete. Reads may update access times; same-user races are not an atomic guarantee."
    )]
    InspectCache {
        #[arg(long)]
        cache: PathBuf,
    },
    /// Strictly unpack a pinned staged cache to a NEW private non-executable directory.
    #[command(
        long_about = "Linux x86_64 only: validate the explicit staged Qoder 1.1.85 baseline cache and extract its single pinned payload into a NEW private directory. Requires --yes. No execution, activation, PATH, login or configuration changes. Completion means extracted cache only, not installed. Records are not authorization; fixed archive and payload hashes are independently rechecked. Existing destinations are never overwritten. Failure may retain partial output; inspect before manual cleanup, never retry in place."
    )]
    UnpackCache {
        #[arg(long)]
        cache: PathBuf,
        #[arg(long)]
        destination: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    /// Stage a pinned archive in a NEW private directory; never unpack, install or execute.
    #[command(
        long_about = "Create a new explicit private archive cache, only for linux-amd64-baseline@1.1.85 on Linux x86_64. Requires --yes. Existing destinations are never reused or overwritten. Copies and hashes the same audited source handle, then rechecks the private copy. No unpacking, execution, PATH or configuration writes. Failure may retain an incomplete destination; inspect before manual cleanup. Completion means staged archive only, not installed/runtime-ready. Same-user hostile races and mount I/O deadlines are not solved."
    )]
    StagePackage {
        #[arg(long, value_parser = [qoder::staging::TARGET])]
        target: String,
        #[arg(long)]
        archive: PathBuf,
        #[arg(long)]
        destination: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    /// Print a pinned native distribution plan; never download, install or execute.
    DistributionPlan {
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(qoder::distribution::TARGETS.iter().copied()))]
        target: String,
    },
    /// Hash an explicit archive against pinned upstream SHA256. No unpacking or execution.
    #[command(
        long_about = "Check an explicit local archive against the fixed reviewed Qoder 1.1.85 target SHA256. Does not download, unpack, execute, install, modify PATH or configuration. Requires the official archive basename. Exit 0 means only a digest match; NOT independent signature verification, installation or runtime compatibility. Unknown target, unsupported/unsafe file, oversized input and mismatch exit nonzero. Only audited Windows/Linux readers are supported; access time may change and same-user races/mount I/O deadlines are not solved."
    )]
    VerifyPackage {
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(qoder::distribution::TARGETS.iter().copied()))]
        target: String,
        #[arg(long, value_name = "ABSOLUTE_OFFICIAL_ARCHIVE")]
        archive: PathBuf,
    },
    /// Summarize selected scopes and repeated MCP names, without exposing names/values.
    #[command(
        long_about = "Read explicitly selected Qoder CLI settings and project MCP files. Emits fixed redacted section flags, server counts and potential cross-scope overlap only. No execution, discovery, initialization, writes, backup or BYOK editing. This is not effective configuration, version verification, a diff or apply readiness. Exit 0 means a report was produced (including unavailable/unsupported observations); invalid selections exit with an error."
    )]
    Inspect {
        #[arg(long, value_name = "ABSOLUTE_SETTINGS_JSON")]
        user_settings: Option<PathBuf>,
        #[arg(long, value_name = "ABSOLUTE_SETTINGS_JSON")]
        project_settings: Option<PathBuf>,
        #[arg(long, value_name = "ABSOLUTE_SETTINGS_LOCAL_JSON")]
        local_settings: Option<PathBuf>,
        #[arg(long, value_name = "ABSOLUTE_DOT_MCP_JSON")]
        project_mcp: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum QwenAction {
    /// Preview one explicit non-credential field; no writes.
    VimPreview {
        #[arg(long, value_parser = parse_qwen_settings)]
        settings: PathBuf,
        #[arg(long, value_parser = parse_qwen_manifest)]
        runtime_manifest: PathBuf,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
    },
    /// Apply general.vimMode with a current preview token and field-only recovery receipt.
    VimApply {
        #[arg(long, value_parser = parse_qwen_settings)]
        settings: PathBuf,
        #[arg(long, value_parser = parse_qwen_manifest)]
        runtime_manifest: PathBuf,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
        #[arg(long)]
        preview_token: String,
        #[arg(long, required = true)]
        yes: bool,
    },
    /// Preview an already-owned Vim update; preserve the first original value.
    VimUpdatePreview {
        #[arg(long, value_parser = parse_qwen_settings)]
        settings: PathBuf,
        #[arg(long, value_parser = parse_qwen_manifest)]
        runtime_manifest: PathBuf,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
    },
    /// Update owned general.vimMode with a separate update token; retain original recovery.
    VimUpdate {
        #[arg(long, value_parser = parse_qwen_settings)]
        settings: PathBuf,
        #[arg(long, value_parser = parse_qwen_manifest)]
        runtime_manifest: PathBuf,
        #[arg(long, action = clap::ArgAction::Set)]
        enabled: bool,
        #[arg(long)]
        preview_token: String,
        #[arg(long, required = true)]
        yes: bool,
    },
    /// Inspect recovery state without writes or lock creation; not permission to restore.
    #[command(
        long_about = "Inspect explicitly selected Qwen settings and the field-only recovery receipt without writes, lock creation or Agent execution. Exit 0 means an observation was produced, including no_receipt or conflict; it is not a restore authorization or write-readiness guarantee. Invalid, unsafe or mismatched files return an error. Restore always rechecks under its lock."
    )]
    VimRecovery {
        #[arg(long, value_parser = parse_qwen_settings)]
        settings: PathBuf,
        #[arg(long, value_parser = parse_qwen_manifest)]
        runtime_manifest: PathBuf,
    },
    /// Restore only the owned field, refusing detected drift; stop other editors first.
    VimRestore {
        #[arg(long, value_parser = parse_qwen_settings)]
        settings: PathBuf,
        #[arg(long, value_parser = parse_qwen_manifest)]
        runtime_manifest: PathBuf,
        #[arg(long, required = true)]
        yes: bool,
    },
    /// Read fixed section presence only; never executes Qwen, writes files or returns values.
    #[command(
        long_about = "Inspect explicitly selected Qwen package/settings files and emit fixed, redacted JSON observations. Only the reviewed package and settings schema are eligible for content inspection. No execution, environment expansion, credential discovery, initialization, backup or writes. This is not effective configuration, installation/authentication verification, a diff or a restore plan. Exit 0 means a report was produced, including unsupported or unavailable observations; invalid paths exit 2."
    )]
    Inspect {
        #[arg(long, value_name = "ABSOLUTE_SETTINGS_JSON", value_parser = parse_qwen_settings)]
        settings: PathBuf,
        #[arg(long, value_name = "ABSOLUTE_PACKAGE_JSON", value_parser = parse_qwen_manifest)]
        runtime_manifest: Option<PathBuf>,
    },
}

fn parse_qwen_settings(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    qwen::validate_settings_path(&path)
        .map_err(|_| "invalid explicit local settings.json path".to_string())?;
    Ok(path)
}

fn parse_qwen_manifest(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    qwen::validate_runtime_manifest_path(&path)
        .map_err(|_| "invalid explicit local package.json path".to_string())?;
    Ok(path)
}

#[derive(Subcommand)]
enum DshAction {
    /// Fixed offline dependency evidence, not a resolved tree or installation plan.
    #[command(
        long_about = "Emit a fixed, reviewed dependency-evidence snapshot as JSON. Reads only embedded fixtures: no network, local discovery, package execution or installation. Root archive verification is historical; illustrative dependency metadata is not artifact verification or a complete closure. Exit 0 means a report was produced, not that DSH is installable. No version, source, target or path overrides are accepted."
    )]
    DependencyEvidence,
    /// Fixed prerelease root npm archive plan, not an installation or resolved dependency tree.
    DistributionPlan,
    /// Verify fixed root tgz size/SHA256/SHA512; no unpack or execution.
    VerifyPackage {
        #[arg(long, value_name = "ABSOLUTE_TGZ")]
        archive: PathBuf,
    },
    /// Emit passive inventory as JSON. Exit 0 means a report was produced, not healthy or installed.
    #[command(
        long_about = "Emit passive DSH inventory as JSON using only explicit paths and allowed package manifests. Never executes DSH, Node, npm, or plugin code, and never creates a profile. Missing or blocked evidence and unknown versions remain observations in the JSON report. Exit 0 means the report was produced, not that DSH is installed, healthy, or active; invalid arguments exit 2 before inspection, and output failures exit 1."
    )]
    Inspect {
        /// Absolute local profile directory, without '..'; no HOME-based default.
        #[arg(long, value_name = "ABSOLUTE_PATH", value_parser = parse_dsh_profile_dir)]
        profile_dir: PathBuf,
        /// Optional absolute local runtime package.json path, without '..'.
        #[arg(long, value_name = "ABSOLUTE_PACKAGE_JSON", value_parser = parse_dsh_runtime_manifest)]
        runtime_manifest: Option<PathBuf>,
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

// These checks are lexical: validate every argument before the inspection
// callback can observe any filesystem entry, including the profile directory.
fn parse_dsh_profile_dir(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    dsh::validate_profile_path(&path)
        .map_err(|issue| format!("invalid explicit local profile directory: {issue:?}"))?;
    Ok(path)
}

fn parse_dsh_runtime_manifest(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    dsh::validate_runtime_manifest_path(&path)
        .map_err(|issue| format!("invalid explicit local runtime package.json: {issue:?}"))?;
    Ok(path)
}

/// This path never uses the generic deployment, verification, registry, backup,
/// or profile-initialization flows. Observed inventory states are data, not
/// installation or health checks; successfully serializing any report is OK.
fn run_dsh_inspect_with(
    profile_dir: &Path,
    runtime_manifest: Option<&Path>,
    inspect: impl FnOnce(&Path, Option<&Path>) -> dsh::DshInventoryReport,
) -> anyhow::Result<String> {
    dsh::validate_profile_path(profile_dir)
        .map_err(|issue| anyhow::anyhow!("invalid explicit local profile directory: {issue:?}"))?;
    if let Some(path) = runtime_manifest {
        dsh::validate_runtime_manifest_path(path).map_err(|issue| {
            anyhow::anyhow!("invalid explicit local runtime package.json: {issue:?}")
        })?;
    }
    let report = inspect(profile_dir, runtime_manifest);
    Ok(serde_json::to_string_pretty(&report)?)
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
                "{} has no supported profile configuration adapter",
                definition.id
            );
            // Shared rules apply before profile preparation and again in core.
            deploy::validate_configuration_eligibility(definition, &inspect(definition))?;
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

/// Validate the entire personalization selection before any backup, profile
/// initialization or write. Memory-only export stays independent of installation.
fn run_sync_with(
    agents: &[registry::AgentDefinition],
    mut inspect: impl FnMut(&registry::AgentDefinition) -> deploy::InstallationDetails,
    backup: impl FnOnce() -> aisync_core::Result<()>,
    mut personalization: impl FnMut(&str) -> aisync_core::Result<()>,
    mut memory: impl FnMut(&str) -> aisync_core::Result<()>,
) -> anyhow::Result<()> {
    for agent in agents {
        anyhow::ensure!(
            config::supports_personalization(agent) || agent.config_inject.memory,
            "{} has no supported personalization or memory adapter",
            agent.id
        );
        if config::supports_personalization(agent) {
            deploy::validate_configuration_eligibility(agent, &inspect(agent))?;
        }
    }
    backup()?;
    for agent in agents {
        if config::supports_personalization(agent) {
            personalization(&agent.id)?;
        } else if agent.config_inject.personalization.is_some() {
            eprintln!(
                "Warning: {} personalization has no supported destination; skipped",
                agent.id
            );
        }
        if agent.config_inject.memory {
            memory(&agent.id)?;
        }
        println!("{}: synchronized", agent.id);
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
                    if deploy::can_install(&agent) {
                        "automatic installation"
                    } else {
                        "manual installation"
                    }
                );
            }
        }
        Commands::Dsh {
            action: DshAction::DependencyEvidence,
        } => {
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&dsh::dependency_evidence::report()?)?
            )?;
        }
        Commands::Dsh {
            action: DshAction::DistributionPlan,
        } => {
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&dsh::distribution::plan())?
            )?;
        }
        Commands::Dsh {
            action: DshAction::VerifyPackage { archive },
        } => {
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&dsh::distribution::verify_package(&archive)?)?
            )?;
        }
        Commands::Dsh {
            action:
                DshAction::Inspect {
                    profile_dir,
                    runtime_manifest,
                },
        } => {
            let report = run_dsh_inspect_with(
                &profile_dir,
                runtime_manifest.as_deref(),
                dsh::inspect_profile,
            )?;
            use std::io::Write;
            writeln!(std::io::stdout().lock(), "{report}")?;
        }
        Commands::Qoder {
            action: QoderAction::InspectCache { cache },
        } => {
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&qoder::cache_inspection::inspect_cache(&cache)?)?
            )?;
        }
        Commands::Qoder {
            action:
                QoderAction::UnpackCache {
                    cache,
                    destination,
                    yes,
                },
        } => {
            require_confirmation(yes)?;
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&qoder::unpacking::unpack_cache(
                    &cache,
                    &destination
                )?)?
            )?;
        }
        Commands::Qoder {
            action:
                QoderAction::StagePackage {
                    target,
                    archive,
                    destination,
                    yes,
                },
        } => {
            require_confirmation(yes)?;
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&qoder::staging::stage_package(
                    &target,
                    &archive,
                    &destination
                )?)?
            )?;
        }
        Commands::Qoder {
            action: QoderAction::DistributionPlan { target },
        } => {
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&qoder::distribution::plan(&target)?)?
            )?;
        }
        Commands::Qoder {
            action: QoderAction::VerifyPackage { target, archive },
        } => {
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&qoder::distribution::verify_package(
                    &target, &archive
                )?)?
            )?;
        }
        Commands::Qoder {
            action:
                QoderAction::Inspect {
                    user_settings,
                    project_settings,
                    local_settings,
                    project_mcp,
                },
        } => {
            let selections = qoder::Selections {
                user_settings: user_settings.as_deref(),
                project_settings: project_settings.as_deref(),
                local_settings: local_settings.as_deref(),
                project_mcp: project_mcp.as_deref(),
            };
            qoder::validate_selections(&selections)
                .map_err(|issue| anyhow::anyhow!("invalid Qoder selection: {issue:?}"))?;
            let report = qoder::inspect_scopes(&selections);
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&report)?
            )?;
        }
        Commands::Qwen {
            action:
                QwenAction::VimPreview {
                    settings,
                    runtime_manifest,
                    enabled,
                },
        } => {
            let preview = qwen::owned::preview(&settings, &runtime_manifest, enabled)?;
            println!("{}", serde_json::to_string_pretty(&preview)?);
        }
        Commands::Qwen {
            action:
                QwenAction::VimApply {
                    settings,
                    runtime_manifest,
                    enabled,
                    preview_token,
                    yes,
                },
        } => {
            anyhow::ensure!(yes, "--yes is required");
            let changed =
                qwen::owned::apply(&settings, &runtime_manifest, enabled, &preview_token)?;
            println!(
                "{}",
                serde_json::json!({"changed": changed, "field": "general.vimMode"})
            );
        }
        Commands::Qwen {
            action:
                QwenAction::VimUpdatePreview {
                    settings,
                    runtime_manifest,
                    enabled,
                },
        } => {
            let preview = qwen::owned::update_preview(&settings, &runtime_manifest, enabled)?;
            println!("{}", serde_json::to_string_pretty(&preview)?);
        }
        Commands::Qwen {
            action:
                QwenAction::VimUpdate {
                    settings,
                    runtime_manifest,
                    enabled,
                    preview_token,
                    yes,
                },
        } => {
            anyhow::ensure!(yes, "--yes is required");
            let changed =
                qwen::owned::update(&settings, &runtime_manifest, enabled, &preview_token)?;
            println!(
                "{}",
                serde_json::json!({"changed": changed, "field": "general.vimMode"})
            );
        }
        Commands::Qwen {
            action:
                QwenAction::VimRecovery {
                    settings,
                    runtime_manifest,
                },
        } => {
            let report = qwen::owned::recovery_preview(&settings, &runtime_manifest)?;
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&report)?
            )?;
        }
        Commands::Qwen {
            action:
                QwenAction::VimRestore {
                    settings,
                    runtime_manifest,
                    yes,
                },
        } => {
            anyhow::ensure!(yes, "--yes is required");
            let changed = qwen::owned::restore(&settings, &runtime_manifest)?;
            println!(
                "{}",
                serde_json::json!({"changed": changed, "field": "general.vimMode"})
            );
        }
        Commands::Qwen {
            action:
                QwenAction::Inspect {
                    settings,
                    runtime_manifest,
                },
        } => {
            // Deliberately independent of registry, profile, backup and process paths.
            let report = qwen::inspect_settings(&settings, runtime_manifest.as_deref());
            use std::io::Write;
            writeln!(
                std::io::stdout().lock(),
                "{}",
                serde_json::to_string_pretty(&report)?
            )?;
        }
        Commands::Sync {
            tools,
            yes,
            dry_run,
        } => {
            if dry_run {
                // Do not use find_agent: the normal registry loader may create
                // a lock and recover an interrupted update even on a read.
                let doc = registry::load_registry_read_only()?;
                let agents = tools
                    .iter()
                    .map(|id| {
                        doc.agents
                            .get(id)
                            .cloned()
                            .ok_or_else(|| anyhow::anyhow!("Unknown agent: {id}"))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let report = sync::plan_sync(&doc, &agents);
                use std::io::Write;
                writeln!(
                    std::io::stdout().lock(),
                    "{}",
                    serde_json::to_string_pretty(&report)?
                )?;
                return Ok(());
            }
            require_confirmation(yes)?;
            // Validate the whole selection before changing any selected agent.
            let agents = tools
                .iter()
                .map(|id| find_agent(id))
                .collect::<anyhow::Result<Vec<_>>>()?;
            run_sync_with(
                &agents,
                deploy::inspect_installation,
                || git::backup("CLI sync configurations").map(|_| ()),
                sync::sync_personalization,
                sync::sync_memory_rules,
            )?;
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

    fn run_dsh_args_with(
        args: &[&str],
        inspect: impl FnOnce(&Path, Option<&Path>) -> dsh::DshInventoryReport,
    ) -> anyhow::Result<String> {
        let Commands::Dsh {
            action:
                DshAction::Inspect {
                    profile_dir,
                    runtime_manifest,
                },
        } = Cli::try_parse_from(args)?.command
        else {
            panic!("expected the DSH command")
        };
        run_dsh_inspect_with(&profile_dir, runtime_manifest.as_deref(), inspect)
    }

    struct DshFixture(PathBuf);

    impl DshFixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "agentdeck-dsh-cli-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for DshFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn invalid_dsh_arguments_fail_before_any_inspection() {
        // Merely construct paths here: malformed commands must not even check
        // whether this profile exists before rejecting all arguments.
        let profile = std::env::temp_dir().join("dsh-cli-unobserved-profile");
        let profile = profile.to_str().unwrap();
        let parent = std::env::temp_dir().join("../dsh-cli-unobserved-profile");
        let parent = parent.to_str().unwrap();
        let non_manifest = std::env::temp_dir().join("cordis.yml");
        let non_manifest = non_manifest.to_str().unwrap();
        let oversized_manifest = std::env::temp_dir()
            .join("x".repeat(dsh::LIMITS.max_path_bytes))
            .join("package.json");
        let oversized_manifest = oversized_manifest.to_str().unwrap();
        let mut too_deep_profile = std::env::temp_dir();
        for _ in 0..dsh::LIMITS.max_path_components {
            too_deep_profile.push("segment");
        }
        let too_deep_profile = too_deep_profile.to_str().unwrap();
        for args in [
            vec!["aisync", "dsh"],
            vec!["aisync", "dsh", "inspect"],
            vec!["aisync", "dsh", "inspect", "--profile-dir"],
            vec!["aisync", "dsh", "inspect", "--profile-dir", ""],
            vec!["aisync", "dsh", "inspect", "--profile-dir", "relative"],
            vec!["aisync", "dsh", "inspect", "--profile-dir", "~/dsh"],
            vec!["aisync", "dsh", "inspect", "--profile-dir", parent],
            vec![
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                too_deep_profile,
            ],
            vec![
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                profile,
                "--runtime-manifest",
            ],
            vec![
                "aisync",
                "dsh",
                "inspect",
                "--runtime-manifest",
                non_manifest,
            ],
            vec![
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                profile,
                "--runtime-manifest",
                "package.json",
            ],
            vec![
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                profile,
                "--runtime-manifest",
                non_manifest,
            ],
            vec![
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                profile,
                "--runtime-manifest",
                oversized_manifest,
            ],
            vec![
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                profile,
                "--profile-dir",
                profile,
            ],
            vec![
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                profile,
                "--unknown",
            ],
            vec![
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                profile,
                "--yes",
            ],
            vec!["aisync", "dsh", "install", "--profile-dir", profile],
            vec!["aisync", "dsh", "verify", "--profile-dir", profile],
            vec!["aisync", "dsh", "configure", "--profile-dir", profile],
        ] {
            let error = run_dsh_args_with(&args, |_, _| {
                panic!("invalid arguments must not observe the filesystem")
            })
            .unwrap_err();
            let parse_error = error.downcast_ref::<clap::Error>().unwrap();
            assert_eq!(parse_error.exit_code(), 2, "{args:?}");
        }
    }

    #[test]
    fn dsh_dispatch_revalidates_all_paths_before_inspection() {
        let profile = std::env::temp_dir().join("dsh-cli-unobserved-profile");
        let invalid_manifest = std::env::temp_dir().join("credentials.json");
        let parent_manifest = std::env::temp_dir().join("runtime/../package.json");
        for (profile, runtime) in [
            (Path::new("relative"), None),
            (profile.as_path(), Some(Path::new("package.json"))),
            (profile.as_path(), Some(invalid_manifest.as_path())),
            (profile.as_path(), Some(parent_manifest.as_path())),
        ] {
            assert!(
                run_dsh_inspect_with(profile, runtime, |_, _| {
                    panic!("all paths must be valid before inspecting any of them")
                })
                .is_err()
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn dsh_rejects_unc_and_device_paths_before_inspection() {
        for path in [
            r"\\server\share\profile",
            r"\\?\C:\profile",
            r"\\.\C:\profile",
            r"C:\NUL\profile",
            r"C:\profile:stream",
            r"C:\profile.",
            r"C:\profile ",
        ] {
            let error = run_dsh_args_with(
                &["aisync", "dsh", "inspect", "--profile-dir", path],
                |_, _| panic!("network and device paths must not be inspected"),
            )
            .unwrap_err();
            assert_eq!(error.downcast_ref::<clap::Error>().unwrap().exit_code(), 2);
        }
    }

    #[test]
    fn dsh_help_explains_read_only_and_report_exit_semantics() {
        let error = run_dsh_args_with(&["aisync", "dsh", "inspect", "--help"], |_, _| {
            panic!("help must not inspect anything")
        })
        .unwrap_err();
        let error = error.downcast_ref::<clap::Error>().unwrap();
        assert_eq!(error.exit_code(), 0);
        let help = error.to_string();
        for phrase in [
            "Never executes DSH, Node, npm, or plugin code",
            "never creates a profile",
            "Exit 0 means the report was produced",
            "invalid arguments exit 2 before inspection",
            "output failures exit 1",
            "--profile-dir",
            "--runtime-manifest",
        ] {
            assert!(help.contains(phrase), "missing help text: {phrase}");
        }
    }

    #[test]
    fn dsh_missing_profile_produces_json_without_creating_it_or_discovering_runtime() {
        let fixture = DshFixture::new();
        let profile = fixture.0.join("missing-profile");
        let called = std::cell::Cell::new(false);
        let output = run_dsh_args_with(
            &[
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                profile.to_str().unwrap(),
            ],
            |selected_profile, runtime| {
                called.set(true);
                assert_eq!(selected_profile, profile);
                assert!(runtime.is_none(), "no ambient runtime discovery");
                dsh::inspect_profile(selected_profile, runtime)
            },
        )
        .unwrap();
        assert!(called.get());
        let report: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(report["profile_manifest"]["status"], "missing");
        assert!(report["runtime_manifest"].is_null());
        assert!(!profile.exists());
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 0);
    }

    #[test]
    fn dsh_explicit_inspection_serializes_the_entire_report_without_writes() {
        let fixture = DshFixture::new();
        let profile = fixture.0.join("profile");
        let runtime = fixture.0.join("runtime");
        std::fs::create_dir(&profile).unwrap();
        std::fs::create_dir(&runtime).unwrap();
        let runtime_manifest = runtime.join("package.json");
        let files = [
            (
                profile.join("package.json"),
                r#"{"dependencies":{"example-plugin":"^1.0.0"},"scripts":{"start":"exit 97"}}"#,
            ),
            (
                runtime_manifest.clone(),
                r#"{"name":"@deepseek-ai/dsh-app-boot","version":"999.0.0","engines":{"node":">=99"},"scripts":{"postinstall":"exit 97"}}"#,
            ),
            (profile.join("cordis.yml"), "DO_NOT_INCLUDE_CONFIG"),
            (profile.join(".env"), "DO_NOT_INCLUDE_ENV"),
            (
                profile.join("credentials.json"),
                "DO_NOT_INCLUDE_CREDENTIALS",
            ),
        ];
        for (path, content) in &files {
            std::fs::write(path, content).unwrap();
        }
        let expected =
            serde_json::to_value(dsh::inspect_profile(&profile, Some(&runtime_manifest))).unwrap();
        let calls = std::cell::Cell::new(0);
        let output = run_dsh_args_with(
            &[
                "aisync",
                "dsh",
                "inspect",
                "--profile-dir",
                profile.to_str().unwrap(),
                "--runtime-manifest",
                runtime_manifest.to_str().unwrap(),
            ],
            |selected_profile, selected_runtime| {
                calls.set(calls.get() + 1);
                assert_eq!(selected_profile, profile);
                assert_eq!(selected_runtime, Some(runtime_manifest.as_path()));
                dsh::inspect_profile(selected_profile, selected_runtime)
            },
        )
        .unwrap();
        assert_eq!(calls.get(), 1);
        let report: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(
            report, expected,
            "the CLI must not reinterpret observed states"
        );
        assert_eq!(
            report["runtime_manifest"]["metadata"]["reported_version"],
            "999.0.0"
        );
        assert_eq!(report["mode"], "passive_manifest_only");
        assert_eq!(
            report["version_recognition"],
            "unrecognized_manifest_version"
        );
        for field in ["runtime_status", "authentication", "actual_enabled"] {
            assert_eq!(report[field], "unknown");
        }
        let mutations = report["mutation_capabilities"].as_object().unwrap();
        assert!(!mutations.is_empty());
        assert!(mutations.values().all(|value| value == false));
        for (path, content) in files {
            assert_eq!(std::fs::read_to_string(path).unwrap(), content);
        }
        assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 2);
        assert_eq!(std::fs::read_dir(&profile).unwrap().count(), 4);
        assert_eq!(std::fs::read_dir(&runtime).unwrap().count(), 1);
        assert!(!output.contains("DO_NOT_INCLUDE"));
        assert!(!output.contains("exit 97"));
    }

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
        assert!(error.to_string().contains("Multiple installation targets"));
    }

    #[test]
    fn sync_validates_all_agents_before_backup_profile_or_write() {
        for (status, ambiguous) in [
            (deploy::InstallationStatus::NotDetected, false),
            (deploy::InstallationStatus::Error, false),
            (deploy::InstallationStatus::Unsupported, false),
            (deploy::InstallationStatus::Installed, true),
        ] {
            let mut inspected = 0;
            assert!(
                run_sync_with(
                    &[configurable_agent(), configurable_agent()],
                    |_| {
                        inspected += 1;
                        if inspected == 1 {
                            fake_details(deploy::InstallationStatus::Installed, false)
                        } else {
                            fake_details(status, ambiguous)
                        }
                    },
                    || panic!("Blocked batch must not create a backup"),
                    |_| panic!(
                        "Blocked batch must not initialize profiles or write personalization"
                    ),
                    |_| panic!("Blocked batch must not export memory"),
                )
                .is_err()
            );
            assert_eq!(inspected, 2);
        }
    }

    #[test]
    fn memory_only_sync_does_not_acquire_an_installation_requirement() {
        let mut agent = fake_agent();
        agent.config_inject.memory = true;
        let exported = std::cell::Cell::new(false);
        run_sync_with(
            &[agent],
            |_| panic!("Memory-only export does not inspect software"),
            || Ok(()),
            |_| panic!("No personalization adapter"),
            |_| {
                exported.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(exported.get());
    }

    #[test]
    fn qoder_cache_inspection_is_explicit_read_only() {
        let cli = Cli::try_parse_from([
            "aisync",
            "qoder",
            "inspect-cache",
            "--cache",
            "/selected/cache",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Qoder {
                action: QoderAction::InspectCache { .. }
            }
        ));
        assert!(Cli::try_parse_from(["aisync", "qoder", "inspect-cache"]).is_err());
        assert!(
            Cli::try_parse_from([
                "aisync",
                "qoder",
                "inspect-cache",
                "--cache",
                "/selected/cache",
                "--yes"
            ])
            .is_err()
        );
    }

    #[test]
    fn sync_accepts_comma_separated_targets() {
        let cli = Cli::try_parse_from(["aisync", "sync", "--tools", "codex,claude-code", "--yes"])
            .unwrap();
        let Commands::Sync {
            tools,
            yes,
            dry_run,
        } = cli.command
        else {
            panic!("wrong command")
        };
        assert_eq!(tools, ["codex", "claude-code"]);
        assert!(!dry_run);
        assert!(yes);
    }
}
