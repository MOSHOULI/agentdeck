//! Memory collector adapters（M3）
//!
//! - codex：扫描 `~/.codex/sessions/**/*.jsonl`（response_item 格式）
//! - claude：扫描 `~/.claude/projects/**/*.jsonl`（message.content 格式）
//! - opencode：读取 `~/.local/share/opencode/opencode.db`（part.data 文本块）

use crate::memory::CollectionControl;
use std::io::{BufRead, Read};
use std::path::Path;

use rusqlite::Connection;
use serde_json::Value;
use walkdir::WalkDir;

use crate::memory::storage::MemoryStore;

fn extract_content(content: &Value) -> Option<String> {
    match content {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(t.to_string());
                } else if let Some(s) = item.as_str() {
                    parts.push(s.to_string());
                }
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        _ => None,
    }
}

fn extract_codex_line(value: &Value) -> Option<String> {
    let payload = value.get("payload")?;
    if payload.get("type").and_then(|t| t.as_str()) != Some("message") {
        return None;
    }
    if payload.get("role").and_then(|r| r.as_str()) != Some("user") {
        return None;
    }
    extract_content(payload.get("content")?)
}

fn extract_claude_line(value: &Value) -> Option<String> {
    let message = value.get("message")?;
    if message.get("role").and_then(|r| r.as_str()) != Some("user") {
        return None;
    }
    extract_content(message.get("content")?)
}

/// 扫描目录下所有 .jsonl，逐行抽取用户文本写入记忆库
#[cfg(test)]
fn collect_jsonl_dir(
    store: &MemoryStore,
    agent_id: &str,
    root: &Path,
    extract: impl Fn(&Value) -> Option<String>,
) -> crate::Result<usize> {
    collect_jsonl_dir_controlled(
        store,
        agent_id,
        root,
        extract,
        &CollectionControl::default(),
    )
}

fn collect_jsonl_dir_controlled(
    store: &MemoryStore,
    agent_id: &str,
    root: &Path,
    extract: impl Fn(&Value) -> Option<String>,
    control: &CollectionControl,
) -> crate::Result<usize> {
    control.check()?;
    if !root.try_exists()? {
        return Ok(0);
    }
    let mut count = 0usize;
    let mut errors = Vec::new();
    let mut error_count = 0usize;
    let mut record_error = |message: String| {
        error_count += 1;
        if errors.len() < 8 {
            errors.push(message);
        }
    };
    for entry in WalkDir::new(root).follow_links(false) {
        control.check()?;
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                record_error(e.to_string());
                continue;
            }
        };
        let path = entry.path();
        if !entry.file_type().is_file()
            || path.extension().and_then(|e| e.to_str()) != Some("jsonl")
        {
            continue;
        }
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) => {
                record_error(format!("{}: {}", path.display(), e));
                continue;
            }
        };
        let mut reader = std::io::BufReader::new(file);
        let mut idx = 0usize;
        loop {
            control.check()?;
            // Bound allocation for damaged/untrusted session records (1 MiB per line).
            let mut line = String::new();
            match reader.by_ref().take(1_048_577).read_line(&mut line) {
                Ok(0) => break,
                Ok(_) if line.len() > 1_048_576 => {
                    record_error(format!(
                        "{}:{}: record exceeds 1 MiB",
                        path.display(),
                        idx + 1
                    ));
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    record_error(format!("{}:{}: {}", path.display(), idx + 1, e));
                    break;
                }
            }
            idx += 1;
            if line.trim().is_empty() {
                continue;
            }
            let value: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(e) => {
                    record_error(format!("{}:{}: invalid JSON: {}", path.display(), idx, e));
                    continue;
                }
            };
            let Some(text) = extract(&value) else {
                continue;
            };
            let text = text.trim();
            if text.len() < 4 {
                continue;
            }
            let ts = value
                .get("timestamp")
                .and_then(|t| {
                    t.as_i64().map(|n| n / 1000).or_else(|| {
                        t.as_str()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .map(|t| t.timestamp())
                    })
                })
                .unwrap_or(0);
            let source = format!("{}:{}:{}", path.display(), idx - 1, ts);
            if store
                .insert(agent_id, text, &source, ts, false, &[agent_id.to_string()])?
                .is_some()
            {
                count += 1;
            }
        }
    }
    if error_count > 0 {
        return Err(crate::Error::Other(format!(
            "{}: collected {} new memories, {} source errors: {}",
            agent_id,
            count,
            error_count,
            errors.join("; ")
        )));
    }
    Ok(count)
}

