//! 配置同步模块（M2-M5）。
//!
//! 五类内容同步：skill、插件、个性化、MCP、规则记忆。
//! 适配器模式：每种配置格式一个 Writer（TOML Writer / JSONC Merge / YAML Section / JSON Merge）。

pub fn sync_skills(_agent_id: &str) -> crate::Result<()> {
    todo!("M2: 同步 skills")
}

/// 同步 MCP 配置：确保 enabled 列表中的服务器已注入，其余移除
pub fn sync_mcp(agent_id: &str, enabled_servers: &[String]) -> crate::Result<()> {
    crate::mcp::sync(agent_id, enabled_servers).map(|_| ())
}

pub fn sync_personalization(_agent_id: &str) -> crate::Result<()> {
    todo!("M2: 同步个性化配置")
}

pub fn sync_memory_rules(_agent_id: &str) -> crate::Result<()> {
    todo!("M2: 同步规则记忆文件")
}
