use std::process::Command;
#[test]
fn cache_inspection_errors_do_not_echo_private_paths_or_create_data() {
    let path = std::env::temp_dir().join("SECRET-nonexistent-qoder-cache-inspection");
    assert!(!path.exists());
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["qoder", "inspect-cache", "--cache"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("SECRET"));
    assert!(!path.exists());
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn unknown_cache_is_a_report_not_deletion_permission() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let path = std::env::temp_dir().join(format!("qoder-inspection-cli-{}", std::process::id()));
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let before = fs::metadata(&path).unwrap().modified().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["qoder", "inspect-cache", "--cache"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["state"], "unknown_ownership");
    assert_eq!(report["deletion_authorized"], false);
    assert_eq!(report["manual_review_required"], true);
    assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
    assert_eq!(before, fs::metadata(&path).unwrap().modified().unwrap());
    fs::remove_dir(&path).unwrap();
}
