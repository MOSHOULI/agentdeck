//! Fixed reviewed distribution contract. No network, installation or execution.
use crate::inspection::{reader::NativeSelectedFileReader, types::InspectionIssue};
use serde::Serialize;
use std::path::Path;

pub const VERSION: &str = "1.1.85";
pub const MANIFEST_SHA256: &str =
    "a78234316fe2065cdae6fc17ac425bbc2ae11006d04b6ab5216fd6093597e4de";
pub const RELEASES: &[&str] = &["1.1.85", "1.1.86"];
#[derive(Clone, Copy, Debug)]
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    allow(dead_code)
)]
pub(crate) struct ReviewedRelease {
    pub version: &'static str,
    pub archive_bytes: u64,
    pub tar_bytes: u64,
    pub payload_bytes: u64,
    pub payload_sha256: &'static str,
}
pub(crate) fn reviewed_release(release: &str) -> Result<ReviewedRelease, DistributionError> {
    match release {
        "1.1.85" => Ok(ReviewedRelease {
            version: VERSION,
            archive_bytes: 78_587_775,
            tar_bytes: 182_545_920,
            payload_bytes: 182_543_840,
            payload_sha256: "8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b",
        }),
        "1.1.86" => Ok(ReviewedRelease {
            version: "1.1.86",
            archive_bytes: 78_537_803,
            tar_bytes: 182_414_848,
            payload_bytes: 182_412_768,
            payload_sha256: "fccd0ce90d1119f6bb213146de6e45358db0c89585048c31315ddb180c925ae8",
        }),
        _ => Err(DistributionError::UnknownRelease),
    }
}
pub const LIMITATIONS: &str = "Pinned upstream SHA256 is same-source integrity, not independent signature verification. No installation, execution, authentication, runtime compatibility or effective configuration has been verified. No download, unpacking, PATH, configuration or ownership changes. Explicit target is not host detection. File reads are bounded but not atomic against same-user races and have no mount I/O deadline; access times may change. Windows ARM64 documentation contradicts the manifest; manifest presence does not establish support.";
#[derive(Debug, Serialize)]
pub struct DistributionPlan {
    pub version: &'static str,
    pub target: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub manifest_sha256: &'static str,
    pub archive_name: &'static str,
    pub runtime: &'static str,
    pub variant: &'static str,
    pub min_windows_build: Option<u32>,
    pub host_eligibility: &'static str,
    pub requirements: &'static str,
    pub install_supported: bool,
    pub execution_verified: bool,
    pub signature_verified: bool,
    pub max_archive_bytes: u64,
    pub limitations: &'static str,
}
#[derive(Debug, Serialize)]
pub struct VerifiedPackage {
    pub state: &'static str,
    pub bytes: u64,
    pub plan: DistributionPlan,
}
#[derive(Debug, thiserror::Error)]
pub enum DistributionError {
    #[error("Unknown explicit reviewed Qoder release")]
    UnknownRelease,
    #[error("Unknown explicit Qoder distribution target; use one of the fixed reviewed target IDs")]
    UnknownTarget,
    #[error("Selected Qoder archive is unavailable, unsafe, changed, oversized or unsupported")]
    UnsafeArchive,
    #[error(
        "Archive does not match the pinned Qoder distribution SHA256; do not install or execute it"
    )]
    DigestMismatch,
}
impl From<InspectionIssue> for DistributionError {
    fn from(_: InspectionIssue) -> Self {
        Self::UnsafeArchive
    }
}

