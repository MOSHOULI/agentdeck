//! Fixed root npm archive integrity only. No dependency resolution or execution.
use crate::inspection::{reader::NativeSelectedFileReader, types::InspectionIssue};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Serialize;
use sha2::{Digest, Sha256, Sha512};
use std::path::Path;
pub const VERSION: &str = "0.2.2-rc.1";
pub const NAME: &str = "@deepseek-ai/dsh";
pub const ARCHIVE_NAME: &str = "dsh-0.2.2-rc.1.tgz";
pub const URL: &str = "https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.2.2-rc.1.tgz";
pub const SHA256: &str = "ad21968aed0337a326b853507a23b30eacbbec4d67ab07f3b6fde69b75b19cd6";
pub const INTEGRITY: &str = "sha512-ijR9UEVYkG15Q/Q79JzvzLBzQOQfc63VLr9n6flpwHPhr5IKn2UxthnSoto/P9l7qGd7pPvI1Ra6TW7LH3oKpg==";
pub const BYTES: u64 = 25197;
pub const LIMITATIONS: &str = "Root npm archive only, not a self-contained installation. Prerelease fixed from reviewed metadata, not a rolling latest promise. Ninety direct dependencies include ranges; no dependency closure, lockfile, installation or runtime has been verified. No root scripts declaration does not exclude dependency lifecycle scripts (dsh-subprocess-local declares postinstall). Node >=24.9.0 is an upstream declaration, not a tested environment. No os/cpu declaration is not compatibility evidence. Same-source hashes are not independent signature verification. No network, extraction, npm/npx/Node, shell, PATH, profile or configuration changes. Audited reader fails closed on unsupported hosts; same-user races, filesystem I/O deadlines and atomic snapshots are not solved; access times may change.";
#[derive(Debug, Serialize)]
pub struct DistributionPlan {
    pub package: &'static str,
    pub version: &'static str,
    pub release_channel: &'static str,
    pub archive_name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub integrity: &'static str,
    pub bytes: u64,
    pub max_read_bytes: u64,
    pub node_requirement: &'static str,
    pub direct_dependency_count: usize,
    pub direct_dependency_range_count: usize,
    pub dependencies_resolved: bool,
    pub installed: bool,
    pub runtime_verified: bool,
    pub signature_verified: bool,
    pub platform_compatibility: &'static str,
    pub limitations: &'static str,
}
pub fn plan() -> DistributionPlan {
    DistributionPlan {
        package: NAME,
        version: VERSION,
        release_channel: "prerelease",
        archive_name: ARCHIVE_NAME,
        url: URL,
        sha256: SHA256,
        integrity: INTEGRITY,
        bytes: BYTES,
        max_read_bytes: 65536,
        node_requirement: ">=24.9.0",
        direct_dependency_count: 90,
        direct_dependency_range_count: 8,
        dependencies_resolved: false,
        installed: false,
        runtime_verified: false,
        signature_verified: false,
        platform_compatibility: "unverified",
        limitations: LIMITATIONS,
    }
}
#[derive(Debug, Serialize)]
pub struct VerifiedPackage {
    pub state: &'static str,
    pub bytes: u64,
    pub dependencies_resolved: bool,
    pub installed: bool,
    pub runtime_verified: bool,
    pub signature_verified: bool,
    pub platform_compatibility: &'static str,
    pub plan: DistributionPlan,
}
#[derive(Debug, thiserror::Error)]
pub enum DistributionError {
    #[error("DSH root archive reader is unsupported on this host")]
    Unsupported,
    #[error(
        "Selected DSH root archive is unavailable, unsafe, changed or outside the bounded read contract"
    )]
    UnsafeArchive,
    #[error(
        "Selected DSH root archive does not match the pinned size and hashes; no installation or execution is authorized"
    )]
    IntegrityMismatch,
}
impl From<InspectionIssue> for DistributionError {
    fn from(issue: InspectionIssue) -> Self {
        if matches!(issue, InspectionIssue::PlatformUnsupported) {
            Self::Unsupported
        } else {
            Self::UnsafeArchive
        }
    }
}
/// No caller-provided hash, URL or version. The existing 64 KiB selected-source
/// reader checks paths and the same opened handle before/after reading. No unpack.
pub fn verify_package(archive: &Path) -> Result<VerifiedPackage, DistributionError> {
    let bytes = NativeSelectedFileReader.read_named(archive, ARCHIVE_NAME)?;
    if bytes.len() as u64 != BYTES
        || format!("{:x}", Sha256::digest(&bytes)) != SHA256
        || format!("sha512-{}", STANDARD.encode(Sha512::digest(&bytes))) != INTEGRITY
    {
        return Err(DistributionError::IntegrityMismatch);
    }
    Ok(VerifiedPackage {
        state: "root_archive_verified",
        bytes: BYTES,
        dependencies_resolved: false,
        installed: false,
        runtime_verified: false,
        signature_verified: false,
        platform_compatibility: "unverified",
        plan: plan(),
    })
}
#[cfg(test)]
mod tests;
