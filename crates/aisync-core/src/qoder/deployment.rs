//! Explicit private executable deployment. Never activation, execution or cache mutation.
use serde::Serialize;
use std::path::Path;

pub const LIMITATIONS: &str = "Private executable file deployment only, not PATH activation, runtime compatibility, authentication or installed-instance verification. No program execution or configuration changes. Same-source fixed hashes and unsigned records are not signatures or authority. Existing destinations are never reused. Failure after creation may leave executable partial output; preserve it for manual review, never retry in place. Same-user races, mount I/O deadlines and power-loss recovery are not solved. Mode bits do not establish executable-mount or runtime readiness.";
#[derive(Debug, Serialize)]
pub struct DeploymentPreview {
    pub version: &'static str,
    pub target: &'static str,
    pub payload_bytes: u64,
    pub payload_sha256: &'static str,
    pub executable_mode: &'static str,
    pub token: String,
    pub limitations: &'static str,
}
#[derive(Debug, Serialize)]
pub struct DeploymentOutcome {
    pub state: &'static str,
    pub version: &'static str,
    pub target: &'static str,
    pub executable_mode: &'static str,
    pub execution_verified: bool,
    pub activated: bool,
    pub limitations: &'static str,
}
#[derive(Debug, thiserror::Error)]
pub enum DeploymentError {
    #[error("Private Qoder deployment is reviewed only for Linux x86_64")]
    Unsupported,
    #[error(
        "Cache or destination is unsafe, changed, unreviewed or already exists; no deployment is authorized by a cache record"
    )]
    Invalid,
    #[error(
        "Deployment preview no longer matches the explicit cache and destination; review a new preview"
    )]
    Changed,
    #[error(
        "Deployment not confirmed complete; partial output MAY ALREADY BE EXECUTABLE. Preserve for manual review; do not retry over the destination or automatically clean it"
    )]
    Incomplete,
}
pub fn preview(cache: &Path, destination: &Path) -> Result<DeploymentPreview, DeploymentError> {
    preview_for_release(crate::qoder::distribution::VERSION, cache, destination)
}
pub fn preview_for_release(
    release: &str,
    cache: &Path,
    destination: &Path,
) -> Result<DeploymentPreview, DeploymentError> {
    let reviewed = crate::qoder::distribution::reviewed_release(release)
        .map_err(|_| DeploymentError::Unsupported)?;

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        linux::preview(reviewed, cache, destination)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (reviewed, cache, destination);
        Err(DeploymentError::Unsupported)
    }
}
pub fn deploy(
    cache: &Path,
    destination: &Path,
    token: &str,
) -> Result<DeploymentOutcome, DeploymentError> {
    deploy_for_release(
        crate::qoder::distribution::VERSION,
        cache,
        destination,
        token,
    )
}
pub fn deploy_for_release(
    release: &str,
    cache: &Path,
    destination: &Path,
    token: &str,
) -> Result<DeploymentOutcome, DeploymentError> {
    let reviewed = crate::qoder::distribution::reviewed_release(release)
        .map_err(|_| DeploymentError::Unsupported)?;

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        linux::deploy(reviewed, cache, destination, token)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (reviewed, cache, destination, token);
        Err(DeploymentError::Unsupported)
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "deployment/linux.rs"]
mod linux;

pub const INSPECTION_LIMITATIONS: &str = "Read-only private deployment observation; not installation, ownership, activation, runtime or authentication verification. Fixed same-source hashes and unsigned records are not signatures or authority. No cache required or filesystem locks, execution, writes, repair, cleanup or permission changes. Reads may update access times; same-user races, atomic snapshots and mount I/O deadlines are not solved. Preserve incomplete or changed data for manual review.";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentInspectionState {
    PayloadVerified,
    Missing,
    Incomplete,
    UnsupportedOrChanged,
}
#[derive(Debug, Serialize)]
pub struct DeploymentInspection {
    pub version: &'static str,
    pub target: &'static str,
    pub state: DeploymentInspectionState,
    pub payload_bytes: u64,
    pub payload_sha256: &'static str,
    pub execution_verified: bool,
    pub activated: bool,
    pub ownership_proven: bool,
    pub deletion_authorized: bool,
    pub repair_authorized: bool,
    pub manual_review_required: bool,
    pub limitations: &'static str,
}
#[derive(Debug, thiserror::Error)]
pub enum DeploymentInspectionError {
    #[error("Deployment inspection is supported only for reviewed releases on Linux x86_64")]
    Unsupported,
    #[error("An explicit absolute deployment directory is required; no changes were made")]
    Unavailable,
}
pub fn inspect_for_release(
    release: &str,
    deployment: &Path,
) -> Result<DeploymentInspection, DeploymentInspectionError> {
    let reviewed = crate::qoder::distribution::reviewed_release(release)
        .map_err(|_| DeploymentInspectionError::Unsupported)?;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let state = linux::inspect(reviewed, deployment)?;
        Ok(DeploymentInspection {
            version: reviewed.version,
            target: crate::qoder::staging::TARGET,
            state,
            payload_bytes: reviewed.payload_bytes,
            payload_sha256: reviewed.payload_sha256,
            execution_verified: false,
            activated: false,
            ownership_proven: false,
            deletion_authorized: false,
            repair_authorized: false,
            manual_review_required: true,
            limitations: INSPECTION_LIMITATIONS,
        })
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (reviewed, deployment);
        Err(DeploymentInspectionError::Unsupported)
    }
}
