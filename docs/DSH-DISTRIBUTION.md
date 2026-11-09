# DSH root package contract / DSH 根包分发契约

## English

```sh
aisync dsh distribution-plan
aisync dsh verify-package --archive /absolute/dsh-0.2.2-rc.1.tgz
```

This **offline root-archive-only** capability pins `@deepseek-ai/dsh` **0.2.2-rc.1**, a prerelease. npm `latest` and `next` pointed to it when reviewed on 2026-11-09; tags can move, and this is not a promise to follow latest. No caller-supplied version, URL, checksum or target override exists. The plan is not a download or installation command.

- Official tarball: https://registry.npmjs.org/@deepseek-ai/dsh/-/dsh-0.2.2-rc.1.tgz
- Exact archive length: **25,197 bytes**.
- SHA256: `ad21968aed0337a326b853507a23b30eacbbec4d67ab07f3b6fde69b75b19cd6`.
- SHA512 SRI: `sha512-ijR9UEVYkG15Q/Q79JzvzLBzQOQfc63VLr9n6flpwHPhr5IKn2UxthnSoto/P9l7qGd7pPvI1Ra6TW7LH3oKpg==`.
- Declared Node requirement: `>=24.9.0`, not a tested runtime.

The explicit local file must have the exact archive basename and an absolute path. The existing selected-source reader is reused unchanged with its **64 KiB ceiling**, regular-file/no-link/hardlink/identity checks on audited Windows and Linux x86_64/ARM64 hosts. Both hashes and the exact length are checked against the same read bytes. Other hosts fail closed. This reads no sibling manifest, dedicated credential, dependency or configuration file. It never unpacks, invokes npm/npx/Node/DSH/shell, installs, stages, changes PATH or writes a profile. Reads may update access times; same-user ancestor races, multi-operation atomicity and filesystem I/O deadlines are not solved.

Success emits `state: root_archive_verified`, with `dependencies_resolved`, `installed`, `runtime_verified`, `signature_verified` all false, and `platform_compatibility: unverified`. Root integrity does not establish a self-contained installation: this root declares **90 direct dependencies, including 8 ranges**, not a pinned transitive closure. Its own `scripts` is absent, but a dependency (`@deepseek-ai/dsh-subprocess-local`) declares a `postinstall` lifecycle action. Absence of root `os`/`cpu` constraints is not proof of platform compatibility. Same-source npm integrity hashes are not independent signature/authenticity verification.

Missing, renamed, unsafe, truncated, changed, oversized or mismatching input fails nonzero, with a fixed verification error and no successful JSON report. Clap argument errors may echo arguments; verification errors do not echo file paths/content. The fixed contract is separate from passive DSH profile inspection and adds no configure/restore/runtime loader capability. Installing or executing the dependency tree requires a separately reviewed lifecycle contract.

