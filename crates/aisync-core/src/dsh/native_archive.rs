//! Four fixed native dependency archives, verified as data only. No extraction,
//! module loading, lifecycle execution, package resolution or install capability.
use crate::inspection::{reader::NativeSelectedFileReader, types::InspectionIssue};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha512};
use std::{io::Read, path::Path};

const CATALOG: &[u8] =
    include_bytes!("../../../../docs/upstream/dsh/native-evidence/artifacts.json");
const CATALOG_SHA256: &str = "4565645d30baa92db67511f155ad774b1ca4a3d21b1a0e27b37783a248052573";
const SOURCES: &[(&str, &[u8])] = &[
    (
        "koffi/LICENSE.txt.txt",
        include_bytes!("../../../../docs/upstream/dsh/native-evidence/koffi/LICENSE.txt.txt"),
    ),
    (
        "koffi/cnoke.cjs.txt",
        include_bytes!("../../../../docs/upstream/dsh/native-evidence/koffi/cnoke.cjs.txt"),
    ),
    (
        "koffi/metadata.json",
        include_bytes!("../../../../docs/upstream/dsh/native-evidence/koffi/metadata.json"),
    ),
    (
        "koffi/package.json.txt",
        include_bytes!("../../../../docs/upstream/dsh/native-evidence/koffi/package.json.txt"),
    ),
    (
        "koffi/src__koffi__index.cjs.txt",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/koffi/src__koffi__index.cjs.txt"
        ),
    ),
    (
        "koffi-linux-x64/index.js.txt",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/koffi-linux-x64/index.js.txt"
        ),
    ),
    (
        "koffi-linux-x64/metadata.json",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/koffi-linux-x64/metadata.json"
        ),
    ),
    (
        "koffi-linux-x64/package.json.txt",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/koffi-linux-x64/package.json.txt"
        ),
    ),
    (
        "koffi-win32-x64/index.js.txt",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/koffi-win32-x64/index.js.txt"
        ),
    ),
    (
        "koffi-win32-x64/metadata.json",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/koffi-win32-x64/metadata.json"
        ),
    ),
    (
        "koffi-win32-x64/package.json.txt",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/koffi-win32-x64/package.json.txt"
        ),
    ),
    (
        "node-pty/LICENSE.txt",
        include_bytes!("../../../../docs/upstream/dsh/native-evidence/node-pty/LICENSE.txt"),
    ),
    (
        "node-pty/lib__unixTerminal.js.txt",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/node-pty/lib__unixTerminal.js.txt"
        ),
    ),
    (
        "node-pty/lib__utils.js.txt",
        include_bytes!("../../../../docs/upstream/dsh/native-evidence/node-pty/lib__utils.js.txt"),
    ),
    (
        "node-pty/metadata.json",
        include_bytes!("../../../../docs/upstream/dsh/native-evidence/node-pty/metadata.json"),
    ),
    (
        "node-pty/package.json.txt",
        include_bytes!("../../../../docs/upstream/dsh/native-evidence/node-pty/package.json.txt"),
    ),
    (
        "node-pty/scripts__post-install.js.txt",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/node-pty/scripts__post-install.js.txt"
        ),
    ),
    (
        "node-pty/scripts__prebuild.js.txt",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/node-pty/scripts__prebuild.js.txt"
        ),
    ),
    (
        "node-pty/src__unix__pty.cc.txt",
        include_bytes!(
            "../../../../docs/upstream/dsh/native-evidence/node-pty/src__unix__pty.cc.txt"
        ),
    ),
];