pub const TARGETS: &[&str] = &[
    "darwin-arm64",
    "darwin-amd64",
    "linux-arm64-musl",
    "linux-arm64",
    "linux-amd64-baseline",
    "linux-amd64-musl",
    "linux-amd64",
    "openharmony-arm64",
    "windows-arm64-standard",
    "windows-amd64-legacy",
    "windows-amd64-standard",
];
pub fn plan(target: &str) -> Result<DistributionPlan, DistributionError> {
    let (target, url, sha256, archive_name) = match target {
        "darwin-arm64" => (
            "darwin-arm64",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-darwin-arm64.tar.gz",
            "308ab97e1159dbb5f659153ee73c51661587e7aba10008f922c48e99c15c2c4c",
            "qodercli-darwin-arm64.tar.gz",
        ),
        "darwin-amd64" => (
            "darwin-amd64",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-darwin-x64.tar.gz",
            "3e84104ad041e4e2cd9c62338743a6e5e2adea99187bb1c53830cdabf55a6890",
            "qodercli-darwin-x64.tar.gz",
        ),
        "linux-arm64-musl" => (
            "linux-arm64-musl",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-linux-arm64-musl.tar.gz",
            "4f8c4ee4820e7d88ca1cbc63e35246dcd9c30f256805bb9e5f10bba7f9e21e3c",
            "qodercli-linux-arm64-musl.tar.gz",
        ),
        "linux-arm64" => (
            "linux-arm64",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-linux-arm64.tar.gz",
            "d853ae33f2cbf716a2913d6b5e22d0305551f5ce1495fa2a14662c72c3892f7a",
            "qodercli-linux-arm64.tar.gz",
        ),
        "linux-amd64-baseline" => (
            "linux-amd64-baseline",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-linux-x64-baseline.tar.gz",
            "a2fd8cc04711603f9637f54b63097380efc79939739967985cd6193507472dc4",
            "qodercli-linux-x64-baseline.tar.gz",
        ),
        "linux-amd64-musl" => (
            "linux-amd64-musl",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-linux-x64-musl.tar.gz",
            "d96ccee2834d982e301c3926877da38f1785058255511c6b5dbf380f9cdbdd3e",
            "qodercli-linux-x64-musl.tar.gz",
        ),
        "linux-amd64" => (
            "linux-amd64",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-linux-x64.tar.gz",
            "6642210dbc48c10ca50fa427678723dd232c39cb668a18353d1cdc885c0ef3c7",
            "qodercli-linux-x64.tar.gz",
        ),
        "openharmony-arm64" => (
            "openharmony-arm64",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-openharmony-arm64.tar.gz",
            "ceeb5264023eb1a4cde9531ea3c8cf7b28334cfa1268d1d4b8627f11ef0bf07e",
            "qodercli-openharmony-arm64.tar.gz",
        ),
        "windows-arm64-standard" => (
            "windows-arm64-standard",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-windows-arm64.zip",
            "67a9e419f87ce86a2a44c465f92a7cd4170fd6a86b6560c88adc31457b79992d",
            "qodercli-windows-arm64.zip",
        ),
        "windows-amd64-legacy" => (
            "windows-amd64-legacy",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-windows-x64-legacy.zip",
            "ff687cf2573fc5d0fd1dc26b33023c1b0b6570623cc2195589da1f42f92ddd69",
            "qodercli-windows-x64-legacy.zip",
        ),
        "windows-amd64-standard" => (
            "windows-amd64-standard",
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-windows-x64.zip",
            "91ad848d7450a64876b2dd01646db8755ce871766ee655f8c615c6bc05fb5ecd",
            "qodercli-windows-x64.zip",
        ),
        _ => return Err(DistributionError::UnknownTarget),
    };
    let (runtime, variant, min_windows_build, requirements) = match target {
        "windows-amd64-standard" | "windows-arm64-standard" => (
            "bun",
            "standard",
            Some(17763),
            "Manifest requires Windows build >=17763. Architecture must match. ARM64 installation prose contradicts artifact availability. Host compatibility is not evaluated.",
        ),
        "windows-amd64-legacy" => (
            "node-sea",
            "legacy",
            None,
            "Official installer selects legacy for known x64 Windows builds below 17763; manifest declares no minimum build. This is not a guarantee for all older Windows systems.",
        ),
        "linux-amd64" => (
            "not_declared",
            "optimized",
            None,
            "Official bootstrap requires sse4_2 popcnt avx avx2 bmi1 bmi2 fma for optimized Linux x64. libc compatibility and host CPU are not evaluated.",
        ),
        "linux-amd64-baseline" => (
            "not_declared",
            "baseline",
            None,
            "Official bootstrap selects baseline when optimized x64 CPU requirements are absent or unknown. libc and runtime compatibility remain unverified.",
        ),
        "linux-amd64-musl" | "linux-arm64-musl" => (
            "not_declared",
            "musl",
            None,
            "Separate musl artifact; CPU, musl version and host compatibility are not evaluated. Do not substitute based only on architecture.",
        ),
        "linux-arm64" => (
            "not_declared",
            "not_declared",
            None,
            "Separate Linux ARM64 artifact; libc and host runtime compatibility are not evaluated.",
        ),
        "openharmony-arm64" => (
            "not_declared",
            "not_declared",
            None,
            "Manifest-listed OpenHarmony ARM64 only; no tested runtime or minimum OS contract.",
        ),
        _ => (
            "not_declared",
            "not_declared",
            None,
            "Explicit macOS architecture artifact; OS version, code signing/notarization and runtime compatibility are not verified.",
        ),
    };
    Ok(DistributionPlan {
        version: VERSION,
        target,
        url,
        sha256,
        archive_name,
        manifest_sha256: MANIFEST_SHA256,
        runtime,
        variant,
        min_windows_build,
        requirements,
        host_eligibility: "not_evaluated",
        install_supported: false,
        execution_verified: false,
        signature_verified: false,
        max_archive_bytes: 256 * 1024 * 1024,
        limitations: LIMITATIONS,
    })
}
/// The caller cannot supply a digest or remote URL. Inspect only the explicit archive.
pub fn verify_package(target: &str, archive: &Path) -> Result<VerifiedPackage, DistributionError> {
    verify_package_for_release(VERSION, target, archive)
}
pub fn verify_package_for_release(
    release: &str,
    target: &str,
    archive: &Path,
) -> Result<VerifiedPackage, DistributionError> {
    let plan = plan_for_release(release, target)?;
    let (digest, bytes) = NativeSelectedFileReader.hash_archive(archive, plan.archive_name)?;
    if digest != plan.sha256 {
        return Err(DistributionError::DigestMismatch);
    }
    Ok(VerifiedPackage {
        state: "matches_pinned_archive",
        bytes,
        plan,
    })
}

