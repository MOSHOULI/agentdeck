//! Selected fixed archive verification, with bounded memory parsing only.
//! Never extract to disk, resolve packages, run lifecycle hooks or authorize install.
use crate::inspection::{reader::NativeSelectedFileReader, types::InspectionIssue};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::{io::Read, path::Path};

pub const ARCHIVE_NAME: &str = "dsh-subprocess-local-0.2.2-rc.1.tgz";
pub const BYTES: usize = 77_649;
pub const SHA256: &str = "ac2aae27078f34ccc068c62371672da84fb83bf9230bc26b24e1ecaee9d4ae97";
pub const INTEGRITY: &str = "sha512-/fakFwvyt20BMZTD+Dm89uHyFFMHZzWPmvHWDj/m7Q8wpgeWxX2lFtS2nVFVaqZBmgGUWkUr4tPGPa6a0gLyMw==";
const TAR_BYTES: usize = 279_040;
const CONTENT_BYTES: usize = 249_439;
const PACKAGE: &[u8] = include_bytes!(
    "../../../../docs/upstream/dsh/dependency-evidence/subprocess-package-0.2.2-rc.1.json"
);
const SCRIPT: &[u8] = include_bytes!(
    "../../../../docs/upstream/dsh/dependency-evidence/ensure-spawn-helper-0.2.2-rc.1.mjs"
);
const MEMBERS: &[u8] =
    include_bytes!("../../../../docs/upstream/dsh/subprocess-archive-layout-0.2.2-rc.1.json");

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("Selected DSH subprocess archive is unavailable, unsafe, changed or unsupported")]
    UnsafeArchive,
    #[error("Selected DSH subprocess archive does not match the fixed size and hashes")]
    IntegrityMismatch,
    #[error("DSH subprocess archive does not match the bounded reviewed static content contract")]
    ContentMismatch,
}
impl From<InspectionIssue> for VerifyError {
    fn from(_: InspectionIssue) -> Self {
        Self::UnsafeArchive
    }
}
type Result<T> = std::result::Result<T, VerifyError>;
#[derive(Debug, Serialize)]
pub struct VerifiedSubprocess {
    pub state: &'static str,
    pub package: &'static str,
    pub version: &'static str,
    pub archive_name: &'static str,
    pub bytes: usize,
    pub sha256: &'static str,
    pub integrity: &'static str,
    pub archive_verified_during_this_command: bool,
    pub manifest_snapshot_matched: bool,
    pub postinstall_snapshot_matched: bool,
    pub metadata_archive_agreement: &'static str,
    pub regular_member_count: usize,
    pub closure_complete: bool,
    pub install_ready: bool,
    pub dependencies_resolved: bool,
    pub runtime_verified: bool,
    pub signature_verified: bool,
    pub limitations: &'static str,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Member {
    path: String,
    size: usize,
    sha256: String,
    #[serde(rename = "type")]
    kind: String,
    mode: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    source: String,
    archive_bytes: usize,
    archive_sha256: String,
    tar_bytes: usize,
    members: Vec<Member>,
}
fn mismatch<T>(_: T) -> VerifyError {
    VerifyError::ContentMismatch
}
fn octal(field: &[u8]) -> Result<usize> {
    let raw = std::str::from_utf8(field)
        .map_err(mismatch)?
        .trim_matches(['\0', ' ']);
    if raw.is_empty() || !raw.bytes().all(|v| (b'0'..=b'7').contains(&v)) {
        return Err(VerifyError::ContentMismatch);
    }
    usize::from_str_radix(raw, 8).map_err(mismatch)
}
fn name(field: &[u8]) -> Result<&str> {
    let end = field.iter().position(|b| *b == 0).unwrap_or(field.len());
    if field[end..].iter().any(|b| *b != 0) {
        return Err(VerifyError::ContentMismatch);
    }
    std::str::from_utf8(&field[..end]).map_err(mismatch)
}
fn validate_header<'a>(h: &'a [u8], member: &Member) -> Result<&'a str> {
    if h.len() != 512 {
        return Err(VerifyError::ContentMismatch);
    }
    let path = name(&h[..100])?;
    let sum: usize = h
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if (148..156).contains(&i) {
                32
            } else {
                *v as usize
            }
        })
        .sum();
    if path != member.path
        || !path.starts_with("package/")
        || path
            .split('/')
            .any(|v| v.is_empty() || v == "." || v == "..")
        || &h[257..265] != b"ustar\x0000"
        || member.kind != "regular"
        || member.mode != "0644"
        || path.contains('\\')
        || h[156] != b'0'
        || octal(&h[100..108])? != 0o644
        || !name(&h[157..257])?.is_empty()
        || !name(&h[345..500])?.is_empty()
        || octal(&h[148..156])? != sum
        || octal(&h[124..136])? != member.size
    {
        return Err(VerifyError::ContentMismatch);
    }
    Ok(path)
}
fn parse_tar(tar: &[u8]) -> Result<()> {
    if tar.len() != TAR_BYTES {
        return Err(VerifyError::ContentMismatch);
    }
    if format!("{:x}", Sha256::digest(MEMBERS))
        != "0ccccffe051afe15679f1ae92c4834df3d2891dd73d7d0b74c3f8363205f53e3"
    {
        return Err(VerifyError::ContentMismatch);
    }
    let layout: Layout = serde_json::from_slice(MEMBERS).map_err(mismatch)?;
    if layout.source
        != "https://registry.npmjs.org/@deepseek-ai/dsh-subprocess-local/-/dsh-subprocess-local-0.2.2-rc.1.tgz"
        || layout.archive_bytes != BYTES
        || layout.archive_sha256 != SHA256
        || layout.tar_bytes != TAR_BYTES
    {
        return Err(VerifyError::ContentMismatch);
    }
    let members = layout.members;
    if members.len() != 35 {
        return Err(VerifyError::ContentMismatch);
    }
    let contents = scan_tar(tar, &members)?;
    let mut package_seen = false;
    let mut script_seen = false;
    for (member, content) in members.iter().zip(contents) {
        let path = member.path.as_str();
        validate_member_content(member, content)?;
        package_seen |= path == "package/package.json";
        script_seen |= path == "package/scripts/ensure-spawn-helper.mjs";
    }
    if !package_seen || !script_seen {
        return Err(VerifyError::ContentMismatch);
    }
    Ok(())
}
fn validate_member_content(member: &Member, content: &[u8]) -> Result<()> {
    if content.len() != member.size || format!("{:x}", Sha256::digest(content)) != member.sha256 {
        return Err(VerifyError::ContentMismatch);
    }
    if member.path == "package/package.json" {
        if content != PACKAGE {
            return Err(VerifyError::ContentMismatch);
        }
        super::dependency_evidence::validate_subprocess_manifest(content).map_err(mismatch)?;
    }
    if member.path == "package/scripts/ensure-spawn-helper.mjs" && content != SCRIPT {
        return Err(VerifyError::ContentMismatch);
    }
    Ok(())
}
/// Structural parser independently tested with authored inert bytes. Production
/// always follows with exact per-member content and snapshot verification.
fn scan_tar<'a>(tar: &'a [u8], members: &[Member]) -> Result<Vec<&'a [u8]>> {
    if tar.len() != TAR_BYTES || members.len() != 35 {
        return Err(VerifyError::ContentMismatch);
    }
    let mut position = 0usize;
    let mut total = 0usize;
    let mut contents = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for member in members {
        let h = tar
            .get(position..position + 512)
            .ok_or(VerifyError::ContentMismatch)?;
        let path = validate_header(h, member)?;
        if !seen.insert(path) {
            return Err(VerifyError::ContentMismatch);
        }
        position += 512;
        let content = tar
            .get(
                position
                    ..position
                        .checked_add(member.size)
                        .ok_or(VerifyError::ContentMismatch)?,
            )
            .ok_or(VerifyError::ContentMismatch)?;
        contents.push(content);
        total = total
            .checked_add(member.size)
            .ok_or(VerifyError::ContentMismatch)?;
        let padded = member
            .size
            .checked_add(511)
            .ok_or(VerifyError::ContentMismatch)?
            / 512
            * 512;
        let padding = tar
            .get(position + member.size..position + padded)
            .ok_or(VerifyError::ContentMismatch)?;
        if padding.iter().any(|b| *b != 0) {
            return Err(VerifyError::ContentMismatch);
        }
        position += padded;
    }
    let end = tar.get(position..).ok_or(VerifyError::ContentMismatch)?;
    if total != CONTENT_BYTES || end.len() < 1024 || end.iter().any(|b| *b != 0) {
        return Err(VerifyError::ContentMismatch);
    }
    Ok(contents)
}
fn decode_gzip(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut gz = flate2::bufread::GzDecoder::new(bytes);
    let mut tar = Vec::new();
    (&mut gz)
        .take((TAR_BYTES + 1) as u64)
        .read_to_end(&mut tar)
        .map_err(mismatch)?;
    if tar.len() != TAR_BYTES || !gz.into_inner().is_empty() {
        return Err(VerifyError::ContentMismatch);
    }
    Ok(tar)
}
fn parse_gzip(bytes: &[u8]) -> Result<()> {
    parse_tar(&decode_gzip(bytes)?)
}
fn verify_bytes(bytes: &[u8]) -> Result<VerifiedSubprocess> {
    // Reject even parseable hostile gzip before any decompression or metadata parsing.
    if bytes.len() != BYTES
        || format!("{:x}", Sha256::digest(bytes)) != SHA256
        || format!("sha512-{}", STANDARD.encode(Sha512::digest(bytes))) != INTEGRITY
    {
        return Err(VerifyError::IntegrityMismatch);
    }
    parse_gzip(bytes)?;
    Ok(VerifiedSubprocess {
        state: "subprocess_archive_static_verified",
        package: "@deepseek-ai/dsh-subprocess-local",
        version: "0.2.2-rc.1",
        archive_name: ARCHIVE_NAME,
        bytes: BYTES,
        sha256: SHA256,
        integrity: INTEGRITY,
        archive_verified_during_this_command: true,
        manifest_snapshot_matched: true,
        postinstall_snapshot_matched: true,
        metadata_archive_agreement: "reviewed_declaration_fields_match",
        regular_member_count: 35,
        closure_complete: false,
        install_ready: false,
        dependencies_resolved: false,
        runtime_verified: false,
        signature_verified: false,
        limitations: "Selected fixed prerelease archive only. Bounded in-memory gzip/tar parsing; no filesystem extraction, npm/Node/shell, lifecycle execution, installation or credentials. Matching script bytes is not a script safety verdict. Dependency archives and native binaries remain unverified; complete closure, peer resolution and platform compatibility remain unknown. Same-source hashes are not signatures. Audited reader checks are best effort, not atomic same-user race or mount I/O deadline guarantees; access times may change.",
    })
}
pub fn verify_subprocess_package(archive: &Path) -> Result<VerifiedSubprocess> {
    verify_bytes(&NativeSelectedFileReader.read_dsh_subprocess_archive(archive)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checksum(h: &mut [u8]) {
        h[148..156].fill(b' ');
        let sum: usize = h.iter().map(|b| *b as usize).sum();
        h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    }
    fn header() -> (Vec<u8>, Member) {
        let member = Member {
            path: "package/file.js".into(),
            size: 3,
            sha256: "unused".into(),
            kind: "regular".into(),
            mode: "0644".into(),
        };
        let mut h = vec![0; 512];
        h[..member.path.len()].copy_from_slice(member.path.as_bytes());
        h[100..108].copy_from_slice(b"0000644\0");
        h[124..136].copy_from_slice(b"00000000003\0");
        h[156] = b'0';
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        checksum(&mut h);
        (h, member)
    }
    #[test]
    fn reject_links_special_headers_traversal_size_mode_and_checksum() {
        let (valid, member) = header();
        assert!(validate_header(&valid, &member).is_ok());
        for kind in [
            0, b'1', b'2', b'3', b'4', b'5', b'6', b'x', b'g', b'L', b'K',
        ] {
            let mut h = valid.clone();
            h[156] = kind;
            checksum(&mut h);
            assert!(validate_header(&h, &member).is_err());
        }
        for offset in [100, 124, 157, 345] {
            let mut h = valid.clone();
            h[offset] = b'7';
            checksum(&mut h);
            assert!(validate_header(&h, &member).is_err());
        }
        let mut h = valid.clone();
        h[148] ^= 1;
        assert!(validate_header(&h, &member).is_err());
        for path in [
            "../file",
            "/package/file",
            "package/../file",
            "package//file",
            "package/./file",
            "package/a\\b",
            "package/",
        ] {
            let mut h = valid.clone();
            h[..100].fill(0);
            h[..path.len()].copy_from_slice(path.as_bytes());
            checksum(&mut h);
            let m = Member {
                path: path.into(),
                size: 3,
                sha256: "unused".into(),
                kind: "regular".into(),
                mode: "0644".into(),
            };
            assert!(validate_header(&h, &m).is_err());
        }
    }
    #[test]
    fn structural_member_inventory_rejects_extra_missing_duplicate_and_padding() {
        let layout: Layout = serde_json::from_slice(MEMBERS).unwrap();
        let members = layout.members;
        let mut tar = Vec::new();
        let mut positions = Vec::new();
        for member in &members {
            positions.push(tar.len());
            let (mut h, _) = header();
            h[..100].fill(0);
            h[..member.path.len()].copy_from_slice(member.path.as_bytes());
            h[124..136].copy_from_slice(format!("{:011o}\0", member.size).as_bytes());
            checksum(&mut h);
            tar.extend(h);
            tar.resize(tar.len() + member.size.div_ceil(512) * 512, 0);
        }
        tar.resize(TAR_BYTES, 0);
        assert_eq!(scan_tar(&tar, &members).unwrap().len(), 35);
        // Authored zero payloads are structurally valid, never accepted as real contents.
        assert!(parse_tar(&tar).is_err());
        for index in 0..members.len() {
            let mut changed = tar.clone();
            changed[positions[index]..positions[index] + 512].fill(0);
            assert!(scan_tar(&changed, &members).is_err()); // includes missing manifest and postinstall
        }
        let mut extra = tar.clone();
        extra[TAR_BYTES - 1024..TAR_BYTES - 512].copy_from_slice(&tar[..512]);
        assert!(scan_tar(&extra, &members).is_err());
        let mut duplicate = tar.clone();
        duplicate[positions[1]..positions[1] + 512].copy_from_slice(&tar[..512]);
        assert!(scan_tar(&duplicate, &members).is_err());
        let mut padded = tar.clone();
        let i = members.iter().position(|m| m.size % 512 != 0).unwrap();
        padded[positions[i] + 512 + members[i].size] = 1;
        assert!(scan_tar(&padded, &members).is_err());
        for i in [TAR_BYTES - 1, TAR_BYTES - 1024] {
            let mut changed = tar.clone();
            changed[i] = 1;
            assert!(scan_tar(&changed, &members).is_err());
        }
        assert!(scan_tar(&tar[..TAR_BYTES - 1], &members).is_err());
    }
    #[test]
    fn valid_member_content_baselines_detect_hash_and_snapshot_tampering() {
        let member = |path: &str, bytes: &[u8]| Member {
            path: path.into(),
            size: bytes.len(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            kind: "regular".into(),
            mode: "0644".into(),
        };
        let content = b"authored inert text";
        let ordinary = member("package/example.js", content);
        validate_member_content(&ordinary, content).unwrap();
        let mut modified = content.to_vec();
        modified[0] ^= 1;
        assert!(validate_member_content(&ordinary, &modified).is_err());
        assert!(validate_member_content(&ordinary, &content[..content.len() - 1]).is_err());
        for (path, baseline) in [
            ("package/package.json", PACKAGE),
            ("package/scripts/ensure-spawn-helper.mjs", SCRIPT),
        ] {
            validate_member_content(&member(path, baseline), baseline).unwrap();
            let mut changed = baseline.to_vec();
            changed[0] ^= 1;
            // Even a self-consistent member size/hash cannot replace the reviewed snapshots.
            assert!(validate_member_content(&member(path, &changed), &changed).is_err());
        }
    }
    #[test]
    fn public_integrity_gate_rejects_before_parsing() {
        for bytes in [
            vec![],
            vec![0; BYTES],
            vec![0; BYTES + 1],
            b"not gzip".to_vec(),
        ] {
            assert!(matches!(
                verify_bytes(&bytes),
                Err(VerifyError::IntegrityMismatch)
            ));
        }
    }
    #[test]
    fn gzip_and_tar_bounds_fail_closed() {
        use std::io::Write;
        let gzip = |data: &[u8]| {
            let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(data).unwrap();
            e.finish().unwrap()
        };
        for size in [0, TAR_BYTES - 1, TAR_BYTES + 1, TAR_BYTES * 2] {
            assert!(decode_gzip(&gzip(&vec![0; size])).is_err());
        }
        let baseline = gzip(&vec![0; TAR_BYTES]);
        assert_eq!(decode_gzip(&baseline).unwrap(), vec![0; TAR_BYTES]);
        for removed in [1, 4, 8, baseline.len() / 2] {
            assert!(decode_gzip(&baseline[..baseline.len() - removed]).is_err());
        }
        let mut joined = baseline.clone();
        joined.extend(gzip(b"extra"));
        assert!(decode_gzip(&joined).is_err());
        let mut trailing = gzip(&vec![0; TAR_BYTES]);
        trailing.extend(b"junk");
        assert!(decode_gzip(&trailing).is_err());
        assert!(decode_gzip(b"\x1f\x8b").is_err());
    }
    #[test]
    fn selected_reader_rejects_mismatch_oversize_and_unexpected_name_without_writes() {
        let dir = std::env::temp_dir().join(format!("dsh-subprocess-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let archive = dir.join(ARCHIVE_NAME);
        std::fs::write(&archive, b"sentinel secret").unwrap();
        assert!(verify_subprocess_package(&archive).is_err());
        assert_eq!(std::fs::read(&archive).unwrap(), b"sentinel secret");
        assert!(verify_subprocess_package(&dir.join("other.tgz")).is_err());
        std::fs::write(&archive, vec![0; BYTES + 1]).unwrap();
        assert!(matches!(
            verify_subprocess_package(&archive),
            Err(VerifyError::UnsafeArchive)
        ));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        #[cfg(unix)]
        {
            std::fs::remove_file(&archive).unwrap();
            std::os::unix::fs::symlink("missing", &archive).unwrap();
            assert!(matches!(
                verify_subprocess_package(&archive),
                Err(VerifyError::UnsafeArchive)
            ));
        }
        // Physical aliases must not turn a reviewed basename into a trusted input.
        std::fs::remove_file(&archive).unwrap();
        let source = dir.join("source");
        std::fs::write(&source, b"private").unwrap();
        std::fs::hard_link(&source, &archive).unwrap();
        assert!(matches!(
            verify_subprocess_package(&archive),
            Err(VerifyError::UnsafeArchive)
        ));
        assert_eq!(std::fs::read(&source).unwrap(), b"private");
        std::fs::remove_file(&archive).unwrap();
        std::fs::create_dir(&archive).unwrap();
        assert!(matches!(
            verify_subprocess_package(&archive),
            Err(VerifyError::UnsafeArchive)
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
