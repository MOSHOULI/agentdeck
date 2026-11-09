//! Fixed, offline evidence inventory, not a resolver, installer, or safety verdict.
//! Only compile-time fixtures are parsed. No runtime path, network, or process API.
use super::distribution;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const REVIEWED: &str = "2026-11-09";
const ROOT: &[u8] = include_bytes!(
    "../../../../docs/upstream/dsh/dependency-evidence/root-package-0.2.2-rc.1.json"
);
const ROOT_SHA256: &str = "3e14837ceb3bbc0ba419ed08e9b5ba164338369b1268b93c3c25d89f7b1c9953";
const ROOT_REVIEW: &[u8] = include_bytes!("../../../../docs/upstream/dsh/root-0.2.2-rc.1.json");
const ROOT_REVIEW_SHA256: &str = "e23a5772276c9a7709ffb2daeaa5b2d6099a431a4bb9f48cf8c87c5a2517539e";

struct Snapshot {
    package: &'static str,
    version: &'static str,
    source: &'static str,
    bytes: &'static [u8],
    sha256: &'static str,
}

const SAMPLES: [Snapshot; 3] = [
    Snapshot {
        package: "@deepseek-ai/dsh-subprocess-local",
        version: "0.2.2-rc.1",
        source: "https://registry.npmjs.org/@deepseek-ai%2fdsh-subprocess-local/0.2.2-rc.1",
        bytes: include_bytes!(
            "../../../../docs/upstream/dsh/dependency-evidence/dsh-subprocess-local-0.2.2-rc.1.json"
        ),
        sha256: "4f7bf72dcfdc4769c0c63648d5c1c7276089e39d016a9af305455c816dae7de8",
    },
    Snapshot {
        package: "node-pty",
        version: "1.2.0-beta.15",
        source: "https://registry.npmjs.org/node-pty/1.2.0-beta.15",
        bytes: include_bytes!(
            "../../../../docs/upstream/dsh/dependency-evidence/node-pty-1.2.0-beta.15.json"
        ),
        sha256: "371717f3c9568b63681e11555eeb100e65dc51f02ee6e30e13f8444773bf13aa",
    },
    Snapshot {
        package: "koffi",
        version: "3.1.1",
        source: "https://registry.npmjs.org/koffi/3.1.1",
        bytes: include_bytes!("../../../../docs/upstream/dsh/dependency-evidence/koffi-3.1.1.json"),
        sha256: "10daa36c7aba172d929aa632773419a72b575dfaed8dc680cbe757e1a5e843c7",
    },
];

/// Original published specifications, not selected/resolved versions. Null means
/// the field was not declared in this snapshot, not absence of runtime behavior.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Declarations {
    pub dependencies: Option<BTreeMap<String, String>>,
    pub optional_dependencies: Option<BTreeMap<String, String>>,
    pub peer_dependencies: Option<BTreeMap<String, String>>,
    pub peer_dependencies_meta: Option<BTreeMap<String, serde_json::Value>>,
    pub scripts: Option<BTreeMap<String, String>>,
    pub engines: Option<BTreeMap<String, String>>,
    pub os: Option<Vec<String>>,
    pub cpu: Option<Vec<String>>,
    pub libc: Option<Vec<String>>,
    pub gypfile: Option<bool>,
    pub bundle_dependencies: Option<serde_json::Value>,
    pub bundled_dependencies: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegistryDist {
    pub tarball: String,
    pub integrity: String,
    pub shasum: String,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    name: String,
    version: String,
    #[serde(flatten)]
    declarations: Declarations,
    dist: Option<RegistryDist>,
}

#[derive(Debug, Serialize)]
pub struct Provenance {
    pub source: &'static str,
    pub source_kind: &'static str,
    pub reviewed: &'static str,
    pub snapshot_bytes: usize,
    pub snapshot_sha256: &'static str,
}

