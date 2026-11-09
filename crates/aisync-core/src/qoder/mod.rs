//! Qoder CLI explicit scope inventory based on official docs reviewed 2026-11-08.
//! No effective-config claim, automatic discovery, execution, BYOK editing or writes.
pub mod distribution;
pub mod staging;
pub use crate::inspection::types::{InspectionIssue, InspectionLimits};
use crate::inspection::{
    parse,
    reader::{self, NativeSelectedFileReader, SelectedFileReader},
};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};

pub const LIMITS: InspectionLimits = InspectionLimits {
    max_files: 4,
    ..crate::inspection::types::LIMITS
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    ProjectMcp,
}
impl Scope {
    pub fn filename(self) -> &'static str {
        match self {
            Self::UserSettings | Self::ProjectSettings => "settings.json",
            Self::LocalSettings => "settings.local.json",
            Self::ProjectMcp => ".mcp.json",
        }
    }
}
#[derive(Default)]
pub struct Selections<'a> {
    pub user_settings: Option<&'a Path>,
    pub project_settings: Option<&'a Path>,
    pub local_settings: Option<&'a Path>,
    pub project_mcp: Option<&'a Path>,
}
impl Selections<'_> {
    fn entries(&self) -> [(Scope, Option<&Path>); 4] {
        [
            (Scope::UserSettings, self.user_settings),
            (Scope::ProjectSettings, self.project_settings),
            (Scope::LocalSettings, self.local_settings),
            (Scope::ProjectMcp, self.project_mcp),
        ]
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionIssue {
    NothingSelected,
    InvalidPath,
    RepeatedPath,
}

/// Validate every selection before reading any file. Scope is a caller declaration;
/// no HOME, environment variable, CWD or directory inference occurs.
pub fn validate_selections(selections: &Selections<'_>) -> Result<(), SelectionIssue> {
    let mut seen = BTreeSet::new();
    for (scope, path) in selections.entries() {
        if let Some(path) = path {
            validate_path(path, scope).map_err(|_| SelectionIssue::InvalidPath)?;
            let normalized: std::path::PathBuf = path.components().collect();
            #[cfg(windows)]
            let normalized = normalized.to_string_lossy().to_lowercase();
            if !seen.insert(normalized) {
                return Err(SelectionIssue::RepeatedPath);
            }
        }
    }
    if seen.is_empty() {
        return Err(SelectionIssue::NothingSelected);
    }
    Ok(())
}
pub fn validate_path(path: &Path, scope: Scope) -> Result<(), InspectionIssue> {
    reader::validate_path(path, scope.filename())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    NotSelected,
    NotRead,
    Read,
    Missing,
    Unsupported,
    Unreadable,
}
#[derive(Debug, Serialize)]
pub struct ScopeObservation {
    pub scope: Scope,
    pub status: Status,
    pub issue: Option<InspectionIssue>,
    pub summary: Option<ScopeSummary>,
}
/// Only fixed known keys and numeric counts, never server names or config values.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct ScopeSummary {
    pub mcp_server_count: usize,
    pub mcp_section_present: bool,
    pub permissions_present: bool,
    pub hooks_present: bool,
    pub model_present: bool,
    pub security_present: bool,
    pub other_top_level_fields_present: bool,
}
#[derive(Debug, Serialize)]
pub struct QoderScopeReport {
    pub mode: &'static str,
    pub contract: &'static str,
    pub selection_issue: Option<SelectionIssue>,
    pub scopes: Vec<ScopeObservation>,
    /// None if any selected file was unavailable/unsupported. A repeated identifier
    /// is only a potential collision; trust/CLI/env/merge rules are not evaluated.
    pub repeated_mcp_identifier_count: Option<usize>,
    pub effective_configuration: &'static str,
    pub runtime_version: &'static str,
    pub can_apply: bool,
    pub can_restore: bool,
    pub limits: InspectionLimits,
    pub limitations: Vec<&'static str>,
}
pub fn inspect_scopes(selections: &Selections<'_>) -> QoderScopeReport {
    inspect_with(&NativeSelectedFileReader, selections)
}
fn inspect_with(reader: &impl SelectedFileReader, selections: &Selections<'_>) -> QoderScopeReport {
    let validation = validate_selections(selections);
    let mut report = QoderScopeReport {
        mode: "explicit_scope_summary",
        contract: "qoder_cli_documentation_2026_11_08",
        selection_issue: validation.err(),
        scopes: vec![],
        repeated_mcp_identifier_count: None,
        effective_configuration: "unknown",
        runtime_version: "unverified",
        can_apply: false,
        can_restore: false,
        limits: LIMITS,
        limitations: vec![
            "Scope and file association are caller declarations, not installation or runtime-version verification. Only explicitly selected files are read; no QODER_CONFIG_DIR or environment expansion, imports, trust lookup or other discovery.",
            "Strict JSON plus // line comments only. Unsupported syntax is not proof Qoder rejects a file. No migration or repair. Unknown fields are not interpreted.",
            "Only fixed section flags and counts are returned. Values, server identifiers, arbitrary keys, paths and raw errors are suppressed. Selected settings may contain secrets in memory.",
            "Repeated MCP identifiers indicate possible overlap only, not errors or effective precedence. Qoder's folder trust, environment, --settings and list merging can change effective configuration.",
            "No execution, writes, backup, authentication or restore. BYOK must use the official /model wizard; this inspector does not read dedicated credential files or determine account eligibility.",
            "Lexical path deduplication only; physical aliases can inflate overlap counts. Best-effort link/special-file/race checks, not a sandbox or atomic multi-file snapshot. Reads may change access times or block on filesystem I/O. Review documentation before any future write adapter.",
        ],
    };
    let mut seen = BTreeSet::new();
    let mut repeated = BTreeSet::new();
    let mut complete = report.selection_issue.is_none();
    for (scope, path) in selections.entries() {
        let mut observation = ScopeObservation {
            scope,
            status: Status::NotSelected,
            issue: None,
            summary: None,
        };
        if let Some(path) = path {
            if report.selection_issue.is_some() {
                observation.status = Status::NotRead;
            } else {
                let result = reader
                    .read_selected(path)
                    .and_then(|bytes| parse_jsonc(&bytes))
                    .and_then(summarize);
                match result {
                    Ok((summary, identifiers)) => {
                        observation.status = Status::Read;
                        observation.summary = Some(summary);
                        for id in identifiers {
                            if !seen.insert(id.clone()) {
                                repeated.insert(id);
                            }
                        }
                    }
                    Err(issue) => {
                        complete = false;
                        observation.status = match issue {
                            InspectionIssue::Missing => Status::Missing,
                            InspectionIssue::PermissionDenied
                            | InspectionIssue::IoFailure
                            | InspectionIssue::FileChanged => Status::Unreadable,
                            _ => Status::Unsupported,
                        };
                        observation.issue = Some(issue);
                    }
                }
            }
        }
        report.scopes.push(observation);
    }
    if complete {
        report.repeated_mcp_identifier_count = Some(repeated.len());
    }
    report
}
fn summarize(value: Value) -> Result<(ScopeSummary, BTreeSet<String>), InspectionIssue> {
    let object = value.as_object().ok_or(InspectionIssue::UnsupportedShape)?;
    let mut identifiers = BTreeSet::new();
    if let Some(servers) = object.get("mcpServers") {
        let servers = servers
            .as_object()
            .ok_or(InspectionIssue::UnsupportedShape)?;
        for (name, server) in servers {
            if name.is_empty() || !server.is_object() {
                return Err(InspectionIssue::UnsupportedShape);
            }
            identifiers.insert(name.clone());
        }
    }
    let summary = ScopeSummary {
        mcp_server_count: identifiers.len(),
        mcp_section_present: object.contains_key("mcpServers"),
        permissions_present: object.contains_key("permissions"),
        hooks_present: object.contains_key("hooks"),
        model_present: object.contains_key("model"),
        security_present: object.contains_key("security"),
        other_top_level_fields_present: object.keys().any(|k| {
            !matches!(
                k.as_str(),
                "mcpServers" | "permissions" | "hooks" | "model" | "security"
            )
        }),
    };
    Ok((summary, identifiers))
}
/// Replace only // comments outside JSON strings with spaces. Preserve newlines
/// and string bytes; bounded strict parser rejects all remaining invalid syntax,
/// duplicate keys, trailing commas and unsupported block comments without echoing.
fn parse_jsonc(bytes: &[u8]) -> Result<Value, InspectionIssue> {
    if bytes.len() > LIMITS.max_manifest_bytes {
        return Err(InspectionIssue::FileTooLarge);
    }
    std::str::from_utf8(bytes).map_err(|_| InspectionIssue::UnsupportedJson)?;
    let mut filtered = bytes.to_vec();
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
        } else if b == b'"' {
            in_string = true;
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\r' && bytes[i] != b'\n' {
                filtered[i] = b' ';
                i += 1;
            }
            continue;
        }
        i += 1;
    }
    parse::parse_json(&filtered)
}
#[cfg(test)]
mod tests;
