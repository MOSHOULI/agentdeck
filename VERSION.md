# AgentDeck 版本记录

## v1.2.0（发布准备）

计划交付安装实例与来源证据、受限 npm 纯安装及安装后核验、CLI 同步守卫、目录元数据修正和 DSH 桌面/CLI 被动声明盘点。OpenCode 仅保留 V1 npm 安装途径与受支持的 Markdown 指令写入，模型/MCP 写入继续关闭，包括旧运行时注册表描述。核心版本及内置注册表下限同步为 1.2.0，保留自定义/更新的本地注册表及原签名信任链。

不覆盖历史 Release，不替换 v1.0.0 签名注册表的 Latest 入口。是否已发布及具体提交以 Release、`BUILD_COMMIT.txt` 和该提交的 CI 为准；真实 Agent 安装、GUI 与 DSH 执行仍未验证。详见 [中英文发布说明](docs/RELEASE-1.2.0.md)。

## v1.1.1（2026-11-06）

从包含已审查 Linux GLib 两行修复的源码构建新版 Windows 安装包；真实 GLib 三档优化回归和来源审计纳入 CI。Windows 安装包不包含受影响的 Linux GLib 依赖。未签名，真实安装及 GUI 验收限制不变。详见 [中英文发布说明](docs/RELEASE-1.1.1.md)。

## v1.1.0（2026-11-02）

安全与可靠性重构：密钥保留恢复、随机 nonce、事务更新、配置回滚、记忆撤销、真实 CLI、显式演示模式。安装包及 SHA256/构建提交由通过门禁的 Windows CI 生成。详见 [发布说明](docs/RELEASE-1.1.0.md)。

## v1.0.0（2026-08-11）

首个正式发布版本。

- 安装包：`AgentDeck_1.0.0_x64-setup.exe`（Windows NSIS）
- SHA256：`7C960F3BBA36B1D839E3AED6608C4787A1DC70D42D81D28693CD8DBAB55B30BC`
- 功能：Agent 广场 / 配置注入 / 记忆管理 / vault 加密 / git 备份回滚 / MCP 管理 / 在线更新