#[cfg(test)]
mod tests;

/// Explicit reviewed release only. Never infer a release from an unsigned receipt.
pub fn plan_for_release(
    release: &str,
    target: &str,
) -> Result<DistributionPlan, DistributionError> {
    let reviewed = reviewed_release(release)?;
    let mut plan = plan(target)?;
    if reviewed.version == VERSION {
        return Ok(plan);
    }
    let (url, sha256) = match target {
        "darwin-arm64" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-darwin-arm64.tar.gz",
            "d57de0037657a8db855bad0136f64a6cf85a296e3aac0952aadd9f82c159c2a1",
        ),
        "darwin-amd64" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-darwin-x64.tar.gz",
            "797c4991a52e7e29e3743c3a53b2b68337b2929c04e94a162d2e44adf8eb3006",
        ),
        "linux-arm64-musl" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-linux-arm64-musl.tar.gz",
            "76f6c0e97e579fa2a332e1d652be6816efe6ce3d57221862112f07f929b54825",
        ),
        "linux-arm64" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-linux-arm64.tar.gz",
            "640d6528a60b045e83eabf57c642096ccec9fb09940633847617c603d703bbaf",
        ),
        "linux-amd64-baseline" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-linux-x64-baseline.tar.gz",
            "9673b64b8f651fc883ea8c29fbd61ff19bf78db02d30db6ad4892b58ab3c9b7e",
        ),
        "linux-amd64-musl" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-linux-x64-musl.tar.gz",
            "ef2cbce359495d4d923071b4bd8b6a997e597191db547504fd3fef42d44769b1",
        ),
        "linux-amd64" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-linux-x64.tar.gz",
            "f88de4dcd41af7d8264ef80da9cc94ef94fb3daf7069920604e8460db2241354",
        ),
        "openharmony-arm64" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-openharmony-arm64.tar.gz",
            "19ee72e6c3ae306cfe73f705c4a761b6775eae229b1646cf28abd83b0c3f42ad",
        ),
        "windows-arm64-standard" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-windows-arm64.zip",
            "40d7d3704b154571ba33ec4608bbfcdca5abf97ee94df3d5f711ca605c9c3b45",
        ),
        "windows-amd64-legacy" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-windows-x64-legacy.zip",
            "f749602e9eaf1a6aa47228a16d49321af963eb16c7aa1f198498283705067af2",
        ),
        "windows-amd64-standard" => (
            "https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-windows-x64.zip",
            "404f991c2ac665f2507930c89ccfcbee07005623530f766329cd7babe910b923",
        ),
        _ => return Err(DistributionError::UnknownTarget),
    };
    plan.version = reviewed.version;
    plan.url = url;
    plan.sha256 = sha256;
    plan.manifest_sha256 = "f323905da3f75088b1e9ae4cf9b0fef6f117879f4f6942b609e92228a533fa78";
    Ok(plan)
}
