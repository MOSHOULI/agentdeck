//! Agent deployment: installation and optional configuration are separate actions.

use std::collections::HashMap;

use aisync_core::deploy::{
    InstallationDetails, InstallationProbe, InstallationSource, InstallationStatus,
};
use aisync_core::registry::{self, AgentDefinition};
use serde::Serialize;

/// 前端 bridge.ts 的 AgentInfo 结构（camelCase 对齐）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    id: String,
    name: String,
    installed: bool,
    installation_status: InstallationStatus,
    installation_message: Option<String>,
    installation_details: InstallationDetails,
    install_source: &'static str,
    version: Option<String>,
    auth_verified: bool,
    source: String,
    ui_type: String,
    stars: u64,
    popular: bool,
    description: String,
    icon: String,
    category: String,
    can_install: bool,
    can_verify_install: bool,
    can_uninstall: bool,
    can_configure: bool,
    install_help: Option<String>,
    uninstall_help: Option<String>,
    homepage: Option<String>,
}

fn load_agents() -> aisync_core::Result<Vec<AgentDefinition>> {
    let doc = registry::load_registry_runtime()?;
    Ok(doc.agents.into_values().collect())
}

fn find_agent(id: &str) -> aisync_core::Result<AgentDefinition> {
    let doc = registry::load_registry_runtime()?;
    doc.agents
        .get(id)
        .cloned()
        .ok_or_else(|| aisync_core::Error::AgentNotFound(id.to_string()))
}

fn ui_type_string(ui_type: &registry::UiType) -> String {
    serde_json::from_value(serde_json::to_value(ui_type).unwrap_or_default()).unwrap_or_default()
}

fn to_agent_info(agent: AgentDefinition, details: InstallationDetails) -> AgentInfo {
    let installed = matches!(details.probe.status, InstallationStatus::Installed);
    let mut active_instances = details.instances.iter().filter(|instance| instance.active);
    let first_active = active_instances.next();
    let active = if active_instances.next().is_none() {
        first_active
    } else {
        None
    };
    let version = active.and_then(|instance| instance.version.clone());
    let install_source = match active.map(|instance| &instance.source) {
        Some(InstallationSource::Npm { .. }) => "npm",
        _ => "unknown",
    };
    let auth_verified = installed && aisync_core::deploy::check_auth(&agent).unwrap_or(false);
    let nonempty = |value: &Option<String>| value.as_ref().is_some_and(|v| !v.trim().is_empty());
    let can_verify_install = nonempty(&agent.install.verify_cmd);
    let can_install = aisync_core::deploy::can_install(&agent);
    // A registry method is not evidence of this machine's installation source.
    let can_uninstall = aisync_core::deploy::can_uninstall(&agent);
    let uninstall_help = aisync_core::deploy::uninstall_block_reason(&agent).map(str::to_owned);
    let can_configure =
        agent.id != "codex" && aisync_core::deploy::can_configure(&agent) && !details.ambiguous;
    let install_help = if can_install {
        None
    } else if can_verify_install {
        Some("此工具需要手动安装，请访问官方主页，安装后刷新列表。".to_string())
    } else {
        Some("此工具需要手动安装，请访问官方主页。暂不支持自动检测安装状态。".to_string())
    };
    AgentInfo {
        can_install,
        can_verify_install,
        can_uninstall,
        can_configure,
        install_help,
        uninstall_help,
        homepage: agent.homepage,
        id: agent.id,
        name: agent.name,
        installed,
        installation_status: details.probe.status,
        installation_message: details.probe.message.clone(),
        installation_details: details,
        install_source,
        version,
        auth_verified,
        source: agent.source,
        ui_type: ui_type_string(&agent.ui_type),
        stars: agent.stars.unwrap_or(0),
        popular: agent.popular,
        description: agent.description,
        icon: agent.icon,
        category: agent.category,
    }
}

