# Qoder private executable deployment / Qoder 私有可执行部署

> **Release selection / 版本选择:** 1.1.85 remains the default. Explicit
> 1.1.86 is additionally reviewed; CLI accepts `--release 1.1.86` on Qoder
> distribution/cache/deployment subcommands, and the desktop has a reviewed-release
> selector. Caches, receipts and previews are version-bound; no migration,
> implicit latest selection or reuse of another release's confirmation.
> 旧版仍可显式选择；不同版本的归档、收据与载荷不能混用。下文历史 1.1.85
> 证据仍仅适用于该版本，新版来源见 `upstream/qoder/PROVENANCE.md`。


## English

This explicit CLI operation deploys **reviewed Qoder 1.1.85, linux-amd64-baseline**
only, on Linux x86_64. It is not a latest-version resolver: upstream rolling latest
was 1.1.86 when this increment was reviewed. Other versions/targets require separate
archive and payload review; they are not selected automatically.

```sh
# The parent must already exist, be owned by the current user, mode 0700,
# and have no visible extended attributes (including extended ACLs).
aisync qoder deploy-preview --cache /absolute/verified-unpacked-cache \
  --destination /absolute/private-parent/new-deployment
# Review the preview and executable-file warning, then explicitly confirm:
aisync qoder deploy-private --cache /absolute/verified-unpacked-cache \
  --destination /absolute/private-parent/new-deployment \
  --preview-token TOKEN --yes
```

**This creates an executable file; it never executes it.** No global installation,
PATH edit, symlink, activation, Qoder install command, authentication or configuration
change occurs. The registry's manual installation status is not upgraded by this
operation. A 0700 permission mode is not proof of CPU/libc compatibility, executable
mount support, login, or a working Qoder application.

The source is the existing strict [unpacked cache](QODER-CONTROLLED-UNPACK.md), not
an arbitrary executable or caller-supplied checksum. Its complete fixed record
contracts and both archive and payload hashes are independently revalidated.
Unsigned records are not authority or signature evidence. The reviewed archive is
78,587,775 bytes and the payload is 182,543,840 bytes with SHA-256
`8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b`.
The fixed distribution manifest remains the evidence in
[QODER-DISTRIBUTION.md](QODER-DISTRIBUTION.md); no independent signature verification
or source-to-binary reproducibility claim is made.

### Transaction and failure boundaries

- Preview reads only, apart from possible access-time updates. Its token binds the
  explicit cache/destination paths, source directory and payload identity/metadata,
  record bytes and metadata, target parent identity/ownership/permissions, fixed
  target/version and payload digest. Apply repeats validation and requires `--yes`.
- Parent paths are traversed with no-follow directory descriptors. The target parent
  must be an existing owned 0700 directory. A new child is created with `mkdirat`;
  every existing destination, including a symlink, empty directory or failed attempt,
  is refused. There is no recursive directory creation or in-place retry.
- Only a new exclusive 0600 output file receives bytes from the already-open source
  handle. The copy and a reread of that same output handle must match the fixed length
  and digest. Source records, source metadata/named identity, target directory,
  target parent, output metadata and named identity are rechecked. Visible extended
  attributes on the parent/source/output are rejected, never silently copied.
- Only the held verified output file is changed to **0700** by `fchmod`. The original
  cache stays 0600. There are no group/world or setuid/setgid permissions.
- The new directory contains `qodercli`, `deployment.intent.json` and, on success,
  `deployment.complete.json`. Records are 0600. Completion is written to an exclusive
  `deployment.pending.json`, synced and published with no-replace rename. This is a
  different state/record contract from cache staging/unpacking. No restore, ownership
  adoption or deletion authority is derived from these records.
- **Any failure after directory creation may leave partial output, including an
  already executable file.** A sync, completion-publication or stdout failure can
  occur after permission change. Do not assume an error means nothing happened.
  No automatic deletion, rollback, chmod reversal or retry is performed. Preserve
  output for manual review; choose a new destination for a new confirmed attempt.
- Directory descriptors do not lock all ancestors or exclude another same-user
  writer. Rechecks detect some changes, not a transactional snapshot or hostile-user
  sandbox. Mount I/O deadlines and complete power-loss recovery are not guaranteed.

Tests using tiny authored files cover private helper behavior, not official payload
compatibility. The separate real-payload CLI acceptance test checks pinned official
bytes copied to a new private executable file without running them; it is not a
Qoder runtime or authentication acceptance test.

## 中文

本命令只部署已审核 **Qoder 1.1.85 / linux-amd64-baseline**，主机仅 Linux x86_64。
检查时上游 rolling latest 已是 1.1.86，本批不声称 1.1.85 最新，也不自动扩大版本范围。
流程为显式 `deploy-preview`，核对后携带 token 和 `--yes` 调用 `deploy-private`。

