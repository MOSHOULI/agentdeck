# Qoder cache inspection / Qoder 缓存只读诊断

```sh
aisync qoder inspect-cache --cache /absolute/explicit/cache
```

Linux x86_64 only, for the fixed Qoder 1.1.85 `linux-amd64-baseline` cache contract. This is an observation and preservation plan, **not deletion, repair, installation or execution authorization**. No `--yes` is required or accepted because the operation does not write. Exit zero means a report was obtained, not that the cache is ready or safe to delete; unsupported hosts and inaccessible/unsafe paths fail with fixed errors without echoing paths.

| `state` | Meaning | `recovery_plan` |
|---|---|---|
| `staged_archive_verified` | Strict stage intent/completion records plus actual fixed-size archive SHA256 verified | `retain_cache_no_action` |
| `unpacked_payload_verified` | Strict unpack records plus archive and non-executable payload sizes/SHA256 verified | `retain_cache_no_action` |
| `incomplete` | Recognized intent and safe known partial members, without a completion record; bytes are not claimed complete/valid | `preserve_and_review` |
| `conflict` | Unsafe metadata, unexpected members, pending publication, record/content disagreement or observed change | `preserve_and_review` |
| `unknown_ownership` | No recognizable strict intent; historical ownership cannot be established | `preserve_and_review` |

All reports contain `manual_review_required: true`, `deletion_authorized: false`, `installed: false`. A successful record/digest match proves only current contract agreement, not who created the directory. The report is not a cleanup token, signature, atomic snapshot or future execution credential. Do not overwrite or retry staging/unpacking in place. Preserve partial/conflicting/unknown data and review it manually before any separate action. No removal command is supplied.

## Read boundary

Existing audited Linux dirfd/no-follow traversal and bounded record readers are reused. The selected directory must belong to the effective user and have mode 0700. Known members must be regular files, mode 0600, effective-user-owned and single-linked. Directory enumeration is bounded to five names; unknown member contents are never opened and names are not printed. No lock or other file is created. Stage/unpack intents cannot substitute for one another. Strict JSON rejects duplicate and unknown fields. Completion requires exactly the relevant file set, with no pending or extra member.

Complete states rehash actual archive bytes (78,587,775) and, for unpacked state, actual payload bytes (182,543,840), using the pinned archive and payload digests already used by stage/unpack. They do not re-extract or run the payload. Record bytes, descriptor/path identities, permissions, sizes, link counts, modification/change times and directory members are rechecked. Access times may change from reading. Same-user malicious races, mount I/O deadlines and atomic filesystem snapshots remain unsolved. No arbitrary file names, content or paths are included in reports.

## 中文

仅 Linux x86_64，固定 Qoder 1.1.85 baseline。`inspect-cache` 读取显式目录，给出完整暂存、完整解包、未完成、冲突或未知归属五态及保留/人工检查计划，不删除、不修复、不创建锁、不激活、不运行、不修改 PATH。无需也不接受 `--yes`。退出码 0 只表示得到报告，绝不表示可安全清理或可执行。

所有报告均为 `manual_review_required: true`、`deletion_authorized: false`、`installed: false`。记录和摘要一致只证明当前符合已知契约，不能证明历史创建者或提供签名。完整态重新读取实际归档/载荷并核对固定大小与摘要；未完成态不声称部分字节有效。暂存与解包记录不能混用，未知字段、重复 JSON 键、链接、特殊文件、权限异常、额外成员、pending 记录及观测到的变化均不接受为完整态。未知条目的内容不读取，名称与路径不输出。

目录要求本用户 0700，已知文件要求本用户、普通单链接 0600；最多枚举五个名称，使用已有 dirfd/no-follow 与有界读取。读取前后重验目录和文件身份、权限、大小、mtime/ctime 及记录；读取可能更新 atime，不保证抵抗同用户恶意竞态、挂载点卡顿或原子快照。应保留未知/冲突/半途现场，独立人工检查后再决定操作，不能原地覆盖或把本报告当删除许可。

## Validation boundary

Offline tests cover unknown/malformed/duplicate records, partial files, swapped intent kinds, false completion, extra/pending names, symlink/hardlink/FIFO, unsafe modes, oversize, incorrect same-length digest, inaccessible/linked paths and no-write snapshots. The CLI parser requires an explicit cache and rejects mutation confirmation flags. Existing real staged and unpacked official caches can be inspected without executing Qoder; matching hashes and unchanged bytes/mtime/ctime do not imply runtime compatibility. No Windows/macOS cache management support is claimed.

## Desktop read-only entry / 桌面只读入口

The Qoder workspace accepts a separate explicit absolute cache directory. Its native IPC dispatches the existing core observer on a blocking worker; path bytes are capped at 4096 and control characters are rejected. Only Linux x86_64 can produce observations. Windows and other unsupported hosts return a distinct fixed unsupported-host result, not unknown ownership or simulated success. Browser/demo modes never dispatch or fabricate cache diagnostics.

The UI strictly projects the five states and their corresponding preservation plan, requiring deletion/installed false and manual review true. Backend paths, names, limitations and arbitrary errors are not rendered. Cache changes, archive/target changes, Clear, Cancel and unmount invalidate pending results. Package checks, plans and cache diagnostics share one request/busy guard; other active workspaces block starting a Qoder read. Cancellation only discards its result; it does not abort file reading or free the request slot early. Clear also clears the cache path.

Windows installed acceptance selects an existing synthetic directory through the actual UI, requires the explicit unsupported-host alert, and verifies unchanged bytes, directory members and modification time. This is unsupported-host/zero-write coverage, **not successful Linux GUI acceptance**. Linux core real-cache evidence and frontend mocked reports do not substitute for Linux installed GUI testing; this cloud environment lacks the required GTK runtime/development environment. No desktop cleanup, deletion, extraction, activation or execution action was added.

桌面 Qoder 页面新增独立显式缓存绝对目录，通过阻塞工作线程调用现有 core；输入限制为 4096 字节并拒绝控制字符。只有 Linux x86_64 能产生诊断，其他主机返回明确不支持，不冒充未知归属或模拟成功；浏览器/演示模式不调用。

前端严格校验五态与保留计划对应关系，删除/安装必须为 false、人工复核必须为 true，只呈现固定文案，不显示后端任意字段。改路径/目标、清除、取消、卸载均使迟到结果失效；缓存与计划/归档读取互斥，其他工作区忙碌时不启动。取消仅丢弃结果，不中止底层读取，也不提前释放忙碌。Windows 安装版用真实 UI 对存在目录核实明确不支持及零写，绝不当成 Linux GUI 成功验收；当前云端缺少 GTK，Linux 真实 GUI 仍未验证。没有增加任何清理、删除、修复、解包、激活或执行按钮。
