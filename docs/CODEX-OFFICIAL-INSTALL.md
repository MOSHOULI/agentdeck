# Pinned Codex filesystem install / 固定 Codex 官方包安装验收

Development verification only. This does not enable a new adapter or publish an installer.

## English

### Linux x86_64

`scripts/codex_official_install.py` verifies a real npm global installation of the
fixed official `@openai/codex` **0.167.0** package and its Linux x86_64 native
optional package (`@openai/codex@0.167.0-linux-x64`, installed under the alias
`@openai/codex-linux-x64`). This is not the synthetic npm/Codex substitute used by
the separate WebView test.

Before invoking npm, the harness downloads the two fixed official registry
archives, verifies their pinned SHA-512 integrity, and reads a bounded archive
inventory without extraction or execution. It checks the exact package identity,
entry point, six fixed optional platform aliases and absence of required/peer/
bundled dependencies and lifecycle scripts. Native platform identity must match.
The expected inventories are 3 root files (13,570 bytes) and 45 native files
(458,593,658 bytes). Integrity is not an independent publisher signature check.

The trusted preinstalled Node/npm runs once with a fresh private HOME, global
prefix, cache, temporary directory, and empty user/global npm configuration.
The environment does not inherit credentials, provider variables, proxies,
`NODE_OPTIONS` or npm configuration. Both default and `@openai` registry are
fixed to `https://registry.npmjs.org/`; `--ignore-scripts` is mandatory, automatic
fetch retries and audit/fund/update requests are disabled. npm installs the
already verified root archive and resolves its fixed optional alias online.
The separately downloaded native archive is the post-install byte authority.

After npm exits successfully, every installed root and native package file must
match the pinned archive inventory (size, SHA-256 and executable-bit state).
Unexpected files, packages, directories, links, hard links, writable-by-others
files, missing native payloads and a changed launcher are rejected. The only
launcher is the expected `bin/codex` symlink. HOME/config/data/state stay empty,
and the empty npm configuration files must remain unchanged. npm's scoped
cache/log/temp activity is allowed. The temporary verification tree is removed
afterward; this does not install into the person's actual global prefix.

Downloads have fixed source, length, size and duration checks. npm has a
five-minute process-group deadline and bounded captured output. Failures have
fixed classifications; package output is not published. No uncertain operation
is retried. A newly reserved `--evidence` JSON records success or failure, always
keeping the unverified boundaries explicit. A failed check may have involved a
partial temporary installation before cleanup; it is not a success claim.

Run with trusted official developer tools and a **new** evidence path:

```sh
python -B scripts/codex_official_install.py \
  --node "$(command -v node)" --npm "$(command -v npm)" \
  --evidence /tmp/codex-install-evidence.json
```

Offline regression checks:

```sh
python -B scripts/test_codex_official_install.py
```

### Not verified

- Codex is never executed, including `--version` or `--help`.
- No lifecycle script runs; this does not establish script compatibility.
- The AgentDeck install adapter is not invoked, because it executes the installed
  agent for version/provenance detection. Its separate synthetic GUI acceptance
  is not promoted into real-package acceptance by this test.
- No login, model call, personal configuration, automatic upgrade or uninstall.
- No runtime network isolation claim: downloads and npm contact the official
  registry. This is not a hostile-code sandbox or protection against same-user
  concurrent filesystem substitution. Existing blocked runtime isolation gates
  remain blocked, not bypassed.
- Other versions, operating systems, architectures and npm layouts are not
  implied supported. No installer or release is published.

## 中文

本验收通过真实 npm，把固定官方 Codex **0.167.0** 根包及 Linux x86_64 平台包
安装到一次性的私有目录。不是界面验收中的合成替身，也不会写入用户实际的全局安装目录。

执行 npm 前，先下载两份官方归档，核对固定 SHA-512，限制归档数量、大小、路径、
类型及权限，逐文件记录摘要。根包必须只有固定的平台可选依赖，不能出现生命周期脚本、
额外必需依赖或改变的入口；平台包身份也必须一致。摘要匹配不等于独立验证发布者签名。

