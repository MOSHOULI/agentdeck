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

// The editor exposes only one non-credential field. Core owns version, path,
// drift, receipt and permission validation; IPC never returns raw I/O errors.
#[tauri::command]
pub async fn preview_qwen_vim(
    settings_path: String,
    runtime_manifest: String,
    enabled: bool,
) -> Result<qwen::owned::Preview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        qwen::owned::preview(
            Path::new(&settings_path),
            Path::new(&runtime_manifest),
            enabled,
        )
        .map_err(edit_error)
    })
    .await
    .map_err(|_| "Qwen 预览未完成。".to_string())?
}

#[tauri::command]
pub async fn apply_qwen_vim(
    settings_path: String,
    runtime_manifest: String,
    enabled: bool,
    token: String,
) -> Result<bool, String> {
    // Reject malformed or unbounded caller input before any filesystem access.
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("预览无效，请重新预览。".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        qwen::owned::apply(
            Path::new(&settings_path),
            Path::new(&runtime_manifest),
            enabled,
            &token,
        )
        .map_err(edit_error)
    })
    .await
    .map_err(|_| "Qwen 操作结果未确认，请检查后再试。".to_string())?
}

#[tauri::command]
pub async fn restore_qwen_vim(
    settings_path: String,
    runtime_manifest: String,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        qwen::owned::restore(Path::new(&settings_path), Path::new(&runtime_manifest))
            .map_err(edit_error)
    })
    .await
    .map_err(|_| "Qwen 撤销结果未确认，请检查后再试。".to_string())?
}

fn edit_error(error: qwen::owned::EditError) -> String {
    use qwen::owned::EditError::*;
    match error {
        Unsupported => "所选文件、版本或权限不受支持。",
        Changed => "文件或预览已变化，请重新预览。",
        AlreadyOwned => "已有受管记录，请先撤销原操作。",
        Drift => "受管字段已被外部修改，未覆盖该修改。",
        Busy => "另一个配置操作正在进行。",
        Io => "操作结果未确认，请检查受管记录后再试。",
    }
    .to_string()
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
    #[cfg(any(
        windows,
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    ))]
    #[test]
    fn native_vim_preview_apply_restore_and_stale_guard() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "qwen-vim-ipc-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let settings = root.join("settings.json");
        let manifest = root.join("package.json");
        let original = br#"{"$version":4,"unrelated":"SECRET","general":{"vimMode":false}}"#;
        std::fs::write(&settings, original).unwrap();
        std::fs::write(
            &manifest,
            br#"{"name":"@qwen-code/qwen-code","version":"0.27.0"}"#,
        )
        .unwrap();
        let s = settings.to_string_lossy().into_owned();
        let m = manifest.to_string_lossy().into_owned();
        tauri::async_runtime::block_on(async {
            let preview = preview_qwen_vim(s.clone(), m.clone(), true).await.unwrap();
            assert!(!serde_json::to_string(&preview).unwrap().contains("SECRET"));
            assert_eq!(std::fs::read(&settings).unwrap(), original);
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
            assert!(
                apply_qwen_vim(s.clone(), m.clone(), true, "bad".into())
                    .await
                    .is_err()
            );
            assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
            std::fs::write(&settings, br#"{"$version":4,"unrelated":"SECRET","general":{"vimMode":false},"changed":true}"#).unwrap();
            assert!(
                apply_qwen_vim(s.clone(), m.clone(), true, preview.token)
                    .await
                    .is_err()
            );
            let next = preview_qwen_vim(s.clone(), m.clone(), true).await.unwrap();
            assert!(
                apply_qwen_vim(s.clone(), m.clone(), true, next.token)
                    .await
                    .unwrap()
            );
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
            assert_eq!(value["general"]["vimMode"], true);
            assert_eq!(value["unrelated"], "SECRET");
            assert!(restore_qwen_vim(s.clone(), m.clone()).await.unwrap());
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
            assert_eq!(value["general"]["vimMode"], false);
            assert_eq!(value["changed"], true);
            assert!(restore_qwen_vim(s.clone(), m.clone()).await.is_err());
        });
        std::fs::remove_dir_all(root).unwrap();
    }
}
