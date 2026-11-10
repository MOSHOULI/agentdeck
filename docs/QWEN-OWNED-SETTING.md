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
4. Optionally inspect recovery first (read-only):
   `aisync qwen vim-recovery --settings /absolute/settings.json --runtime-manifest /absolute/package.json`
5. `aisync qwen vim-restore --settings /absolute/settings.json --runtime-manifest /absolute/package.json --yes`

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

The CLI `vim-recovery` command reads the explicit files and sibling receipt without
creating/acquiring a lock, initializing a directory or writing anything. It reports
one fixed state: `no_receipt`, `restore_field`, `cleanup_receipt`, or `conflict`.
`cleanup_receipt` means restoration would only remove a receipt for an already
restored or never-applied exact document. `conflict` means detected drift prevents
restoration. Unsafe, malformed or differently bound receipts remain errors.
No raw receipt, digest, path, arbitrary key or configuration value is returned.
This is a best-effort observation, not a restore token, permission check or atomic
snapshot: a concurrent change can invalidate it, and restore still revalidates.
The desktop “只读检查 Vim 恢复状态” button now exposes the same observation through
native IPC with a strict fixed-field response projection. `no_receipt` does not
prove the configuration was never changed. No observation authorizes restore or
cleans a receipt: the separate restore confirmation and core revalidation remain.
Starting a diagnosis clears pending apply/restore confirmations; path changes,
clear/cancel, target changes and unmount discard old results. Cancelling discards
the response, not the filesystem read; other operations remain blocked until it
settles. Read-only diagnostics do not lock path inputs. Browser/demo mode never
invokes this inspection. Raw errors and backend-provided prose are not rendered.

Atomic replacement preserves Unix mode and Windows destination ACL; Linux rejects
extended POSIX ACLs, visible extended attributes, special mode bits and owner/group
mismatches rather than silently losing metadata. Linux queries only the attribute
name-list size on opened source/temporary handles; it never reads attribute names
or values, and rejects nonempty lists or unexpected query failures. Apply checks
settings before creating a receipt; restore checks before changing receipt phase;
replacement rechecks source and temporary inodes. A later failure can still leave
a receipt for recovery. Read-only previews are not write-readiness checks.
No security labels are copied or host policies changed. Metadata hidden from the
process by host policy and actual SELinux-host behavior are not verified; temporary
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

可先运行 `aisync qwen vim-recovery --settings /绝对路径/settings.json --runtime-manifest /绝对路径/package.json`
查看恢复状态。该 CLI 命令不创建/获取锁、不初始化目录、不写文件，只返回固定
状态：无日志、可恢复字段、只需清理日志或检测到冲突。异常/不安全或路径绑定
不符的日志仍报错，不返回日志内容、摘要哈希、路径或配置值。预览只是尽力保持
一致的读取，不代表具备写权限或保证稍后能成功；恢复仍重新校验。

桌面“只读检查 Vim 恢复状态”现通过原生 IPC 显示同一观察，严格校验状态与布尔值，
只保留固定字段，不渲染后端说明、原始错误或动态内容。“无记录”不代表从未修改；
“仅清理记录”可能是原本从未应用，不自动删除记录。撤销仍需单独确认和后端复查。
诊断开始时清除待确认应用/撤销；路径、目标值、清除、取消、卸载界面使旧响应失效。
取消仅丢弃结果，不中止读取，读取结束前其他操作仍互斥；只读期间路径可编辑。
浏览器与演示模式不触发此读取。

## Linux extended-attribute regression / Linux 扩展属性保护

The unchanged CLI previously returned success while replacing the inode and
dropping a synthetic `user.agentdeck-test` attribute, independently on apply and
restore. Linux CI now explicitly runs four filesystem-capability tests covering
apply refusal before receipt creation, restore refusal before phase transition,
receipt replacement refusal, and fd-based temporary-inode inspection. Tests
verify unchanged bytes, inode where relevant, and attribute values; they do not
configure SELinux, change host security policy or execute Qwen.

Linux 原子替换现在拒绝无法保留的可见扩展属性，不再静默丢失用户元数据。仅查询已打开
文件句柄的属性名列表长度，不读取名称或值；非空或非预期错误均拒绝。apply 在创建
所有权记录前检查，restore 在改变记录阶段前检查，实际替换前再次检查源文件和临时
文件。普通只读预览不保证可以写入；后期失败仍可能保留恢复记录。测试只使用合成
user.* 属性，没有更改 SELinux/主机安全策略，也不声称验证了隐藏元数据或全部安全标签。

```sh
python scripts/run-isolated.py cargo test --locked -p aisync-core extended_attributes_ -- --ignored
```

## Updating an owned value / 更新已受管的值

After a successful apply, use `aisync qwen vim-update-preview` with the same
explicit paths and the new `--enabled` value. Confirm using `vim-update`, its
returned `--preview-token`, and `--yes`. The desktop exposes a separate managed
update preview and confirmation. This is not a fallback that silently takes
ownership: a valid applied receipt and an unchanged managed field are required.
The update token binds the selected document, manifest, value and receipt.
Changing any of them requires another preview.

The first original value or absence remains the restore destination across
repeated updates. Existing version-1 receipts can be read; a real update records
a version-2 transaction. An interrupted update is not resumed automatically:
recovery accepts only its exact pre-update or post-update document image and
restores the first original value. Other images are conflicts. Completed updates
still allow unrelated edits to be retained during a later restore. Existing
strict-JSON, metadata and cooperating-writer limits remain unchanged.

首次应用成功后，可使用相同明确路径及新的 `--enabled` 值调用
`vim-update-preview`，检查后用返回的 token、`--yes` 调用 `vim-update`。
桌面提供独立的受管更新预览与确认，不会把初次应用失败自动转成更新。
必须已有有效且已完成的受管记录，当前字段未被外部修改；配置、清单、目标值
或受管记录变化后，旧预览失效。

反复切换仍保留首次原值或“原本不存在”，撤销始终回到首次状态。旧版记录
仍可读取，实际更新使用新版事务记录；更新中断时不自动继续，只允许对精确的
更新前或更新后文档进行恢复，其他内容判为冲突。正常完成后撤销仍保留无关
字段的外部修改。严格 JSON、元数据和协作锁的既有限制不变。

Coverage uses authored configuration fixtures, the product core/CLI/native IPC,
and the pinned readConfigFile helper. It does not execute the full Qwen runtime
or establish effective layered settings.
验证使用自建配置、产品核心/CLI/原生 IPC 及固定读取函数，不等于执行完整
Qwen 运行时或验证多层设置的最终生效值。
