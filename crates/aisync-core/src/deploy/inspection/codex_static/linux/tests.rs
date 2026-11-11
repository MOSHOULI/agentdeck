use super::*;
use std::os::unix::fs::{PermissionsExt, symlink};
struct Fixture {
    root: PathBuf,
    prefix: PathBuf,
    node: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("static-linux 中文-{}", uuid::Uuid::new_v4()));
        let prefix = root.join("prefix");
        let package = prefix.join("lib/node_modules/@openai/codex");
        fs::create_dir_all(package.join("bin")).unwrap();
        fs::create_dir_all(prefix.join("bin")).unwrap();
        fs::write(
            package.join("package.json"),
            br#"{"name":"@openai/codex","version":"SECRET","bin":{"codex":"bin/codex.js"}}"#,
        )
        .unwrap();
        fs::write(
            package.join("bin/codex.js"),
            b"#!/usr/bin/env node\nthrow Error('MUST_NOT_RUN');\n",
        )
        .unwrap();
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
        let node = root.join("node");
        let mut bytes = [0; 64];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16] = 3;
        bytes[18] = 62;
        bytes[20] = 1;
        fs::write(&node, bytes).unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(0o755)).unwrap();
        Self { root, prefix, node }
    }
    fn package(&self) -> PathBuf {
        self.prefix.join("lib/node_modules/@openai/codex")
    }
    fn inspect(&self) -> Result<CodexStaticObservation> {
        inspect_codex_linux_static(&self.prefix, &self.node)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn linux_static_contract_has_no_runtime_version_or_authority() {
    let f = Fixture::new();
    let value = serde_json::to_value(f.inspect().unwrap()).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"schemaVersion":1,"agentId":"codex","observation":"selected_layout_matched",
        "runtimeStatus":"runtime_not_run","searchScope":"selected_prefix_only","selectedLauncher":"bin/codex",
        "applySupported":false,"upgradeSupported":false,"uninstallSupported":false})
    );
}
#[test]
fn linux_manifest_malformed_duplicate_and_unsupported_bins_fail_closed() {
    for contents in [
        "{",
        r#"{"name":"@openai/codex","name":"@openai/codex","bin":{"codex":"bin/codex.js"}}"#,
        r#"{"name":"@evil/codex","bin":{"codex":"bin/codex.js"}}"#,
        r#"{"name":"@openai/codex","bin":{"codex":"../../outside"}}"#,
        r#"{"name":"@openai/codex","bin":"bin/codex.js"}"#,
    ] {
        let f = Fixture::new();
        fs::write(f.package().join("package.json"), contents).unwrap();
        assert!(f.inspect().is_err());
    }
}
#[test]
fn linux_rejects_wrong_links_files_and_redirected_ancestors() {
    let f = Fixture::new();
    let launcher = f.prefix.join("bin/codex");
    fs::remove_file(&launcher).unwrap();
    symlink(f.package().join("bin/codex.js"), &launcher).unwrap();
    assert!(f.inspect().is_err());
    fs::remove_file(&launcher).unwrap();
    fs::write(&launcher, b"MUST_NOT_RUN").unwrap();
    assert!(f.inspect().is_err());
    let f = Fixture::new();
    let alias = f.root.join("alias");
    symlink(&f.prefix, &alias).unwrap();
    assert!(inspect_codex_linux_static(&alias, &f.node).is_err());
    let f = Fixture::new();
    let manifest = f.package().join("package.json");
    fs::hard_link(&manifest, f.root.join("hardlink")).unwrap();
    assert!(f.inspect().is_err());
    let f = Fixture::new();
    let bin = f.package().join("bin/codex.js");
    let outside = f.root.join("outside.js");
    fs::rename(&bin, &outside).unwrap();
    symlink(outside, bin).unwrap();
    assert!(f.inspect().is_err());
}
#[test]
fn linux_native_node_shebang_permissions_and_relative_paths_fail_closed() {
    let f = Fixture::new();
    assert!(inspect_codex_linux_static(Path::new("relative"), &f.node).is_err());
    fs::write(&f.node, b"#!/bin/sh\ntouch SHOULD_NOT_RUN").unwrap();
    assert!(f.inspect().is_err());
    let f = Fixture::new();
    fs::set_permissions(&f.node, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(f.inspect().is_err());
    let f = Fixture::new();
    let mut bytes = fs::read(&f.node).unwrap();
    bytes[18] = 183;
    fs::write(&f.node, bytes).unwrap();
    assert!(f.inspect().is_err());
    let f = Fixture::new();
    fs::write(f.package().join("bin/codex.js"), b"#!/usr/bin/env evil\n").unwrap();
    assert!(f.inspect().is_err());
}
