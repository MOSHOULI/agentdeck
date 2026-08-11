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

pub fn install_agent(_agent: &AgentDefinition) -> crate::Result<()> {
    // M1 实现：执行安装命令 + 验证 + 安全闸
    todo!("M1: 实现 Agent 安装")
}

pub fn uninstall_agent(_agent: &AgentDefinition) -> crate::Result<()> {
    todo!("M1: 实现 Agent 卸载")
}

pub fn verify_installation(_agent: &AgentDefinition) -> crate::Result<bool> {
    todo!("M1: 验证 Agent 是否安装成功")
}

pub fn check_auth(_agent: &AgentDefinition) -> crate::Result<bool> {
    todo!("M1: 检测 Agent 认证状态")
}
