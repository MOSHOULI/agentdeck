# Reviewed npm installation / npm 安装计划确认

Development source only; not yet shipped in a public installer.

## What the desktop confirms

The Install action first requests a fresh backend plan. The confirmation shows
exactly the supported package, canonical npm entrypoint, **installer** Node
executable, global prefix/root, package destination and launcher directory.
It also shows the exact Codex root-package version and the package-version and lifecycle-script policies. Only a separate
confirmation submits the plan revision to the real install adapter.

This does not add an installation channel. The existing adapters are Codex,
Gemini CLI, OpenCode's `opencode-ai` **V1** route, and Pi. Claude Code's deprecated
npm channel remains disabled, even if a retained registry describes it. Manual
entries, Qwen, Qoder and DSH do not become installable through this interface.

The preview measures the frozen native installer Node version and performs bounded local npm prefix/root queries and
filesystem inspection. For Codex it also queries the configured registry through
the frozen npm entrypoint for one exact stable latest root-package version. It
does not run an installer, download a package archive, prepare
a profile/vault/Git backup, or change agent configuration. npm itself is local
code: it may consult configuration and touch caches/logs. A preview is not a
zero-side-effect sandbox, publisher verification, or an installation guarantee.

## Confirmation and changed targets

`preview_agent_install(id)` returns a fixed camelCase schema-version-1 projection.
`install_agent(id, revision)` requires a 64-character lowercase SHA256 revision;
there is no desktop fallback that omits it. The backend does not accept caller
paths, package names, commands or program hashes as a replacement plan.

The revision binds the agent, displayed paths/package/policies, actual command
and arguments, platform/execution mode, and inspected Node/npm-entrypoint bytes.
Before installing, the backend independently rebuilds the plan under the shared
in-process installer lock and compares its revision. It then retains the existing
second preflight and occupied-target checks. Codex latest-version resolution is
repeated at both checks; a changed result requires a fresh review. A stale revision, changed manager,
redirected prefix, newly occupied package/launcher, or incomplete inspection
cannot reach the installer callback. Paths that cannot be represented losslessly,
or contain ambiguous controls, are rejected rather than displayed differently.

A revision is a change fingerprint, not proof of a human click, a reusable
permission grant, or a durable installation-ownership receipt. It is not signed
and does not exclude another process or a same-user filesystem race. The
interpreter shown is the one running npm; Windows launchers can later select a
different Node, so the preview does not claim to freeze every future runtime.

Cancelling, leaving/filtering away the card, or refreshing invalidates the displayed
confirmation. Discarding a pending preview does not cancel its underlying queries;
the global busy state lasts until settlement. Repeated clicks do not start a second
request. Confirmation is consumed when installation starts; uncertain failures are
not retried automatically. A completed installation and a failed subsequent list
refresh remain separate outcomes.

## What remains unresolved

For Codex, the frozen npm entrypoint runs direct argv
`view @openai/codex@latest version --json --fetch-retries=0 --fetch-timeout=25000`
with a 30-second process/descendant bound and a 128-byte output limit. Local
Node/prefix/root probes retain their three-second bound. npm fetch retries are
disabled and its per-fetch timeout is 25 seconds. Only one stable `MAJOR.MINOR.PATCH` JSON string is accepted (no ranges,
tags, prereleases, URLs, arrays or extra records). The nullable `packageVersion`
field is non-null and `versionPolicy` is `exact_root`; both are revision-bound.
The installer requests `@openai/codex@<reviewed-version>`, never bare latest.
Even if latest moves after the final preflight, that argv remains exact.
Metadata lookup failure or drift fails closed before package mutation; preview
can therefore require network access and may touch npm caches/logs.

Other adapters retain `packageVersion: null` and `versionPolicy: npm_configured`.
Registry identity, npm configuration, tarball integrity, publisher identity and
the complete dependency closure are **not pinned or fully fingerprinted**.
An exact version is not an immutable-content guarantee, even for Codex.
No dependency download or source-signature verification happens during preview.
Review these limitations before confirming.

For Codex and Pi, `--ignore-scripts` is mandatory. For other supported adapters, lifecycle
behavior follows npm configuration and may execute installation scripts. This is
not a general sandbox or an assertion that a package has no scripts. npm can
contact its configured registry and write dependencies, launchers, caches and logs.

Installation remains software-only, with a five-minute subprocess bound and
captured-output limits. Successful npm exit is not enough: the observed active
instance, measured version and npm directory association must match the plan.
Codex also requires the bounded, parsed root manifest name/version to match the
reviewed package and exact version. A CLI version is not substituted for that
manifest check; both independent checks remain required.
Failure can leave partial software changes. No automatic rollback, uninstall,
upgrade, authentication or profile/model configuration is added.

