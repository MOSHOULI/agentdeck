//! Explicit, passive manifest inspection. This path must not load the registry,
//! initialize an application home, invoke a tool, or prepare a DSH profile.
use std::path::Path;

use aisync_core::dsh::{self, DshInventoryReport};

fn inspect_selected_with(
    profile_dir: &Path,
    runtime_manifest: Option<&Path>,
    inspect: impl FnOnce(&Path, Option<&Path>) -> DshInventoryReport,
) -> Result<DshInventoryReport, String> {
    // Validate both inputs before permitting even the first filesystem read.
    dsh::validate_profile_path(profile_dir)
        .map_err(|_| "Profile 路径不受支持，请选择绝对目录路径。".to_string())?;
    if let Some(path) = runtime_manifest {
        dsh::validate_runtime_manifest_path(path)
            .map_err(|_| "Runtime 路径不受支持，请选择 package.json 的绝对路径。".to_string())?;
    }
    Ok(inspect(profile_dir, runtime_manifest))
}

/// Only the caller-selected package manifests are inspected. All mutation
/// capabilities remain false; declaration evidence never becomes runtime state.
#[tauri::command]
pub async fn inspect_dsh_profile(
    profile_dir: String,
    runtime_manifest: Option<String>,
) -> Result<DshInventoryReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        inspect_selected_with(
            Path::new(&profile_dir),
            runtime_manifest.as_deref().map(Path::new),
            dsh::inspect_profile,
        )
    })
    .await
    .map_err(|_| "DSH 声明盘点未完成，请重试。".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn invalid_selections_never_reach_the_inspector() {
        let root = std::env::temp_dir().join("dsh-unobserved-ipc-fixture");
        for (profile, runtime) in [
            (PathBuf::from("relative"), None),
            (root.join("../parent"), None),
            (root.clone(), Some(PathBuf::from("package.json"))),
            (root.clone(), Some(root.join("credentials.json"))),
        ] {
            assert!(
                inspect_selected_with(&profile, runtime.as_deref(), |_, _| {
                    panic!("Invalid IPC inputs must not inspect any file")
                })
                .is_err()
            );
        }
    }

    #[test]
    fn dispatch_passes_only_explicit_inputs_and_preserves_unknown_capabilities() {
        let root = std::env::temp_dir().join("dsh-unobserved-ipc-fixture");
        let runtime = root.join("runtime/package.json");
        let report = inspect_selected_with(&root, Some(&runtime), |profile, selected| {
            assert_eq!(profile, root);
            assert_eq!(selected, Some(runtime.as_path()));
            // An invalid core input yields a report without filesystem I/O.
            dsh::inspect_profile(Path::new("relative"), None)
        })
        .unwrap();
        let value = serde_json::to_value(report).unwrap();
        for key in ["runtime_status", "authentication", "actual_enabled"] {
            assert_eq!(value[key], "unknown");
        }
        assert!(
            value["mutation_capabilities"]
                .as_object()
                .unwrap()
                .values()
                .all(|value| value == false)
        );
    }

    #[cfg(any(
        windows,
        all(
            target_os = "linux",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )
    ))]
    #[test]
    fn native_dispatch_leaves_synthetic_files_and_missing_profiles_unchanged() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        struct Fixture(PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "agentdeck-dsh-ipc-{}-{}-{}", std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&fixture.0).unwrap();
        let manifest = fixture.0.join("package.json");
        let private = fixture.0.join("credentials.json");
        fs::write(
            &manifest,
            br#"{"dependencies":{"example-plugin":"https://private.invalid/SECRET"}}"#,
        )
        .unwrap();
        fs::write(&private, b"PRIVATE_SENTINEL").unwrap();
        let before = [fixture.0.clone(), manifest.clone(), private.clone()].map(|p| {
            (
                p.clone(),
                fs::metadata(&p).unwrap().modified().unwrap(),
                fs::read(&p).ok(),
            )
        });
        let report = inspect_selected_with(&fixture.0, None, dsh::inspect_profile).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        assert!(matches!(
            report.profile_manifest.status,
            dsh::ManifestStatus::Read
        ));
        assert!(!json.contains("SECRET"));
        assert!(!json.contains("PRIVATE_SENTINEL"));
        assert!(!json.contains(&fixture.0.to_string_lossy().to_string()));
        let missing = fixture.0.join("missing-profile");
        let report = inspect_selected_with(&missing, None, dsh::inspect_profile).unwrap();
        assert!(matches!(
            report.profile_manifest.status,
            dsh::ManifestStatus::Missing
        ));
        assert!(!missing.exists());
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 2);
        for (path, modified, bytes) in before {
            assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
            assert_eq!(fs::read(&path).ok(), bytes);
        }
    }
}
