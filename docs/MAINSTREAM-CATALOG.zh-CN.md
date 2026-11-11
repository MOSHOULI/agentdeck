# 主流产品官方指引（2026-11-08）

[English](MAINSTREAM-CATALOG.md)

内置目录新增四个产品家族的六个**手动指引条目**：Cursor Desktop、Cursor Agent CLI、GitHub Copilot CLI、Kimi Code CLI、Kimi Code Desktop 与 Qoder CLI。目录合计 17 条，不等于 17 款已经端到端接入的应用。CLI 与桌面是独立安装形态，不能从一个的状态推断另一个。

六项的安装、验证、卸载命令与认证检查均为空；技能、MCP/配置写入、记忆导出与采集全部关闭。不扫描 PATH，不执行 Agent、npm、安装器、迁移或登录。安装、版本、来源与认证保持未知。界面可在“仅手动安装”和“待确认状态”中找到，不能进入“已安装”或“当前可自动安装”。本次只更新内置目录，不覆盖用户保留的自定义运行时注册表，也不发布新的注册表更新源。

## Cursor

- [桌面官方下载](https://cursor.com/download)：Windows x64/ARM64、macOS Intel/Apple Silicon、Linux DEB/RPM/AppImage。用户选择适合其系统的官方包。
- [当前 CLI 安装文档](https://cursor.com/docs/cli/installation)：支持 macOS、Linux、WSL 和原生 Windows。当前命令为 `agent`，不是旧文档中的 `cursor-agent`。Shell/PowerShell 安装器只作为用户自行操作的官方指引。
- [CLI 认证](https://cursor.com/docs/cli/reference/authentication)：`agent login` 浏览器登录或 `CURSOR_API_KEY`。

`agent` 是通用命令名，同名文件不能作为 Cursor 身份证据。旧 docs.cursor.com 与当前 cursor.com 文档可能不同；本次使用当前页面。桌面和 CLI 分开列出，不推断安装或登录状态，不声称它们使用开源许可证。

## GitHub Copilot CLI

- [官方安装](https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/install-copilot-cli)：独立 `copilot`，npm 包为 `@github/copilot`（Node.js 22+）；也可使用 Windows WinGet、macOS/Linux Homebrew 或官方 macOS/Linux 脚本。Windows 要求 PowerShell 6+。
- [认证](https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/authenticate-copilot-cli)：`copilot login` 提供 OAuth，官方还支持指定环境 token 与 GitHub CLI fallback。BYOK 模式可无需 GitHub 认证；权益和组织策略依实际模式而定。
- [官方仓库](https://github.com/github/copilot-cli)与[自定义许可证](https://github.com/github/copilot-cli/blob/main/LICENSE.md)：公开仓库不代表二进制是 MIT/Apache 开源软件。

此项不是旧 `gh-copilot` 扩展，也不是 Copilot IDE 插件。官方 npm 安装依赖生命周期脚本，不能套用其他适配器的 `--ignore-scripts` 后声称安装可用。

## Kimi Code

- [当前 CLI 仓库](https://github.com/MoonshotAI/kimi-code)：TypeScript/Node.js、MIT。[入门文档](https://www.kimi.com/code/docs/en/kimi-code-cli/guides/getting-started)说明 npm 包为 `@moonshot-ai/kimi-code`，npm 途径要求 Node.js 22.19.0+；独立安装器无需预装 Node。Windows 首次运行需要 Git for Windows/Git Bash。会话内 `/login` 提供 Kimi Code OAuth 或 Kimi Platform API key。
- [旧 kimi-cli 仓库](https://github.com/MoonshotAI/kimi-cli)已归档。两代均叫 `kimi`，不能靠文件名识别版本。新数据默认在 `~/.kimi-code`，支持 `KIMI_CODE_HOME`；旧数据在 `~/.kimi`。[迁移](https://www.kimi.com/code/docs/en/kimi-code-cli/guides/migration.html)由用户决定，OAuth 凭据与 MCP 授权不会迁移，AgentDeck 不执行迁移。
- [当前桌面文档](https://www.kimi.com/code/docs/en/kimi-code-desktop/getting-started.html)与[安装包 FAQ](https://www.kimi.com/code/docs/en/kimi-code-desktop/help.html)：macOS 13+、Windows 10+ 64 位、x86_64 Linux。旧帮助中心页面仍写 macOS 12 且未列 Linux，本次采用团队维护的完整文档。桌面 OAuth、账号权益及第三方模型设置仍由用户处理，不将 CLI 的 MIT 许可证套用到桌面二进制。

## 验证边界

这只是截至检查日期的官方文档核实，不是安装包、版本或发布者签名验证。未测试真实安装、npm 发布完整性、登录、平台 GUI 或 Agent 运行。回归覆盖内置元数据、零探针回调、执行/配置能力关闭，以及手动/未知状态界面呈现。没有增加自动适配器或凭据。以后上游版本或文档变化，须重新审查后才能扩大能力。

## 2026-11-08 既有与参考条目复核

- [Claude Code](https://github.com/anthropics/claude-code/blob/0ed94698aa0aeee471465fc9c867c2e8434c4d68/README.md)：官方已弃用 npm 安装，改推荐原生安装器、Homebrew 或 WinGet。目录改为官方手动安装指引，不再自动执行旧 npm 配方。保留既有配置适配器，但不表示新增原生包管理或运行验证。移除未经验证的凭据文件登录判断。
- [Gemini CLI](https://github.com/google-gemini/gemini-cli/blob/62bab5b232c15b925d31e655ce844ab939c882b7/README.md)：当前 README 面向企业 Code Assist 与旧版付费 API/Vertex。个人免费及 Google AI Pro/Ultra 于 2026-06-18 转至 Antigravity CLI。保留既有 CLI 安装配方用于兼容，不把它当个人用户的新推荐；尚未接入 Antigravity。移除 auth.json 登录判断，不用另一凭据文件存在检查替代。
- [Qwen Code](https://github.com/QwenLM/qwen-code)：上游已有 CLI 之外的形态，AgentDeck 仍仅提供既定的手动 CLI 指引与被动发现，不管理桌面、IDE 或 WebUI。
- [OpenCode](https://opencode.ai/docs/migrate-v1)：保留明确的 V1 包与兼容边界，不宣称 V2 安装或模型/MCP 管理。
- [Kilo](https://github.com/Kilo-Org/kilocode) 与 [Aider](https://github.com/Aider-AI/aider)：列为下一轮适配评审候选，本轮不新增自动管理，也不以此作流行度排名。
- [Continue](https://github.com/continuedev/continue)：README 声明只读、不再积极维护，并有最终 2.0.0 版；不推荐为仍积极维护的默认选择。
- [Plandex](https://github.com/plandex-ai/plandex)：云服务收尾、不接收新用户，自托管/本地说明仍保留；不等于所有本地代码均不可用。
- [Roo Code](https://github.com/RooCodeInc/Roo-Code)：2026-05-15 归档，README 声明扩展停运；保留历史参考，不推荐为当前默认。

为兼容保留既有 ID 与参考条目。上游产品状态与 AgentDeck 管理能力分别判断，不用星数或未经核实的采用量代替适用性证据。

Claude 保留既有只读安装检查及配置适配器，未新增验证原生包识别；即使旧运行时注册表仍提供 npm 配方，固定安装器也拒绝该弃用渠道。

## Qoder CLI

另有独立的原生分发/私有部署工作流可显式选择 1.1.85（默认）或 1.1.86；它不把此手动目录条目升级为通用安装器，也不声明 Qoder 运行或认证验收。

[官方安装说明](https://docs.qoder.com/cli/installation) 首选原生独立安装器；`@qoder-ai/qodercli` 为旧 npm 兼容路线，需要 Node.js 20+，不作为新安装首选。命令为 `qoder`，Windows ARM64 文档仍称不支持，但已审阅的 1.1.85 清单与安装脚本列出原生 ARM64 包，实际兼容性未验收。[自定义模型](https://docs.qoder.com/cli/custom-models) 使用 `/model` 向导。本条仅链接官方说明，不启用可执行文件检测、凭据注入、安装、配置或桌面管理。

## 其余既有条目：带日期的维护证据

以下为 2026-11-08 对上游发布/来源的观察，不是已安装版本测量，也不是流行度背书。

| 条目 | 官方维护/来源证据 | AgentDeck 边界 |
| --- | --- | --- |
| Codex | [0.167.0 发布](https://github.com/openai/codex/releases/tag/rust-v0.167.0)，2026-11-06 | 保留既有 CLI 适配；不能用 CLI 证据推断 IDE/桌面已接入。 |
| Cline | [4.1.26 发布](https://github.com/cline/cline/releases/tag/v4.1.26)，2026-10-26 | 保留扩展条目；上游 CLI、桌面、JetBrains、SDK 不自动变成受管理形态。 |
| Pi | [1.5.0 发布](https://github.com/earendil-works/pi/releases/tag/v1.5.0)，2026-11-02；[官方迁移](https://pi.dev/changelog/2026/5/7/pi-has-a-new-home) | 保留 earendil-works 包，不回退旧归属。 |
| Hermes | [0.23.0 发布](https://github.com/NousResearch/hermes-agent/releases/tag/v0.23.0)，2026-11-05 | 维持手动边界；Windows 原生路径不能套用 Unix 路径。 |
| OpenClaw | [2026.11.2 发布](https://github.com/openclaw/openclaw/releases/tag/v2026.11.2)，2026-11-08 | Gateway/消息渠道/插件常驻助手，手动指引；不是终端编程代理的等价替换。 |
| ZCode | [官方更新记录](https://zcode.z.ai/cn/changelog)，2026-11-03 发布 3.17.2 | 桌面产品，维持手动指引，不推断 CLI 管理。 |

## DSH 沿用现有独立盘点工作区

DSH 已有单独的[桌面/CLI 被动声明盘点](DSH-PASSIVE-INVENTORY.zh-CN.md)，现已纳入主目录并可从卡片直达该工作区。它读取明确选择的 profile/runtime 声明文件，不启动 DSH。保存的依赖与 bundle 选择不代表已验证安装、认证或实际启用插件。继续关闭安装、更新、激活与配置，目录卡片不表示新增自动管理能力。

参考条目保留原 ID：Continue 为 `unmaintained`，Plandex 为 `cloud_winding_down`，Roo Code 为 `archived`，其余为 `not_integrated`。这些状态不承诺交付版本。Kilo 与 Aider 指向当前组织。
