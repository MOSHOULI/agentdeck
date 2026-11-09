# Qoder native distribution plan / Qoder 原生分发计划

```sh
aisync qoder distribution-plan --target linux-amd64-baseline
aisync qoder verify-package --target linux-amd64-baseline \
  --archive /absolute/qodercli-linux-x64-baseline.tar.gz
```

This is a fixed **1.1.85** contract, not a rolling latest-version lookup. Run
`distribution-plan --help` for all 11 exact target IDs. Windows x64 requires
`windows-amd64-standard` or `windows-amd64-legacy`, never an ambiguous os/arch pair.
Selection is entirely explicit; a target may differ from the host. URLs, names and
SHA256 values are pinned from the [reviewed manifest](upstream/qoder/PROVENANCE.md).
The command does not fetch a URL, infer CPU features/libc/Windows build, or certify
platform suitability. ARM64 Windows documentation contradicts the manifest.

`verify-package` requires an existing absolute local path with the exact official
archive basename. It streams at most 256 MiB (+ one over-limit detection byte) in
64 KiB chunks, uses the audited no-follow/nonblocking reader on Windows and Linux
x86-64/ARM64, and rejects links, hardlinks, special files, changed files and oversized
input. Other reader platforms fail closed even if a distribution plan exists for
them. It never unpacks or executes the archive. Existing settings-inspection limits
remain 64 KiB. There are no locks, config/PATH writes, profile discovery, remote calls,
credential reads, installation receipts, or automatic downloads. Reads may update
access times. This is best effort, not a same-user race sandbox; mount I/O deadlines
and a multi-file atomic transaction are not provided.

Exit 0 from verification means **only matching the pinned upstream archive SHA256**.
Mismatch, invalid targets/paths, unsupported readers and I/O errors exit nonzero;
no file contents or dynamic OS errors are returned. CLI argument errors may echo
arguments. Checksums from the publishing source are not independent signature
verification. No Qoder runtime, login, installation, update, rollback, trust, GUI or
effective-configuration support is implied. Registry installation stays manual.

## 中文

新增命令提供固定 1.1.85 原生包计划和显式本地包校验，不安装、不执行、不解包，
也不自动下载或访问网络。用 `--help` 查看 11 个精确目标；Windows amd64 必须
明确 standard/legacy，不能按第一条匹配猜测。目标不证明与本机兼容，Windows
ARM64 的文档和分发清单仍矛盾。URL、文件名与 SHA256 固定于已审阅清单。

校验仅接受官方文件名的绝对路径，以 64 KiB 缓冲分块，归档最大 256 MiB，拒绝
链接、硬链接、特殊文件、超限和检测到的变化。仅审阅 Windows、Linux x86-64/ARM64
读取边界，其他主机失败关闭；提供平台计划不等于该平台可在本机验证。原配置
检查的 64 KiB 限制不变。不发现 HOME、不读取认证、不写配置/PATH/锁或所有权记录。
访问时间可能变化；同权限竞态和挂载 I/O 超时仍有边界。

退出码 0 仅代表包摘要与固定上游摘要相符，不代表独立签名、兼容、安装或运行验收。
错误/不匹配返回非零，不回显文件内容或原始系统错误；参数解析错误可能回显参数。
注册表继续手动安装，后续安全暂存、安装所有权和回滚需另行实现与测试。

A separate, opt-in [controlled archive staging command](QODER-CONTROLLED-STAGING.md)
can copy the reviewed Linux baseline archive into a new private cache. It does not
change the no-write behavior of `distribution-plan` or `verify-package`, and does
not enable installation or extraction. 另有需明确确认的[受控暂存命令](QODER-CONTROLLED-STAGING.md)，
仅创建全新私有归档缓存，不改变前述两个命令的只读行为，也不启用安装或解包。

The desktop now provides a separate [read-only plan and verification workspace](QODER-DESKTOP-VERIFICATION.md),
without download, staging, extraction or installation actions. 桌面现有独立[只读分发计划与包校验](QODER-DESKTOP-VERIFICATION.md)，不开放下载、暂存、解包或安装。
