use std::{fs, process::Command};
#[test]
fn plans_are_explicit_offline_and_never_install() {
    for target in aisync_core::qoder::distribution::TARGETS {
        let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
            .args(["qoder", "distribution-plan", "--target", target])
            .output()
            .unwrap();
        assert!(output.status.success());
        let plan: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(plan["target"], *target);
        assert_eq!(plan["version"], "1.1.85");
        assert_eq!(plan["install_supported"], false);
        assert_eq!(plan["execution_verified"], false);
    }
    for args in [
        vec!["qoder", "distribution-plan"],
        vec!["qoder", "distribution-plan", "--target", "windows-amd64"],
    ] {
        assert!(
            !Command::new(env!("CARGO_BIN_EXE_aisync"))
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
}
#[test]
fn mismatch_returns_nonzero_without_output_or_mutation() {
    let dir = std::env::temp_dir().join(format!(
        "qoder-archive-cli-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    let archive = dir.join("qodercli-linux-x64-baseline.tar.gz");
    fs::write(&archive, b"SECRET").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args([
            "qoder",
            "verify-package",
            "--target",
            "linux-amd64-baseline",
            "--archive",
        ])
        .arg(&archive)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(!error.contains("SECRET"));
    assert!(!error.contains(&dir.to_string_lossy().to_string()));
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    assert_eq!(fs::read(&archive).unwrap(), b"SECRET");
    fs::remove_dir_all(dir).unwrap();
}
