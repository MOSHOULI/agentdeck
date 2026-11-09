//! Dependency guard and real WAL/FTS compatibility checks, not an exploit proof.
use aisync_core::memory::storage::MemoryStore;
use rusqlite::{Connection, OpenFlags};

#[test]
fn bundled_sqlite_meets_rustsec_2026_0459_fixed_version() {
    assert!(rusqlite::version_number() >= 3_054_000);
    let conn = Connection::open_in_memory().unwrap();
    let sql_version: String = conn
        .query_row("SELECT sqlite_version()", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sql_version, rusqlite::version());
}

#[test]
fn memory_store_wal_readonly_and_reopen_preserve_fts_data() {
    let root = std::env::temp_dir().join(format!("agentdeck-sqlite-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("memory.db");
    {
        let store = MemoryStore::open(&path).unwrap();
        let tags = vec!["regression".to_owned()];
        store
            .insert(
                "test",
                "sqlite compatibility marker",
                "synthetic",
                1,
                false,
                &tags,
            )
            .unwrap();
        assert!(root.join("memory.db-wal").is_file());
        let reader = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let count: i64 = reader
            .query_row("SELECT COUNT(*) FROM memories", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert!(reader.execute("DELETE FROM memories", []).is_err());
        assert_eq!(store.search("compatibility", 10).unwrap().len(), 1);
    }
    {
        let reopened = MemoryStore::open(&path).unwrap();
        assert_eq!(reopened.count().unwrap(), 1);
        let entries = reopened.search("compatibility", 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "sqlite compatibility marker");
    }
    std::fs::remove_dir_all(root).unwrap();
}
