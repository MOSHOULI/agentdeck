use super::*;
use std::fs;
struct Fixture {
    root: PathBuf,
    prefix: PathBuf,
    node: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("codex-static-{}", uuid::Uuid::new_v4()));
        let prefix = root.join("prefix");
        let package = prefix.join("node_modules/@openai/codex");
        fs::create_dir_all(package.join("bin")).unwrap();
        fs::create_dir(root.join("tools")).unwrap();
        fs::write(package.join("package.json"),br#"{"name":"@openai/codex","version":"SECRET_MANIFEST_VALUE","bin":{"codex":"bin/codex.js"}}"#).unwrap();
        fs::write(
            package.join("bin/codex.js"),
            b"#!/usr/bin/env node\nthrow Error('MUST_NOT_RUN');\n",
        )
        .unwrap();
        fs::write(
            prefix.join("codex.cmd"),
            node_cmd_shim("node_modules/@openai/codex/bin/codex.js"),
        )
        .unwrap();
        fs::write(prefix.join("codex"), b"MUST_NOT_RUN").unwrap();
        fs::write(prefix.join("codex.ps1"), b"throw 'MUST_NOT_RUN'").unwrap();
        let node = root.join("tools/node.exe");
        let mut bytes = vec![0u8; 70];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..70].copy_from_slice(b"PE\0\0\x64\x86");
        fs::write(&node, bytes).unwrap();
        Self { root, prefix, node }
    }
    fn manifest(&self) -> PathBuf {
        self.prefix.join("node_modules/@openai/codex/package.json")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn shared_parser_returns_no_runtime_or_management_authority() {
    let f = Fixture::new();
    let result = inspect(&f.prefix, &f.node).unwrap();
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("runtime_not_run"));
    assert!(!json.contains("SECRET_MANIFEST_VALUE"));
    assert!(!json.contains("Installed"));
    assert!(!result.apply_supported);
    assert!(!result.upgrade_supported);
    assert!(!result.uninstall_supported);
    assert_eq!(fs::read_dir(&f.root).unwrap().count(), 2);
}
#[test]
fn alternate_active_candidate_and_changed_shim_are_rejected() {
    let f = Fixture::new();
    fs::write(f.prefix.join("codex.exe"), b"MZ").unwrap();
    assert!(inspect(&f.prefix, &f.node).is_err());
    fs::remove_file(f.prefix.join("codex.exe")).unwrap();
    fs::write(f.prefix.join("codex.cmd"), b"@echo malicious").unwrap();
    assert!(inspect(&f.prefix, &f.node).is_err());
}
#[test]
fn manifest_name_and_escaping_entrypoints_fail_closed() {
    for manifest in [
        r#"{"name":"@other/codex","bin":{"codex":"bin/codex.js"}}"#,
        r#"{"name":"@openai/codex","bin":{"codex":"../../outside.js"}}"#,
    ] {
        let f = Fixture::new();
        fs::write(f.manifest(), manifest).unwrap();
        assert!(inspect(&f.prefix, &f.node).is_err());
    }
}
#[test]
fn native_node_header_and_adjacent_shadow_are_checked() {
    let f = Fixture::new();
    fs::write(f.prefix.join("node.exe"), fs::read(&f.node).unwrap()).unwrap();
    assert!(inspect(&f.prefix, &f.node).is_err());
    fs::remove_file(f.prefix.join("node.exe")).unwrap();
    fs::write(&f.node, b"MZ not a PE").unwrap();
    assert!(inspect(&f.prefix, &f.node).is_err());
}
#[test]
fn package_hardlink_is_rejected() {
    let f = Fixture::new();
    fs::hard_link(f.manifest(), f.root.join("alias")).unwrap();
    assert!(inspect(&f.prefix, &f.node).is_err());
}
#[cfg(not(windows))]
#[test]
fn public_entry_is_explicitly_windows_only() {
    let f = Fixture::new();
    assert!(inspect_codex_windows_static(&f.prefix, &f.node).is_err());
}

#[test]
fn observation_preserves_all_fixture_bytes_and_directory_members() {
    fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut pending = vec![root.to_path_buf()];
        let mut result = Vec::new();
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(dir).unwrap() {
                let p = entry.unwrap().path();
                if p.is_dir() {
                    result.push((p.clone(), Vec::new()));
                    pending.push(p);
                } else {
                    result.push((p.clone(), fs::read(p).unwrap()));
                }
            }
        }
        result.sort();
        result
    }
    let f = Fixture::new();
    let before = snapshot(&f.root);
    inspect(&f.prefix, &f.node).unwrap();
    assert_eq!(before, snapshot(&f.root));
}
#[test]
fn missing_companion_wrong_shebang_and_relative_inputs_are_rejected() {
    let f = Fixture::new();
    assert!(inspect(Path::new("prefix"), &f.node).is_err());
    fs::remove_file(f.prefix.join("codex.ps1")).unwrap();
    assert!(inspect(&f.prefix, &f.node).is_err());
    fs::write(f.prefix.join("codex.ps1"), b"never run").unwrap();
    fs::write(
        f.prefix.join("node_modules/@openai/codex/bin/codex.js"),
        b"#!/usr/bin/env evil\n",
    )
    .unwrap();
    assert!(inspect(&f.prefix, &f.node).is_err());
}
#[cfg(unix)]
#[test]
fn linked_prefix_and_escaping_entrypoint_are_rejected() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let alias = f.root.join("alias");
    symlink(&f.prefix, &alias).unwrap();
    assert!(inspect(&alias, &f.node).is_err());
    let entry = f.prefix.join("node_modules/@openai/codex/bin/codex.js");
    fs::remove_file(&entry).unwrap();
    let outside = f.root.join("outside.js");
    fs::write(&outside, b"#!/usr/bin/env node\n").unwrap();
    symlink(outside, entry).unwrap();
    assert!(inspect(&f.prefix, &f.node).is_err());
}
#[cfg(windows)]
#[test]
fn canonical_verbatim_drive_is_supported_but_device_namespace_is_not() {
    let f = Fixture::new();
    inspect(
        &fs::canonicalize(&f.prefix).unwrap(),
        &fs::canonicalize(&f.node).unwrap(),
    )
    .unwrap();
    assert!(reader_path(Path::new(r"\\?\UNC\server\share\node.exe")).is_err());
}
