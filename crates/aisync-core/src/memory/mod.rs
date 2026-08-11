//! 记忆引擎模块（M3）。
//!
//! SQLite + FTS5 全文搜索存储。
//! Collector 适配器模式采集各 agent 会话历史。
//! 被动注入 + 自动标签双模式。

pub mod collector;
pub mod search;
pub mod storage;

use std::path::{Path, PathBuf};

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

/// 默认记忆库路径：~/.agentdeck/memory.db
pub fn default_db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".agentdeck")
        .join("memory.db")
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
    Ok(agent_id.to_string())
}

/// 初始化记忆数据库（创建表 + FTS5 索引）
pub fn init_db(db_path: &Path) -> crate::Result<()> {
    MemoryStore::open(db_path).map(|_| ())
}

/// 从指定 agent 采集记忆，返回该 agent 最近 100 条
pub fn collect(agent_id: &str) -> crate::Result<Vec<MemoryEntry>> {
    let canonical = match agent_id {
        "claude" => "claude-code",
        other => other,
    };
    let store = MemoryStore::open(&default_db_path())?;
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
    let store = MemoryStore::open(&default_db_path())?;
    store.search(query, 50)
}

/// 注入记忆到指定 agent（生成 ~/.agentdeck/memory/<agent_id>.md）
pub fn inject(agent_id: &str, memory_ids: &[String]) -> crate::Result<()> {
    let agent_id = sanitize_agent_id(agent_id)?;
    let store = MemoryStore::open(&default_db_path())?;
    let ids: Vec<i64> = memory_ids.iter().filter_map(|s| s.parse().ok()).collect();
    let entries = store.get_by_ids(&ids)?;

    let home = dirs::home_dir().ok_or_else(|| crate::Error::Other("无法定位用户主目录".into()))?;
    let dir = home.join(".agentdeck").join("memory");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.md", agent_id));

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
    std::fs::write(&path, out)?;
    Ok(())
}

/// 切换自动注入标签
pub fn toggle_auto_inject(memory_id: &str, enabled: bool) -> crate::Result<()> {
    let id: i64 = memory_id
        .parse()
        .map_err(|_| crate::Error::Other(format!("非法记忆 id: {}", memory_id)))?;
    let store = MemoryStore::open(&default_db_path())?;
    store.set_auto_inject(id, enabled)
}

/// 最近记忆（供 UI 展示）
pub fn recent(agent_id: &str, limit: usize) -> crate::Result<Vec<MemoryEntry>> {
    let store = MemoryStore::open(&default_db_path())?;
    store.recent(agent_id, limit)
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

    /// 真实环境验证：采集本机 Codex/OpenCode 会话并搜索
    #[test]
    #[ignore = "真实环境验证：扫描 ~/.codex 与 opencode.db 写入 ~/.agentdeck/memory.db"]
    fn real_collect_codex_and_opencode() {
        let store = MemoryStore::open(&default_db_path()).unwrap();
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
}
