# Codex 单字段所有权协议 / Single-field ownership protocol

## 当前范围 / Current scope

纯内存 `tui.animations` 协议之上，现增加 `config::codex_animations_fs` 库 API：`preview(path, after)`、`apply(path, after, token)`、`recovery_preview(path)`、`restore_preview(path)`、`restore_checked(path, token)` 和底层 `restore(path)`。该读写与恢复库 API 仅支持 Linux x86_64；Windows、macOS 及其他目标返回 Unsupported。调用方必须显式提供已有 `config.toml` 的绝对路径，不自动发现安装、不根据 `CODEX_HOME` 选取路径、不创建配置目录或缺失配置，不执行 Codex。

The in-memory `tui.animations` protocol now has a `config::codex_animations_fs` library API: `preview(path, after)`, `apply(path, after, token)`, `recovery_preview(path)`, `restore_preview(path)`, `restore_checked(path, token)`, and the low-level `restore(path)`. This read/write and recovery library API supports Linux x86_64 only; Windows, macOS, and other targets return Unsupported. The caller supplies an absolute path to an existing `config.toml`. This layer does not discover installations, select a path from `CODEX_HOME`, create configuration directories or missing configuration, or execute Codex.

桌面 Codex 卡片新增独立终端动画预览、确认应用、恢复观察与确认恢复入口；仅 Linux x86_64 原生模式可执行。IPC 固定选择应用 home 下的 `~/.codex/config.toml`，核对固定注册表适配器；显式为空或不同的 `CODEX_HOME` 拒绝，不接受前端任意路径。浏览器演示模式不执行文件操作，也不模拟成功。没有新增 CLI。旧 Codex 桌面批量配置快捷入口仍拒绝；只读计划的 `revision` 不是写入 token。

The desktop Codex card adds separate animation preview, confirmed application, recovery observation, and confirmed restoration. Execution requires Linux x86_64 native mode. IPC selects `~/.codex/config.toml` under the application home and checks the fixed registry adapter; an explicitly empty or different `CODEX_HOME` is rejected, and the frontend cannot supply arbitrary paths. Browser/demo mode neither operates on files nor simulates success. No CLI entry is added. The old desktop bulk-configuration shortcut remains blocked, and a read-only plan's `revision` is not a write token.

IPC 为 `preview_codex_animations`、`apply_codex_animations`、`inspect_codex_animations_recovery` 和 `restore_codex_animations`。应用与恢复分别确认并使用不同 token。修改目标值或切换上下文会丢弃旧确认；清除展示或离开页面不代表中止已发出的写入。结果不确定时不自动重试，应重新检查恢复状态并保留 receipt。固定错误提示不回传配置正文或原始系统错误。

IPC commands are `preview_codex_animations`, `apply_codex_animations`, `inspect_codex_animations_recovery`, and `restore_codex_animations`. Application and restoration require separate confirmations and different tokens. Changing the requested value or context discards an old confirmation. Clearing the display or leaving the page does not cancel an already-dispatched write. Uncertain outcomes must not trigger automatic retries: inspect recovery again and preserve the receipt. Fixed error messages do not expose configuration contents or raw system errors.

## Windows 只读观察 / Windows read-only observation

`get_codex_animations_capability` 仅返回固定枚举 `linux_managed`、`windows_read_only` 或 `unsupported`，不发现文件、不探测程序、不返回系统指纹。界面等待能力响应后再开放对应入口；未知能力关闭入口。Linux 保留原预览、应用与恢复流程。Windows 隐藏写入、恢复及目标值控件，仅提供 `observe_codex_animations` 只读检查。浏览器与演示模式不执行或模拟该检查。

`get_codex_animations_capability` returns only the fixed enum `linux_managed`, `windows_read_only`, or `unsupported`: no file discovery, executable probing, or system fingerprint. The UI waits for capability resolution and fails closed for unknown capabilities. Linux retains its existing preview/application/recovery flow. Windows hides write, restore, and requested-value controls, exposing only the read-only `observe_codex_animations` check. Browser/demo mode does not perform or simulate this observation.

Windows IPC 使用相同的固定 home 路径、`CODEX_HOME` 和注册表门禁，调用 `config::codex_animations_observation::observe`，不初始化目录或配置。DTO 仅含 `schemaVersion: 1`、`readOnly: true`、固定状态、当前布尔值或 null，以及恒为 false 的 `applySupported`、`restoreSupported`，没有 token。`ready` 仅表示观察到受支持结构中的显式布尔值，不表示写入就绪或 Codex 已加载；`missing` 不创建文件；`unsupported` 不显示原始错误；`receipt_present` 只提示受管收据存在，不读取内容、不提供恢复。取消或卸载仍保持在途 busy，直至读取实际结束，并丢弃迟到结果。

