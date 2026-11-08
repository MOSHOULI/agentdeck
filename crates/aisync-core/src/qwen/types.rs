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

/// Sanitized fixed issues; never paths, JSON text, OS messages, or input values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionIssue {
    PathMustBeAbsolute,
    ParentTraversal,
    UnsupportedPath,
    OtherSelectionInvalid,
    PathLimit,
    UnexpectedFileName,
    Missing,
    PermissionDenied,
    IoFailure,
    LinkOrReparsePoint,
    NotDirectory,
    NotRegularFile,
    MultipleHardLinks,
    FileChanged,
    FileTooLarge,
    UnsupportedJson,
    DuplicateJsonKey,
    JsonLimit,
    UnsupportedShape,
    UnsupportedSchema,
    UnrecognizedManifest,
    ManifestRequired,
    PlatformUnsupported,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct InspectionLimits {
    pub max_files: usize,
    pub max_manifest_bytes: usize,
    pub max_path_bytes: usize,
    pub max_path_components: usize,
    pub max_json_depth: usize,
    pub max_json_entries: usize,
    pub max_json_string_bytes: usize,
}

pub const LIMITS: InspectionLimits = InspectionLimits {
    max_files: 2,
    max_manifest_bytes: 65_536,
    max_path_bytes: 4096,
    max_path_components: 64,
    max_json_depth: 16,
    max_json_entries: 4096,
    max_json_string_bytes: 8192,
};
