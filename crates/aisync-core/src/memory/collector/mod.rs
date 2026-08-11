//! Memory collector adapters（M3）
//!
//! - codex：扫描 `~/.codex/sessions/**/*.jsonl`（response_item 格式）
//! - claude：扫描 `~/.claude/projects/**/*.jsonl`（message.content 格式）
//! - opencode：读取 `~/.local/share/opencode/opencode.db`（part.data 文本块）

use std::io::BufRead;
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
fn collect_jsonl_dir(
    store: &MemoryStore,
    agent_id: &str,
    root: &Path,
    extract: impl Fn(&Value) -> Option<String>,
) -> crate::Result<usize> {
    if !root.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let file = std::fs::File::open(path)?;
        for (idx, line) in std::io::BufReader::new(file).lines().enumerate() {
            let line = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            let value: Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(text) = extract(&value) else {
                continue;
            };
            let text = text.trim();
            if text.len() < 4 {
                continue;
            }
            let ts = value.get("timestamp").and_then(|t| t.as_i64()).unwrap_or(0) / 1000;
            let source = format!("{}:{}:{}", path.display(), idx, ts);
            if store
                .insert(agent_id, text, &source, ts, false, &[agent_id.to_string()])?
                .is_some()
            {
                count += 1;
            }
        }
    }
    Ok(count)
}

/// 从 Codex 会话 JSONL 采集
pub fn collect_from_codex(store: &MemoryStore) -> crate::Result<usize> {
    let root = dirs::home_dir()
        .unwrap_or_default()
        .join(".codex")
        .join("sessions");
    collect_jsonl_dir(store, "codex", &root, extract_codex_line)
}

/// 从 Claude Code 会话 JSONL 采集
pub fn collect_from_claude(store: &MemoryStore) -> crate::Result<usize> {
    let root = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("projects");
    collect_jsonl_dir(store, "claude-code", &root, extract_claude_line)
}

/// 从 OpenCode SQLite（opencode.db）采集文本 part
pub fn collect_from_opencode(store: &MemoryStore) -> crate::Result<usize> {
    let db_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db");
    collect_from_opencode_at(store, &db_path)
}

fn collect_from_opencode_at(store: &MemoryStore, db_path: &Path) -> crate::Result<usize> {
    if !db_path.exists() {
        return Ok(0);
    }
    let conn = Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut stmt =
        conn.prepare("SELECT id, session_id, message_id, time_created, data FROM part")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;

    let mut count = 0usize;
    for row in rows {
        let (part_id, session_id, message_id, time_created, data) = row?;
        let value: Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(_) => continue,
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
        let count = collect_jsonl_dir(&store, "codex", &sessions, extract_codex_line).unwrap();
        assert_eq!(count, 1, "只应采集用户消息");
        let entries = store.recent("codex", 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "帮我写一个排序算法");
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
