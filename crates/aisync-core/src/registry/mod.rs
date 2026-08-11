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

/// 配置注入定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigInject {
    #[serde(default)]
    pub skills: bool,
    #[serde(default)]
    pub mcp: bool,
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

/// 加载 Agent 注册表
pub fn load_registry(path: &std::path::Path) -> crate::Result<RegistryDoc> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| crate::Error::Other(format!("无法读取注册表文件: {}", e)))?;

    let doc: RegistryDoc = serde_json::from_str(&content)
        .map_err(|e| crate::Error::Other(format!("注册表 JSON 解析失败: {}", e)))?;

    Ok(doc)
}

/// 按热度排序的 agent 定义列表（已安装优先 → popular → stars 降序）
pub fn sorted_agents(
    registry: &RegistryDoc,
    installed_ids: &[String],
) -> Vec<(&String, &AgentDefinition)> {
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
