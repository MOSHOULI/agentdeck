# 注册表来源核验

[English](REGISTRY-SOURCES.md) | 简体中文

最近一次定向复核：**2026-11-06**。本次覆盖 Gemini CLI 和 Claude Code 的 Node 要求与登录说明、Pi 的运行时和配置布局，Roo Code、Crush、Oh-My-Pi 的来源条目，以及 OpenCode V1/V2 的安装与配置边界。Hermes 的结论仍标注为 **2026-11-02** 核验，不代表本次重新核验。以下不是对全部目录字段或 Agent 的认证。

## 区分上游能力、适配器与实际观察

- **上游文档**说明产品自身能力，不证明 AgentDeck 已实现相应适配器。
- **AgentDeck 支持**取决于随应用提供的 `install`、`config_inject` 元数据、实现和测试。`null` 表示禁用写入适配器，不表示上游没有该功能。Node 依赖字符串记录上游要求，不证明本机运行时满足要求。
- **本次实际观察**仅包括解析随应用提供的注册表和静态断言。没有运行 Agent、安装器、登录流程、供应商请求或 MCP 服务；没有读取或修改真实用户配置、凭据。
- `future_registry` 仅保存参考来源，包含已归档项目，不是安装清单或交付承诺。迁入 `agents` 之前必须重新核验来源、实现适配器并通过隔离测试。

`registry-v3.json` 的日期表示本次编辑，不代表重新验证所有安装途径或现有认证文件提示。文件存在不能证明登录可用或远端令牌有效。

**本目录不认证热度排名。** 所有核心与参考条目均设为 `stars: null`、`popular: false`。null 表示没有提供当前已核验星标数，不表示零星标；false 表示撤销旧推荐标志，不是对产品的负面评价。描述已移除未核验的下载量、上下文/模型版本数字、市场排名和主观最高级表述，保留已核验的 Roo Code 归档日期和明确的 OpenCode V1/V2 版本边界。产品描述仅用于识别，不是比较推荐或全部功能认证。

结构保持兼容：`stars` 可为空，排序将缺失数量按零处理，仍保留已安装优先。同等无排名条目按核心 ID 稳定排序，这不是已认证的热度顺序。独立 UI 标签、演示数据和此前缓存的注册表不构成当前核验依据，也不会因本次 bundled 目录修改而自动更新。

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

`crates/aisync-core/tests/catalog_metadata.rs` 解析真实的随应用注册表和 `tests/fixtures/catalog-verification-2026-11-06.json` 中注明日期的预期值，检查 Node/认证修正、Pi/Hermes 写入禁用、OpenCode 明确的 V1 途径及 MCP/模型写入禁用与指令目标保留、归档/仅供参考状态、经复核的固定安装配方、现有九个核心 ID，以及全部核心/参考条目撤销未核验热度数据。定向排序测试保留已安装优先，不臆造排名。测试不执行目录中的命令、不读取用户主目录，也不联系供应商。

通过仓库隔离主目录包装器运行：

```sh
python scripts/run-isolated.py cargo test --locked -p aisync-core --test catalog_metadata
```

这些测试只能发现目录回归，不会重新抓取上游资料，也不能证明真实安装、登录或配置往返成功。
