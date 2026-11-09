use super::*;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
};
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let p =
            std::env::temp_dir().join(format!("agentdeck-cache-inspect-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Self(p)
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        fs::write(self.0.join(name), bytes).unwrap();
        fs::set_permissions(self.0.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn intent(&self, unpack: bool) {
        self.write("incomplete.json",serde_json::to_string(&serde_json::json!({"format":1,"owner":"agentdeck","state":if unpack {"incomplete_archive_unpack"} else {"incomplete_archive_staging"},"version":distribution::VERSION,"target":staging::TARGET,"installed":false})).unwrap().as_bytes());
    }
    fn state(&self) -> CacheState {
        inspect_cache(&self.0).unwrap().state
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn unknown_and_partial_are_not_cleanup_authorization() {
    let f = Fixture::new();
    assert_eq!(f.state(), CacheState::UnknownOwnership);
    f.intent(false);
    assert_eq!(f.state(), CacheState::Incomplete);
    let report = inspect_cache(&f.0).unwrap();
    assert!(!report.deletion_authorized);
    assert!(report.manual_review_required);
    assert!(!report.installed);
    f.write("archive.tar.gz", b"partial");
    assert_eq!(f.state(), CacheState::Incomplete);
    f.write("qodercli", b"wrong intent");
    assert_eq!(f.state(), CacheState::Conflict);
}
#[test]
fn unknown_records_and_duplicate_fields_fail_closed() {
    let f = Fixture::new();
    for bytes in [
        b"{broken".as_slice(),
        b"{\"owner\":\"foreign\"}",
        b"{\"format\":1,\"format\":1,\"owner\":\"agentdeck\"}",
    ] {
        f.write("incomplete.json", bytes);
        assert_eq!(f.state(), CacheState::UnknownOwnership);
    }
}
#[test]
fn extra_and_pending_never_read_or_approved() {
    let f = Fixture::new();
    f.intent(true);
    f.write("completion.pending", b"unfinished");
    assert_eq!(f.state(), CacheState::Conflict);
    fs::remove_file(f.0.join("completion.pending")).unwrap();
    f.write("SECRET-arbitrary-name", b"SECRET");
    assert_eq!(f.state(), CacheState::Conflict);
    assert!(
        !serde_json::to_string(&inspect_cache(&f.0).unwrap())
            .unwrap()
            .contains("SECRET")
    );
    for i in 0..8 {
        f.write(&format!("unknown-{i}"), b"");
    }
    assert_eq!(f.state(), CacheState::Conflict);
}
#[test]
fn links_fifo_permissions_and_oversize_rejected() {
    let f = Fixture::new();
    f.intent(false);
    let p = f.0.join("archive.tar.gz");
    symlink("incomplete.json", &p).unwrap();
    assert_eq!(f.state(), CacheState::Conflict);
    fs::remove_file(&p).unwrap();
    fs::hard_link(f.0.join("incomplete.json"), &p).unwrap();
    assert_eq!(f.state(), CacheState::Conflict);
    fs::remove_file(&p).unwrap();
    let c = std::ffi::CString::new(p.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    assert_eq!(f.state(), CacheState::Conflict);
    fs::remove_file(&p).unwrap();
    f.write("archive.tar.gz", b"partial");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(f.state(), CacheState::Conflict);
    fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&p)
        .unwrap()
        .set_len(ARCHIVE_BYTES + 1)
        .unwrap();
    assert_eq!(f.state(), CacheState::Conflict);
}
#[test]
fn false_completion_and_cross_kind_records_rejected() {
    let f = Fixture::new();
    f.intent(false);
    f.write("archive.tar.gz", b"not pinned");
    let p = distribution::plan(staging::TARGET).unwrap();
    f.write("complete.json",serde_json::to_string(&serde_json::json!({"format":1,"owner":"agentdeck","state":"verified_archive_staged_not_installed","version":distribution::VERSION,"target":staging::TARGET,"archive_sha256":p.sha256,"bytes":ARCHIVE_BYTES,"installed":false,"extracted":false,"limitations":staging::LIMITATIONS})).unwrap().as_bytes());
    assert_eq!(f.state(), CacheState::Conflict);
    f.intent(true);
    f.write("qodercli", b"partial");
    assert_eq!(f.state(), CacheState::Conflict);
}
#[test]
fn read_only_no_lock_or_modified_times_or_path_leaks() {
    let f = Fixture::new();
    f.intent(true);
    f.write("qodercli", b"partial");
    let before = entries(&fsio::directory(&f.0).unwrap()).unwrap();
    let dm = snapshot(&fs::metadata(&f.0).unwrap());
    let metadata: Vec<_> = before
        .iter()
        .map(|n| {
            (
                n,
                snapshot(&fs::metadata(f.0.join(n)).unwrap()),
                fs::read(f.0.join(n)).unwrap(),
            )
        })
        .collect();
    assert_eq!(f.state(), CacheState::Incomplete);
    assert_eq!(dm, snapshot(&fs::metadata(&f.0).unwrap()));
    assert_eq!(before, entries(&fsio::directory(&f.0).unwrap()).unwrap());
    for (n, m, bytes) in metadata {
        assert_eq!(m, snapshot(&fs::metadata(f.0.join(n)).unwrap()));
        assert_eq!(bytes, fs::read(f.0.join(n)).unwrap());
    }
    assert!(
        !serde_json::to_string(&inspect_cache(&f.0).unwrap())
            .unwrap()
            .contains(f.0.to_str().unwrap())
    );
}
#[test]
fn relative_linked_and_missing_directories_error_without_creation() {
    assert!(inspect_cache(Path::new("relative")).is_err());
    let f = Fixture::new();
    let missing = f.0.join("missing");
    assert!(inspect_cache(&missing).is_err());
    assert!(!missing.exists());
    let link = f.0.join("linked");
    symlink(&f.0, &link).unwrap();
    assert!(inspect_cache(&link).is_err());
}
#[test]
fn bad_same_length_digest_and_file_snapshot_validation() {
    let f = Fixture::new();
    f.write("archive.tar.gz", b"abc");
    let dir = fsio::directory(&f.0).unwrap();
    let expected = format!("{:x}", Sha256::digest(b"abc"));
    assert!(digest(&dir, "archive.tar.gz", 3, &expected).is_ok());
    assert!(digest(&dir, "archive.tar.gz", 3, &"0".repeat(64)).is_err());
    assert!(digest(&dir, "archive.tar.gz", 2, &expected).is_err());
}
