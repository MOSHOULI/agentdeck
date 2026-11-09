use super::*;
use sha2::{Digest, Sha256};
use std::fs;
struct Fixture {
    dir: std::path::PathBuf,
    archive: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("qoder-distribution-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let archive = dir.join(plan("linux-amd64-baseline").unwrap().archive_name);
        Self { dir, archive }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn fixed_contract_matches_entire_upstream_manifest() {
    let bytes = include_bytes!("../../../../../docs/upstream/qoder/manifest-1.1.85.json");
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), MANIFEST_SHA256);
    let manifest: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(manifest["latest"], VERSION);
    let files = manifest["files"].as_array().unwrap();
    assert_eq!(TARGETS.len(), files.len());
    for file in files {
        let mut id = format!(
            "{}-{}",
            file["os"].as_str().unwrap(),
            file["arch"].as_str().unwrap()
        );
        if let Some(variant) = file["variant"].as_str() {
            id.push('-');
            id.push_str(variant);
        }
        let p = plan(&id).unwrap();
        assert_eq!(p.url, file["url"]);
        assert_eq!(p.sha256, file["sha256"]);
        assert!(!p.install_supported && !p.execution_verified && !p.signature_verified);
        assert_eq!(p.max_archive_bytes, 256 * 1024 * 1024);
        assert_eq!(p.host_eligibility, "not_evaluated");
        if file["os"] == "windows" {
            assert_eq!(p.runtime, file["runtime"]);
            assert_eq!(p.variant, file["variant"]);
            assert_eq!(
                p.min_windows_build.map(u64::from),
                file["min_windows_build"].as_u64()
            );
        }
    }
    for invalid in ["windows-amd64", "latest", "auto", "linux-x64", "SECRET"] {
        assert!(plan(invalid).is_err());
    }
}
#[test]
fn wrong_truncated_and_empty_archives_fail_without_writes() {
    let f = Fixture::new();
    for bytes in [
        b"".as_slice(),
        b"\x1f\x8b",
        b"SECRET not an official archive",
    ] {
        fs::write(&f.archive, bytes).unwrap();
        assert!(matches!(
            verify_package("linux-amd64-baseline", &f.archive),
            Err(DistributionError::DigestMismatch)
        ));
        assert_eq!(fs::read(&f.archive).unwrap(), bytes);
        assert_eq!(fs::read_dir(&f.dir).unwrap().count(), 1);
    }
}
#[test]
fn streaming_hash_preserves_small_inspection_limit_and_handles_multiple_chunks() {
    let f = Fixture::new();
    let bytes = vec![37u8; 3 * 65536 + 1];
    fs::write(&f.archive, &bytes).unwrap();
    let name = f.archive.file_name().unwrap().to_str().unwrap();
    let (hash, size) = NativeSelectedFileReader
        .hash_archive(&f.archive, name)
        .unwrap();
    assert_eq!(size, bytes.len() as u64);
    assert_eq!(hash, format!("{:x}", Sha256::digest(&bytes)));
    assert!(matches!(
        NativeSelectedFileReader.read_named(&f.archive, name),
        Err(InspectionIssue::FileTooLarge)
    ));
    let file = fs::File::create(&f.archive).unwrap();
    file.set_len(256 * 1024 * 1024 + 1).unwrap();
    assert!(matches!(
        NativeSelectedFileReader.hash_archive(&f.archive, name),
        Err(InspectionIssue::FileTooLarge)
    ));
}
#[test]
fn rejects_wrong_basename_relative_paths_and_directories() {
    let f = Fixture::new();
    assert!(verify_package("linux-amd64-baseline", Path::new("SECRET.tar.gz")).is_err());
    fs::create_dir(&f.archive).unwrap();
    assert!(verify_package("linux-amd64-baseline", &f.archive).is_err());
    let error = verify_package("SECRET", &f.archive)
        .unwrap_err()
        .to_string();
    assert!(!error.contains("SECRET"));
}
#[cfg(unix)]
#[test]
fn symlink_rejected() {
    let f = Fixture::new();
    let source = f.dir.join("source");
    fs::write(&source, b"secret").unwrap();
    std::os::unix::fs::symlink(&source, &f.archive).unwrap();
    assert!(verify_package("linux-amd64-baseline", &f.archive).is_err());
    assert_eq!(fs::read(&source).unwrap(), b"secret");
}
#[test]
fn hardlink_rejected() {
    let f = Fixture::new();
    let source = f.dir.join("source");
    fs::write(&source, b"secret").unwrap();
    fs::hard_link(&source, &f.archive).unwrap();
    assert!(verify_package("linux-amd64-baseline", &f.archive).is_err());
    assert_eq!(fs::read(&source).unwrap(), b"secret");
}
#[cfg(target_os = "linux")]
#[test]
fn fifo_rejected_without_open_blocking() {
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn mkfifo(path: *const std::ffi::c_char, mode: u32) -> i32;
    }
    let f = Fixture::new();
    let path = std::ffi::CString::new(f.archive.as_os_str().as_bytes()).unwrap();
    // SAFETY: valid NUL-terminated pathname and POSIX mode, disposable fixture only.
    assert_eq!(unsafe { mkfifo(path.as_ptr(), 0o600) }, 0);
    assert!(verify_package("linux-amd64-baseline", &f.archive).is_err());
}