#[derive(Debug, Serialize)]
pub struct RootEvidence {
    pub package: &'static str,
    pub version: &'static str,
    pub manifest_source: Provenance,
    pub recorded_archive_state: &'static str,
    pub archive_sha256: &'static str,
    pub archive_integrity: &'static str,
    pub archive_bytes: u64,
    pub archive_verified_during_this_command: bool,
    pub declarations: Declarations,
    pub direct_dependency_count: usize,
    pub direct_exact_version_declaration_count: usize,
    pub direct_unresolved_ranges: BTreeMap<String, String>,
    pub full_implicit_lifecycle_assessed: bool,
}

#[derive(Debug, Serialize)]
pub struct MetadataEvidence {
    pub package: &'static str,
    pub version: &'static str,
    pub state: &'static str,
    pub provenance: Provenance,
    pub registry_dist_declaration: RegistryDist,
    pub declarations: Declarations,
    pub declared_lifecycle_scripts: BTreeMap<String, String>,
    pub dependency_archive_verified: bool,
    pub metadata_archive_agreement: &'static str,
    pub full_implicit_lifecycle_assessed: bool,
}

#[derive(Debug, Serialize)]
pub struct DependencyEvidenceReport {
    pub schema_version: u32,
    pub mode: &'static str,
    pub reviewed: &'static str,
    pub root: RootEvidence,
    pub illustrative_exact_metadata: Vec<MetadataEvidence>,
    pub sample_scope: &'static str,
    pub closure_complete: bool,
    pub install_ready: bool,
    pub dependencies_resolved: bool,
    pub runtime_verified: bool,
    pub signature_verified: bool,
    pub peer_resolution: &'static str,
    pub platform_compatibility: &'static str,
    pub limitations: Vec<&'static str>,
}

#[derive(Debug, thiserror::Error)]
#[error("Embedded DSH dependency evidence failed validation; no report produced")]
pub struct EvidenceError;

fn validate_hash(bytes: &[u8], sha256: &str) -> Result<(), EvidenceError> {
    // Fixed embedded bytes only. This is not a general file or metadata reader.
    if bytes.len() > 65536 || format!("{:x}", Sha256::digest(bytes)) != sha256 {
        return Err(EvidenceError);
    }
    Ok(())
}

fn parse_metadata(
    bytes: &[u8],
    sha256: &str,
    name: &str,
    version: &str,
) -> Result<Metadata, EvidenceError> {
    validate_hash(bytes, sha256)?;
    let metadata: Metadata = serde_json::from_slice(bytes).map_err(|_| EvidenceError)?;
    if metadata.name != name || metadata.version != version {
        return Err(EvidenceError);
    }
    Ok(metadata)
}

