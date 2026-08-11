pub mod config;
/// AgentDeck 核心引擎库（aisync-core）
///
/// 提供 Agent 部署、记忆管理、密钥加密、配置同步等基础能力。
/// 本库不依赖 Tauri，纯 Rust 实现，可供 CLI 和桌面端共用。
pub mod deploy;
pub mod git;
pub mod memory;
pub mod registry;
pub mod sync;
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
