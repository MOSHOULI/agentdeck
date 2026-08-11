# AgentDeck（AI 中枢）

AgentDeck 是一款跨平台桌面应用，帮助你**一站式发现、安装、配置和管理本地 AI 代理工具**。它把安装、配置注入、对话记忆、密钥加密、MCP 服务器管理整合到一个界面里。

## 功能特性

- **Agent 广场**：卡片式浏览支持的 AI Agent，一键安装 / 卸载 / 验证
- **配置注入**：自动把个性化模板与模型配置写入各 Agent 的配置文件（JSON / JSONC / TOML / YAML），写入前自动备份，失败自动回滚
- **统一记忆**：采集 Codex / Claude Code / OpenCode 的会话历史，全文搜索，跨 Agent 注入记忆文件；支持定时采集与自动标签注入
- **密钥库（vault）**：API Key 用 AES-256-GCM 加密存储，主密钥由 Windows DPAPI 保护；支持从旧版 vault 一键迁移
- **备份回滚**：每次安装 / 注入 / 密钥变动前自动生成 git 备份，支持按操作回滚或按时间点整体恢复；备份内容加密后入库
- **MCP 管理**：内置 MCP 服务器注册表，按 Agent 独立开关，实时写回各 Agent 的 MCP 配置
- **在线更新**：启动时静默检查注册表更新，ed25519 签名校验，一键应用

## 安装

### 下载安装包（推荐）

前往 [GitHub Releases](https://github.com/MOSHOULI/agentdeck/releases) 下载最新的 Windows 安装包（`AgentDeck_x.y.z_x64-setup.exe`），双击运行，按向导安装即可。安装包支持中文 / English 界面。

### 从源码构建

环境要求：

- Windows 10 / 11
- Rust（MSVC 工具链）
- Node.js 18+ 与 npm
- Tauri CLI

```powershell
# 克隆仓库
git clone https://github.com/MOSHOULI/agentdeck.git
cd agentdeck

# 启动开发窗口
cargo tauri dev

# 构建安装包（NSIS）
cargo tauri build
```

## 快速开始

1. 启动 AgentDeck，在「Agent 广场」选择要使用的 Agent，点击安装（安装与卸载前会弹出确认）
2. 安装完成后，配置注入会自动写入对应 Agent 的配置文件（写入前有备份）
3. 在「记忆管理」面板点击「采集」，同步 Codex / Claude / OpenCode 的会话历史；勾选「自动注入」的记忆会在每次采集后自动写入记忆文件
4. 在「密钥与备份」面板添加 API Key（加密存储，界面只显示脱敏内容），并可查看历史备份、执行回滚
5. 在「MCP 服务器」面板按 Agent 开启需要的 MCP 服务器，配置实时生效
6. 在「在线更新」面板检查并应用注册表更新

## 数据与安全

- 应用数据统一存放在用户主目录下的 `~/.agentdeck/`：记忆库（`memory.db`）、密钥库（`vault/`）、备份仓库（`git-backup/`）、在线更新注册表（`registry/`）
- API Key 以密文存储，明文绝不落盘；主密钥由 Windows DPAPI 绑定当前用户保护
- 配置写入前自动备份，注入失败自动回滚；备份文件加密后进入备份仓库，提交前做密钥特征扫描
- 安装 / 卸载 / 删除密钥 / 回滚等操作均有确认提示

## 常见问题

**采集不到某个 Agent 的记忆？**

确认对应 Agent 已安装并产生过会话记录。当前支持 Codex（`~/.codex/sessions`）、Claude Code（`~/.claude/projects`）、OpenCode（`~/.local/share/opencode`）。

**在线更新提示失败？**

更新源需要能访问 GitHub Releases；网络受限时请配置系统代理后重试。

**如何迁移旧版 vault 密钥？**

在「密钥与备份」面板点击「导入 V2 vault 密钥」，从旧版 vault 目录迁移（同名密钥不会被覆盖）。

## 更新与反馈

- 发布与更新：[GitHub Releases](https://github.com/MOSHOULI/agentdeck/releases)
- 问题反馈：欢迎在 [Issues](https://github.com/MOSHOULI/agentdeck/issues) 提交
