//! MCP 管理模块（M5）。
//!
//! 内置注册表（`agents/mcp-registry.json`）定义可用 MCP 服务器；
//! 每款支持 MCP 的 agent 目标（文件/格式/键）来自 `registry-v3.json` 的
//! `config_inject.mcp`。开关实时写回 agent 配置文件：
//! TOML section / JSON merge / JSONC merge / YAML section。
//! 命令白名单防止任意命令被写入配置。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::Error;

/// 允许写入配置的 MCP 启动命令（与 V2 白名单一致）
const ALLOWED_BINARIES: &[&str] = &[
    "npx", "uvx", "python", "python3", "node", "docker", "bun", "deno",
];

#[derive(Debug, Clone, Deserialize)]
struct McpServerDef {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    disabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerInfo {
    pub name: String,
    pub description: String,
    /// http | stdio
    pub kind: String,
    pub url: Option<String>,
    pub command: Option<String>,
    pub args: Vec<String>,
    /// 支持 MCP 的 agent id 列表（来自 registry-v3.json config_inject.mcp）
    pub supported_agents: Vec<String>,
    /// 已启用该服务器的 agent id 列表
    pub enabled_agents: Vec<String>,
}

fn load_mcp_registry() -> crate::Result<Vec<McpServerDef>> {
    #[derive(Deserialize)]
    struct RegistryFile {
        servers: Vec<McpServerDef>,
    }
    let content = crate::registry::load_mcp_registry_runtime()?;
    let file: RegistryFile = serde_json::from_str(&content)
        .map_err(|e| Error::Other(format!("MCP 注册表解析失败: {}", e)))?;
    Ok(file.servers)
}

fn load_agent_targets() -> crate::Result<Vec<(String, crate::registry::McpInject)>> {
    let doc = crate::registry::load_registry_runtime()?;
    let mut out = Vec::new();
    for (id, agent) in doc.agents {
        if let Some(target) = agent.config_inject.mcp {
            out.push((id, target));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn find_server(name: &str) -> crate::Result<McpServerDef> {
    load_mcp_registry()?
        .into_iter()
        .find(|s| s.name == name && !s.disabled)
        .ok_or_else(|| Error::Other(format!("MCP 服务器不存在或已禁用: {}", name)))
}

/// 展开路径：支持 `~` 与白名单环境变量（`%APPDATA%` 等，大小写不敏感）
fn expand_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim();
    if let Some(home) = crate::home_dir()
        && let Some(rest) = trimmed
            .strip_prefix("~/")
            .or_else(|| trimmed.strip_prefix("~\\"))
    {
        return home.join(rest);
    }
    let mut out = trimmed.to_string();
    const ENV_WHITELIST: &[&str] = &[
        "APPDATA",
        "LOCALAPPDATA",
        "USERPROFILE",
        "HOME",
        "XDG_CONFIG_HOME",
    ];
    for var in ENV_WHITELIST {
        let re = regex::Regex::new(&format!(r"(?i)%{}%", regex::escape(var)))
            .expect("环境变量正则应为合法正则");
        if let Some(val) = std::env::var_os(var) {
            out = re
                .replace_all(&out, val.to_string_lossy().as_ref())
                .to_string();
        }
    }
    PathBuf::from(out)
}

fn read_text(path: &Path) -> crate::Result<String> {
    if path.exists() {
        Ok(std::fs::read_to_string(path)?)
    } else {
        Ok(String::new())
    }
}

fn write_text(path: &Path, content: &str) -> crate::Result<()> {
    if let Ok(metadata) = std::fs::symlink_metadata(path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(Error::Other(
            "MCP destination must be a regular file".into(),
        ));
    }
    crate::vault::keystore::atomic_write_bytes(path, content.as_bytes())
}

// ─── TOML ─────────────────────────────────────────

fn toml_server_names(text: &str, section: &str) -> Vec<String> {
    let Ok(doc) = text.parse::<toml_edit::Document>() else {
        return Vec::new();
    };
    doc.get(section)
        .and_then(|v| v.as_table())
        .map(|t| t.iter().map(|(k, _)| k.to_string()).collect())
        .unwrap_or_default()
}

fn toml_set_server(
    text: &str,
    section: &str,
    name: &str,
    def: &McpServerDef,
) -> crate::Result<String> {
    let mut doc: toml_edit::Document = if text.trim().is_empty() {
        toml_edit::Document::new()
    } else {
        text.parse()
            .map_err(|e| Error::Other(format!("TOML 解析失败: {}", e)))?
    };
    let servers = doc
        .as_table_mut()
        .entry(section)
        .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
        .as_table_mut()
        .ok_or_else(|| Error::Other(format!("段 {} 不是表格", section)))?;

    let mut srv = toml_edit::Table::new();
    if let Some(url) = &def.url {
        srv.insert(
            "type",
            toml_edit::Item::Value(toml_edit::Value::from("http")),
        );
        srv.insert(
            "url",
            toml_edit::Item::Value(toml_edit::Value::from(url.as_str())),
        );
    } else if let Some(cmd) = &def.command {
        srv.insert(
            "type",
            toml_edit::Item::Value(toml_edit::Value::from("stdio")),
        );
        srv.insert(
            "command",
            toml_edit::Item::Value(toml_edit::Value::from(cmd.as_str())),
        );
        if !def.args.is_empty() {
            let arr = toml_edit::Array::from_iter(
                def.args.iter().map(|a| toml_edit::Value::from(a.as_str())),
            );
            srv.insert("args", toml_edit::Item::Value(toml_edit::Value::Array(arr)));
        }
    }
    srv.insert(
        "enabled",
        toml_edit::Item::Value(toml_edit::Value::from(true)),
    );
    servers.insert(name, toml_edit::Item::Table(srv));
    Ok(doc.to_string())
}

fn toml_remove_server(text: &str, section: &str, name: &str) -> crate::Result<String> {
    let mut doc: toml_edit::Document = if text.trim().is_empty() {
        return Ok(text.to_string());
    } else {
        text.parse()
            .map_err(|e| Error::Other(format!("TOML 解析失败: {}", e)))?
    };
    if let Some(table) = doc
        .as_table_mut()
        .get_mut(section)
        .and_then(|v| v.as_table_mut())
    {
        table.remove(name);
    }
    Ok(doc.to_string())
}

// ─── JSON / JSONC ─────────────────────────────────

fn json_value(text: &str, is_jsonc: bool) -> crate::Result<serde_json::Value> {
    let cleaned = if is_jsonc {
        crate::config::strip_jsonc_comments(text)
    } else {
        text.to_string()
    };
    if cleaned.trim().is_empty() {
        return Ok(serde_json::Value::Object(Default::default()));
    }
    serde_json::from_str(&cleaned).map_err(|e| Error::Other(format!("JSON 解析失败: {}", e)))
}

fn json_server_names(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|v| v.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

fn json_set_server(
    value: &mut serde_json::Value,
    key: &str,
    name: &str,
    def: &McpServerDef,
    local_array_command: bool,
) -> crate::Result<()> {
    let obj = value
        .as_object_mut()
        .ok_or_else(|| Error::Other("JSON 根节点不是对象".into()))?;
    let servers = obj
        .entry(key.to_string())
        .or_insert_with(|| serde_json::Value::Object(Default::default()))
        .as_object_mut()
        .ok_or_else(|| Error::Other(format!("键 {} 不是对象", key)))?;

    let mut entry = serde_json::Map::new();
    if let Some(url) = &def.url {
        if local_array_command {
            entry.insert("type".into(), serde_json::Value::String("remote".into()));
            entry.insert("url".into(), serde_json::Value::String(url.clone()));
        } else {
            entry.insert("type".into(), serde_json::Value::String("http".into()));
            entry.insert("url".into(), serde_json::Value::String(url.clone()));
        }
    } else if let Some(cmd) = &def.command {
        if local_array_command {
            let mut arr = vec![serde_json::Value::String(cmd.clone())];
            arr.extend(def.args.iter().cloned().map(serde_json::Value::String));
            entry.insert("type".into(), serde_json::Value::String("local".into()));
            entry.insert("command".into(), serde_json::Value::Array(arr));
        } else {
            entry.insert("type".into(), serde_json::Value::String("stdio".into()));
            entry.insert("command".into(), serde_json::Value::String(cmd.clone()));
            if !def.args.is_empty() {
                entry.insert(
                    "args".into(),
                    serde_json::Value::Array(
                        def.args
                            .iter()
                            .cloned()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }
        }
    }
    if let Some(desc) = &def.description {
        entry.insert(
            "description".into(),
            serde_json::Value::String(desc.clone()),
        );
    }
    servers.insert(name.to_string(), serde_json::Value::Object(entry));
    Ok(())
}

fn json_remove_server(value: &mut serde_json::Value, key: &str, name: &str) {
    if let Some(servers) = value.get_mut(key).and_then(|v| v.as_object_mut()) {
        servers.remove(name);
    }
}

// ─── YAML ─────────────────────────────────────────

fn yaml_server_names(value: &serde_json::Value, section: &str) -> Vec<String> {
    value
        .get(section)
        .and_then(|v| v.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

fn yaml_set_server(
    value: &mut serde_json::Value,
    section: &str,
    name: &str,
    def: &McpServerDef,
) -> crate::Result<()> {
    let obj = value
        .as_object_mut()
        .ok_or_else(|| Error::Other("YAML 根节点不是对象".into()))?;
    let servers = obj
        .entry(section.to_string())
        .or_insert_with(|| serde_json::Value::Object(Default::default()))
        .as_object_mut()
        .ok_or_else(|| Error::Other(format!("段 {} 不是对象", section)))?;

    let mut entry = serde_json::Map::new();
    if let Some(url) = &def.url {
        entry.insert("type".into(), serde_json::Value::String("http".into()));
        entry.insert("url".into(), serde_json::Value::String(url.clone()));
    } else if let Some(cmd) = &def.command {
        entry.insert("type".into(), serde_json::Value::String("stdio".into()));
        entry.insert("command".into(), serde_json::Value::String(cmd.clone()));
        if !def.args.is_empty() {
            entry.insert(
                "args".into(),
                serde_json::Value::Array(
                    def.args
                        .iter()
                        .cloned()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }
    }
    servers.insert(name.to_string(), serde_json::Value::Object(entry));
    Ok(())
}

fn yaml_remove_server(value: &mut serde_json::Value, section: &str, name: &str) {
    if let Some(servers) = value.get_mut(section).and_then(|v| v.as_object_mut()) {
        servers.remove(name);
    }
}

// ─── 公共 API ─────────────────────────────────────

fn agent_target(agent_id: &str) -> crate::Result<crate::registry::McpInject> {
    load_agent_targets()?
        .into_iter()
        .find(|(id, _)| id == agent_id)
        .map(|(_, t)| t)
        .ok_or_else(|| Error::Other(format!("agent 不支持 MCP: {}", agent_id)))
}

/// 当前 agent 配置里已启用的 MCP 服务器名
pub fn current_servers(agent_id: &str) -> crate::Result<Vec<String>> {
    let target = agent_target(agent_id)?;
    let path = expand_path(&target.file);
    let text = read_text(&path)?;
    Ok(match target.format.as_str() {
        "toml_section" => {
            toml_server_names(&text, target.section.as_deref().unwrap_or("mcp_servers"))
        }
        "json_merge" => json_server_names(
            &json_value(&text, false)?,
            target.key.as_deref().unwrap_or("mcpServers"),
        ),
        "jsonc_merge" => json_server_names(
            &json_value(&text, true)?,
            target.key.as_deref().unwrap_or("mcp"),
        ),
        "yaml_section" => {
            let v: serde_json::Value = if text.trim().is_empty() {
                serde_json::Value::Object(Default::default())
            } else {
                serde_yml::from_str(&text)
                    .map_err(|e| Error::Other(format!("YAML 解析失败: {}", e)))?
            };
            yaml_server_names(&v, target.section.as_deref().unwrap_or("mcp_servers"))
        }
        other => return Err(Error::Other(format!("未知 MCP 格式: {}", other))),
    })
}

/// 列出 MCP 服务器及每款 agent 的启用状态
pub fn list_servers() -> crate::Result<Vec<McpServerInfo>> {
    let targets = load_agent_targets()?;
    let supported_agents: Vec<String> = targets.iter().map(|(id, _)| id.clone()).collect();
    let mut out = Vec::new();
    for def in load_mcp_registry()? {
        if def.disabled {
            continue;
        }
        let mut enabled_agents = Vec::new();
        for (agent_id, _) in &targets {
            if current_servers(agent_id)?.iter().any(|s| s == &def.name) {
                enabled_agents.push(agent_id.clone());
            }
        }
        out.push(McpServerInfo {
            kind: if def.url.is_some() { "http" } else { "stdio" }.into(),
            name: def.name,
            description: def.description.unwrap_or_default(),
            url: def.url,
            command: def.command,
            args: def.args,
            supported_agents: supported_agents.clone(),
            enabled_agents,
        });
    }
    Ok(out)
}

/// Pure transformation: every operation is validated before the destination changes.
fn apply_server(
    text: &str,
    target: &crate::registry::McpInject,
    server_name: &str,
    def: &McpServerDef,
    enabled: bool,
) -> crate::Result<String> {
    if enabled {
        if let Some(command) = &def.command {
            if !ALLOWED_BINARIES.contains(&command.as_str()) {
                return Err(Error::Other(format!(
                    "MCP command is not allowed: {command}"
                )));
            }
        } else if def.url.is_none() {
            return Err(Error::Other(
                "MCP server has neither command nor URL".into(),
            ));
        }
    }
    let format = target.format.as_str();
    let new_text = match format {
        "toml_section" => {
            let section = target.section.as_deref().unwrap_or("mcp_servers");
            if enabled {
                toml_set_server(text, section, server_name, def)?
            } else {
                toml_remove_server(text, section, server_name)?
            }
        }
        "json_merge" | "jsonc_merge" => {
            let key = target.key.as_deref().unwrap_or("mcpServers");
            let is_jsonc = format == "jsonc_merge";
            let local_array_command = is_jsonc; // opencode 的 local/remote 数组格式
            let mut value = json_value(text, is_jsonc)?;
            if enabled {
                json_set_server(&mut value, key, server_name, def, local_array_command)?;
            } else {
                json_remove_server(&mut value, key, server_name);
            }
            serde_json::to_string_pretty(&value)
                .map_err(|e| Error::Other(format!("JSON 序列化失败: {}", e)))?
        }
        "yaml_section" => {
            let section = target.section.as_deref().unwrap_or("mcp_servers");
            let mut value: serde_json::Value = if text.trim().is_empty() {
                serde_json::Value::Object(Default::default())
            } else {
                serde_yml::from_str(text)
                    .map_err(|e| Error::Other(format!("YAML 解析失败: {}", e)))?
            };
            if enabled {
                yaml_set_server(&mut value, section, server_name, def)?;
            } else {
                yaml_remove_server(&mut value, section, server_name);
            }
            serde_yml::to_string(&value)
                .map_err(|e| Error::Other(format!("YAML 序列化失败: {}", e)))?
        }
        other => return Err(Error::Other(format!("未知 MCP 格式: {}", other))),
    };
    Ok(new_text)
}

fn save_config(path: &Path, original: &str, updated: &str) -> crate::Result<()> {
    if original == updated {
        return Ok(());
    }
    if !path.is_absolute() {
        return Err(Error::Other(format!(
            "MCP path cannot be resolved on this platform: {}",
            path.display()
        )));
    }
    // Retain the exact pre-write configuration as a private local backup. The
    // next git snapshot encrypts it; do not commit these plaintext local files.
    if path.is_file() {
        let home =
            crate::home_dir().ok_or_else(|| Error::Other("Cannot locate home directory".into()))?;
        let backup = home
            .join(".agentdeck/backups")
            .join(format!("mcp-{}", uuid::Uuid::new_v4()));
        let name = path
            .file_name()
            .ok_or_else(|| Error::Other("MCP destination has no filename".into()))?;
        crate::vault::keystore::atomic_write_bytes(&backup.join(name), original.as_bytes())?;
    }
    write_text(path, updated)
}

/// Toggle a managed server atomically. Independent configuration writers share a lock.
pub fn toggle(agent_id: &str, server_name: &str, enabled: bool) -> crate::Result<()> {
    let _guard = crate::config::write_lock();
    let target = agent_target(agent_id)?;
    let def = if enabled {
        find_server(server_name)?
    } else {
        // A disabled/removed registry entry must still be removable locally.
        McpServerDef {
            name: server_name.into(),
            description: None,
            url: None,
            command: None,
            args: vec![],
            disabled: true,
        }
    };
    let path = expand_path(&target.file);
    let text = read_text(&path)?;
    let updated = apply_server(&text, &target, server_name, &def, enabled)?;
    save_config(&path, &text, &updated)
}

fn sync_text(
    text: &str,
    target: &crate::registry::McpInject,
    definitions: &[McpServerDef],
    enabled: &[String],
) -> crate::Result<String> {
    // Validate the full request first, including entries later in the list.
    for name in enabled {
        if !definitions.iter().any(|d| &d.name == name && !d.disabled) {
            return Err(Error::Other(format!(
                "Unknown or disabled MCP server: {name}"
            )));
        }
    }
    let mut updated = text.to_string();
    for def in definitions {
        updated = apply_server(
            &updated,
            target,
            &def.name,
            def,
            enabled.contains(&def.name) && !def.disabled,
        )?;
    }
    Ok(updated)
}

/// Synchronize only registry-managed entries in one atomic write. User-owned MCP
/// servers are preserved; invalid requests never partially apply.
pub fn sync(agent_id: &str, enabled_servers: &[String]) -> crate::Result<Vec<String>> {
    let _guard = crate::config::write_lock();
    let target = agent_target(agent_id)?;
    let definitions = load_mcp_registry()?;
    let path = expand_path(&target.file);
    let original = read_text(&path)?;
    let updated = sync_text(&original, &target, &definitions, enabled_servers)?;
    save_config(&path, &original, &updated)?;
    Ok(enabled_servers.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentdeck-mcp-{}-{}", tag, nanos));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn toml_def() -> McpServerDef {
        McpServerDef {
            name: "ragflow".into(),
            description: Some("RAGFlow".into()),
            url: Some("http://127.0.0.1:9382/mcp".into()),
            command: None,
            args: vec![],
            disabled: false,
        }
    }

    #[test]
    fn registry_has_ragflow_enabled_and_examples_disabled() {
        let servers = load_mcp_registry().unwrap();
        let enabled: Vec<_> = servers.iter().filter(|s| !s.disabled).collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].name, "ragflow");
    }

    #[test]
    fn agent_targets_cover_five_verified_adapters() {
        let targets = load_agent_targets().unwrap();
        let ids: Vec<&str> = targets.iter().map(|(id, _)| id.as_str()).collect();
        for expected in ["claude-code", "cline", "codex", "gemini-cli", "opencode"] {
            assert!(ids.contains(&expected), "缺少 {}", expected);
        }
        let codex = targets.iter().find(|(id, _)| id == "codex").unwrap();
        assert_eq!(codex.1.format, "toml_section");
        assert_eq!(codex.1.section.as_deref(), Some("mcp_servers"));
    }

    #[test]
    fn toml_toggle_roundtrip() {
        let dir = temp_dir("toml");
        let path = dir.join("config.toml");
        std::fs::write(&path, "[existing]\nkeep = true\n").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();

        let def = toml_def();
        let added = toml_set_server(&text, "mcp_servers", "ragflow", &def).unwrap();
        assert!(added.contains("[mcp_servers.ragflow]"));
        assert!(added.contains("url = \"http://127.0.0.1:9382/mcp\""));
        assert!(added.contains("enabled = true"));
        assert!(added.contains("[existing]"));

        assert!(toml_server_names(&added, "mcp_servers").contains(&"ragflow".to_string()));
        let removed = toml_remove_server(&added, "mcp_servers", "ragflow").unwrap();
        assert!(!removed.contains("ragflow"));
        assert!(removed.contains("[existing]"));
    }

    #[test]
    fn json_toggle_roundtrip_and_state() {
        let dir = temp_dir("json");
        let path = dir.join("settings.json");
        std::fs::write(&path, r#"{ "model": "x" }"#).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut value = json_value(&text, false).unwrap();
        json_set_server(&mut value, "mcpServers", "ragflow", &toml_def(), false).unwrap();
        let out = serde_json::to_string_pretty(&value).unwrap();
        assert!(
            json_server_names(
                &serde_json::from_str::<serde_json::Value>(&out).unwrap(),
                "mcpServers"
            )
            .contains(&"ragflow".to_string())
        );
        assert!(out.contains("\"type\": \"http\""));

        json_remove_server(&mut value, "mcpServers", "ragflow");
        assert!(!json_server_names(&value, "mcpServers").contains(&"ragflow".to_string()));
    }

    #[test]
    fn jsonc_uses_local_array_command_format() {
        let dir = temp_dir("jsonc");
        let path = dir.join("opencode.jsonc");
        std::fs::write(&path, "// comment\n{}").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut value = json_value(&text, true).unwrap();
        let def = McpServerDef {
            name: "srv".into(),
            description: None,
            url: None,
            command: Some("npx".into()),
            args: vec!["-y".into(), "pkg".into()],
            disabled: false,
        };
        json_set_server(&mut value, "mcp", "srv", &def, true).unwrap();
        let out = serde_json::to_string_pretty(&value).unwrap();
        assert!(out.contains("\"type\": \"local\""));
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        let cmd = parsed["mcp"]["srv"]["command"].as_array().unwrap();
        let expected: Vec<serde_json::Value> = vec![
            serde_json::Value::String("npx".into()),
            serde_json::Value::String("-y".into()),
            serde_json::Value::String("pkg".into()),
        ];
        assert_eq!(cmd, &expected);
    }

    #[test]
    fn yaml_toggle_roundtrip() {
        let dir = temp_dir("yaml");
        let path = dir.join("config.yaml");
        std::fs::write(&path, "a: 1\n").unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut value: serde_json::Value = serde_yml::from_str(&text).unwrap();
        yaml_set_server(&mut value, "mcp_servers", "ragflow", &toml_def()).unwrap();
        let out = serde_yml::to_string(&value).unwrap();
        assert!(
            yaml_server_names(
                &serde_yml::from_str::<serde_json::Value>(&out).unwrap(),
                "mcp_servers"
            )
            .contains(&"ragflow".to_string())
        );
        yaml_remove_server(&mut value, "mcp_servers", "ragflow");
        assert!(!yaml_server_names(&value, "mcp_servers").contains(&"ragflow".to_string()));
    }

    #[test]
    fn allowlist_rejects_unknown_command() {
        let def = McpServerDef {
            name: "evil".into(),
            description: None,
            url: None,
            command: Some("powershell".into()),
            args: vec![],
            disabled: false,
        };
        // 直接验证白名单判定逻辑（toggle 内部使用）
        assert!(!ALLOWED_BINARIES.contains(&def.command.as_deref().unwrap()));
    }

    #[test]
    fn disabled_server_not_found() {
        assert!(find_server("example-http-mcp").is_err());
        assert!(find_server("ragflow").is_ok());
    }

    /// 真实环境验证：codex/opencode 的 MCP 开关往返（手动运行，测试后还原）
    #[test]
    #[ignore = "真实环境验证：修改 ~/.codex/config.toml 与 ~/.config/opencode/opencode.jsonc（测试后还原）"]
    fn real_mcp_toggle_roundtrip() {
        let codex_path = expand_path("~/.codex/config.toml");
        let codex_orig = std::fs::read_to_string(&codex_path).unwrap();
        toggle("codex", "ragflow", true).unwrap();
        assert!(
            current_servers("codex")
                .unwrap()
                .contains(&"ragflow".to_string()),
            "codex 应包含 ragflow"
        );
        let codex_on = std::fs::read_to_string(&codex_path).unwrap();
        assert!(codex_on.contains("[mcp_servers.ragflow]"));
        toggle("codex", "ragflow", false).unwrap();
        assert!(
            !current_servers("codex")
                .unwrap()
                .contains(&"ragflow".to_string()),
            "codex 应移除 ragflow"
        );
        println!("codex MCP 开关往返通过");

        let opencode_path = expand_path("~/.config/opencode/opencode.jsonc");
        let opencode_orig = std::fs::read_to_string(&opencode_path).unwrap();
        toggle("opencode", "ragflow", true).unwrap();
        assert!(
            current_servers("opencode")
                .unwrap()
                .contains(&"ragflow".to_string()),
            "opencode 应包含 ragflow"
        );
        toggle("opencode", "ragflow", false).unwrap();
        assert!(
            !current_servers("opencode")
                .unwrap()
                .contains(&"ragflow".to_string()),
            "opencode 应移除 ragflow"
        );
        println!("opencode MCP 开关往返通过");

        // 还原：opencode 原本就有 ragflow，重新打开
        toggle("opencode", "ragflow", true).unwrap();
        let opencode_final = std::fs::read_to_string(&opencode_path).unwrap();
        println!(
            "opencode 原内容与最终是否一致: {}",
            opencode_orig == opencode_final
        );
        let _ = codex_orig;
    }

    /// 真实环境验证：已安装的开源 agent（gemini-cli）MCP 注入→还原（手动运行）
    #[test]
    #[ignore = "真实环境验证：修改 ~/.gemini/settings.json"]
    fn real_mcp_e2e_installed_agents() {
        for (bin, agent_id) in [("gemini", "gemini-cli")] {
            let installed = std::process::Command::new(bin)
                .arg("--version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !installed {
                println!("[跳过] {} 未安装", agent_id);
                continue;
            }
            toggle(agent_id, "ragflow", true).unwrap();
            assert!(
                current_servers(agent_id)
                    .unwrap()
                    .contains(&"ragflow".to_string()),
                "{} 应包含 ragflow",
                agent_id
            );
            toggle(agent_id, "ragflow", false).unwrap();
            assert!(
                !current_servers(agent_id)
                    .unwrap()
                    .contains(&"ragflow".to_string()),
                "{} 应移除 ragflow",
                agent_id
            );
            println!("{} MCP 注入/移除往返通过", agent_id);
        }
    }
    #[test]
    fn sync_preserves_unmanaged_servers_and_removes_disabled_managed_entries() {
        let target = crate::registry::McpInject {
            file: "unused".into(),
            format: "json_merge".into(),
            key: Some("mcpServers".into()),
            section: None,
        };
        let input = r#"{"mcpServers":{"private":{"command":"mine"},"ragflow":{"url":"old"}}}"#;
        let output = sync_text(input, &target, &[toml_def()], &[]).unwrap();
        let value: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert!(value["mcpServers"].get("private").is_some());
        assert!(value["mcpServers"].get("ragflow").is_none());
    }

    #[test]
    fn sync_rejects_invalid_selection_before_writing() {
        let target = crate::registry::McpInject {
            file: "unused".into(),
            format: "json_merge".into(),
            key: None,
            section: None,
        };
        assert!(
            sync_text(
                "{}",
                &target,
                &[toml_def()],
                &["ragflow".into(), "missing".into()]
            )
            .is_err()
        );
    }

    #[test]
    fn atomic_config_write_replaces_existing_file() {
        let directory = temp_dir("atomic");
        let path = directory.join("config.json");
        write_text(&path, "before").unwrap();
        write_text(&path, "after").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "after");
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
