use super::*;
use flate2::{Compression, write::GzEncoder};
fn header(name: &str, kind: u8, size: u64) -> tar::Header {
    let mut h = tar::Header::new_ustar();
    h.set_path(name).unwrap();
    h.set_entry_type(tar::EntryType::new(kind));
    h.set_size(size);
    h.set_mode(0o755);
    h.set_cksum();
    h
}
fn raw_tar() -> Vec<u8> {
    let mut root = header("./", b'5', 0);
    root.as_mut_bytes()[..100].fill(0);
    root.as_mut_bytes()[..2].copy_from_slice(b"./");
    root.set_cksum();
    let mut file = header("./qodercli", b'0', 3);
    file.as_mut_bytes()[..100].fill(0);
    file.as_mut_bytes()[..10].copy_from_slice(b"./qodercli");
    file.set_cksum();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(root.as_bytes());
    bytes.extend_from_slice(file.as_bytes());
    bytes.extend_from_slice(b"abc");
    bytes.resize(10240, 0);
    bytes
}
fn gzip(data: &[u8]) -> Vec<u8> {
    let mut out = GzEncoder::new(Vec::new(), Compression::fast());
    out.write_all(data).unwrap();
    out.finish().unwrap()
}
fn parse(data: &[u8]) -> io::Result<(String, String)> {
    extract(data, &mut Vec::new(), 3, 10240)
}
fn fix_checksum(bytes: &mut [u8], offset: usize) {
    let mut h = tar::Header::from_byte_slice(&bytes[offset..offset + 512]).clone();
    h.set_cksum();
    bytes[offset..offset + 512].copy_from_slice(h.as_bytes());
}
#[test]
fn strict_small_fixture_and_both_stream_hashes() {
    let bytes = gzip(&raw_tar());
    let (h, c) = parse(&bytes).unwrap();
    assert_eq!(h, format!("{:x}", Sha256::digest(b"abc")));
    assert_eq!(c, format!("{:x}", Sha256::digest(&bytes)));
}
#[test]
fn crc_truncation_junk_and_concatenated_gzip_rejected() {
    let valid = gzip(&raw_tar());
    let mut crc = valid.clone();
    let n = crc.len();
    crc[n - 8] ^= 1;
    let mut junk = valid.clone();
    junk.extend_from_slice(b"junk");
    let mut concat = valid.clone();
    concat.extend_from_slice(&valid);
    for data in [crc, junk, concat, valid[..valid.len() - 5].to_vec()] {
        assert!(parse(&data).is_err());
    }
}
#[test]
fn raw_extension_link_special_and_wrong_size_headers_rejected() {
    for kind in *b"xgLKS123465" {
        let mut t = raw_tar();
        t[512 + 156] = kind;
        fix_checksum(&mut t, 512);
        assert!(parse(&gzip(&t)).is_err());
    }
    let mut t = raw_tar();
    let mut h = tar::Header::from_byte_slice(&t[512..1024]).clone();
    h.set_size(u64::MAX);
    h.set_cksum();
    t[512..1024].copy_from_slice(h.as_bytes());
    assert!(parse(&gzip(&t)).is_err());
}
#[test]
fn traversal_absolute_duplicate_extra_and_nonzero_padding_rejected() {
    for path in [b"../qodercli".as_slice(), b"/qodercli", b"./", b"qodercli"] {
        let mut t = raw_tar();
        t[512..612].fill(0);
        t[512..512 + path.len()].copy_from_slice(path);
        fix_checksum(&mut t, 512);
        assert!(parse(&gzip(&t)).is_err());
    }
    for offset in [1027, 1536, 10239] {
        let mut t = raw_tar();
        t[offset] = 1;
        assert!(parse(&gzip(&t)).is_err());
    }
    let mut t = raw_tar();
    t.truncate(2048);
    assert!(parse(&gzip(&t)).is_err());
    let mut t = raw_tar();
    t.extend_from_slice(&[0; 512]);
    assert!(parse(&gzip(&t)).is_err());
}
#[test]
fn bad_checksum_prefix_and_linkname_rejected() {
    for offset in [0, 512 + 345, 512 + 157] {
        let mut t = raw_tar();
        t[offset] = b'x';
        if offset != 0 {
            fix_checksum(&mut t, 512);
        }
        assert!(parse(&gzip(&t)).is_err());
    }
}
#[test]
fn writer_failure_propagates() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("write failure"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert!(extract(gzip(&raw_tar()).as_slice(), &mut Broken, 3, 10240).is_err());
}
struct CacheFixture {
    root: std::path::PathBuf,
}
impl CacheFixture {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("qoder-unpack-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let d = fsio::directory(&root).unwrap();
        fsio::record(&d,"incomplete.json",&serde_json::json!({"format":1,"owner":"agentdeck","state":"incomplete_archive_staging","version":distribution::VERSION,"target":staging::TARGET,"installed":false})).unwrap();
        fsio::record(&d,"complete.json",&serde_json::json!({"format":1,"owner":"agentdeck","state":"verified_archive_staged_not_installed","version":distribution::VERSION,"target":staging::TARGET,"archive_sha256":distribution::plan(staging::TARGET).unwrap().sha256,"bytes":ARCHIVE_BYTES,"installed":false,"extracted":false,"limitations":staging::LIMITATIONS})).unwrap();
        let a = fsio::new_file(&d, "archive.tar.gz").unwrap();
        a.set_len(ARCHIVE_BYTES).unwrap();
        Self { root }
    }
}
impl Drop for CacheFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
#[test]
fn records_reject_duplicates_unknown_keys_wrong_state_and_no_execution_claims() {
    let f = CacheFixture::new();
    let d = fsio::directory(&f.root).unwrap();
    assert!(validate_records(&d).is_ok());
    let p = f.root.join("complete.json");
    let original = std::fs::read_to_string(&p).unwrap();
    for text in [
        original.replacen('{', "{\"owner\":\"agentdeck\",", 1),
        original.replacen('{', "{\"surprise\":true,", 1),
        original.replace("verified_archive_staged_not_installed", "installed"),
        original.replace("\"installed\": false", "\"installed\": true"),
    ] {
        std::fs::write(&p, text).unwrap();
        assert!(validate_records(&d).is_err());
    }
}
#[test]
fn linked_or_overpermissive_record_rejected() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let f = CacheFixture::new();
    let d = fsio::directory(&f.root).unwrap();
    let p = f.root.join("complete.json");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(validate_records(&d).is_err());
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
    let alias = f.root.join("other");
    std::fs::hard_link(&p, &alias).unwrap();
    assert!(validate_records(&d).is_err());
    std::fs::remove_file(&alias).unwrap();
    std::fs::rename(&p, &alias).unwrap();
    symlink(&alias, &p).unwrap();
    assert!(validate_records(&d).is_err());
}
#[test]
fn existing_destination_never_reused_even_with_valid_cache_record() {
    let f = CacheFixture::new();
    let dest = f.root.join("already");
    std::fs::create_dir(&dest).unwrap();
    std::fs::write(dest.join("keep"), b"sentinel").unwrap();
    assert!(matches!(
        unpack(
            distribution::reviewed_release(distribution::VERSION).unwrap(),
            &f.root,
            &dest
        ),
        Err(UnpackError::Destination)
    ));
    assert_eq!(std::fs::read_dir(&dest).unwrap().count(), 1);
    assert_eq!(std::fs::read(dest.join("keep")).unwrap(), b"sentinel");
}
#[test]
fn forged_complete_record_does_not_authorize_wrong_archive() {
    let f = CacheFixture::new();
    let dest = f.root.join("unpacked");
    assert!(
        unpack(
            distribution::reviewed_release(distribution::VERSION).unwrap(),
            &f.root,
            &dest
        )
        .is_err()
    );
    assert!(!dest.join("complete.json").exists());
    assert!(!dest.join("qodercli").exists());
    assert!(dest.join("incomplete.json").is_file());
}

