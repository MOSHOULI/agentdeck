use std::cell::RefCell;
use std::collections::BTreeMap;
#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    windows
))]
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    windows
))]
use std::sync::atomic::{AtomicUsize, Ordering};

use super::reader::ManifestReader;
use super::*;

const PROFILE: &[u8] = include_bytes!("../../../../tests/fixtures/dsh/profile/package.json");
const RUNTIME: &[u8] = include_bytes!("../../../../tests/fixtures/dsh/runtime/package.json");

/// Deny-by-default boundary: opening ANY unlisted file immediately fails the test.
/// There is intentionally no mutation or process API on this boundary.
#[derive(Default)]
struct AllowlistReader {
    files: BTreeMap<PathBuf, Result<Vec<u8>, InspectionIssue>>,
    reads: RefCell<Vec<PathBuf>>,
}

impl ManifestReader for AllowlistReader {
    fn read_manifest(&self, path: &Path) -> Result<Vec<u8>, InspectionIssue> {
        self.reads.borrow_mut().push(path.to_owned());
        self.files
            .get(path)
            .expect("forbidden read outside the explicit manifest allowlist")
            .clone()
    }
}

fn selected_root() -> PathBuf {
    #[cfg(windows)]
    {
        PathBuf::from(r"C:\synthetic-profile")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/synthetic-profile")
    }
}

fn allow_profile(bytes: &[u8]) -> (AllowlistReader, PathBuf) {
    let root = selected_root();
    let mut reader = AllowlistReader::default();
    reader
        .files
        .insert(root.join("package.json"), Ok(bytes.to_vec()));
    (reader, root)
}

fn report_for(bytes: &[u8]) -> DshInventoryReport {
    let (reader, root) = allow_profile(bytes);
    inspect_with(&reader, &root, None)
}

fn assert_profile_issue(bytes: &[u8], expected: InspectionIssue) {
    let report = report_for(bytes);
    assert_eq!(report.profile_manifest.issue, Some(expected));
    assert!(report.profile_manifest.metadata.is_none());
    assert_eq!(
        report.mutation_capabilities,
        MutationCapabilities::default()
    );
}

#[test]
fn explicit_allowlist_only_and_no_secret_or_executable_output() {
    let (mut reader, root) = allow_profile(PROFILE);
    let runtime = selected_root().join("explicit-runtime/package.json");
    reader.files.insert(runtime.clone(), Ok(RUNTIME.to_vec()));
    let report = inspect_with(&reader, &root, Some(&runtime));
    assert_eq!(
        *reader.reads.borrow(),
        vec![root.join("package.json"), runtime]
    );
    assert_eq!(report.profile_manifest.status, ManifestStatus::Read);
    assert_eq!(
        report.runtime_manifest.as_ref().unwrap().status,
        ManifestStatus::Read
    );
    assert_eq!(
        report.version_recognition,
        VersionRecognition::ReviewedManifestVersion
    );
    let metadata = report.profile_manifest.metadata.as_ref().unwrap();
    assert_eq!(
        metadata.saved_bundle_selections,
        vec!["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"]
    );
    assert_eq!(metadata.declared_dependencies.len(), 2);
    assert_eq!(
        metadata.declared_dependencies[1].specification_kind,
        SpecificationKind::OtherRedacted
    );
    let json = serde_json::to_string(&report).unwrap();
    for forbidden in [
        "SYNTHETIC_SECRET",
        "DO_NOT_EXECUTE",
        "example.invalid",
        "lib/bin.js",
        "synthetic-profile/",
    ] {
        assert!(!json.contains(forbidden), "unexpected output: {forbidden}");
    }
    assert_eq!(report.runtime_status, UnknownState::Unknown);
    assert_eq!(report.authentication, UnknownState::Unknown);
    assert_eq!(report.actual_enabled, UnknownState::Unknown);
    assert_eq!(
        report.mutation_capabilities,
        MutationCapabilities::default()
    );
}

