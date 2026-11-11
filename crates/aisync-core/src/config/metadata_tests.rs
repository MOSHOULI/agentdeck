use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("config-extended-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn file(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, b"original").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn attribute(file: &fs::File) {
    use std::os::fd::AsRawFd;
    assert_eq!(
        unsafe {
            libc::fsetxattr(
                file.as_raw_fd(),
                c"user.audit".as_ptr(),
                b"keep".as_ptr().cast(),
                4,
                0,
            )
        },
        0,
        "test requires xattr support: {}",
        std::io::Error::last_os_error()
    );
}
fn annotate(path: &Path) {
    attribute(&fs::File::open(path).unwrap());
}
fn refused<T: std::fmt::Debug>(result: crate::Result<T>) {
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("extended metadata")
    );
}

#[test]
fn extended_metadata_precedes_reads_and_backup_for_every_writer() {
    let fixture = Fixture::new();
    let path = fixture.file("private");
    fs::write(&path, [0xff]).unwrap(); // Any content read/parse would fail differently.
    annotate(&path);
    for format in [
        Format::PlainText,
        Format::JsonMerge,
        Format::JsoncMerge,
        Format::TomlSection,
        Format::YamlSection,
    ] {
        refused(inject_in_home(format, &path, "replacement", &fixture.0));
    }
    refused(managed_instructions(&path, "replacement"));
    let mut journal = Vec::new();
    refused(inject_journaled(
        Format::PlainText,
        &path,
        "replacement",
        Some(&fixture.0),
        &mut journal,
    ));
    assert!(journal.is_empty());
    assert!(!fixture.0.join(".agentdeck/backups").exists());
    assert_eq!(fs::read(&path).unwrap(), [0xff]);
}

#[test]
fn inherited_staging_metadata_refused_before_permissions_or_bytes() {
    let fixture = Fixture::new();
    // Both backup and replacement creation use this exact checked creator.
    for name in ["backup", "replacement"] {
        let path = fixture.0.join(name);
        refused(checked_staging_file(|| {
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            attribute(&file); // deterministic substitute for inherited filesystem ACL
            Ok(file)
        }));
        assert!(fs::read(&path).unwrap().is_empty());
        refused(metadata::check_path(&path)); // never cleared to force success
    }
}

#[test]
fn replacement_rechecks_both_destination_and_staging() {
    for on_target in [false, true] {
        let fixture = Fixture::new();
        let target = fixture.file("target");
        let staged = fixture.file("staged");
        destination::check(&target).unwrap();
        destination::check(&staged).unwrap();
        annotate(if on_target { &target } else { &staged });
        let before = fs::metadata(&target).unwrap();
        refused(replace_staged_file(&target, &staged));
        assert_eq!(fs::metadata(&target).unwrap().ino(), before.ino());
        assert_eq!(fs::read(&target).unwrap(), b"original");
        assert!(staged.exists());
    }
}

#[test]
fn rollback_refuses_added_metadata_for_restore_and_delete() {
    let fixture = Fixture::new();
    let target = fixture.file("target");
    write_bytes(&target, b"committed").unwrap();
    annotate(&target);
    let before = fs::metadata(&target).unwrap();
    refused(rollback_target(&target, Some(b"original".to_vec())));
    refused(rollback_target(&target, None));
    assert_eq!(fs::metadata(&target).unwrap().ino(), before.ino());
    assert_eq!(fs::read(&target).unwrap(), b"committed");
}

#[test]
fn ordinary_backup_write_missing_creation_and_rollback_still_work() {
    let fixture = Fixture::new();
    let target = fixture.file("target");
    let backup = backup_file_in(&target, &fixture.0).unwrap().unwrap();
    assert_eq!(fs::read(&backup).unwrap(), b"original");
    assert_eq!(fs::metadata(&backup).unwrap().mode() & 0o777, 0o600);
    inject_in_home(Format::PlainText, &target, "replacement", &fixture.0).unwrap();
    assert_eq!(fs::metadata(&target).unwrap().mode() & 0o777, 0o600);
    rollback_target(&target, Some(b"original".to_vec())).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"original");
    let missing = fixture.0.join("new/target");
    inject_in_home(Format::PlainText, &missing, "new", &fixture.0).unwrap();
    rollback_target(&missing, None).unwrap();
    assert!(!missing.exists());
}

#[test]
fn skipped_adapters_do_not_inspect_extended_metadata() {
    let fixture = Fixture::new();
    let source = profile_source_dir_in(&fixture.0).unwrap();
    let doc = crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
        .unwrap();
    let mut agent = doc.agents["codex"].clone();
    let target = fixture.file("private.md");
    annotate(&target);
    let personalization = agent.config_inject.personalization.as_mut().unwrap();
    personalization.target = "~/private.md".into();
    personalization.template = "nonexistent-template".into();
    let model = agent.config_inject.model_config.as_mut().unwrap();
    model.file = "~/private.md".into();
    model.format = "unknown".into();
    let report = inject_for_agent_in_home(&agent, &source, &fixture.0).unwrap();
    assert!(matches!(
        report.personalization,
        Some(InjectStatus::Skipped(_))
    ));
    assert!(matches!(
        report.model_config,
        Some(InjectStatus::Skipped(_))
    ));
    assert_eq!(fs::read(target).unwrap(), b"original");
    assert!(!fixture.0.join(".agentdeck/backups").exists());
}

#[test]
fn failed_staging_creator_never_claims_an_existing_file() {
    let fixture = Fixture::new();
    let path = fixture.file("collision");
    let mut created = false;
    assert!(create_staging_file(&path, &mut created).is_err());
    assert!(
        !created,
        "caller must not clean up a file it did not create"
    );
    assert_eq!(fs::read(&path).unwrap(), b"original");
    let missing_parent = fixture.0.join("missing/stage");
    assert!(create_staging_file(&missing_parent, &mut created).is_err());
    assert!(!created);
    assert!(!missing_parent.exists());
}