#[test]
fn release_contracts_reject_mixed_receipts_before_destination_creation() {
    use std::fs;
    let f = CacheFixture::new();
    let d = fsio::directory(&f.root).unwrap();
    for version in distribution::RELEASES {
        let r = distribution::reviewed_release(version).unwrap();
        let p = distribution::plan_for_release(version, staging::TARGET).unwrap();
        let intent = serde_json::json!({"format":1,"owner":"agentdeck","state":"incomplete_archive_staging","version":version,"target":staging::TARGET,"installed":false});
        let complete = serde_json::json!({"format":1,"owner":"agentdeck","state":"verified_archive_staged_not_installed","version":version,"target":staging::TARGET,"archive_sha256":p.sha256,"bytes":r.archive_bytes,"installed":false,"extracted":false,"limitations":staging::LIMITATIONS});
        fs::write(
            f.root.join("incomplete.json"),
            serde_json::to_vec(&intent).unwrap(),
        )
        .unwrap();
        fs::write(
            f.root.join("complete.json"),
            serde_json::to_vec(&complete).unwrap(),
        )
        .unwrap();
        assert!(validate_records_for_release(r, &d).is_ok()); // receipt contract only, not byte verification
        for other in distribution::RELEASES
            .iter()
            .filter(|other| *other != version)
        {
            let other_r = distribution::reviewed_release(other).unwrap();
            let destination = f.root.join("must-not-exist");
            assert!(unpack(other_r, &f.root, &destination).is_err());
            assert!(!destination.exists());
            for field in ["version", "archive_sha256", "bytes"] {
                let mut mixed = complete.clone();
                mixed[field] = match field {
                    "version" => serde_json::json!(other),
                    "archive_sha256" => serde_json::json!(
                        distribution::plan_for_release(other, staging::TARGET)
                            .unwrap()
                            .sha256
                    ),
                    _ => serde_json::json!(other_r.archive_bytes),
                };
                fs::write(
                    f.root.join("complete.json"),
                    serde_json::to_vec(&mixed).unwrap(),
                )
                .unwrap();
                assert!(unpack(r, &f.root, &destination).is_err());
                assert!(!destination.exists());
            }
            fs::write(
                f.root.join("complete.json"),
                serde_json::to_vec(&complete).unwrap(),
            )
            .unwrap();
            let mut mixed_intent = intent.clone();
            mixed_intent["version"] = serde_json::json!(other);
            fs::write(
                f.root.join("incomplete.json"),
                serde_json::to_vec(&mixed_intent).unwrap(),
            )
            .unwrap();
            assert!(unpack(r, &f.root, &destination).is_err());
            assert!(!destination.exists());
            fs::write(
                f.root.join("incomplete.json"),
                serde_json::to_vec(&intent).unwrap(),
            )
            .unwrap();
            assert!(validate_records_for_release(r, &d).is_ok());
        }
    }
}