The CLI's existing `deploy --action install --yes` keeps its explicit-command
semantics and the same installer guards. The reviewed revision is mandatory for
the desktop IPC path; this increment does not silently change the CLI contract.

## Installer Node prerequisite

The review includes the measured frozen installer Node version and the fixed
AgentDeck minimum policy. Both are bound into the revision and rechecked before
package mutation. This is not a package engine/dependency or future runtime
compatibility result. See [the policy and evidence boundaries](INSTALLER-NODE-POLICY.md).

## Verification boundary

Core tests use synthetic manager files and injected subprocess results. They
check exact successful confirmation, stale Node/npm bytes and destinations,
occupied targets, invalid revisions, script policy, path projection and existing
post-install checks. Frontend tests cover strict response projection, displayed
facts, cancellation, stale responses, repeated actions, unmount and remount.

The Linux WebView acceptance uses the real application, DOM controls, IPC and
subprocess runner with an **authored synthetic npm/Codex fixture**. Its manager
handles only fixture prefix/root and exact-version metadata queries and the exact expected install command;
it creates a synthetic version-only executable and never downloads a package.
Acceptance checks cancellation, latest-version drift rejection with unchanged manager bytes, one confirmed install,
and observed source/version. This does not establish real registry availability,
package lifecycle behavior, login, model execution or real-agent compatibility.

The existing Windows native/installed-app and Linux WebView gates remain required.
No installer is published by this change; publication retains its separate gate.

## 中文

开发源码新增“先检查安装计划，再明确确认”的桌面流程。确认中展示真实检查得到的 npm
入口、安装器 Node、包名、全局 prefix/root、包目录和启动器目录，以及版本和脚本策略。
只复用现有 Codex、Gemini CLI、OpenCode V1 和 Pi 适配器，不重新开放 Claude 已弃用的
npm 安装，也不将 Qwen、Qoder、DSH 或手动条目升级为自动安装能力。

确认时仅提交 agent ID 和计划 revision。后端重新检查，不信任界面传入的路径、命令或
摘要；目标、Node/npm 程序字节改变，或目标已被占用时，旧确认不能触发安装。后续原有
二次预检、安装超时和安装后版本/目录关联核验仍保留。取消或离开仅丢弃待显示结果，
底层检查完成前保持忙碌；不重复提交、不自动重试不确定结果。

预览不执行安装、不准备 profile/密钥库/备份，也不改 agent 配置，但会运行本地 npm
路径查询，Codex 还会查询配置 registry 的版本元数据，因此不承诺 npm 无缓存/日志副作用。revision 不是人类点击证明、安装所有权
记录或安全沙箱。Codex 通过 npm 查询并固定预览中的根包精确版本，确认和二次预检
发现 latest 变化即拒绝，安装后另核对根 manifest 版本；其他适配器仍由 npm 选版本。
完整 npm 配置、registry、归档完整性和依赖闭包未固定。Windows
未来启动器选用的 Node 可能不同于当前安装器 Node。

Codex 和 Pi 强制禁用安装脚本；其他现有适配器遵循 npm 配置，可能下载依赖并执行安装脚本。
安装成功还需通过现有入口、版本和来源目录核验；失败可能已有局部改动，不自动回滚、
卸载或升级，也不登录、不写模型配置。CLI 原有显式 `--yes` 流程不变。

测试包括合成 core/前端回归，以及真实 Linux WebView 经原生 IPC 调用“自建合成 npm”
并安装“仅输出合成版本的 Codex 替身”。该验收验证管理流程，不代表安装过真实 Codex、
验证过 npm 网络/依赖脚本、登录或模型运行。仍需完整 CI；本次不发布单独安装包。

### Read-only empty vault / 空密钥库只读检查

Startup also loads the vault list. Reading a missing vault now returns an empty
result without creating its directory, lock or key material. Existing vaults
retain their locking behavior. This does not provide an adversarial concurrent
filesystem sandbox.

启动时还会读取密钥列表。缺失的密钥库现在返回空结果，不创建目录、锁或主密钥；
已有密钥库继续使用原文件锁。这不是防御恶意并发文件系统替换的沙箱。

### Codex lifecycle override / Codex 生命周期脚本强制策略