源必须是既有完整受控解包缓存，重新验证固定记录、归档及 payload 的完整 hash/长度，
不能由自填 checksum 或 unsigned record 授权。目标父目录须已存在、归当前用户、0700，
且无可见扩展属性；目标目录必须全新，任何既有文件、链接、空目录或失败残留都拒绝。

新 payload 先以0600独占创建，复制后复读校验完整字节，再复核源记录、身份/元数据和
目标的具名身份。仅对持有的新文件句柄设置0700，缓存本身仍0600。不会执行 Qoder，
不会调用其 install 子命令，不建全局链接、不改 PATH、配置或认证，不冒充全局安装成功。
0700只是权限设置，不证明 noexec 挂载、CPU/libc、运行时或登录可用。

成功记录是独立 deployment 合同，不是 cache completion、签名或清理授权。权限变更
之后仍可能发生 sync、记录发布或输出失败，因此**失败时可能已有可执行的部分结果**。
保留残留人工核对，绝不自动删、回滚、降权或原地重试。前后复核不是原子快照，不防止
同权限恶意竞态，不保证挂载 I/O 截止时间或所有断电情形。

小型合成测试不等于真实软件验收；官方固定 payload 的实际复制验收也不执行其程序，
不能因此声称完成运行、认证或完整 GUI 验收。

## Desktop entry / 桌面入口

The Qoder workspace now exposes a separate explicit private deployment section.
The desktop sends bounded absolute source/destination paths to the real preview
and displays the captured paths, fixed payload digest/length and 0700 mode. Only
an explicit second confirmation submits that exact selection and core token.
Cancel, input change or navigation discards a preview; late preview responses
cannot revive it. Actual deployment cannot be cancelled or automatically retried.
A process-local native lock is shared with cache writes and held inside the
blocking task; the frontend activity owner survives unmount until IPC settles.

No backend limitation text is rendered as instructions. Unsupported hosts fail
closed. The session submission record preserves the actual submitted destination
and complete/unconfirmed outcome across workspace navigation, without persisting
authorization tokens, claiming current file state or granting cleanup rights.
This entry is still private deployment only: no PATH activation, program execution,
login, runtime compatibility claim or automatic removal of partial executables.

Qoder 工作区新增独立私有部署区域。先真实只读预览并展示精确源、目标、固定摘要、
长度与0700权限，再由用户第二次确认；IPC提交原样路径和核心token，不替换路径。
取消、编辑或导航会作废预览，迟到结果不能恢复确认；实际部署不可取消，不自动重试。
原生缓存写入与部署共用进程内锁，锁随阻塞任务真实结束才释放；前端操作锁也跨卸载保留。

提交记录只保留当前会话中的实际源、目标与完成/未确认结果，导航不会丢失未知结果，
但它不是当前文件扫描、所有权或清理授权；不持久保存确认token。
任何失败都保留“可能已有可执行部分文件”的警告；不自动回滚、删除、激活或执行。
Windows等非Linux x86_64主机明确拒绝。本功能不代表Qoder运行、登录或兼容性验收。

## Read-only deployment recheck / 只读部署重验

```sh
aisync qoder inspect-deployment --release 1.1.86 --deployment /absolute/private-parent/deployment
```

After restarting, explicitly select a previously deployed directory and its reviewed
release (1.1.85 default, or 1.1.86). No source cache is required. `payload_verified`
means this observation matched the fixed official payload length/SHA-256, strict
intent/completion records, exact three-member inventory and private filesystem
metadata. It does **not** establish who installed it, signature authenticity,
activation, runtime compatibility or authentication. Records remain unsigned.
`missing`, `incomplete`, and `unsupported_or_changed` are preservation observations,
not cleanup or retry permission. Exit zero means a report was produced, not success
of installation. Unknown releases/hosts and invalid explicit paths return errors.

The operation opens no-follow handles, checks current-user ownership, 0700 directory
and payload modes, 0600 single-link records, no extended attributes, and rechecks
identities/metadata/inventory after hashing. It never executes Qoder, creates locks,
changes permissions, repairs or removes files. Access times may change; no atomic
snapshot or protection against all same-user races is claimed. Reports always set
`execution_verified`, `activated`, `ownership_proven`, `deletion_authorized` and
`repair_authorized` to false, and `manual_review_required` to true. Payload fields
are the selected reviewed expectation, not evidence on non-verified states.

重启后显式选择既有部署目录与已审核版本即可重验，不需要原缓存。
`payload_verified` 只表示本次固定官方字节、严格记录和私有文件元数据匹配，
不代表安装所有权、签名、激活、运行兼容或认证成功。其他状态仅供保留与人工检查，
退出码为零只代表成功生成报告，不授予清理、修复或原地重试权限。
不执行 Qoder、不写锁、不修改权限或任何内容；读取可能更新访问时间，不能排除全部
同用户竞态。报告中的 payload 长度/hash 是所选版本的预期，失败状态不证明实际匹配。
