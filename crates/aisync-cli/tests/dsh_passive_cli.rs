//! Linux regression for known tool execution and filesystem content writes.
//! PATH sentinels catch dsh/node/npm/pnpm calls; this is not syscall tracing or
//! proof against arbitrary executables. Access-time updates are not prohibited.
#![cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    root: PathBuf,
    profile: PathBuf,
    runtime_manifest: PathBuf,
    home: PathBuf,
    dsh_home: PathBuf,
    bin: PathBuf,
    marker: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "agentdeck-dsh-cli-process-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self {
            profile: root.join("selected-profile"),
            runtime_manifest: root.join("runtime/package.json"),
            home: root.join("isolated-home"),
            dsh_home: root.join("ambient-dsh-home"),
            bin: root.join("sentinel-bin"),
            marker: root.join("unexpected-process.marker"),
            root,
        };
        for directory in [
            fixture.profile.join("node_modules/fixture-plugin"),
            fixture.runtime_manifest.parent().unwrap().to_path_buf(),
            fixture.home.join(".dsh"),
            fixture.home.join(".config"),
            fixture.home.join(".local/share"),
            fixture.dsh_home.clone(),
            fixture.bin.clone(),
        ] {
            fs::create_dir_all(directory).unwrap();
        }
        for (path, contents) in [
            (
                fixture.profile.join("package.json"),
                r#"{"name":"passive-fixture","dependencies":{"fixture-plugin":"^1.0.0"},"dsh":{"manifestVersion":1,"profile":{"bundles":["fixture-bundle"]}}}"#,
            ),
            (
                fixture.runtime_manifest.clone(),
                r#"{"name":"@deepseek-ai/dsh-app-boot","version":"999.0.0","engines":{"node":">=24.9.0"}}"#,
            ),
            (fixture.profile.join("cordis.yml"), "PRIVATE_CONFIG_FIXTURE"),
            (fixture.profile.join(".env"), "PRIVATE_ENV_FIXTURE"),
            (
                fixture.profile.join("credentials.json"),
                "PRIVATE_CREDENTIAL_FIXTURE",
            ),
            (
                fixture.profile.join("node_modules/fixture-plugin/index.js"),
                "throw new Error('PLUGIN_CODE_FIXTURE');",
            ),
            (
                fixture.home.join(".dsh/package.json"),
                r#"{"dependencies":{"ambient-home-only":"*"}}"#,
            ),
            (
                fixture.dsh_home.join("package.json"),
                r#"{"dependencies":{"ambient-dsh-home-only":"*"}}"#,
            ),
        ] {
            fs::write(path, contents).unwrap();
        }
        for name in ["dsh", "node", "npm", "pnpm"] {
            let path = fixture.bin.join(name);
            fs::write(
                &path,
                "#!/bin/sh\nprintf '%s\\n' \"${0##*/}\" >> \"$DSH_PASSIVE_SENTINEL\"\nexit 97\n",
            )
            .unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fixture
    }

    fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        // Do not change the test process environment, and never inherit the
        // caller's real home, DSH installation, PATH, credentials, or config.
        command
            .env_clear()
            .current_dir(&self.root)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("AGENTDECK_TEST_HOME", &self.home)
            .env("DSH_HOME", &self.dsh_home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .env("PATH", &self.bin)
            .env("DSH_PASSIVE_SENTINEL", &self.marker);
        command
    }

    fn inspect(&self, profile: &Path, runtime: Option<&Path>) -> Output {
        let mut command = self.command(env!("CARGO_BIN_EXE_aisync"));
        command
            .args(["dsh", "inspect", "--profile-dir"])
            .arg(profile);
        if let Some(path) = runtime {
            command.arg("--runtime-manifest").arg(path);
        }
        command.output().unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug, PartialEq, Eq)]
struct EntrySnapshot {
    modified: SystemTime,
    contents: Option<Vec<u8>>,
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, EntrySnapshot> {
    fn visit(root: &Path, path: &Path, entries: &mut BTreeMap<PathBuf, EntrySnapshot>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(
            metadata.is_dir() || metadata.is_file(),
            "fixture entries are regular"
        );
        entries.insert(
            path.strip_prefix(root).unwrap().to_path_buf(),
            EntrySnapshot {
                modified: metadata.modified().unwrap(),
                contents: metadata.is_file().then(|| fs::read(path).unwrap()),
            },
        );
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                visit(root, &entry.unwrap().path(), entries);
            }
        }
    }
    let mut entries = BTreeMap::new();
    visit(root, root, &mut entries);
    entries
}

fn successful_report(output: &Output) -> serde_json::Value {
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["mode"], "passive_manifest_only");
    for field in ["runtime_status", "authentication", "actual_enabled"] {
        assert_eq!(report[field], "unknown");
    }
    assert!(
        report["mutation_capabilities"]
            .as_object()
            .unwrap()
            .values()
            .all(|value| value == false)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for private in [
        "PRIVATE_",
        "PLUGIN_CODE_FIXTURE",
        "ambient-home-only",
        "ambient-dsh-home-only",
    ] {
        assert!(!stdout.contains(private));
    }
    report
}

#[test]
fn sentinel_tools_really_mark_attempted_execution() {
    let fixture = Fixture::new();
    for name in ["dsh", "node", "npm", "pnpm"] {
        // Positive controls execute only our own synthetic shell scripts.
        let output = fixture.command(name).output().unwrap();
        assert_eq!(output.status.code(), Some(97));
    }
    assert_eq!(
        fs::read_to_string(&fixture.marker).unwrap(),
        "dsh\nnode\nnpm\npnpm\n"
    );
}

#[test]
fn static_inventory_does_not_spawn_known_tools_or_change_content_and_mtimes() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.root);
    let output = fixture.inspect(&fixture.profile, Some(&fixture.runtime_manifest));
    let report = successful_report(&output);
    assert_eq!(report["profile_manifest"]["status"], "read");
    assert_eq!(report["runtime_manifest"]["status"], "read");
    assert_eq!(
        report["version_recognition"],
        "unrecognized_manifest_version"
    );
    assert_eq!(
        report["profile_manifest"]["metadata"]["saved_bundle_selections"],
        serde_json::json!(["fixture-bundle"])
    );
    assert!(!fixture.marker.exists(), "a known tool was invoked");
    assert_eq!(
        snapshot(&fixture.root),
        before,
        "content, entries and mtimes must be unchanged"
    );
}

#[test]
fn missing_profile_does_not_spawn_known_tools_create_profile_or_change_mtimes() {
    let fixture = Fixture::new();
    let missing = fixture.root.join("missing-profile");
    let before = snapshot(&fixture.root);
    let output = fixture.inspect(&missing, None);
    let report = successful_report(&output);
    assert_eq!(report["profile_manifest"]["status"], "missing");
    assert!(report["runtime_manifest"].is_null());
    assert_eq!(report["version_recognition"], "unknown");
    assert!(!missing.exists());
    assert!(!fixture.marker.exists(), "a known tool was invoked");
    assert_eq!(
        snapshot(&fixture.root),
        before,
        "content, entries and mtimes must be unchanged"
    );
}