Codex's supported bundled and retained `npm install -g @openai/codex` recipe
remains eligible; it is not executed as a shell string. The internal adapter adds
`--ignore-scripts` unconditionally, and the preview reports `disabled` from the
same policy helper. Pi keeps its existing policy; Gemini CLI and OpenCode V1
remain npm-configured. A stale Codex preview from before this change is rejected.
Codex now resolves and pins the reviewed stable latest root version, not a hardcoded 0.167.0.

The separate [official-package gate](CODEX-OFFICIAL-INSTALL.md) verified the
pinned 0.167.0 Linux x64 npm filesystem layout without lifecycle scripts. That
narrow result does not establish compatibility of every future version, other
platforms, the real AgentDeck adapter's version postcheck, login or model runtime.
The application/WebView tests still use authored synthetic manager/agent fixtures.

Codex 的现有及保留配方 `npm install -g @openai/codex` 仍可匹配适配器；
后端不通过 shell 执行配方，而是强制加入 `--ignore-scripts`，预览与执行共用策略。
Pi 策略不变；Gemini CLI、OpenCode V1 仍遵循 npm 配置。更新前的 Codex 预览不能
用于新策略安装。Codex 根包版本现按预览解析结果精确固定，并非硬编码为 0.167.0。
独立官方包门禁仅验证了固定 0.167.0 Linux x64 包在不运行生命周期脚本时的 npm
文件安装布局，不代表未来版本、其他平台、真实适配器版本后检、登录或模型运行均已验收。
应用和 WebView 安装流程测试仍使用自建合成替身。

## Windows installed-app synthetic acceptance / Windows 已安装应用合成验收

The Windows installed-lifecycle lane additionally exercises the reviewed install
adapter through the real installed WebView and native UIAutomation. A copied,
trusted official Node executable runs **authored test npm and Codex JavaScript**.
The fixture is not the official npm implementation or Codex package. Its npm
substitute only accepts bounded prefix/root and exact-version metadata queries and the exact reviewed
installation arguments, including `--ignore-scripts`; its Codex substitute only
answers the expected version probe. Fixture generation and validation remain
outside the application; preview, cancellation and confirmation go through the
actual UI/IPC/adapter, never a direct backend call or fabricated Installed state.

Required stages are preview/cancel with no install, mutation of the fixture latest
version with unchanged manager bytes before confirmation and stale-plan rejection, a fresh preview and one
confirmed install, observed version/source association, and unchanged agent
configuration. Native accessibility activation is single-shot; uncertain writes
are not retried. The authored process counters and complete stage list are
recorded in the installed-lifecycle evidence, alongside the existing Qwen,
Codex read-only and Qoder gates. A separate fail fixture records **every** install
invocation before checking its exclusive marker, writes bounded partial package
state, and exits unsuccessfully. The gate checks the production failure warning,
consumed confirmation, no success claim, zero version probes, and unchanged
partial bytes after a bounded wait, explicit list refresh and application restart.
A subsequent preview remains rejected while durable protection is armed; independent
absent-only destination checks also refuse an occupied partial package directory;
no confirmation or additional installation is allowed in that observed state.
These checks prove no automatic retry or rollback in those observed flows; they
do not implement repair, test a manually confirmed retry, or establish arbitrary
installer-failure recovery. The test never deletes partial state to make retry pass.
No durable installation ownership receipt is created. A real failure still needs
inspection before any separately reviewed retry or manual recovery.

This proves the tested synthetic Windows adapter path, **not** official-agent
execution, registry compatibility, model calls, lifecycle scripts, real Codex
configuration loading or runtime network isolation. The separate official-package
layout gate retains its own evidence and never executes Codex. Neither result
substitutes for the other, and this test does not publish an installer.

Windows 已安装应用门禁通过真实 WebView 和原生 UIAutomation 操作安装计划。
官方 Node 执行的是**测试编写的 npm 与 Codex JavaScript 替身**，不是官方 Agent。
合成 npm 只接受路径/精确版本元数据查询和包含 `--ignore-scripts` 的精确安装参数，合成 Codex
只允许预定版本探测。只有 fixture 的创建与核验由测试程序完成；预览、取消和
确认必须经过真实界面、IPC 与适配器，不直接调用后端或伪造“已安装”。

必须验证取消零安装、预览后 latest 版本变化且管理器不变导致过期拒绝、重新预览后单次确认安装、
版本及来源关联展示，以及配置未变化。每次原生激活只发送一次，不自动重试不确定写入。
证据记录合成进程计数和完整阶段，原有 Qwen、Codex 只读及 Qoder 门禁全部保留。
独立失败替身在检查独占标记之前记录每次安装调用，写入有限部分文件后返回失败。
门禁验证生产错误提示、确认已消耗、无成功提示、零版本探测，并在有限等待、应用重启、
手动刷新列表后检查残留字节不变，证明这些被观察流程没有自动重试或回滚。
重启后持久保护继续拒绝安装预览，不出现安装确认，也不新增安装调用；原有目标占用检查并未移除。
这没有实现修复，也没有验收手动确认重试或任意安装器失败恢复；测试不删除残留以绕过失败。
产品未生成持久安装所有权凭据；真实失败仍须先检查，再独立审核重试或手动恢复。