#[test]
fn official_partial_profile_does_not_require_name_or_version() {
    for bytes in [
        b"{}".as_slice(),
        br#"{"private":true,"dependencies":{},"dsh":{"profile":{"bundles":[]}}}"#,
    ] {
        let report = report_for(bytes);
        assert_eq!(report.profile_manifest.status, ManifestStatus::Read);
        let metadata = report.profile_manifest.metadata.unwrap();
        assert_eq!(metadata.name, None);
        assert!(metadata.declared_dependencies.is_empty());
        assert!(metadata.saved_bundle_selections.is_empty());
        assert!(report.runtime_manifest.is_none());
        assert_eq!(report.version_recognition, VersionRecognition::Unknown);
    }
}

#[test]
fn profile_version_is_not_runtime_version_and_no_defaults_are_invented() {
    let report = report_for(br#"{"version":"0.2.2-alpha.3","dsh":{"profile":{}}}"#);
    assert_eq!(report.version_recognition, VersionRecognition::Unknown);
    assert!(
        report
            .profile_manifest
            .metadata
            .unwrap()
            .saved_bundle_selections
            .is_empty()
    );
}

#[test]
fn unknown_or_missing_runtime_version_never_enables_mutations() {
    for (name, version, expected) in [
        (
            "@deepseek-ai/dsh",
            Some("999.0.0"),
            VersionRecognition::UnrecognizedManifestVersion,
        ),
        (
            "unrelated",
            Some("0.2.2-alpha.3"),
            VersionRecognition::UnrecognizedManifestVersion,
        ),
        ("@deepseek-ai/dsh", None, VersionRecognition::Unknown),
        (
            "@deepseek-ai/dsh",
            Some("0.2.2-rc.1"),
            VersionRecognition::ReviewedManifestVersion,
        ),
        (
            "@deepseek-ai/dsh-app-boot",
            Some("0.2.2-rc.1"),
            VersionRecognition::ReviewedManifestVersion,
        ),
        (
            "unrelated",
            Some("0.2.2-rc.1"),
            VersionRecognition::UnrecognizedManifestVersion,
        ),
        (
            "@deepseek-ai/dsh",
            Some("0.2.2-rc.2"),
            VersionRecognition::UnrecognizedManifestVersion,
        ),
        (
            "@deepseek-ai/dsh-app-boot",
            Some("0.2.2-alpha.3"),
            VersionRecognition::ReviewedManifestVersion,
        ),
    ] {
        let (mut reader, root) = allow_profile(b"{}");
        let runtime = root.join("runtime/package.json");
        let mut value = serde_json::json!({"name": name});
        if let Some(version) = version {
            value["version"] = version.into();
        }
        reader
            .files
            .insert(runtime.clone(), Ok(serde_json::to_vec(&value).unwrap()));
        let report = inspect_with(&reader, &root, Some(&runtime));
        assert_eq!(report.version_recognition, expected);
        assert_eq!(
            report.mutation_capabilities,
            MutationCapabilities::default()
        );
        assert_eq!(report.runtime_status, UnknownState::Unknown);
    }
}

#[test]
fn malformed_relative_parent_and_non_manifest_selections_never_call_reader() {
    let reader = AllowlistReader::default();
    let root = selected_root();
    for profile in [PathBuf::from("relative"), root.join("../escape")] {
        let report = inspect_with(&reader, &profile, Some(&root.join("runtime/package.json")));
        assert_eq!(report.profile_manifest.status, ManifestStatus::Unsupported);
        assert_eq!(
            report.runtime_manifest.unwrap().issue,
            Some(InspectionIssue::OtherSelectionInvalid)
        );
    }
    for runtime in [
        PathBuf::from("relative/package.json"),
        root.join("../package.json"),
        root.join(".credentials.yaml"),
        root.join("cordis.yml"),
        root.join("package.json/child"),
        root.join("bad\0path/package.json"),
    ] {
        let report = inspect_with(&reader, &root, Some(&runtime));
        assert_eq!(
            report.profile_manifest.issue,
            Some(InspectionIssue::OtherSelectionInvalid)
        );
        assert_eq!(
            report.runtime_manifest.unwrap().status,
            ManifestStatus::Unsupported
        );
    }
    assert!(reader.reads.borrow().is_empty());
}

#[test]
fn path_byte_and_component_budgets_are_checked_before_any_read() {
    let reader = AllowlistReader::default();
    let root = selected_root();
    let too_long = root.join("a".repeat(LIMITS.max_path_bytes));
    assert_eq!(
        validate_profile_path(&too_long),
        Err(InspectionIssue::PathLimit)
    );
    let too_deep = (0..LIMITS.max_path_components).fold(root.clone(), |path, _| path.join("a"));
    assert_eq!(
        validate_profile_path(&too_deep),
        Err(InspectionIssue::PathLimit)
    );
    for invalid in [too_long, too_deep] {
        let report = inspect_with(&reader, &root, Some(&invalid.join("package.json")));
        assert_eq!(
            report.profile_manifest.issue,
            Some(InspectionIssue::OtherSelectionInvalid)
        );
    }
    assert!(reader.reads.borrow().is_empty());
}

#[test]
fn profile_appended_manifest_also_counts_against_path_budget() {
    let root = selected_root();
    let remaining = LIMITS.max_path_bytes - root.as_os_str().len() - 1;
    let root = root.join("a".repeat(remaining));
    assert_eq!(root.as_os_str().len(), LIMITS.max_path_bytes);
    assert_eq!(
        validate_profile_path(&root),
        Err(InspectionIssue::PathLimit)
    );
}

#[test]
fn missing_unreadable_corrupt_unsupported_are_distinct_and_sanitized() {
    for (issue, status) in [
        (InspectionIssue::Missing, ManifestStatus::Missing),
        (
            InspectionIssue::PermissionDenied,
            ManifestStatus::Unreadable,
        ),
        (InspectionIssue::IoFailure, ManifestStatus::Unreadable),
        (InspectionIssue::FileChanged, ManifestStatus::Unreadable),
        (InspectionIssue::NotRegularFile, ManifestStatus::Unsupported),
    ] {
        let (mut reader, root) = allow_profile(b"{}");
        reader.files.insert(root.join("package.json"), Err(issue));
        let report = inspect_with(&reader, &root, None);
        assert_eq!(report.profile_manifest.status, status);
        assert_eq!(report.profile_manifest.issue, Some(issue));
        assert!(report.profile_manifest.metadata.is_none());
    }
    assert_profile_issue(
        br#"{"secret":"SYNTHETIC_SECRET", bad}"#,
        InspectionIssue::InvalidJson,
    );
    assert_eq!(
        report_for(b"{").profile_manifest.status,
        ManifestStatus::Corrupt
    );
    assert_eq!(
        report_for(b"[]").profile_manifest.status,
        ManifestStatus::Unsupported
    );
}

#[test]
fn decoded_duplicate_keys_are_rejected_at_every_depth_even_ignored_fields() {
    for bytes in [
        br#"{"name":"first","name":"second"}"#.as_slice(),
        br#"{"name":"first","\u006eame":"second"}"#,
        br#"{"dependencies":{"plugin":"1.0.0","plugin":"2.0.0"}}"#,
        br#"{"ignored":{"deep":[{"key":1,"key":2}]}}"#,
    ] {
        assert_profile_issue(bytes, InspectionIssue::DuplicateJsonKey);
    }
}

#[test]
fn malformed_json_utf8_trailing_input_and_yaml_are_not_evaluated() {
    for bytes in [
        b"{ incomplete".as_slice(),
        b"{} {}",
        &[0xff, 0xfe],
        b"!!js/function function() {}",
        br#"{"n":1e9999}"#,
    ] {
        assert_profile_issue(bytes, InspectionIssue::InvalidJson);
    }
}

#[test]
fn structural_and_manifest_version_mismatches_reject_the_whole_inventory() {
    for bytes in [
        b"null".as_slice(),
        b"[]",
        br#"{"dependencies":[]}"#,
        br#"{"dependencies":{"plugin":null}}"#,
        br#"{"dsh":[]}"#,
        br#"{"dsh":{"profile":[]}}"#,
        br#"{"dsh":{"profile":{"bundles":{}}}}"#,
        br#"{"dsh":{"profile":{"bundles":[["plugin",{"secret":"hidden"}]]}}}"#,
        br#"{"name":42}"#,
    ] {
        assert_profile_issue(bytes, InspectionIssue::UnsupportedShape);
    }
    for value in [
        serde_json::json!(2),
        serde_json::json!("1"),
        serde_json::json!(null),
        serde_json::json!(1.0),
    ] {
        let bytes =
            serde_json::to_vec(&serde_json::json!({"dsh":{"manifestVersion":value}})).unwrap();
        assert_profile_issue(&bytes, InspectionIssue::UnsupportedManifestVersion);
    }
}

#[test]
fn unsafe_names_bundle_paths_and_control_sequences_never_escape() {
    for name in [
        "../escape",
        "file:/secret",
        "https://token@example.invalid",
        "bad\nname",
        "\u{1b}[31m",
        "@scope/../../secret",
        "UPPER",
        ".hidden",
        "",
        "秘密",
    ] {
        let bytes = serde_json::to_vec(&serde_json::json!({"name":name})).unwrap();
        assert_profile_issue(&bytes, InspectionIssue::UnsupportedMetadata);
        let bytes =
            serde_json::to_vec(&serde_json::json!({"dsh":{"profile":{"bundles":[name]}}})).unwrap();
        assert_profile_issue(&bytes, InspectionIssue::UnsupportedMetadata);
    }
    assert_profile_issue(
        &serde_json::to_vec(&serde_json::json!({"name":"x".repeat(LIMITS.max_name_bytes + 1)}))
            .unwrap(),
        InspectionIssue::UnsupportedMetadata,
    );
}

#[test]
fn dependency_specs_are_classified_without_echoing_or_resolving_them() {
    let (reader, root) = allow_profile(br#"{"dependencies":{"a":"file:../../.credentials.yaml","b":"https://u:TOKEN@host.invalid/repo","c":"git+ssh://host/SECRET","d":"workspace:*","e":"^1.2.3"}}"#);
    let report = inspect_with(&reader, &root, None);
    let deps = &report
        .profile_manifest
        .metadata
        .as_ref()
        .unwrap()
        .declared_dependencies;
    assert_eq!(deps.len(), 5);
    assert!(
        deps[..4]
            .iter()
            .all(|d| d.specification_kind == SpecificationKind::OtherRedacted)
    );
    assert_eq!(
        deps[4].specification_kind,
        SpecificationKind::RegistryVersionOrRange
    );
    assert!(
        deps.iter()
            .all(|d| d.installed == UnknownState::Unknown
                && d.actual_enabled == UnknownState::Unknown)
    );
    let json = serde_json::to_string(&report).unwrap();
    for secret in [
        "TOKEN",
        "SECRET",
        ".credentials.yaml",
        "host.invalid",
        "../../",
    ] {
        assert!(!json.contains(secret));
    }
    assert_eq!(reader.reads.borrow().len(), 1);
}

#[test]
fn all_supported_dependency_sections_are_bounded_together() {
    let mut value =
        serde_json::json!({"dependencies":{},"optionalDependencies":{},"peerDependencies":{}});
    for i in 0..LIMITS.max_dependencies {
        let section = ["dependencies", "optionalDependencies", "peerDependencies"][i % 3];
        value[section][format!("plugin-{i}")] = "1.0.0".into();
    }
    let report = report_for(&serde_json::to_vec(&value).unwrap());
    assert_eq!(
        report
            .profile_manifest
            .metadata
            .unwrap()
            .declared_dependencies
            .len(),
        LIMITS.max_dependencies
    );
    value["peerDependencies"]["overflow"] = "1.0.0".into();
    assert_profile_issue(
        &serde_json::to_vec(&value).unwrap(),
        InspectionIssue::DependencyLimit,
    );
}

#[test]
fn bundle_cardinality_is_bounded_without_truncation_or_reordering() {
    let mut bundles: Vec<String> = (0..LIMITS.max_saved_bundles)
        .map(|i| format!("bundle-{i}"))
        .collect();
    let bytes =
        serde_json::to_vec(&serde_json::json!({"dsh":{"profile":{"bundles":bundles}}})).unwrap();
    assert_eq!(
        report_for(&bytes)
            .profile_manifest
            .metadata
            .unwrap()
            .saved_bundle_selections,
        bundles
    );
    bundles.push("overflow".into());
    assert_profile_issue(
        &serde_json::to_vec(&serde_json::json!({"dsh":{"profile":{"bundles":bundles}}})).unwrap(),
        InspectionIssue::BundleLimit,
    );
}

#[test]
fn byte_depth_entry_and_string_limits_are_fail_closed() {
    assert_profile_issue(
        &vec![b' '; LIMITS.max_manifest_bytes + 1],
        InspectionIssue::FileTooLarge,
    );
    let nested = format!(
        "{}0{}",
        "[".repeat(LIMITS.max_json_depth + 1),
        "]".repeat(LIMITS.max_json_depth + 1)
    );
    assert_profile_issue(nested.as_bytes(), InspectionIssue::JsonLimit);
    let many = format!(
        "{{\"ignored\":[{}]}}",
        vec!["0"; LIMITS.max_json_entries + 1].join(",")
    );
    assert_profile_issue(many.as_bytes(), InspectionIssue::JsonLimit);
    let value = serde_json::json!({"ignored":"x".repeat(LIMITS.max_json_string_bytes + 1)});
    assert_profile_issue(
        &serde_json::to_vec(&value).unwrap(),
        InspectionIssue::JsonLimit,
    );
    let value = serde_json::json!({"x".repeat(LIMITS.max_json_string_bytes + 1):0});
    assert_profile_issue(
        &serde_json::to_vec(&value).unwrap(),
        InspectionIssue::JsonLimit,
    );
}

#[test]
fn valid_json_at_exact_byte_depth_entry_and_string_limits_is_accepted() {
    // Root object + value = 2 levels, so fourteen arrays make leaf depth 16.
    let arrays = LIMITS.max_json_depth - 2;
    let nested = format!(
        "{{\"ignored\":{}0{}}}",
        "[".repeat(arrays),
        "]".repeat(arrays)
    );
    assert_eq!(
        report_for(nested.as_bytes()).profile_manifest.status,
        ManifestStatus::Read
    );
    // Object value, key and array value consume three budget entries.
    let entries = format!(
        "{{\"ignored\":[{}]}}",
        vec!["0"; LIMITS.max_json_entries - 3].join(",")
    );
    assert_eq!(
        report_for(entries.as_bytes()).profile_manifest.status,
        ManifestStatus::Read
    );
    let value = serde_json::json!({"ignored":"x".repeat(LIMITS.max_json_string_bytes)});
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.resize(LIMITS.max_manifest_bytes, b' ');
    assert_eq!(
        report_for(&bytes).profile_manifest.status,
        ManifestStatus::Read
    );
}

#[test]
fn runtime_metadata_is_bounded_and_never_echoes_arbitrary_fields() {
    for bytes in [
        br#"{"version":"https://SECRET@example.invalid"}"#.as_slice(),
        br#"{"version":"1.0.0\nSECRET"}"#,
        br#"{"engines":{"node":"SECRET"}}"#,
    ] {
        assert!(super::parse::parse_runtime(bytes).is_err());
    }
    let runtime = super::parse::parse_runtime(RUNTIME).unwrap();
    assert_eq!(runtime.name.as_deref(), Some("@deepseek-ai/dsh"));
    assert_eq!(runtime.reported_version.as_deref(), Some("0.2.2-alpha.3"));
    assert_eq!(runtime.declared_node_engine.as_deref(), Some(">=24.9.0"));
}

#[test]
fn maximum_emitted_metadata_fits_report_budget_even_pretty_printed() {
    let mut dependencies = serde_json::Map::new();
    for i in 0..LIMITS.max_dependencies {
        dependencies.insert(
            format!("p{i:03}{}", "a".repeat(LIMITS.max_name_bytes - 4)),
            "*".into(),
        );
    }
    let bundles: Vec<String> = (0..LIMITS.max_saved_bundles)
        .map(|i| format!("b{i:03}{}", "z".repeat(LIMITS.max_name_bytes - 4)))
        .collect();
    let bytes = serde_json::to_vec(&serde_json::json!({"name":"n".repeat(LIMITS.max_name_bytes),"dependencies":dependencies,"dsh":{"profile":{"bundles":bundles}}})).unwrap();
    let (mut reader, root) = allow_profile(&bytes);
    let runtime_path = root.join("runtime/package.json");
    let runtime = serde_json::json!({
        "name": "r".repeat(LIMITS.max_name_bytes),
        "version": format!("1.2.3-{}", "a".repeat(LIMITS.max_version_bytes - 6)),
        "engines": {"node": format!(">=1.2.3-{}", "a".repeat(LIMITS.max_version_bytes - 8))},
    });
    reader.files.insert(
        runtime_path.clone(),
        Ok(serde_json::to_vec(&runtime).unwrap()),
    );
    let report = inspect_with(&reader, &root, Some(&runtime_path));
    assert_eq!(report.profile_manifest.status, ManifestStatus::Read);
    assert_eq!(
        report.runtime_manifest.as_ref().unwrap().status,
        ManifestStatus::Read
    );
    assert!(serde_json::to_vec_pretty(&report).unwrap().len() < LIMITS.max_report_bytes);
}

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    windows
))]
struct TempFixture(PathBuf);
#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    windows
))]
impl TempFixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "agentdeck-dsh-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, relative: &str, contents: &[u8]) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}
#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    windows
))]
impl Drop for TempFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    windows
))]
#[test]
fn native_reader_does_not_initialize_missing_profiles_or_runtime_paths() {
    let fixture = TempFixture::new();
    let profile = fixture.0.join("missing-profile");
    let runtime = fixture.0.join("missing-runtime/package.json");
    let report = inspect_profile(&profile, Some(&runtime));
    assert_eq!(report.profile_manifest.status, ManifestStatus::Missing);
    assert_eq!(
        report.runtime_manifest.unwrap().status,
        ManifestStatus::Missing
    );
    assert!(!profile.exists());
    assert!(!runtime.parent().unwrap().exists());
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
}

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    windows
))]
#[test]
fn native_fixture_is_unchanged_and_never_resolves_dependencies_or_loads_config() {
    let fixture = TempFixture::new();
    let files = [
        ("profile/package.json", PROFILE),
        ("runtime/package.json", RUNTIME),
        (
            "profile/.credentials.yaml",
            b"SYNTHETIC_CREDENTIAL".as_slice(),
        ),
        ("profile/cordis.yml", b"!!js/function DO_NOT_EVALUATE"),
        ("profile/.env", b"SYNTHETIC_ENV"),
        ("profile/.npmrc", b"SYNTHETIC_NPM_TOKEN"),
        (
            "profile/node_modules/synthetic-plugin/package.json",
            b"SYNTHETIC_INVALID_JSON",
        ),
        ("runtime/lib/bin.js", b"throw Error('MUST_NOT_RUN')"),
    ];
    for (path, bytes) in files {
        fixture.write(path, bytes);
    }
    let before: Vec<_> = files
        .iter()
        .map(|(path, _)| {
            fs::metadata(fixture.0.join(path))
                .unwrap()
                .modified()
                .unwrap()
        })
        .collect();
    let report = inspect_profile(
        &fixture.0.join("profile"),
        Some(&fixture.0.join("runtime/package.json")),
    );
    assert_eq!(report.profile_manifest.status, ManifestStatus::Read);
    assert_eq!(
        report.runtime_manifest.unwrap().status,
        ManifestStatus::Read
    );
    for (index, (path, bytes)) in files.iter().enumerate() {
        assert_eq!(fs::read(fixture.0.join(path)).unwrap(), *bytes);
        assert_eq!(
            fs::metadata(fixture.0.join(path))
                .unwrap()
                .modified()
                .unwrap(),
            before[index]
        );
    }
}

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    windows
))]
#[test]
fn native_reader_rejects_directory_and_oversized_regular_file() {
    let fixture = TempFixture::new();
    fs::create_dir(fixture.0.join("package.json")).unwrap();
    assert_eq!(
        inspect_profile(&fixture.0, None).profile_manifest.issue,
        Some(InspectionIssue::NotRegularFile)
    );
    fs::remove_dir(fixture.0.join("package.json")).unwrap();
    fixture.write("package.json", &vec![b' '; LIMITS.max_manifest_bytes + 1]);
    assert_eq!(
        inspect_profile(&fixture.0, None).profile_manifest.issue,
        Some(InspectionIssue::FileTooLarge)
    );
}

