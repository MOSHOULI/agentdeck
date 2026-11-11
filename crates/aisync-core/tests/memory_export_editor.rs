//! Public API regression in a separate, explicitly isolated synthetic home.
use aisync_core::memory::{self, export, storage::MemoryStore};

#[test]
fn public_memory_export_editor_isolated() {
    if std::env::var_os("AGENTDECK_EXPORT_CHILD").is_none() {
        let root =
            std::env::temp_dir().join(format!("agentdeck-export-public-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "public_memory_export_editor_isolated",
                "--nocapture",
            ])
            .env("AGENTDECK_EXPORT_CHILD", "1")
            .env("AGENTDECK_TEST_HOME", &root)
            .env("HOME", &root)
            .env("USERPROFILE", &root)
            .status()
            .unwrap();
        let _ = std::fs::remove_dir_all(&root);
        assert!(status.success());
        return;
    }
    let home = aisync_core::home_dir().unwrap();
    assert!(!home.join(".agentdeck").exists());
    let state = export::read("codex").unwrap();
    assert!(!state.database_exists);
    let preview = export::preview("codex", &state.state_token, &[]).unwrap();
    assert!(export::replace("codex", &preview.preview_token, &[]).is_err());
    assert!(memory::search("").unwrap().is_empty());
    assert!(memory::recent("codex", 50).unwrap().is_empty());
    assert!(export::read("gemini").is_err());
    assert!(
        !home.join(".agentdeck").exists(),
        "public validation/read/preview must not create registry locks or database directories"
    );

    let db = memory::default_db_path().unwrap();
    let store = MemoryStore::open(&db).unwrap();
    store
        .insert("codex", "synthetic automatic", "authored-one", 1, true, &[])
        .unwrap();
    store
        .insert(
            "opencode",
            "synthetic manual",
            "authored-two",
            2,
            false,
            &[],
        )
        .unwrap();
    store
        .insert(
            "claude-code",
            "synthetic alternative",
            "authored-three",
            3,
            false,
            &[],
        )
        .unwrap();
    memory::inject("codex", &["2".into()]).unwrap();
    let state = export::read("codex").unwrap();
    let preview = export::preview("codex", &state.state_token, &["3".into()]).unwrap();
    // All supported memory writers participate in the same process-local lock.
    // Collection reads only this fresh isolated home's absent authored sources.
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(5));
    let mut handles = Vec::new();
    for mode in 0..5 {
        let barrier = barrier.clone();
        let token = preview.preview_token.clone();
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            match mode {
                0 => memory::inject("codex", &["2".into()]).unwrap(),
                1 => {
                    memory::inject_auto("codex").unwrap();
                }
                2 => memory::toggle_auto_inject("1", false).unwrap(),
                3 => {
                    memory::collect("codex").unwrap();
                }
                4 => {
                    if let Err(error) = export::replace("codex", &token, &["3".into()]) {
                        assert!(
                            error.code == "stale"
                                || (error.code == "read_failed"
                                    && error.message.contains("Registry is busy")),
                            "{error:?}"
                        );
                    }
                }
                _ => unreachable!(),
            }
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
    let rows = store.injection_entries("codex").unwrap();
    let file = std::fs::read_to_string(home.join(".agentdeck/memory/codex.md")).unwrap();
    for content in [
        "synthetic automatic",
        "synthetic manual",
        "synthetic alternative",
    ] {
        assert_eq!(
            file.contains(content),
            rows.iter().any(|r| r.content == content)
        );
    }
    assert_eq!(store.count().unwrap(), 3);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let state = export::read("codex").unwrap();
        let preview = export::preview("codex", &state.state_token, &["3".into()]).unwrap();
        let before = store.manual_injection_ids("codex").unwrap();
        let target = home.join(".agentdeck/memory/codex.md");
        let unrelated = home.join("unrelated.md");
        std::fs::write(&unrelated, "must remain unchanged").unwrap();
        std::fs::remove_file(&target).unwrap();
        symlink(&unrelated, &target).unwrap();
        assert!(export::replace("codex", &preview.preview_token, &["3".into()]).is_err());
        assert_eq!(store.manual_injection_ids("codex").unwrap(), before);
        assert_eq!(
            std::fs::read_to_string(&unrelated).unwrap(),
            "must remain unchanged"
        );
        std::fs::remove_file(&target).unwrap();
        drop(store);
        let original = home.join("original.db");
        std::fs::rename(&db, &original).unwrap();
        symlink(&original, &db).unwrap();
        assert!(export::read("codex").is_err());
        assert!(export::preview("codex", &state.state_token, &["3".into()]).is_err());
        assert!(export::replace("codex", &preview.preview_token, &["3".into()]).is_err());
        assert_eq!(
            MemoryStore::open(&original)
                .unwrap()
                .manual_injection_ids("codex")
                .unwrap(),
            before
        );
    }
}
