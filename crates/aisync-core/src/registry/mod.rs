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
    #[serde(default)]
    pub extension_id: Option<String>,
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

    if doc.version != 3 {
        return Err(crate::Error::Other(format!(
            "Unsupported registry schema: {}",
            doc.version
        )));
    }
    for (key, agent) in doc.agents.iter_mut() {
        if key.is_empty()
            || key == "."
            || key == ".."
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return Err(crate::Error::Other(format!("Invalid agent id: {key}")));
        }
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

/// Explicit data locations keep tests and callers independent of process HOME.
#[derive(Debug, Clone)]
pub struct DataPaths {
    pub registry_dir: std::path::PathBuf,
}

impl DataPaths {
    pub fn from_data_dir(data_dir: impl AsRef<std::path::Path>) -> Self {
        Self {
            registry_dir: data_dir.as_ref().join("registry"),
        }
    }

    pub fn discover() -> crate::Result<Self> {
        let home =
            crate::home_dir().ok_or_else(|| crate::Error::Other("无法定位用户主目录".into()))?;
        Ok(Self::from_data_dir(home.join(".agentdeck")))
    }
}

/// Serializes readers and writers across processes. OS releases the lock on crash;
/// never unlink the inode, which could let another process lock a different file.
pub(crate) fn lock_and_recover(paths: &DataPaths) -> crate::Result<std::fs::File> {
    let parent = paths
        .registry_dir
        .parent()
        .ok_or_else(|| crate::Error::Other("Invalid registry directory".into()))?;
    std::fs::create_dir_all(parent)?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(parent.join("registry-update.lock"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(std::fs::TryLockError::WouldBlock) => {
                if std::time::Instant::now() >= deadline {
                    return Err(crate::Error::Other("Registry lock timed out; retry".into()));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(std::fs::TryLockError::Error(error)) => return Err(error.into()),
        }
    }
    recover_locked(paths)?;
    Ok(lock)
}

#[derive(Serialize, Deserialize)]
pub(crate) struct UpdateJournal {
    pub stage: String,
    pub backup: String,
    pub had_previous: bool,
}

pub(crate) fn sync_directory(path: &std::path::Path) -> crate::Result<()> {
    #[cfg(unix)]
    std::fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn recover_locked(paths: &DataPaths) -> crate::Result<()> {
    let parent = paths.registry_dir.parent().unwrap();
    let journal_path = parent.join("registry-update.json");
    if journal_path
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(crate::Error::Other(
            "Registry recovery journal must not be a symbolic link".into(),
        ));
    }
    let data = match std::fs::read(&journal_path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let journal: UpdateJournal = serde_json::from_slice(&data)
        .map_err(|e| crate::Error::Other(format!("Invalid registry recovery journal: {e}")))?;
    for (name, prefix) in [
        (&journal.stage, ".registry-stage-"),
        (&journal.backup, "registry-backup-"),
    ] {
        if !name
            .strip_prefix(prefix)
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
        {
            return Err(crate::Error::Other("Unsafe registry recovery path".into()));
        }
    }
    let backup = parent.join(&journal.backup);
    for path in [&backup, &parent.join(&journal.stage), &paths.registry_dir] {
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err(crate::Error::Other(
                "Registry recovery directories must not be symbolic links".into(),
            ));
        }
    }
    if !paths.registry_dir.try_exists()? && journal.had_previous {
        // An interrupted commit rolls back rather than exposing embedded data.
        std::fs::rename(&backup, &paths.registry_dir)?;
        sync_directory(parent)?;
    }
    let stage = parent.join(&journal.stage);
    if stage.try_exists()? {
        std::fs::remove_dir_all(stage)?;
    }
    std::fs::remove_file(journal_path)?;
    sync_directory(parent)?;
    Ok(())
}

pub fn runtime_registry_dir() -> crate::Result<std::path::PathBuf> {
    Ok(DataPaths::discover()?.registry_dir)
}

pub fn load_registry_runtime() -> crate::Result<RegistryDoc> {
    load_registry_at(&DataPaths::discover()?)
}

pub fn load_registry_at(paths: &DataPaths) -> crate::Result<RegistryDoc> {
    let _lock = lock_and_recover(paths)?;
    installed_version_locked(paths, env!("CARGO_PKG_VERSION"))?;
    const BUILTIN: &str = include_str!("../../../../agents/registry-v3.json");
    let runtime = paths.registry_dir.join("registry-v3.json");
    match std::fs::read_to_string(runtime) {
        Ok(content) => parse_registry(&content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => parse_registry(BUILTIN),
        Err(e) => Err(e.into()),
    }
}

pub fn load_mcp_registry_runtime() -> crate::Result<String> {
    load_mcp_registry_at(&DataPaths::discover()?)
}

pub fn load_mcp_registry_at(paths: &DataPaths) -> crate::Result<String> {
    let _lock = lock_and_recover(paths)?;
    installed_version_locked(paths, env!("CARGO_PKG_VERSION"))?;
    const BUILTIN: &str = include_str!("../../../../agents/mcp-registry.json");
    match std::fs::read_to_string(paths.registry_dir.join("mcp-registry.json")) {
        Ok(content) => Ok(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BUILTIN.to_string()),
        Err(e) => Err(e.into()),
    }
}

/// The manifest version travels in the same directory transaction as its files.
pub fn installed_version_at(paths: &DataPaths, builtin: &str) -> crate::Result<String> {
    let _lock = lock_and_recover(paths)?;
    installed_version_locked(paths, builtin)
}

pub(crate) fn installed_version_locked(paths: &DataPaths, builtin: &str) -> crate::Result<String> {
    match std::fs::read_to_string(paths.registry_dir.join("installed-version")) {
        Ok(version) if crate::update::valid_version(version.trim()) => {
            for name in crate::update::REGISTRY_FILES {
                if !paths.registry_dir.join(name).is_file() {
                    return Err(crate::Error::Other(format!(
                        "Installed registry is incomplete: {name}"
                    )));
                }
            }
            Ok(version.trim().to_string())
        }
        Ok(_) => Err(crate::Error::Other(
            "Invalid installed registry version".into(),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Pre-versioned installations cannot truthfully claim the binary's
            // embedded data version. Treat them as an unknown legacy baseline.
            if crate::update::REGISTRY_FILES
                .iter()
                .any(|name| paths.registry_dir.join(name).exists())
            {
                Ok("0.0.0".to_string())
            } else {
                Ok(builtin.to_string())
            }
        }
        Err(e) => Err(e.into()),
    }
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
    #[test]
    fn runtime_loader_prefers_installed_and_only_falls_back_when_absent() {
        let root =
            std::env::temp_dir().join(format!("agentdeck-registry-test-{}", uuid::Uuid::new_v4()));
        let paths = DataPaths::from_data_dir(&root);
        assert!(!load_registry_at(&paths).unwrap().agents.is_empty());
        std::fs::create_dir_all(&paths.registry_dir).unwrap();
        let mut doc = parse_registry(REGISTRY_JSON).unwrap();
        doc.description = "runtime marker".into();
        std::fs::write(
            paths.registry_dir.join("registry-v3.json"),
            serde_json::to_vec(&doc).unwrap(),
        )
        .unwrap();
        assert_eq!(
            load_registry_at(&paths).unwrap().description,
            "runtime marker"
        );
        std::fs::write(paths.registry_dir.join("registry-v3.json"), "invalid").unwrap();
        assert!(load_registry_at(&paths).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn concurrent_readers_wait_for_short_lived_writer_without_spurious_errors() {
        let root = std::env::temp_dir().join(format!(
            "agentdeck-registry-concurrent-{}",
            uuid::Uuid::new_v4()
        ));
        let paths = DataPaths::from_data_dir(&root);
        let lock = lock_and_recover(&paths).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(9));
        let readers: Vec<_> = (0..8)
            .map(|_| {
                let paths = paths.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    assert!(!load_registry_at(&paths).unwrap().agents.is_empty());
                    assert!(!load_mcp_registry_at(&paths).unwrap().is_empty());
                    assert_eq!(installed_version_at(&paths, "1.1.0").unwrap(), "1.1.0");
                })
            })
            .collect();
        barrier.wait();
        std::thread::sleep(std::time::Duration::from_millis(80));
        drop(lock);
        for reader in readers {
            reader.join().unwrap();
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
