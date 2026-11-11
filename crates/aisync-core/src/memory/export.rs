//! Bounded review-and-replace editor for existing local memory exports.
//!
//! Fingerprints detect changed reviewed state; they are not authorization tokens.
//! INJECTION_LOCK serializes supported in-process writers only. SQLite and the
//! Markdown file are not one atomic transaction, nor protected against external writers.

use super::{MemoryEntry, MemoryStore};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

pub const MAX_MANUAL_ROWS: usize = 200;
pub const MAX_EXPORT_ROWS: usize = 500;
pub const MAX_CONTENT_BYTES: usize = 64 * 1024;
pub const MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportError {
    pub code: &'static str,
    pub message: String,
    pub refresh_required: bool,
}
impl ExportError {
    fn new(code: &'static str, message: impl ToString) -> Self {
        Self {
            code,
            message: message.to_string(),
            refresh_required: true,
        }
    }
}
impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ExportError {}
pub type ExportResult<T> = Result<T, ExportError>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRow {
    pub id: String,
    pub agent_id: String,
    pub content: String,
    pub content_hash: String,
    pub collected_at: i64,
    pub auto_inject: bool,
    pub tags: Vec<String>,
}
impl From<&MemoryEntry> for ExportRow {
    fn from(row: &MemoryEntry) -> Self {
        Self {
            id: row.id.clone(),
            agent_id: row.agent_id.clone(),
            content: row.content.clone(),
            content_hash: row.content_hash.clone(),
            collected_at: row.collected_at,
            auto_inject: row.auto_inject,
            tags: row.tags.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSnapshot {
    pub agent_id: String,
    pub database_exists: bool,
    pub state_token: String,
    pub manual_entries: Vec<ExportRow>,
    pub automatic_entries: Vec<ExportRow>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreview {
    pub agent_id: String,
    pub state_token: String,
    pub preview_token: String,
    pub manual_entries: Vec<ExportRow>,
    pub automatic_entries: Vec<ExportRow>,
    pub output_entries: Vec<ExportRow>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReceipt {
    pub agent_id: String,
    pub manual_ids: Vec<String>,
    pub exported_count: usize,
}
pub(crate) struct SelectionData {
    pub manual: Vec<MemoryEntry>,
    pub automatic: Vec<MemoryEntry>,
    pub proposed: Vec<MemoryEntry>,
}
fn rows(entries: &[MemoryEntry]) -> Vec<ExportRow> {
    entries.iter().map(ExportRow::from).collect()
}
fn hash(value: &impl Serialize) -> String {
    // All callers use finite, serializable concrete DTOs.
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("export DTO serializes"))
    )
}
fn state_token(agent: &str, exists: bool, data: &SelectionData) -> String {
    hash(&(
        "memory-export-state-v1",
        agent,
        exists,
        rows(&data.manual),
        rows(&data.automatic),
    ))
}
fn preview_token(agent: &str, state: &str, data: &SelectionData) -> String {
    hash(&(
        "memory-export-preview-v1",
        agent,
        state,
        rows(&data.proposed),
    ))
}
fn union(data: &SelectionData) -> Vec<MemoryEntry> {
    let mut by_id = BTreeMap::new();
    for row in data.proposed.iter().chain(&data.automatic) {
        by_id.insert(row.id.clone(), row.clone());
    }
    let mut entries: Vec<_> = by_id.into_values().collect();
    entries.sort_by(|a, b| {
        b.collected_at.cmp(&a.collected_at).then_with(|| {
            b.id.parse::<i64>()
                .unwrap()
                .cmp(&a.id.parse::<i64>().unwrap())
        })
    });
    entries
}
fn target(agent: &str) -> ExportResult<String> {
    if !matches!(agent, "codex" | "claude-code" | "opencode") {
        return Err(ExportError::new(
            "invalid",
            "Unsupported memory editor target",
        ));
    }
    let registry = crate::registry::load_registry_read_only()
        .map_err(|e| ExportError::new("read_failed", e))?;
    super::validate_injection_target_in(agent, &registry)
        .map_err(|e| ExportError::new("invalid", e))
}
fn ids(memory_ids: &[String]) -> ExportResult<Vec<i64>> {
    if memory_ids.len() > MAX_MANUAL_ROWS {
        return Err(ExportError::new(
            "invalid",
            "At most 200 manual selections are supported",
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for value in memory_ids {
        let id = value
            .parse::<i64>()
            .map_err(|_| ExportError::new("invalid", "Invalid memory ID"))?;
        if id <= 0 || id.to_string() != *value || !ids.insert(id) {
            return Err(ExportError::new(
                "invalid",
                "IDs must be unique positive canonical integers",
            ));
        }
    }
    Ok(ids.into_iter().collect())
}
fn validate_token(token: &str) -> ExportResult<()> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ExportError::new(
            "invalid",
            "Invalid review token; refresh and review",
        ));
    }
    Ok(())
}
fn read_at(db: &Path, agent: &str, proposed: &[i64]) -> ExportResult<(bool, SelectionData)> {
    let store =
        MemoryStore::open_existing_read_only(db).map_err(|e| ExportError::new("read_failed", e))?;
    match store {
        Some(store) => Ok((
            true,
            store
                .export_selection(agent, proposed)
                .map_err(|e| ExportError::new("read_failed", e))?,
        )),
        None if proposed.is_empty() => Ok((
            false,
            SelectionData {
                manual: vec![],
                automatic: vec![],
                proposed: vec![],
            },
        )),
        None => Err(ExportError::new(
            "read_failed",
            "Memory database is missing; refresh and review",
        )),
    }
}

pub fn read(agent: &str) -> ExportResult<ExportSnapshot> {
    let agent = target(agent)?;
    let db = super::default_db_path().map_err(|e| ExportError::new("read_failed", e))?;
    snapshot_at(&db, &agent)
}
fn snapshot_at(db: &Path, agent: &str) -> ExportResult<ExportSnapshot> {
    let _guard = super::INJECTION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (exists, data) = read_at(db, agent, &[])?;
    Ok(ExportSnapshot {
        agent_id: agent.into(),
        database_exists: exists,
        state_token: state_token(agent, exists, &data),
        manual_entries: rows(&data.manual),
        automatic_entries: rows(&data.automatic),
    })
}
pub fn preview(
    agent: &str,
    expected_state_token: &str,
    memory_ids: &[String],
) -> ExportResult<ExportPreview> {
    let agent = target(agent)?;
    let db = super::default_db_path().map_err(|e| ExportError::new("read_failed", e))?;
    preview_at(&db, &agent, expected_state_token, memory_ids)
}
fn preview_at(
    db: &Path,
    agent: &str,
    expected: &str,
    memory_ids: &[String],
) -> ExportResult<ExportPreview> {
    validate_token(expected)?;
    let ids = ids(memory_ids)?;
    let _guard = super::INJECTION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (exists, data) = read_at(db, agent, &ids)?;
    let state = state_token(agent, exists, &data);
    if state != expected {
        return Err(ExportError::new(
            "stale",
            "Saved manual or automatic memories changed; refresh and review again",
        ));
    }
    Ok(ExportPreview {
        agent_id: agent.into(),
        preview_token: preview_token(agent, &state, &data),
        state_token: state,
        manual_entries: rows(&data.proposed),
        automatic_entries: rows(&data.automatic),
        output_entries: rows(&union(&data)),
    })
}
pub fn replace(
    agent: &str,
    reviewed_token: &str,
    memory_ids: &[String],
) -> ExportResult<ExportReceipt> {
    let agent = target(agent)?;
    let db = super::default_db_path().map_err(|e| ExportError::new("read_failed", e))?;
    let dir = db
        .parent()
        .ok_or_else(|| ExportError::new("invalid", "Invalid database path"))?
        .join("memory");
    super::storage::check_file_path(&dir.join(format!("{agent}.md")))
        .map_err(|e| ExportError::new("invalid", e))?;
    replace_at(&db, &agent, reviewed_token, memory_ids, |entries| {
        super::write_memory_file_at(&dir, &agent, entries)
    })
}
fn replace_at(
    db: &Path,
    agent: &str,
    reviewed: &str,
    memory_ids: &[String],
    write: impl FnOnce(&[MemoryEntry]) -> crate::Result<()>,
) -> ExportResult<ExportReceipt> {
    validate_token(reviewed)?;
    let ids = ids(memory_ids)?;
    let _guard = super::INJECTION_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let (exists, data) = read_at(db, agent, &ids)?;
    let state = state_token(agent, exists, &data);
    if preview_token(agent, &state, &data) != reviewed {
        return Err(ExportError::new(
            "stale",
            "Reviewed memory export changed; refresh and review again",
        ));
    }
    // An absent DB is never created, even when confirming an empty selection.
    if !exists {
        return Err(ExportError::new(
            "read_failed",
            "Memory database is missing; nothing was written",
        ));
    }
    let store =
        MemoryStore::open_existing(db, false).map_err(|e| ExportError::new("read_failed", e))?;
    let previous: Vec<i64> = data.manual.iter().map(|r| r.id.parse().unwrap()).collect();
    // The replacement transaction is DB-first, followed by file replacement.
    // On write error compensate the DB; report both errors if compensation fails.
    store.set_manual_injection_ids(agent, &ids).map_err(|e| {
        ExportError::new(
            "write_failed_uncertain",
            format!("Database replacement failed: {e}; refresh and inspect before retrying"),
        )
    })?;
    let entries = union(&data);
    if let Err(error) = write(&entries) {
        return Err(match store.set_manual_injection_ids(agent, &previous) {
            Ok(()) => ExportError::new(
                "write_failed_rolled_back",
                format!(
                    "Markdown replacement failed: {error}; previous manual selection restored in database. Refresh and review before retrying"
                ),
            ),
            Err(rollback) => ExportError::new(
                "write_failed_uncertain",
                format!(
                    "Markdown replacement failed: {error}; database rollback also failed: {rollback}. Outcome uncertain; refresh and inspect before retrying"
                ),
            ),
        });
    }
    Ok(ExportReceipt {
        agent_id: agent.into(),
        manual_ids: ids.iter().map(ToString::to_string).collect(),
        exported_count: entries.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::path::PathBuf;

    struct Fixture {
        db: PathBuf,
        dir: PathBuf,
        store: MemoryStore,
    }
    impl Fixture {
        fn new() -> Self {
            let dir =
                std::env::temp_dir().join(format!("agentdeck-export-{}", uuid::Uuid::new_v4()));
            let db = dir.join("memory.db");
            let store = MemoryStore::open(&db).unwrap();
            store
                .insert(
                    "opencode",
                    "old selected outside search",
                    "private/path/one",
                    1,
                    false,
                    &["跨来源".into()],
                )
                .unwrap();
            store
                .insert("codex", "overlap", "private/path/two", 2, true, &[])
                .unwrap();
            store
                .insert("codex", "automatic", "private/path/three", 3, true, &[])
                .unwrap();
            store
                .insert("claude-code", "proposed", "private/path/four", 4, true, &[])
                .unwrap();
            store.set_manual_injection_ids("codex", &[1, 2]).unwrap();
            store.set_manual_injection_ids("opencode", &[4]).unwrap();
            Self { db, dir, store }
        }
        fn preview(&self, ids: &[String]) -> ExportPreview {
            let snapshot = snapshot_at(&self.db, "codex").unwrap();
            preview_at(&self.db, "codex", &snapshot.state_token, ids).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
    fn selected(ids: &[i64]) -> Vec<String> {
        ids.iter().map(ToString::to_string).collect()
    }
    fn row_ids(rows: &[ExportRow]) -> Vec<String> {
        rows.iter().map(|r| r.id.clone()).collect()
    }

    #[test]
    fn snapshot_retains_outside_search_and_omits_source_paths() {
        let f = Fixture::new();
        for id in 5..65 {
            f.store
                .insert(
                    "codex",
                    &format!("new row {id}"),
                    &format!("s{id}"),
                    id,
                    false,
                    &[],
                )
                .unwrap();
        }
        assert!(!f.store.search("", 50).unwrap().iter().any(|r| r.id == "1"));
        let state = snapshot_at(&f.db, "codex").unwrap();
        assert_eq!(row_ids(&state.manual_entries), selected(&[2, 1]));
        assert_eq!(row_ids(&state.automatic_entries), selected(&[3, 2]));
        let json = serde_json::to_string(&state).unwrap();
        assert!(!json.contains("sourceSessionId"));
        assert!(!json.contains("private/path"));
    }

    #[test]
    fn replacement_union_and_clear_preserve_automatic_and_other_targets() {
        let f = Fixture::new();
        let ids = selected(&[4, 2, 1]);
        let preview = f.preview(&ids);
        assert_eq!(row_ids(&preview.output_entries), selected(&[4, 3, 2, 1]));
        let dir = f.dir.join("memory");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("opencode.md"), "other target unchanged").unwrap();
        let receipt = replace_at(&f.db, "codex", &preview.preview_token, &ids, |rows| {
            super::super::write_memory_file_at(&dir, "codex", rows)
        })
        .unwrap();
        assert_eq!(receipt.exported_count, 4);
        assert_eq!(receipt.manual_ids, selected(&[1, 2, 4]));
        assert_eq!(
            std::fs::read_to_string(dir.join("codex.md"))
                .unwrap()
                .matches("overlap")
                .count(),
            1
        );
        let preview = f.preview(&[]);
        let receipt = replace_at(&f.db, "codex", &preview.preview_token, &[], |rows| {
            super::super::write_memory_file_at(&dir, "codex", rows)
        })
        .unwrap();
        assert_eq!(receipt.exported_count, 2);
        let file = std::fs::read_to_string(dir.join("codex.md")).unwrap();
        assert!(file.contains("automatic") && file.contains("overlap"));
        assert!(!file.contains("proposed") && !file.contains("outside search"));
        assert_eq!(f.store.manual_injection_ids("opencode").unwrap(), [4]);
        assert_eq!(
            std::fs::read_to_string(dir.join("opencode.md")).unwrap(),
            "other target unchanged"
        );
        assert_eq!(f.store.count().unwrap(), 4);
        assert_eq!(f.store.auto_inject_entries("codex").unwrap().len(), 2);
    }

    #[test]
    fn invalid_ids_targets_and_missing_rows_never_write() {
        let f = Fixture::new();
        for ids in [
            vec!["0".into()],
            vec!["01".into()],
            vec!["-1".into()],
            vec!["1".into(), "1".into()],
            vec!["9223372036854775808".into()],
            vec!["1".into(); 201],
        ] {
            assert_eq!(
                preview_at(&f.db, "codex", &"a".repeat(64), &ids)
                    .unwrap_err()
                    .code,
                "invalid"
            );
        }
        for agent in ["claude", "gemini", "qwen", "../codex", ""] {
            assert!(target(agent).is_err());
        }
        let snapshot = snapshot_at(&f.db, "codex").unwrap();
        assert_eq!(
            preview_at(&f.db, "codex", &snapshot.state_token, &selected(&[999]))
                .unwrap_err()
                .code,
            "read_failed"
        );
        assert!(!f.dir.join("memory").exists());
        assert_eq!(f.store.manual_injection_ids("codex").unwrap(), [1, 2]);
    }

    #[test]
    fn stale_manual_auto_and_relevant_content_require_new_review() {
        for update in [
            "INSERT INTO manual_injections VALUES ('codex',4)",
            "UPDATE memories SET auto_inject=0 WHERE id=3",
            "UPDATE memories SET content='changed selected' WHERE id=1",
            "UPDATE memories SET tags='changed' WHERE id=2",
            "UPDATE memories SET content='changed proposed' WHERE id=4",
            "UPDATE memories SET collected_at=99 WHERE id=4",
        ] {
            let f = Fixture::new();
            let ids = selected(&[4]);
            let preview = f.preview(&ids);
            Connection::open(&f.db)
                .unwrap()
                .execute_batch(update)
                .unwrap();
            let result = replace_at(&f.db, "codex", &preview.preview_token, &ids, |_| {
                panic!("stale state wrote")
            });
            assert_eq!(result.unwrap_err().code, "stale", "{update}");
        }
        let f = Fixture::new();
        let snapshot = snapshot_at(&f.db, "codex").unwrap();
        f.store.set_manual_injection_ids("codex", &[4]).unwrap();
        assert_eq!(
            preview_at(&f.db, "codex", &snapshot.state_token, &[])
                .unwrap_err()
                .code,
            "stale"
        );
        let preview = f.preview(&selected(&[1]));
        assert_eq!(
            replace_at(
                &f.db,
                "opencode",
                &preview.preview_token,
                &selected(&[1]),
                |_| panic!("wrong target wrote")
            )
            .unwrap_err()
            .code,
            "stale"
        );
        assert_eq!(
            replace_at(
                &f.db,
                "codex",
                &preview.preview_token,
                &selected(&[2]),
                |_| panic!("changed draft wrote")
            )
            .unwrap_err()
            .code,
            "stale"
        );
    }

    #[test]
    fn missing_database_inspection_creates_nothing_and_wal_commits_are_visible() {
        let dir =
            std::env::temp_dir().join(format!("agentdeck-export-missing-{}", uuid::Uuid::new_v4()));
        let db = dir.join("nested/memory.db");
        let state = snapshot_at(&db, "codex").unwrap();
        assert!(!state.database_exists);
        let preview = preview_at(&db, "codex", &state.state_token, &[]).unwrap();
        assert_eq!(
            replace_at(&db, "codex", &preview.preview_token, &[], |_| panic!(
                "missing database wrote"
            ))
            .unwrap_err()
            .code,
            "read_failed"
        );
        assert!(!dir.exists());
        let f = Fixture::new();
        let writer = Connection::open(&f.db).unwrap();
        writer.execute_batch("PRAGMA wal_autocheckpoint=0; UPDATE memories SET content='committed in WAL' WHERE id=1;").unwrap();
        assert!(
            std::fs::metadata(f.db.with_extension("db-wal"))
                .unwrap()
                .len()
                > 0
        );
        assert!(
            snapshot_at(&f.db, "codex")
                .unwrap()
                .manual_entries
                .iter()
                .any(|r| r.content == "committed in WAL")
        );
        writer
            .execute_batch("BEGIN; UPDATE memories SET content='uncommitted' WHERE id=1;")
            .unwrap();
        assert!(
            !snapshot_at(&f.db, "codex")
                .unwrap()
                .manual_entries
                .iter()
                .any(|r| r.content == "uncommitted")
        );
        writer.execute_batch("ROLLBACK;").unwrap();
    }

    #[test]
    fn write_failure_restores_db_and_failed_compensation_preserves_both_errors() {
        let f = Fixture::new();
        let ids = selected(&[4]);
        let preview = f.preview(&ids);
        let error = replace_at(&f.db, "codex", &preview.preview_token, &ids, |_| {
            Err(crate::Error::Other("synthetic original failure".into()))
        })
        .unwrap_err();
        assert_eq!(error.code, "write_failed_rolled_back");
        assert!(error.message.contains("synthetic original failure"));
        assert_eq!(f.store.manual_injection_ids("codex").unwrap(), [1, 2]);
        let preview = f.preview(&ids);
        let error = replace_at(&f.db, "codex", &preview.preview_token, &ids, |_| {
            Connection::open(&f.db).unwrap().execute_batch("CREATE TRIGGER fail_rollback BEFORE DELETE ON manual_injections BEGIN SELECT RAISE(ABORT, 'synthetic rollback failure'); END;").unwrap();
            Err(crate::Error::Other("synthetic original failure".into()))
        }).unwrap_err();
        assert_eq!(error.code, "write_failed_uncertain");
        assert!(
            error.message.contains("synthetic original failure")
                && error.message.contains("synthetic rollback failure")
        );
        assert_eq!(f.store.manual_injection_ids("codex").unwrap(), [4]);
    }

    #[test]
    fn limits_fail_instead_of_truncating_saved_or_proposed_selections() {
        let f = Fixture::new();
        Connection::open(&f.db)
            .unwrap()
            .execute(
                "UPDATE memories SET content=?1 WHERE id=1",
                ["a".repeat(MAX_CONTENT_BYTES)],
            )
            .unwrap();
        assert!(snapshot_at(&f.db, "codex").is_ok());
        Connection::open(&f.db)
            .unwrap()
            .execute(
                "UPDATE memories SET content=?1 WHERE id=1",
                ["a".repeat(MAX_CONTENT_BYTES + 1)],
            )
            .unwrap();
        assert!(snapshot_at(&f.db, "codex").is_err());
        Connection::open(&f.db)
            .unwrap()
            .execute(
                "UPDATE memories SET content='small', tags=?1 WHERE id=1",
                ["\\".repeat(MAX_SNAPSHOT_BYTES / 2)],
            )
            .unwrap();
        assert!(
            snapshot_at(&f.db, "codex").is_err(),
            "serialized DTO bytes, including escapes, are bounded"
        );
        let f = Fixture::new();
        f.store
            .set_manual_injection_ids("codex", &(1..=201).collect::<Vec<_>>())
            .unwrap();
        assert!(
            snapshot_at(&f.db, "codex")
                .unwrap_err()
                .message
                .contains("row limit")
        );
        let f = Fixture::new();
        for i in 5..504 {
            f.store
                .insert("codex", &format!("row{i}"), &format!("s{i}"), i, true, &[])
                .unwrap();
        }
        assert!(
            snapshot_at(&f.db, "codex")
                .unwrap_err()
                .message
                .contains("row limit")
        );
    }

    #[test]
    fn concurrent_checked_replacements_cannot_both_commit_stale_reviews() {
        let f = Fixture::new();
        let first = f.preview(&selected(&[1]));
        let second = f.preview(&selected(&[4]));
        let dir = f.dir.join("memory");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = [(first, selected(&[1])), (second, selected(&[4]))]
            .into_iter()
            .map(|(preview, ids)| {
                let db = f.db.clone();
                let dir = dir.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    replace_at(&db, "codex", &preview.preview_token, &ids, |rows| {
                        super::super::write_memory_file_at(&dir, "codex", rows)
                    })
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|r| r.as_ref().is_err_and(|e| e.code == "stale"))
                .count(),
            1
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("codex.md")).unwrap(),
            super::super::render_memory_file(&f.store.injection_entries("codex").unwrap())
        );
    }
    #[test]
    fn malformed_stored_metadata_fails_closed() {
        for update in [
            "UPDATE memories SET agent_id='/private/session/path' WHERE id=1",
            "UPDATE memories SET agent_id='' WHERE id=1",
            "UPDATE memories SET collected_at=9223372036854775807 WHERE id=1",
            "UPDATE memories SET auto_inject=2 WHERE id=1",
        ] {
            let f = Fixture::new();
            Connection::open(&f.db)
                .unwrap()
                .execute_batch(update)
                .unwrap();
            let error = snapshot_at(&f.db, "codex").unwrap_err();
            assert!(!error.message.contains("/private/session/path"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_parents_sidecars_and_nonregular_database_fail_closed() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        let linked_parent = f.dir.join("linked");
        symlink(&f.dir, &linked_parent).unwrap();
        assert!(snapshot_at(&linked_parent.join("memory.db"), "codex").is_err());
        let missing = f.dir.join("missing.db");
        let sidecar = f.dir.join("missing.db-wal");
        symlink(&f.db, &sidecar).unwrap();
        assert!(snapshot_at(&missing, "codex").is_err());
        assert!(!missing.exists());
        let hardlink = f.dir.join("hardlink.db");
        std::fs::hard_link(&f.db, &hardlink).unwrap();
        assert!(snapshot_at(&hardlink, "codex").is_err());
        assert!(snapshot_at(&f.dir, "codex").is_err());
    }
    #[cfg(any(unix, windows))]
    #[test]
    fn hardlinked_database_is_rejected_on_supported_desktops() {
        let f = Fixture::new();
        let hardlink = f.dir.join("hardlink.db");
        std::fs::hard_link(&f.db, &hardlink).unwrap();
        assert!(snapshot_at(&hardlink, "codex").is_err());
    }
}
