# AgentDeck 1.2.0

## 中文

本次是自 v1.1.1 以来的累计功能与安全更新，目标安装包为 **Windows 10/11 x64**。用户已于 2026-11-12 要求发布本批并暂停后续开发。本文是发布说明，不是发布成功证明：以 [v1.2.0 Release](https://github.com/MOSHOULI/agentdeck/releases/tag/v1.2.0)、`BUILD_COMMIT.txt`、标签及该精确提交的发布 CI 为准。页面不存在则尚未公开。

### 主要变化

- **安装前确认与失败保护**：展示实际 npm/Node、目标 prefix、包及启动器计划，确认后重新预检；验证 Node 版本要求。Codex 根包锁定预览时的精确版本，`latest` 漂移会要求重新预览；安装后先核对根 manifest，再允许版本探测。失败保留残留，不自动回滚、删除、升级或重装。桌面 Codex 的持久状态与锁在重启后继续阻止不安全探测；可信的已返回失败状态仅在独立只读恢复检查和明确确认后解除。中断/未知、损坏、路径冲突和残留仍阻止恢复。正常桌面 Codex 列表首次访问会创建持久锁/状态；不会追溯旧版仅保存在内存的失败，初始 clear 不证明现有包安全。此保护不是任意外部进程或所有 CLI 的通用沙箱。
- **配置与恢复**：MCP 变更摘要和同步 dry-run、Codex 只读配置计划；Qwen 0.27.0/0.27.2 的实验性 `general.vimMode` 单字段预览、应用、恢复和只读恢复检查；Linux x86_64 的 Codex `tui.animations` 受管字段操作。受管字段保留无关值，拒绝陈旧确认与冲突；不是任意配置迁移或完整三方对账。通用写入拒绝链接/多硬链接目标，Linux 拒绝无法保留的可见扩展元数据，Windows 在支持范围保留 DACL、属主和组；不匹配继承策略安全拒绝，非完整安全描述符或无竞态保证。
- **Qoder 分发管理**：显式选择固定审核版本 1.1.85、1.1.86、1.1.87，默认仍为 1.1.85，不追随 rolling。提供来源/作用域观察、归档校验、受控缓存/解包及 **Linux x64 baseline** 私有文件部署和重新检查。不同版本不混用归档、记录或确认。部署重验重新检查字节、记录及权限，不依赖缓存。不会运行 Qoder、登录、激活 PATH、自动升级、修复或卸载。Windows 界面校验 Linux 归档不代表 Windows Qoder 安装支持；Windows ARM64 来源矛盾仍未消除。
- **DSH 静态证据**：显式 profile/runtime 声明盘点、根包与部分 subprocess/native 依赖归档的有界静态核验，以及桌面入口。支持范围按固定来源快照限制，不扫描凭据、不执行 DSH/生命周期脚本/原生代码。不是完整依赖闭包、真实安装、插件启停或运行验证。
- **目录与日常操作**：主流工具目录的官方手动指引、能力筛选和未知状态更明确；目录条目不等于完整管理。记忆导出选择支持预览、确认与跨搜索保留，取消不写入；本地 Markdown 不代表外部 Agent 已加载。Pi/Hermes 配置写入、OpenCode 模型/MCP 写入仍关闭；`opencode-ai` 仍仅 V1 路径。

### 验证证据与边界

发布前的功能基线为 `4dbe8ed7d69e8529398d5f4b383abcaa72131fa7`，[主线 CI 43237572060](https://github.com/MOSHOULI/agentdeck/actions/runs/43237572060) 的八项检查通过。发布说明修改后的最终提交仍须完整重新验证；旧绿灯不替代发布提交。

- Windows 一次性 runner 验证实际 NSIS 安装、启动真实 WebView/IPC、重启、同版重装、从 v1.1.1 升级和卸载，保留合成数据；覆盖受控配置和安装失败/恢复界面流程。
- Linux 原生 WebView 验证三版 Qoder 官方归档/私有部署、只读重验、混版拒绝及相关受控流程。官方载荷只做静态检查；不执行 Qoder。Codex 官方 npm 包在 Linux/Windows 的安装布局与生产解析有非执行证据；完整安装器/探测流程使用自编替身，不是官方 Codex 完整运行验收。
- Qwen 固定分发/源码静态证据与合成配置 GUI 验证已覆盖；**真实 Qwen 设置加载仍受 #22 网络隔离门禁阻塞**。未绕过隔离，也不将其并入发布。
- 未验证真实 Agent 登录、模型/在线服务、真实 DSH 执行、所有用户配置迁移、真实断电或安装子进程骤停恢复。持久状态的合成 armed/corrupt 和应用正常关闭重启不等于真实崩溃恢复。
- Linux GUI 的特定流程成功不意味着完整 Linux/macOS 产品支持；本次不发布这些平台安装包或非 Windows 受保护密钥库。NSIS 包不单独安装 `aisync` CLI。
- GLib 官方两行回移、来源和三档原生回归保留；保守依赖审计及上游维护警告未被隐藏。手动/真实环境忽略用例与有意跳过项须从精确 CI 查看，不能按旧文档固定数量推定全部测试完成。

### 下载、数据与发布安全

下载 `AgentDeck_1.2.0_x64-setup.exe`、`SHA256SUMS.txt` 和 `BUILD_COMMIT.txt`，比对 SHA256、实际 tag 与构建提交。工作流先创建草稿，重新下载并逐字节比较三项资产，核对源码标签后才公开；不覆盖现有 Release 或不同提交标签。发布后请使用明确版本链接。

**安装包未做 Windows Authenticode 签名。** 校验和只证明完整性，不等同发布者证书。升级前备份有价值的配置、密钥和记忆；DPAPI 主密钥绑定 Windows 用户/机器，不能只复制 vault.json 或 Git 历史实现跨机器恢复。本地记忆与配置恢复副本可能含明文；不要分享整个数据目录。多文件回滚不是断电日志。

此次仅发布安装包，保留 `--latest=false` 与原 v1.0.0 签名注册表 Latest 入口。不生成替代签名密钥，不改变信任公钥。内置注册表版本下限为 1.2.0，自定义/未知/更新的本地注册表保留；仅精确匹配原 v1.0.0 官方资产的旧注册表改用内置数据，原文件不删除。

详见 [安装与持久恢复](INSTALL-PREVIEW.md)、[Windows 验收](WINDOWS-ACCEPTANCE.md)、[Linux 验收](LINUX-WEBVIEW-ACCEPTANCE.md)、[Qoder 部署](QODER-PRIVATE-DEPLOYMENT.md)、[Qwen 字段](QWEN-OWNED-SETTING.md)、[Windows 配置权限](WINDOWS-CONFIGURATION-DACL.md)、[依赖审计](DEPENDENCY-SECURITY.md)。

## English

This cumulative feature and safety update since v1.1.1 targets **Windows 10/11 x64**. On 2026-11-12 the owner requested publication of this batch and a pause in further development. These notes do not prove publication: check the [v1.2.0 Release](https://github.com/MOSHOULI/agentdeck/releases/tag/v1.2.0), `BUILD_COMMIT.txt`, actual tag and exact-commit release CI. A missing page means it is not public yet.

### Changes

- **Reviewed installation and failure protection:** Preview the actual npm/Node, prefix, package and launcher plan; recheck it after confirmation, including Node requirements. Codex pins the exact root version shown in the preview; changed `latest` requires a new preview. Root-manifest validation precedes post-install execution probes. Failures retain residual files without automatic rollback, removal, upgrades or reinstall. Desktop Codex persistent state and locking survive app restart; a trusted returned-failure state can be cleared only after a separate read-only recovery check and explicit confirmation. Interrupted/unknown, corrupt, conflicting or occupied states remain blocked. Normal desktop Codex list access initializes persistent lock/state on first use. This does not reconstruct old in-memory failures, and an initial clear state does not prove existing packages safe. This is not a general OS sandbox for external processes or every CLI.
- **Configuration and recovery:** MCP action summaries and sync dry-run, read-only Codex plans, experimental Qwen 0.27.0/0.27.2 `general.vimMode` preview/apply/restore and read-only recovery, plus Linux x86_64 Codex `tui.animations` ownership. Owned fields preserve unrelated values and reject stale confirmation/conflicts, not arbitrary migrations or full three-way reconciliation. General writes reject linked/multi-hardlinked targets; Linux refuses unpreserved visible extended metadata, while Windows preserves DACL, owner and group within documented limits and rejects incompatible inheritance. This is not full security-descriptor preservation or race freedom.
- **Qoder distribution management:** Explicit reviewed versions 1.1.85, 1.1.86 and 1.1.87; default remains 1.1.85, not rolling. Source/scope observation, archive verification, controlled cache/unpack, **Linux x64 baseline** private file deployment and fresh deployment inspection do not mix versions, records or confirmations. Reinspection checks actual bytes, records and permissions, not cached success. No Qoder execution, login, PATH activation, automatic upgrade, repair or uninstall. Hashing a Linux archive on Windows does not establish Windows Qoder installation support; conflicting Windows ARM64 source information remains unresolved.
- **DSH static evidence:** Explicit profile/runtime declaration inventory and bounded static verification of the root and selected subprocess/native dependency archives, with desktop entry points. Fixed-source scope only: no credentials scan, DSH/lifecycle/native-code execution, complete dependency closure, actual installation, plugin activation or runtime validation.
- **Catalog and daily workflows:** Official manual guidance, capability filters and honest unknown states; catalog entries are not complete integrations. Memory export selection supports preview/confirmation and cross-search retention, with no writes on cancel. Local Markdown does not prove an external agent loaded it. Pi/Hermes configuration and OpenCode model/MCP writes remain disabled; `opencode-ai` remains V1-only.

### Evidence and limitations

The feature baseline is `4dbe8ed7d69e8529398d5f4b383abcaa72131fa7`; all eight checks in [main CI 43237572060](https://github.com/MOSHOULI/agentdeck/actions/runs/43237572060) passed. The final revision after release-documentation edits must pass all gates again. Earlier success is not release-commit evidence.

- Disposable Windows runners exercise actual NSIS installation, the installed WebView/IPC, restart, same-version reinstall, v1.1.1 upgrade and uninstall, preserving synthetic data. Controlled configuration and failed-install/recovery UI flows are covered.
- Native Linux WebView checks cover three fixed Qoder archives/private deployments, reinspection, mixed-version rejection and related controlled flows. Official payloads are inspected, never run. Official Codex npm layout and production parsing have non-executing Linux/Windows evidence; the complete installer/probe flow uses authored substitutes, not a full official Codex run.
- Qwen pinned distribution/source evidence and synthetic configuration GUI paths are covered. **Actual Qwen settings loading remains blocked by PR #22's required network isolation.** No isolation bypass or inclusion of that PR.
- Real Agent authentication, model/online-service flows, DSH execution, every user-data migration, power-loss or real installer-subprocess crash recovery are unverified. Synthetic armed/corrupt states and normal app close/restart do not prove actual crash recovery.
- Specific Linux GUI passes do not establish full Linux/macOS product support. No installers or protected non-Windows vault are released for those platforms. NSIS does not separately install the `aisync` CLI.
- The official GLib two-line backport, provenance and three-profile native regressions remain. Conservative dependency-audit and upstream maintenance warnings are retained. Consult exact CI for intentionally ignored/skipped checks, rather than inferring completion from historical fixed counts.

### Download, data and publication safety

Download `AgentDeck_1.2.0_x64-setup.exe`, `SHA256SUMS.txt` and `BUILD_COMMIT.txt`; compare SHA256, actual tag and build commit. The workflow stages a draft, downloads and byte-compares all three assets, verifies the source tag, then publishes. Existing releases or differently targeted tags are never overwritten. Use the explicit version link.

**The installer is not Windows Authenticode-signed.** Checksums establish integrity, not publisher-certificate trust. Back up valuable configuration, keys and memory before upgrading. DPAPI keys bind to the Windows user/machine; copying only vault.json or Git history is not cross-machine recovery. Local memory and configuration recovery copies may contain plaintext; do not share the entire data directory. Multi-file rollback is not a power-loss journal.

This installer-only release retains `--latest=false` and the original v1.0.0 signed-registry Latest endpoint. No replacement signing key or trust anchor is introduced. The bundled registry floor is 1.2.0; custom, unknown or newer local registries remain preserved. Only exact original v1.0.0 assets select the newer bundled source without deleting the old files.

See [installation/recovery](INSTALL-PREVIEW.md), [Windows acceptance](WINDOWS-ACCEPTANCE.md), [Linux acceptance](LINUX-WEBVIEW-ACCEPTANCE.md), [Qoder deployment](QODER-PRIVATE-DEPLOYMENT.md), [Qwen field](QWEN-OWNED-SETTING.md), [Windows permissions](WINDOWS-CONFIGURATION-DACL.md) and [dependency audit](DEPENDENCY-SECURITY.md).
