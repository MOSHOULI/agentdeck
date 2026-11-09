# Controlled Qoder archive staging / Qoder 受控归档暂存

```sh
aisync qoder stage-package --target linux-amd64-baseline \
  --archive /absolute/qodercli-linux-x64-baseline.tar.gz \
  --destination /existing-parent/new-agentdeck-cache --yes
```

Only the fixed 1.1.85 Linux amd64-baseline archive on a Linux x86_64 host is in this
slice. Other targets/hosts fail closed. The existing eleven distribution plans and
read-only package verification are unchanged. Staging is **not installation**.
No archive members are inspected, extracted or certified safe; no Qoder process,
installer, login, PATH change, configuration discovery or global registration occurs.

The caller chooses a NEW absolute destination under existing parents. Directory
handles are opened component-by-component with no-follow flags. `mkdirat` creates
the destination without replacing any existing file, directory or dangling link.
The new directory is mode 0700; all files are mode 0600, never executable. Leaf
files use exclusive creation. Existing Qoder instances and even previous partial
staging directories are never reused or overwritten.

The same audited source handle supplies streaming bytes to a private copy and the
fixed SHA256 calculation (maximum 256 MiB). Only a matching fixed upstream digest
can progress. AgentDeck then hashes the copy again via its original private handle,
checks identity/permissions/link count, and checks parent/destination identity. The
source pathname is not reopened to extract or execute anything. This is best effort,
not isolation against a malicious same-UID process; filesystem/mount I/O deadlines
are not provided. Same-source hashes are not independent signature verification.

Files in the requested destination:

- `incomplete.json`: initial intent/ownership record, deliberately retained even
  after success. Its presence alone never proves completion or installation.
- `archive.tar.gz`: verified archive on success; possibly partial or unverified on
  failure. It is not executable and is never unpacked by this command.
- `complete.json`: published last using atomic no-replace rename after archive and
  record flush. Its state is `verified_archive_staged_not_installed`, with fixed
  version, target, checksum and byte count. It is a cache record, not a signature or
  trust assertion against user tampering, authorization, or permission to execute later.
- `completion.pending` may remain if final publication fails.

The command flushes files and directory entries, but does not promise recovery from
all power loss/filesystem failures. If any step fails, it returns nonzero and leaves
whatever was created for inspection; it never recursively deletes a possibly
replaced directory. A late sync or CLI stdout failure may occur after the completion marker was
published: inspect rather than retrying in place. To try again, choose another new
destination. Manual cleanup of the explicit partial cache is separate from this
command. No automatic resume, activation, uninstallation or garbage collection is
implemented. API callers are responsible for obtaining write authorization; CLI
requires `--yes` before any new destination is created.

## 中文

此片仅支持 Linux x86_64 主机暂存固定 1.1.85 的 linux-amd64-baseline 归档。
仍是缓存，**不是安装**；不验证归档成员、不解包、不执行 Qoder、不登录、不修改
PATH、配置或全局注册。其余平台分发计划和只读校验不变。

必须显式指定全新绝对目标目录，父目录须已存在且不经过链接。通过逐层 no-follow
目录句柄、mkdirat 原子拒绝覆盖、文件独占创建，目录权限为 0700、文件为 0600，
不开放执行位。已有文件、目录、悬空链接及旧暂存目录全部拒绝复用。

同一安全打开的源句柄有界流式复制并计算固定 SHA256，最大 256 MiB；匹配后再
从私有副本原句柄复核摘要、身份、权限及链接数，不从源路径重新打开用于解包。
同权限恶意进程竞态仍不是沙箱，挂载 I/O 不提供硬超时；上游同源摘要不等于独立签名。

初始 incomplete.json 会保留；只有最后原子无覆盖发布的 complete.json 表示
“已核验归档暂存，未安装”。archive.tar.gz 在失败时可能不完整，不能擅自执行。
完成记录亦不是对抗用户篡改的证明、执行授权或今后直接运行的凭据。失败返回非零且保留现场，不自动递归清理、
续传或覆盖重试；可能在完成记录发布后才发生同步或 CLI 输出失败，须先检查。另选新目录
可重新开始，旧目录手动清理另行处理。CLI 必须显式 --yes；不宣称断电安全。

A separate [strict cache-unpack command](QODER-CONTROLLED-UNPACK.md) can now validate
this completed cache and extract the fixed payload to another new non-executable
private directory. `stage-package` itself still never extracts or installs.
另有[严格缓存解包命令](QODER-CONTROLLED-UNPACK.md)，需再次明确确认，写入另一个全新
非可执行私有目录；stage-package 本身仍不解包、不安装。