#[derive(Debug, Clone, Copy)]
pub enum NativePackage {
    NodePty,
    Koffi,
    KoffiLinuxX64,
    KoffiWin32X64,
}
impl NativePackage {
    pub fn id(self) -> &'static str {
        match self {
            Self::NodePty => "node-pty",
            Self::Koffi => "koffi",
            Self::KoffiLinuxX64 => "koffi-linux-x64",
            Self::KoffiWin32X64 => "koffi-win32-x64",
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("Selected native dependency archive is unavailable, unsafe, changed or unsupported")]
    UnsafeArchive,
    #[error("Selected native dependency archive does not match the fixed size and hashes")]
    IntegrityMismatch,
    #[error(
        "Native dependency evidence or bounded archive content does not match its reviewed contract"
    )]
    ContentMismatch,
}
impl From<InspectionIssue> for VerifyError {
    fn from(_: InspectionIssue) -> Self {
        Self::UnsafeArchive
    }
}
type Result<T> = std::result::Result<T, VerifyError>;
fn mismatch<T>(_: T) -> VerifyError {
    VerifyError::ContentMismatch
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    schema_version: u32,
    reviewed: String,
    source_kind: String,
    artifacts: Vec<Artifact>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    id: String,
    package: String,
    version: String,
    metadata_url: String,
    metadata_sha256: String,
    archive_url: String,
    archive_name: String,
    archive_bytes: usize,
    archive_sha256: String,
    integrity: String,
    tar_bytes: usize,
    content_bytes: usize,
    members: Vec<Member>,
    snapshots: Vec<Snapshot>,
    declarations: serde_json::Value,
}
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Member {
    path: String,
    size: usize,
    mode: u32,
    sha256: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    member: String,
    file: String,
    bytes: usize,
    sha256: String,
}
#[derive(Debug, Serialize)]
pub struct VerifiedNative {
    pub schema_version: u32,
    pub state: &'static str,
    pub package_id: String,
    pub package: String,
    pub version: String,
    pub archive_name: String,
    pub bytes: usize,
    pub sha256: String,
    pub integrity: String,
    pub regular_member_count: usize,
    pub archive_verified_during_this_command: bool,
    pub reviewed_manifest_and_sources_matched: bool,
    pub metadata_archive_agreement: &'static str,
    pub declarations: serde_json::Value,
    pub lifecycle_executed: bool,
    pub native_code_loaded: bool,
    pub closure_complete: bool,
    pub install_ready: bool,
    pub dependencies_resolved: bool,
    pub runtime_verified: bool,
    pub signature_verified: bool,
    pub platform_compatibility: &'static str,
    pub limitations: &'static str,
}
fn source(name: &str) -> Result<&'static [u8]> {
    SOURCES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, b)| *b)
        .ok_or(VerifyError::ContentMismatch)
}
fn catalog() -> Result<Catalog> {
    if CATALOG.len() > 65536 || sha(CATALOG) != CATALOG_SHA256 {
        return Err(VerifyError::ContentMismatch);
    }
    let catalog: Catalog = serde_json::from_slice(CATALOG).map_err(mismatch)?;
    if catalog.schema_version != 1
        || catalog.reviewed != "2026-11-11"
        || catalog.source_kind != "exact_npm_archives_and_registry_metadata"
        || catalog.artifacts.len() != 4
    {
        return Err(VerifyError::ContentMismatch);
    }
    for (a, (id, package, version, size, count)) in catalog.artifacts.iter().zip([
        ("node-pty", "node-pty", "1.2.0-beta.15", 7_150_506, 49),
        ("koffi", "koffi", "3.1.1", 302_173, 81),
        (
            "koffi-linux-x64",
            "@koromix/koffi-linux-x64",
            "3.1.1",
            614_185,
            5,
        ),
        (
            "koffi-win32-x64",
            "@koromix/koffi-win32-x64",
            "3.1.1",
            399_006,
            5,
        ),
    ]) {
        if a.id != id
            || a.package != package
            || a.version != version
            || a.archive_bytes != size
            || a.members.len() != count
            || a.tar_bytes > 32 * 1024 * 1024
            || a.tar_bytes % 512 != 0
            || a.archive_url.rsplit('/').next() != Some(a.archive_name.as_str())
        {
            return Err(VerifyError::ContentMismatch);
        }
        validate_sources(a)?;
    }
    Ok(catalog)
}
fn validate_sources(a: &Artifact) -> Result<()> {
    let metadata = source(&format!("{}/metadata.json", a.id))?;
    if sha(metadata) != a.metadata_sha256 {
        return Err(VerifyError::ContentMismatch);
    }
    let metadata: serde_json::Value = serde_json::from_slice(metadata).map_err(mismatch)?;
    if metadata["name"] != a.package
        || metadata["version"] != a.version
        || metadata["dist"]["tarball"] != a.archive_url
        || metadata["dist"]["integrity"] != a.integrity
        || a.metadata_url
            != format!(
                "https://registry.npmjs.org/{}/{}",
                a.package.replace('/', "%2f"),
                a.version
            )
    {
        return Err(VerifyError::ContentMismatch);
    }
    let mut manifest_seen = false;
    for snapshot in &a.snapshots {
        let bytes = source(&snapshot.file)?;
        let member = a
            .members
            .iter()
            .find(|m| m.path == snapshot.member)
            .ok_or(VerifyError::ContentMismatch)?;
        if bytes.len() != snapshot.bytes
            || sha(bytes) != snapshot.sha256
            || member.sha256 != snapshot.sha256
            || member.size != bytes.len()
        {
            return Err(VerifyError::ContentMismatch);
        }
        if snapshot.member == "package/package.json" {
            manifest_seen = true;
            let manifest: serde_json::Value = serde_json::from_slice(bytes).map_err(mismatch)?;
            if manifest["name"] != a.package || manifest["version"] != a.version {
                return Err(VerifyError::ContentMismatch);
            }
            for (key, value) in a
                .declarations
                .as_object()
                .ok_or(VerifyError::ContentMismatch)?
            {
                if &manifest[key] != value || &metadata[key] != value {
                    return Err(VerifyError::ContentMismatch);
                }
            }
        }
    }
    if !manifest_seen {
        return Err(VerifyError::ContentMismatch);
    }
    Ok(())
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
            .any(|p| p.is_empty() || p == "." || p == "..")
        || path.chars().any(|c| c.is_control() || c == '\\')
        || &h[257..265] != b"ustar\x0000"
        || h[156] != b'0'
        || !matches!(member.mode, 0o644 | 0o755)
        || octal(&h[100..108])? != member.mode as usize
        || !name(&h[157..257])?.is_empty()
        || !name(&h[345..500])?.is_empty()
        || octal(&h[148..156])? != sum
        || octal(&h[124..136])? != member.size
    {
        return Err(VerifyError::ContentMismatch);
    }
    Ok(path)
}
// A separate structural pass is testable with authored inert bytes. Production
// first checks the entire immutable compressed digest, then every member digest.
fn scan_tar<'a>(tar: &'a [u8], a: &Artifact) -> Result<Vec<&'a [u8]>> {
    if tar.len() != a.tar_bytes || tar.len() > 32 * 1024 * 1024 || a.members.len() > 81 {
        return Err(VerifyError::ContentMismatch);
    }
    let mut position = 0usize;
    let mut total = 0usize;
    let mut contents = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for member in &a.members {
        let header_end = position
            .checked_add(512)
            .ok_or(VerifyError::ContentMismatch)?;
        let h = tar
            .get(position..header_end)
            .ok_or(VerifyError::ContentMismatch)?;
        let path = validate_header(h, member)?;
        if !seen.insert(path) {
            return Err(VerifyError::ContentMismatch);
        }
        position = header_end;
        let end = position
            .checked_add(member.size)
            .ok_or(VerifyError::ContentMismatch)?;
        contents.push(tar.get(position..end).ok_or(VerifyError::ContentMismatch)?);
        total = total
            .checked_add(member.size)
            .ok_or(VerifyError::ContentMismatch)?;
        let padded = member
            .size
            .checked_add(511)
            .ok_or(VerifyError::ContentMismatch)?
            / 512
            * 512;
        let padded_end = position
            .checked_add(padded)
            .ok_or(VerifyError::ContentMismatch)?;
        let padding = tar
            .get(end..padded_end)
            .ok_or(VerifyError::ContentMismatch)?;
        if padding.iter().any(|b| *b != 0) {
            return Err(VerifyError::ContentMismatch);
        }
        position = padded_end;
    }
    let end = tar.get(position..).ok_or(VerifyError::ContentMismatch)?;
    if total != a.content_bytes || end.len() != 1024 || end.iter().any(|b| *b != 0) {
        return Err(VerifyError::ContentMismatch);
    }
    Ok(contents)
}
fn decode(bytes: &[u8], limit: usize) -> Result<Vec<u8>> {
    if limit > 32 * 1024 * 1024 {
        return Err(VerifyError::ContentMismatch);
    }
    let mut gz = flate2::bufread::GzDecoder::new(bytes);
    let mut tar = Vec::new();
    (&mut gz)
        .take((limit + 1) as u64)
        .read_to_end(&mut tar)
        .map_err(mismatch)?;
    if tar.len() != limit || !gz.into_inner().is_empty() {
        return Err(VerifyError::ContentMismatch);
    }
    Ok(tar)
}
fn verify_bytes(bytes: &[u8], a: Artifact) -> Result<VerifiedNative> {
    // Hash gate always precedes gzip/JSON/TAR parsing of user-selected bytes.
    if bytes.len() != a.archive_bytes
        || sha(bytes) != a.archive_sha256
        || format!("sha512-{}", STANDARD.encode(Sha512::digest(bytes))) != a.integrity
    {
        return Err(VerifyError::IntegrityMismatch);
    }
    let tar = decode(bytes, a.tar_bytes)?;
    for (member, content) in a.members.iter().zip(scan_tar(&tar, &a)?) {
        if sha(content) != member.sha256 {
            return Err(VerifyError::ContentMismatch);
        }
        if let Some(snapshot) = a.snapshots.iter().find(|s| s.member == member.path)
            && content != source(&snapshot.file)?
        {
            return Err(VerifyError::ContentMismatch);
        }
    }
    Ok(VerifiedNative {
        schema_version: 1,
        state: "native_archive_static_verified",
        package_id: a.id,
        package: a.package,
        version: a.version,
        archive_name: a.archive_name,
        bytes: a.archive_bytes,
        sha256: a.archive_sha256,
        integrity: a.integrity,
        regular_member_count: a.members.len(),
        archive_verified_during_this_command: true,
        reviewed_manifest_and_sources_matched: true,
        metadata_archive_agreement: "reviewed_declaration_fields_match",
        declarations: a.declarations,
        lifecycle_executed: false,
        native_code_loaded: false,
        closure_complete: false,
        install_ready: false,
        dependencies_resolved: false,
        runtime_verified: false,
        signature_verified: false,
        platform_compatibility: "unverified",
        limitations: "One selected exact native dependency archive only. Bounded in-memory verification, no extraction, npm/Node/shell, import, native loading, lifecycle execution, installation, PATH or configuration changes. Source snapshots describe behavior, not permission or a safety verdict. node-addon-api ranges, other platform dependencies and the full DSH closure remain unresolved. Native ABI, libc/OS and runtime compatibility are untested. Same-source hashes are not signatures. File checks are best effort, not hostile same-user race or mount I/O deadline guarantees; access times may change.",
    })
}
pub fn verify_native_package(package: NativePackage, archive: &Path) -> Result<VerifiedNative> {
    let a = catalog()?
        .artifacts
        .into_iter()
        .find(|a| a.id == package.id())
        .ok_or(VerifyError::ContentMismatch)?;
    crate::inspection::reader::validate_path(archive, &a.archive_name)?;
    verify_bytes(
        &NativeSelectedFileReader.read_dsh_native_archive(archive)?,
        a,
    )
}

#[cfg(test)]
mod tests;
