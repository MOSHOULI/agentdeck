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
