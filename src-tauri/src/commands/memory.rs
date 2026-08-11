//! 记忆管理命令（M3）

use aisync_core::memory::{self, MemoryEntry};

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
