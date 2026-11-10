//! Narrow default-home editor. No initialization, executable probing, or bulk fallback.
use aisync_core::{config::codex_animations_fs as fs, registry};
use std::path::{Path, PathBuf};
const ERROR: &str = "Codex 动画操作未完成或结果未确认；请检查受支持平台、配置格式和恢复状态。";
fn location() -> Result<PathBuf, String> {
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Err("仅 Linux x86_64 支持动画配置操作。".into());
    }
    let home = aisync_core::home_dir().ok_or(ERROR)?;
    let dir = home.join(".codex");
    if let Some(value) = std::env::var_os("CODEX_HOME")
        && (value.is_empty() || Path::new(&value) != dir)
    {
        return Err(ERROR.into());
    }
    let registry = registry::load_registry_read_only().map_err(|_| ERROR)?;
    let agent = registry.agents.get("codex").ok_or(ERROR)?;
    if agent.id != "codex"
        || !agent
            .config_inject
            .personalization
            .as_ref()
            .is_some_and(|p| p.template == "codex" && p.target == "~/.codex/AGENTS.md")
        || !agent
            .config_inject
            .model_config
            .as_ref()
            .is_some_and(|m| m.file == "~/.codex/config.toml" && m.format == "toml_section")
    {
        return Err(ERROR.into());
    }
    Ok(dir.join("config.toml"))
}
fn valid_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(serde::Serialize)]
pub struct Recovery {
    state: &'static str,
    token: Option<String>,
}
#[tauri::command]
pub async fn preview_codex_animations(
    enabled: bool,
) -> Result<aisync_core::config::codex_animations::Preview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        fs::preview(&location()?, enabled).map_err(|_| ERROR.to_string())
    })
    .await
    .map_err(|_| ERROR.to_string())?
}
#[tauri::command]
pub async fn apply_codex_animations(enabled: bool, token: String) -> Result<bool, String> {
    if !valid_token(&token) {
        return Err(ERROR.into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        fs::apply(&location()?, enabled, &token).map_err(|_| ERROR.to_string())
    })
    .await
    .map_err(|_| ERROR.to_string())?
}
#[tauri::command]
pub async fn inspect_codex_animations_recovery() -> Result<Recovery, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let preview = fs::restore_preview(&location()?).map_err(|_| ERROR.to_string())?;
        Ok(Recovery {
            state: match preview.state {
                fs::RecoveryState::NoReceipt => "no_receipt",
                fs::RecoveryState::RestoreOriginal => "restore_original",
                fs::RecoveryState::CleanupReceipt => "cleanup_receipt",
            },
            token: preview.token,
        })
    })
    .await
    .map_err(|_| ERROR.to_string())?
}
#[tauri::command]
pub async fn restore_codex_animations(token: String) -> Result<bool, String> {
    if !valid_token(&token) {
        return Err(ERROR.into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        fs::restore_checked(&location()?, &token).map_err(|_| ERROR.to_string())
    })
    .await
    .map_err(|_| ERROR.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_are_bounded_and_lower_hex_only() {
        assert!(valid_token(&"a".repeat(64)));
        for value in ["", "secret", &"A".repeat(64), &"a".repeat(65)] {
            assert!(!valid_token(value));
        }
    }
    #[test]
    fn ipc_rejects_without_initialization_or_execution() {
        const CHILD: &str = "AGENTDECK_ANIMATIONS_IPC_CHILD";
        if let Some(scenario) = std::env::var_os(CHILD) {
            let home = aisync_core::home_dir().unwrap();
            assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
            let allowed = scenario == "default" || scenario == "same";
            if cfg!(all(target_os = "linux", target_arch = "x86_64")) && allowed {
                assert_eq!(location().unwrap(), home.join(".codex/config.toml"));
            } else {
                assert!(location().is_err());
            }
            assert!(tauri::async_runtime::block_on(preview_codex_animations(false)).is_err());
            assert!(
                tauri::async_runtime::block_on(apply_codex_animations(false, "bad".into()))
                    .is_err()
            );
            assert!(
                tauri::async_runtime::block_on(restore_codex_animations("bad".into())).is_err()
            );
            assert_eq!(std::fs::read_dir(&home).unwrap().count(), 0);
            return;
        }
        let base = PathBuf::from(
            std::env::var_os("AGENTDECK_TEST_HOME").expect("run with scripts/run-isolated.py"),
        );
        assert!(base.is_absolute());
        for scenario in ["default", "same", "empty", "different"] {
            let root = base.join(format!(
                "animations-ipc-{}-{}-{}",
                std::process::id(),
                scenario,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command.args(["--exact", "commands::codex_animations::tests::ipc_rejects_without_initialization_or_execution", "--nocapture"])
                .env(CHILD, scenario).env("AGENTDECK_TEST_HOME", &root).env("HOME", &root).env("USERPROFILE", &root)
                .env("XDG_CONFIG_HOME", root.join(".config")).env("XDG_DATA_HOME", root.join(".local/share"))
                .env("APPDATA", root.join("AppData/Roaming")).env("LOCALAPPDATA", root.join("AppData/Local"))
                .env_remove("CODEX_HOME").env("PATH", "");
            match scenario {
                "same" => {
                    command.env("CODEX_HOME", root.join(".codex"));
                }
                "empty" => {
                    command.env("CODEX_HOME", "");
                }
                "different" => {
                    command.env("CODEX_HOME", root.join("other"));
                }
                _ => {}
            }
            let output = command.output().unwrap();
            std::fs::remove_dir_all(&root).unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
    }
}
