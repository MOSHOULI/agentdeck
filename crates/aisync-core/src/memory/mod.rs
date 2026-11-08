//! 记忆引擎模块（M3）。
//!
//! SQLite + FTS5 全文搜索存储。
//! Collector 适配器模式采集各 agent 会话历史。
//! 被动注入 + 自动标签双模式。

pub mod collector;
pub mod search;
pub mod storage;

use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

/// Cooperative cancellation and a finite time budget for a collection pass.
pub struct CollectionControl {
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
}
impl CollectionControl {
    pub fn new(cancelled: Arc<AtomicBool>, budget: Duration) -> Self {
        Self {
            cancelled,
            deadline: Instant::now() + budget,
        }
    }
    pub(crate) fn check(&self) -> crate::Result<()> {
        if self.cancelled.load(Ordering::Relaxed) {
            return Err(crate::Error::Other("Memory collection cancelled".into()));
        }
        if Instant::now() >= self.deadline {
            return Err(crate::Error::Other(
                "Memory collection exceeded its time budget; retry collection".into(),
            ));
        }
        Ok(())
    }
}
impl Default for CollectionControl {
    fn default() -> Self {
        Self::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(300))
    }
}

// Serialize read-modify-write injection operations within the application.
static COLLECTION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static INJECTION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

use serde::Serialize;

use crate::memory::storage::MemoryStore;

/// 记忆条目
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: String,
    pub agent_id: String,
    pub content: String,
    pub content_hash: String,
    pub collected_at: i64,
    pub source_session_id: Option<String>,
    pub auto_inject: bool,
    pub tags: Vec<String>,
}

/// 一次全量定时采集的报告（供 UI/日志展示）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectReport {
    pub agents: Vec<AgentCollectResult>,
    pub total: usize,
    pub auto_injected: Vec<String>,
    /// 单源采集失败记录（best-effort，不阻塞其他源）
    pub errors: Vec<String>,
}

/// 单个 agent 的采集结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCollectResult {
    pub agent_id: String,
    pub new_count: usize,
}

/// 默认记忆库路径：~/.agentdeck/memory.db
pub fn default_db_path() -> crate::Result<PathBuf> {
    db_path_from_home(crate::home_dir())
}

fn db_path_from_home(home: Option<PathBuf>) -> crate::Result<PathBuf> {
    Ok(home
        .ok_or_else(|| {
            crate::Error::Other("Cannot locate user home; refusing relative memory database".into())
        })?
        .join(".agentdeck")
        .join("memory.db"))
}

fn collection_settings_path() -> crate::Result<PathBuf> {
    Ok(default_db_path()?.with_file_name("memory-collection.json"))
}

/// Automatic conversation collection is opt-in; absent settings mean disabled.
pub fn load_collection_enabled() -> crate::Result<bool> {
    load_collection_enabled_at(&collection_settings_path()?)
}

