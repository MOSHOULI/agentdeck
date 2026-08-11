//! 同步/MCP 命令（M2-M5）：get_mcp_servers / toggle_mcp_for_agent

use aisync_core::mcp;

/// 列出 MCP 服务器及每款 agent 的启用状态
#[tauri::command]
pub async fn get_mcp_servers() -> Result<Vec<mcp::McpServerInfo>, String> {
    tauri::async_runtime::spawn_blocking(mcp::list_servers)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// 切换某 agent 的 MCP 服务器开关（实时写回配置文件，写前自动 git 备份）
#[tauri::command]
pub async fn toggle_mcp_for_agent(
    server_name: String,
    agent_id: String,
    enabled: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let action = if enabled { "on" } else { "off" };
        aisync_core::git::backup(&format!("mcp: {} {} {}", action, server_name, agent_id))?;
        mcp::toggle(&agent_id, &server_name, enabled)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}
