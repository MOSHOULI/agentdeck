//! Bounded execution of trusted registry install/uninstall/verification commands.
//!
//! Callers are responsible for obtaining user confirmation before installation or
//! removal. Authentication checks detect local setup evidence only. Configuration
//! failures never uninstall software that may have existed before this operation.
use crate::registry::AgentDefinition;

use std::process::Command;
use std::time::{Duration, Instant};

/// 执行注册表里的 shell 命令（Windows 用 cmd /C）
fn run_shell(cmdline: &str) -> std::io::Result<()> {
    run_shell_with_timeout(cmdline, Duration::from_secs(300))
}

fn run_shell_with_timeout(cmdline: &str, timeout: Duration) -> std::io::Result<()> {
    if cmdline.trim().is_empty() {
        return Err(std::io::Error::other("Empty deployment command"));
    }
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut c = Command::new("cmd");
        c.creation_flags(0x08000000); // CREATE_NO_WINDOW: background enumeration must not flash consoles.
        c.args(["/C", cmdline]);
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
    let mut child = command.spawn()?;
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
                let _ = Command::new("kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .status();
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

/// 安装 Agent：执行 install.command → verify_cmd 验证
pub fn install_agent(_agent: &AgentDefinition) -> crate::Result<()> {
    let cmd = _agent
        .install
        .command
        .as_deref()
        .ok_or_else(|| crate::Error::InstallFailed(format!("{} 缺少安装命令", _agent.name)))?;
    if _agent
        .install
        .verify_cmd
        .as_deref()
        .is_none_or(|cmd| cmd.trim().is_empty())
    {
        return Err(crate::Error::InstallFailed("安装缺少可用的验证命令".into()));
    }
    run_shell(cmd)
        .map_err(|e| crate::Error::InstallFailed(format!("安装 {} 失败: {}", _agent.name, e)))?;
    if !verify_installation(_agent)? {
        return Err(crate::Error::InstallFailed(format!(
            "安装 {} 后验证失败",
            _agent.name
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
    let report = crate::config::inject_for_agent(agent, source_dir)?;
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

/// 卸载 Agent：执行 uninstall_cmd
pub fn uninstall_agent(_agent: &AgentDefinition) -> crate::Result<()> {
    let cmd = _agent
        .install
        .uninstall_cmd
        .as_deref()
        .ok_or_else(|| crate::Error::InstallFailed(format!("{} 缺少卸载命令", _agent.name)))?;
    run_shell(cmd)
        .map_err(|e| crate::Error::InstallFailed(format!("卸载 {} 失败: {}", _agent.name, e)))?;
    Ok(())
}

/// 验证 Agent 是否安装成功：执行 verify_cmd，3 秒超时
pub fn verify_installation(_agent: &AgentDefinition) -> crate::Result<bool> {
    let Some(verify_cmd) = _agent.install.verify_cmd.clone() else {
        return Ok(false);
    };
    Ok(run_shell_with_timeout(&verify_cmd, Duration::from_secs(3)).is_ok())
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
    fn install_verify_uninstall_roundtrip_with_echo() {
        let agent = fake_agent();
        install_agent(&agent).expect("echo 安装命令应成功");
        assert!(verify_installation(&agent).unwrap());
        uninstall_agent(&agent).expect("echo 卸载命令应成功");
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
    fn timeout_terminates_windows_descendant_before_late_write() {
        let directory =
            std::env::temp_dir().join(format!("agentdeck-timeout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let marker = directory.join("late-write.txt");
        // PowerShell is a descendant of cmd.exe, not the shell handle we reap.
        let quoted_marker = marker.to_string_lossy().replace("'", "''");
        let command = format!(
            "powershell.exe -NoLogo -NoProfile -NonInteractive -Command \"Start-Sleep -Seconds 5; Set-Content -LiteralPath '{quoted_marker}' -Value 'leaked'\""
        );
        let error = run_shell_with_timeout(&command, Duration::from_secs(3)).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
        std::thread::sleep(Duration::from_secs(6));
        assert!(!marker.exists(), "timed-out Windows descendant survived");
        std::fs::remove_dir_all(directory).unwrap();
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
    fn successful_install_returns_skipped_configuration_warnings() {
        let mut agent = fake_agent();
        agent.config_inject.personalization = Some(crate::registry::PersonalizationInject {
            template: "fake".into(),
            target: "unsupported://instructions".into(),
        });
        let outcome = install_agent_with_inject(&agent, std::path::Path::new(".")).unwrap();
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

    fn npm_has_opencode() -> bool {
        run_shell("npm ls -g --depth=0 opencode-ai").is_ok()
    }

    /// 真实环境验证：npm 全局安装 opencode-ai → 验证 → 卸载（需要手动运行）
    #[test]
    #[ignore = "真实环境验证：执行 npm install/uninstall -g opencode-ai"]
    fn real_install_roundtrip() {
        let doc =
            crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
                .unwrap();
        let opencode = doc.agents.get("opencode").expect("注册表应有 opencode");

        assert!(
            !npm_has_opencode(),
            "前置条件：npm 全局不应已有 opencode-ai"
        );
        install_agent(opencode).expect("真实安装应成功");
        assert!(verify_installation(opencode).unwrap());
        assert!(npm_has_opencode());

        uninstall_agent(opencode).expect("真实卸载应成功");
        assert!(!npm_has_opencode(), "卸载后 npm 全局不应再有 opencode-ai");
    }
}
