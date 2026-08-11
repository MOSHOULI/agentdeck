//! Agent 部署命令（M1）：get_agents / install_agent / uninstall_agent / verify_agent_auth

use std::collections::HashMap;

use aisync_core::registry::{self, AgentDefinition};
use serde::Serialize;

const REGISTRY_JSON: &str = include_str!("../../../agents/registry-v3.json");

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
}

fn load_agents() -> aisync_core::Result<Vec<AgentDefinition>> {
    let doc = registry::parse_registry(REGISTRY_JSON)?;
    Ok(doc.agents.into_values().collect())
}

fn find_agent(id: &str) -> aisync_core::Result<AgentDefinition> {
    let doc = registry::parse_registry(REGISTRY_JSON)?;
    doc.agents
        .get(id)
        .cloned()
        .ok_or_else(|| aisync_core::Error::AgentNotFound(id.to_string()))
}

/// 共享配置数据源（可被 AGENTDECK_SHARED_CONFIG 覆盖）
fn shared_config_source() -> std::path::PathBuf {
    std::env::var_os("AGENTDECK_SHARED_CONFIG")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            // 默认使用用户主目录下的通用共享配置目录
            let home = dirs::home_dir().unwrap_or_default();
            home.join("shared-ai-config")
        })
}

fn ui_type_string(ui_type: &registry::UiType) -> String {
    serde_json::from_value(serde_json::to_value(ui_type).unwrap_or_default()).unwrap_or_default()
}

fn to_agent_info(agent: AgentDefinition, installed: bool) -> AgentInfo {
    AgentInfo {
        id: agent.id,
        name: agent.name,
        installed,
        auth_verified: false,
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
pub fn get_agents() -> Result<Vec<AgentInfo>, String> {
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
pub async fn install_agent(id: String) -> Result<(), String> {
    let agent = find_agent(&id).map_err(|e| e.to_string())?;
    let source = shared_config_source();
    tauri::async_runtime::spawn_blocking(move || {
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
