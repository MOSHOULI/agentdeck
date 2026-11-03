//! Agent 部署命令（M1）：get_agents / install_agent / uninstall_agent / verify_agent_auth

use std::collections::HashMap;

use aisync_core::registry::{self, AgentDefinition};
use serde::Serialize;

/// 前端 bridge.ts 的 AgentInfo 结构（camelCase 对齐）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    id: String,
    name: String,
    installed: bool,
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
    install_help: Option<String>,
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

fn to_agent_info(agent: AgentDefinition, installed: bool) -> AgentInfo {
    let auth_verified = installed && aisync_core::deploy::check_auth(&agent).unwrap_or(false);
    let nonempty = |value: &Option<String>| value.as_ref().is_some_and(|v| !v.trim().is_empty());
    let can_verify_install = nonempty(&agent.install.verify_cmd);
    let can_install = nonempty(&agent.install.command) && can_verify_install;
    let can_uninstall = nonempty(&agent.install.uninstall_cmd);
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
        install_help,
        homepage: agent.homepage,
        id: agent.id,
        name: agent.name,
        installed,
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
                let installed = aisync_core::deploy::verify_installation(&agent).unwrap_or(false);
                (agent.id.clone(), installed)
            })
        })
        .collect();

    let installed: HashMap<String, bool> =
        handles.into_iter().filter_map(|h| h.join().ok()).collect();

    Ok(agents
        .into_iter()
        .map(|a| {
            let installed = installed.get(&a.id).copied().unwrap_or(false);
            to_agent_info(a, installed)
        })
        .collect())
}

/// 安装指定 agent（真实执行注册表里的安装命令）
#[tauri::command]
pub async fn install_agent(id: String) -> Result<aisync_core::deploy::InstallOutcome, String> {
    let agent = find_agent(&id).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let source = aisync_core::config::profile_source_dir()?;
        aisync_core::git::backup(&format!("install {}", agent.name))?;
        aisync_core::deploy::install_agent_with_inject(&agent, &source)
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
        aisync_core::git::backup(&format!("uninstall {}", agent.name))?;
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
