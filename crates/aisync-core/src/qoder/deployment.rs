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
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        linux::preview(cache, destination)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (cache, destination);
        Err(DeploymentError::Unsupported)
    }
}
pub fn deploy(
    cache: &Path,
    destination: &Path,
    token: &str,
) -> Result<DeploymentOutcome, DeploymentError> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        linux::deploy(cache, destination, token)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (cache, destination, token);
        Err(DeploymentError::Unsupported)
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "deployment/linux.rs"]
mod linux;
