# 注册表来源核验

**2026-11-08 更新优先：** [最新目录复核](MAINSTREAM-CATALOG.zh-CN.md) 替代下文历史 Claude npm 安装与 Gemini 认证/受众说明。Claude 改手动安装指引，两者凭据文件登录提示已移除。当前测试快照为 `catalog-verification-2026-11-08.json`（16 条核心目录）。保留旧日期记录用于溯源，不作为当前安装建议。

[English](REGISTRY-SOURCES.md) | 简体中文

最近一次定向复核：**2026-11-06**。本次覆盖 Gemini CLI 和 Claude Code 的 Node 要求与登录说明、Pi 的运行时和配置布局，Roo Code、Crush、Oh-My-Pi 的来源条目，OpenCode V1/V2 的安装与配置边界，以及 Qwen Code 手动目录和被动发现的边界。Hermes 的结论仍标注为 **2026-11-02** 核验，不代表本次重新核验。以下不是对全部目录字段或 Agent 的认证。

## 区分上游能力、适配器与实际观察

- **上游文档**说明产品自身能力，不证明 AgentDeck 已实现相应适配器。
- **AgentDeck 支持**取决于随应用提供的 `install`、`config_inject` 元数据、实现和测试。`null` 表示禁用写入适配器，不表示上游没有该功能。Node 依赖字符串记录上游要求，不证明本机运行时满足要求。
- **本次实际观察**仅包括解析随应用提供的注册表和静态断言。没有运行 Agent、安装器、登录流程、供应商请求或 MCP 服务；没有读取或修改真实用户配置、凭据。
- `future_registry` 仅保存参考来源，包含已归档项目，不是安装清单或交付承诺。迁入 `agents` 之前必须重新核验来源、实现适配器并通过隔离测试。

**仅限后续开发版本的新增项：** Qwen Code 正在下一开发版本中接入，不包含在已发布的 v1.1.1 安装包中；本文不证明新的安装包已经发布。原有九个条目与八个参考条目保留。

`registry-v3.json` 的日期表示本次编辑，不代表重新验证所有安装途径或现有认证文件提示。文件存在不能证明登录可用或远端令牌有效。

**本目录不认证热度排名。** 所有核心与参考条目均设为 `stars: null`、`popular: false`。null 表示没有提供当前已核验星标数，不表示零星标；false 表示撤销旧推荐标志，不是对产品的负面评价。描述已移除未核验的下载量、上下文/模型版本数字、市场排名和主观最高级表述，保留已核验的 Roo Code 归档日期和明确的 OpenCode V1/V2 版本边界。产品描述仅用于识别，不是比较推荐或全部功能认证。

结构保持兼容：`stars` 可为空，排序将缺失数量按零处理，仍保留已安装优先。同等无排名条目按核心 ID 稳定排序，这不是已认证的热度顺序。独立 UI 标签、演示数据和此前缓存的注册表不构成当前核验依据，也不会因本次 bundled 目录修改而自动更新。

## Qwen Code：手动安装与仅被动 PATH 发现

