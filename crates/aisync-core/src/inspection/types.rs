use serde::Serialize;

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
