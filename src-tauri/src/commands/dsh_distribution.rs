//! Fixed DSH root plan and explicit read-only archive verification, no dependency loader.
use aisync_core::dsh::distribution::{self, DistributionPlan, VerifiedPackage};
use aisync_core::dsh::native_archive::{self, NativePackage, VerifiedNative};
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
/// Compact read-only result: declarations and verbose source records stay in
/// the audited evidence, rather than crossing the desktop IPC boundary.
#[derive(Debug, serde::Serialize)]
pub struct NativeArchiveVerification {
    schema_version: u32,
    state: &'static str,
    package_id: String,
    package: String,
    version: String,
    archive_name: String,
    bytes: usize,
    sha256: String,
    integrity: String,
    regular_member_count: usize,
    archive_verified_during_this_command: bool,
    reviewed_manifest_and_sources_matched: bool,
    metadata_archive_agreement: &'static str,
    lifecycle_executed: bool,
    native_code_loaded: bool,
    closure_complete: bool,
    install_ready: bool,
    dependencies_resolved: bool,
    runtime_verified: bool,
    signature_verified: bool,
    platform_compatibility: &'static str,
}
impl From<VerifiedNative> for NativeArchiveVerification {
    fn from(v: VerifiedNative) -> Self {
        Self {
            schema_version: v.schema_version,
            state: v.state,
            package_id: v.package_id,
            package: v.package,
            version: v.version,
            archive_name: v.archive_name,
            bytes: v.bytes,
            sha256: v.sha256,
            integrity: v.integrity,
            regular_member_count: v.regular_member_count,
            archive_verified_during_this_command: v.archive_verified_during_this_command,
            reviewed_manifest_and_sources_matched: v.reviewed_manifest_and_sources_matched,
            metadata_archive_agreement: v.metadata_archive_agreement,
            lifecycle_executed: v.lifecycle_executed,
            native_code_loaded: v.native_code_loaded,
            closure_complete: v.closure_complete,
            install_ready: v.install_ready,
            dependencies_resolved: v.dependencies_resolved,
            runtime_verified: v.runtime_verified,
            signature_verified: v.signature_verified,
            platform_compatibility: v.platform_compatibility,
        }
    }
}
/// Only the four reviewed IDs are accepted; package versions, sources and limits
/// remain compiled into the core verifier. All failures are path-free codes.
#[tauri::command]
pub async fn verify_dsh_native_package(
    package_id: String,
    archive_path: String,
) -> Result<NativeArchiveVerification, String> {
    let package = match package_id.as_str() {
        "node-pty" => NativePackage::NodePty,
        "koffi" => NativePackage::Koffi,
        "koffi-linux-x64" => NativePackage::KoffiLinuxX64,
        "koffi-win32-x64" => NativePackage::KoffiWin32X64,
        _ => return Err("DSH_NATIVE_INVALID_PACKAGE".into()),
    };
    if archive_path.trim().is_empty()
        || archive_path.len() > 4096
        || archive_path.chars().any(char::is_control)
        || !std::path::Path::new(&archive_path).is_absolute()
    {
        return Err("DSH_NATIVE_INVALID_PATH".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        native_archive::verify_native_package(package, std::path::Path::new(&archive_path))
            .map(NativeArchiveVerification::from)
            .map_err(|_| "DSH_NATIVE_NOT_VERIFIED".to_string())
    })
    .await
    .map_err(|_| "DSH_NATIVE_NOT_VERIFIED".to_string())?
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
    #[test]
    fn native_ids_are_exact_and_errors_do_not_echo_input() {
        tauri::async_runtime::block_on(async {
            for id in [
                "".into(),
                "Node-Pty".into(),
                " node-pty".into(),
                "koffi@3.1.1".into(),
                "@koromix/koffi-linux-x64".into(),
                "SECRET".repeat(1000),
            ] {
                assert_eq!(
                    verify_dsh_native_package(id, "SECRET".into())
                        .await
                        .unwrap_err(),
                    "DSH_NATIVE_INVALID_PACKAGE"
                );
            }
            for id in ["node-pty", "koffi", "koffi-linux-x64", "koffi-win32-x64"] {
                for path in [
                    "".into(),
                    " ".into(),
                    "relative/SECRET.tgz".into(),
                    "/SECRET\narchive".into(),
                    "/SECRET\0archive".into(),
                    "/SECRET\u{85}archive".into(),
                    format!("/{}", "界".repeat(1400)),
                    format!("/{}", "x".repeat(4096)),
                ] {
                    assert_eq!(
                        verify_dsh_native_package(id.into(), path)
                            .await
                            .unwrap_err(),
                        "DSH_NATIVE_INVALID_PATH"
                    );
                }
            }
        });
    }

    #[test]
    fn native_wrong_archives_remain_unchanged() {
        let dir = std::env::temp_dir().join(format!(
            "dsh-native-ipc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        for (id, name) in [
            ("node-pty", "node-pty-1.2.0-beta.15.tgz"),
            ("koffi", "koffi-3.1.1.tgz"),
            ("koffi-linux-x64", "koffi-linux-x64-3.1.1.tgz"),
            ("koffi-win32-x64", "koffi-win32-x64-3.1.1.tgz"),
        ] {
            let p = dir.join(name);
            std::fs::write(&p, b"SECRET inert archive, never executable").unwrap();
            let before = std::fs::metadata(&p).unwrap().modified().unwrap();
            let error = tauri::async_runtime::block_on(verify_dsh_native_package(
                id.into(),
                p.to_string_lossy().into_owned(),
            ))
            .unwrap_err();
            assert_eq!(error, "DSH_NATIVE_NOT_VERIFIED");
            assert_eq!(
                std::fs::read(&p).unwrap(),
                b"SECRET inert archive, never executable"
            );
            assert_eq!(std::fs::metadata(&p).unwrap().modified().unwrap(), before);
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 4);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn native_projection_preserves_evidence_without_source_records() {
        // Inert serialization fixture, not a successful archive verification.
        let verified = VerifiedNative {
            schema_version: 1,
            state: "native_archive_static_verified",
            package_id: "koffi".into(),
            package: "koffi".into(),
            version: "3.1.1".into(),
            archive_name: "koffi-3.1.1.tgz".into(),
            bytes: 302_173,
            sha256: "fixture hash".into(),
            integrity: "fixture integrity".into(),
            regular_member_count: 81,
            archive_verified_during_this_command: true,
            reviewed_manifest_and_sources_matched: true,
            metadata_archive_agreement: "reviewed_declaration_fields_match",
            declarations: serde_json::json!({"scripts": "not transmitted"}),
            lifecycle_executed: false,
            native_code_loaded: false,
            closure_complete: false,
            install_ready: false,
            dependencies_resolved: false,
            runtime_verified: false,
            signature_verified: false,
            platform_compatibility: "unverified",
            limitations: "not transmitted",
        };
        let mut expected = serde_json::to_value(&verified).unwrap();
        let object = expected.as_object_mut().unwrap();
        object.remove("declarations");
        object.remove("limitations");
        assert_eq!(object.len(), 21);
        assert_eq!(
            serde_json::to_value(NativeArchiveVerification::from(verified)).unwrap(),
            expected
        );
    }
}
