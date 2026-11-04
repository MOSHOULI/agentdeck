//! Bounded execution of trusted registry installation and verification commands.
//!
//! Callers are responsible for obtaining user confirmation before installation or
//! configuration. Removal is blocked until installation provenance is recorded.
//! Authentication checks detect local setup evidence only. Configuration
//! failures never uninstall software that may have existed before this operation.
use crate::registry::AgentDefinition;

mod inspection;
mod process_guard;
pub use inspection::{
    InstallationDetails, InstallationInstance, InstallationSource, inspect_installation,
};

use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// 执行注册表里的 shell 命令（Windows 用 cmd /C）
fn run_shell(cmdline: &str) -> std::io::Result<()> {
    run_shell_with_timeout(cmdline, Duration::from_secs(300))
}

fn run_shell_with_timeout(cmdline: &str, timeout: Duration) -> std::io::Result<()> {
    wait_for_shell_with_timeout(spawn_shell(cmdline)?, timeout)
}

fn spawn_shell(cmdline: &str) -> std::io::Result<Child> {
    if cmdline.trim().is_empty() {
        return Err(std::io::Error::other("Empty deployment command"));
    }
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("cmd.exe");
        c.creation_flags(0x08000000); // CREATE_NO_WINDOW: background enumeration must not flash consoles.
        // This is a trusted shell program, not one C-runtime argument. `arg`
        // escapes embedded quotes with backslashes, which cmd.exe does not
        // decode. /S removes only our outer quotes, preserving quoted paths and
        // nested commands. /D prevents unrelated AutoRun commands from running.
        c.args(["/D", "/S", "/C"]).raw_arg(format!("\"{cmdline}\""));
        c
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut c = Command::new("sh");
        c.args(["-c", cmdline]);
        c
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    // Never let unattended deployment wait for an interactive prompt.
    command.stdin(std::process::Stdio::null());
    command.spawn()
}

fn wait_for_shell_with_timeout(mut child: Child, timeout: Duration) -> std::io::Result<()> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(std::io::Error::other(format!(
                        "命令退出码 {}",
                        status.code().unwrap_or(-1)
                    )))
                };
            }
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(20))
            }
            result => {
                // Kill the shell's descendants as well as the shell, then reap it.
                #[cfg(unix)]
                process_guard::kill_group(child.id());
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    let _ = Command::new("taskkill")
                        .creation_flags(0x08000000) // CREATE_NO_WINDOW
                        .args(["/F", "/T", "/PID", &child.id().to_string()])
                        .status();
                }
                let _ = child.kill();
                let _ = child.wait();
                return Err(match result {
                    Err(e) => e,
                    _ => std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Deployment command timed out",
                    ),
                });
            }
        }
    }
}

/// Install only after a reliable missing-executable observation, then verify.
/// This path is not an implicit repair, upgrade or reinstall action.
pub fn install_agent(agent: &AgentDefinition) -> crate::Result<()> {
    install_agent_with(agent, probe_installation, run_shell)
}

