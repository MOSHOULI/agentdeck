use super::reader::SelectedFileReader;
use super::*;
#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
use std::fs;
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const MANIFEST: &[u8] = br#"{"name":"@qwen-code/qwen-code","version":"0.27.0"}"#;
const SETTINGS: &[u8] = br#"{"$version":4,"general":{},"modelProviders":{"secret-key":"private-value"},"mcpServers":{"private-server":{"apiKey":"secret-token"}}}"#;
#[derive(Default)]
struct AllowlistReader {
    files: BTreeMap<PathBuf, Vec<u8>>,
    reads: RefCell<Vec<PathBuf>>,
}
impl SelectedFileReader for AllowlistReader {
    fn read_selected(&self, path: &Path) -> Result<Vec<u8>, InspectionIssue> {
        self.reads.borrow_mut().push(path.to_owned());
        Ok(self
            .files
            .get(path)
            .expect("read outside explicit allowlist")
            .clone())
    }
}
fn root() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(r"C:\synthetic-qwen")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/synthetic-qwen")
    }
}
fn fixture(manifest: &[u8], settings: &[u8]) -> (AllowlistReader, PathBuf, PathBuf) {
    let m = root().join("runtime/package.json");
    let s = root().join("profile/settings.json");
    let mut r = AllowlistReader::default();
    r.files.insert(m.clone(), manifest.to_vec());
    r.files.insert(s.clone(), settings.to_vec());
    (r, m, s)
}
#[test]
fn bounded_allowlist_summary_never_exposes_values_keys_or_paths() {
    let (r, m, s) = fixture(MANIFEST, SETTINGS);
    let report = inspect_with(&r, &s, Some(&m));
    assert_eq!(*r.reads.borrow(), vec![m, s]);
    assert_eq!(
        report.version_recognition,
        VersionRecognition::ReviewedManifestVersion
    );
    assert_eq!(report.settings.status, ObservationStatus::Read);
    let sections = report.settings.sections.as_ref().unwrap();
    assert!(sections.general && sections.model_providers && sections.mcp_servers);
    assert!(!sections.ui && !sections.security && !sections.experimental);
    let output = serde_json::to_string(&report).unwrap();
    for forbidden in [
        "secret-key",
        "private-value",
        "private-server",
        "secret-token",
        "synthetic-qwen",
    ] {
        assert!(!output.contains(forbidden));
    }
    assert_eq!(
        report.mutation_capabilities,
        MutationCapabilities::default()
    );
    assert_eq!(report.authentication, UnknownState::Unknown);
    assert_eq!(report.effective_configuration, UnknownState::Unknown);
}
#[test]
fn no_manifest_means_no_read_even_of_settings() {
    let r = AllowlistReader::default();
    let report = inspect_with(&r, &root().join("settings.json"), None);
    assert!(r.reads.borrow().is_empty());
    assert_eq!(
        report.settings.issue,
        Some(InspectionIssue::ManifestRequired)
    );
}
#[test]
fn unknown_package_version_and_release_qualifiers_never_read_settings() {
    for (name, version) in [
        ("other", "0.27.0"),
        ("@qwen-code/qwen-code", "0.27.1"),
        ("@qwen-code/qwen-code", "0.27.0-beta"),
        ("@qwen-code/qwen-code", "0.27.0+build"),
        ("@qwen-code/qwen-code", "00.27.0"),
    ] {
        let m = root().join("package.json");
        let mut r = AllowlistReader::default();
        r.files.insert(
            m.clone(),
            serde_json::to_vec(&serde_json::json!({"name":name,"version":version})).unwrap(),
        );
        let report = inspect_with(&r, &root().join("settings.json"), Some(&m));
        assert_eq!(*r.reads.borrow(), vec![m]);
        assert_eq!(
            report.version_recognition,
            VersionRecognition::UnrecognizedManifestVersion
        );
        assert_eq!(report.settings.status, ObservationStatus::NotRead);
        assert!(report.settings.sections.is_none());
    }
}
#[test]
fn invalid_selection_blocks_all_io() {
    for (s, m) in [
        (PathBuf::from("settings.json"), root().join("package.json")),
        (root().join("auth.json"), root().join("package.json")),
        (root().join("settings.json"), root().join("secret.json")),
        (
            root().join("a/../settings.json"),
            root().join("package.json"),
        ),
    ] {
        let r = AllowlistReader::default();
        let report = inspect_with(&r, &s, Some(&m));
        assert!(r.reads.borrow().is_empty());
        assert!(report.settings.sections.is_none());
    }
}
#[test]
fn unsupported_schema_and_shape_never_return_partial_summary() {
    for bytes in [
        br#"{"general":{},"$version":3}"#.as_slice(),
        br#"{"general":{}}"#,
        br#"{"$version":"4"}"#,
        br#"{"$version":4.0}"#,
        br#"[]"#,
    ] {
        let (r, m, s) = fixture(MANIFEST, bytes);
        let report = inspect_with(&r, &s, Some(&m));
        assert_eq!(report.settings.status, ObservationStatus::Unsupported);
        assert!(report.settings.sections.is_none());
    }
}
#[test]
fn unsupported_syntax_is_not_reported_as_qwen_corruption() {
    for bytes in [
        b"{ private-secret".as_slice(),
        b"\xef\xbb\xbf{\"$version\":4}",
        b"{ /* comment */ \"$version\":4}",
    ] {
        let (r, m, s) = fixture(MANIFEST, bytes);
        let report = inspect_with(&r, &s, Some(&m));
        assert_eq!(
            report.settings.issue,
            Some(InspectionIssue::UnsupportedJson)
        );
        assert!(
            !serde_json::to_string(&report)
                .unwrap()
                .contains("private-secret")
        );
    }
}
#[test]
fn duplicate_keys_and_oversize_and_depth_are_rejected() {
    for (bytes, issue) in [
        (
            br#"{"$version":4,"$version":4}"#.to_vec(),
            InspectionIssue::DuplicateJsonKey,
        ),
        (
            vec![b' '; LIMITS.max_manifest_bytes + 1],
            InspectionIssue::FileTooLarge,
        ),
        (
            format!("{}0{}", "[".repeat(20), "]".repeat(20)).into_bytes(),
            InspectionIssue::JsonLimit,
        ),
    ] {
        let (r, m, s) = fixture(MANIFEST, &bytes);
        let report = inspect_with(&r, &s, Some(&m));
        assert_eq!(report.settings.issue, Some(issue));
        assert!(report.settings.sections.is_none());
    }
}
#[test]
fn malformed_runtime_never_attempts_settings() {
    let m = root().join("package.json");
    let mut r = AllowlistReader::default();
    r.files.insert(m.clone(), b"{secret".to_vec());
    let report = inspect_with(&r, &root().join("settings.json"), Some(&m));
    assert_eq!(*r.reads.borrow(), vec![m]);
    assert_eq!(
        report.runtime_manifest.issue,
        Some(InspectionIssue::UnsupportedJson)
    );
}