这证明合成输入下的 Windows 适配器闭环，不代表官方 Agent 已运行、registry 或
生命周期脚本兼容、真实 Codex 已读取配置、模型调用成功或运行网络隔离。官方包
文件布局验收仍单独保留且不执行 Codex，两类证据不能相互替代；不自动发布安装包。

## Unconfirmed-request review / 未确认请求的人工核对

After a confirmed install request fails, the desktop UI retains the reviewed
installation targets for that Agent **in this application session only**. This is
historical preview information, not a fresh filesystem scan, durable receipt,
proof of ownership, or authorization for another operation. The frontend string failure
cannot distinguish an internal preflight rejection from a failed installer or
incomplete postcheck; the UI therefore says only that completion is unconfirmed,
not that software was necessarily executed or written. Raw failure output is not
shown in this panel or the install error notice.

Use the recorded package, manager, Node and target directories to compare with
current local state and the official installation instructions. Preserve unknown
files and credentials. Do not infer that a directory can be deleted merely
because it appeared in the reviewed plan. AgentDeck does not repair, roll back,
uninstall or repeat the request from this panel. Where a later attempt is allowed,
it still requires a fresh valid preview and separate confirmation, and occupied
targets remain rejected. The Codex persistent suppression described below
refuses further previews/install requests after attempted-dispatch failure;
a fresh preview cannot clear that guard. Refreshing detection does not turn this historical record into an
ownership claim. Restarting the application loses this session-only record.

已确认的安装请求未确认完成时，桌面界面在**本次应用会话内**保留对应 Agent
的已审核安装目标。这是历史预览信息，不是当前文件扫描、持久凭据、所有权证明，
也不是再次操作的授权。前端收到的字符串错误不能可靠区分内部预检拒绝、安装器失败和
后验不完整，所以界面只说明结果未确认，不推断一定执行或写入了软件。
面板与安装错误提示不展示原始失败输出。

请将记录的包、管理器、Node 和目标目录与当前本地状态及官方安装说明核对。
保留不明文件和凭据，不因目录出现在预览中就认定可以删除。此面板不会修复、
回滚、卸载或重复请求。在允许后续尝试的情况下，仍须重新取得有效预览并单独确认；
目标占用仍会拒绝。下述 Codex 持久门禁在已尝试调用安装器且失败后拒绝新的预览
与安装请求，重新预览不能解除门禁。刷新检测不会让历史记录变成所有权证明；重启应用后会话记录不保留。

### Persistent Codex installation protection and explicit recovery

The early and late exact root-manifest checks remain. The desktop additionally
holds both the existing process-local gate and an exclusive OS-backed lease
across each Codex list probe, installation preview, and complete installation
attempt/postcheck. Normal simultaneous app instances cannot independently clear
or ignore another lease. Contention, poison, unreadable state and failed path or
identity/security checks mean suppression, not presumed absence or success.
The final list projection rechecks durable state and removes runtime/source/auth
evidence if protection is no longer clear. This is not an atomic UI snapshot
against a transition after the final lease is released.

**New first-use side effect:** the normal Codex list/preview/install path initializes
private `.agentdeck/install-protection/codex/gate.lock` and `state.json` under the
application home. The record explicitly says `clear`, `armed`, or `failed_returned`.
The fixed coordination lock is never removed during transitions. Existing invalid
or partially missing state is not repaired or treated as clear. This metadata is
not a package ownership receipt, credential vault, profile or backup. Merely
opening the recovery preview uses existing-state-only access and creates nothing.
This is not a migration of historical installation failures. First-use `clear`
does not certify existing packages or recover failure information that older
versions kept only in memory; only attempts recorded by this store are retained.

After both preflights, a fallible core gate receives the authoritative rebuilt
preview. The durable store records the attempt and reviewed target before the
installer can dispatch; a failed write/flush/validation prevents dispatch. A
normally returned preflight failure before this point leaves the clear state
unchanged, preserving stale-preview → fresh-preview behavior. Successful complete
installation writes clear only for the matching attempt. Normal failed returns,
after the bounded runner cleanup path has returned, record `failed_returned`.
Panic or crash leaves `armed`; restarting does not clear that state. A crash may
leave an installer descendant alive on platforms without parent-death containment,
so **armed/unknown attempts are never eligible for in-app recovery**, even when
slots currently look empty. This increment does not implement process repair.

