use std::process::Command;

#[test]
fn linux_static_failure_is_redacted_and_has_no_side_effects() {
    let missing = std::env::temp_dir().join(format!("SECRET-linux-static-{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["codex-static-linux", "--prefix"])
        .arg(&missing)
        .arg("--node")
        .arg(missing.join("node"))
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("SECRET"));
    assert!(!missing.exists());
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn linux_static_success_never_executes_or_reads_path_candidates() {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    let root = std::env::temp_dir().join(format!("static-cli 中文-{}", std::process::id()));
    let prefix = root.join("prefix");
    let package = prefix.join("lib/node_modules/@openai/codex");
    fs::create_dir_all(package.join("bin")).unwrap();
    fs::create_dir_all(prefix.join("bin")).unwrap();
    fs::write(
        package.join("package.json"),
        br#"{"name":"@openai/codex","version":"SECRET","bin":{"codex":"bin/codex.js"}}"#,
    )
    .unwrap();
    let script = b"#!/usr/bin/env node\nthrow Error('MUST_NOT_RUN');\n";
    fs::write(package.join("bin/codex.js"), script).unwrap();
    fs::set_permissions(
        package.join("bin/codex.js"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    symlink(
        "../lib/node_modules/@openai/codex/bin/codex.js",
        prefix.join("bin/codex"),
    )
    .unwrap();
    // Header-only fixture cannot run: success proves this observation is static.
    let node = root.join("node");
    let mut header = [0; 64];
    header[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    header[16] = 3;
    header[18] = 62;
    header[20] = 1;
    fs::write(&node, header).unwrap();
    fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(root.join("npm"), b"#!/bin/sh\nexit 99\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["codex-static-linux", "--prefix"])
        .arg(&prefix)
        .arg("--node")
        .arg(&node)
        .env("PATH", &root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        serde_json::json!({
        "schemaVersion":1,"agentId":"codex","observation":"selected_layout_matched",
        "runtimeStatus":"runtime_not_run","searchScope":"selected_prefix_only","selectedLauncher":"bin/codex",
        "applySupported":false,"upgradeSupported":false,"uninstallSupported":false})
    );
    assert_eq!(fs::read(package.join("bin/codex.js")).unwrap(), script);
    assert_eq!(fs::read(&node).unwrap(), header);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 3);
    fs::remove_dir_all(root).unwrap();
}
