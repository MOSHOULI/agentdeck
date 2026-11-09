# Qwen owned Vim preference / Qwen 受管 Vim 设置

## English

Experimental **CLI and desktop** operation for one non-credential boolean:
`general.vimMode`. Requires explicit existing local `settings.json` and
`package.json` declaring `@qwen-code/qwen-code` **0.27.0**, settings `$version: 4`.
This declaration is not proof of a genuine installation or the selected scope.
No discovery, execution, login, environment interpolation or migration occurs.
The desktop summary inspector stays read-only. A separate Vim preference editor
uses the same core contract and requires a preview and explicit confirmation
before apply; restore has its own confirmation. Changing paths/value discards
a pending preview. Inputs are locked during mutations; no automatic retry occurs.

1. Stop Qwen and other configuration editors first.
2. `aisync qwen vim-preview --settings /absolute/settings.json --runtime-manifest /absolute/package.json --enabled true`
3. Review the boolean before/after and limitations. Apply with the returned token:
   `aisync qwen vim-apply --settings /absolute/settings.json --runtime-manifest /absolute/package.json --enabled true --preview-token TOKEN --yes`
4. `aisync qwen vim-restore --settings /absolute/settings.json --runtime-manifest /absolute/package.json --yes`

Absolute paths must select existing ordinary files; links, hardlinks, unsupported
platforms, oversized/deep/duplicate-key JSON, BOM/comments, missing schema,
unknown manifest versions and non-boolean Vim values fail closed. Strict JSON
is a narrower subset of Qwen's JSONC support, not a claim that Qwen rejects JSONC.
JSON formatting/order can change. All unrelated values are preserved semantically.
No whole-file backup is made, so this is not general configuration recovery.

The sibling `.agentdeck-qwen-vim.json` receipt backs up only original boolean or
absence, parent presence, path/manifest bindings, digests and transaction phase.
It contains no raw settings, arbitrary keys, authentication values or credential
files. An existing receipt prevents taking ownership again. A persistent sibling
lock serializes cooperating AgentDeck processes; do not delete it during use.
Apply checks a path/content-bound preview token and writes its receipt first.
Restore changes only the owned field, preserves unrelated user changes, and
refuses a different managed value. A prepared but uncommitted receipt permits
recovery only against exact before/after images; restoring-state retries use
exact document hashes. Failures retain the receipt for inspection/recovery.

Atomic replacement preserves Unix mode and Windows destination ACL; Linux rejects
extended POSIX ACLs, special mode bits and owner/group mismatches rather than
silently losing security metadata; temporary
configuration content receives the source DACL before being written on Windows.
File checks and advisory locks do **not** prevent a same-user hostile race or
Qwen/other editors writing between final check and replacement. A change away
and back to the same boolean is not detectable. Receipt files are not signed.
Process interruption is covered by tests, not a guarantee against power loss,
filesystem failure, malicious same-user edits, or every storage implementation.
Keep other writers stopped; do not manually edit/copy receipts.

The offline compatibility test uses upstream's unmodified `readConfigFile` at
[6c3d5355](https://github.com/QwenLM/qwen-code/blob/6c3d5355de2be05b8d1ab61f4699f52c02c42068/packages/cli/src/config/read-config-file.ts)
and the strict-JSON parse operation. The same upstream settings schema defines
`general.vimMode` as boolean/default false/requiresRestart false. This verifies
synthetic apply, persisted new-process read, restore and conflict refusal.
It does **not** execute the full Qwen settings loader/UI or prove effective value:
other layers, environment and CLI arguments can override the selected file.
A separate [official distribution bootstrap smoke](QWEN-DISTRIBUTION-SMOKE.md)
checks the real pinned package version/help without login. Full installation,
authentication, effective settings and GUI acceptance remain outstanding.

## 中文

这是 CLI 与桌面开放的实验性单字段管理：`general.vimMode`，不是模型、认证或
完整配置管理。必须明确选择已有普通文件，清单声明精确版本 0.27.0、配置
`$version: 4`；声明不代表安装真实或作用域已确认。桌面摘要检查仍为只读；独立 Vim 设置区需要先预览、明确确认后应用，
撤销也需单独确认。更改路径/值会取消旧预览，写入期间锁定输入，不自动重试。

先停止 Qwen 和其他配置编辑器，运行上述 preview 命令，检查布尔值及限制，
再用预览 token 和 `--yes` 应用；restore 也需要 `--yes`。
注释、BOM、重复键、未知版本等未支持输入拒绝处理，不自动迁移。

只备份原布尔值/不存在状态，不复制完整配置或凭据。原有其他字段按语义保留，
排版和键顺序可能变化。恢复只撤销本次受管字段，其他修改保留；检测到受管
字段外部变化则拒绝覆盖。receipt 记录准备、应用、恢复阶段，允许精确状态下
重试；不能把它当通用备份工具。运行时不要删除锁文件或手动复制/编辑 receipt。

本字段闭环测试使用合成文件与固定官方读取函数；另有[真实官方包启动检查](QWEN-DISTRIBUTION-SMOKE.md)验证版本/帮助，不涉及登录或完整运行时。多层设置可能
覆盖该字段，尚未验证真实登录/GUI。协作锁和最后检查不能消除跨进程竞态，
也无法检测改走又改回相同布尔值；不承诺断电或恶意同用户操作安全。
