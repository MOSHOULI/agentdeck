use std::{fs, process::Command};
#[test]
fn fixed_root_plan_is_offline_and_overrides_are_not_accepted() {
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["dsh", "distribution-plan"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let p: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(p["package"], "@deepseek-ai/dsh");
    assert_eq!(p["version"], "0.2.2-rc.1");
    assert_eq!(p["release_channel"], "prerelease");
    assert_eq!(p["direct_dependency_count"], 90);
    assert_eq!(p["dependencies_resolved"], false);
    assert_eq!(p["installed"], false);
    assert_eq!(p["runtime_verified"], false);
    for args in [
        vec!["dsh", "distribution-plan", "--version", "latest"],
        vec!["dsh", "verify-package"],
        vec![
            "dsh",
            "verify-package",
            "--archive",
            "file.tgz",
            "--sha256",
            "fake",
        ],
        vec![
            "dsh",
            "verify-package",
            "--archive",
            "file.tgz",
            "--url",
            "https://example.com",
        ],
    ] {
        let o = Command::new(env!("CARGO_BIN_EXE_aisync"))
            .args(args)
            .output()
            .unwrap();
        assert!(!o.status.success());
    }
}
#[test]
fn mismatch_returns_nonzero_without_json_leak_or_mutation() {
    let dir = std::env::temp_dir().join(format!(
        "dsh-dist-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("dsh-0.2.2-rc.1.tgz");
    fs::write(&path, b"SECRET").unwrap();
    let before = fs::metadata(&path).unwrap().modified().unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["dsh", "verify-package", "--archive"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(o.stdout.is_empty());
    let error = String::from_utf8_lossy(&o.stderr);
    assert!(!error.contains("SECRET"));
    assert!(!error.contains(&path.to_string_lossy().to_string()));
    assert_eq!(fs::read(&path).unwrap(), b"SECRET");
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}
