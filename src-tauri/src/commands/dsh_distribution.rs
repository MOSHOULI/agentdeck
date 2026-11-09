//! Fixed DSH root plan and explicit read-only verification, no dependency loader.
use aisync_core::dsh::distribution::{self, DistributionPlan, VerifiedPackage};
#[tauri::command]
pub fn dsh_distribution_plan() -> DistributionPlan {
    distribution::plan()
}
#[tauri::command]
pub async fn verify_dsh_package(archive_path: String) -> Result<VerifiedPackage, String> {
    if archive_path.trim().is_empty()
        || archive_path.len() > 4096
        || archive_path.chars().any(char::is_control)
    {
        return Err("DSH_PACKAGE_INVALID_PATH".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        distribution::verify_package(std::path::Path::new(&archive_path)).map_err(|e| match e {
            distribution::DistributionError::Unsupported => {
                "DSH_PACKAGE_UNSUPPORTED_HOST".to_string()
            }
            _ => "DSH_PACKAGE_NOT_VERIFIED".to_string(),
        })
    })
    .await
    .map_err(|_| "DSH_PACKAGE_NOT_VERIFIED".to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_plan_has_no_runtime_or_install_claim() {
        let p = dsh_distribution_plan();
        assert_eq!(p.version, "0.2.2-rc.1");
        assert!(
            !p.installed
                && !p.dependencies_resolved
                && !p.runtime_verified
                && !p.signature_verified
        );
    }
    #[test]
    fn invalid_inputs_are_bounded_and_redacted() {
        tauri::async_runtime::block_on(async {
            for path in ["".into(), "SECRET\narchive".into(), "x".repeat(4097)] {
                assert_eq!(
                    verify_dsh_package(path).await.unwrap_err(),
                    "DSH_PACKAGE_INVALID_PATH"
                );
            }
        });
    }
    #[test]
    fn wrong_archive_remains_unchanged() {
        let dir = std::env::temp_dir().join(format!(
            "dsh-dist-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let p = dir.join(distribution::ARCHIVE_NAME);
        std::fs::write(&p, b"SECRET").unwrap();
        let before = std::fs::metadata(&p).unwrap().modified().unwrap();
        let error =
            tauri::async_runtime::block_on(verify_dsh_package(p.to_string_lossy().into_owned()))
                .unwrap_err();
        assert!(matches!(
            error.as_str(),
            "DSH_PACKAGE_NOT_VERIFIED" | "DSH_PACKAGE_UNSUPPORTED_HOST"
        ));
        assert!(!error.contains("SECRET"));
        assert_eq!(std::fs::read(&p).unwrap(), b"SECRET");
        assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), before);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_file(p).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