- [官方仓库 QwenLM/qwen-code](https://github.com/QwenLM/qwen-code)及 [Apache-2.0 许可证](https://github.com/QwenLM/qwen-code/blob/600aff0592bcfc21668ca4aba95a5b20c6491b86/LICENSE)。
- 本次复核的 GitHub 发布版：[v0.26.9](https://github.com/QwenLM/qwen-code/releases/tag/v0.26.9)，对应提交 `600aff0592bcfc21668ca4aba95a5b20c6491b86`。
- [固定提交的根包清单](https://github.com/QwenLM/qwen-code/blob/600aff0592bcfc21668ca4aba95a5b20c6491b86/package.json)及[发布包清单生成逻辑](https://github.com/QwenLM/qwen-code/blob/600aff0592bcfc21668ca4aba95a5b20c6491b86/scripts/prepare-package.js#L360-L434)。
- [固定提交的官方快速入门](https://github.com/QwenLM/qwen-code/blob/600aff0592bcfc21668ca4aba95a5b20c6491b86/docs/users/quickstart.md)。
- [固定提交的可执行入口包装器](https://github.com/QwenLM/qwen-code/blob/600aff0592bcfc21668ca4aba95a5b20c6491b86/scripts/cli-entry.js)。
- [固定提交的平台与 shell 文档](https://github.com/QwenLM/qwen-code/blob/600aff0592bcfc21668ca4aba95a5b20c6491b86/docs/developers/tools/shell.md)。
- [固定提交的认证文档](https://github.com/QwenLM/qwen-code/blob/600aff0592bcfc21668ca4aba95a5b20c6491b86/docs/users/configuration/auth.md)。

这是特定时点的上游源码复核，不是本机已安装版本。复核的发布版为 **v0.26.9**；本次无法独立读取 npm 注册服务，因此没有验证 npm dist-tag、tarball 完整性或 npm 发布状态。源码生成的发布清单将 `@qwen-code/qwen-code` 映射到命令 `qwen`，发布包的 bin 为 `cli-entry.js`；仓库中的 `scripts/cli-entry.js` 或工作区入口路径不能代替发布包 bin 路径。

官方 npm 替代途径为 `npm install -g @qwen-code/qwen-code@latest`，要求 **Node.js >=22.0.0**。目录中的 `node >= 22` 仅记录这条 npm 途径的前提，不表示独立安装必须另装 Node 运行时。上游目前推荐独立的 shell/PowerShell 安装器。npm 命令与独立安装器都仅是**用户自行执行的说明**：AgentDeck 保持 `install.method: manual`、`install.command: null`、`uninstall_cmd: null`，不会把 Qwen 加入现有六个固定 npm 安装适配器。`@latest` 会变化，不是固定版本。本次未运行安装器或依赖生命周期脚本。

上游支持原生 Windows，不仅是 WSL。shell 文档描述了 `cmd.exe /c`，以及 Windows build `<=19041` 时的 PTY 回退；这不是一份覆盖全部平台的最低操作系统要求表。本次未核实 macOS/Linux 的精确最低版本。这些上游平台说明不证明 AgentDeck 的 Windows GUI 或真实 Agent 往返流程已通过。

包装器会处理 `QWEN_CODE_RELAUNCH_ARGS`、允许 `CLI_VERSION` 覆盖输出、读取主目录 `.env` 以选择 `QWEN_HOME`，并在**处理版本查询之前**考虑受管安装的元数据。仅清除参数和版本覆盖变量，仍不能避免主目录查找，也不能证明报告的版本属于 PATH 上的 npm 安装。因此，AgentDeck 对该条目不运行 `qwen --version`、不查询 npm、不检查 Qwen manifest，也不读取 Qwen 配置或凭据。保留的 `verify_cmd: qwen --version` 仅是后端固定识别标记，**不是会执行的验证配方**。

当前只提供被动 PATH 候选观察。完整查找中找到的候选仍为 `Unsupported`，版本与安装来源未知；未找到为所检查 PATH 上的 `NotDetected`，不表示整台机器未安装。查找不完整或候选不可用时返回 `Error`，不会声称未安装。不同目标的多个候选继续标记为有歧义。自动安装、升级、卸载均不可用，不能根据源码快照、文件名或目录配方推断包关联或真实运行结果。

手动安装后，官方用户流程是启动 `qwen`，再使用会话内 `/auth`；独立的 `qwen auth` 已移除。复核文档中的 Qwen Account OAuth 需要 **Plus/Pro 订阅**。API Provider 选项包括 QwenCloud、Alibaba ModelStudio、第三方与自定义服务商；本次不承诺免费额度或服务商可用性。`auth_type: account_login` 仅描述账号入口，`completion_check: null` 保持登录未验证，不猜测凭据路径。AgentDeck 不执行认证或检查认证文件。

Qwen 的模型、MCP、个性化目标全部为 `null`；skills 与 memory 为 `false`，记忆采集器为 `null`。配置与管理能力检查继续返回 false。上游提供这些功能不等于 AgentDeck 启用写入适配器。此条目属于**下一开发版本**，不在已发布的 **v1.1.1** 安装包中；静态 fixture 与合成的被动发现测试均不证明真实安装、认证或已发布支持。

## OpenCode：V1 安装途径与有限配置支持

- [官方 V1 安装指南](https://opencode.ai/docs/)
- [官方 V2 安装指南](https://opencode.ai/v2/docs)
- [V1 到 V2 兼容与迁移指南](https://opencode.ai/v2/docs/migrate-v1/)
- [V2 原生 MCP 格式](https://opencode.ai/v2/docs/mcp-servers/)
- [V2 指令文件位置](https://opencode.ai/v2/docs/instructions/)

保留的 `npm install -g opencode-ai` 是官方 **V1** 途径，不表示 V2 支持。V2 文档使用 `@opencode/cli`，明确不支持 Windows 包管理器，并提供独立二进制。AgentDeck 不静默替换包或执行迁移。两版共享 `opencode` 命令，可能相互覆盖或冲突。安装预检仅保护已观察到的实例，不承诺发现全机所有安装。

V2 仍读取 `~/.config/opencode/opencode.json(c)`，并支持文档列出的 V1 字段，不能笼统地说旧路径或字段全部不兼容。插件、服务端 API、终端客户端配置各有迁移边界，原生嵌套结构也可能不同。

AgentDeck 当前 MCP 适配器读写 `mcp.<name>`，而 V2 原生结构是 `mcp.servers.<name>`，无法可靠列出或关闭原生 V2 服务。通用模型配置合并也不验证 V1/V2 嵌套结构；随应用提供的空模型模板不能证明用户配置兼容。因此 **MCP 与模型配置适配器已禁用**，不猜测新路径，也不启用未经验证的 V2 适配器。

个性化仍启用：V2 指令文档明确保留 `~/.config/opencode/AGENTS.md`，AgentDeck 在该处合并受管 Markdown 区块。这一有限路径支持不证明真实 Agent 端到端验证通过。skills、memory 标志和会话采集器保持不变，本次未认证其 V2 兼容性。恢复 V2 配置写入前必须实现明确识别版本/结构的适配器并通过隔离测试。

## Gemini CLI

- [官方安装要求](https://geminicli.com/docs/get-started/installation/)
- [官方认证指南](https://geminicli.com/docs/get-started/authentication/)

文档要求 **Node.js 20.0.0 或更高版本**。Google 账号登录流程为运行 `gemini`，选择 **Sign in with Google**，再按浏览器提示完成认证。已移除旧的 `gemini login` 说明。本次未验证现有 `completion_check` 路径，也未实测 OAuth；现有 AgentDeck 适配器开关保持不变。

## Claude Code

- [官方设置指南中的 npm 安装](https://code.claude.com/docs/en/setup#install-with-npm)
- [官方 CLI 参考](https://code.claude.com/docs/en/cli-reference)

**npm 包**要求 **Node.js 22 或更高版本**。这适用于 AgentDeck 现有 npm 路径，不表示原生 Claude Code 运行时需要 Node。官方账号登录命令是 `claude auth login`，已替换旧的 `claude login` 说明。安装配方与现有适配器保持不变。本次未验证认证存储和现有 `completion_check` 路径。

## Pi

- [2026-05-07 官方迁移与包名公告](https://pi.dev/changelog/2026/5/7/pi-has-a-new-home)
- [官方仓库](https://github.com/earendil-works/pi) / [官网](https://pi.dev)
- [当前包清单](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/package.json)
- [配置布局与指令文件](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/configuration.md)
- [路径解析实现](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/src/config.ts)
- [上游 MCP 支持](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/mcp.md)

当前包名为 `@earendil-works/pi-coding-agent`，旧的 `@mariozechner` 命名空间已弃用。安装和卸载命令使用当前包名，CLI 仍为 `pi`，版本检测仍为 `pi --version`。卸载不会移除另外安装的旧命名空间包。

当前包清单要求 **Node.js >=22.19.0**。用户配置默认位于 `~/.pi/agent`，可由 `PI_CODING_AGENT_DIR` 覆盖。设置、供应商/模型、凭据和 MCP 服务分别使用 `settings.json`、`models.json`、`auth.json`、`mcp.json`。包括 `AGENTS.md` 在内的全局指令文件也位于选定的 Agent 目录；项目配置和上下文文件另有规则。

AgentDeck 旧的模型目标 `~/.pi/config.json` 与此布局不符，现已将 `model_config` **禁用**，没有改写为猜测的新路径。仅修正路径不足以证明配置结构、凭据处理、配置目录选择和合并行为正确。个性化配置此前已为 `null`，继续禁用，因此没有需要迁移的有效指令文件目标。MCP 保持 `null`，skills 和 memory 保持 `false`，也没有记忆采集器。上游支持 MCP 不等于 AgentDeck 支持 MCP 注入。

Pi 的安装配方现与上游 npm 替代路径一致：`npm install -g --ignore-scripts @earendil-works/pi-coding-agent`。固定 npm 适配器使用相同参数。上游说明正常 npm 安装不需要依赖生命周期脚本；禁止这些脚本不代表所有依赖均获安全认证，也不等于对软件进行沙箱隔离。上游 README 还说明直接 npm 安装不会锁定间接依赖。本次没有执行安装配方。

为便于识别复核来源，GitHub 返回了以下**文件 blob SHA**，它们不是仓库提交 ID：

- 包清单：`13e6335774d9809f74a1ccd2dd690770aad51473`。
- 配置参考：`b16fd001d836f764a3e50f9cdbde7e72ccbdef41`。
- 路径实现：`5a9552d5de451817ab06e99f2626e931d15c3384`。

## 仅供参考与已归档条目

- [Roo Code](https://github.com/RooCodeInc/Roo-Code)：仓库于 **2026-05-15** 归档，README 说明扩展于 5 月 15 日停止运营。目录仅保留历史参考，状态为 `archived`，不作为待接入推荐或受支持安装。历史扩展安装类型不是可执行安装配方。
- [Crush](https://github.com/charmbracelet/crush)：来源从 `charms/crush` 改为 `charmbracelet/crush`。仍不在 `agents` 中，保持 manual/仅供参考，没有配置适配器。
- [Oh-My-Pi](https://github.com/can1357/oh-my-pi)：来源从 `oh-my-pi/oh-my-pi` 改为 `can1357/oh-my-pi`。仍不在 `agents` 中，保持 manual/仅供参考，没有配置适配器。

## Hermes Agent（2026-11-02 核验）

- [官方安装指南](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/getting-started/installation.md)
- [官方平台支持矩阵](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/getting-started/platform-support.md)
- [官方下载站点](https://hermes-agent.nousresearch.com/)

平台矩阵明确不支持 PyPI 安装。AgentDeck 不运行 `pip install hermes-agent` 或对应 pip 卸载。Hermes 保持手动安装条目，链接至官网，保留 `hermes --version` 检测。由用户选择官方软件包或平台安装器。

原生 Windows 默认使用 `%LOCALAPPDATA%\hermes`，POSIX 默认使用 `~/.hermes`，`HERMES_HOME` 可覆盖用户数据位置。AgentDeck 尚未解析并验证平台与配置目录，因此 MCP、个性化和模型配置写入适配器继续**禁用**，避免写入可能无关的 `~/.hermes/config.yaml`。请通过 Hermes 自身工具配置，尤其不能用 Markdown 指令模板覆盖 YAML 配置。

上游元数据会变化，重新启用自动安装或配置之前必须复核。

## 隔离目录回归测试

`crates/aisync-core/tests/catalog_metadata.rs` 解析真实的随应用注册表和 `tests/fixtures/catalog-verification-2026-11-06.json` 中注明日期的预期值，检查 Node/认证修正、Pi/Hermes 写入禁用、OpenCode 明确的 V1 途径及 MCP/模型写入禁用与指令目标保留、归档/仅供参考状态、经复核的固定安装配方、十个核心 ID（原有九个加 Qwen Code）、未改变的八个参考 ID、Qwen Code 的手动/仅被动观察边界及禁用的管理与配置能力，以及全部核心/参考条目撤销未核验热度数据。定向排序测试保留已安装优先，不臆造排名。测试不执行目录中的命令、不读取用户主目录，也不联系供应商。

通过仓库隔离主目录包装器运行：

```sh
python scripts/run-isolated.py cargo test --locked -p aisync-core --test catalog_metadata
```

这些测试只能发现目录回归，不会重新抓取上游资料，也不能证明真实安装、登录或配置往返成功。

## 2026-11-08 catalog expansion

[主流产品官方指引](MAINSTREAM-CATALOG.zh-CN.md)

- https://github.com/anthropics/claude-code
- https://github.com/google-gemini/gemini-cli

- https://github.com/Aider-AI/aider
- https://github.com/Kilo-Org/kilocode
- https://github.com/continuedev/continue
- https://github.com/plandex-ai/plandex
