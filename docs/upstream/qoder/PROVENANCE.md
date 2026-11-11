# Qoder 1.1.85 distribution provenance / 分发来源

Read-only retrieval on 2026-11-09. No installer or downloaded executable was run.

- Official installation guide: https://docs.qoder.com/cli/installation
- Official changelog: https://changelog.qoder.com/cli (1.1.85 dated 2026-11-07).
- Version-pinned manifest: https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/channels/1.1.85/manifest.json
- Rolling manifest: https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/channels/manifest.json
- The two manifest responses were byte-identical; retained snapshot SHA256:
  `a78234316fe2065cdae6fc17ac425bbc2ae11006d04b6ab5216fd6093597e4de`.
- Bootstrap shell script https://qoder.com/install was statically inspected, SHA256
  `1e850fa3375bdccd179215f91638f4065d912c5dbddbd346d0ba352bad8ca9be`.
- PowerShell bootstrap https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/install.ps1 was statically inspected, SHA256
  `870b0f1db2b7fe2e8ad818e761f947432d7d8f26447cf5090ee6f1332e0140df`.

Both scripts delegate installation to the downloaded executable's `install` command;
this can place files, alter PATH and maintain installation state/telemetry. AgentDeck
does not run either script. The manifest contains per-archive SHA256, not an
independent signature. Same-source checksums do not protect against compromise of
that source. npm metadata also reports 1.1.85, SHA512 integrity and registry signature
fields, but those signatures were not verified and do not authenticate native archives.

Windows ARM64 is explicitly listed by the manifest/PowerShell selector despite
contradictory installation prose. Windows x64 has both Node SEA legacy and Bun standard
variants; standard requires build >=17763. Linux has distinct optimized/baseline/musl
variants. The bootstrap checks multiple CPU flags for optimized Linux x64. Therefore
AgentDeck requires explicit exact target IDs; it does not infer compatibility or
silently choose the first os/arch match. OpenHarmony is manifest-listed, not tested.

仅保留公开分发清单及来源证据，不运行脚本。校验哈希来自同一上游，不等于独立签名。
Windows ARM64 文档与清单矛盾、Windows x64 多变体与 Linux CPU/libc 差异均不通过
猜测消解。平台条目不证明兼容性；包核验不证明安装完成。npm 签名字段未验签，
也不适用于原生包。后续更新须重新审阅清单、来源、平台及摘要，不跟随 latest 自动执行。

## Explicit additional release 1.1.86 / 显式新增版本

Read-only retrieval on 2026-11-11; the preceding 1.1.85 evidence is retained.
The default remains 1.1.85. Selecting 1.1.86 does not migrate an old cache or
installation and does not opt into following `latest`.

- https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/channels/1.1.86/manifest.json
- Raw retained `manifest-1.1.86.json`: 3358 bytes, SHA256
  `f323905da3f75088b1e9ae4cf9b0fef6f117879f4f6942b609e92228a533fa78`.
- At retrieval, rolling manifest had identical bytes; published_at
  `2026-11-09T13:05:36.036Z`. All eleven target IDs remain present.
- Official CLI changelog dated 2026-11-09 describes enterprise command-rule
  management. This is upstream release information, not AgentDeck runtime acceptance.
- Linux x64 baseline archive: 78,537,803 bytes, SHA256
  `9673b64b8f651fc883ea8c29fbd61ff19bf78db02d30db6ad4892b58ab3c9b7e`.
- Payload: 182,412,768 bytes, SHA256
  `fccd0ce90d1119f6bb213146de6e45358db0c89585048c31315ddb180c925ae8`.
- Expanded TAR: 182,414,848 bytes. Single gzip member without trailing data;
  raw `./` directory followed by raw `./qodercli` regular file, USTAR headers,
  no prefix/link/extra entries; 32 padding bytes plus 1024 zero terminator bytes.
  Both header checksums and the payload hash were checked without execution.

Same-source checksum is not independent signature verification. Windows ARM64
manifest/documentation disagreement remains unresolved. Only the Linux x64
baseline archive was downloaded and structurally inspected in this investigation.

新增版本显式选择，保留旧版默认及来源证据；不是自动升级。真实运行、认证、
在线服务与其他平台兼容性未由包结构检查证明。不得把旧版本验收冒充新版验收。
