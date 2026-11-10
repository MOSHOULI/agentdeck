use std::process::Command;
#[test]
fn selected_codex_slot_errors_are_redacted_and_create_nothing() {
    let p = std::env::temp_dir().join(format!("SECRET-residual-missing-{}", std::process::id()));
    assert!(!p.exists());
    let out = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["codex-residual", "--prefix"])
        .arg(&p)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&out.stderr).contains("SECRET"));
    assert!(!p.exists());
}
#[cfg(windows)]
#[test]
fn windows_selected_codex_slots_cli_never_executes_or_reports_contents() {
    use std::fs;
    let p = std::env::temp_dir().join(format!("residual-cli-{}", std::process::id()));
    fs::create_dir(&p).unwrap();
    let cmd = p.join("codex.cmd");
    let marker = p.join("executed");
    let bytes = format!("@echo SECRET-CONTENT > \"{}\"\r\n", marker.display());
    fs::write(&cmd, &bytes).unwrap();
    let before = fs::metadata(&cmd).unwrap().modified().unwrap();
    // Fake npm would execute the same sentinel if the command incorrectly probes it.
    fs::write(p.join("npm.cmd"), &bytes).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["codex-residual", "--prefix"])
        .arg(&p)
        .env("PATH", &p)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["slots"][0]["state"], "missing");
    assert_eq!(value["slots"][2]["state"], "file");
    assert_eq!(value["runtimeNotRun"], true);
    assert_eq!(value["cleanupSupported"], false);
    assert!(!String::from_utf8_lossy(&out.stdout).contains("SECRET-CONTENT"));
    assert!(!marker.exists());
    assert_eq!(fs::read_to_string(&cmd).unwrap(), bytes);
    assert_eq!(fs::metadata(cmd).unwrap().modified().unwrap(), before);
    fs::remove_dir_all(p).unwrap();
}
