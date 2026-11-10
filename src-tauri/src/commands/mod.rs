pub mod backup;
/// AgentDeck Tauri 命令层。
///
/// 每个 Tauri command 对应前端 bridge.ts 的一个函数。
/// M0 骨架：仅健康检查。M1-M6 逐步添加 deploy/memory/vault/sync/backup 命令。
pub mod deploy;
pub mod dsh;
pub mod memory;
pub mod sync;
pub mod update;
pub mod vault;

/// 健康检查——验证后端可用
#[tauri::command]
pub fn health_check() -> String {
    format!("AgentDeck v{} OK", env!("CARGO_PKG_VERSION"))
}

pub mod qwen;

pub mod qoder;

pub mod qoder_scopes;

pub mod dsh_distribution;

pub mod codex_animations;

mod install_session;
