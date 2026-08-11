pub mod keystore;

/// 保存 API Key（AES-256-GCM 加密）
pub fn save(_agent_id: &str, _provider: &str, _key: &str) -> crate::Result<()> {
    todo!("M4: vault 加密存储")
}

/// 列出已存储的 Key（脱敏）
pub fn list() -> crate::Result<Vec<(String, String, String)>> {
    todo!("M4: vault 列表")
}

/// 删除 Key
pub fn delete(_agent_id: &str, _provider: &str) -> crate::Result<()> {
    todo!("M4: vault 删除")
}

/// 导出 vault（供备份）
pub fn export() -> crate::Result<String> {
    todo!("M4: vault 导出")
}

/// 导入 vault
pub fn import(_data: &str) -> crate::Result<()> {
    todo!("M4: vault 导入")
}
