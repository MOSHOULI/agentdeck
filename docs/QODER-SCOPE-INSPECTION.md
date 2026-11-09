# Qoder CLI scope inspection / Qoder CLI 作用域检查

## English

```sh
aisync qoder inspect \
  --user-settings /absolute/user-config/settings.json \
  --project-settings /absolute/project/.qoder/settings.json \
  --local-settings /absolute/project/.qoder/settings.local.json \
  --project-mcp /absolute/project/.mcp.json
```

Select one to four files explicitly. This read-only CLI feature inventories fixed section flags, MCP entry counts, and how many distinct MCP identifiers appear in multiple selected files. It never returns server names, configuration values, dynamic keys, paths or raw parser/OS errors. A repeated identifier is a potential overlap, **not a conflict diagnosis or an effective configuration result**. Missing/unsupported selected files suppress the aggregate overlap count; unselected files are never assumed empty. Scope labels are user declarations, not proof that files belong to the same project or active installation.

This is a dated documentation contract, **not a verified Qoder runtime-version adapter**. No Qoder, Node, npm, shell, MCP process or network operation runs. No HOME/project discovery, `QODER_CONFIG_DIR` expansion, dedicated credential lookup, profile initialization, locks, backup or configuration writes occur. The selected settings may themselves contain credentials; their bytes are read for parsing but omitted from output. BYOK remains in Qoder's official `/model` wizard. Do not use this tool to hand-edit provider credentials.

The official settings model has user/project/local layers; project MCP uses `.mcp.json` instead of project settings. Folder trust, environment, command-line settings, and field-specific merge rules can affect actual behavior. No layer merge, trust inference or runtime eligibility check is performed. Arbitrary unknown settings are not interpreted; fixed section presence is not schema validation. This release accepts JSON plus `//` line comments, preserving strings such as URLs. BOM, block comments, trailing commas and duplicate keys produce unsupported observations, not a claim that Qoder rejects them. No repair or migration is attempted.

Paths must be absolute local paths with the expected basename. Invalid or lexically repeated selections are rejected before any content read. This is not physical-file deduplication: platform aliases (such as Windows short paths or mount aliases) may still refer to the same file and inflate overlap counts. The shared inspection reader rejects links/reparse points, hard links, non-regular files and oversized input on audited Windows/Linux targets, with bounded JSON parsing. It is best effort: same-user ancestor replacement races, multi-file atomicity and mount I/O deadlines are not solved. Reads can update access times. Other targets fail closed. Exit 0 means a JSON report was emitted, including missing/unsupported observations, not that Qoder is installed, working or safe to apply. Invalid selections/output failures return nonzero; argument errors may echo input paths.

Tests use synthetic settings and blocked process sentinels. They do not establish real Qoder compatibility, online authentication, GUI quality, configuration ownership or recovery safety. No install/configure/restore capability is enabled in the registry. A future editor requires independently versioned contracts, field ownership, preserved originals and drift checks, not whole-file copying from other managers.

## 中文

此 CLI 功能必须明确选择一至四个文件，输出固定分区是否存在、MCP 条目数量，以及多个所选文件中重复出现的 MCP 标识数量。不输出服务器名称、配置值、任意键名、路径或原始错误。重复标识只是可能重叠，不代表已判断冲突或生效优先级；任一已选文件缺失或不支持时，不给出汇总重叠数。未选文件不视为空文件，作用域由调用方声明，不验证项目归属。

这是按 2026-11-08 官方文档整理的只读检查，不是已验证的 Qoder 运行版本适配器。不执行 Agent、Node、npm、shell 或 MCP，不自动寻找文件、展开环境变量、读取独立认证文件、初始化、加锁、备份或写入。所选设置内可能包含凭据，其字节会进入解析内存但不会输出。BYOK 仍应通过官方 `/model` 向导完成，不手写提供商凭据。

用户/项目/本地设置与项目 `.mcp.json` 分开选择；不推断目录信任、CLI 参数及环境变量，不合并生效配置。只解析 JSON 与 `//` 行注释；不支持 BOM、块注释、尾逗号和重复键，不自动修复。“不支持”不等于 Qoder 配置无效。固定分区存在也不等于类型正确。