While blocked, refresh/navigation/restart returns an unknown/error Codex row
without active npm or Agent probing for that row. No residual file is removed,
normalized, retried or rolled back. The existing Windows session-only residual
inspection remains separate; losing its UI history does not remove durable protection.

The separate “检查恢复探测条件” action reads trusted existing state, checks the
recorded prefix identity, every fixed installation slot, and the complete current
Codex PATH candidate set without running npm or an Agent. Unix checks the package
directory and `bin/codex`; Windows also checks all supported launcher suffixes,
including `.exe`, `.com`, and `.bat`, not only npm's three companions. Recovery is
eligible only for a normally returned failure with unchanged trusted state/target
and all affected slots and PATH candidates absent. Correct-looking manifests or
version strings never authorize executing a remaining package.

A fresh recovery revision binds those observations. “确认恢复 Codex 探测” rechecks
under the lease and only then writes clear for that exact state. Cancel, stale
revision, changed identity, occupied slots, incomplete scan or damaged record
cannot clear it. The UI consumes confirmation before dispatch, never automatically
retries uncertainty, and refreshes the list only after confirmed success. This
restores permission for ordinary probing; it does not install or delete software,
authenticate packages, prove runtime behavior, or automatically retry installation.
Any later install requires its own fresh preview and explicit confirmation.

An error during final clear-state replacement does not prove the old bytes remain:
the clear record may have been committed before a later flush or identity check
failed. The current process stays conservative; a later fresh open validates the
actual on-disk state and may find clear. The UI therefore reports an unconfirmed
protection state, never “unchanged” or automatic retry. Linux synchronizes file
content and the containing directory; Windows uses its supported file flushing
and replacement path without a portable directory-sync guarantee. Neither path
claims universal power-loss durability.

An already-running instance may retain its conservative in-memory suppression
after another instance completes explicit recovery. This batch does not silently
reconcile that memory gate. Only when read-only recovery inspection reports a
validated durable `clear` record does the UI explain that reopening can read the
already-cleared state. This is not a restart-based reset: `armed`, corrupt or
unreadable durable records remain blocked after restart.

These guards target the desktop's Codex row on supported Linux/Windows local
filesystems. Other desktop platforms explicitly refuse Codex active inspection
and installation rather than falling back to unprotected operation; macOS native
acceptance is not claimed. CLI/core callers retain their existing behavior and do
not acquire this desktop store. Custom registry aliases, other Agent rows and hostile
same-user filesystem changes remain outside the guarantee. Other rows can perform
their normal probes. State/lock deletion by an external actor, hardware/storage
faults and arbitrary malicious races are not a tamper-proof or universal crash-
durability guarantee. No #22 isolation boundary or installer publication gate changes.

Storage tests kill an authored independent test process after arming and reopen
the same record. Native desktop acceptance separately exercises normal exit and
restart in the same home, plus explicitly prepared armed/corrupt fixture states;
those prepared states are not evidence of an actual desktop crash. The positive
recovery fixture deliberately moves only its own authored package/launcher slots
aside while retaining the prefix identity, then uses the real review/confirmation
flow. The product never performs that fixture cleanup or runs an official Agent.

桌面首次正常检查 Codex 会创建固定的私有保护状态与协调锁；这不是零写入列表读取。
不会追溯恢复旧版本只存在内存中的失败记录；首次 clear 不代表既有软件已经验证。
安装器调用前必须完成持久 armed 记录，失败写入不得启动安装器。正常返回的失败记为
failed_returned；崩溃或未知 armed 状态跨重启继续阻止该条目的 npm/Agent 探测，且不能
仅凭槽为空在应用内解除，避免仍存活的安装器随后继续写入。明确的调用前预检拒绝
仍允许重新预览。成功只释放匹配的本次尝试，不清理任何软件或配置。

恢复是独立的只读检查与明确确认：必须记录可信、路径身份一致、所有固定安装槽及
当前 PATH 候选均不存在。占用、损坏、无法访问、重定向或检查不完整一律保留保护。
确认后仅允许恢复正常探测，不安装、不删除、不证明包真实性或运行安全；取消或
过期确认不释放保护。重启不是清除或修复手段。普通多实例通过固定锁协调，但不声称
抵抗恶意同用户/跨进程篡改，也不扩展到其他条目、别名或直接 CLI/core 调用。
