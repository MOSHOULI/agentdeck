# AgentDeck 1.2.0

## 中文

本说明对应 v1.2.0 源码及计划发布的 Windows 10/11 x64 安装包。安装实例与来源证据、受限 npm 安装、目录元数据修正和 DSH 桌面/CLI 被动盘点不在旧 v1.1.1 安装包中。**本文不证明安装包已发布或最终 CI 已通过**：发布状态以对应 Release 为准，构建来源以 `BUILD_COMMIT.txt`、标签提交与该提交的成功 CI 为准。

### 本次变化与操作边界

- **安装实例与来源证据**：分开显示 PATH 候选、规范化路径、可证明的实际入口和当前活动实例的实测版本。PATH 别名不冒充多套安装；歧义、探测失败或检查不完整会阻止配置。PATH 搜索不是整机扫描。npm 全局目录匹配只说明当前 manager、root/prefix、包 name/bin 和入口一致，不证明安装历史或软件发布者身份，也不生成持久化安装凭证。
- **受限 npm 纯安装**：只执行已实现的固定适配器，核验实际 npm/Node、prefix、包与启动器目标，执行前重建计划；目标已有软件、不在 PATH 中的包或启动器、未知或变化的证据都会阻止安装。npm 成功退出后仍须验证唯一活动实例、实测版本及计划对应来源。安装器或安装后核验失败时软件可能已经变化，**不自动卸载或回滚**。不隐式升级、不执行任意注册表 shell 配方，不初始化 profile、vault、Git 备份或应用配置。自动卸载、Agent 软件升级与持久化安装凭证仍未实现；npm 及其生命周期脚本也不是操作系统沙箱。
- **独立配置与 CLI 同步守卫**：配置和个性化同步在准备 profile/备份前要求受支持的适配器及最新、成功、无歧义的安装验证，写入前再次检查。CLI `sync` 先检查全部个性化目标，任一不合格都不开始备份或写入；仅记忆导出不要求安装。来源关联未知本身不会禁用已验证的唯一实例。受管理的 Markdown 区块保留区块外原文。
- **目录元数据核验**：修正 Gemini/Claude npm 的 Node 与认证指引、Pi 的包来源和安装参数，以及 Roo Code、Crush、Oh-My-Pi 的历史或仅来源条目。未经核实的 star 数、推荐标记、使用量和排名描述已撤下，不以空值代表零，也不将稳定显示顺序当作人气排名。Pi/Hermes 的配置写入继续关闭。
- **OpenCode 版本边界**：`opencode-ai` 仅对应 V1 npm 安装途径，不新增 V2 安装或迁移支持。模型与 MCP 写入保持关闭，保留的旧运行时注册表描述也不能重新启用这些适配器。受支持的 `AGENTS.md` Markdown 个性化仍保留；这不等于完成了真实 V1/V2 全流程验收。
- **DSH 被动声明盘点**：新增第六个桌面工作区，并提供独立 `aisync dsh inspect`。只有显式提交绝对路径后，才读取所选 profile 的 `package.json` 和可选 runtime `package.json`，最多两份有界 manifest。路径不持久化，不扫描 HOME，不创建 profile，不启动 DSH/Node/包管理器，不读取凭据、`cordis.yml` 或插件代码，不写配置、安装或启停插件。保存的依赖和 bundle 选择不证明运行、认证或实际启用状态，这些状态始终未知。只审查了 `0.2.2-alpha.3` 的 manifest 子集；已看到的上游 `0.2.2-rc.1` 元数据不构成新增支持。普通浏览器不读取本地文件，显式演示仅使用合成示例。

### 版本与本地注册表

桌面、CLI、核心库及前端版本同步为 **1.2.0**。现有规则将核心包版本作为内置注册表版本下限，因此下限也变为 1.2.0；这不意味着存在同版本的在线签名注册表。更新必须同时高于实际使用版本与 1.2.0 下限，旧版和同版更新均被拒绝。版本边界回归按核心包版本派生下一版本，不再将固定的 1.1.2 当作未来版本。

升级继续只对两份文件都与官方 v1.0.0 签名资产 SHA256 完全相同、且版本标记为 v1.0.0 或不存在的本地注册表，改用新版内置数据。原文件及版本标记保持不动。修改过、来源未知或版本更新的本地注册表继续保留和使用，不以版本升级为由覆盖。签名公钥、外部更新源及原有信任链不变。

### 下载、校验与发布门禁

