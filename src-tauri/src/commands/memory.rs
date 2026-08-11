//! 记忆管理命令（M3）
//!
//! 定时采集：Tauri 启动时挂后台线程，首轮立即采集，之后每小时一轮；
//! 每轮采集后把 auto_inject 记忆回写 `~/.agentdeck/memory/<agent>.md`。

use std::sync::Mutex;
use std::time::Duration;

use aisync_core::memory::{self, MemoryEntry};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

/// 定时采集间隔（1 小时）
const COLLECT_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// 定时采集状态（供 UI 展示「上次/下次采集时间」）
#[derive(Default)]
pub struct CollectionState {
    inner: Mutex<CollectionStateInner>,
}

#[derive(Default)]
struct CollectionStateInner {
    last_run_at: Option<i64>,
    last_error: Option<String>,
    next_run_at: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionStatus {
    pub last_run_at: Option<i64>,
    pub last_error: Option<String>,
    pub next_run_at: Option<i64>,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// 采集指定 agent 的记忆
#[tauri::command]
pub fn collect_memory(agent_id: String) -> Result<Vec<MemoryEntry>, String> {
    memory::collect(&agent_id).map_err(|e| e.to_string())
}

/// 搜索记忆（FTS5）
#[tauri::command]
pub fn search_memory(query: String) -> Result<Vec<MemoryEntry>, String> {
    memory::search(&query).map_err(|e| e.to_string())
}

/// 注入记忆到指定 agent
#[tauri::command]
pub fn inject_memory(agent_id: String, memory_ids: Vec<String>) -> Result<(), String> {
    memory::inject(&agent_id, &memory_ids).map_err(|e| e.to_string())
}

/// 切换记忆自动注入标签
#[tauri::command]
pub fn toggle_auto_inject(memory_id: String, enabled: bool) -> Result<(), String> {
    memory::toggle_auto_inject(&memory_id, enabled).map_err(|e| e.to_string())
}

/// 最近记忆列表
#[tauri::command]
pub fn recent_memories(agent_id: String) -> Result<Vec<MemoryEntry>, String> {
    memory::recent(&agent_id, 50).map_err(|e| e.to_string())
}

/// 查询定时采集状态
#[tauri::command]
pub fn memory_collection_status(state: State<'_, CollectionState>) -> CollectionStatus {
    // 采集线程若异常 panic 会毒化锁，降级返回空状态而不是连带崩溃
    let inner = state
        .inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    CollectionStatus {
        last_run_at: inner.last_run_at,
        last_error: inner.last_error.clone(),
        next_run_at: inner.next_run_at,
    }
}

/// 后台定时采集循环：启动即跑一轮，之后每小时一轮
pub fn scheduled_collect_loop(app: AppHandle) {
    loop {
        let now = unix_now();
        match memory::collect_all_and_inject() {
            Ok(report) => {
                let per_agent = report
                    .agents
                    .iter()
                    .map(|a| format!("{}={}", a.agent_id, a.new_count))
                    .collect::<Vec<_>>()
                    .join(", ");
                let injected = if report.auto_injected.is_empty() {
                    "无".to_string()
                } else {
                    report.auto_injected.join(",")
                };
                let partial = if report.errors.is_empty() {
                    String::new()
                } else {
                    format!("；部分失败: {}", report.errors.join("; "))
                };
                println!(
                    "[AgentDeck] 定时采集完成：新增 {} 条（{}），自动注入: {}{}",
                    report.total, per_agent, injected, partial
                );
                let state = app.state::<CollectionState>();
                let mut inner = state.inner.lock().unwrap();
                inner.last_run_at = Some(now);
                inner.last_error = None;
                inner.next_run_at = Some(now + COLLECT_INTERVAL.as_secs() as i64);
            }
            Err(e) => {
                eprintln!("[AgentDeck] 定时采集失败: {}", e);
                let state = app.state::<CollectionState>();
                let mut inner = state.inner.lock().unwrap();
                inner.last_run_at = Some(now);
                inner.last_error = Some(e.to_string());
                inner.next_run_at = Some(now + COLLECT_INTERVAL.as_secs() as i64);
            }
        }
        std::thread::sleep(COLLECT_INTERVAL);
    }
}
