//! Explicit, read-only Qwen settings summary. No discovery, execution or writes.
//!
//! Reviewed contract: QwenLM/qwen-code at
//! `6c3d5355de2be05b8d1ab61f4699f52c02c42068`, package version 0.27.0,
//! settings schema `$version: 4`. A selected manifest is only a declaration,
//! never proof that the settings belong to that executable or its effective scope.
//!
//! Native I/O follows the DSH reader's no-follow, bounded regular-file checks.
//! These narrow but do not eliminate same-privilege ancestor replacement races;
//! filesystem reads can block on mounts or update atime. This is not a sandbox.
pub mod owned;
mod parse;
mod reader;
mod types;

use reader::{NativeSelectedFileReader, SelectedFileReader};
use std::path::Path;
pub use types::*;

/// Inspect only two explicitly selected files. No process, HOME lookup, credential
/// file, registry, backup, environment loading, migration or mutation is performed.
pub fn inspect_settings(
    settings_path: &Path,
    runtime_manifest: Option<&Path>,
) -> QwenInspectionReport {
    inspect_with(&NativeSelectedFileReader, settings_path, runtime_manifest)
}

pub fn validate_settings_path(path: &Path) -> Result<(), InspectionIssue> {
    reader::validate_path(path, "settings.json")
}

pub fn validate_runtime_manifest_path(path: &Path) -> Result<(), InspectionIssue> {
    reader::validate_path(path, "package.json")
}

fn inspect_with(
    reader: &impl SelectedFileReader,
    settings_path: &Path,
    runtime_path: Option<&Path>,
) -> QwenInspectionReport {
    let mut report = QwenInspectionReport {
        mode: InspectionMode::ExplicitSettingsSummary,
        runtime_manifest: FileObservation {
            status: ObservationStatus::NotSelected,
            issue: None,
        },
        version_recognition: VersionRecognition::Unknown,
        settings: SettingsObservation {
            status: ObservationStatus::NotRead,
            issue: Some(InspectionIssue::ManifestRequired),
            sections: None,
        },
        authentication: UnknownState::Unknown,
        effective_configuration: UnknownState::Unknown,
        mutation_capabilities: MutationCapabilities::default(),
        limits: LIMITS,
        limitations: vec![
            "Only explicit settings.json and package.json files are read. No code, environment, credential files, other settings layers or registry are loaded; nothing is executed or written.",
            "Only the declared @qwen-code/qwen-code 0.27.0 and schema $version 4 are reviewed. A declaration does not verify installation, authenticity or association with these settings.",
            "Only known section presence is summarized. No configuration values, dynamic keys or raw errors are returned. Presence is not validity, ownership, authentication or effective configuration.",
            "Strict JSON only: comments, BOM and other unsupported syntax produce no summary, not a claim that Qwen rejects the file. No migration or repair is attempted.",
            "No backup, restoration or editing capability is enabled. Other configuration layers, CLI arguments and environment overrides are not resolved.",
            "Link and special-file checks are best effort, not a sandbox against same-user races or hostile mounts. File access times may change; there is no filesystem wall-clock deadline.",
        ],
    };
    // Validate all selections before I/O; an invalid sibling must never cause a read.
    let settings_validation = validate_settings_path(settings_path);
    let runtime_validation = runtime_path.map(validate_runtime_manifest_path).transpose();
    if settings_validation.is_err() || runtime_validation.is_err() {
        let issue = settings_validation
            .err()
            .unwrap_or(InspectionIssue::OtherSelectionInvalid);
        report.settings = settings_failure(issue);
        if runtime_path.is_some() {
            report.runtime_manifest = file_failure(
                runtime_validation
                    .err()
                    .unwrap_or(InspectionIssue::OtherSelectionInvalid),
            );
        }
        return report;
    }
    let Some(path) = runtime_path else {
        return report;
    };
    let runtime = reader
        .read_selected(path)
        .and_then(|bytes| parse::parse_json(&bytes))
        .and_then(|value| {
            let object = value.as_object().ok_or(InspectionIssue::UnsupportedShape)?;
            if object.get("name").and_then(|v| v.as_str()) != Some("@qwen-code/qwen-code")
                || object.get("version").and_then(|v| v.as_str()) != Some("0.27.0")
            {
                return Err(InspectionIssue::UnrecognizedManifest);
            }
            Ok(())
        });
    if let Err(issue) = runtime {
        if issue == InspectionIssue::UnrecognizedManifest {
            report.version_recognition = VersionRecognition::UnrecognizedManifestVersion;
        }
        report.runtime_manifest = file_failure(issue);
        report.settings.issue = Some(InspectionIssue::UnrecognizedManifest);
        return report;
    }
    report.runtime_manifest = FileObservation {
        status: ObservationStatus::Read,
        issue: None,
    };
    report.version_recognition = VersionRecognition::ReviewedManifestVersion;
    let settings = reader
        .read_selected(settings_path)
        .and_then(|bytes| parse::parse_json(&bytes))
        .and_then(|value| {
            let object = value.as_object().ok_or(InspectionIssue::UnsupportedShape)?;
            if object.get("$version").and_then(|v| v.as_u64()) != Some(4) {
                return Err(InspectionIssue::UnsupportedSchema);
            }
            Ok(SettingsSections {
                general: object.contains_key("general"),
                ui: object.contains_key("ui"),
                model: object.contains_key("model"),
                model_providers: object.contains_key("modelProviders"),
                context: object.contains_key("context"),
                tools: object.contains_key("tools"),
                mcp_servers: object.contains_key("mcpServers"),
                security: object.contains_key("security"),
                experimental: object.contains_key("experimental"),
            })
        });
    report.settings = match settings {
        Ok(sections) => SettingsObservation {
            status: ObservationStatus::Read,
            issue: None,
            sections: Some(sections),
        },
        Err(issue) => settings_failure(issue),
    };
    report
}

fn file_failure(issue: InspectionIssue) -> FileObservation {
    FileObservation {
        status: issue.status(),
        issue: Some(issue),
    }
}
fn settings_failure(issue: InspectionIssue) -> SettingsObservation {
    SettingsObservation {
        status: issue.status(),
        issue: Some(issue),
        sections: None,
    }
}
impl InspectionIssue {
    fn status(self) -> ObservationStatus {
        match self {
            Self::Missing => ObservationStatus::Missing,
            Self::PermissionDenied | Self::IoFailure | Self::FileChanged => {
                ObservationStatus::Unreadable
            }
            _ => ObservationStatus::Unsupported,
        }
    }
}

#[cfg(test)]
mod tests;