/// A deterministic inventory of reviewed bytes. Success means evidence was
/// rendered, never that a dependency was resolved, verified, or installable.
pub fn report() -> Result<DependencyEvidenceReport, EvidenceError> {
    let root = parse_metadata(ROOT, ROOT_SHA256, distribution::NAME, distribution::VERSION)?;
    validate_hash(ROOT_REVIEW, ROOT_REVIEW_SHA256)?;
    let review: serde_json::Value =
        serde_json::from_slice(ROOT_REVIEW).map_err(|_| EvidenceError)?;
    let dependencies = root
        .declarations
        .dependencies
        .as_ref()
        .ok_or(EvidenceError)?;
    let ranges: BTreeMap<_, _> = dependencies
        .iter()
        // This classification is deliberately limited to this fixed root, whose
        // exact declarations all equal VERSION. It is not a semver resolver.
        .filter(|(_, spec)| spec.as_str() != distribution::VERSION)
        .map(|(name, spec)| (name.clone(), spec.clone()))
        .collect();
    if review["package"] != distribution::NAME
        || review["version"] != distribution::VERSION
        || review["reviewed"] != REVIEWED
        || review["url"] != distribution::URL
        || review["sha256"] != distribution::SHA256
        || review["integrity"] != distribution::INTEGRITY
        || review["bytes"] != distribution::BYTES
        || review["package_manifest_sha256"] != ROOT_SHA256
        || review["dependencies"]
            != serde_json::to_value(dependencies).map_err(|_| EvidenceError)?
        || dependencies.len() != 90
        || ranges.len() != 8
        || !ranges.values().all(|spec| spec.starts_with(['~', '^']))
    {
        return Err(EvidenceError);
    }
    let mut samples = Vec::new();
    for snapshot in &SAMPLES {
        let metadata = parse_metadata(
            snapshot.bytes,
            snapshot.sha256,
            snapshot.package,
            snapshot.version,
        )?;
        let lifecycle = metadata
            .declarations
            .scripts
            .iter()
            .flatten()
            .filter(|(name, _)| {
                matches!(
                    name.as_str(),
                    "preinstall"
                        | "install"
                        | "postinstall"
                        | "prepublish"
                        | "preprepare"
                        | "prepare"
                        | "postprepare"
                )
            })
            .map(|(name, script)| (name.clone(), script.clone()))
            .collect();
        samples.push(MetadataEvidence {
            package: snapshot.package,
            version: snapshot.version,
            state: "registry_metadata_observed",
            provenance: Provenance {
                source: snapshot.source,
                source_kind: "exact_version_registry_response",
                reviewed: REVIEWED,
                snapshot_bytes: snapshot.bytes.len(),
                snapshot_sha256: snapshot.sha256,
            },
            registry_dist_declaration: metadata.dist.ok_or(EvidenceError)?,
            declarations: metadata.declarations,
            declared_lifecycle_scripts: lifecycle,
            dependency_archive_verified: false,
            metadata_archive_agreement: "unchecked",
            full_implicit_lifecycle_assessed: false,
        });
    }
    Ok(DependencyEvidenceReport {
        schema_version: 1,
        mode: "fixed_offline_dependency_evidence",
        reviewed: REVIEWED,
        root: RootEvidence {
            package: distribution::NAME,
            version: distribution::VERSION,
            manifest_source: Provenance {
                source: distribution::URL,
                source_kind: "package/package.json_from_reviewed_root_archive",
                reviewed: REVIEWED,
                snapshot_bytes: ROOT.len(),
                snapshot_sha256: ROOT_SHA256,
            },
            recorded_archive_state: "root_archive_verified_at_review",
            archive_sha256: distribution::SHA256,
            archive_integrity: distribution::INTEGRITY,
            archive_bytes: distribution::BYTES,
            archive_verified_during_this_command: false,
            direct_dependency_count: dependencies.len(),
            direct_exact_version_declaration_count: dependencies.len() - ranges.len(),
            direct_unresolved_ranges: ranges,
            declarations: root.declarations,
            full_implicit_lifecycle_assessed: false,
        },
        illustrative_exact_metadata: samples,
        sample_scope: "Three illustrative metadata snapshots, not all direct or transitive dependencies. Root reachability is not established by these samples; only their original declared edges are shown.",
        closure_complete: false,
        install_ready: false,
        dependencies_resolved: false,
        runtime_verified: false,
        signature_verified: false,
        peer_resolution: "unresolved",
        platform_compatibility: "unverified",
        limitations: vec![
            "Frozen review snapshot, not a live registry check or latest-version promise. Refresh requires explicit review of fixture bytes and hashes.",
            "Root archive verification is historical evidence, not a verification of any current local file. Use verify-package for an explicitly selected root tgz.",
            "Dependency maps preserve original specifications and parent identity; exact declarations are not resolved or artifact-verified dependencies. Dev dependencies are outside this report.",
            "No npm consumer lock or complete dependency closure is established. Source-workspace links, patches and allowBuilds do not establish the published npm installation tree.",
            "All selected dependency archives are unchecked; registry metadata has not been compared with their archive manifests or contents. Other dependency metadata is unobserved here.",
            "Lifecycle script strings are declarations only, never executed or analyzed as safe or unsafe. Implicit hooks, native builds and scripts in unobserved dependencies remain unknown.",
            "Optional dependencies and peers remain unresolved. Null fields mean undeclared in the snapshot, not absent at runtime. Platform/engine declarations do not establish host compatibility.",
            "Hashes detect drift of the reviewed bytes; same-source npm hashes are not independent authenticity or signature verification. Registry signatures are not verified.",
            "No downloads, extraction, package manager, Node, shell, local discovery, installation, configuration or filesystem writes occur in this command.",
        ],
    })
}

#[cfg(test)]
mod tests;