Evidence: [minimal reviewed upstream metadata](upstream/dsh/root-0.2.2-rc.1.json) records original registry-response SHA256, tarball/SRI, package-manifest hash and dependency declarations. It is a derived excerpt, not the full registry response or a cryptographic signature. Official sources: [npm registry](https://registry.npmjs.org/@deepseek-ai%2fdsh), [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness). Only root package bytes are verified by the command; no archive members are interpreted at runtime.

## 中文

本片是**离线根归档验证**，固定 `@deepseek-ai/dsh` **0.2.2-rc.1 预发布版**。2026-11-09 核查时 npm 的 latest/next 指向此版本，但标签会移动，本工具不自动追随最新版本。用户不能覆盖版本、URL 或摘要；计划不是下载或安装命令。

命令如上。必须明确给出绝对路径和精确文件名 `dsh-0.2.2-rc.1.tgz`。复用既有安全读取器，不扩大其 **64 KiB** 上限；从同一受检读取的字节核对 **25,197 字节、SHA256 与 SHA512 SRI**。Windows 与 Linux x86_64/ARM64 使用已审阅的普通文件、链接/硬链接及身份前后检查；其他平台失败关闭。不读取同目录清单、独立凭据、依赖或配置，不解包、不调用 npm/npx/Node/DSH/shell、不安装、不暂存、不改 PATH 或 profile。访问时间可能变化，同用户路径竞争与文件系统 I/O 阻塞等限制仍存在。

成功仅输出 `root_archive_verified`，依赖已解析、已安装、运行已验证、签名已验证均为 false，平台兼容性为 unverified。Node `>=24.9.0` 只是官方声明。根包有 **90 个直接依赖，其中 8 项为范围**，不是已锁定的传递依赖闭包。根包自身没有 scripts 不代表依赖无生命周期脚本：`@deepseek-ai/dsh-subprocess-local` 声明了 postinstall。未声明 os/cpu 不等于跨平台可用；同源摘要不等于独立签名验证。

缺失、重命名、不安全、截断、变动、超限或摘要不符均非零退出，不输出成功 JSON，也不回显验证路径或内容；参数解析错误可能显示用户参数。此功能独立于 profile 只读检查，不开启配置、恢复或真实运行能力。最小上游证据是注明来源与摘要的摘录，不是签名；运行时验证不解析归档成员。真正安装或执行依赖树仍需另行审查生命周期契约。

## CI root-integrity acceptance / CI 根包完整性验收

The CI helper `scripts/dsh_distribution_smoke.py` fetches only the fixed official version metadata and tarball, rejecting redirects, non-identity encoding, duplicate JSON keys and oversized responses, with bounded time/read budgets. It requires a fresh destination and retains partial evidence on failure rather than overwriting a cache. The actual built CLI must accept the pinned archive and reject a same-size one-byte-corrupted copy; snapshots check that verification does not change fixture bytes or metadata. Archives, extracted content and dependencies are not uploaded as artifacts. This is npm root-integrity evidence, not runtime, platform, dependency-closure or lifecycle-script acceptance. Nothing is extracted or executed.

CI 辅助脚本仅获取固定官方版本元数据与根归档，拒绝跳转、非恒等编码、重复 JSON 键与超限响应，读取及时间预算有界。目标目录必须全新；失败保留部分现场，不覆盖缓存。实际构建的 CLI 必须接受固定根包，并拒绝同长度单字节损坏副本；前后快照确认校验不改变字节与元数据。不上传包、载荷或依赖，不解包、不执行。这仅证明 npm 根包完整性，不代表运行、平台、依赖闭包或生命周期脚本已验收。

The CLI subprocess pipe reader retains at most 64 KiB per output stream and
fails immediately on excess; its 30-second deadline kills and reaps the child.
The hosted worker owns a dedicated process group; a 240-second outer watchdog
kills that group, including an in-flight CLI, with the CI step allowing five
minutes. The corrupt archive must produce a non-signal nonzero exit and the
fixed integrity-mismatch category, not merely crash. Offline regressions use
real Python processes to test output overflow and timeout cleanup; they never
run package content. These checks do not replace the official-byte CLI run.

CLI 输出每路最多保留 64 KiB，超限立即失败；30 秒超时终止并回收子进程。
240 秒外层 watchdog 终止专用 worker 进程组，覆盖运行中的 CLI；CI 步骤五分钟。
坏包须产生明确完整性错误，不以崩溃代替拒绝。离线溢出/超时测试运行 Python
测试进程，不执行包内代码，也不替代官方真实包校验。

Local integration evidence (2026-11-09): the freshly built aisync passed the
fixed official metadata/tarball download, valid-package report, same-size
corrupt-package integrity rejection and fixture zero-write checks. The version
metadata response SHA256 was
`d98830e89cf9b064e405fd08985b86dbdfda3ee396498f5981d01b9be0769881`.
The downloader uses the environment's normal system proxy routing, like the
existing Qoder downloader; it does not alter proxy variables or TLS checks.
An initial direct-DNS attempt was unavailable before any CLI call; normal
routing then succeeded. Hosted CI remains a separate required check.

2026-11-09 本地新 aisync 已完成官方固定元数据/归档、正常包/损坏包及零写验收，
未执行 DSH。使用环境原有代理路由，没有修改代理或 TLS 设置；托管 CI 仍需独立通过。
