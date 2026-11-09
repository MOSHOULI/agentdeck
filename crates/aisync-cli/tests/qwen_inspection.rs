//! Explicit-file CLI boundary; synthetic fixtures, no real Qwen installation.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "qwen-inspection-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("home")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"name":"@qwen-code/qwen-code","version":"0.27.0"}"#,
        )
        .unwrap();
        fs::write(root.join("settings.json"), r#"{"$version":4,"model":{"name":"SECRET_MODEL"},"security":{"auth":{"apiKey":"SECRET_KEY"}},"mcpServers":{"PRIVATE_SERVER":{"env":{"TOKEN":"SECRET_TOKEN"}}},"PRIVATE_TOP_LEVEL":"SECRET_VALUE"}"#).unwrap();
        fs::write(root.join("home/.env"), "SECRET_ENV=do-not-read").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for name in ["qwen", "node", "npm", "git"] {
                let path = root.join("bin").join(name);
                fs::write(
                    &path,
                    "#!/bin/sh\necho unexpected > \"$QWEN_SENTINEL\"\nexit 97\n",
                )
                .unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        Self { root }
    }
    fn command(&self) -> Command {
        self.action_command("inspect")
    }
    fn action_command(&self, action: &str) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_aisync"));
        cmd.env_clear()
            .current_dir(&self.root)
            .env("HOME", self.root.join("home"))
            .env("USERPROFILE", self.root.join("home"))
            .env("APPDATA", self.root.join("home/appdata"))
            .env("LOCALAPPDATA", self.root.join("home/local"))
            .env("XDG_CONFIG_HOME", self.root.join("home/config"))
            .env("XDG_DATA_HOME", self.root.join("home/data"))
            .env("PATH", self.root.join("bin"))
            .env("QWEN_SENTINEL", self.root.join("executed"));
        #[cfg(windows)]
        for key in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
            if let Some(value) = std::env::var_os(key) {
                cmd.env(key, value);
            }
        }
        cmd.args(["qwen", action, "--settings"])
            .arg(self.root.join("settings.json"));
        cmd
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for e in fs::read_dir(path).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), vec![]);
                walk(root, &path, out);
            } else {
                out.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
#[test]
#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn explicit_inspection_is_redacted_and_has_no_content_writes() {
    let f = Fixture::new();
    let before = snapshot(&f.root);
    let output = f
        .command()
        .arg("--runtime-manifest")
        .arg(f.root.join("package.json"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    let text = String::from_utf8(output.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["version_recognition"], "reviewed_manifest_version");
    assert_eq!(report["settings"]["status"], "read");
    assert_eq!(report["settings"]["sections"]["security"], true);
    for forbidden in ["SECRET", "PRIVATE", "do-not-read", f.root.to_str().unwrap()] {
        assert!(!text.contains(forbidden));
    }
    assert!(output.stderr.is_empty());
    assert_eq!(before, snapshot(&f.root));
}
#[test]
fn unsupported_manifest_and_absent_manifest_produce_reports_without_changes() {
    let f = Fixture::new();
    fs::write(
        f.root.join("package.json"),
        r#"{"name":"OTHER_SECRET_PACKAGE","version":"999.0.0"}"#,
    )
    .unwrap();
    fs::write(f.root.join("settings.json"), "INVALID_SECRET_CONTENT").unwrap();
    let before = snapshot(&f.root);
    for use_manifest in [false, true] {
        let mut cmd = f.command();
        if use_manifest {
            cmd.arg("--runtime-manifest")
                .arg(f.root.join("package.json"));
        }
        let output = cmd.output().unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let _: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(!text.contains("SECRET"));
    }
    assert_eq!(before, snapshot(&f.root));
}
#[test]
fn relative_selection_is_rejected_before_inspection() {
    let f = Fixture::new();
    let before = snapshot(&f.root);
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["qwen", "inspect", "--settings", "settings.json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(before, snapshot(&f.root));
}

#[test]
fn fixture_roots_are_distinct_under_parallel_creation() {
    let fixtures: Vec<_> = (0..16)
        .map(|_| std::thread::spawn(Fixture::new))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    let roots: std::collections::BTreeSet<_> = fixtures.iter().map(|f| f.root.clone()).collect();
    assert_eq!(roots.len(), fixtures.len());
}

#[test]
fn recovery_cli_no_receipt_is_read_only_and_redacted() {
    let f = Fixture::new();
    let before = snapshot(&f.root);
    let output = f
        .action_command("vim-recovery")
        .arg("--runtime-manifest")
        .arg(f.root.join("package.json"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    let text = String::from_utf8(output.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["state"], "no_receipt");
    assert_eq!(report["changes_settings"], false);
    for forbidden in ["SECRET", "PRIVATE", f.root.to_str().unwrap()] {
        assert!(!text.contains(forbidden));
    }
    assert!(output.stderr.is_empty());
    assert_eq!(before, snapshot(&f.root));
}

#[test]
fn recovery_cli_invalid_receipt_does_not_write_or_expose_content() {
    let f = Fixture::new();
    fs::write(
        f.root.join(".agentdeck-qwen-vim.json"),
        r#"{"SECRET_RECEIPT":true}"#,
    )
    .unwrap();
    let before = snapshot(&f.root);
    let output = f
        .action_command("vim-recovery")
        .arg("--runtime-manifest")
        .arg(f.root.join("package.json"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("SECRET"));
    assert_eq!(before, snapshot(&f.root));
}

#[test]
fn recovery_cli_observes_applied_and_conflict_without_mutation() {
    let f = Fixture::new();
    let settings = f.root.join("settings.json");
    let manifest = f.root.join("package.json");
    let preview = aisync_core::qwen::owned::preview(&settings, &manifest, true).unwrap();
    aisync_core::qwen::owned::apply(&settings, &manifest, true, &preview.token).unwrap();
    for conflict in [false, true] {
        if conflict {
            fs::write(
                &settings,
                r#"{"$version":4,"general":{"vimMode":false},"extra":"SECRET"}"#,
            )
            .unwrap();
        }
        let before = snapshot(&f.root);
        let output = f
            .action_command("vim-recovery")
            .arg("--runtime-manifest")
            .arg(&manifest)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output.stderr);
        let text = String::from_utf8(output.stdout).unwrap();
        let report: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            report["state"],
            if conflict {
                "conflict"
            } else {
                "restore_field"
            }
        );
        assert_eq!(report["changes_settings"], !conflict);
        assert!(!text.contains("SECRET"));
        assert!(output.stderr.is_empty());
        assert_eq!(before, snapshot(&f.root));
    }
}
