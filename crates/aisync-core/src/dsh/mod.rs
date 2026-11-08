//! Passive DSH metadata, with no process runner, configuration loader, or writer.
//!
//! Only the explicitly selected profile's `package.json` and an optional explicitly
//! selected runtime `package.json` are opened. No HOME lookup, directory discovery,
//! dependency resolution, nested package traversal, or module import is performed.
//! Never call DSH's own loaders here: even native `listPlugins` can repair manifests.
//!
//! The declaration shapes were reviewed against deepseek-ai/deepseek-harness at
//! `8bd073392eef6f54e921a093b95076248eb08cbb`: `packages/boot/app-boot/src/profile.ts`,
//! `packages/util/package-manifest/src/types.ts`, and `apps/cli/package.json`.
//! Profile manifests are partial: neither name nor version is required. The CLI
//! declares `@deepseek-ai/dsh` version `0.2.2-alpha.3`, with Node `>=24.9.0`.
//! Re-reviewed the same profile/bundle subset and app-boot package at
//! `fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159` (`0.2.2-rc.1`). Both exact
//! name/version pairs are recognized; no semver range grants compatibility.
//!
//! This is not an OS sandbox. Components and handles are checked, but a same-user
//! adversary can race ancestor replacement, and mounts can block reads. No content
//! write is requested; reading may update filesystem access times. Do not use this
//! API as a security boundary for hostile, concurrently modified filesystems.

mod parse;
mod reader;
mod types;

pub use types::*;

use std::path::Path;

use parse::{parse_profile, parse_runtime};
use reader::{ManifestReader, NativeManifestReader};

/// Inspect only explicitly selected, absolute local package-manifest paths.
///
/// All failures are returned as typed observations; neither errors nor reports
/// expose caller paths, raw JSON, dependency specifications, or OS error messages.
/// Other config files, credentials, executable code and dependency packages are
/// never requested. This call cannot enable any mutation capability.
pub fn inspect_profile(profile_root: &Path, runtime_manifest: Option<&Path>) -> DshInventoryReport {
    inspect_with(&NativeManifestReader, profile_root, runtime_manifest)
}

/// Validate a profile selection without touching the filesystem.
pub fn validate_profile_path(profile_root: &Path) -> Result<(), InspectionIssue> {
    reader::validate_path(profile_root, false)?;
    reader::validate_path(&profile_root.join("package.json"), true)
}

/// Validate a runtime manifest selection without touching the filesystem.
pub fn validate_runtime_manifest_path(path: &Path) -> Result<(), InspectionIssue> {
    reader::validate_path(path, true)
}

fn inspect_with(
    reader: &impl ManifestReader,
    profile_root: &Path,
    runtime_manifest: Option<&Path>,
) -> DshInventoryReport {
    // Validate every selection before any I/O, including a valid sibling path.
    let profile_validation = validate_profile_path(profile_root);
    let runtime_validation = runtime_manifest
        .map(validate_runtime_manifest_path)
        .transpose();
    let profile = profile_validation.and_then(|()| {
        runtime_validation.map_err(|_| InspectionIssue::OtherSelectionInvalid)?;
        reader
            .read_manifest(&profile_root.join("package.json"))
            .and_then(|bytes| parse_profile(&bytes))
    });
    let profile_manifest = match profile {
        Ok(metadata) => ProfileManifestReport {
            status: ManifestStatus::Read,
            issue: None,
            metadata: Some(metadata),
        },
        Err(issue) => ProfileManifestReport {
            status: issue.status(),
            issue: Some(issue),
            metadata: None,
        },
    };
    let runtime_manifest = runtime_manifest.map(|path| {
        let result = validate_runtime_manifest_path(path)
            .and_then(|()| profile_validation.map_err(|_| InspectionIssue::OtherSelectionInvalid))
            .and_then(|()| reader.read_manifest(path))
            .and_then(|bytes| parse_runtime(&bytes));
        match result {
            Ok(metadata) => RuntimeManifestReport {
                status: ManifestStatus::Read,
                issue: None,
                metadata: Some(metadata),
            },
            Err(issue) => RuntimeManifestReport {
                status: issue.status(),
                issue: Some(issue),
                metadata: None,
            },
        }
    });
    let version_recognition = match runtime_manifest.as_ref().and_then(|r| r.metadata.as_ref()) {
        Some(metadata) if metadata.reported_version.is_some() => {
            if matches!(
                metadata.name.as_deref(),
                Some("@deepseek-ai/dsh" | "@deepseek-ai/dsh-app-boot")
            ) && matches!(
                metadata.reported_version.as_deref(),
                Some("0.2.2-alpha.3" | "0.2.2-rc.1")
            ) {
                VersionRecognition::ReviewedManifestVersion
            } else {
                VersionRecognition::UnrecognizedManifestVersion
            }
        }
        _ => VersionRecognition::Unknown,
    };
    DshInventoryReport {
        mode: InspectionMode::PassiveManifestOnly,
        profile_manifest,
        runtime_manifest,
        version_recognition,
        runtime_status: UnknownState::Unknown,
        authentication: UnknownState::Unknown,
        actual_enabled: UnknownState::Unknown,
        mutation_capabilities: MutationCapabilities::default(),
        limits: LIMITS,
        limitations: vec![
            "Only explicitly selected package.json files are read; configuration, credentials, executable code and nested packages are not inspected.",
            "Names, versions, engine requirements, dependencies and bundle selections are unverified declarations, not proof of installation, authenticity, activity or authentication.",
            "Only dependencies, optionalDependencies and peerDependencies are inventoried; no dependency is resolved and no bundle patch is read.",
            "Missing supported fields mean undeclared, not absent at runtime. Rejected manifests return no partial inventory.",
            "Link, reparse-point and special-file checks are best-effort confinement, not a sandbox against same-privilege races or hostile mounts; there is no filesystem wall-clock deadline.",
            "Only Windows and Linux x86_64/aarch64 are supported. No content writes are requested; filesystem access times may change when files are read.",
        ],
    }
}

impl InspectionIssue {
    fn status(self) -> ManifestStatus {
        match self {
            Self::Missing => ManifestStatus::Missing,
            Self::InvalidJson | Self::DuplicateJsonKey => ManifestStatus::Corrupt,
            Self::PermissionDenied | Self::IoFailure | Self::FileChanged => {
                ManifestStatus::Unreadable
            }
            _ => ManifestStatus::Unsupported,
        }
    }
}

#[cfg(test)]
mod tests;
