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
pub async fn preview_qwen_vim_update(
    settings_path: String,
    runtime_manifest: String,
    enabled: bool,
) -> Result<qwen::owned::Preview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        qwen::owned::update_preview(
            Path::new(&settings_path),
            Path::new(&runtime_manifest),
            enabled,
        )
        .map_err(edit_error)
    })
    .await
    .map_err(|_| "Qwen 预览未完成。".to_string())?
}

/// Separate read-only restore confirmation; observation does not grant this token.
#[tauri::command]
pub async fn preview_qwen_vim_restore(
    settings_path: String,
    runtime_manifest: String,
) -> Result<qwen::owned::RestorePreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        qwen::owned::restore_preview(Path::new(&settings_path), Path::new(&runtime_manifest))
            .map_err(edit_error)
    })
    .await
    .map_err(|_| "Qwen 预览未完成。".to_string())?
}

/// Observation only: does not acquire a lock or authorize a later restore.
#[tauri::command]
pub async fn inspect_qwen_vim_recovery(
    settings_path: String,
    runtime_manifest: String,
) -> Result<qwen::owned::RecoveryPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        qwen::owned::recovery_preview(Path::new(&settings_path), Path::new(&runtime_manifest))
            .map_err(edit_error)
    })
    .await
    .map_err(|_| "Qwen 恢复诊断未完成。".to_string())?
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
pub async fn update_qwen_vim(
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
        qwen::owned::update(
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
    token: String,
) -> Result<bool, String> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("预览无效，请重新预览。".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        qwen::owned::restore_checked(
            Path::new(&settings_path),
            Path::new(&runtime_manifest),
            &token,
        )
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
        AlreadyOwned => "已有受管记录，请选择更新受管值或撤销原操作。",
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
            let update = preview_qwen_vim_update(s.clone(), m.clone(), false)
                .await
                .unwrap();
            assert_eq!(update.before, Some(true));
            assert!(!serde_json::to_string(&update).unwrap().contains("SECRET"));
            assert!(
                update_qwen_vim(s.clone(), m.clone(), false, "bad".into())
                    .await
                    .is_err()
            );
            assert!(
                update_qwen_vim(s.clone(), m.clone(), false, update.token.clone())
                    .await
                    .unwrap()
            );
            assert!(
                update_qwen_vim(s.clone(), m.clone(), false, update.token)
                    .await
                    .is_err()
            );
            let update = preview_qwen_vim_update(s.clone(), m.clone(), true)
                .await
                .unwrap();
            assert_eq!(update.before, Some(false));
            assert!(
                update_qwen_vim(s.clone(), m.clone(), true, update.token)
                    .await
                    .unwrap()
            );
            let restore = preview_qwen_vim_restore(s.clone(), m.clone())
                .await
                .unwrap();
            assert_eq!(restore.current, Some(true));
            assert_eq!(restore.initial, Some(false));
            assert!(!serde_json::to_string(&restore).unwrap().contains("SECRET"));
            let receipt_path = root.join(".agentdeck-qwen-vim.json");
            let receipt = std::fs::read(&receipt_path).unwrap();
            let mut edited: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
            edited["after_preview"] = serde_json::json!(true);
            let changed = serde_json::to_vec(&edited).unwrap();
            std::fs::write(&settings, &changed).unwrap();
            assert!(
                restore_qwen_vim(s.clone(), m.clone(), restore.token)
                    .await
                    .is_err()
            );
            assert_eq!(std::fs::read(&settings).unwrap(), changed);
            assert_eq!(std::fs::read(&receipt_path).unwrap(), receipt);
            assert!(
                restore_qwen_vim(s.clone(), m.clone(), "BAD".into())
                    .await
                    .is_err()
            );
            let fresh = preview_qwen_vim_restore(s.clone(), m.clone())
                .await
                .unwrap();
            assert!(
                restore_qwen_vim(s.clone(), m.clone(), fresh.token)
                    .await
                    .unwrap()
            );
            let value: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
            assert_eq!(value["general"]["vimMode"], false);
            assert_eq!(value["changed"], true);
            assert_eq!(value["after_preview"], true);
            assert!(
                preview_qwen_vim_restore(s.clone(), m.clone())
                    .await
                    .is_err()
            );
            assert!(
                restore_qwen_vim(s.clone(), m.clone(), "0".repeat(64))
                    .await
                    .is_err()
            );
        });
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
    fn native_recovery_observes_all_states_without_writes() {
        use qwen::owned::RecoveryState;
        let root = std::env::temp_dir().join(format!(
            "qwen-recovery-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let settings = root.join("settings.json");
        let manifest = root.join("package.json");
        let receipt = root.join(".agentdeck-qwen-vim.json");
        let original = br#"{"$version":4,"general":{"vimMode":false},"unrelated":"SECRET"}"#;
        std::fs::write(&settings, original).unwrap();
        std::fs::write(
            &manifest,
            br#"{"name":"@qwen-code/qwen-code","version":"0.27.0"}"#,
        )
        .unwrap();
        let snapshot = || -> std::collections::BTreeMap<_, _> {
            std::fs::read_dir(&root)
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    (entry.file_name(), std::fs::read(entry.path()).unwrap())
                })
                .collect()
        };
        let observe = |expected| {
            let before = snapshot();
            let report = tauri::async_runtime::block_on(inspect_qwen_vim_recovery(
                settings.to_string_lossy().into_owned(),
                manifest.to_string_lossy().into_owned(),
            ))
            .unwrap();
            assert_eq!(report.state, expected);
            assert_eq!(
                report.changes_settings,
                expected == RecoveryState::RestoreField
            );
            let serialized = serde_json::to_string(&report).unwrap();
            assert!(!serialized.contains("SECRET"));
            assert!(!serialized.contains("digest"));
            assert!(!serialized.contains(&root.to_string_lossy().to_string()));
            assert_eq!(snapshot(), before);
        };
        observe(RecoveryState::NoReceipt);
        assert_eq!(snapshot().len(), 2); // No lock, receipt, or initialization.
        let preview = qwen::owned::preview(&settings, &manifest, true).unwrap();
        qwen::owned::apply(&settings, &manifest, true, &preview.token).unwrap();
        observe(RecoveryState::RestoreField);
        std::fs::write(
            &settings,
            br#"{"$version":4,"general":{"vimMode":false},"unrelated":"SECRET","external":true}"#,
        )
        .unwrap();
        observe(RecoveryState::Conflict);
        std::fs::write(&settings, original).unwrap();
        observe(RecoveryState::CleanupReceipt);
        assert!(receipt.exists()); // Observation must never clean the receipt.
        std::fs::write(&receipt, b"SECRET invalid receipt").unwrap();
        let before = snapshot();
        let error = tauri::async_runtime::block_on(inspect_qwen_vim_recovery(
            settings.to_string_lossy().into_owned(),
            manifest.to_string_lossy().into_owned(),
        ))
        .unwrap_err();
        assert!(!error.contains("SECRET"));
        assert_eq!(snapshot(), before);
        assert!(
            tauri::async_runtime::block_on(inspect_qwen_vim_recovery(
                "SECRET/settings.json".into(),
                "SECRET/package.json".into()
            ))
            .is_err()
        );
        assert_eq!(snapshot(), before);
        std::fs::remove_dir_all(root).unwrap();
    }
}
