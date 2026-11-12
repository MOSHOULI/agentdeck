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

## Explicit additional release 1.1.87 / 显式新增版本

Read-only retrieval on 2026-11-12. Default remains 1.1.85; this is an explicitly
selected reviewed release, **not a claim of latest version**. The official CLI
changelog already lists 1.1.88 dated November 12 (title generation, MCP tools and
new clear fixes); 1.1.87 is dated November 11 (stability/UX). At this review the
rolling distribution and release-note timeline differed; no 1.1.88 contract is
implicitly admitted by this increment.

- Pinned official manifest: https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/channels/1.1.87/manifest.json
- Retained `manifest-1.1.87.json`: 3358 bytes, SHA256
  `5bea07a5bb10187aac873a1b3c5f7362e0b18f464daf4612cdc804cde31cd8ba`;
  published_at `2026-11-11T12:07:29.788Z`.
- All eleven target identities and metadata (os/arch/runtime/variant/minimum Windows
  build) are unchanged from 1.1.86. All archive URLs are version-pinned and all
  eleven digests changed. Windows standard is still Bun >=17763, legacy x64 is
  Node SEA. Linux baseline/optimized/musl remain distinct selections.
- The installation guide https://docs.qoder.com/cli/installation still says Windows
  ARM64 is unsupported while the manifest lists it. This contradiction is retained,
  not resolved by assuming a downloadable artifact is compatible.
- Actual Linux x64 baseline archive: 78,554,027 bytes, SHA256
  `738ee635eb868f5963526779ce023addbd63377b77861060a4c2af53bb83c2db`,
  equal to the same-source manifest's pinned checksum.
- Expanded TAR: 182,468,096 bytes. Single gzip stream, no trailing compressed data;
  raw `./` directory followed by raw `./qodercli` regular file, USTAR, no prefix,
  links or additional entries; both header checksums valid, 32 payload padding
  bytes followed by 1024 zero terminator bytes.
- Actual payload: 182,466,016 bytes, SHA256
  `61e318f6c4d2ebae319aecfce2f18472c51e7b413a546accddaeb82e2592af2e`.
  Structural inspection hashed the payload in memory without extracting an
  executable, setting execute bits, invoking it or running an installer script.

Only the Linux x64 baseline archive was downloaded for structural inspection.
Other target hashes are manifest contracts, not evidence of runtime compatibility.
Same-source checksums and unsigned receipts are not independent signatures.

本次仅显式新增 1.1.87，默认保持 1.1.85，不追随 latest。更新日志已列 1.1.88，
不能称 1.1.87 为最新，也不能把发布说明自动升级为受审分发契约。11 平台元数据与
前版一致但摘要全部变化；Windows ARM64 文档/清单矛盾继续保留。仅实读 Linux x64
baseline 官方归档并核对结构与完整载荷摘要，未运行、未提取为可执行文件、未赋执行
权限。目录条目、同源 hash、unsigned record 均不证明签名、所有权、运行或登录成功。
