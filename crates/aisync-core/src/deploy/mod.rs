/// Agent 部署模块（M1-M2）。
///
/// 负责 Agent 的安装、卸载、版本验证、认证检测。
/// 安装命令走白名单校验，执行前需用户确认。
///
/// 安装流程（闭源+账号登录型 Agent，如 Codex）：
///   1. 执行 install.command（如 npm install -g @openai/codex）
///   2. verify_cmd 验证安装成功
///   3. 检测 auth_guide.completion_check 判断认证是否完成
///   4. 若未完成：弹窗引导用户手动登录
///   5. 轮询/监听 completion_check 直到认证完成
///   6. 触发 config inject（配置注入）
use crate::registry::AgentDefinition;

use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

/// 执行注册表里的 shell 命令（Windows 用 cmd /C）
fn run_shell(cmdline: &str) -> std::io::Result<()> {
    #[cfg(windows)]
    let status = Command::new("cmd").args(["/C", cmdline]).status()?;
    #[cfg(not(windows))]
    let status = Command::new("sh").args(["-c", cmdline]).status()?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "命令退出码 {}",
            status.code().unwrap_or(-1)
        )))
    }
}

/// 安装 Agent：执行 install.command → verify_cmd 验证
pub fn install_agent(_agent: &AgentDefinition) -> crate::Result<()> {
    let cmd = _agent
        .install
        .command
        .as_deref()
        .ok_or_else(|| crate::Error::InstallFailed(format!("{} 缺少安装命令", _agent.name)))?;
    run_shell(cmd)
        .map_err(|e| crate::Error::InstallFailed(format!("安装 {} 失败: {}", _agent.name, e)))?;
    if !verify_installation(_agent)? {
        return Err(crate::Error::InstallFailed(format!(
            "安装 {} 后验证失败",
            _agent.name
        )));
    }
    Ok(())
}

/// 安装 Agent 并执行配置注入；注入失败时卸载回滚（原子化流程）
pub fn install_agent_with_inject(
    agent: &AgentDefinition,
    source_dir: &std::path::Path,
) -> crate::Result<()> {
    install_agent(agent)?;
    if let Err(e) = crate::config::inject_for_agent(agent, source_dir) {
        let _ = uninstall_agent(agent);
        return Err(e);
    }
    Ok(())
}

/// 卸载 Agent：执行 uninstall_cmd
pub fn uninstall_agent(_agent: &AgentDefinition) -> crate::Result<()> {
    let cmd = _agent
        .install
        .uninstall_cmd
        .as_deref()
        .ok_or_else(|| crate::Error::InstallFailed(format!("{} 缺少卸载命令", _agent.name)))?;
    run_shell(cmd)
        .map_err(|e| crate::Error::InstallFailed(format!("卸载 {} 失败: {}", _agent.name, e)))?;
    Ok(())
}

/// 验证 Agent 是否安装成功：执行 verify_cmd，3 秒超时
pub fn verify_installation(_agent: &AgentDefinition) -> crate::Result<bool> {
    let Some(verify_cmd) = _agent.install.verify_cmd.clone() else {
        return Ok(false);
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(run_shell(&verify_cmd).is_ok());
    });
    Ok(rx.recv_timeout(Duration::from_secs(3)).unwrap_or(false))
}

/// 检测 Agent 认证状态（M1 简化：统一返回未认证）
pub fn check_auth(_agent: &AgentDefinition) -> crate::Result<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{AuthType, ConfigInject, InstallConfig, InstallMethod, UiType};

    fn fake_agent() -> AgentDefinition {
        AgentDefinition {
            id: "fake".into(),
            name: "Fake".into(),
            source: "open".into(),
            license: None,
            github: None,
            stars: Some(1),
            ui_type: UiType::Cli,
            category: "test".into(),
            popular: false,
            description: "测试用".into(),
            install: InstallConfig {
                method: InstallMethod::Manual,
                command: Some("echo installed".into()),
                url: None,
                verify_cmd: Some("echo ok".into()),
                uninstall_cmd: Some("echo uninstalled".into()),
                dependencies: vec![],
            },
            auth_type: AuthType::None,
            auth_guide: None,
            config_inject: ConfigInject {
                skills: false,
                mcp: None,
                memory: false,
                personalization: None,
                model_config: None,
            },
            memory_collector: None,
            icon: "t".into(),
            homepage: None,
        }
    }

    #[test]
    fn install_verify_uninstall_roundtrip_with_echo() {
        let agent = fake_agent();
        install_agent(&agent).expect("echo 安装命令应成功");
        assert!(verify_installation(&agent).unwrap());
        uninstall_agent(&agent).expect("echo 卸载命令应成功");
    }

    fn npm_has_opencode() -> bool {
        run_shell("npm ls -g --depth=0 opencode-ai").is_ok()
    }

    /// 真实环境验证：npm 全局安装 opencode-ai → 验证 → 卸载（需要手动运行）
    #[test]
    #[ignore = "真实环境验证：执行 npm install/uninstall -g opencode-ai"]
    fn real_install_roundtrip() {
        let doc =
            crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
                .unwrap();
        let opencode = doc.agents.get("opencode").expect("注册表应有 opencode");

        assert!(
            !npm_has_opencode(),
            "前置条件：npm 全局不应已有 opencode-ai"
        );
        install_agent(opencode).expect("真实安装应成功");
        assert!(verify_installation(opencode).unwrap());
        assert!(npm_has_opencode());

        uninstall_agent(opencode).expect("真实卸载应成功");
        assert!(!npm_has_opencode(), "卸载后 npm 全局不应再有 opencode-ai");
    }
}
