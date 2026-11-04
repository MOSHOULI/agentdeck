//! Agent deployment: installation and optional configuration are separate actions.

use std::collections::HashMap;

use aisync_core::deploy::{InstallationProbe, InstallationStatus};
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
    install_source: &'static str,
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

fn to_agent_info(agent: AgentDefinition, probe: InstallationProbe) -> AgentInfo {
    let installed = matches!(probe.status, InstallationStatus::Installed);
    let auth_verified = installed && aisync_core::deploy::check_auth(&agent).unwrap_or(false);
    let nonempty = |value: &Option<String>| value.as_ref().is_some_and(|v| !v.trim().is_empty());
    let can_verify_install = nonempty(&agent.install.verify_cmd);
    let can_install = nonempty(&agent.install.command) && can_verify_install;
    // A registry method is not evidence of this machine's installation source.
    let can_uninstall = aisync_core::deploy::can_uninstall(&agent);
    let uninstall_help = aisync_core::deploy::uninstall_block_reason(&agent).map(str::to_owned);
    let can_configure = aisync_core::deploy::can_configure(&agent);
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
        installation_status: probe.status,
        installation_message: probe.message,
        install_source: "unknown",
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

/// 获取所有注册的 agent（含安装状态，并行验证，单条 3 秒超时）
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
                let probe = aisync_core::deploy::probe_installation(&agent);
                (agent.id.clone(), probe)
            })
        })
        .collect();

    let mut probes: HashMap<String, InstallationProbe> =
        handles.into_iter().filter_map(|h| h.join().ok()).collect();

    Ok(agents
        .into_iter()
        .map(|a| {
            let probe = probes.remove(&a.id).unwrap_or(InstallationProbe {
                status: InstallationStatus::Error,
                message: Some("安装状态检测未完成，请刷新后重试。".to_string()),
            });
            to_agent_info(a, probe)
        })
        .collect())
}

/// Install software only. This must not initialize profiles, vaults or backups.
#[tauri::command]
pub async fn install_agent(id: String) -> Result<aisync_core::deploy::InstallOutcome, String> {
    let agent = find_agent(&id).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        aisync_core::deploy::install_agent(&agent)?;
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
        let probe = aisync_core::deploy::probe_installation(&agent);
        if !matches!(probe.status, InstallationStatus::Installed) {
            return Err(aisync_core::Error::InstallFailed(
                "尚未验证软件已安装，无法应用配置；请先刷新安装状态。".to_string(),
            ));
        }
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
            InstallationProbe {
                status: InstallationStatus::Installed,
                message: None,
            },
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
            InstallationProbe {
                status: InstallationStatus::Error,
                message: Some("synthetic timeout".to_string()),
            },
        );
        let actual = serde_json::to_value(info).unwrap();
        assert_eq!(actual["installationStatus"], "error");
        assert_eq!(actual["installationMessage"], "synthetic timeout");
        assert_eq!(actual["installed"], false);
        assert_eq!(actual["authVerified"], false);
        assert_eq!(actual["installSource"], "unknown");
        assert_eq!(actual["canUninstall"], false);
        assert!(actual.get("version").is_none());
    }

    #[test]
    fn agent_without_configuration_adapter_is_not_configurable() {
        let mut agent = fixture_agent();
        agent.config_inject.personalization = None;
        let info = to_agent_info(
            agent,
            InstallationProbe {
                status: InstallationStatus::Installed,
                message: None,
            },
        );
        assert!(!info.can_configure);
    }
}
