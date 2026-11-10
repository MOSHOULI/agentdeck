# Codex 单字段所有权协议 / Single-field ownership protocol

## 当前范围 / Current scope

本批仅建立 `tui.animations` 的纯内存协议内核：输入文件字节和显式目标布尔值，计算预览、候选输出与恢复判断。它不读取或写入真实配置，不创建目录、锁或备份，不执行 Codex。没有新增可写 UI/IPC，原 Codex 批量配置入口继续拒绝；已有只读计划的 `revision` 仍不是写入 token。

This increment is an in-memory protocol kernel for `tui.animations`: supplied bytes and an explicit target boolean produce a preview, candidate output, and recovery decision. It does not read or write live configuration, create directories, locks or backups, or execute Codex. No writable UI/IPC is enabled. The legacy Codex bulk-configuration entry remains blocked; the existing read-only plan's `revision` is not a write token.

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

## 验收边界与下一步 / Validation boundary and next steps

纯内存测试只能证明候选字节、token 和状态转换规则，不能证明安全文件替换、崩溃恢复、Codex 加载或可用的产品写入功能。

In-memory tests establish candidate-byte, token, and state-transition behavior only. They do not establish safe filesystem replacement, crash recovery, Codex loading, or a usable product write feature.

建议按以下次序推进，每步独立审查：

1. 建立真实文件边界：固定目标与 `CODEX_HOME` 处理、句柄身份、链接/硬链接拒绝、权限及 ACL/xattr 保全或拒绝、受限读取。
2. 实现锁内重新预览和安全替换；定义 receipt 持久化顺序、同步和清理，逐断点注入故障验证精确恢复；已有 receipt 时必须拒绝重新接管，不能覆盖首次原值。不得把内存协议称为跨进程事务。
3. 通过文件级测试后才接独立预览/应用/恢复 IPC 与明确确认 UI，覆盖过期预览、重复点击、卸载重挂载与不确定结果。旧批量入口保持拒绝。
4. 验证真实 App/IPC 对合成配置的行为；官方 Codex 的实际加载和运行仍需单独验收，不能用模拟配置测试替代。

Recommended sequence, with independent review at each step:

1. Establish the filesystem boundary: fixed targets and `CODEX_HOME`, handle identity, link/hard-link rejection, permission and ACL/xattr preservation or refusal, and bounded reads.
2. Re-preview under a lock and implement safe replacement. Define receipt persistence order, synchronization, and cleanup; inject failures at each boundary to verify exact recovery. The persistence layer must reject taking ownership again while a receipt exists, retaining the first original value. Do not call the in-memory protocol a cross-process transaction.
3. Only after file-level validation, add separate preview/apply/restore IPC and explicit confirmation UI. Cover stale previews, repeated clicks, unmount/remount, and uncertain outcomes. Keep the legacy bulk entry blocked.
4. Verify real App/IPC behavior against synthetic configuration. Official Codex loading and execution require separate acceptance; synthetic configuration tests cannot substitute for it.
