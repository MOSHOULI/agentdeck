//! Fixed Qoder distribution plan and explicit archive verification only.
use aisync_core::qoder::distribution::{self, DistributionPlan, VerifiedPackage};
#[tauri::command]
pub async fn qoder_distribution_plan(target: String) -> Result<DistributionPlan, String> {
    if target.len() > 64 {
        return Err("请选择已审阅的 Qoder 分发目标。".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        distribution::plan(&target).map_err(|_| "请选择已审阅的 Qoder 分发目标。".to_string())
    })
    .await
    .map_err(|_| "Qoder 分发计划未完成。".to_string())?
}
#[tauri::command]
pub async fn verify_qoder_package(
    target: String,
    archive_path: String,
) -> Result<VerifiedPackage, String> {
    if target.len() > 64
        || archive_path.is_empty()
        || archive_path.len() > 4096
        || archive_path.chars().any(char::is_control)
    {
        return Err("请选择有效且长度受限的 Qoder 归档路径与目标。".into());
    }
    tauri::async_runtime::spawn_blocking(move || distribution::verify_package(&target,std::path::Path::new(&archive_path)).map_err(|_| "未能确认包与固定官方摘要一致。请选择正确目标和官方文件名；不受支持或不安全的文件也会被拒绝。".to_string()))
        .await.map_err(|_| "Qoder 包校验未完成。".to_string())?
}
/// Only the core's audited Linux x86_64 observer can return a report.
#[tauri::command]
pub async fn inspect_qoder_cache(
    cache_path: String,
) -> Result<aisync_core::qoder::cache_inspection::CacheObservation, String> {
    if cache_path.trim().is_empty()
        || cache_path.len() > 4096
        || cache_path.chars().any(char::is_control)
    {
        return Err("请选择有效且长度受限的 Qoder 缓存路径。".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        aisync_core::qoder::cache_inspection::inspect_cache(std::path::Path::new(&cache_path))
            .map_err(|error| match error {
                aisync_core::qoder::cache_inspection::InspectError::Unsupported => {
                    "QODER_CACHE_UNSUPPORTED_HOST".to_string()
                }
                _ => "QODER_CACHE_UNAVAILABLE".to_string(),
            })
    })
    .await
    .map_err(|_| "Qoder 缓存诊断未完成。".to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_invalid_paths_are_bounded_and_redacted() {
        tauri::async_runtime::block_on(async {
            for path in [
                "".into(),
                " ".into(),
                "SECRET\npath".into(),
                "x".repeat(4097),
            ] {
                assert!(
                    !inspect_qoder_cache(path)
                        .await
                        .unwrap_err()
                        .contains("SECRET")
                );
            }
        });
    }
    #[test]
    fn cache_observer_never_writes_and_unsupported_hosts_fail_closed() {
        let dir = std::env::temp_dir().join(format!(
            "qoder-cache-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let before = std::fs::metadata(&dir).unwrap().modified().unwrap();
        let result =
            tauri::async_runtime::block_on(inspect_qoder_cache(dir.to_string_lossy().into_owned()));
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            let report = result.unwrap();
            assert_eq!(
                report.state,
                aisync_core::qoder::cache_inspection::CacheState::UnknownOwnership
            );
            assert!(!report.deletion_authorized);
        }
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let error = result.unwrap_err();
            assert!(!error.contains(&dir.to_string_lossy().to_string()));
            assert_eq!(error, "QODER_CACHE_UNSUPPORTED_HOST");
        }
        assert_eq!(before, std::fs::metadata(&dir).unwrap().modified().unwrap());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn every_plan_remains_fixed_and_non_executable() {
        tauri::async_runtime::block_on(async {
            for target in distribution::TARGETS {
                let p = qoder_distribution_plan(target.to_string()).await.unwrap();
                assert_eq!(p.target, *target);
                assert_eq!(p.version, "1.1.85");
                assert!(!p.install_supported && !p.execution_verified && !p.signature_verified);
            }
            assert!(
                !qoder_distribution_plan("SECRET".into())
                    .await
                    .unwrap_err()
                    .contains("SECRET")
            );
        });
    }
    #[test]
    fn invalid_archive_is_redacted_and_preserved() {
        let dir = std::env::temp_dir().join(format!(
            "qoder-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let p = dir.join("qodercli-linux-x64-baseline.tar.gz");
        std::fs::write(&p, b"SECRET").unwrap();
        let before = std::fs::metadata(&p).unwrap().modified().unwrap();
        let e = tauri::async_runtime::block_on(verify_qoder_package(
            "linux-amd64-baseline".into(),
            p.to_string_lossy().into_owned(),
        ))
        .unwrap_err();
        assert!(!e.contains("SECRET"));
        assert!(!e.contains(&dir.to_string_lossy().to_string()));
        assert_eq!(std::fs::read(&p).unwrap(), b"SECRET");
        assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), before);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_inputs_rejected_before_any_read() {
        tauri::async_runtime::block_on(async {
            for path in [
                "".to_string(),
                "SECRET\narchive".to_string(),
                "x".repeat(4097),
            ] {
                let e = verify_qoder_package("linux-amd64-baseline".into(), path)
                    .await
                    .unwrap_err();
                assert!(!e.contains("SECRET"));
            }
            assert!(qoder_distribution_plan("x".repeat(65)).await.is_err());
        });
    }
}
