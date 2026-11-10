# Reviewed npm installation / npm 安装计划确认

Development source only; not yet shipped in a public installer.

## What the desktop confirms

The Install action first requests a fresh backend plan. The confirmation shows
exactly the supported package, canonical npm entrypoint, **installer** Node
executable, global prefix/root, package destination and launcher directory.
It also shows the package-version and lifecycle-script policies. Only a separate
confirmation submits the plan revision to the real install adapter.

This does not add an installation channel. The existing adapters are Codex,
Gemini CLI, OpenCode's `opencode-ai` **V1** route, and Pi. Claude Code's deprecated
npm channel remains disabled, even if a retained registry describes it. Manual
entries, Qwen, Qoder and DSH do not become installable through this interface.

The preview performs the existing bounded local npm prefix/root queries and
filesystem inspection. It does not run an installer, download a package, prepare
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
second preflight and occupied-target checks. A stale revision, changed manager,
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

The command still requests the supported package without an exact version. npm's
current configuration chooses the version/registry/dependency closure; those are
**not pinned or fully fingerprinted** by this plan. They can change independently
of the displayed target. No dependency download or source-signature verification
happens during preview. Review this limitation before confirming.

For Pi, `--ignore-scripts` is mandatory. For other supported adapters, lifecycle
behavior follows npm configuration and may execute installation scripts. This is
not a general sandbox or an assertion that a package has no scripts. npm can
contact its configured registry and write dependencies, launchers, caches and logs.

Installation remains software-only, with a five-minute subprocess bound and
captured-output limits. Successful npm exit is not enough: the observed active
instance, measured version and npm directory association must match the plan.
Failure can leave partial software changes. No automatic rollback, uninstall,
upgrade, authentication or profile/model configuration is added.

The CLI's existing `deploy --action install --yes` keeps its explicit-command
semantics and the same installer guards. The reviewed revision is mandatory for
the desktop IPC path; this increment does not silently change the CLI contract.

## Verification boundary

Core tests use synthetic manager files and injected subprocess results. They
check exact successful confirmation, stale Node/npm bytes and destinations,
occupied targets, invalid revisions, script policy, path projection and existing
post-install checks. Frontend tests cover strict response projection, displayed
facts, cancellation, stale responses, repeated actions, unmount and remount.

The Linux WebView acceptance uses the real application, DOM controls, IPC and
subprocess runner with an **authored synthetic npm/Codex fixture**. Its manager
handles only fixture prefix/root queries and the exact expected install command;
it creates a synthetic version-only executable and never downloads a package.
Acceptance checks cancellation, changed-manager rejection, one confirmed install,
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
路径查询，因此不承诺 npm 无缓存/日志副作用。revision 不是人类点击证明、安装所有权
记录或安全沙箱，也未固定完整 npm 配置、registry、实际包版本和依赖闭包。Windows
未来启动器选用的 Node 可能不同于当前安装器 Node。

Pi 强制禁用安装脚本；其他现有适配器遵循 npm 配置，可能下载依赖并执行安装脚本。
安装成功还需通过现有入口、版本和来源目录核验；失败可能已有局部改动，不自动回滚、
卸载或升级，也不登录、不写模型配置。CLI 原有显式 `--yes` 流程不变。

测试包括合成 core/前端回归，以及真实 Linux WebView 经原生 IPC 调用“自建合成 npm”
并安装“仅输出合成版本的 Codex 替身”。该验收验证管理流程，不代表安装过真实 Codex、
验证过 npm 网络/依赖脚本、登录或模型运行。仍需完整 CI；本次不发布单独安装包。
