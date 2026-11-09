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