从 [v1.2.0 发布页](https://github.com/MOSHOULI/agentdeck/releases/tag/v1.2.0) 下载 `AgentDeck_1.2.0_x64-setup.exe`、`SHA256SUMS.txt` 和 `BUILD_COMMIT.txt`。页面不存在则尚未发布。核对 SHA256，并确认构建提交、实际标签提交与成功的发布 CI 完全一致。旧提交的通过结果不能替代最终提交的验证。

**安装包未做 Windows Authenticode 签名。** SHA256 用于字节完整性校验，不等同发布者证书。升级前备份有价值的配置、密钥和记忆，并先在可恢复的测试环境试用；DPAPI 主密钥绑定 Windows 用户/机器，不能仅凭复制 vault.json 或 Git 历史实现跨机器恢复。

发布须在 main 手动触发并开启 `publish_release`，通过该精确提交的前端、Linux 核心/CLI、三档原生 GLib 回归、保守依赖审计、Windows 全工作区及 NSIS 构建门禁。工作流先上传草稿，再重新下载并比较三个资产的字节、SHA256、构建提交与标签提交，全部一致后才公开。已有 Release（包括草稿）或不匹配的标签会阻止发布，不覆盖历史 v1.0.0、v1.1.0 或 v1.1.1。

这是安装包发布，不是注册表更新发布。保留 `--latest=false` 和 v1.0.0 已签名注册表的 Latest 入口；桌面新版请使用明确版本链接。原签名私钥不可用，因此不会发布新签名注册表，也不会生成新密钥替换既有信任公钥。

### 验证与仍未验证的范围

- 常规测试通过 `scripts/run-isolated.py` 使用隔离 HOME、合成 manifest、替身进程/安装回调和模拟 IPC。**8 项手动/真实环境测试继续忽略，不运行 `--ignored` 或 `--include-ignored`。** 本说明不预填最终 CI 的通过数量或结果，以最终提交的运行记录为准。
- **真实 Agent 安装、登录/提供商网络流程、手工桌面 GUI 验收及真实 DSH 执行均未验证。** 临时文件、合成报告和 Windows 自动化测试不能替代这些验证。NSIS 桌面包也不单独安装 `aisync` CLI。
- 保留 v1.1.1 的 GLib 0.18.5 官方两行回移修复、来源检查和三档真实 GLib 回归门禁；不升级该依赖或关闭告警。保守审计仍保留原 GLib 公告及其他上游维护告警，上游编译警告也未清零。完整 Linux 桌面、GTK/WebKit GUI、Linux 分发与非 Windows 受保护密钥库仍未验证或不受支持。
- 本地探测和文件检查不是抵御同权限进程并发修改的操作系统沙箱。配置差异预览、三方冲突对账、自动软件卸载/升级和持久化安装凭证仍未实现。其他数据恢复与密钥保管限制继续适用。

详细边界：[安装证据](INSTALLATION-EVIDENCE.zh-CN.md)、[目录来源](REGISTRY-SOURCES.zh-CN.md)、[DSH 被动盘点](DSH-PASSIVE-INVENTORY.zh-CN.md)、[GLib 修复](GLIB-BACKPORT.zh-CN.md)、[依赖审计](DEPENDENCY-SECURITY.md)、[v1.1.0 数据与恢复限制](RELEASE-1.1.0.md)。

## English

These notes describe the v1.2.0 source and planned Windows 10/11 x64 installer. Installation-instance and source evidence, restricted npm installation, catalog corrections, and desktop/CLI DSH passive inventory are not included in the older v1.1.1 installer. **These notes do not establish publication or a final CI pass.** Check the versioned Release page, and match `BUILD_COMMIT.txt`, the actual tag commit, and successful CI for that exact commit.

### Changes and operating boundaries

- **Installation instances and source evidence:** Separately report PATH candidates, canonical paths, provable entrypoints and the active instance's measured version. PATH aliases are not counted as separate installations; ambiguity, failed probes or incomplete inspection block configuration. PATH search is not a whole-machine scan. An npm global-directory match establishes only current consistency among the manager, root/prefix, package name/bin and entrypoint. It does not prove installation history or publisher identity, and does not create a durable installation receipt.
- **Restricted install-only npm path:** Execute only implemented fixed adapters after inspecting the actual npm/Node, prefix, package and launcher targets, then rebuild the plan before execution. Existing software, off-PATH packages/launchers, unknown evidence or changed evidence block installation. A successful npm exit still requires postchecks of the unique active instance, measured version and source matching the plan. If npm or the postcheck fails, software may already have changed; **there is no automatic uninstall or rollback**. No implicit upgrade, arbitrary registry shell recipe, profile/vault/Git-backup initialization or configuration application occurs. Automatic uninstall, agent software upgrades and durable installation receipts remain unimplemented; npm and its lifecycle scripts are not an OS sandbox.
- **Separate configuration and CLI sync guards:** Configuration and personalization require a supported adapter and fresh, successful, unambiguous installation verification before profile/backup preparation, then recheck before writes. CLI `sync` checks every personalization target first; any ineligible target prevents backup and writes from starting. Memory-only export does not require installation. Unknown package association alone does not disable a verified unique instance. Managed Markdown blocks preserve surrounding text.
- **Catalog source review:** Correct Gemini/Claude npm Node requirements and authentication guidance, Pi's package source/install arguments, and historical or source-only Roo Code, Crush and Oh-My-Pi entries. Unverified star counts, recommendation flags, usage counts and ranking claims are withdrawn. Null is not zero, and stable display ordering is not a popularity ranking. Pi/Hermes configuration writes remain disabled.
- **OpenCode version boundary:** `opencode-ai` remains a V1 npm route, without adding V2 installation or migration support. Model and MCP writes remain disabled even when a retained older runtime registry describes those adapters. The supported `AGENTS.md` Markdown personalization path remains; this does not establish a real V1/V2 end-to-end run.
- **DSH passive declaration inventory:** Add a sixth desktop workspace and separate `aisync dsh inspect` command. Only explicit submission of absolute paths reads the selected profile `package.json` and optional runtime `package.json`, at most two bounded manifests. Paths are not persisted. Inspection does not scan HOME, create a profile, spawn DSH/Node/package managers, read credentials, `cordis.yml` or plugin code, write configuration, or install/activate plugins. Saved dependencies and bundle selections do not establish running, authenticated or actually enabled states; those always remain unknown. Only the `0.2.2-alpha.3` manifest subset was reviewed. Observed upstream `0.2.2-rc.1` metadata does not add rc.1 support. Ordinary browsers cannot read local files; explicit demo mode uses synthetic examples only.

### Versions and local registries

Desktop, CLI, core and frontend versions are synchronized at **1.2.0**. Existing rules use the core package version as the bundled registry floor, which therefore also becomes 1.2.0. This does not imply an online signed registry of that version exists. Updates must exceed both the effective current version and the 1.2.0 floor; older and equal versions are rejected. Boundary regressions derive a future version from the core package version instead of assuming the fixed 1.1.2 is still in the future.

The upgrade continues to select newer bundled data only when both local registry files exactly match the SHA256 fingerprints of the official signed v1.0.0 assets and their version marker is v1.0.0 or absent. Original files and the marker remain untouched. Modified, unknown-source or newer local registries remain preserved and in use; an application version bump does not authorize overwriting them. The signing public key, external feed and existing trust chain are unchanged.

### Download, verification and publication gates

Download `AgentDeck_1.2.0_x64-setup.exe`, `SHA256SUMS.txt` and `BUILD_COMMIT.txt` from the [v1.2.0 release page](https://github.com/MOSHOULI/agentdeck/releases/tag/v1.2.0). A missing page means it has not been published. Verify the checksum and match the build commit, actual tag commit and successful release CI exactly. An older commit's pass does not substitute for final-commit validation.

**The Windows installer is not Authenticode-signed.** SHA256 verifies byte integrity, not publisher certificate trust. Back up valuable configuration, keys and memory before upgrading, and try a recoverable test environment first. DPAPI-protected master keys are bound to the Windows user/machine; copying only vault.json or Git history does not provide cross-machine recovery.

Publication requires manual workflow dispatch on main with `publish_release` enabled, passing frontend, Linux core/CLI, three-profile native GLib, conservative dependency-audit, full Windows workspace and NSIS build gates for that exact commit. The workflow uploads a draft, downloads all three assets again, and verifies their bytes, SHA256, build commit and tag commit before publication. Existing releases, including drafts, or mismatched tags block publication. Historical v1.0.0, v1.1.0 and v1.1.1 releases are not overwritten.

This is an installer release, not a registry update. `--latest=false` preserves the signed v1.0.0 registry Latest endpoint; use the explicit version link for the desktop installer. The original signing private key is unavailable, so no new signed registry is published and no replacement key is generated to substitute for the established trust key.

### Validation and remaining unverified scope

- Ordinary tests use `scripts/run-isolated.py`, isolated HOME directories, synthetic manifests, fake processes/installer callbacks and mocked IPC. **The eight manual/real-environment tests remain ignored. Do not run `--ignored` or `--include-ignored`.** These notes do not prefill final CI counts or outcomes; the final commit's run is authoritative.
- **Real agent installation, login/provider network flows, manual desktop GUI acceptance and real DSH execution remain unverified.** Temporary files, synthetic reports and Windows automation do not replace those checks. The NSIS desktop package does not separately install the `aisync` CLI.
- Retain the v1.1.1 official two-line GLib 0.18.5 backport, provenance checks and three-profile real GLib regression gates, without upgrading that dependency or suppressing advisories. The conservative audit retains the original GLib advisory and other upstream maintenance warnings; upstream compiler warnings remain. Full Linux desktop, GTK/WebKit GUI, Linux distribution and protected non-Windows vault support remain unverified or unsupported.
- Local probes and filesystem checks are not an OS sandbox against concurrent changes by same-user processes. Configuration diff previews, three-way reconciliation, automatic agent uninstall/upgrade and durable installation receipts remain unimplemented. Existing recovery and key-safeguarding limitations still apply.

Detailed boundaries: [installation evidence](INSTALLATION-EVIDENCE.md), [catalog sources](REGISTRY-SOURCES.md), [DSH passive inventory](DSH-PASSIVE-INVENTORY.md), [GLib backport](GLIB-BACKPORT.md), [dependency audit](DEPENDENCY-SECURITY.md), and [v1.1.0 data and recovery limits](RELEASE-1.1.0.md).
