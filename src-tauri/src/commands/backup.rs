//! 备份恢复命令（M4）：list_backups / restore_backup

use aisync_core::git;

/// 列出备份历史
#[tauri::command]
pub async fn list_backups() -> Result<Vec<git::BackupInfo>, String> {
    tauri::async_runtime::spawn_blocking(git::list_backups)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// 按时间戳回滚：full=false 只还原该次备份变更，full=true 整体恢复到该时间点
#[tauri::command]
pub async fn restore_backup(timestamp: String, full: bool) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || git::restore(&timestamp, full))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
