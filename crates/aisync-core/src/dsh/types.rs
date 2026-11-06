use serde::Serialize;

/// A bounded declaration inventory, never a runtime, authentication, or install probe.
#[derive(Debug, Clone, Serialize)]
pub struct DshInventoryReport {
    pub mode: InspectionMode,
    pub profile_manifest: ProfileManifestReport,
    pub runtime_manifest: Option<RuntimeManifestReport>,
    pub version_recognition: VersionRecognition,
    pub runtime_status: UnknownState,
    pub authentication: UnknownState,
    pub actual_enabled: UnknownState,
    pub mutation_capabilities: MutationCapabilities,
    pub limits: InspectionLimits,
    pub limitations: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionMode {
    PassiveManifestOnly,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileManifestReport {
    pub status: ManifestStatus,
    pub issue: Option<InspectionIssue>,
    pub metadata: Option<ProfileMetadata>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeManifestReport {
    pub status: ManifestStatus,
    pub issue: Option<InspectionIssue>,
    pub metadata: Option<RuntimeMetadata>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestStatus {
    Read,
    Missing,
    Corrupt,
    Unreadable,
    Unsupported,
}

/// Stable, sanitized errors: no filesystem paths, OS errors, JSON text, or secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionIssue {
    PathMustBeAbsolute,
    ParentTraversal,
    UnsupportedPath,
    OtherSelectionInvalid,
    PathLimit,
    ExpectedPackageManifest,
    Missing,
    PermissionDenied,
    IoFailure,
    LinkOrReparsePoint,
    NotDirectory,
    NotRegularFile,
    MultipleHardLinks,
    FileChanged,
    FileTooLarge,
    InvalidJson,
    DuplicateJsonKey,
    JsonLimit,
    UnsupportedShape,
    UnsupportedManifestVersion,
    UnsupportedMetadata,
    DependencyLimit,
    BundleLimit,
    PlatformUnsupported,
}

/// No manifest can establish these facts in this slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownState {
    Unknown,
}

/// Recognition is only of a declared name/version pair, never code authenticity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionRecognition {
    ReviewedManifestVersion,
    UnrecognizedManifestVersion,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileMetadata {
    /// An unverified package.json declaration, not a resolved profile identity.
    pub name: Option<String>,
    /// Complete within the supported sections, or the whole manifest is rejected.
    pub declared_dependencies: Vec<DeclaredDependency>,
    /// Saved selections in declaration order; not resolved, installed, or enabled.
    pub saved_bundle_selections: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeMetadata {
    pub name: Option<String>,
    /// A package.json declaration. This is deliberately not `measured_version`.
    pub reported_version: Option<String>,
    /// A declaration of a requirement, not the installed Node version.
    pub declared_node_engine: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeclaredDependency {
    pub name: String,
    pub section: DependencySection,
    /// Raw specifications are never emitted (URLs and paths can contain secrets).
    pub specification_kind: SpecificationKind,
    pub installed: UnknownState,
    pub actual_enabled: UnknownState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencySection {
    Dependencies,
    OptionalDependencies,
    PeerDependencies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecificationKind {
    RegistryVersionOrRange,
    OtherRedacted,
}

/// Every capability is false, including for reviewed manifest versions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MutationCapabilities {
    pub configure_profile: bool,
    pub manage_plugins: bool,
    pub authenticate: bool,
    pub launch_runtime: bool,
    pub upgrade_runtime: bool,
    pub uninstall_runtime: bool,
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
    pub max_dependencies: usize,
    pub max_saved_bundles: usize,
    pub max_name_bytes: usize,
    pub max_version_bytes: usize,
    pub max_report_bytes: usize,
}

pub const LIMITS: InspectionLimits = InspectionLimits {
    max_files: 2,
    max_manifest_bytes: 65_536,
    max_path_bytes: 4096,
    max_path_components: 64,
    max_json_depth: 16,
    max_json_entries: 4096,
    max_json_string_bytes: 8192,
    max_dependencies: 128,
    max_saved_bundles: 64,
    max_name_bytes: 214,
    max_version_bytes: 64,
    max_report_bytes: 131_072,
};