fn install_agent_with(
    agent: &AgentDefinition,
    mut probe: impl FnMut(&AgentDefinition) -> InstallationProbe,
    run: impl FnOnce(&str) -> std::io::Result<()>,
) -> crate::Result<()> {
    let command = agent
        .install
        .command
        .as_deref()
        .filter(|cmd| !cmd.trim().is_empty())
        .ok_or_else(|| crate::Error::InstallFailed(format!("{} 缺少安装命令", agent.name)))?;
    let before = probe(agent);
    if before.status != InstallationStatus::NotDetected {
        return Err(crate::Error::InstallFailed(format!(
            "Installation is blocked for {}: {}",
            agent.name,
            before
                .message
                .as_deref()
                .unwrap_or("Already installed; reinstall or upgrade must be explicit")
        )));
    }
    run(command)
        .map_err(|e| crate::Error::InstallFailed(format!("安装 {} 失败: {}", agent.name, e)))?;
    let after = probe(agent);
    if after.status != InstallationStatus::Installed {
        return Err(crate::Error::InstallFailed(format!(
            "安装 {} 后验证失败: {}",
            agent.name,
            after
                .message
                .as_deref()
                .unwrap_or("Installation was not verified")
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct InstallOutcome {
    pub warnings: Vec<String>,
}

/// Installation remains installed even if configuration fails. Skipped adapters
/// are returned as warnings so the UI cannot silently claim complete setup.
pub fn install_agent_with_inject(
    agent: &AgentDefinition,
    source_dir: &std::path::Path,
) -> crate::Result<InstallOutcome> {
    install_agent(agent)?;
    configure_agent(agent, source_dir)
}

/// Whether a profile adapter exists for this registry definition. Capability is
/// independent of installation status; both are checked before configuration.
pub fn can_configure(agent: &AgentDefinition) -> bool {
    crate::config::supports_personalization(agent)
        || (matches!(agent.id.as_str(), "codex" | "opencode")
            && agent
                .config_inject
                .model_config
                .as_ref()
                .is_some_and(|model| {
                    !model.file.contains("://")
                        && (model.file.starts_with("~/") || model.file.starts_with("~\\"))
                        && matches!(
                            model.format.as_str(),
                            "json_merge" | "jsonc_merge" | "toml_section" | "yaml_section"
                        )
                }))
}

/// Apply profile configuration only; this never installs software. Callers obtain
/// write confirmation and any application-level backup before invoking this API.
/// The configuration adapter also retains its existing per-file backup/rollback.
pub fn configure_agent(
    agent: &AgentDefinition,
    source_dir: &std::path::Path,
) -> crate::Result<InstallOutcome> {
    configure_agent_with(agent, source_dir, crate::config::inject_for_agent)
}

fn configure_agent_with(
    agent: &AgentDefinition,
    source_dir: &std::path::Path,
    apply: impl FnOnce(&AgentDefinition, &std::path::Path) -> crate::Result<crate::config::InjectReport>,
) -> crate::Result<InstallOutcome> {
    if !can_configure(agent) {
        return Err(crate::Error::InjectFailed(format!(
            "{} has no supported profile configuration adapter",
            agent.name
        )));
    }
    let details = inspect_installation(agent);
    let probe = details.probe;
    if details.ambiguous {
        return Err(crate::Error::InjectFailed(
            "Multiple installation targets were found; choose and resolve the intended installation before configuring".into(),
        ));
    }
    if probe.status != InstallationStatus::Installed {
        return Err(crate::Error::InjectFailed(format!(
            "Cannot configure {} without successful installation verification: {}",
            agent.name,
            probe
                .message
                .as_deref()
                .unwrap_or("Installation was not verified")
        )));
    }
    let report = apply(agent, source_dir)?;
    let warnings = [
        ("Personalization", report.personalization),
        ("Model configuration", report.model_config),
    ]
    .into_iter()
    .filter_map(|(label, status)| match status {
        Some(crate::config::InjectStatus::Skipped(reason)) => Some(format!("{label}: {reason}")),
        _ => None,
    })
    .collect();
    Ok(InstallOutcome { warnings })
}

/// Registry installation methods describe available recipes, not the origin of
/// an existing installation. Until provenance is recorded, removal fails closed.
pub fn uninstall_block_reason(_agent: &AgentDefinition) -> Option<&'static str> {
    Some("自动卸载尚未实现，包括已匹配 npm 全局目录的实例；请使用原安装方式手动卸载。")
}

pub fn can_uninstall(agent: &AgentDefinition) -> bool {
    uninstall_block_reason(agent).is_none()
}

/// Never execute an uninstall recipe without recorded installation provenance.
pub fn uninstall_agent(agent: &AgentDefinition) -> crate::Result<()> {
    Err(crate::Error::InstallFailed(
        uninstall_block_reason(agent)
            .unwrap_or("Automatic uninstall is not implemented")
            .into(),
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationStatus {
    Installed,
    /// A known executable was not found in this process's command search path.
    /// This is not proof that the software is absent from the machine.
    NotDetected,
    Error,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstallationProbe {
    pub status: InstallationStatus,
    pub message: Option<String>,
}

/// Verify installation using a bounded registry command, preserving uncertainty.
pub fn probe_installation(agent: &AgentDefinition) -> InstallationProbe {
    inspect_installation(agent).probe
}

fn probe_installation_with(
    agent: &AgentDefinition,
    resolve: impl FnOnce(&str) -> std::io::Result<bool>,
    run: impl FnOnce(&str) -> std::io::Result<()>,
) -> InstallationProbe {
    let Some(command) = agent
        .install
        .verify_cmd
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
    else {
        return InstallationProbe {
            status: InstallationStatus::Unsupported,
            message: Some("No installation verification command is available".into()),
        };
    };
    // Deliberately recognize only these exact bundled recipes. Do not infer an
    // executable by splitting arbitrary shell syntax or interpreting exit codes.
    let executable = match command {
        "opencode --version" => Some("opencode"),
        "claude --version" => Some("claude"),
        "codex --version" => Some("codex"),
        "hermes --version" => Some("hermes"),
        "gemini --version" => Some("gemini"),
        "pi --version" => Some("pi"),
        _ => None,
    };
    if let Some(executable) = executable {
        match resolve(executable) {
            Ok(false) => {
                return InstallationProbe {
                    status: InstallationStatus::NotDetected,
                    message: Some(format!(
                        "{executable} was not found on this process's command search path; it may be installed elsewhere"
                    )),
                };
            }
            Err(error) => {
                return InstallationProbe {
                    status: InstallationStatus::Error,
                    message: Some(format!(
                        "Cannot inspect the executable search path: {error}"
                    )),
                };
            }
            Ok(true) => {}
        }
    }
    match run(command) {
        Ok(()) => InstallationProbe {
            status: InstallationStatus::Installed,
            message: None,
        },
        Err(error) => InstallationProbe {
            status: InstallationStatus::Error,
            message: Some(format!("Installation verification failed: {error}")),
        },
    }
}

fn executable_on_path(executable: &str) -> std::io::Result<bool> {
    let path =
        std::env::var_os("PATH").ok_or_else(|| std::io::Error::other("PATH is not available"))?;
    let directories = std::env::split_paths(&path);
    #[cfg(not(windows))]
    {
        executable_in_paths(executable, directories, &[String::new()])
    }
    #[cfg(windows)]
    {
        // cmd.exe also searches the working directory and uses PATHEXT. If its
        // search rules cannot be determined, report an error instead of absence.
        let extensions = std::env::var("PATHEXT")
            .map_err(|_| std::io::Error::other("PATHEXT is not available"))?;
        let mut suffixes = vec![String::new()];
        for extension in extensions.split(';') {
            if !extension.starts_with('.')
                || extension.len() < 2
                || !extension[1..].bytes().all(|b| b.is_ascii_alphanumeric())
            {
                return Err(std::io::Error::other("PATHEXT cannot be resolved reliably"));
            }
            suffixes.push(extension.to_string());
        }
        executable_in_paths(
            executable,
            std::iter::once(std::env::current_dir()?).chain(directories),
            &suffixes,
        )
    }
}

fn executable_in_paths(
    executable: &str,
    directories: impl IntoIterator<Item = std::path::PathBuf>,
    suffixes: &[String],
) -> std::io::Result<bool> {
    let mut failure = None;
    for directory in directories {
        for suffix in suffixes {
            let path = directory.join(format!("{executable}{suffix}"));
            // A dangling link, permission failure or non-executable candidate is
            // an inspection error, not reliable evidence of an absent program.
            match std::fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    failure = Some(error);
                    continue;
                }
                Ok(_) => {}
            }
            match std::fs::metadata(&path) {
                Ok(metadata) if metadata.is_file() => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if metadata.permissions().mode() & 0o111 == 0 {
                            failure = Some(std::io::Error::new(
                                std::io::ErrorKind::PermissionDenied,
                                "Candidate exists but is not executable",
                            ));
                            continue;
                        }
                    }
                    return Ok(true);
                }
                Ok(_) => {
                    failure = Some(std::io::Error::other("Executable candidate is not a file"))
                }
                Err(error) => failure = Some(error),
            }
        }
    }
    match failure {
        Some(error) => Err(error),
        None => Ok(false),
    }
}

/// Compatibility wrapper. New callers should use the structured probe to avoid
/// presenting errors or unsupported verification as a missing installation.
pub fn verify_installation(agent: &AgentDefinition) -> crate::Result<bool> {
    Ok(probe_installation(agent).status == InstallationStatus::Installed)
}

/// Check local authentication evidence without reading or exposing credentials.
/// File presence is only local setup evidence, not validation of remote token validity.
pub fn check_auth(agent: &AgentDefinition) -> crate::Result<bool> {
    if agent.auth_type == crate::registry::AuthType::None {
        return Ok(true);
    }
    let Some(check) = agent
        .auth_guide
        .as_ref()
        .and_then(|g| g.completion_check.as_ref())
    else {
        return Ok(false);
    };
    let Some(path) = check.get("file_exists").and_then(|v| v.as_str()) else {
        return Ok(false);
    };
    let path = if let Some(relative) = path.strip_prefix("~/") {
        let home =
            crate::home_dir().ok_or_else(|| crate::Error::Other("无法定位用户主目录".into()))?;
        home.join(relative)
    } else {
        std::path::PathBuf::from(path)
    };
    Ok(path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{AuthType, ConfigInject, InstallConfig, InstallMethod, UiType};

    fn fake_agent() -> AgentDefinition {
        AgentDefinition {
            id: "fake".into(),
            name: "Fake".into(),
            source: "open".into(),
            license: None,
            github: None,
            stars: Some(1),
            ui_type: UiType::Cli,
            category: "test".into(),
            popular: false,
            description: "测试用".into(),
            install: InstallConfig {
                method: InstallMethod::Manual,
                extension_id: None,
                command: Some("echo installed".into()),
                url: None,
                verify_cmd: Some("echo ok".into()),
                uninstall_cmd: Some("echo uninstalled".into()),
                dependencies: vec![],
            },
            auth_type: AuthType::None,
            auth_guide: None,
            config_inject: ConfigInject {
                skills: false,
                mcp: None,
                memory: false,
                personalization: None,
                model_config: None,
            },
            memory_collector: None,
            icon: "t".into(),
            homepage: None,
        }
    }

    #[test]
    fn install_and_verify_with_echo() {
        let agent = fake_agent();
        let mut probes = [
            InstallationStatus::NotDetected,
            InstallationStatus::Installed,
        ]
        .into_iter();
        install_agent_with(
            &agent,
            |_| observation(probes.next().expect("preflight and verification only")),
            run_shell,
        )
        .expect("echo 安装命令应成功");
        assert!(verify_installation(&agent).unwrap());
        assert!(!can_uninstall(&agent));
        assert!(uninstall_agent(&agent).is_err());
    }

    fn observation(status: InstallationStatus) -> InstallationProbe {
        InstallationProbe {
            status,
            message: None,
        }
    }

    #[test]
    fn installation_status_wire_values_are_explicit() {
        for (status, value) in [
            (InstallationStatus::Installed, "installed"),
            (InstallationStatus::NotDetected, "not_detected"),
            (InstallationStatus::Error, "error"),
            (InstallationStatus::Unsupported, "unsupported"),
        ] {
            assert_eq!(
                serde_json::to_value(observation(status)).unwrap()["status"],
                value
            );
        }
    }

    #[test]
    fn unsupported_probe_never_runs_a_command_or_resolves_path() {
        for command in [None, Some(""), Some("  ")] {
            let mut agent = fake_agent();
            agent.install.verify_cmd = command.map(str::to_owned);
            let probe = probe_installation_with(
                &agent,
                |_| panic!("No executable to resolve"),
                |_| panic!("No verification to run"),
            );
            assert_eq!(probe.status, InstallationStatus::Unsupported);
            assert!(probe.message.is_some());
        }
    }

    #[test]
    fn only_known_simple_missing_executables_are_not_detected() {
        let mut agent = fake_agent();
        agent.install.verify_cmd = Some("codex --version".into());
        let probe = probe_installation_with(
            &agent,
            |name| {
                assert_eq!(name, "codex");
                Ok(false)
            },
            |_| panic!("A reliably missing executable must not be run"),
        );
        assert_eq!(probe.status, InstallationStatus::NotDetected);
        assert!(probe.message.unwrap().contains("installed elsewhere"));
        for command in [
            "codex --version || exit 127",
            "unknown-agent --version",
            "'codex' --version",
        ] {
            agent.install.verify_cmd = Some(command.into());
            let probe = probe_installation_with(
                &agent,
                |_| panic!("Do not parse shell programs"),
                |_| Err(std::io::Error::other("Command exited 127")),
            );
            assert_eq!(probe.status, InstallationStatus::Error);
        }
    }

    #[test]
    fn probe_errors_are_not_reported_as_absence() {
        let mut agent = fake_agent();
        agent.install.verify_cmd = Some("codex --version".into());
        assert_eq!(
            probe_installation_with(&agent, |_| Ok(true), |_| Ok(())).status,
            InstallationStatus::Installed
        );
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::TimedOut,
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::Other,
        ] {
            let execution = probe_installation_with(
                &agent,
                |_| Ok(true),
                |_| Err(std::io::Error::new(kind, "probe failure")),
            );
            assert_eq!(execution.status, InstallationStatus::Error);
            let resolution = probe_installation_with(
                &agent,
                |_| Err(std::io::Error::new(kind, "search failure")),
                |_| panic!("Do not run after failed resolution"),
            );
            assert_eq!(resolution.status, InstallationStatus::Error);
        }
        agent.install.verify_cmd = Some("exit 7".into());
        assert_eq!(probe_installation(&agent).status, InstallationStatus::Error);
    }

    #[test]
    fn path_lookup_distinguishes_absence_from_unusable_candidates() {
        let directory =
            std::env::temp_dir().join(format!("agentdeck-path-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let suffixes = [String::new()];
        assert!(!executable_in_paths("codex", [directory.clone()], &suffixes).unwrap());
        let candidate = directory.join("codex");
        std::fs::create_dir(&candidate).unwrap();
        assert!(executable_in_paths("codex", [directory.clone()], &suffixes).is_err());
        std::fs::remove_dir(&candidate).unwrap();
        std::fs::write(&candidate, "test executable; never run").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert_eq!(
                executable_in_paths("codex", [directory.clone()], &suffixes)
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::PermissionDenied
            );
            std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(executable_in_paths("codex", [directory.clone()], &suffixes).unwrap());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn install_rejects_existing_unknown_and_unsupported_before_running() {
        for status in [
            InstallationStatus::Installed,
            InstallationStatus::Error,
            InstallationStatus::Unsupported,
        ] {
            assert!(
                install_agent_with(
                    &fake_agent(),
                    |_| observation(status),
                    |_| panic!("Installer must remain blocked"),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn public_install_entry_point_blocks_existing_and_unverified_software() {
        let directory =
            std::env::temp_dir().join(format!("agentdeck-install-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let marker = directory.join("installed.txt");
        let mut agent = fake_agent();
        agent.install.command = Some(format!("echo installed > \"{}\"", marker.display()));
        for command in [Some("echo verified"), Some("exit 7"), None] {
            agent.install.verify_cmd = command.map(str::to_owned);
            assert!(install_agent(&agent).is_err());
            assert!(!marker.exists(), "Blocked install executed its command");
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn install_requires_successful_post_install_verification() {
        for status in [
            InstallationStatus::NotDetected,
            InstallationStatus::Error,
            InstallationStatus::Unsupported,
        ] {
            let mut probes = [InstallationStatus::NotDetected, status].into_iter();
            assert!(
                install_agent_with(
                    &fake_agent(),
                    |_| observation(probes.next().unwrap()),
                    |_| Ok(()),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn uninstall_unknown_provenance_never_executes_registry_command() {
        let directory =
            std::env::temp_dir().join(format!("agentdeck-uninstall-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let marker = directory.join("removed.txt");
        let mut agent = fake_agent();
        agent.install.method = InstallMethod::Npm;
        agent.install.uninstall_cmd = Some(format!("echo removed > \"{}\"", marker.display()));
        assert!(!can_uninstall(&agent));
        assert!(
            uninstall_agent(&agent)
                .unwrap_err()
                .to_string()
                .contains("自动卸载尚未实现")
        );
        assert!(
            !marker.exists(),
            "Registry method is not installation provenance"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn configurable_agent() -> AgentDefinition {
        let mut agent = fake_agent();
        agent.config_inject.personalization = Some(crate::registry::PersonalizationInject {
            template: "synthetic".into(),
            target: "~/.synthetic-agent/AGENTS.md".into(),
        });
        agent
    }

    #[test]
    fn configuration_capability_matches_supported_adapters() {
        assert!(!can_configure(&fake_agent()));
        assert!(can_configure(&configurable_agent()));
        let mut agent = fake_agent();
        agent.config_inject.model_config = Some(crate::registry::ModelConfigInject {
            file: "~/.synthetic-agent/config.json".into(),
            format: "json_merge".into(),
        });
        assert!(!can_configure(&agent));
        for id in ["codex", "opencode"] {
            agent.id = id.into();
            for format in ["json_merge", "jsonc_merge", "toml_section", "yaml_section"] {
                for file in [
                    "~/.synthetic-agent/config.json",
                    "~\\.synthetic-agent\\config.json",
                ] {
                    agent.config_inject.model_config = Some(crate::registry::ModelConfigInject {
                        file: file.into(),
                        format: format.into(),
                    });
                    assert!(can_configure(&agent));
                }
            }
            for (file, format) in [
                ("unsupported://configuration", "json_merge"),
                ("relative/config.json", "json_merge"),
                ("/absolute/config.json", "json_merge"),
                ("~/.synthetic-agent/config.json", "unknown_format"),
            ] {
                agent.config_inject.model_config = Some(crate::registry::ModelConfigInject {
                    file: file.into(),
                    format: format.into(),
                });
                assert!(!can_configure(&agent));
            }
        }
        agent.config_inject.model_config = None;
        agent.config_inject.personalization = Some(crate::registry::PersonalizationInject {
            template: "synthetic".into(),
            target: "unsupported://instructions".into(),
        });
        assert!(!can_configure(&agent));
    }

    #[test]
    fn configuration_without_adapter_does_not_even_probe() {
        let directory =
            std::env::temp_dir().join(format!("agentdeck-configure-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let marker = directory.join("probed.txt");
        let mut agent = fake_agent();
        agent.install.verify_cmd = Some(format!("echo probed > \"{}\"", marker.display()));
        let error = configure_agent(&agent, &directory).unwrap_err();
        assert!(error.to_string().contains("no supported profile"));
        assert!(!marker.exists(), "Unsupported configuration ran a probe");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn configuration_never_installs_and_requires_verified_installation() {
        let mut agent = configurable_agent();
        agent.install.command = Some("exit 97".into());
        let result = configure_agent_with(&agent, std::path::Path::new("."), |agent, _| {
            Ok(crate::config::InjectReport {
                agent_id: agent.id.clone(),
                personalization: Some(crate::config::InjectStatus::Ok),
                model_config: None,
            })
        });
        assert!(result.unwrap().warnings.is_empty());
        for command in [None, Some("exit 7")] {
            agent.install.verify_cmd = command.map(str::to_owned);
            assert!(
                configure_agent_with(&agent, std::path::Path::new("."), |_, _| {
                    panic!("Unverified installation must not apply configuration")
                })
                .is_err()
            );
        }
    }

    #[test]
    fn no_auth_required_is_ready() {
        assert!(check_auth(&fake_agent()).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn timeout_terminates_descendants_before_they_write() {
        let dir = std::env::temp_dir().join(format!("agentdeck-timeout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("late-write");
        let command = format!("sleep 0.3; echo leaked > '{}'", marker.display());
        let error = run_shell_with_timeout(&command, Duration::from_millis(40)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        std::thread::sleep(Duration::from_millis(400));
        assert!(!marker.exists(), "timed-out descendant survived");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_executes_nested_quoted_powershell_commands() {
        let directory = std::env::temp_dir().join(format!(
            "agentdeck quoted & nested-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let marker = directory.join("quoted output.txt");
        let quoted_marker = marker.to_string_lossy().replace("'", "''");
        let command = format!(
            "powershell.exe -NoLogo -NoProfile -NonInteractive -Command \"$ErrorActionPreference = 'Stop'; Set-Content -LiteralPath '{quoted_marker}' -Value 'quoted value & literal text' -Encoding ascii\""
        );
        let result = run_shell_with_timeout(&command, Duration::from_secs(30));
        let contents = std::fs::read_to_string(&marker);
        std::fs::remove_dir_all(directory).unwrap();
        result.expect("quoted PowerShell code must execute, not print as a string");
        assert_eq!(contents.unwrap().trim(), "quoted value & literal text");
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_executes_quoted_script_paths_and_arguments() {
        let directory = std::env::temp_dir().join(format!(
            "agentdeck quoted & script-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let script = directory.join("write marker.cmd");
        let marker = directory.join("quoted output.txt");
        std::fs::write(&script, "@echo off\r\n> \"%~1\" echo %~2\r\n").unwrap();
        let command = format!(
            "\"{}\" \"{}\" \"argument with spaces\"",
            script.display(),
            marker.display()
        );
        let result = run_shell_with_timeout(&command, Duration::from_secs(10));
        let contents = std::fs::read_to_string(&marker);
        std::fs::remove_dir_all(directory).unwrap();
        result.expect("a quoted script path and its arguments must remain intact");
        assert_eq!(contents.unwrap().trim(), "argument with spaces");
    }

    #[cfg(windows)]
    #[test]
    fn timeout_terminates_windows_descendant_before_late_write() {
        let directory =
            std::env::temp_dir().join(format!("agentdeck-timeout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let ready = directory.join("descendant-ready.txt");
        let marker = directory.join("late-write.txt");
        // PowerShell is a descendant of cmd.exe, not the shell handle we reap.
        let quoted_ready = ready.to_string_lossy().replace("'", "''");
        let quoted_marker = marker.to_string_lossy().replace("'", "''");
        let command = format!(
            "powershell.exe -NoLogo -NoProfile -NonInteractive -Command \"$ErrorActionPreference = 'Stop'; Set-Content -LiteralPath '{quoted_ready}' -Value $PID -Encoding ascii; Start-Sleep -Seconds 5; Set-Content -LiteralPath '{quoted_marker}' -Value 'leaked'\""
        );
        let mut child = spawn_shell(&command).unwrap();
        let shell_id = child.id();
        let startup = Instant::now();
        // Prove a real descendant has started before exercising timeout cleanup.
        // Slow PowerShell startup must not turn broken quoting into a false pass.
        let descendant_id = loop {
            if let Ok(contents) = std::fs::read_to_string(&ready)
                && let Ok(pid) = contents.trim().parse::<u32>()
            {
                break pid;
            }
            if child.try_wait().unwrap().is_some() || startup.elapsed() > Duration::from_secs(30) {
                let result = wait_for_shell_with_timeout(child, Duration::ZERO);
                std::fs::remove_dir_all(directory).unwrap();
                panic!("PowerShell descendant did not signal readiness: {result:?}");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let result = wait_for_shell_with_timeout(child, Duration::from_millis(40));
        std::thread::sleep(Duration::from_secs(6));
        let late_write = marker.exists();
        std::fs::remove_dir_all(directory).unwrap();
        assert_ne!(descendant_id, shell_id, "fixture must run in a descendant");
        let error = result.unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        assert!(!late_write, "timed-out Windows descendant survived");
    }

    #[test]
    fn missing_install_command_is_an_error() {
        let mut agent = fake_agent();
        agent.install.command = None;
        assert!(install_agent(&agent).is_err());
    }

    #[test]
    fn auth_file_evidence_is_checked_without_reading_contents() {
        let directory =
            std::env::temp_dir().join(format!("agentdeck-auth-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("auth.json");
        let mut agent = fake_agent();
        agent.auth_type = AuthType::OauthCli;
        agent.auth_guide = Some(crate::registry::AuthGuide {
            steps: Vec::new(),
            completion_check: Some(serde_json::json!({"file_exists": path})),
        });
        assert!(!check_auth(&agent).unwrap());
        std::fs::write(&path, "test-local-evidence").unwrap();
        assert!(check_auth(&agent).unwrap());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn configuration_returns_skipped_configuration_warnings() {
        let agent = configurable_agent();
        let outcome = configure_agent_with(&agent, std::path::Path::new("."), |agent, _| {
            Ok(crate::config::InjectReport {
                agent_id: agent.id.clone(),
                personalization: Some(crate::config::InjectStatus::Skipped(
                    "Missing template".into(),
                )),
                model_config: None,
            })
        })
        .unwrap();
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].starts_with("Personalization:"));
    }

    #[test]
    fn bundled_pi_and_hermes_installation_metadata_matches_official_methods() {
        let registry =
            crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
                .unwrap();
        let pi = registry.agents.get("pi").unwrap();
        assert_eq!(
            pi.install.command.as_deref(),
            Some("npm install -g @earendil-works/pi-coding-agent")
        );
        assert_eq!(
            pi.install.uninstall_cmd.as_deref(),
            Some("npm uninstall -g @earendil-works/pi-coding-agent")
        );
        assert_eq!(pi.github.as_deref(), Some("earendil-works/pi"));
        let hermes = registry.agents.get("hermes").unwrap();
        assert_eq!(hermes.install.method, InstallMethod::Manual);
        assert!(hermes.install.command.is_none());
        assert!(hermes.install.uninstall_cmd.is_none());
        assert_eq!(
            hermes.install.verify_cmd.as_deref(),
            Some("hermes --version")
        );
        assert!(hermes.config_inject.mcp.is_none());
        assert!(hermes.config_inject.personalization.is_none());
        assert!(hermes.config_inject.model_config.is_none());
    }

    /// Read-only opt-in check. No installer or automatic cleanup is run because
    /// installation provenance is not yet recorded by AgentDeck.
    #[test]
    #[ignore = "Requires a pre-existing opencode installation on PATH; read-only"]
    fn real_existing_opencode_verification() {
        let doc =
            crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
                .unwrap();
        let opencode = doc.agents.get("opencode").expect("注册表应有 opencode");
        assert_eq!(
            probe_installation(opencode).status,
            InstallationStatus::Installed
        );
    }
}