#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
struct TempRoot(PathBuf);
#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
impl TempRoot {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("qwen-inspect-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        fs::write(&p, bytes).unwrap();
        p
    }
}
#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
#[test]
fn native_reader_leaves_synthetic_files_and_directory_unchanged() {
    let t = TempRoot::new();
    let m = t.write("package.json", MANIFEST);
    let s = t.write("settings.json", SETTINGS);
    let auth = t.write("oauth_creds.json", b"never read or copy");
    let before = fs::metadata(&s).unwrap().modified().unwrap();
    let report = inspect_settings(&s, Some(&m));
    assert_eq!(report.settings.status, ObservationStatus::Read);
    assert_eq!(fs::read(&s).unwrap(), SETTINGS);
    assert_eq!(fs::metadata(&s).unwrap().modified().unwrap(), before);
    assert_eq!(fs::read(&auth).unwrap(), b"never read or copy");
    assert_eq!(fs::read_dir(&t.0).unwrap().count(), 3);
}
#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
#[test]
fn native_reader_rejects_directory_missing_and_oversize() {
    let t = TempRoot::new();
    let m = t.write("package.json", MANIFEST);
    let s = t.0.join("settings.json");
    assert_eq!(
        inspect_settings(&s, Some(&m)).settings.issue,
        Some(InspectionIssue::Missing)
    );
    fs::create_dir(&s).unwrap();
    assert_eq!(
        inspect_settings(&s, Some(&m)).settings.issue,
        Some(InspectionIssue::NotRegularFile)
    );
    fs::remove_dir(&s).unwrap();
    fs::write(&s, vec![b' '; LIMITS.max_manifest_bytes + 1]).unwrap();
    assert_eq!(
        inspect_settings(&s, Some(&m)).settings.issue,
        Some(InspectionIssue::FileTooLarge)
    );
}
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn native_reader_rejects_leaf_ancestor_and_hardlinks() {
    use std::os::unix::fs::symlink;
    let t = TempRoot::new();
    let m = t.write("package.json", MANIFEST);
    let actual = t.write("actual.json", SETTINGS);
    let s = t.0.join("settings.json");
    symlink(&actual, &s).unwrap();
    assert_eq!(
        inspect_settings(&s, Some(&m)).settings.issue,
        Some(InspectionIssue::LinkOrReparsePoint)
    );
    fs::remove_file(&s).unwrap();
    fs::hard_link(&actual, &s).unwrap();
    assert_eq!(
        inspect_settings(&s, Some(&m)).settings.issue,
        Some(InspectionIssue::MultipleHardLinks)
    );
    fs::remove_file(&s).unwrap();
    let dir = t.0.join("real");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("settings.json"), SETTINGS).unwrap();
    let link = t.0.join("linked");
    symlink(dir, &link).unwrap();
    assert_eq!(
        inspect_settings(&link.join("settings.json"), Some(&m))
            .settings
            .issue,
        Some(InspectionIssue::LinkOrReparsePoint)
    );
}

