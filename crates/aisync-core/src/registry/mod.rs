/// Agent 注册表模块。
///
/// 加载 `agents/registry-v3.json`，提供 agent 定义查询。
/// 支持本地缓存和在线更新（GitHub Releases 合并）。
use serde::{Deserialize, Serialize};

/// Agent 安装方式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InstallMethod {
    Npm,
    Pip,
    Download,
    Winget,
    VscodeExtension,
    Manual,
}

/// Agent 认证方式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    None,
    ApiKey,
    OauthCli,
    AccountLogin,
    License,
}

/// Agent 界面形态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum UiType {
    All,
    #[serde(rename = "cli+desktop")]
    CliDesktop,
    Cli,
    Desktop,
    #[serde(rename = "vscode-extension")]
    VscodeExtension,
    #[serde(rename = "ide-extension")]
    IdeExtension,
    #[serde(rename = "cli-bot")]
    CliBot,
}

/// 安装配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallConfig {
    pub method: InstallMethod,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub verify_cmd: Option<String>,
    #[serde(default)]
    pub uninstall_cmd: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// 认证引导
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthGuide {
    pub steps: Vec<String>,
    #[serde(default)]
    pub completion_check: Option<serde_json::Value>,
}

/// 个性化注入配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalizationInject {
    pub template: String,
    pub target: String,
}

/// 模型配置注入
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfigInject {
    pub file: String,
    pub format: String,
}

/// MCP 配置注入目标（M5）：每款支持 MCP 的 agent 定义注入文件与格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInject {
    /// 目标配置文件（支持 ~ 展开）
    pub file: String,
    /// toml_section | json_merge | jsonc_merge | yaml_section
    pub format: String,
    /// JSON/JSONC 的服务器对象键（如 mcpServers / mcp）
    #[serde(default)]
    pub key: Option<String>,
    /// TOML/YAML 的段名（默认 mcp_servers）
    #[serde(default)]
    pub section: Option<String>,
}

/// 配置注入定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigInject {
    #[serde(default)]
    pub skills: bool,
    #[serde(default)]
    pub mcp: Option<McpInject>,
    #[serde(default)]
    pub memory: bool,
    #[serde(default)]
    pub personalization: Option<PersonalizationInject>,
    #[serde(default)]
    pub model_config: Option<ModelConfigInject>,
}

/// Agent 定义（从 registry-v3.json 解析）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub source: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub github: Option<String>,
    #[serde(default)]
    pub stars: Option<u64>,
    pub ui_type: UiType,
    pub category: String,
    #[serde(default)]
    pub popular: bool,
    pub description: String,
    pub install: InstallConfig,
    pub auth_type: AuthType,
    #[serde(default)]
    pub auth_guide: Option<AuthGuide>,
    pub config_inject: ConfigInject,
    #[serde(default)]
    pub memory_collector: Option<String>,
    pub icon: String,
    #[serde(default)]
    pub homepage: Option<String>,
}

/// 注册表文档根
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryDoc {
    pub version: u32,
    pub description: String,
    pub updated_at: String,
    pub agents: std::collections::HashMap<String, AgentDefinition>,
    #[serde(default)]
    pub future_registry: Option<FutureRegistry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FutureRegistry {
    pub description: String,
    pub agents: std::collections::HashMap<String, serde_json::Value>,
}

/// 从 JSON 文本解析注册表，并用 map key 回填每条 agent 的 id
pub fn parse_registry(content: &str) -> crate::Result<RegistryDoc> {
    let mut doc: RegistryDoc = serde_json::from_str(content)
        .map_err(|e| crate::Error::Other(format!("注册表 JSON 解析失败: {}", e)))?;

    for (key, agent) in doc.agents.iter_mut() {
        agent.id = key.clone();
    }
    Ok(doc)
}

/// 加载 Agent 注册表
pub fn load_registry(path: &std::path::Path) -> crate::Result<RegistryDoc> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| crate::Error::Other(format!("无法读取注册表文件: {}", e)))?;
    parse_registry(&content)
}

/// 按热度排序的 agent 定义列表（已安装优先 → popular → stars 降序）
pub fn sorted_agents<'a>(
    registry: &'a RegistryDoc,
    installed_ids: &[String],
) -> Vec<(&'a String, &'a AgentDefinition)> {
    let mut agents: Vec<_> = registry.agents.iter().collect();
    agents.sort_by(|(id_a, a), (id_b, b)| {
        let a_installed = installed_ids.contains(id_a);
        let b_installed = installed_ids.contains(id_b);
        b_installed
            .cmp(&a_installed)
            .then_with(|| b.popular.cmp(&a.popular))
            .then_with(|| b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0)))
    });
    agents
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY_JSON: &str = include_str!("../../../../agents/registry-v3.json");

    #[test]
    fn parses_registry_and_backfills_ids() {
        let doc = parse_registry(REGISTRY_JSON).expect("注册表应可解析");
        assert_eq!(doc.agents.len(), 9);
        for (key, agent) in &doc.agents {
            assert_eq!(&agent.id, key, "id 应与 map key 一致");
            assert!(!agent.name.is_empty());
            assert!(!agent.description.is_empty());
            assert!(!agent.icon.is_empty());
        }
    }

    #[test]
    fn sorted_agents_orders_popular_then_stars() {
        let doc = parse_registry(REGISTRY_JSON).unwrap();
        let list = sorted_agents(&doc, &[]);

        let popular_flags: Vec<bool> = list.iter().map(|(_, a)| a.popular).collect();
        let mut expected = popular_flags.clone();
        expected.sort_by(|a, b| b.cmp(a));
        assert_eq!(popular_flags, expected, "popular 应排在前面");

        let stars: Vec<u64> = list.iter().map(|(_, a)| a.stars.unwrap_or(0)).collect();
        let mut sorted = stars.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(stars, sorted, "stars 应降序");
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_registry("{ not json").is_err());
    }
}
