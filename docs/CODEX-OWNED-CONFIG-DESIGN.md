# Codex 单字段所有权协议 / Single-field ownership protocol

## 当前范围 / Current scope

纯内存 `tui.animations` 协议之上，现增加 `config::codex_animations_fs` 库 API：`preview(path, after)`、`apply(path, after, token)`、`recovery_preview(path)` 和 `restore(path)`。文件操作仅支持 Linux x86_64；Windows、macOS 及其他目标返回 Unsupported。调用方必须显式提供已有 `config.toml` 的绝对路径，不自动发现安装、不根据 `CODEX_HOME` 选取路径、不创建配置目录或缺失配置，不执行 Codex。

The in-memory `tui.animations` protocol now has a `config::codex_animations_fs` library API: `preview(path, after)`, `apply(path, after, token)`, `recovery_preview(path)`, and `restore(path)`. File operations support Linux x86_64 only; Windows, macOS, and other targets return Unsupported. The caller supplies an absolute path to an existing `config.toml`. This layer does not discover installations, select a path from `CODEX_HOME`, create configuration directories or missing configuration, or execute Codex.

没有新增可写 UI、IPC 或 CLI 入口。旧 Codex 桌面批量配置快捷入口仍拒绝；已有只读计划的 `revision` 仍不是写入 token。库 API 可操作真实文件，不等于产品界面已开放该功能。

No writable UI, IPC, or CLI entry is added. The old Codex desktop bulk-configuration shortcut remains blocked; the existing read-only plan's `revision` is still not a write token. Library-level file operations do not mean this feature is available through the product interface.

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

当前本地文件回归覆盖精确字节往返、过期 inode/预览、锁，以及 apply 9 个、restore 8 个持久化故障边界；普通元数据和 3 项 user-xattr 回归已通过。2 项 POSIX ACL 测试受本地文件系统 EOPNOTSUPP 限制，不能计为通过，仍须 CI 强制验收。新增进程退出故障测试与最终完整 CI 结果应另行核验；本说明不代表整批验收已完成。

Current local file regressions cover exact-byte round trips, stale inode/preview rejection, locking, and 9 apply / 8 restore persistence-failure boundaries. Ordinary metadata and three user-xattr regressions have passed locally. Two POSIX ACL tests are blocked by local filesystem EOPNOTSUPP and must not be counted as passes; required CI validation remains. Newly added process-exit fault tests and final full CI results need separate verification. This document does not declare the entire increment accepted.

后续先完成独立审查、Linux ACL/故障测试与精确提交 CI，再接独立预览/应用/恢复 IPC 和明确确认 UI，覆盖路径选择、取消、过期预览、重复操作与不确定结果。其他平台需单独实现安全文件边界，不可直接开放。真实 App/IPC 对合成文件的测试也不证明官方 Codex 加载或动画实际生效；实际运行仍需单独验收。旧批量入口继续拒绝。

Next complete independent review, Linux ACL/fault tests, and exact-commit CI before adding separate preview/apply/restore IPC and explicit confirmation UI. Cover path selection, cancellation, stale previews, repeated operations, and uncertain outcomes. Other platforms need their own safe filesystem implementation before exposure. Even real App/IPC tests with synthetic files do not prove official Codex loading or animation effects; runtime acceptance remains separate. Keep the legacy bulk entry blocked.
