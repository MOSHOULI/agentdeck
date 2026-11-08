//! Real CLI-process regression: dry-run must not initialize application state,
//! repair registry transactions, execute tools, or expose configuration data.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "agentdeck-sync-plan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_aisync"))
            .args(arguments)
            .env("AGENTDECK_TEST_HOME", &self.0)
            .env("HOME", &self.0)
            .env("USERPROFILE", &self.0)
            .env("APPDATA", self.0.join("AppData/Roaming"))
            .env("LOCALAPPDATA", self.0.join("AppData/Local"))
            .env("PATH", self.0.join("bin"))
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn write(&self, path: &str, data: impl AsRef<[u8]>) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, data).unwrap();
    }
    fn snapshot(&self) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
        fn visit(root: &Path, dir: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    result.insert(path.strip_prefix(root).unwrap().to_owned(), None);
                    visit(root, &path, result);
                } else {
                    result.insert(
                        path.strip_prefix(root).unwrap().to_owned(),
                        Some(std::fs::read(path).unwrap()),
                    );
                }
            }
        }
        let mut result = BTreeMap::new();
        visit(&self.0, &self.0, &mut result);
        result
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fresh_home_plan_is_json_and_creates_nothing() {
    let fixture = Fixture::new();
    let output = fixture.run(&["sync", "--tools", "codex,claude-code", "--dry-run"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["mode"], "dry_run");
    assert_eq!(report["batchStatus"], "unverified");
    assert_eq!(report["agents"].as_array().unwrap().len(), 2);
    assert_eq!(
        report["agents"][0]["installationEligibility"],
        "not_checked"
    );
    assert!(fixture.snapshot().is_empty());
}

#[test]
fn configuration_and_memory_content_are_never_returned_or_changed() {
    let fixture = Fixture::new();
    let secret = "SYNTHETIC_SECRET_NOT_FOR_REPORT";
    for file in [
        ".codex/config.toml",
        ".codex/AGENTS.md",
        ".agentdeck/memory.db",
        ".agentdeck/profiles/default/personalization/codex/AGENTS.md",
    ] {
        fixture.write(file, secret);
    }
    // A malicious executable and registry verify recipe must remain inert.
    fixture.write("bin/codex", "#!/bin/sh\necho executed > executed-marker\n");
    fixture.write(
        "bin/codex.cmd",
        "@echo off\necho executed > executed-marker\n",
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            fixture.0.join("bin/codex"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    let before = fixture.snapshot();
    let output = fixture.run(&["sync", "--tools", "codex", "--dry-run"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains(secret));
    assert!(!String::from_utf8_lossy(&output.stderr).contains(secret));
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn invalid_or_conflicting_arguments_do_not_initialize_state() {
    for arguments in [
        vec!["sync", "--dry-run"],
        vec!["sync", "--tools", "codex", "--dry-run", "--yes"],
        vec!["sync", "--tools", "codex,not-a-real-agent", "--dry-run"],
    ] {
        let fixture = Fixture::new();
        let output = fixture.run(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(fixture.snapshot().is_empty());
    }
}

#[test]
fn unsupported_agent_report_does_not_claim_a_valid_batch() {
    let fixture = Fixture::new();
    let output = fixture.run(&["sync", "--tools", "codex,qwen-code", "--dry-run"]);
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["batchStatus"], "unsupported");
    assert_eq!(report["agents"][1]["status"], "unsupported");
    assert_eq!(report["agents"][1]["operations"], serde_json::json!([]));
    assert!(fixture.snapshot().is_empty());
}

#[test]
fn interrupted_registry_update_is_not_recovered() {
    let fixture = Fixture::new();
    fixture.write(
        ".agentdeck/registry-update.json",
        "synthetic invalid recovery journal",
    );
    fixture.write(".agentdeck/registry-update.lock", "");
    let before = fixture.snapshot();
    let output = fixture.run(&["sync", "--tools", "codex", "--dry-run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("will not recover"));
    assert!(output.stdout.is_empty());
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn runtime_registry_without_lock_is_not_silently_repaired() {
    let fixture = Fixture::new();
    fixture.write(
        ".agentdeck/registry/registry-v3.json",
        include_bytes!("../../../agents/registry-v3.json"),
    );
    let before = fixture.snapshot();
    let output = fixture.run(&["sync", "--tools", "codex", "--dry-run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no coordination lock"));
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn existing_runtime_catalog_is_used_without_execution_or_changes() {
    let fixture = Fixture::new();
    let mut registry: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../agents/registry-v3.json")).unwrap();
    let mut custom = registry["agents"]["codex"].clone();
    custom["config_inject"]["personalization"]["target"] =
        serde_json::json!("~/.synthetic/INSTRUCTIONS.md");
    custom["install"]["verify_cmd"] = serde_json::json!("echo execute > executed-marker");
    registry["agents"]["synthetic-custom"] = custom;
    fixture.write(
        ".agentdeck/registry/registry-v3.json",
        serde_json::to_vec(&registry).unwrap(),
    );
    fixture.write(".agentdeck/registry-update.lock", "");
    let before = fixture.snapshot();
    let output = fixture.run(&["sync", "--tools", "synthetic-custom", "--dry-run"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["agents"][0]["operations"][0]["target"],
        "~/.synthetic/INSTRUCTIONS.md"
    );
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn locked_runtime_registry_is_not_read_or_modified() {
    let fixture = Fixture::new();
    fixture.write(".agentdeck/registry-update.lock", "");
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.0.join(".agentdeck/registry-update.lock"))
        .unwrap();
    lock.lock().unwrap();
    let before = fixture.snapshot();
    let output = fixture.run(&["sync", "--tools", "codex", "--dry-run"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("busy"));
    assert_eq!(fixture.snapshot(), before);
}
