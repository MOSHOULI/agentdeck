use serde::Serialize;

/// Metadata-only observations, not installation, authentication, or apply readiness.
#[derive(Debug, Clone, Serialize)]
pub struct QwenInspectionReport {
    pub mode: InspectionMode,
    pub runtime_manifest: FileObservation,
    pub version_recognition: VersionRecognition,
    pub settings: SettingsObservation,
    pub authentication: UnknownState,
    pub effective_configuration: UnknownState,
    pub mutation_capabilities: MutationCapabilities,
    pub limits: InspectionLimits,
    pub limitations: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionMode {
    ExplicitSettingsSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    NotSelected,
    NotRead,
    Read,
    Missing,
    Unsupported,
    Unreadable,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileObservation {
    pub status: ObservationStatus,
    pub issue: Option<InspectionIssue>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsObservation {
    pub status: ObservationStatus,
    pub issue: Option<InspectionIssue>,
    pub sections: Option<SettingsSections>,
}

/// Known keys only. Presence says nothing about value, validity, ownership or activity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsSections {
    pub general: bool,
    pub ui: bool,
    pub model: bool,
    pub model_providers: bool,
    pub context: bool,
    pub tools: bool,
    pub mcp_servers: bool,
    pub security: bool,
    pub experimental: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionRecognition {
    Unknown,
    ReviewedManifestVersion,
    UnrecognizedManifestVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownState {
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MutationCapabilities {
    pub configure: bool,
    pub authenticate: bool,
    pub install: bool,
    pub restore: bool,
    pub execute: bool,
}

pub use crate::inspection::types::{InspectionIssue, InspectionLimits, LIMITS};