全部路径先校验再读取，拒绝词法重复路径、链接、特殊文件及超限输入；不做物理文件身份去重，Windows 短路径或挂载别名可能导致重复计数。Windows/Linux 的读取检查不能完全消除同用户竞态，也不是跨文件原子快照；访问时间可能变化。退出码 0 只表示报告生成，真实安装、登录、GUI、写入与恢复均未验收。此功能没有开启注册表的安装、配置或恢复能力。

## Official references / 官方依据

Reviewed 2026-11-08; rolling documentation, not a pinned binary release:

- [Settings layers and JSON format](https://docs.qoder.com/cli/settings)
- [MCP scopes and files](https://docs.qoder.com/cli/mcp-servers)
- [BYOK wizard and prohibition on manual settings.json configuration](https://docs.qoder.com/cli/custom-models)

No Magpie or CC Switch code was copied. Their separation of capability and ownership informed the design; this slice makes no claim to provide their configuration switching features.

Separate from settings inspection, [native distribution planning and explicit archive
verification](QODER-DISTRIBUTION.md) now pin the reviewed 1.1.85 artifacts. These offline
commands do not enable installation, execution, configuration or restore support.

另有独立的[原生分发计划与显式归档校验](QODER-DISTRIBUTION.md)，固定已审阅的
1.1.85 包；不因此启用安装、执行、配置或恢复能力。

## Desktop explicit inspection / 桌面显式检查

The Qoder workspace now includes a separate read-only scope inspection panel. Its four optional inputs correspond exactly to the CLI selections above; choose at least one. No default path, directory picker discovery or credential lookup occurs. Native IPC validates all selected paths together (including bounded length, expected basename and lexical duplication) before calling the existing core reader. Windows and Linux x86_64/ARM64 use the audited reader; other platforms produce unsupported observations, never simulated success. Selected files remain read-only and may update access times.

The desktop validates the fixed report contract, exact four-scope order, requested selection/status relationship, limits, boolean section flags and bounded counts. Incomplete observations suppress aggregate overlap rather than implying zero. Dynamic names, values, raw errors, backend limitation text and paths are not rendered in results. Paths remain visible only in the explicit user input fields. Repeated identifiers are possible overlap, not proven conflict, priority, active installation, project association or effective configuration. No merge/apply/restore capability is enabled.

Input edits, cancellation, clearing and unmount discard late results. Cancellation does not stop underlying reads; an independent Qoder activity owner remains until native IPC settles even after unmount. Reads cannot start while any workspace operation reports busy, including Qoder cache writes; this is cross-workspace coordination, not an atomic global backend lock. Browser/demo modes never invoke or simulate file inspection. Synthetic secret-bearing fixtures test response redaction and no writes; they are not evidence of Qoder runtime compatibility or online authentication.

Qoder 工作区新增独立只读作用域面板，四项可选路径对应上述 CLI 选择，至少明确选择一项，不自动填写默认路径、扫描目录或查找凭据。原生 IPC 先统一校验全部所选路径（有界长度、预期文件名和词法去重），再调用既有核心读取器。Windows 与 Linux x86_64/ARM64 使用已审阅读取边界；其他平台输出不支持，不模拟成功。只读可能改变访问时间。

桌面严格校验固定契约、四作用域顺序、与本次选择对应的状态、读取上限、布尔分区标志和计数。缺失或不支持的观察不把汇总重叠数当作零。结果不显示任意名称、配置值、原始错误、后端限制文本或路径；路径只保留在用户明确填写的输入框中。重复标识仅为潜在重叠，不代表冲突、优先级、已安装、项目归属或生效配置，不启用合并、写入或恢复能力。

编辑输入、取消、清除和卸载均废弃迟到结果。取消不停止底层读取，独立忙碌标记跨组件卸载保留至 IPC 真实结算；任何工作区报告忙碌时（包括 Qoder 缓存写入）不启动本检查。这是跨工作区协调，不是原子全局后端锁。浏览器和演示模式不调用或模拟检查。含合成秘密的测试验证脱敏与零写，不代表真实 Qoder 兼容性或在线认证已验收。
