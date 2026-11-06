# AgentDeck 1.1.1

## 中文

这是基于已合并 GLib 修复源码构建的 Windows 10/11 x64 补丁安装包，不包含 PR #2 的安装来源识别或 PR #3 的 DSH 被动盘点功能。

### 本次变化

- 为 Linux 的 GTK/GLib 依赖回移了 `RUSTSEC-2024-0429` 的官方两行修复，保留 `glib 0.18.5` 原版本、完整源码与许可。受影响的 Linux GLib 依赖不在 Windows 安装包的依赖图中。
- 添加官方包校验、121 个文件的源码核验、实际应用依赖图检查，以及18项真实 GLib 回归在三档优化下的执行。审计保留原公告，不用 path 依赖隐藏告警。
- 桌面、CLI、核心库和前端版本同步为 1.1.1。现有版本规则也将内置注册表版本下限提高至 1.1.1；注册表文件内容、签名公钥和外部更新源未改变。对应边界测试继续拒绝旧版本和同版本更新。

### 下载与校验

从 [v1.1.1 发布页](https://github.com/MOSHOULI/agentdeck/releases/tag/v1.1.1) 下载 `AgentDeck_1.1.1_x64-setup.exe`、`SHA256SUMS.txt` 和 `BUILD_COMMIT.txt`。页面不存在则尚未发布。比对 SHA256 清单，并将构建提交与成功的发布 CI 对照。

**安装包未做 Windows Authenticode 签名。** SHA256 只能核对文件完整性，不等同发布者证书。升级前请备份有价值的配置、密钥和记忆数据，并先在可恢复的测试环境试用。加密备份不能代替对原有密钥的妥善保管；受 DPAPI 保护的密钥绑定 Windows 用户/机器。

发布工作流先创建草稿、重新下载并比较全部资产、校验 tag 和构建提交，再公开发布。已有 v1.1.0 不会覆盖。本发布使用 `--latest=false`，保留既有签名注册表的 Latest 入口；桌面新版请使用上述明确版本链接。没有原签名私钥，因此不发布新签名注册表或替换信任公钥。

### 验证与限制

发布须通过当前提交的前端、Linux 核心/CLI、三档原生 GLib 回归、保守依赖审计、Windows 原生测试及 NSIS 构建门禁。CI 运行记录是最终证据，旧提交的通过结果不能代替本次验证。

- Windows 自动化测试和打包不等于实际安装、手工 GUI 验收或第三方 Agent 的安装、登录、网络流程验证；这些未完成。
- 完整 Linux 桌面、GTK/WebKit GUI、Linux 分发和非 Windows 受保护密钥库仍未验证或不受支持。此修复不宣布 Linux 桌面支持。
- 保守审计仍显示 GLib 原公告及4项上游维护警告；上游编译警告也未清零。
- 真实环境测试继续忽略，普通 CI 使用隔离目录和合成数据，不读取真实密钥或会话。
- v1.1.0 的其他限制继续适用：未知来源不能自动卸载，Agent 软件升级、配置差异预览和三方冲突对账尚未实现。

更多说明：[GLib 修复](GLIB-BACKPORT.zh-CN.md)、[依赖审计](DEPENDENCY-SECURITY.md)、[v1.1.0 完整限制](RELEASE-1.1.0.md)。

## English

This Windows 10/11 x64 patch installer is built from the source revision containing the merged GLib fix. It does not include installation-origin discovery from PR #2 or DSH passive inventory from PR #3.

### Changes

- Backport the official two-line `RUSTSEC-2024-0429` fix to the Linux GTK/GLib dependency, preserving `glib 0.18.5`, its complete source and license. The affected Linux GLib dependency is absent from the Windows installer dependency graph.
- Add official archive verification, checks of all 121 vendored files, actual application dependency-graph validation, and 18 real GLib regressions at three optimization levels. The conservative audit retains the original advisory rather than hiding it through a path dependency.
- Synchronize desktop, CLI, core and frontend versions at 1.1.1. Existing version rules also raise the bundled registry floor to 1.1.1; registry contents, signing public key and external feed remain unchanged. Boundary tests continue rejecting older and equal-version registry updates.

### Download and verification

Download `AgentDeck_1.1.1_x64-setup.exe`, `SHA256SUMS.txt` and `BUILD_COMMIT.txt` from the explicit [v1.1.1 release page](https://github.com/MOSHOULI/agentdeck/releases/tag/v1.1.1). A missing page means it has not been published. Verify the checksum and compare the build commit with the successful release CI run.

**The Windows installer is not Authenticode-signed.** SHA256 verifies byte integrity, not publisher certificate trust. Back up valuable configuration, keys and memory before upgrading, and try a recoverable test environment first. Encrypted backups do not replace safeguarding the original key material; DPAPI-protected keys are bound to the Windows user/machine.

The workflow stages a draft, downloads and compares every asset, verifies the tag and build commit, then publishes. It never overwrites v1.1.0. This installer-only release uses `--latest=false` to preserve the existing signed registry Latest endpoint; use the explicit version link for the desktop installer. The original signing private key is unavailable, so no new signed registry or replacement trust key is published.

### Validation and limitations

Publication requires this commit to pass frontend, Linux core/CLI, three-profile native GLib, conservative dependency-audit, Windows-native and NSIS build gates. The release CI run is authoritative; an older commit's pass does not substitute for these checks.

- Windows automation and packaging do not establish actual installation, manual GUI acceptance, or third-party Agent installation/login/network behavior. Those remain unverified.
- Full Linux desktop, GTK/WebKit GUI, Linux distribution and protected non-Windows vault support remain unverified or unsupported. This fix does not announce Linux desktop support.
- The conservative audit still displays the original GLib advisory and four upstream maintenance warnings; upstream compiler warnings remain.
- Real-environment tests remain ignored. Ordinary CI uses isolated directories and synthetic data, not actual secrets or conversations.
- Other v1.1.0 limits remain: unknown installation sources cannot be automatically uninstalled; Agent software upgrades, configuration diff previews and three-way conflict reconciliation are not implemented.

Further details: [GLib backport](GLIB-BACKPORT.md), [dependency audit](DEPENDENCY-SECURITY.md), [complete v1.1.0 limitations](RELEASE-1.1.0.md).
