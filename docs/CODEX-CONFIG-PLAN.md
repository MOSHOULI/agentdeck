# Codex configuration plan / Codex 配置计划

## Scope

The Codex card provides a **read-only configuration plan**, not an apply preview
that authorizes a write. It examines the selected fixed adapter's `.codex/AGENTS.md`
and `.codex/config.toml` under the application home and reads the two relevant
AgentDeck profile files, using bundled defaults in memory when a source is absent.
It does not initialize a profile, create directories, back up files, acquire a
write lock, run Codex, or change configuration. The rest of the application may
perform its ordinary installation observations separately.

The response contains fixed target categories, paths, planned action categories
and conflict counts. It never returns template text, TOML values, arbitrary keys,
credentials or raw parser errors. A revision identifies the observed snapshot;
it is **not** a write token. Cancel simply discards the displayed result.
Other Agent-workspace activity invalidates the plan, including a pending result;
when that activity ends, a new read requires another explicit preview. This does
not monitor external file edits.

An existing instruction block may have been manually edited. Markers delimit a
block but do not establish ownership or an untouched history. A different existing
TOML value or incompatible shape is a conflict, not permission to replace it.
Malformed or ambiguous markers, unsafe paths, unsupported files and excessive
inputs fail closed. An explicitly empty or different `CODEX_HOME` is not silently ignored or redirected.
The plan does not prove which configuration is effective in a running Codex.

The old desktop Codex “apply profile” shortcut remains unavailable: the narrow
animation editor does not implement safe bulk profile application. This restriction covers that
desktop shortcut, not a claim that every legacy CLI or separate synchronization
entry point has been replaced. Other agents retain their existing interfaces.
No new installer is published with this incremental change.

## 中文

Codex 卡片新增只读配置计划，不是确认后即可写入的预览。计划检查明确的两个
目标以及对应 profile 源；源缺失时仅在内存中采用内置模板，不初始化模板或目录，
不创建备份、不调用 Codex、不修改配置。应用其他已有安装观察流程不属于此操作。

界面只显示目标、动作类别及冲突数量，不显示模板正文、配置值、任意键名、凭据
或原始解析错误。revision 仅标识本次观察，不是写入授权；取消只清除界面结果。
其他 Agent 管理活动会使旧计划及尚未返回的结果失效；活动结束不会自动重新读取，
需显式再次预览。这不监控外部文件编辑。
现有 Markdown 标记不能证明其中内容没有被人工编辑；不同 TOML 值和类型冲突
均不得当成自动覆盖许可。路径不安全、文件超限或标记损坏时拒绝处理，不能静默
忽略显式为空或不同的 CODEX_HOME。计划不证明实际运行中的 Codex 已采用这些配置。

桌面 Codex 旧的“应用配置”批量快捷入口仍暂停；单字段动画编辑不等于
已完成安全的整份 profile 应用。这不表示所有旧 CLI 或其他独立同步入口已被替换；其他 Agent
保持原有接口。本批不发布新的安装包。

## Verification boundary / 验证边界

Tests use synthetic files and verify unchanged bytes, metadata and directory
membership, including missing-profile cases. Native IPC and real WebView checks
exercise AgentDeck, not the Codex runtime. No model, login, credential access or
configuration activation is established by these checks.

测试使用自建文件，检查字节、元数据及目录成员不变，包括没有 profile 的情况。
原生 IPC 和真实 WebView 验证的是 AgentDeck，不涉及 Codex 运行时、模型、登录
或配置实际生效。

## Separate animations editor / 独立动画编辑

Linux x86_64 native desktop mode additionally offers [single-field animation management](CODEX-OWNED-CONFIG-DESIGN.md) for an existing explicit `tui.animations` boolean. It has its own preview/apply and observation/restore confirmations with operation-specific tokens; this read-only plan never authorizes those writes. Other platforms and browser demo mode cannot perform them. An empty or different `CODEX_HOME`, missing or unsupported config, unsafe metadata, or existing ownership receipt prevents a new application.

Linux x86_64 原生桌面另有[单字段动画管理](CODEX-OWNED-CONFIG-DESIGN.md)，仅支持已有显式 `tui.animations` 布尔值。它分别提供预览/确认应用、观察/确认恢复，使用独立操作 token；本只读计划不授予这些写入权限。其他平台及浏览器演示模式不能执行。显式为空或不同的 `CODEX_HOME`、缺失/不支持的配置、不安全元数据或已有 receipt 均阻止新接管。
