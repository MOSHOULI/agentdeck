# DSH reachable native dependency archive / 可达原生依赖归档

## Scope

The root remains `@deepseek-ai/dsh@0.2.2-rc.1`. Its exact production dependency
`dsh-sdk-minimal@0.2.2-rc.1` declares `dsh-subprocess-local@0.2.2-rc.1`, which
in turn declares `node-pty@1.2.0-beta.15` and `koffi@3.1.1`. These two paths use
`dependencies`, not peer/dev dependencies. The reviewed registry declaration
chain establishes reachability, **not installation, a complete consumer lock or
runtime compatibility**. The existing 90 root edges and eight unresolved ranges
remain, and other dependency branches are not resolved.

`aisync dsh verify-subprocess-package --archive /absolute/dsh-subprocess-local-0.2.2-rc.1.tgz`
verifies the selected fixed archive without running npm, Node, DSH or lifecycle
hooks. This is a separate bounded contract from the smaller root package; it
must not silently relax the root verifier's 64 KiB limit. The archive itself is
not committed or uploaded as CI evidence.

## Reviewed sources and exact bytes (2026-11-11)

- Root pinned manifest: existing `dependency-evidence/root-package-0.2.2-rc.1.json`.
- SDK exact metadata: https://registry.npmjs.org/@deepseek-ai%2fdsh-sdk-minimal/0.2.2-rc.1
  retained raw 4155 bytes; SHA256
  `c38cf52c6f73815f3a5796afd7095ff8c2e680ce09aca60d158878dbc0717f24`.
- Subprocess exact metadata: https://registry.npmjs.org/@deepseek-ai%2fdsh-subprocess-local/0.2.2-rc.1
  existing raw 2939 bytes; SHA256 remains
  `4f7bf72dcfdc4769c0c63648d5c1c7276089e39d016a9af305455c816dae7de8`.
- Archive: https://registry.npmjs.org/@deepseek-ai/dsh-subprocess-local/-/dsh-subprocess-local-0.2.2-rc.1.tgz
  77649 bytes; SHA256
  `ac2aae27078f34ccc068c62371672da84fb83bf9230bc26b24e1ecaee9d4ae97`;
  SRI `sha512-/fakFwvyt20BMZTD+Dm89uHyFFMHZzWPmvHWDj/m7Q8wpgeWxX2lFtS2nVFVaqZBmgGUWkUr4tPGPa6a0gLyMw==`.
- Expanded TAR: 279040 bytes, 35 regular entries, all mode 0644, total file content
  249439 bytes. The derived `subprocess-archive-layout-0.2.2-rc.1.json` records
  each exact path, size and SHA256. No directory/link/special members were present.
- `package/package.json`: unmodified 1726 bytes; retained as
  `dependency-evidence/subprocess-package-0.2.2-rc.1.json`; SHA256
  `48d50b0e9d019954a8689b39af2bc20b03ce68abe5dde3351b2e9e27ee842b9f`.
- `package/scripts/ensure-spawn-helper.mjs`: unmodified 572 bytes; retained as
  `dependency-evidence/ensure-spawn-helper-0.2.2-rc.1.mjs`; SHA256
  `ca5509febf1e6ec1356df121835ebe5ed2f9cace4bdc2ba6d83d41c7e45e0f1b`.

Hashes are same-source integrity evidence, not independent signature verification.
The SDK metadata is raw registry data; the archive-layout list is a derived audit
artifact and is not presented as an upstream signed manifest. Git checkout tests
protect raw snapshots from Windows newline conversion.

## Lifecycle boundary

The declared postinstall is `node scripts/ensure-spawn-helper.mjs`. Static reading
shows it resolves `node-pty`, derives its package root, checks a platform/architecture
prebuild `spawn-helper` and `build/Release/spawn-helper`, and applies `chmodSync`
0755 to each existing candidate. No hook was run. This observation neither proves
safe behavior in a real dependency environment nor validates those helper bytes.
The imported dependency resolution, filesystem candidates, native `node-pty` and
`koffi` hooks/platform packages remain outside this archive's execution acceptance.
Do not turn “script reviewed” into “safe to run”, “installed” or “working agent”.

## Verification gates

CI fetches only fixed official version metadata and the exact archive with no
URL/hash overrides. The fetcher rejects redirects, unsupported transfer/content
encodings, duplicate JSON keys, wrong identity/lifecycle declarations and bounded
read/time overruns. It verifies length, SHA256 and SHA512 before invoking the built
AgentDeck CLI. A fresh directory is required; existing or partial state is not
silently overwritten. The same real CLI must accept official bytes and reject a
same-length one-byte-corrupted archive, with pre/post byte and metadata snapshots
unchanged. Only the small result JSON is uploaded, never archives or hook contents.
Offline transport/contract tests are not a substitute for this real CI gate.

## 中文说明

本批把原先三个“示例元数据”补成两条有生产依赖声明支持的可达路径：根包 →
SDK minimal → subprocess local → node-pty / koffi。它不等于完整依赖闭包、
消费端锁文件、跨平台安装或实际运行成功。现有未解析范围仍明确保留。

新增命令仅静态核验用户显式选择的固定 subprocess 归档。真实下载摘要、包内
清单和 postinstall 原文匹配，才可以报告对应静态验证通过；不执行 postinstall，
不调用 npm/Node/DSH，不修改 PATH、配置或用户安装目录。该归档 77649 字节大于
原根包读限，必须使用独立有界合同，不能放宽原根包核验语义。

脚本静态行为是定位 node-pty 的 spawn-helper 候选并在存在时 chmod 0755；
这不是安全背书。依赖解析结果、原生构建、平台包、helper 字节与实际运行均未
由该检查证明。CI 仍须通过官方归档正例、等长损坏反例及文件不变检查；离线
单测、来源快照不能冒充真实验收。没有发布新的安装包。

The subprocess-archive CI lane additionally requires `strace` for both successful
and corrupted-input CLI invocations: exactly one successful initial AgentDeck CLI
exec and no child exec/execveat are allowed. Missing/denied/truncated tracing fails
the gate; there is no untraced fallback. Trace files have a 64 KiB process file-size
limit, output is separately bounded, and the supervising worker imposes a hard
process-group deadline. Only summary counts are published. A direct local CLI
pass where ptrace is unavailable cannot substitute for this hosted trace gate.

真实正反例均要求执行追踪证明只有一次初始 AgentDeck CLI exec；任何额外执行、
缺失或被拒绝的追踪均使验收失败，不允许退化为未追踪通过。此边界仅覆盖这两次
静态核验命令，不代表任意未来安装或平台运行行为安全。

## Fixed native archive follow-up

A separate [four-package native archive verifier](DSH-NATIVE-ARCHIVES.md) now covers
node-pty, Koffi and Koffi Linux/Windows x64 archive bytes and reviewed source snapshots.
It does not change this subprocess command’s scope or establish installation, native
loading, lifecycle safety or full dependency closure.