npm 使用空 HOME、独立 prefix/cache/tmp、空 user/global npmrc，不继承密钥、代理、
模型服务环境变量或 NODE_OPTIONS。默认与 @openai 作用域均固定为官方 registry，
强制禁用脚本，禁用自动重试和审计等附带请求。安装完成后，核对根包与平台包全部文件
的字节、大小和执行位；缺文件、多出包、异常链接、来源不符或写入配置均判定失败。
缓存与临时文件允许写入。验收结束清理自身临时目录，不进行用户软件卸载。

只验证真实 npm 的文件安装与目录关联，不运行 Codex，包括不运行 --version/--help，
不验证登录、模型调用或真实 AgentDeck 安装适配器。后者会运行已安装程序来检测版本，
仍需要独立的运行隔离验收。本测试需要联网下载，不能当作“已实现网络隔离”；原本
受阻的运行时隔离要求保持不变。无新安装包发布，其他平台、版本和 npm 布局不自动算通过。


## Windows x64 / Windows x64 官方包布局验收

### English

The Windows lane is a separate real-package installation/layout gate. It does
not convert the Linux verifier into a cross-platform success claim. It runs on
an ephemeral GitHub-hosted Windows runner with the official Node 24/npm setup,
using the pinned official Codex **0.167.0** root archive and Windows x64 optional
package (`@openai/codex@0.167.0-win32-x64`, installed under
`@openai/codex-win32-x64`). This is a fixed acceptance sample, not a promise that
the pinned version remains the latest. The native inventory contains 47 files
with 501,483,647 expanded bytes.
Lifecycle scripts remain disabled. Codex and its generated launchers must never
be executed, even for a version check. A layout test can read executable bytes
without executing them.

The Windows gate is mandatory inside the existing native Windows job, before
building the AgentDeck installer. Its separate JSON artifact is tied to the
exact `BUILD_SHA`, is uploaded even on failure, and must not be interpreted as a
runtime, GUI, authentication or configuration-load result. Offline harness tests
also run on Linux; those tests validate guard logic, not a Windows installation.
All existing native, installed-lifecycle and Linux WebView gates remain in place.
This CI addition does not publish a release or an installation package.

**Boundaries:** successful npm installation and correct Windows file layout do
not prove that Codex starts, reads AgentDeck configuration, or operates offline.
AgentDeck's real install adapter runs a provenance/version probe and therefore
still needs its separate runtime-isolation acceptance. The Windows Codex
configuration UI remains read-only; this package-layout gate does not enable
apply or restore. Unsupported architectures, other versions and alternative npm
layouts are not covered.

Run with official Node and its adjacent npm JavaScript CLI, never `npm.cmd`,
using a fresh evidence path:

```powershell
$node = (Get-Command node.exe -CommandType Application | Select-Object -First 1).Source
$npm = Join-Path (Split-Path -Parent $node) 'node_modules/npm/bin/npm-cli.js'
cargo build --locked -p aisync-cli
$builtParser = Join-Path $PWD 'target/debug/aisync.exe'
$verifierDirectory = Join-Path $env:RUNNER_TEMP ('codex-static-verifier-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $verifierDirectory | Out-Null
$parser = Join-Path $verifierDirectory 'aisync.exe'
[IO.File]::Copy($builtParser, $parser, $false)
if ((Get-Item $builtParser).Length -ne (Get-Item $parser).Length -or (Get-FileHash $builtParser).Hash -cne (Get-FileHash $parser).Hash) { throw 'Trusted parser copy mismatch' }
python -B scripts/codex_official_install_windows.py --node $node --npm $npm --parser $parser --evidence "$env:RUNNER_TEMP/codex-windows-install-result.json"
```

Offline regressions on Linux and Windows:

```sh
python -B scripts/test_codex_official_install_windows.py
```

### 中文

Windows 使用独立的真实官方包安装与布局门禁，不把 Linux 的成功结果直接当作
Windows 支持。验收在一次性的 GitHub-hosted Windows runner 中，使用官方 Node 24/npm，
安装固定 **0.167.0** 的 Codex 根包及 Windows x64 平台包（47 个文件，展开
501,483,647 字节）。这是固定验收样本，不承诺未来始终为最新版。生命周期脚本始终禁用；只检查文件，
不执行 Codex 或生成的启动器，也不通过运行 --version 来验证版本。

