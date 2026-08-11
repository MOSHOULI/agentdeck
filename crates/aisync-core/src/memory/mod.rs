/// 记忆引擎模块（M3）。
///
/// SQLite + FTS5 全文搜索存储。
/// Collector 适配器模式采集各 agent 会话历史。
/// 被动注入 + 自动标签双模式。

pub mod collector;
pub mod storage;
pub mod search;

/// 记忆条目
#[derive(Debug, Clone)]
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

/// 初始化记忆数据库（创建表 + FTS5 索引）
pub fn init_db(_db_path: &std::path::Path) -> crate::Result<()> {
    todo!("M3: 初始化 SQLite 记忆数据库")
}

/// 从指定 agent 采集记忆
pub fn collect(_agent_id: &str) -> crate::Result<Vec<MemoryEntry>> {
    todo!("M3: 采集 agent 记忆")
}

/// 全文搜索记忆
pub fn search(_query: &str) -> crate::Result<Vec<MemoryEntry>> {
    todo!("M3: FTS5 全文搜索")
}

/// 注入记忆到指定 agent
pub fn inject(_agent_id: &str, _memory_ids: &[String]) -> crate::Result<()> {
    todo!("M3: 注入记忆")
}

/// 切换自动注入标签
pub fn toggle_auto_inject(_memory_id: &str, _enabled: bool) -> crate::Result<()> {
    todo!("M3: 切换自动注入")
}
