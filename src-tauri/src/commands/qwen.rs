//! Explicit Qwen file inspection. No registry, initialization, credentials or execution.
use aisync_core::qwen::{self, QwenInspectionReport};
use std::path::Path;

fn inspect_selected(settings: &Path, manifest: &Path) -> Result<QwenInspectionReport, String> {
    qwen::validate_settings_path(settings)
        .map_err(|_| "请选择 settings.json 的绝对路径。".to_string())?;
    qwen::validate_runtime_manifest_path(manifest)
        .map_err(|_| "请选择 package.json 的绝对路径。".to_string())?;
    Ok(qwen::inspect_settings(settings, Some(manifest)))
}

#[tauri::command]
pub async fn inspect_qwen_settings(
    settings_path: String,
    runtime_manifest: String,
) -> Result<QwenInspectionReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        inspect_selected(Path::new(&settings_path), Path::new(&runtime_manifest))
    })
    .await
    .map_err(|_| "Qwen 配置检查未完成。".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_paths_are_rejected_without_echoing_input() {
        let root = std::env::temp_dir();
        for (settings, manifest) in [
            (
                Path::new("SECRET/settings.json").to_path_buf(),
                root.join("package.json"),
            ),
            (root.join("settings.json"), root.join("credentials.json")),
        ] {
            let error = inspect_selected(&settings, &manifest).unwrap_err();
            assert!(!error.contains("SECRET"));
            assert!(!error.contains("credentials"));
        }
    }

    #[cfg(any(
        windows,
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    ))]
    #[test]
    fn native_async_summary_preserves_files_and_hides_values() {
        let root = std::env::temp_dir().join(format!(
            "qwen-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let settings = root.join("settings.json");
        let manifest = root.join("package.json");
        let credentials = root.join("oauth_creds.json");
        let bytes = br#"{"$version":4,"general":{"SECRET":"PRIVATE"},"model":{"name":"PRIVATE"}}"#;
        std::fs::write(&settings, bytes).unwrap();
        std::fs::write(
            &manifest,
            br#"{"name":"@qwen-code/qwen-code","version":"0.27.0"}"#,
        )
        .unwrap();
        std::fs::write(&credentials, b"PRIVATE_CREDENTIAL").unwrap();
        let before = std::fs::metadata(&settings).unwrap().modified().unwrap();
        let report = tauri::async_runtime::block_on(inspect_qwen_settings(
            settings.to_string_lossy().into_owned(),
            manifest.to_string_lossy().into_owned(),
        ))
        .unwrap();
        assert!(matches!(
            report.settings.status,
            qwen::ObservationStatus::Read
        ));
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("PRIVATE"));
        assert!(!json.contains("SECRET"));
        assert_eq!(std::fs::read(&settings).unwrap(), bytes);
        assert_eq!(
            std::fs::metadata(&settings).unwrap().modified().unwrap(),
            before
        );
        assert_eq!(std::fs::read(&credentials).unwrap(), b"PRIVATE_CREDENTIAL");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 3);
        std::fs::remove_dir_all(root).unwrap();
    }
}
