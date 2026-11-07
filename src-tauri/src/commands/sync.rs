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
        mcp::toggle_with_backup(&agent_id, &server_name, enabled, || {
            aisync_core::git::backup(&format!("mcp: {} {} {}", action, server_name, agent_id))
                .map(|_| ())
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_ipc_targets_never_start_backups() {
        const CHILD: &str = "AGENTDECK_MCP_PREFLIGHT_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let home = std::path::PathBuf::from(
                std::env::var_os("AGENTDECK_TEST_HOME").expect("isolated child HOME"),
            );
            assert!(home.is_absolute());
            for agent in ["opencode", "not-an-agent", "qwen-code"] {
                for enabled in [true, false] {
                    // Exercise the real asynchronous command and spawn_blocking
                    // boundary, not a copy of its ordering or a real agent.
                    let error = tauri::async_runtime::block_on(toggle_mcp_for_agent(
                        "ragflow".into(),
                        agent.into(),
                        enabled,
                    ))
                    .unwrap_err();
                    assert!(error.contains(&format!("agent 不支持 MCP: {agent}")));
                    for name in ["git-backup", "vault", "backups"] {
                        assert!(
                            !home.join(".agentdeck").join(name).exists(),
                            "unexpected {name}"
                        );
                    }
                }
            }
            return;
        }

        // Separate child process so parallel workspace tests do not share an
        // application HOME. Never mutate this process's environment.
        let base = std::path::PathBuf::from(
            std::env::var_os("AGENTDECK_TEST_HOME").expect("run with scripts/run-isolated.py"),
        );
        assert!(base.is_absolute());
        let root = base.join(format!(
            "mcp-ipc-{}-{}",
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
                "commands::sync::tests::unsupported_ipc_targets_never_start_backups",
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
            .output()
            .unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            result.status.success(),
            "IPC child failed:\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
    }
}
