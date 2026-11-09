use super::*;
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "dsh-dist-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn archive(&self) -> std::path::PathBuf {
        self.0.join(ARCHIVE_NAME)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn fixed_plan_matches_reviewed_metadata_and_never_claims_installed() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/upstream/dsh/root-0.2.2-rc.1.json"
    ))
    .unwrap();
    let p = plan();
    assert_eq!(evidence["url"], p.url);
    assert_eq!(evidence["sha256"], p.sha256);
    assert_eq!(evidence["integrity"], p.integrity);
    assert_eq!(evidence["bytes"], p.bytes);
    assert_eq!(evidence["node"], p.node_requirement);
    assert_eq!(
        evidence["direct_dependency_count"],
        p.direct_dependency_count
    );
    assert_eq!(
        evidence["direct_dependency_range_count"],
        p.direct_dependency_range_count
    );
    assert_eq!(p.release_channel, "prerelease");
    assert_eq!(p.platform_compatibility, "unverified");
    assert!(
        !p.dependencies_resolved && !p.installed && !p.runtime_verified && !p.signature_verified
    );
    assert!(p.limitations.contains("postinstall"));
}
#[test]
fn wrong_truncated_and_oversized_archives_never_write_or_echo() {
    let f = Fixture::new();
    let path = f.archive();
    for len in [0, 17, 25197, 65536, 65537] {
        std::fs::write(&path, vec![b'x'; len]).unwrap();
        let before = std::fs::metadata(&path).unwrap().modified().unwrap();
        let error = verify_package(&path).unwrap_err();
        assert!(
            !error
                .to_string()
                .contains(&path.to_string_lossy().to_string())
        );
        assert_eq!(std::fs::read(&path).unwrap(), vec![b'x'; len]);
        assert_eq!(
            std::fs::metadata(&path).unwrap().modified().unwrap(),
            before
        );
        assert_eq!(std::fs::read_dir(&f.0).unwrap().count(), 1);
    }
}
#[test]
fn wrong_name_relative_missing_directory_and_hardlink_rejected() {
    let f = Fixture::new();
    let path = f.archive();
    assert!(verify_package(&path).is_err());
    assert!(verify_package(Path::new(ARCHIVE_NAME)).is_err());
    assert!(verify_package(&f.0).is_err());
    let wrong = f.0.join("not-the-archive.tgz");
    std::fs::write(&wrong, b"wrong").unwrap();
    assert!(verify_package(&wrong).is_err());
    std::fs::hard_link(&wrong, &path).unwrap();
    assert!(matches!(
        verify_package(&path),
        Err(DistributionError::UnsafeArchive) | Err(DistributionError::Unsupported)
    ));
}
#[cfg(unix)]
#[test]
fn symlink_and_fifo_fail_without_following_or_blocking() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let path = f.archive();
    symlink("missing-target", &path).unwrap();
    assert!(verify_package(&path).is_err());
    std::fs::remove_file(&path).unwrap();
    let status = std::process::Command::new("mkfifo")
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    assert!(verify_package(&path).is_err());
}
