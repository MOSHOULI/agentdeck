pub mod config;
/// AgentDeck 核心引擎库（aisync-core）
///
/// 提供 Agent 部署、记忆管理、密钥加密、配置同步等基础能力。
/// 本库不依赖 Tauri，纯 Rust 实现，可供 CLI 和桌面端共用。
pub mod deploy;
pub mod dsh;
pub mod git;
pub mod mcp;
pub mod memory;
pub mod registry;
pub mod release;
pub mod sync;
pub mod update;
pub mod vault;

/// AgentDeck 错误类型
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Agent 未找到: {0}")]
    AgentNotFound(String),

    #[error("安装失败: {0}")]
    InstallFailed(String),

    #[error("配置注入失败: {0}")]
    InjectFailed(String),

    #[error("Git 操作失败: {0}")]
    GitFailed(String),

    #[error("Vault 错误: {0}")]
    VaultError(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Resolve the user profile used by all application data and source adapters.
/// The explicit absolute override is used by the isolated test runner on every
/// platform; Windows Known Folder APIs ignore HOME/USERPROFILE overrides.
pub fn home_dir() -> Option<std::path::PathBuf> {
    resolve_home(std::env::var_os("AGENTDECK_TEST_HOME"), dirs::home_dir)
}

fn resolve_home(
    override_path: Option<std::ffi::OsString>,
    system_home: impl FnOnce() -> Option<std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    match override_path {
        Some(path) => {
            let path = std::path::PathBuf::from(path);
            path.is_absolute().then_some(path)
        }
        None => system_home(),
    }
}

#[cfg(test)]
mod home_tests {
    use super::*;
    #[test]
    fn explicit_home_wins_without_calling_platform_profile_api() {
        let temporary = std::env::temp_dir();
        assert_eq!(
            resolve_home(Some(temporary.clone().into_os_string()), || panic!(
                "must not consult real profile"
            )),
            Some(temporary)
        );
        assert_eq!(
            resolve_home(Some("relative".into()), || panic!("must fail closed")),
            None
        );
        assert_eq!(resolve_home(None, || None), None);
    }
}
