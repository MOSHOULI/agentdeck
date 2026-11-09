# Strict Qoder cache unpacking / Qoder 缓存严格解包

```sh
aisync qoder unpack-cache --cache /absolute/staged-cache \
  --destination /existing-parent/new-extracted-cache --yes
```

Linux x86_64 only; fixed Qoder 1.1.85 `linux-amd64-baseline`. This consumes the
[controlled archive cache](QODER-CONTROLLED-STAGING.md), not an arbitrary download
or caller-provided checksum. Other host platforms fail closed. **No installation,
activation or execution occurs.** No PATH, login, configuration, registry or MCP
changes occur. Output files, including `qodercli`, have mode 0600, not executable;
new directories are 0700. Existing destinations, even partial ones, are rejected.

## Validation boundary

- Source cache directory and files must be private and owned by the current user;
  regular files must have one link. Parent traversal, symbolic links and special
  files are rejected using pinned directory handles and no-follow/nonblocking opens.
- Initial/completion records are bounded to 64 KiB, strictly deserialized with
  unknown/duplicate fields rejected and fixed state, version, target and metadata
  checked. Records are not authorization, signatures or sufficient trust evidence.
- The archive is copied from one checked source handle into a new exclusive private
  file while hashing. Its fixed 78,587,775-byte size and upstream SHA256 must match.
  Source identity, metadata and record bytes are rechecked. Subsequent decoding uses
  the same private copy handle, not a reopened source pathname.
- Mature `tar` raw-entry iteration and `flate2` single-member gzip decoding are used;
  no `tar.unpack` path-based writes and no custom general-purpose tar parser. Only
  raw `./` directory followed by raw `./qodercli` regular file are accepted. USTAR
  header magic, empty prefix/link fields and exact member sizes are required.
  PAX/GNU extensions, sparse files, links, special files, duplicate/extra entries,
  absolute/traversal paths and nonzero member padding are rejected.
- Expanded stream length is exactly 182,545,920 bytes; the payload is exactly
  182,543,840 bytes. All bytes after the payload must be zero, including tar end
  markers and padding. A second zero block is required. Gzip is consumed to EOF
  for CRC/ISIZE validation; concatenated members and all trailing bytes are rejected.
- Compressed bytes are hashed again during decoding, and decoded payload bytes are
  hashed while writing. The actual output file is then reread through its original
  handle and hashed again. Payload SHA256 must equal
  `8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b`.
- File identity/permissions/link count and named output identity are checked before
  atomic no-replace publication of the final completion record. Source archives,
  their settings and installed Qoder instances are never overwritten.

Output contains `archive.tar.gz`, non-executable `qodercli`, initial `incomplete.json`
and final `complete.json`; `completion.pending` can remain on failure. The final
state is `verified_payload_unpacked_not_installed`. Initial intent remains even
after success. Failure returns nonzero and preserves partial output for inspection,
not automatic deletion or in-place retry. A late sync or stdout failure can happen
after publication; inspect before deciding what to do. The record is not a promise
of runtime compatibility or authorization to execute later.

The fixed hashes are same-source integrity, not independent signing/notarization.
Same-user malicious races remain outside this best-effort boundary; the directory
creation/open gap is not a sandbox. Mount I/O has no hard deadline. Full power-loss
recovery, installer ownership, update/rollback, authentication and Qoder runtime
behavior are not verified. Tests with small malicious archives exercise only the
private parser; production has no fixture checksum/size override.

## 中文

此命令仅在 Linux x86_64 将固定 Qoder 1.1.85 baseline 受控缓存解包到显式全新
私有目录，须 --yes。不安装、不激活、不执行、不改 PATH/配置、不登录。目录0700，
所有文件0600，qodercli 也不开执行位；既有目录或旧失败目录绝不覆盖复用。

先用安全句柄核缓存目录/记录/归档权限、身份、单链接及固定元数据。记录限制64KiB，
重复/未知字段拒绝；归属记录不是授权或签名。源归档从同一句柄复制并核固定摘要、
长度；后续只用私有副本原句柄解码，并复查原缓存记录与身份。

使用成熟 tar/flate2 库的 raw entries 与单gzip流，不调用自动路径落盘 unpack。
只允许原始 ./ 目录与 ./qodercli 普通文件两条，拒绝路径穿越、绝对路径、链接、
特殊文件、稀疏/PAX/GNU扩展、重复/额外条目及非零padding。展开总长度固定
182545920，payload固定182543840字节，末尾tar双零块及全零padding必须完整；
解码至gzip EOF校CRC/ISIZE，拒绝多gzip成员和任何尾随数据。解码期间重算压缩流
摘要，payload写入后从原句柄再次读取核固定成员摘要，最后才原子无覆盖发布完成记录。

完成状态只是“已解包缓存，未安装”；初始 incomplete 记录保留。失败保留私有partial，
不自动清理、不原地重试。可能在发布后发生同步或输出失败，须先检查。完成记录亦非
未来执行授权。不能据此宣称运行兼容、登录、GUI、更新回滚或断电安全；同权限恶意竞态
不是沙箱，挂载I/O无硬超时。合成恶意测试不增加任何生产checksum/size绕过入口。
