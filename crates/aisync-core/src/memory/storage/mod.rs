//! Memory storage layer（SQLite + FTS5）

use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;

use crate::memory::MemoryEntry;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS memories (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  agent_id TEXT NOT NULL,
  content TEXT NOT NULL,
  collected_at INTEGER NOT NULL,
  source TEXT,
  auto_inject INTEGER NOT NULL DEFAULT 0,
  tags TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS manual_injections (
  agent_id TEXT NOT NULL,
  memory_id INTEGER NOT NULL,
  PRIMARY KEY (agent_id, memory_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_memories_dedup ON memories(source, content);
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
  content, tags, content='memories', content_rowid='id'
);
CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
  INSERT INTO memories_fts(rowid, content, tags) VALUES (new.id, new.content, new.tags);
END;
CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
  INSERT INTO memories_fts(memories_fts, rowid, content, tags)
  VALUES ('delete', old.id, old.content, old.tags);
END;
CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
  INSERT INTO memories_fts(memories_fts, rowid, content, tags)
  VALUES ('delete', old.id, old.content, old.tags);
  INSERT INTO memories_fts(rowid, content, tags) VALUES (new.id, new.content, new.tags);
END;
"#;

fn content_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(content.as_bytes());
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

fn build_fts_query(query: &str) -> String {
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect();
    if tokens.is_empty() {
        "*".into()
    } else {
        tokens.join(" ")
    }
}

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<MemoryEntry> {
    let id: i64 = row.get(0)?;
    let agent_id: String = row.get(1)?;
    let content: String = row.get(2)?;
    let collected_at: i64 = row.get(3)?;
    let source: Option<String> = row.get(4)?;
    let auto_inject: bool = row.get(5)?;
    let tags: String = row.get(6)?;
    let hash = content_hash(&content);
    Ok(MemoryEntry {
        id: id.to_string(),
        agent_id,
        content_hash: hash,
        content,
        collected_at,
        source_session_id: source,
        auto_inject,
        tags: tags
            .split(',')
            .filter(|t| !t.is_empty())
            .map(ToString::to_string)
            .collect(),
    })
}

pub struct MemoryStore {
    conn: Connection,
}

impl MemoryStore {
    pub fn open(path: &Path) -> crate::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// 插入一条记忆；source+content 已存在时返回 None
    pub fn insert(
        &self,
        agent_id: &str,
        content: &str,
        source: &str,
        collected_at: i64,
        auto_inject: bool,
        tags: &[String],
    ) -> crate::Result<Option<i64>> {
        let tags_str = tags.join(",");
        self.conn.execute(
            "INSERT OR IGNORE INTO memories (agent_id, content, collected_at, source, auto_inject, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![agent_id, content, collected_at, source, auto_inject, tags_str],
        )?;
        if self.conn.changes() == 0 {
            return Ok(None);
        }
        Ok(Some(self.conn.last_insert_rowid()))
    }

    pub fn search(&self, query: &str, limit: usize) -> crate::Result<Vec<MemoryEntry>> {
        if query.trim().is_empty() {
            let mut stmt = self.conn.prepare(
                "SELECT id, agent_id, content, collected_at, source, auto_inject, tags
                 FROM memories
                 ORDER BY collected_at DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit as i64], row_to_entry)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            return Ok(out);
        }
        let fts_query = build_fts_query(query);
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, content, collected_at, source, auto_inject, tags
             FROM memories
             WHERE id IN (SELECT rowid FROM memories_fts WHERE memories_fts MATCH ?1)
             ORDER BY collected_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![fts_query, limit as i64], row_to_entry);
        match rows {
            Ok(rows) => {
                let mut out = Vec::new();
                for row in rows {
                    out.push(row?);
                }
                Ok(out)
            }
            // FTS5 词法异常时降级为 LIKE 搜索
            Err(_) => self.search_like(query, limit),
        }
    }

    fn search_like(&self, query: &str, limit: usize) -> crate::Result<Vec<MemoryEntry>> {
        let pattern = format!("%{}%", query.trim());
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, content, collected_at, source, auto_inject, tags
             FROM memories
             WHERE content LIKE ?1 OR tags LIKE ?1
             ORDER BY collected_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], row_to_entry)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn recent(&self, agent_id: &str, limit: usize) -> crate::Result<Vec<MemoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, content, collected_at, source, auto_inject, tags
             FROM memories WHERE agent_id = ?1
             ORDER BY collected_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![agent_id, limit as i64], row_to_entry)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// 某 agent 全部 auto_inject 记忆（定时采集后回写记忆文件用）
    pub fn auto_inject_entries(&self, agent_id: &str) -> crate::Result<Vec<MemoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, content, collected_at, source, auto_inject, tags
             FROM memories
             WHERE agent_id = ?1 AND auto_inject = 1
             ORDER BY collected_at DESC",
        )?;
        let rows = stmt.query_map(params![agent_id], row_to_entry)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_by_ids(&self, ids: &[i64]) -> crate::Result<Vec<MemoryEntry>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let sql = format!(
            "SELECT id, agent_id, content, collected_at, source, auto_inject, tags
             FROM memories WHERE id IN ({})",
            placeholders
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params = rusqlite::params_from_iter(ids.iter());
        let rows = stmt.query_map(params, row_to_entry)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn manual_injection_ids(&self, agent_id: &str) -> crate::Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT memory_id FROM manual_injections WHERE agent_id = ?1")?;
        let rows = stmt.query_map(params![agent_id], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_manual_injection_ids(&self, agent_id: &str, ids: &[i64]) -> crate::Result<()> {
        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM manual_injections WHERE agent_id = ?1",
            params![agent_id],
        )?;
        for id in ids {
            transaction.execute(
                "INSERT OR IGNORE INTO manual_injections (agent_id, memory_id) VALUES (?1, ?2)",
                params![agent_id, id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Explicit manual selections survive automatic refreshes. Missing/deleted
    /// rows naturally disappear, while automatic revocations remove only auto selections.
    pub fn injection_entries(&self, agent_id: &str) -> crate::Result<Vec<MemoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, agent_id, content, collected_at, source, auto_inject, tags FROM memories
             WHERE (agent_id = ?1 AND auto_inject = 1) OR id IN
             (SELECT memory_id FROM manual_injections WHERE agent_id = ?1)
             ORDER BY collected_at DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![agent_id], row_to_entry)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn set_auto_inject(&self, id: i64, enabled: bool) -> crate::Result<()> {
        let changed = self.conn.execute(
            "UPDATE memories SET auto_inject = ?1 WHERE id = ?2",
            params![enabled, id],
        )?;
        if changed == 0 {
            return Err(crate::Error::Other(format!("Memory {} does not exist", id)));
        }
        Ok(())
    }

    pub fn count(&self) -> crate::Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
            .optional()?
            .unwrap_or(0))
    }
}