门禁位于现有 Windows 原生任务中，必须通过后才能构建 AgentDeck 安装包。独立 JSON
证据 artifact 与精确 BUILD_SHA 关联，失败也上传。新增离线安全回归同时在 Linux
执行，但离线测试通过不代表 Windows 真实安装通过。原有原生测试、已安装应用生命周期
和 Linux WebView 等门禁全部保留；本次增加 CI 不自动发布版本或安装包。

**边界：** npm 安装成功、平台文件布局正确，不等于 Codex 能运行、已读取 AgentDeck
配置、已完成登录或具备离线运行能力。真实 AgentDeck 安装适配器会执行版本和来源探测，
仍须单独完成运行隔离验收。Windows Codex 配置界面仍为只读，本门禁不开放应用或恢复。
其他架构、版本、npm 布局没有因此获得支持声明。


### Production selected-prefix static parsing / 生产静态解析接合

After the original official Windows package gate verifies all 50 package files
and three exact shims, the harness invokes the trusted `aisync codex-static`
command built from the same commit, before removing the temporary installation.
The selected installation path contains Chinese characters and spaces. The
command uses production static parsing for the selected prefix and supplied
Node path; it does not execute npm, Codex, a launcher, or a version probe.
The harness strictly checks its bounded JSON contract and re-runs the complete
unchanged official layout verification afterward. Pins, inventories and shim
hashes are not relaxed.

`production_static_parser_verified` means only selected-prefix layout association.
The observation explicitly says `runtime_not_run` and `selected_prefix_only`;
apply, upgrade and uninstall remain unsupported in this observation.
`agentdeck_adapter_verified` remains false: this is not a real-agent installation
adapter/runtime test, measured version, complete PATH discovery, publisher
attestation, or configuration-loading evidence. Failure remains a blocking gate.

原有官方 Windows 门禁先完整验证 50 个包文件及 3 个精确启动器，随后在清理前
调用同一提交构建的可信 `aisync codex-static`。临时安装路径包含中文和空格；
入口使用生产静态解析逻辑，只观察指定 prefix 与 Node 路径，不执行 npm、Codex、
启动器或版本探测。测试严格核对有界 JSON 返回，并在调用后重新执行完整官方布局
验证，不放宽固定摘要、清单或启动器字节标准。

新增证据仅证明指定目录的静态关联，明确记录“未运行”及“只检查选定 prefix”；
不开放应用、升级、卸载。`agentdeck_adapter_verified` 仍为 false，不把静态结果
称为真实 Agent 适配器运行、实测版本、完整 PATH 检测、发布者认证或配置读取成功。
失败继续阻断，不发布新安装包。

The standalone static command parses only `codex.cmd`. Its two companion
launchers (`codex` and `codex.ps1`) receive regular-file and consistency checks,
not equivalent launcher-content parsing. Exact byte verification of all three
shims belongs to the outer official-package harness. Checks before and after
parsing do not provide an atomic snapshot against concurrent external writers.

独立静态命令只解析 `codex.cmd`；另外两个启动器（`codex`、`codex.ps1`）仅接受
普通文件与一致性检查，不代表同等的启动器内容解析。三个启动器的精确字节验证由
外层官方包验收程序完成。解析前后检查不构成针对外部并发写入的原子快照。


Cargo can hardlink `target/debug/aisync.exe` to its `deps` build output. CI makes
a new, non-overwriting copy in a dedicated verifier directory and checks its
size and SHA-256 against the reviewed build before use. The harness still
requires a regular, single-link, non-reparse executable; it does not exempt
Cargo outputs from that rule.

Cargo 可能将顶层可执行文件硬链接到 `deps` 构建产物。CI 在独立验收目录创建
不覆盖既有文件的新副本，核对大小及 SHA-256 后使用。验收程序仍要求普通、单链接、
非 reparse 的可执行文件，不为 Cargo 产物放宽限制。
