# Fixed DSH native dependency archives

The source CLI can verify four exact native-dependency tarballs without extracting
or executing their contents:

```sh
aisync dsh verify-native-package --package node-pty --archive /absolute/node-pty-1.2.0-beta.15.tgz
aisync dsh verify-native-package --package koffi --archive /absolute/koffi-3.1.1.tgz
aisync dsh verify-native-package --package koffi-linux-x64 --archive /absolute/koffi-linux-x64-3.1.1.tgz
aisync dsh verify-native-package --package koffi-win32-x64 --archive /absolute/koffi-win32-x64-3.1.1.tgz
```

These are installation **prerequisite evidence**, not an installation plan or
permission to run a hook. Only the explicit selected archive is read. The command
has no downloader, package manager, module loader, extraction directory, runtime
probe, configuration writer, version/source/hash override or install flag. An
exit code of zero means the selected archive passed static verification, not that
DSH or its native dependencies are installed or working. It does not probe the
host or claim that the selected package matches this machine.

## Reviewed official sources (2026-11-11)

The production-declaration path from DSH to node-pty and Koffi is documented in
[the native chain](DSH-NATIVE-CHAIN.md). Koffi's own optional-dependency declarations
supply the two additional exact platform edges. Sources are fixed npm versions:

- [node-pty 1.2.0-beta.15 metadata](https://registry.npmjs.org/node-pty/1.2.0-beta.15)
  and [archive](https://registry.npmjs.org/node-pty/-/node-pty-1.2.0-beta.15.tgz):
  7,150,506 compressed bytes; 26,915,840 expanded TAR bytes; 49 regular members.
  SHA256 `114ac80c3fe075eff76217a4122d135576582695f49c03a5da3835cdfc2f89c5`.
- [Koffi 3.1.1 metadata](https://registry.npmjs.org/koffi/3.1.1) and
  [archive](https://registry.npmjs.org/koffi/-/koffi-3.1.1.tgz):
  302,173 compressed bytes; 1,848,832 expanded TAR bytes; 81 regular members.
  SHA256 `d6e3cb95f5d155d16cfe40f39ce09be18568c7f84dfb0b0722820a0da9e03b17`.
- [Koffi Linux x64 3.1.1 metadata](https://registry.npmjs.org/@koromix%2fkoffi-linux-x64/3.1.1)
  and [archive](https://registry.npmjs.org/@koromix/koffi-linux-x64/-/koffi-linux-x64-3.1.1.tgz):
  614,185 compressed bytes; 2,366,464 expanded TAR bytes; 5 regular members.
  SHA256 `0b4038a46282a354aab77d6d05059ac8a8fccb0dfd231b4c18acb8623953d1fe`.
- [Koffi Windows x64 3.1.1 metadata](https://registry.npmjs.org/@koromix%2fkoffi-win32-x64/3.1.1)
  and [archive](https://registry.npmjs.org/@koromix/koffi-win32-x64/-/koffi-win32-x64-3.1.1.tgz):
  399,006 compressed bytes; 1,050,624 expanded TAR bytes; 5 regular members.
  SHA256 `b0be9b6a916b9f5de5b97fb5de844efe1121f7ed44cafb481a1479c7c90aec9b`.

Raw metadata, all member sizes/modes/SHA256, archive SHA512 integrity declarations,
and selected unmodified manifests/lifecycle/loader sources are retained under
`docs/upstream/dsh/native-evidence/`. `artifacts.json` is a derived audit inventory,
not an upstream signed document. Source snapshots use `.txt` names and are inert
data; licenses are retained. Binary archives and native binaries are not committed
or uploaded as CI evidence. Exact fixture bytes are protected from Git CRLF
conversion and independently checked in Windows-style checkout regressions.

## What the sources establish

- node-pty's install command first checks whether a platform/architecture prebuild
  directory exists. It does not validate a binary ABI or prove successful loading.
  `npm_config_build_from_source=true` removes the prebuild tree before returning
  failure to trigger `node-gyp rebuild`. The postinstall cleans `build/Release`
  outside an allowlist and, on Windows, copies bundled ConPTY files. These are
  mutating lifecycle operations, not read-only checks.
- The node-pty archive has Linux x64/ARM64 `pty.node` prebuilds without Linux
  `spawn-helper`. Static native source places helper execution in the Apple
  branch; Linux uses `forkpty`. The missing Linux helper is **not reported as an
  installation defect**. Neither native branch was executed during this review.
- Koffi's install command invokes `cnoke.cjs --prebuild`. Its prebuild check
  starts Node to require the package, which loads native code. Failure may lead
  to CMake/build-tool invocation. Do not execute that check as a metadata probe.
- Koffi's Linux x64 optional package declares `os: [linux]`, `cpu: [x64]`, and no
  libc filter. It contains both `linux_x64/koffi.node` and `musl_x64/koffi.node`;
  its loader tries the former, then the latter. The Windows x64 package declares
  Windows/x64 and contains `win32_x64/koffi.node`. Neither platform manifest
  declares lifecycle scripts. This does not imply that requiring it is inert.
- `node-addon-api@^7.1.0`, other optional platforms, peer resolution and the full
  DSH dependency closure remain unresolved. The root's Node requirement remains
  `>=24.9.0`. Native ABI, operating-system/library versions, libc compatibility,
  build-tool availability and actual module behavior are not verified here.

## Verification contract and tests

The audited selected-file reader checks explicit absolute paths, exact basenames,
regular non-linked files and per-package byte bounds. The limits for existing
settings, root and subprocess reads are unchanged. Size, SHA256 and SHA512 must
match **before** any selected-byte decompression or TAR parsing. The parser then
checks bounded expanded size, exact member order/count/path/mode/content hashes,
header checksums, no links/extensions, zero padding, and the exact two-block zero
trailer. Selected source/manifest bytes must equal the embedded snapshots; their
reviewed declaration fields must also agree with the pinned registry metadata.

Reports explicitly retain `install_ready`, `closure_complete`,
`dependencies_resolved`, `runtime_verified`, `signature_verified`,
`lifecycle_executed` and `native_code_loaded` as false. Platform compatibility is
`unverified`. Same-source hashes verify integrity, not independent publisher
signatures. File checks are best effort, not protection against hostile same-user
races or mount I/O deadlines; access times can change.

Offline tests cover changed evidence, invalid metadata agreement, malformed
headers, links/extensions, path/mode changes, padding/trailer/duplicates, gzip
bounds/CRC/truncation/extra members, file links/oversize, redaction and unsupported
CLI overrides. Hosted Linux CI fetches all four exact official archives and runs
the actual built CLI on each valid archive and same-length corruption. Each call
must preserve fixture bytes/metadata and have exactly one traced initial CLI
exec, with no child exec. Missing/denied tracing fails closed. Only the result JSON
is uploaded. Local positive/corruption verification without that hosted trace
cannot substitute for the traced CI gate.

## 中文说明

新增命令只核验显式选择的四种固定原生依赖归档，不解包、不安装、不执行 Node、
生命周期脚本或原生模块，也不探测本机兼容性。它补齐安装前的归档、平台声明与
脚本来源证据，不能称为“DSH 已可安装”或“真实运行通过”。

特别注意：Koffi 的 prebuild 检查会实际加载原生代码，失败后可能调用编译工具；
node-pty 的安装/后处理也会删除或复制文件。不能把这些操作包装成被动探针。
Linux node-pty 没有 spawn-helper 与其 Apple 专用源码分支一致，本批不据此误报缺陷。

依赖闭包、node-addon-api 范围、其他平台包、ABI/libc 与真正运行仍未验证。现有 DSH
安装和配置写入能力保持关闭，PR #22 的强制网络隔离阻塞不变，不发布零散安装包。
