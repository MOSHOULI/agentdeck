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
    let can_configure = aisync_core::deploy::can_configure(&agent) && !details.ambiguous;
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
) -> Result<aisync_core::deploy::InstallPreview, String> {
    let agent = find_agent(&id).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || aisync_core::deploy::preview_install(&agent))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Install software only. This must not initialize profiles, vaults or backups.
#[tauri::command]
pub async fn install_agent(
    id: String,
    revision: String,
) -> Result<aisync_core::deploy::InstallOutcome, String> {
    let agent = find_agent(&id).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        aisync_core::deploy::install_agent_from_preview(&agent, &revision)?;
        Ok::<_, aisync_core::Error>(aisync_core::deploy::InstallOutcome {
            warnings: Vec::new(),
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// Apply the optional profile only after a separate explicit user action.
#[tauri::command]
pub async fn configure_agent(id: String) -> Result<aisync_core::deploy::InstallOutcome, String> {
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