/// 从 Codex 会话 JSONL 采集
pub fn collect_from_codex(store: &MemoryStore) -> crate::Result<usize> {
    collect_from_codex_controlled(store, &CollectionControl::default())
}

pub(crate) fn collect_from_codex_controlled(
    store: &MemoryStore,
    control: &CollectionControl,
) -> crate::Result<usize> {
    let root = crate::home_dir()
        .ok_or_else(|| crate::Error::Other("Cannot locate home directory".into()))?
        .join(".codex")
        .join("sessions");
    collect_jsonl_dir_controlled(store, "codex", &root, extract_codex_line, control)
}

/// 从 Claude Code 会话 JSONL 采集
pub fn collect_from_claude(store: &MemoryStore) -> crate::Result<usize> {
    collect_from_claude_controlled(store, &CollectionControl::default())
}

pub(crate) fn collect_from_claude_controlled(
    store: &MemoryStore,
    control: &CollectionControl,
) -> crate::Result<usize> {
    let root = crate::home_dir()
        .ok_or_else(|| crate::Error::Other("Cannot locate home directory".into()))?
        .join(".claude")
        .join("projects");
    collect_jsonl_dir_controlled(store, "claude-code", &root, extract_claude_line, control)
}

/// 从 OpenCode SQLite（opencode.db）采集文本 part
pub fn collect_from_opencode(store: &MemoryStore) -> crate::Result<usize> {
    collect_from_opencode_controlled(store, &CollectionControl::default())
}

pub(crate) fn collect_from_opencode_controlled(
    store: &MemoryStore,
    control: &CollectionControl,
) -> crate::Result<usize> {
    let db_path = crate::home_dir()
        .ok_or_else(|| crate::Error::Other("Cannot locate home directory".into()))?
        .join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db");
    collect_from_opencode_at_controlled(store, &db_path, control)
}

#[cfg(test)]
fn collect_from_opencode_at(store: &MemoryStore, db_path: &Path) -> crate::Result<usize> {
    collect_from_opencode_at_controlled(store, db_path, &CollectionControl::default())
}