#[cfg(any(
    all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ),
    windows
))]
#[test]
fn native_reader_rejects_hardlink_before_content_read() {
    let fixture = TempFixture::new();
    fixture.write("sensitive-synthetic", b"not JSON");
    fs::hard_link(
        fixture.0.join("sensitive-synthetic"),
        fixture.0.join("package.json"),
    )
    .unwrap();
    assert_eq!(
        inspect_profile(&fixture.0, None).profile_manifest.issue,
        Some(InspectionIssue::MultipleHardLinks)
    );
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn native_reader_rejects_leaf_ancestor_and_profile_symlinks() {
    use std::os::unix::fs::symlink;
    let fixture = TempFixture::new();
    fixture.write("actual/package.json", b"{}");
    symlink(fixture.0.join("actual"), fixture.0.join("linked")).unwrap();
    assert_eq!(
        inspect_profile(&fixture.0.join("linked"), None)
            .profile_manifest
            .issue,
        Some(InspectionIssue::LinkOrReparsePoint)
    );
    symlink(
        fixture.0.join("actual/package.json"),
        fixture.0.join("package.json"),
    )
    .unwrap();
    assert_eq!(
        inspect_profile(&fixture.0, None).profile_manifest.issue,
        Some(InspectionIssue::LinkOrReparsePoint)
    );
    fixture.write("actual/nested/package.json", b"{}");
    assert_eq!(
        inspect_profile(&fixture.0.join("linked/nested"), None)
            .profile_manifest
            .issue,
        Some(InspectionIssue::LinkOrReparsePoint)
    );
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[test]
fn native_reader_rejects_fifo_without_opening_or_blocking() {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let fixture = TempFixture::new();
    let path = fixture.0.join("package.json");
    unsafe extern "C" {
        fn mkfifo(path: *const std::ffi::c_char, mode: u32) -> i32;
    }
    let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: NUL-terminated test-owned path, valid mode, no retained pointer.
    assert_eq!(unsafe { mkfifo(c_path.as_ptr(), 0o600) }, 0);
    assert_eq!(
        inspect_profile(&fixture.0, None).profile_manifest.issue,
        Some(InspectionIssue::NotRegularFile)
    );
}

#[cfg(windows)]
#[test]
fn windows_unc_device_ads_reserved_and_verbatim_paths_fail_before_reads() {
    for path in [
        r"\\server\share\package.json",
        r"\\?\C:\profile\package.json",
        r"\\.\C:\profile\package.json",
        r"C:\profile:secret\package.json",
        r"C:\CON\package.json",
        r"C:\profile.\package.json",
        r"C:\bad?path\package.json",
        r"C:\bad*path\package.json",
    ] {
        assert!(
            validate_runtime_manifest_path(Path::new(path)).is_err(),
            "accepted {path}"
        );
    }
}
