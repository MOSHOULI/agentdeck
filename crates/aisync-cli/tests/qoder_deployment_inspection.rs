use std::process::Command;
#[test]
fn invalid_private_deployment_path_is_not_echoed() {
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args([
            "qoder",
            "inspect-deployment",
            "--deployment",
            "SECRET-relative-path",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("SECRET"));
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn report_states_do_not_authorize_mutation_or_claim_installation() {
    use std::{fs, os::unix::fs::PermissionsExt};
    let parent = std::env::temp_dir().join(format!(
        "qoder-deployment-inspect-cli-{}",
        std::process::id()
    ));
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let target = parent.join("deployment");
    for state in ["missing", "incomplete", "unsupported_or_changed"] {
        if state == "incomplete" {
            fs::create_dir(&target).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        }
        if state == "unsupported_or_changed" {
            fs::write(target.join("sentinel"), b"do not change").unwrap();
        }
        let before = fs::metadata(&parent).unwrap().modified().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
            .args([
                "qoder",
                "inspect-deployment",
                "--release",
                "1.1.86",
                "--deployment",
            ])
            .arg(&target)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["state"], state);
        assert_eq!(report["version"], "1.1.86");
        for flag in [
            "execution_verified",
            "activated",
            "ownership_proven",
            "deletion_authorized",
            "repair_authorized",
        ] {
            assert_eq!(report[flag], false);
        }
        assert_eq!(report["manual_review_required"], true);
        assert_eq!(before, fs::metadata(&parent).unwrap().modified().unwrap());
        if state == "missing" {
            assert!(!target.exists());
        }
        if state == "incomplete" {
            assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        }
        if state == "unsupported_or_changed" {
            assert_eq!(fs::read(target.join("sentinel")).unwrap(), b"do not change");
            assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
        }
    }
    fs::remove_dir_all(parent).unwrap();
}
