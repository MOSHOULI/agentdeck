//! 注册表在线更新命令（M6）：check_updates / apply_updates

use serde::Serialize;

/// 更新检查结果（供 UI 展示）
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSummary {
    pub version: String,
    pub released: String,
    pub files: Vec<String>,
    pub pubkey_fp: String,
}

/// 更新基准 URL（默认 GitHub Releases；可用 AGENTDECK_UPDATE_BASE 覆盖，便于本地验证）
fn update_base() -> String {
    std::env::var("AGENTDECK_UPDATE_BASE").unwrap_or_else(|_| {
        "https://github.com/MOSHOULI/agentdeck/releases/latest/download".to_string()
    })
}

fn current_version() -> Result<String, String> {
    let paths = aisync_core::registry::DataPaths::discover().map_err(|e| e.to_string())?;
    aisync_core::registry::installed_version_at(&paths, env!("CARGO_PKG_VERSION"))
        .map_err(|e| e.to_string())
}

/// 拉取并验签 manifest，返回 manifest JSON 文本
fn fetch_verified_manifest() -> Result<aisync_core::update::VerifiedManifest, String> {
    let pubkey = aisync_core::release::embedded_public_key()
        .ok_or_else(|| "应用未内置发布公钥（开发版本无更新能力）".to_string())?;
    let base = update_base();
    let manifest_json =
        aisync_core::update::fetch_bytes(&format!("{}/manifest.json", base.trim_end_matches('/')))
            .map_err(|e| e.to_string())?;
    let manifest_json = String::from_utf8(manifest_json).map_err(|e| e.to_string())?;
    let sig_b64 = aisync_core::update::fetch_bytes(&format!(
        "{}/manifest.json.sig",
        base.trim_end_matches('/')
    ))
    .map_err(|e| e.to_string())?;
    let sig_b64 = String::from_utf8(sig_b64).map_err(|e| e.to_string())?;
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    let sig = B64
        .decode(sig_b64.trim())
        .map_err(|e| format!("签名解码失败: {}", e))?;
    aisync_core::update::parse_and_verify(&manifest_json, &sig, &pubkey).map_err(|e| e.to_string())
}

/// 检查更新：拉取最新发布的已签名 manifest，有更新返回摘要
#[tauri::command]
pub async fn check_updates() -> Result<Option<UpdateSummary>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let current = current_version()?;
        let manifest = fetch_verified_manifest()?;
        if !aisync_core::update::is_update_available(&current, &manifest) {
            return Ok(None);
        }
        Ok(Some(UpdateSummary {
            version: manifest.version.clone(),
            released: manifest.released.clone(),
            files: manifest.files.iter().map(|f| f.name.clone()).collect(),
            pubkey_fp: manifest.pubkey_fp.clone(),
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 应用更新（下载 → 校验 → 备份 → 原子写入运行时注册表目录）
#[tauri::command]
pub async fn apply_updates() -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let current = current_version()?;
        let manifest = fetch_verified_manifest()?;
        if !aisync_core::update::is_update_available(&current, &manifest) {
            return Err("没有可应用的更新".to_string());
        }
        aisync_core::update::apply_update(&manifest).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
