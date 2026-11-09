//! Explicit selected-file scope summary. Never discovery, credentials lookup or writes.
use aisync_core::qoder::{self, QoderScopeReport, Selections};
use std::path::Path;
#[tauri::command]
pub async fn inspect_qoder_scopes(
    user_settings: Option<String>,
    project_settings: Option<String>,
    local_settings: Option<String>,
    project_mcp: Option<String>,
) -> Result<QoderScopeReport, String> {
    let paths = [user_settings, project_settings, local_settings, project_mcp];
    if paths
        .iter()
        .flatten()
        .any(|p| p.is_empty() || p.len() > 4096 || p.chars().any(char::is_control))
    {
        return Err("QODER_SCOPE_INVALID_SELECTION".into());
    }
    let selections = selected(&paths);
    qoder::validate_selections(&selections)
        .map_err(|_| "QODER_SCOPE_INVALID_SELECTION".to_string())?;
    tauri::async_runtime::spawn_blocking(move || qoder::inspect_scopes(&selected(&paths)))
        .await
        .map_err(|_| "QODER_SCOPE_UNAVAILABLE".into())
}
fn selected(paths: &[Option<String>; 4]) -> Selections<'_> {
    Selections {
        user_settings: paths[0].as_deref().map(Path::new),
        project_settings: paths[1].as_deref().map(Path::new),
        local_settings: paths[2].as_deref().map(Path::new),
        project_mcp: paths[3].as_deref().map(Path::new),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selections_are_bounded_and_checked_before_reading() {
        tauri::async_runtime::block_on(async {
            for paths in [
                [None, None, None, None],
                [Some("SECRET\nsettings.json".into()), None, None, None],
                [Some("x".repeat(4097)), None, None, None],
                [Some("relative/settings.json".into()), None, None, None],
            ] {
                assert_eq!(
                    inspect_qoder_scopes(
                        paths[0].clone(),
                        paths[1].clone(),
                        paths[2].clone(),
                        paths[3].clone()
                    )
                    .await
                    .unwrap_err(),
                    "QODER_SCOPE_INVALID_SELECTION"
                );
            }
            let p = std::env::temp_dir()
                .join("settings.json")
                .to_string_lossy()
                .into_owned();
            assert_eq!(
                inspect_qoder_scopes(Some(p.clone()), Some(p), None, None)
                    .await
                    .unwrap_err(),
                "QODER_SCOPE_INVALID_SELECTION"
            );
        });
    }
    #[test]
    fn explicit_synthetic_scope_does_not_leak_or_write() {
        let dir = std::env::temp_dir().join(format!(
            "qoder-scope-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("settings.json");
        let bytes =
            br#"{"mcpServers":{"SECRET_NAME":{"token":"SECRET_VALUE"}},"model":"SECRET_MODEL"}"#;
        std::fs::write(&path, bytes).unwrap();
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        let report = tauri::async_runtime::block_on(inspect_qoder_scopes(
            Some(path.to_string_lossy().into_owned()),
            None,
            None,
            None,
        ))
        .unwrap();
        let output = serde_json::to_string(&report).unwrap();
        assert!(!output.contains("SECRET"));
        assert!(!output.contains(&dir.to_string_lossy().to_string()));
        #[cfg(any(
            windows,
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            )
        ))]
        assert_eq!(report.scopes[0].status, qoder::Status::Read);
        #[cfg(not(any(
            windows,
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            )
        )))]
        assert_eq!(report.scopes[0].status, qoder::Status::Unsupported);
        assert_eq!(report.scopes[1].status, qoder::Status::NotSelected);
        assert!(!report.can_apply && !report.can_restore);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
