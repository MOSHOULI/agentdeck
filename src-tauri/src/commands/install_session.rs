//! Process-local inspection references. No paths or references are persisted.
use aisync_core::deploy::{InstallPreview, residual::ResidualObservation};
use serde::Serialize;
use std::sync::Mutex;

pub const UNAVAILABLE: &str = "本会话只读检查不可用，请重新检查安装记录。未清理或执行程序。";
pub static SESSION: Mutex<Option<Session>> = Mutex::new(None);
#[derive(PartialEq, Debug)]
pub enum Phase {
    Reviewed,
    Submitted,
    Failed,
}
pub struct Session {
    reference: String,
    revision: String,
    prefix: String,
    phase: Phase,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPreview {
    #[serde(flatten)]
    preview: InstallPreview,
    session_ref: Option<String>,
}
pub fn remember(slot: &mut Option<Session>, preview: InstallPreview) -> DesktopPreview {
    *slot = None;
    let session_ref = if cfg!(windows) && preview.agent_id == "codex" {
        let reference = uuid::Uuid::new_v4().to_string();
        *slot = Some(Session {
            reference: reference.clone(),
            revision: preview.revision.clone(),
            prefix: preview.prefix.clone(),
            phase: Phase::Reviewed,
        });
        Some(reference)
    } else {
        None
    };
    DesktopPreview {
        preview,
        session_ref,
    }
}
pub fn submitted(slot: &mut Option<Session>, id: &str, revision: &str, reference: Option<&str>) {
    if let Some(session) = slot.as_mut()
        && id == "codex"
        && session.phase == Phase::Reviewed
        && reference == Some(session.reference.as_str())
        && revision == session.revision
    {
        session.phase = Phase::Submitted;
        return;
    }
    *slot = None;
}
pub fn finished(slot: &mut Option<Session>, success: bool) {
    if !success
        && let Some(session) = slot.as_mut()
        && session.phase == Phase::Submitted
    {
        session.phase = Phase::Failed;
    } else {
        *slot = None;
    }
}
fn prefix_for_inspection(prefix: &str) -> &str {
    // Only the canonical DOS disk spelling is convertible. Core rejects all
    // ambiguous components again; UNC/device namespaces are not converted.
    if let Some(disk) = prefix.strip_prefix(r"\\?\") {
        let bytes = disk.as_bytes();
        if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1..3] == *b":\\" {
            return disk;
        }
    }
    prefix
}
pub fn inspect(slot: &Option<Session>, reference: &str) -> Result<ResidualObservation, String> {
    let session = slot
        .as_ref()
        .filter(|s| s.phase == Phase::Failed && s.reference == reference)
        .ok_or(UNAVAILABLE)?;
    aisync_core::deploy::residual::inspect_codex_residual_slots(std::path::Path::new(
        prefix_for_inspection(&session.prefix),
    ))
    .map_err(|_| UNAVAILABLE.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn record() -> Option<Session> {
        Some(Session {
            reference: "reference".into(),
            revision: "revision".into(),
            prefix: "C:\\private".into(),
            phase: Phase::Reviewed,
        })
    }
    #[test]
    fn only_exact_submitted_failure_is_eligible() {
        for (id, revision, reference) in [
            ("other", "revision", Some("reference")),
            ("codex", "wrong", Some("reference")),
            ("codex", "revision", None),
        ] {
            let mut slot = record();
            submitted(&mut slot, id, revision, reference);
            assert!(slot.is_none());
        }
        let mut slot = record();
        submitted(&mut slot, "codex", "revision", Some("reference"));
        assert_eq!(slot.as_ref().unwrap().phase, Phase::Submitted);
        finished(&mut slot, false);
        assert_eq!(slot.as_ref().unwrap().phase, Phase::Failed);
        assert_eq!(inspect(&slot, "wrong").unwrap_err(), UNAVAILABLE);
        submitted(&mut slot, "codex", "revision", Some("reference"));
        assert!(slot.is_none());
        let mut slot = record();
        submitted(&mut slot, "codex", "revision", Some("reference"));
        finished(&mut slot, true);
        assert!(slot.is_none());
    }
    #[test]
    fn verbatim_conversion_is_only_disk_prefix() {
        assert_eq!(prefix_for_inspection(r"\\?\C:\test"), r"C:\test");
        for value in [
            r"\\?\UNC\server\share",
            r"\\.\C:\test",
            r"\\?\é:\test",
            r"C:\test",
        ] {
            assert_eq!(prefix_for_inspection(value), value);
        }
    }
    fn preview() -> InstallPreview {
        InstallPreview {
            schema_version: 1,
            agent_id: "codex".into(),
            package: "@openai/codex".into(),
            executable: "codex".into(),
            manager_path: "C:\\npm".into(),
            node_path: "C:\\node.exe".into(),
            prefix: "C:\\prefix".into(),
            global_root: "C:\\prefix\\node_modules".into(),
            package_root: "C:\\prefix\\node_modules\\@openai\\codex".into(),
            launcher_directory: "C:\\prefix".into(),
            script_policy: "disabled".into(),
            version_policy: "npm_configured".into(),
            revision: "revision".into(),
        }
    }
    #[test]
    fn preview_never_grants_failed_install_inspection() {
        let mut slot = record();
        let result = remember(&mut slot, preview());
        if cfg!(windows) {
            let old_ref = result.session_ref.unwrap();
            assert_eq!(inspect(&slot, &old_ref).unwrap_err(), UNAVAILABLE);
            let next = remember(&mut slot, preview()).session_ref.unwrap();
            assert_ne!(old_ref, next); // same revision does not create an ABA reference
            submitted(&mut slot, "codex", "revision", Some(&old_ref));
            assert!(slot.is_none());
        } else {
            assert!(result.session_ref.is_none());
            assert!(slot.is_none());
        }
        let mut other = preview();
        other.agent_id = "opencode".into();
        assert!(remember(&mut slot, other).session_ref.is_none());
        assert!(slot.is_none());
    }
    #[test]
    fn contention_is_fail_closed() {
        let lock = Mutex::new(record());
        let _held = lock.lock().unwrap();
        assert!(lock.try_lock().is_err());
    }
}