/// List registered agents with bounded, parallel instance and source inspection.
#[tauri::command]
pub async fn get_agents() -> Result<Vec<AgentInfo>, String> {
    tauri::async_runtime::spawn_blocking(list_agents_with_status)
        .await
        .map_err(|e| e.to_string())?
}

fn list_agents_with_status() -> Result<Vec<AgentInfo>, String> {
    let agents = load_agents().map_err(|e| e.to_string())?;

    let handles: Vec<_> = agents
        .iter()
        .map(|agent| {
            let agent = agent.clone();
            std::thread::spawn(move || {
                let details = aisync_core::deploy::inspect_installation(&agent);
                (agent.id.clone(), details)
            })
        })
        .collect();

    let mut probes: HashMap<String, InstallationDetails> =
        handles.into_iter().filter_map(|h| h.join().ok()).collect();

    Ok(agents
        .into_iter()
        .map(|a| {
            let details = probes.remove(&a.id).unwrap_or(InstallationDetails {
                probe: InstallationProbe {
                    status: InstallationStatus::Error,
                    message: Some("安装状态检测未完成，请刷新后重试。".to_string()),
                },
                instances: Vec::new(),
                ambiguous: true,
                message: Some("未取得完整实例证据，写操作保持受限。".to_string()),
            });
            to_agent_info(a, details)
        })
        .collect())
}

/// Inspect an ephemeral target for review without running the installer.
#[tauri::command]
pub async fn preview_agent_install(
    id: String,
) -> Result<super::install_session::DesktopPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut slot = super::install_session::SESSION
            .try_lock()
            .map_err(|_| super::install_session::UNAVAILABLE.to_string())?;
        *slot = None;
        let agent = find_agent(&id).map_err(|e| e.to_string())?;
        let preview = aisync_core::deploy::preview_install(&agent).map_err(|e| e.to_string())?;
        Ok(super::install_session::remember(&mut slot, preview))
    })
    .await
    .map_err(|_| super::install_session::UNAVAILABLE.to_string())?
}

/// The reference binds inspection only, never expands installation authority.
#[tauri::command]
pub async fn install_agent(
    id: String,
    revision: String,
    session_ref: Option<String>,
) -> Result<aisync_core::deploy::InstallOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut slot = super::install_session::SESSION
            .try_lock()
            .map_err(|_| super::install_session::UNAVAILABLE.to_string())?;
        super::install_session::submitted(&mut slot, &id, &revision, session_ref.as_deref());
        let result = find_agent(&id)
            .and_then(|agent| aisync_core::deploy::install_agent_from_preview(&agent, &revision));
        super::install_session::finished(&mut slot, result.is_ok());
        result.map_err(|e| e.to_string())?;
        Ok(aisync_core::deploy::InstallOutcome {
            warnings: Vec::new(),
        })
    })
    .await
    .map_err(|_| super::install_session::UNAVAILABLE.to_string())?
}

/// Explicit metadata inspection; accepts no frontend path. Lock excludes preview/install.
#[tauri::command]
pub async fn inspect_codex_install_residual(
    session_ref: String,
) -> Result<aisync_core::deploy::residual::ResidualObservation, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let slot = super::install_session::SESSION
            .try_lock()
            .map_err(|_| super::install_session::UNAVAILABLE.to_string())?;
        super::install_session::inspect(&slot, &session_ref)
    })
    .await
    .map_err(|_| super::install_session::UNAVAILABLE.to_string())?
}

/// Read-only planning: no profile initialization, installation probe, or backup.
#[tauri::command]
pub async fn preview_codex_configuration()
-> Result<aisync_core::config::CodexConfigurationPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let home = aisync_core::home_dir().ok_or_else(|| "无法确定配置目录。".to_string())?;
        let registry =
            registry::load_registry_read_only().map_err(|_| "配置计划未完成。".to_string())?;
        let agent = registry
            .agents
            .get("codex")
            .ok_or_else(|| "配置计划未完成。".to_string())?;
        let source = home.join(".agentdeck").join("profiles").join("default");
        aisync_core::config::preview_codex_configuration(agent, &source, &home)
            .map_err(|_| "配置计划未完成，请检查文件、路径及受支持格式。".to_string())
    })
    .await
    .map_err(|_| "配置计划未完成。".to_string())?
}