#[test]
fn oversized_strings_and_entry_budget_never_return_sections() {
    let long = serde_json::to_vec(&serde_json::json!({"$version": 4, "private": "x".repeat(LIMITS.max_json_string_bytes + 1)})).unwrap();
    let entries = serde_json::to_vec(
        &serde_json::json!({"$version": 4, "private": vec![0; LIMITS.max_json_entries]}),
    )
    .unwrap();
    for bytes in [long, entries] {
        let (r, m, s) = fixture(MANIFEST, &bytes);
        let report = inspect_with(&r, &s, Some(&m));
        assert_eq!(report.settings.issue, Some(InspectionIssue::JsonLimit));
        assert!(report.settings.sections.is_none());
    }
}

#[cfg(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
))]
#[test]
fn native_missing_runtime_does_not_read_or_create_settings() {
    let t = TempRoot::new();
    let s = t.0.join("settings.json");
    let m = t.0.join("package.json");
    let report = inspect_settings(&s, Some(&m));
    assert_eq!(
        report.runtime_manifest.issue,
        Some(InspectionIssue::Missing)
    );
    assert_eq!(report.settings.status, ObservationStatus::NotRead);
    assert_eq!(fs::read_dir(&t.0).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
fn windows_reserved_namespace_and_stream_paths_fail_before_io() {
    for path in [
        r"\\server\share\settings.json",
        r"\\?\C:\profile\settings.json",
        r"C:\NUL\settings.json",
        r"C:\profile\settings.json:stream",
        r"C:\profile.\settings.json",
    ] {
        assert!(validate_settings_path(Path::new(path)).is_err());
    }
}

#[cfg(windows)]
#[test]
fn native_windows_reader_rejects_hardlinked_settings() {
    let t = TempRoot::new();
    let m = t.write("package.json", MANIFEST);
    let actual = t.write("actual.json", SETTINGS);
    let s = t.0.join("settings.json");
    fs::hard_link(actual, &s).unwrap();
    assert_eq!(
        inspect_settings(&s, Some(&m)).settings.issue,
        Some(InspectionIssue::MultipleHardLinks)
    );
}

#[cfg(not(any(
    windows,
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    )
)))]
#[test]
fn native_unsupported_platform_fails_closed_without_reading() {
    let r = inspect_settings(
        &root().join("settings.json"),
        Some(&root().join("package.json")),
    );
    assert_eq!(
        r.runtime_manifest.issue,
        Some(InspectionIssue::PlatformUnsupported)
    );
    assert_eq!(r.settings.status, ObservationStatus::NotRead);
    assert!(r.settings.sections.is_none());
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn native_reader_rejects_fifo_without_opening_or_blocking() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let t = TempRoot::new();
    let m = t.write("package.json", MANIFEST);
    let s = t.0.join("settings.json");
    unsafe extern "C" {
        fn mkfifo(path: *const std::ffi::c_char, mode: u32) -> i32;
    }
    let c_path = CString::new(s.as_os_str().as_bytes()).unwrap();
    // SAFETY: NUL-terminated test-owned path, valid mode, no retained pointer.
    assert_eq!(unsafe { mkfifo(c_path.as_ptr(), 0o600) }, 0);
    assert_eq!(
        inspect_settings(&s, Some(&m)).settings.issue,
        Some(InspectionIssue::NotRegularFile)
    );
}