Windows IPC uses the same fixed-home, `CODEX_HOME`, and registry gates, then calls `config::codex_animations_observation::observe` without initializing directories or configuration. Its DTO contains only `schemaVersion: 1`, `readOnly: true`, a fixed state, the current boolean or null, and permanently false `applySupported`/`restoreSupported`; it has no token. `ready` means only that an explicit boolean was observed in a supported structure, not write readiness or effective Codex loading. `missing` creates nothing; `unsupported` exposes no raw errors; `receipt_present` reports existence without reading the receipt or offering recovery. Cancellation/unmount retains busy ownership until the read settles and discards late results.

Windows 文件与安装后界面验收仍须独立通过；组件合成测试不能替代它们，也不验证官方 Codex 运行效果。本批没有 Windows 写入器。

Windows filesystem and installed-app UI acceptance must pass independently; synthetic component tests do not replace either or verify official Codex runtime behavior. This increment does not provide a Windows writer.

## 官方契约 / Official contract

固定依据：[OpenAI Codex schema，提交 `6dfaca4dad030f5ecaf4925f210f539353b15594`](https://github.com/openai/codex/blob/6dfaca4dad030f5ecaf4925f210f539353b15594/codex-rs/core/config.schema.json)，`definitions.Tui.properties.animations`：布尔值，默认 `true`，控制欢迎画面、闪烁效果及进度旋转等动画。此项不是模型、安全权限或网络设置。schema 核对不证明用户安装了该版本，也不证明配置实际加载。

Pinned source: the linked OpenAI Codex schema at commit `6dfaca4dad030f5ecaf4925f210f539353b15594`, `definitions.Tui.properties.animations`. It is a boolean, defaults to `true`, and enables animations such as the welcome screen, shimmer effects, and spinners. It is not a model, security-permission, or network setting. Schema verification proves neither the installed version nor effective configuration loading.

## 输入与所有权 / Input and ownership

- 仅接受已有普通 `[tui]` 表中的显式 `animations = true/false`。缺失字段、缺失表、点号/内联表等其他表示或不受支持结构均拒绝，不自动迁移或补默认值。
- 只改变该布尔值；注释、无关字段及其他字节必须保留。不能保留时拒绝。
- 专用 receipt 记录首次原布尔值、受管目标及协议所需摘要/阶段，不保存完整配置或凭据。它不是完整备份。摘要不认证 receipt 作者，调用方提供的选择标识也不等于路径安全验证。
- 预览 token 使用独立操作域，绑定受管选择、输入字节、目标值及相关协议状态，不接受批量只读计划的 revision。
- 恢复仅接受协议记录的精确前/后图像。任何字节漂移，包括无关字段、空白或注释修改，都拒绝恢复；不得覆盖人工编辑，也不自动重新应用。

- Accept only an explicit `animations = true/false` inside an existing ordinary `[tui]` table. Reject missing fields/tables, dotted or inline representations, and other unsupported structures. Do not migrate syntax or insert defaults.
- Change only that boolean while preserving comments, unrelated fields, and all other bytes. Refuse when preservation cannot be guaranteed.
- A dedicated receipt retains the initial boolean, managed target, and required digests/phases, never the complete configuration or credentials. It is not a full backup. Digests do not authenticate the receipt author, and the caller-supplied selection identity does not validate path safety.
- Use a separate operation domain for preview tokens, binding the managed selection, input bytes, requested value, and relevant protocol state. Reject read-only bulk-plan revisions.
- Recovery accepts only the recorded exact before/after images. Refuse every byte-level drift, including unrelated edits, whitespace, or comments. Never overwrite manual edits or automatically reapply.

## 恢复确认 / Restore confirmation

`restore_preview` 只读观察；没有 receipt 时不提供 token。其独立恢复 token 绑定路径/父目录身份、配置与 receipt 的精确字节、文件身份/相关元数据及恢复状态。`restore_checked` 在取得同一协作锁后复验 token，再执行恢复。确认期间 receipt 被另一操作替换、即使内容相同但 inode 改变，也必须重新观察和确认。应用 token 与只读批量计划 revision 不能作为恢复 token。底层 `restore(path)` 仍保留，但不能用于需要绑定用户先前确认的界面流程。

`restore_preview` is read-only and returns no token when there is no receipt. Its separate restore token binds the selected path/parent identity, exact config and receipt bytes, file identities/relevant metadata, and recovery state. `restore_checked` revalidates it after acquiring the same cooperative lock, before restoration. A replaced receipt or changed inode, even with identical contents, requires a fresh observation and confirmation. Apply tokens and bulk-plan revisions are not restore tokens. The low-level `restore(path)` remains available but is not suitable for UI flows that must bind a prior confirmation.

## 文件持久化边界 / Filesystem persistence boundary

- 通过目录句柄执行文件操作，检查父目录与文件身份、普通文件/单链接、属主及权限；不跟随符号链接。拒绝可见的扩展属性，包括 POSIX ACL，而不是静默丢失。无法取得所需元数据时拒绝。
- 写操作使用同目录 `.agentdeck-codex-animations.lock` 协作锁，锁文件会保留。首次写入创建 `.agentdeck-codex-animations.json` receipt；已有 receipt 时拒绝重新接管，不能覆盖首次原值。预览和恢复观察不创建锁或 receipt。
- 写前在锁内重新检查目标；通过同目录临时文件、文件同步、替换及目录同步推进。失败可能已改变目标或留下 receipt/临时文件，不能因报错假定零写入。恢复仍要求精确图像；残缺 receipt 保留并拒绝自动恢复，不自动删除证据。
- 替换临时文件可能包含完整配置及敏感值，权限按目标文件保留。进程在临时文件同步后退出可留下该副本；恢复不扫描或自动删除这些遗留文件，以免误删。receipt 不存全文不等于磁盘上永远没有完整临时副本。
- 这是参与同一锁的操作之间的协调，不是对外部编辑器的原子 compare-and-swap（CAS）。同权限进程可绕过锁，摘要不认证 receipt。同步调用和注入故障测试不构成真实断电持久性、恶意并发或隐藏元数据保全保证。

- Operations use directory handles and check parent/file identity, regular-file/single-link status, ownership, and permissions without following symlinks. Visible extended attributes, including POSIX ACLs, are rejected rather than silently lost. Refuse when required metadata cannot be obtained.
- Writes use a sibling cooperative lock, `.agentdeck-codex-animations.lock`, which remains on disk. Initial application creates `.agentdeck-codex-animations.json`; an existing receipt blocks taking ownership again and replacing the first original value. Preview and recovery observation create neither lock nor receipt.
- Recheck the target under the lock, then use sibling temporary files, file synchronization, replacement, and directory synchronization. A failure may already have changed the target or left a receipt/temporary file; an error does not imply zero writes. Recovery still requires exact images. Preserve malformed receipts and refuse automatic recovery rather than deleting evidence.
- Replacement temporary files can contain the complete configuration, including sensitive values, with permissions preserved from the target. Process exit after temporary-file synchronization can leave such a copy. Recovery does not scan for or automatically delete these remnants, to avoid deleting unrelated files. A receipt without full contents does not mean full temporary copies can never remain on disk.
- This coordinates operations that participate in the same lock; it is not an atomic compare-and-swap (CAS) against external editors. Same-user processes can bypass the lock, and digests do not authenticate receipts. Synchronization calls and fault-injection tests do not guarantee real power-loss durability, hostile-concurrency safety, or preservation of hidden metadata.

## 验收边界与下一步 / Validation boundary and next steps

文件回归覆盖精确字节往返、过期 inode/预览、锁、apply 9 个与 restore 8 个持久化断点的错误返回及子进程退出、外部文件/锁/目录/临时文件变动。元数据测试覆盖普通权限、user-xattr 与 POSIX ACL；本地文件系统不支持 ACL 时须明确区分为未验证，并由支持该能力的 CI 强制检查。恢复确认测试还覆盖操作域隔离、receipt/config 同内容换 inode、后续另一受管操作、阶段变动及 Prepared 清理。

File regressions cover exact-byte round trips, stale inode/preview rejection, locking, errors and subprocess exits at 9 apply / 8 restore persistence boundaries, and external changes to files, locks, directories, and temporary files. Metadata tests cover ordinary permissions, user xattrs, and POSIX ACLs. A local filesystem without ACL support leaves those cases unverified; capable CI must require them. Restore-confirmation tests additionally cover operation-domain separation, identical-content inode replacements, later owned operations, phase changes, and Prepared cleanup.

本批仍需独立复核、原生 IPC/真实 WebView 与精确提交 CI 验收。其他平台需单独实现安全文件边界，不可直接开放。真实 App/IPC 对合成文件的测试不证明官方 Codex 加载或动画实际生效；实际运行仍需单独验收。旧批量入口继续拒绝，不发布零散安装包。

This increment still requires independent review, native IPC/real-WebView checks, and exact-commit CI acceptance. Other platforms need their own safe filesystem implementation before exposure. Real App/IPC tests with synthetic files do not prove official Codex loading or animation effects; runtime acceptance remains separate. The legacy bulk entry stays blocked, with no incremental installer release.
