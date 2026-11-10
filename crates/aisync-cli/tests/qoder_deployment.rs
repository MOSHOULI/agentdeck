use std::process::Command;
#[test]
fn missing_confirmation_and_token_do_not_touch_destination_or_echo_private_data() {
    let root = std::env::temp_dir().join(format!("qoder-cli-deploy-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let destination = root.join("new");
    let cache = root.join("private-source");
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["qoder", "deploy-private", "--cache"])
        .arg(&cache)
        .arg("--destination")
        .arg(&destination)
        .args(["--preview-token", &"0".repeat(64)])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--yes"));
    assert!(!destination.exists());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-source"));
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["qoder", "deploy-preview", "--cache"])
        .arg(&cache)
        .arg("--destination")
        .arg(&destination)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!destination.exists());
    std::fs::remove_dir(&root).unwrap();
}