fn collect_from_opencode_at_controlled(
    store: &MemoryStore,
    db_path: &Path,
    control: &CollectionControl,
) -> crate::Result<usize> {
    control.check()?;
    if !db_path.try_exists()? {
        return Ok(0);
    }
    let conn = Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(std::time::Duration::from_secs(1))?;
    let mut stmt =
        conn.prepare("SELECT id, session_id, message_id, time_created, CASE WHEN length(CAST(data AS BLOB)) > 1048576 THEN NULL ELSE data END FROM part")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut count = 0usize;
    let mut malformed = 0usize;
    for row in rows {
        control.check()?;
        let (part_id, session_id, message_id, time_created, data) = row?;
        let Some(data) = data else {
            malformed += 1;
            continue;
        };
        let value: Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(_) => {
                malformed += 1;
                continue;
            }
        };
        if value.get("type").and_then(|t| t.as_str()) != Some("text") {
            continue;
        }
        let Some(text) = value.get("text").and_then(|t| t.as_str()) else {
            continue;
        };
        let text = text.trim();
        if text.len() < 4 || text.starts_with('/') {
            continue;
        }
        let source = format!("opencode:{}:{}", session_id, part_id);
        if store
            .insert(
                "opencode",
                text,
                &source,
                time_created / 1000,
                false,
                &["opencode".to_string()],
            )?
            .is_some()
        {
            count += 1;
        }
        let _ = message_id;
    }
    if malformed > 0 {
        return Err(crate::Error::Other(format!(
            "OpenCode: collected {} new memories, skipped {} invalid or oversized JSON records",
            count, malformed
        )));
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentdeck-collect-{}-{}", tag, nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn codex_jsonl_collects_user_messages_only() {
        let dir = temp_dir("codex");
        let sessions = dir.join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let file = sessions.join("a.jsonl");
        std::fs::write(
            &file,
            r#"{"timestamp":1700000000000,"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"帮我写一个排序算法"}]}}
{"timestamp":1700000001000,"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"这是回答"}]}}
{"timestamp":1700000002000,"type":"response_item","payload":{"type":"reasoning","summary":["推理"]}}
not json"#,
        )
        .unwrap();

        let db = dir.join("memory.db");
        let store = MemoryStore::open(&db).unwrap();
        let error = collect_jsonl_dir(&store, "codex", &sessions, extract_codex_line).unwrap_err();
        assert!(error.to_string().contains("collected 1 new memories"));
        assert!(error.to_string().contains("invalid JSON"));
        let entries = store.recent("codex", 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "帮我写一个排序算法");
    }

    #[test]
    fn rfc3339_timestamp_and_duplicate_collection() {
        let dir = temp_dir("timestamp");
        let root = dir.join("sessions");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.jsonl"), r#"{"timestamp":"2023-11-14T22:13:20Z","payload":{"type":"message","role":"user","content":"example memory"}}"#).unwrap();
        let store = MemoryStore::open(&dir.join("memory.db")).unwrap();
        assert_eq!(
            collect_jsonl_dir(&store, "codex", &root, extract_codex_line).unwrap(),
            1
        );
        assert_eq!(
            collect_jsonl_dir(&store, "codex", &root, extract_codex_line).unwrap(),
            0
        );
        assert_eq!(
            store.recent("codex", 1).unwrap()[0].collected_at,
            1700000000
        );
    }

    #[test]
    fn oversized_record_is_bounded_and_reported() {
        let dir = temp_dir("oversized");
        let root = dir.join("sessions");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("a.jsonl"), "x".repeat(1_048_577)).unwrap();
        let store = MemoryStore::open(&dir.join("memory.db")).unwrap();
        let error = collect_jsonl_dir(&store, "codex", &root, extract_codex_line).unwrap_err();
        assert!(error.to_string().contains("record exceeds 1 MiB"));
        assert_eq!(store.count().unwrap(), 0);
    }

    #[test]
    fn cancelled_collection_does_not_open_source() {
        let dir = temp_dir("cancelled");
        let store = MemoryStore::open(&dir.join("memory.db")).unwrap();
        let control = CollectionControl::new(
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            std::time::Duration::from_secs(30),
        );
        let error = collect_jsonl_dir_controlled(
            &store,
            "codex",
            &dir.join("missing"),
            extract_codex_line,
            &control,
        )
        .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
    }

    #[test]
    fn opencode_db_collects_text_parts() {
        let dir = temp_dir("opencode");
        let db_path = dir.join("opencode.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE part (
                id TEXT PRIMARY KEY,
                session_id TEXT,
                message_id TEXT,
                time_created INTEGER,
                data TEXT
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "p1",
                "s1",
                "m1",
                1700000000000i64,
                r#"{"type":"text","text":"opencode 会话里的用户提问"}"#
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "p2",
                "s1",
                "m1",
                1700000001000i64,
                r#"{"type":"reasoning","text":"思考过程不应采集"}"#
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "p3",
                "s1",
                "m1",
                1700000002000i64,
                r#"{"type":"text","text":"/connect deepseek"}"#
            ],
        )
        .unwrap();
        drop(conn);

        let store = MemoryStore::open(&dir.join("memory.db")).unwrap();
        let count = collect_from_opencode_at(&store, &db_path).unwrap();
        assert_eq!(count, 1, "只应采集 text part，跳过命令");
        let entries = store.recent("opencode", 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "opencode 会话里的用户提问");
    }
}
