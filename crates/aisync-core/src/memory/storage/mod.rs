//! Memory storage layer（SQLite + FTS5）

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
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

/// Refuse linked/non-regular DB or output files and linked/non-directory parents.
/// These checks and SQLite NOFOLLOW reduce accidental redirection; they are not
/// a cross-process filesystem race defense.
pub(crate) fn check_file_path(path: &Path) -> crate::Result<()> {
    for (index, component) in path.ancestors().enumerate() {
        let meta = match std::fs::symlink_metadata(component) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        #[cfg(windows)]
        let linked = {
            use std::os::windows::fs::MetadataExt;
            meta.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let linked = meta.file_type().is_symlink();
        if linked || (index == 0 && !meta.is_file()) || (index != 0 && !meta.is_dir()) {
            return Err(crate::Error::Other(
                "Unsafe memory file or parent type; linked paths are not supported".into(),
            ));
        }
        #[cfg(windows)]
        if index == 0 {
            use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
            use windows_sys::Win32::Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
                FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, GetFileInformationByHandle,
            };
            let file = std::fs::OpenOptions::new()
                .access_mode(FILE_READ_ATTRIBUTES)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
                .open(component)?;
            // SAFETY: integer-only POD, and the file owns a live metadata handle.
            let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
                || info.nNumberOfLinks != 1
                || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)
                    != 0
            {
                return Err(crate::Error::Other(
                    "Unsafe or hard-linked memory file".into(),
                ));
            }
        }
        #[cfg(unix)]
        if index == 0 {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() > 1 {
                return Err(crate::Error::Other(
                    "Hard-linked memory files are not supported".into(),
                ));
            }
        }
    }
    Ok(())
}

fn check_database_paths(path: &Path) -> crate::Result<()> {
    check_file_path(path)?;
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        check_file_path(Path::new(&sidecar))?;
    }
    Ok(())
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

    /// Open only an existing database, without schema initialization or journal-mode changes.
    /// Read-only SQLite observes WAL commits and may use/create coordination sidecars;
    /// it does not modify database rows. In particular, a missing path creates nothing.
    pub fn open_existing_read_only(path: &Path) -> crate::Result<Option<Self>> {
        check_database_paths(path)?;
        match std::fs::metadata(path) {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        Self::open_existing(path, true).map(Some)
    }

    pub(crate) fn open_existing(path: &Path, read_only: bool) -> crate::Result<Self> {
        check_database_paths(path)?;
        let flags = if read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        let conn = Connection::open_with_flags(
            path,
            flags | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(Self { conn })
    }

    /// One consistent, bounded read of saved, automatic and proposed rows. Never
    /// select source-session paths. Check byte lengths before materializing text.
    pub(crate) fn export_selection(
        &self,
        agent_id: &str,
        proposed: &[i64],
    ) -> crate::Result<super::export::SelectionData> {
        use super::export::{
            MAX_CONTENT_BYTES, MAX_EXPORT_ROWS, MAX_MANUAL_ROWS, MAX_SNAPSHOT_BYTES, SelectionData,
        };
        let tx = self.conn.unchecked_transaction()?;
        let read_ids = |sql: &str, limit: usize| -> crate::Result<Vec<i64>> {
            let mut stmt = tx.prepare(sql)?;
            let ids = stmt
                .query_map(params![agent_id, (limit + 1) as i64], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<i64>>>()?;
            if ids.len() > limit {
                return Err(crate::Error::Other(
                    "Memory export exceeds selection row limit; nothing was truncated".into(),
                ));
            }
            Ok(ids)
        };
        let manual = read_ids(
            "SELECT memory_id FROM manual_injections WHERE agent_id = ?1 ORDER BY memory_id LIMIT ?2",
            MAX_MANUAL_ROWS,
        )?;
        let automatic = read_ids(
            "SELECT id FROM memories WHERE agent_id = ?1 AND auto_inject = 1 ORDER BY id LIMIT ?2",
            MAX_EXPORT_ROWS,
        )?;
        let old_union: std::collections::BTreeSet<_> =
            manual.iter().chain(&automatic).copied().collect();
        let new_union: std::collections::BTreeSet<_> =
            proposed.iter().chain(&automatic).copied().collect();
        if old_union.len() > MAX_EXPORT_ROWS || new_union.len() > MAX_EXPORT_ROWS {
            return Err(crate::Error::Other(
                "Memory export exceeds output row limit; nothing was truncated".into(),
            ));
        }
        let all: std::collections::BTreeSet<_> = old_union.union(&new_union).copied().collect();
        let mut rows = std::collections::BTreeMap::new();
        let mut bytes = 0usize;
        let mut dto_bytes = 0usize;
        for id in all {
            let lengths: Option<(i64, i64, i64, bool)> = tx.query_row(
                "SELECT length(CAST(content AS BLOB)), length(CAST(tags AS BLOB)), length(CAST(agent_id AS BLOB)), typeof(auto_inject) = 'integer' AND auto_inject IN (0, 1) FROM memories WHERE id = ?1",
                [id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            ).optional()?;
            let (content, tags, agent, valid_auto) = lengths.ok_or_else(|| {
                crate::Error::Other(format!(
                    "Selected memory {id} no longer exists; refresh and review"
                ))
            })?;
            if content < 0
                || content as usize > MAX_CONTENT_BYTES
                || tags < 0
                || agent < 0
                || !valid_auto
            {
                return Err(crate::Error::Other(
                    "Memory export exceeds row content limit".into(),
                ));
            }
            bytes = bytes
                .saturating_add(content as usize)
                .saturating_add(tags as usize)
                .saturating_add(agent as usize);
            if bytes > MAX_SNAPSHOT_BYTES {
                return Err(crate::Error::Other(
                    "Memory export exceeds snapshot byte limit; nothing was truncated".into(),
                ));
            }
            let row = tx.query_row(
                "SELECT id, agent_id, content, collected_at, NULL, auto_inject, tags FROM memories WHERE id = ?1", [id], row_to_entry)?;
            dto_bytes = dto_bytes
                .saturating_add(serde_json::to_vec(&super::export::ExportRow::from(&row))?.len());
            if dto_bytes > MAX_SNAPSHOT_BYTES {
                return Err(crate::Error::Other(
                    "Memory export exceeds snapshot DTO byte limit; nothing was truncated".into(),
                ));
            }
            if row.agent_id.is_empty()
                || row.agent_id.len() > 64
                || !row
                    .agent_id
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                || row.collected_at.unsigned_abs() > 9_007_199_254_740_991
            {
                return Err(crate::Error::Other("Invalid stored memory metadata".into()));
            }
            if id <= 0 {
                return Err(crate::Error::Other("Invalid stored memory ID".into()));
            }
            rows.insert(id, row);
        }
        let entries = |ids: &[i64]| {
            let mut entries: Vec<MemoryEntry> = ids.iter().map(|id| rows[id].clone()).collect();
            entries.sort_by(|a, b| {
                b.collected_at.cmp(&a.collected_at).then_with(|| {
                    b.id.parse::<i64>()
                        .unwrap()
                        .cmp(&a.id.parse::<i64>().unwrap())
                })
            });
            entries
        };
        let result = SelectionData {
            manual: entries(&manual),
            automatic: entries(&automatic),
            proposed: entries(proposed),
        };
        tx.commit()?;
        Ok(result)
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
