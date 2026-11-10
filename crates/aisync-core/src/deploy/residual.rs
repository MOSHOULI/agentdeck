//! Explicit Windows Codex slot metadata, not an installation or ownership check.
//! No recursion, file-content reads, PATH discovery, registry or process execution.
use serde::Serialize;
use std::path::Path;

pub const LIMITATIONS: &str = "Selected Windows Codex prefix slot metadata only. Not installation, authenticity, ownership, recovery or cleanup authorization. No recursion, file-content reads, execution or writes. Before/after observations are not an atomic snapshot; same-user races and filesystem I/O deadlines are not solved. Preserve existing data for manual review.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotState {
    Missing,
    File,
    Directory,
    Blocked,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotObservation {
    pub slot: &'static str,
    pub state: SlotState,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResidualObservation {
    pub schema_version: u8,
    pub agent_id: &'static str,
    pub slots: Vec<SlotObservation>,
    pub runtime_not_run: bool,
    pub ownership: &'static str,
    pub cleanup_supported: bool,
    pub limitations: &'static str,
}
#[derive(Debug, thiserror::Error)]
pub enum ResidualInspectionError {
    #[error("Selected Codex slot inspection is supported only on Windows")]
    Unsupported,
    #[error("Selected Codex prefix could not be safely inspected; no changes were made")]
    Unavailable,
    #[error("Selected Codex paths changed during inspection; no changes were made")]
    Changed,
}
type Result<T> = std::result::Result<T, ResidualInspectionError>;

/// Observe four fixed paths under an explicit existing fixed-local-disk prefix.
/// Exit success produces metadata, including Blocked, never readiness to mutate.
pub fn inspect_codex_residual_slots(prefix: &Path) -> Result<ResidualObservation> {
    #[cfg(windows)]
    {
        native::inspect(prefix)
    }
    #[cfg(not(windows))]
    {
        let _ = prefix;
        Err(ResidualInspectionError::Unsupported)
    }
}

#[cfg(any(windows, test))]
fn report(states: [SlotState; 4]) -> ResidualObservation {
    ResidualObservation {
        schema_version: 1,
        agent_id: "codex",
        slots: [
            "package_directory",
            "posix_launcher",
            "cmd_launcher",
            "powershell_launcher",
        ]
        .into_iter()
        .zip(states)
        .map(|(slot, state)| SlotObservation { slot, state })
        .collect(),
        runtime_not_run: true,
        ownership: "unknown",
        cleanup_supported: false,
        limitations: LIMITATIONS,
    }
}
#[cfg(windows)]
mod native;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codex_residual_contract_is_metadata_only() {
        let value = serde_json::to_value(report([
            SlotState::Missing,
            SlotState::File,
            SlotState::Directory,
            SlotState::Blocked,
        ]))
        .unwrap();
        assert_eq!(value["runtimeNotRun"], true);
        assert_eq!(value["cleanupSupported"], false);
        assert_eq!(value["ownership"], "unknown");
        assert_eq!(value["slots"][3]["state"], "blocked");
        assert_eq!(value.as_object().unwrap().len(), 7);
        for slot in value["slots"].as_array().unwrap() {
            assert_eq!(slot.as_object().unwrap().len(), 2);
        }
    }
    #[cfg(not(windows))]
    #[test]
    fn codex_residual_other_platforms_never_fall_back() {
        assert!(matches!(
            inspect_codex_residual_slots(Path::new("/not/read")),
            Err(ResidualInspectionError::Unsupported)
        ));
    }
}