fn load_collection_enabled_at(path: &Path) -> crate::Result<bool> {
    use std::io::Read;
    for ancestor in path.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(crate::Error::Other(
                    "Collection settings cannot be a symbolic link".into(),
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take(65).read_to_end(&mut bytes)?;
    if bytes.len() > 64 {
        return Err(crate::Error::Other(
            "Collection settings exceed size limit".into(),
        ));
    }
    serde_json::from_slice::<bool>(&bytes)
        .map_err(|e| crate::Error::Other(format!("Invalid collection opt-in setting: {e}")))
}

pub fn save_collection_enabled(enabled: bool) -> crate::Result<()> {
    save_collection_enabled_at(&collection_settings_path()?, enabled)
}

fn save_collection_enabled_at(path: &Path, enabled: bool) -> crate::Result<()> {
    crate::vault::keystore::atomic_write_bytes(path, if enabled { b"true" } else { b"false" })
}

/// 校验 agent_id 只允许小写字母、数字、连字符（防路径穿越）
fn sanitize_agent_id(agent_id: &str) -> crate::Result<String> {
    let valid = !agent_id.is_empty()
        && agent_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !valid {
        return Err(crate::Error::Other(format!("非法 agent_id: {}", agent_id)));
    }
    Ok(if agent_id == "claude" {
        "claude-code"
    } else {
        agent_id
    }
    .to_string())
}

/// Only registered targets with a memory adapter may receive exports.
fn validate_injection_target(agent_id: &str) -> crate::Result<String> {
    let registry = crate::registry::load_registry_runtime()?;
    validate_injection_target_in(agent_id, &registry)
}

pub(crate) fn validate_injection_target_in(
    agent_id: &str,
    registry: &crate::registry::RegistryDoc,
) -> crate::Result<String> {
    let canonical = sanitize_agent_id(agent_id)?;
    let agent = registry.agents.get(&canonical).ok_or_else(|| {
        crate::Error::Other(format!("Unknown memory injection target: {canonical}"))
    })?;
    if !agent.config_inject.memory {
        return Err(crate::Error::Other(format!(
            "Memory injection is unsupported for {canonical}"
        )));
    }
    Ok(canonical)
}

/// 初始化记忆数据库（创建表 + FTS5 索引）
pub fn init_db(db_path: &Path) -> crate::Result<()> {
    MemoryStore::open(db_path).map(|_| ())
}

/// 从指定 agent 采集记忆，返回该 agent 最近 100 条
pub fn collect(agent_id: &str) -> crate::Result<Vec<MemoryEntry>> {
    let _guard = COLLECTION_LOCK.try_lock().map_err(|_| {
        crate::Error::Other("Memory collection already running; retry when complete".into())
    })?;
    let canonical = match agent_id {
        "claude" => "claude-code",
        other => other,
    };
    let store = MemoryStore::open(&default_db_path()?)?;
    match canonical {
        "codex" => {
            collector::collect_from_codex(&store)?;
        }
        "claude-code" => {
            collector::collect_from_claude(&store)?;
        }
        "opencode" => {
            collector::collect_from_opencode(&store)?;
        }
        other => {
            return Err(crate::Error::Other(format!("未知采集源: {}", other)));
        }
    }
    store.recent(canonical, 100)
}

/// 全文搜索记忆
pub fn search(query: &str) -> crate::Result<Vec<MemoryEntry>> {
    let store = MemoryStore::open(&default_db_path()?)?;
    store.search(query, 50)
}

/// 生成记忆注入 Markdown 内容（inject 与自动注入共用）
fn render_memory_file(entries: &[MemoryEntry]) -> String {
    let mut out =
        String::from("# AgentDeck 记忆注入\n\n> 由 AgentDeck 自动维护，手动改动会被覆盖。\n\n");
    for entry in entries {
        let time = chrono::DateTime::from_timestamp(entry.collected_at, 0)
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "未知时间".to_string());
        out.push_str(&format!(
            "- **{}** [{}] {}\n",
            time,
            entry.tags.join(","),
            entry.content.replace('\n', " ")
        ));
    }
    out
}

/// 写记忆注入文件到指定目录（测试可注入目录）
fn write_memory_file_at(dir: &Path, agent_id: &str, entries: &[MemoryEntry]) -> crate::Result<()> {
    let agent_id = sanitize_agent_id(agent_id)?;
    let path = dir.join(format!("{}.md", agent_id));
    crate::vault::keystore::atomic_write_bytes(&path, render_memory_file(entries).as_bytes())
}

/// 写记忆注入文件 ~/.agentdeck/memory/<agent_id>.md
fn write_memory_file(agent_id: &str, entries: &[MemoryEntry]) -> crate::Result<()> {
    let agent_id = validate_injection_target(agent_id)?;
    let home = crate::home_dir().ok_or_else(|| crate::Error::Other("无法定位用户主目录".into()))?;
    write_memory_file_at(&home.join(".agentdeck").join("memory"), &agent_id, entries)
}

/// 注入指定记忆到 agent（生成 ~/.agentdeck/memory/<agent_id>.md）
pub fn inject(agent_id: &str, memory_ids: &[String]) -> crate::Result<()> {
    let agent_id = validate_injection_target(agent_id)?;
    let store = MemoryStore::open(&default_db_path()?)?;
    let _guard = INJECTION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let ids: Vec<i64> = memory_ids
        .iter()
        .map(|s| {
            s.parse::<i64>()
                .map_err(|_| crate::Error::Other(format!("非法记忆 id: {}", s)))
        })
        .collect::<crate::Result<_>>()?;
    let entries = store.get_by_ids(&ids)?;
    if entries.len() != ids.iter().collect::<std::collections::HashSet<_>>().len() {
        return Err(crate::Error::Other(
            "One or more selected memories no longer exist".into(),
        ));
    }
    let previous = store.manual_injection_ids(&agent_id)?;
    store.set_manual_injection_ids(&agent_id, &ids)?;
    let result = store
        .injection_entries(&agent_id)
        .and_then(|entries| write_memory_file(&agent_id, &entries));
    if let Err(error) = result {
        store.set_manual_injection_ids(&agent_id, &previous)?;
        return Err(error);
    }
    Ok(())
}

/// 把某 agent 全部 auto_inject 记忆回写记忆文件，返回写入条数
pub fn inject_auto(agent_id: &str) -> crate::Result<usize> {
    let agent_id = validate_injection_target(agent_id)?;
    let store = MemoryStore::open(&default_db_path()?)?;
    let _guard = INJECTION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let home = crate::home_dir().ok_or_else(|| crate::Error::Other("无法定位用户主目录".into()))?;
    reconcile_auto_at(&store, &agent_id, &home.join(".agentdeck").join("memory"))
}

fn reconcile_auto_at(store: &MemoryStore, agent_id: &str, dir: &Path) -> crate::Result<usize> {
    let entries = store.injection_entries(agent_id)?;
    // Write even when empty: a stale file must never retain revoked memories.
    write_memory_file_at(dir, agent_id, &entries)?;
    Ok(entries.len())
}

/// 全量采集（codex / claude-code / opencode），随后回写 auto_inject 记忆文件
pub fn collect_all_and_inject() -> crate::Result<CollectReport> {
    collect_all_and_inject_controlled(&CollectionControl::default())
}

pub fn collect_all_and_inject_controlled(
    control: &CollectionControl,
) -> crate::Result<CollectReport> {
    let _guard = COLLECTION_LOCK.try_lock().map_err(|_| {
        crate::Error::Other("Memory collection already running; retry when complete".into())
    })?;
    let store = MemoryStore::open(&default_db_path()?)?;
    let mut agents = Vec::new();
    let mut total = 0usize;
    let mut errors = Vec::new();
    for agent_id in ["codex", "claude-code", "opencode"] {
        control.check()?;
        let before = store.count()?;
        let count = match agent_id {
            "codex" => collector::collect_from_codex_controlled(&store, control),
            "claude-code" => collector::collect_from_claude_controlled(&store, control),
            "opencode" => collector::collect_from_opencode_controlled(&store, control),
            _ => unreachable!(),
        };
        let count = match count {
            Ok(n) => n,
            Err(e) => {
                errors.push(format!("{}: {}", agent_id, e));
                (store.count()? - before).max(0) as usize
            }
        };
        total += count;
        agents.push(AgentCollectResult {
            agent_id: agent_id.to_string(),
            new_count: count,
        });
    }

    let mut auto_injected = Vec::new();
    for agent_id in ["codex", "claude-code", "opencode"] {
        control.check()?;
        match inject_auto(agent_id) {
            Ok(n) if n > 0 => auto_injected.push(agent_id.to_string()),
            Ok(_) => {}
            Err(e) => errors.push(format!("{} injection: {}", agent_id, e)),
        }
    }
    Ok(CollectReport {
        agents,
        total,
        auto_injected,
        errors,
    })
}

/// 切换自动注入标签
pub fn toggle_auto_inject(memory_id: &str, enabled: bool) -> crate::Result<()> {
    let id: i64 = memory_id
        .parse()
        .map_err(|_| crate::Error::Other(format!("非法记忆 id: {}", memory_id)))?;
    let store = MemoryStore::open(&default_db_path()?)?;
    let _guard = INJECTION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let entry = store
        .get_by_ids(&[id])?
        .into_iter()
        .next()
        .ok_or_else(|| crate::Error::Other(format!("Memory {} does not exist", id)))?;
    update_auto_and_reconcile(&store, &entry, enabled, |entries| {
        write_memory_file(&sanitize_agent_id(&entry.agent_id)?, entries)
    })
}

fn update_auto_and_reconcile(
    store: &MemoryStore,
    entry: &MemoryEntry,
    enabled: bool,
    write: impl FnOnce(&[MemoryEntry]) -> crate::Result<()>,
) -> crate::Result<()> {
    let id: i64 = entry
        .id
        .parse()
        .map_err(|_| crate::Error::Other("Invalid stored memory ID".into()))?;
    store.set_auto_inject(id, enabled)?;
    let result = store
        .injection_entries(&entry.agent_id)
        .and_then(|entries| write(&entries));
    if let Err(error) = result {
        // Atomic file replacement failed, so the old file remains authoritative.
        // Keep the UI flag consistent with that file rather than claiming revocation.
        store
            .set_auto_inject(id, entry.auto_inject)
            .map_err(|rollback| {
                crate::Error::Other(format!(
                    "Injection failed: {error}; flag rollback also failed: {rollback}"
                ))
            })?;
        return Err(error);
    }
    Ok(())
}

/// 最近记忆（供 UI 展示）
pub fn recent(agent_id: &str, limit: usize) -> crate::Result<Vec<MemoryEntry>> {
    let store = MemoryStore::open(&default_db_path()?)?;
    store.recent(
        if agent_id == "claude" {
            "claude-code"
        } else {
            agent_id
        },
        limit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentdeck-memory-{}-{}", tag, nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("memory.db")
    }

    #[test]
    fn collection_opt_in_defaults_off_and_persists_boolean_only() {
        let path = temp_db("opt-in").with_file_name("settings.json");
        assert!(!load_collection_enabled_at(&path).unwrap());
        save_collection_enabled_at(&path, true).unwrap();
        assert!(load_collection_enabled_at(&path).unwrap());
        save_collection_enabled_at(&path, false).unwrap();
        assert!(!load_collection_enabled_at(&path).unwrap());
        std::fs::write(&path, b"{\"enabled\":true}").unwrap();
        assert!(load_collection_enabled_at(&path).is_err());
        std::fs::write(&path, vec![b' '; 65]).unwrap();
        assert!(load_collection_enabled_at(&path).is_err());
    }

    #[test]
    fn missing_home_never_falls_back_to_working_directory() {
        assert!(db_path_from_home(None).is_err());
        let root = temp_db("home").parent().unwrap().to_path_buf();
        assert_eq!(
            db_path_from_home(Some(root.clone())).unwrap(),
            root.join(".agentdeck/memory.db")
        );
    }

    #[test]
    fn injection_targets_require_registered_memory_capability() {
        let db = temp_db("targets");
        let registry = crate::registry::load_registry_at(
            &crate::registry::DataPaths::from_data_dir(db.parent().unwrap()),
        )
        .unwrap();
        assert_eq!(
            validate_injection_target_in("claude", &registry).unwrap(),
            "claude-code"
        );
        assert!(validate_injection_target_in("codex", &registry).is_ok());
        assert!(validate_injection_target_in("made-up-target", &registry).is_err());
        assert!(validate_injection_target_in("pi", &registry).is_err());
        assert!(validate_injection_target_in("../codex", &registry).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn memory_export_rejects_symlink_directory() {
        let db = temp_db("symlink");
        let root = db.parent().unwrap();
        let outside = root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("memory")).unwrap();
        assert!(write_memory_file_at(&root.join("memory"), "codex", &[]).is_err());
        assert!(!outside.join("codex.md").exists());
    }

    #[test]
    fn insert_search_and_dedup() {
        let db = temp_db("basic");
        let store = MemoryStore::open(&db).unwrap();

        let id = store
            .insert(
                "codex",
                "rust memory engine FTS5",
                "src:a",
                1000,
                false,
                &["codex".into()],
            )
            .unwrap()
            .expect("首次插入应返回 id");
        store
            .insert(
                "opencode",
                "coffee beans recipe",
                "src:b",
                2000,
                true,
                &["opencode".into()],
            )
            .unwrap();

        let again = store
            .insert(
                "codex",
                "rust memory engine FTS5",
                "src:a",
                1000,
                false,
                &[],
            )
            .unwrap();
        assert!(again.is_none(), "重复 source+content 应被忽略");

        let hits = store.search("rust", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].content, "rust memory engine FTS5");

        let all = store.search("", 10).unwrap();
        assert_eq!(all.len(), 2);

        store.set_auto_inject(id, true).unwrap();
        let hits = store.search("rust", 10).unwrap();
        assert!(hits[0].auto_inject);
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn get_by_ids_and_empty_query() {
        let db = temp_db("ids");
        let store = MemoryStore::open(&db).unwrap();
        let a = store
            .insert("codex", "alpha beta", "s1", 100, false, &[])
            .unwrap()
            .unwrap();
        let b = store
            .insert("codex", "gamma delta", "s2", 200, false, &[])
            .unwrap()
            .unwrap();

        let entries = store.get_by_ids(&[a, b]).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.content == "gamma delta"));
    }

    #[test]
    fn sanitize_agent_id_rejects_path_traversal() {
        assert!(sanitize_agent_id("codex").is_ok());
        assert!(sanitize_agent_id("claude-code").is_ok());
        for evil in [
            "../evil", "..\\evil", "a/b", "a\\b", "a:b", "a*b", "a\0b", "",
        ] {
            assert!(sanitize_agent_id(evil).is_err(), "应拒绝: {:?}", evil);
        }
    }

    #[test]
    fn auto_inject_entries_filters_and_writes_file() {
        let db = temp_db("autoinject");
        let store = MemoryStore::open(&db).unwrap();
        store
            .insert(
                "codex",
                "需要自动注入的记忆 A",
                "s1",
                1000,
                true,
                &["codex".into()],
            )
            .unwrap();
        store
            .insert("codex", "不需要注入的记忆 B", "s2", 2000, false, &[])
            .unwrap();

        let entries = store.auto_inject_entries("codex").unwrap();
        assert_eq!(entries.len(), 1, "只应返回 auto_inject 的记忆");
        assert_eq!(entries[0].content, "需要自动注入的记忆 A");
        assert!(
            store.auto_inject_entries("opencode").unwrap().is_empty(),
            "其他 agent 不应有记录"
        );

        let out_dir = db.parent().unwrap().join("memory");
        write_memory_file_at(&out_dir, "codex", &entries).unwrap();
        let content = std::fs::read_to_string(out_dir.join("codex.md")).unwrap();
        assert!(content.contains("需要自动注入的记忆 A"));
        assert!(!content.contains("不需要注入的记忆 B"));
    }

    #[test]
    fn empty_reconciliation_removes_revoked_content() {
        let db = temp_db("revocation");
        let store = MemoryStore::open(&db).unwrap();
        let id = store
            .insert("codex", "revoked memory", "s", 1, true, &[])
            .unwrap()
            .unwrap();
        let dir = db.parent().unwrap().join("memory");
        assert_eq!(reconcile_auto_at(&store, "codex", &dir).unwrap(), 1);
        store.set_auto_inject(id, false).unwrap();
        assert_eq!(reconcile_auto_at(&store, "codex", &dir).unwrap(), 0);
        assert!(
            !std::fs::read_to_string(dir.join("codex.md"))
                .unwrap()
                .contains("revoked memory")
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
    }

    #[test]
    fn manual_selections_survive_refresh_and_auto_revocation() {
        let db = temp_db("manual-selection");
        let store = MemoryStore::open(&db).unwrap();
        let manual = store
            .insert("opencode", "manual cross-agent memory", "m", 1, false, &[])
            .unwrap()
            .unwrap();
        let automatic = store
            .insert("codex", "automatic memory", "a", 2, true, &[])
            .unwrap()
            .unwrap();
        store.set_manual_injection_ids("codex", &[manual]).unwrap();
        let dir = db.parent().unwrap().join("memory");
        assert_eq!(reconcile_auto_at(&store, "codex", &dir).unwrap(), 2);
        store.set_auto_inject(automatic, false).unwrap();
        assert_eq!(reconcile_auto_at(&store, "codex", &dir).unwrap(), 1);
        let content = std::fs::read_to_string(dir.join("codex.md")).unwrap();
        assert!(content.contains("manual cross-agent memory"));
        assert!(!content.contains("automatic memory"));
        store.set_manual_injection_ids("codex", &[]).unwrap();
        assert_eq!(reconcile_auto_at(&store, "codex", &dir).unwrap(), 0);
    }

    #[test]
    fn failed_reconciliation_restores_auto_inject_flag() {
        let db = temp_db("failed-revocation");
        let store = MemoryStore::open(&db).unwrap();
        let id = store
            .insert("codex", "still injected", "s", 1, true, &[])
            .unwrap()
            .unwrap();
        let entry = store.get_by_ids(&[id]).unwrap().remove(0);
        assert!(
            update_auto_and_reconcile(&store, &entry, false, |_| Err(crate::Error::Other(
                "write denied".into()
            )))
            .is_err()
        );
        assert!(store.get_by_ids(&[id]).unwrap()[0].auto_inject);
    }

    #[test]
    fn cancellation_and_expired_budget_are_actionable() {
        let control =
            CollectionControl::new(Arc::new(AtomicBool::new(true)), Duration::from_secs(5));
        assert!(
            control
                .check()
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        let control = CollectionControl::new(Arc::new(AtomicBool::new(false)), Duration::ZERO);
        assert!(
            control
                .check()
                .unwrap_err()
                .to_string()
                .contains("time budget")
        );
    }

    /// 真实环境验证：采集本机 Codex/OpenCode 会话并搜索
    #[test]
    #[ignore = "真实环境验证：扫描 ~/.codex 与 opencode.db 写入 ~/.agentdeck/memory.db"]
    fn real_collect_codex_and_opencode() {
        let store = MemoryStore::open(&default_db_path().unwrap()).unwrap();
        let codex = collector::collect_from_codex(&store).unwrap();
        let opencode = collector::collect_from_opencode(&store).unwrap();
        println!("codex 新增 {} 条，opencode 新增 {} 条", codex, opencode);
        println!("记忆库总数: {}", store.count().unwrap());
        for entry in store.recent("opencode", 5).unwrap() {
            println!("[opencode] {} {}", entry.collected_at, entry.content);
        }
        let hits = store.search("配置", 5).unwrap();
        println!("搜索「配置」命中 {} 条", hits.len());
    }

    /// 真实环境验证：全量采集 + auto_inject 记忆回写文件
    #[test]
    #[ignore = "真实环境验证：扫描三个采集源并回写 ~/.agentdeck/memory/<agent>.md"]
    fn real_collect_all_and_auto_inject() {
        let report = collect_all_and_inject().unwrap();
        println!(
            "全量采集新增 {} 条（{}），自动注入 agent: {:?}",
            report.total,
            report
                .agents
                .iter()
                .map(|a| format!("{}={}", a.agent_id, a.new_count))
                .collect::<Vec<_>>()
                .join(", "),
            report.auto_injected
        );
        assert_eq!(report.agents.len(), 3, "应覆盖三个采集源");
        let ids: Vec<&str> = report.agents.iter().map(|a| a.agent_id.as_str()).collect();
        assert_eq!(ids, ["codex", "claude-code", "opencode"]);
        let sum: usize = report.agents.iter().map(|a| a.new_count).sum();
        assert_eq!(report.total, sum, "total 应等于各 agent 新增之和");
    }
}
