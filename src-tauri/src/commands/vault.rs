//! Vault 密钥管理命令（M4）：save_api_key / list_api_keys / get_api_key / delete_api_key / import_v2_keys

use aisync_core::vault;
use serde::Serialize;

/// 前端 bridge.ts 的密钥列表项（camelCase 对齐）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyInfo {
    agent_id: String,
    provider: String,
    masked_key: String,
}

/// 保存 API Key（加密存入 vault；变动前自动 git 备份）
#[tauri::command]
pub async fn save_api_key(agent_id: String, provider: String, key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        aisync_core::git::backup(&format!("vault: save {}", agent_id))?;
        vault::save(&agent_id, &provider, &key)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// 列出已存储的 API Key（脱敏显示）
#[tauri::command]
pub fn list_api_keys() -> Result<Vec<ApiKeyInfo>, String> {
    let keys = vault::list().map_err(|e| e.to_string())?;
    Ok(keys
        .into_iter()
        .map(|k| ApiKeyInfo {
            agent_id: k.name,
            provider: k.provider,
            masked_key: k.masked,
        })
        .collect())
}

/// 取明文 API Key（复制用）
#[tauri::command]
pub fn get_api_key(agent_id: String) -> Result<Option<String>, String> {
    vault::get(&agent_id).map_err(|e| e.to_string())
}

/// 删除 API Key（变动前自动 git 备份）
#[tauri::command]
pub async fn delete_api_key(agent_id: String) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        aisync_core::git::backup(&format!("vault: delete {}", agent_id))?;
        vault::delete(&agent_id)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// 从 V2 vault（~/.aisync-vault）导入密钥，返回导入条数
#[tauri::command]
pub async fn import_v2_keys() -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(move || {
        aisync_core::git::backup("vault: import v2")?;
        vault::import_from_v2()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}
