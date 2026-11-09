//! Explicit-file CLI boundary; synthetic fixtures, no real Qoder installation.
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
            "qoder-inspection-{}-{}-{}",
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
            r#"{"name":"@qoder-code/qoder-code","version":"0.27.0"}"#,
        )
        .unwrap();
        fs::write(root.join("settings.json"), r#"{"$version":4,"model":{"name":"SECRET_MODEL"},"security":{"auth":{"apiKey":"SECRET_KEY"}},"mcpServers":{"PRIVATE_SERVER":{"env":{"TOKEN":"SECRET_TOKEN"}}},"PRIVATE_TOP_LEVEL":"SECRET_VALUE"}"#).unwrap();
        fs::write(root.join("home/.env"), "SECRET_ENV=do-not-read").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for name in ["qoder", "node", "npm", "git"] {
                let path = root.join("bin").join(name);
                fs::write(
                    &path,
                    "#!/bin/sh\necho unexpected > \"$QODER_SENTINEL\"\nexit 97\n",
                )
                .unwrap();
                fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        Self { root }
    }
    fn command(&self) -> Command {
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
            .env("QODER_CONFIG_DIR", self.root.join("unselected"))
            .env("QODER_SENTINEL", self.root.join("executed"));
        #[cfg(windows)]
        for key in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
            if let Some(value) = std::env::var_os(key) {
                cmd.env(key, value);
            }
        }
        cmd.args(["qoder", "inspect", "--user-settings"])
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
fn explicit_scopes_are_redacted_without_processes_or_writes() {
    let f = Fixture::new();
    fs::write(
        f.root.join(".mcp.json"),
        r#"{//comment
        "mcpServers":{"PRIVATE_SERVER":{"url":"https://SECRET"},"OTHER_PRIVATE":{}}}"#,
    )
    .unwrap();
    let before = snapshot(&f.root);
    let output = f
        .command()
        .arg("--project-mcp")
        .arg(f.root.join(".mcp.json"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    let report: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["scopes"][0]["status"], "read");
    assert_eq!(report["scopes"][3]["status"], "read");
    assert_eq!(report["repeated_mcp_identifier_count"], 1);
    for forbidden in ["SECRET", "PRIVATE", "do-not-read", f.root.to_str().unwrap()] {
        assert!(!text.contains(forbidden));
    }
    assert_eq!(snapshot(&f.root), before);
}
#[test]
#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn invalid_sibling_missing_and_unsupported_files_do_not_initialize() {
    let f = Fixture::new();
    let before = snapshot(&f.root);
    let output = f
        .command()
        .arg("--project-mcp")
        .arg(f.root.join("auth.json"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains(f.root.to_str().unwrap()));
    let output = f
        .command()
        .arg("--local-settings")
        .arg(f.root.join("settings.local.json"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let r: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(r["scopes"][2]["status"], "missing");
    assert!(r["repeated_mcp_identifier_count"].is_null());
    assert_eq!(snapshot(&f.root), before);
}
#[test]
fn no_selection_and_repeated_path_fail_without_discovery() {
    let f = Fixture::new();
    let before = snapshot(&f.root);
    let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["qoder", "inspect"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let output = f
        .command()
        .arg("--project-settings")
        .arg(f.root.join("settings.json"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(snapshot(&f.root), before);
}
#[test]
#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
fn directory_and_hardlink_are_rejected() {
    let f = Fixture::new();
    fs::create_dir(f.root.join("settings.local.json")).unwrap();
    fs::hard_link(f.root.join("settings.json"), f.root.join(".mcp.json")).unwrap();
    let output = f
        .command()
        .arg("--local-settings")
        .arg(f.root.join("settings.local.json"))
        .arg("--project-mcp")
        .arg(f.root.join(".mcp.json"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let r: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(r["scopes"][2]["issue"], "not_regular_file");
    assert_eq!(r["scopes"][3]["issue"], "multiple_hard_links");
}
#[test]
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn symlink_and_fifo_are_rejected_without_following_or_blocking() {
    let f = Fixture::new();
    std::os::unix::fs::symlink(
        f.root.join("settings.json"),
        f.root.join("settings.local.json"),
    )
    .unwrap();
    assert!(
        Command::new("mkfifo")
            .arg(f.root.join(".mcp.json"))
            .status()
            .unwrap()
            .success()
    );
    let output = f
        .command()
        .arg("--local-settings")
        .arg(f.root.join("settings.local.json"))
        .arg("--project-mcp")
        .arg(f.root.join(".mcp.json"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let r: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(r["scopes"][2]["issue"], "link_or_reparse_point");
    assert_eq!(r["scopes"][3]["issue"], "not_regular_file");
}