/// Apply the optional profile only after a separate explicit user action.
#[tauri::command]
pub async fn configure_agent(id: String) -> Result<aisync_core::deploy::InstallOutcome, String> {
    if id == "codex" {
        return Err("Codex 当前仅提供只读配置计划，不支持应用；未创建备份或执行探测。".to_string());
    }
    let agent = find_agent(&id).map_err(|e| e.to_string())?;
    if !aisync_core::deploy::can_configure(&agent) {
        return Err("此 Agent 暂无可用的配置适配器，未更改配置。".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        aisync_core::deploy::ensure_configuration_eligible(&agent)?;
        let source = aisync_core::config::profile_source_dir()?;
        aisync_core::git::backup(&format!("configure {}", agent.name))?;
        aisync_core::deploy::configure_agent(&agent, &source)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// 卸载指定 agent
#[tauri::command]
pub async fn uninstall_agent(id: String) -> Result<(), String> {
    let agent = find_agent(&id).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        // Refuse unknown provenance before making any backup or other changes.
        aisync_core::deploy::uninstall_agent(&agent)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// 验证 agent 认证状态（M1 简化实现）
#[tauri::command]
pub fn verify_agent_auth(id: String) -> Result<bool, String> {
    let agent = find_agent(&id).map_err(|e| e.to_string())?;
    aisync_core::deploy::check_auth(&agent).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aisync_core::deploy::InstallationInstance;

    fn details_from_probe(probe: InstallationProbe) -> InstallationDetails {
        InstallationDetails {
            probe,
            instances: Vec::new(),
            ambiguous: false,
            message: None,
        }
    }

    fn fixture_agent() -> AgentDefinition {
        serde_json::from_value(serde_json::json!({
            "id": "contract-agent",
            "name": "Contract Agent",
            "source": "open",
            "ui_type": "cli",
            "category": "test",
            "description": "Synthetic contract fixture",
            "icon": "test",
            "install": {
                "method": "manual",
                "command": "echo synthetic-install",
                "verify_cmd": "echo synthetic-verify",
                "uninstall_cmd": "echo must-not-run"
            },
            "auth_type": "none",
            "config_inject": {
                "personalization": {
                    "template": "synthetic",
                    "target": "~/.synthetic-agent/AGENTS.md"
                }
            }
        }))
        .unwrap()
    }

    #[test]
    fn codex_plan_ipc_does_not_initialize_or_probe() {
        const CHILD: &str = "AGENTDECK_CODEX_PLAN_IPC_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let home = aisync_core::home_dir().unwrap();
            assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
            let plan = tauri::async_runtime::block_on(preview_codex_configuration()).unwrap();
            let value = serde_json::to_value(plan).unwrap();
            assert_eq!(value["applySupported"], false);
            assert_eq!(value["usesBundledDefaults"], true);
            assert_eq!(value["targets"].as_array().unwrap().len(), 2);
            assert!(tauri::async_runtime::block_on(configure_agent("codex".into())).is_err());
            assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
            return;
        }
        let base = std::path::PathBuf::from(
            std::env::var_os("AGENTDECK_TEST_HOME").expect("run with scripts/run-isolated.py"),
        );
        assert!(base.is_absolute());
        let root = base.join(format!(
            "codex-plan-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "commands::deploy::tests::codex_plan_ipc_does_not_initialize_or_probe",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("AGENTDECK_TEST_HOME", &root)
            .env("HOME", &root)
            .env("USERPROFILE", &root)
            .env("APPDATA", root.join("AppData/Roaming"))
            .env("LOCALAPPDATA", root.join("AppData/Local"))
            .env("XDG_CONFIG_HOME", root.join(".config"))
            .env("XDG_DATA_HOME", root.join(".local/share"))
            .env_remove("CODEX_HOME")
            .env("PATH", "")
            .output()
            .unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
    }

    #[test]
    fn codex_legacy_configuration_rejects_before_lookup_or_side_effects() {
        let error =
            tauri::async_runtime::block_on(configure_agent("codex".to_string())).unwrap_err();
        assert_eq!(
            error,
            "Codex 当前仅提供只读配置计划，不支持应用；未创建备份或执行探测。"
        );
    }

    #[test]
    fn installed_dto_matches_shared_frontend_contract() {
        let info = to_agent_info(
            fixture_agent(),
            details_from_probe(InstallationProbe {
                status: InstallationStatus::Installed,
                message: None,
            }),
        );
        let expected: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/agent-info-installed.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(info).unwrap(), expected);
    }

    #[test]
    fn failed_probe_is_not_reported_as_absence_or_managed_installation() {
        let info = to_agent_info(
            fixture_agent(),
            details_from_probe(InstallationProbe {
                status: InstallationStatus::Error,
                message: Some("synthetic timeout".to_string()),
            }),
        );
        let actual = serde_json::to_value(info).unwrap();
        assert_eq!(actual["installationStatus"], "error");
        assert_eq!(actual["installationMessage"], "synthetic timeout");
        assert_eq!(actual["installed"], false);
        assert_eq!(actual["authVerified"], false);
        assert_eq!(actual["installSource"], "unknown");
        assert_eq!(actual["canUninstall"], false);
        assert!(actual["version"].is_null());
        assert!(
            actual["installationDetails"]["instances"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn agent_without_configuration_adapter_is_not_configurable() {
        let mut agent = fixture_agent();
        agent.config_inject.personalization = None;
        let info = to_agent_info(
            agent,
            details_from_probe(InstallationProbe {
                status: InstallationStatus::Installed,
                message: None,
            }),
        );
        assert!(!info.can_configure);
    }

    fn observed_instance(active: bool) -> InstallationInstance {
        InstallationInstance {
            path: "/synthetic/bin/contract-agent".into(),
            resolved_path: Some("/synthetic/lib/node_modules/contract-agent/bin/cli.js".into()),
            entrypoint_path: Some("/synthetic/lib/node_modules/contract-agent/bin/cli.js".into()),
            version: Some("1.2.3".into()),
            source: InstallationSource::Npm {
                package: "contract-agent".into(),
                package_root: "/synthetic/lib/node_modules/contract-agent".into(),
                global_root: "/synthetic/lib/node_modules".into(),
                prefix: "/synthetic".into(),
                manager_path: "/synthetic/lib/node_modules/npm/bin/npm-cli.js".into(),
            },
            active,
        }
    }

    #[test]
    fn npm_observation_dto_matches_shared_frontend_contract_without_management_grant() {
        let mut details = details_from_probe(InstallationProbe {
            status: InstallationStatus::Installed,
            message: None,
        });
        details.instances.push(observed_instance(true));
        let actual = serde_json::to_value(to_agent_info(fixture_agent(), details)).unwrap();
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/agent-info-npm.json"))
                .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual["canUninstall"], false);
    }

    #[test]
    fn inactive_or_multiple_active_instances_do_not_supply_top_level_version_or_source() {
        for instances in [
            vec![observed_instance(false)],
            vec![observed_instance(true), observed_instance(true)],
        ] {
            let mut details = details_from_probe(InstallationProbe {
                status: InstallationStatus::Installed,
                message: None,
            });
            details.instances = instances;
            details.ambiguous = true;
            let info = to_agent_info(fixture_agent(), details);
            assert!(info.version.is_none());
            assert_eq!(info.install_source, "unknown");
            assert!(!info.can_configure);
            assert!(!info.can_uninstall);
        }
    }
}
