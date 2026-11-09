use std::{fs, path::PathBuf, process::Command};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "dsh-evidence-cli-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_aisync"));
        command
            .current_dir(&self.0)
            .env("HOME", &self.0)
            .env("AGENTDECK_TEST_HOME", &self.0)
            .env("DSH_HOME", &self.0)
            .env("USERPROFILE", &self.0)
            .env("APPDATA", &self.0)
            .env("LOCALAPPDATA", &self.0)
            .env("XDG_CONFIG_HOME", &self.0)
            .env("XDG_DATA_HOME", &self.0)
            .env("PATH", "");
        command
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn embedded_report_ignores_local_manifests_and_needs_no_package_runtime() {
    let fixture = Fixture::new();
    let path = fixture.0.join("package.json");
    let local = b"PRIVATE LOCAL DATA: not a JSON manifest";
    fs::write(&path, local).unwrap();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let first = fixture
        .command()
        .args(["dsh", "dependency-evidence"])
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(first.stderr.is_empty());
    let second = fixture
        .command()
        .args(["dsh", "dependency-evidence"])
        .output()
        .unwrap();
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report["mode"], "fixed_offline_dependency_evidence");
    assert_eq!(report["root"]["direct_dependency_count"], 90);
    assert_eq!(report["root"]["direct_exact_version_declaration_count"], 82);
    assert_eq!(
        report["illustrative_exact_metadata"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    for field in [
        "closure_complete",
        "install_ready",
        "dependencies_resolved",
        "runtime_verified",
        "signature_verified",
    ] {
        assert_eq!(report[field], false);
    }
    assert!(!String::from_utf8_lossy(&first.stdout).contains("PRIVATE LOCAL DATA"));
    assert_eq!(fs::read(&path).unwrap(), local);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
}

#[test]
fn dependency_evidence_rejects_all_input_and_mutation_overrides() {
    let fixture = Fixture::new();
    for extra in [
        vec!["--version", "latest"],
        vec!["--source", "https://example.com"],
        vec!["--target", "linux-x64"],
        vec!["--archive", "root.tgz"],
        vec!["--sha256", "fake"],
        vec!["--profile-dir", "profile"],
        vec!["--install"],
        vec!["--yes"],
        vec!["file.json"],
    ] {
        let output = fixture
            .command()
            .args(["dsh", "dependency-evidence"])
            .args(extra)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
    let help = fixture
        .command()
        .args(["dsh", "dependency-evidence", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    for text in [
        "embedded fixtures",
        "historical",
        "not artifact verification",
        "Exit 0",
        "No version",
    ] {
        assert!(help.contains(text), "{help}");
    }
}
