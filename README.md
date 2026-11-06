# AgentDeck（AI 中枢）

简体中文 | [English](README.en.md)

本文描述 v1.2.0 源码及计划交付内容：安装实例与来源证据、受限 npm 安装、目录元数据修正和 DSH 桌面/CLI 被动盘点。已发布的 v1.1.1 不包含这些增量；是否已发布请以对应 Release 为准，并核对 `BUILD_COMMIT.txt` 与成功的发布 CI 提交。详见 [v1.2.0 中英文说明](docs/RELEASE-1.2.0.md)。

在一个桌面界面中管理本机 AI Agent：发现与安装、配置、可选择的对话记忆导出、加密密钥、MCP 和配置备份。

**当前支持的完整桌面平台是 Windows 10/11 x64。** React + Tauri 2 + Rust 架构；Linux 可运行非密钥相关的核心/CLI，尚不提供 Linux/macOS 的受保护密钥库或完整安装包。

## 安装与运行

1. 从 [AgentDeck v1.2.0 安装包发布页](https://github.com/MOSHOULI/agentdeck/releases/tag/v1.2.0) 下载 `AgentDeck_1.2.0_x64-setup.exe`，用同一发布中的 `SHA256SUMS.txt` 验证文件。发布须通过下述 CI 门禁；若页面尚不存在，安装包尚未发布。
2. 安装 [Git for Windows](https://git-scm.com/downloads/win) 并确保 `git` 在 PATH（加密备份需要）。使用 npm 类型 Agent 前还需要 Node.js/npm。
3. 安装包目前**未做 Windows Authenticode 代码签名**。SHA256 用于完整性核对，不等同于发布者身份签名。

### 从源码构建（Windows）

要求：Rust stable（MSVC）、Node.js 24 LTS、Git、Microsoft C++ Build Tools 与 WebView2。

```powershell
git clone https://github.com/MOSHOULI/agentdeck.git
cd agentdeck
npm ci --prefix web/ui
npm exec --prefix web/ui -- tauri dev
# NSIS 安装包输出到 target/release/bundle/nsis/
npm exec --prefix web/ui -- tauri build --bundles nsis --ci -- --locked
```

无需全局安装 Tauri CLI；Rust 和 npm 依赖锁文件都提交到仓库。

## 使用

v1.2.0 源码的桌面界面有 Agent 管理、记忆管理、MCP 服务、密钥与备份、DSH 声明盘点、注册表更新六个工作区。侧栏可折叠，支持浅色、深色或跟随系统；导航不会中断正在执行的操作，顶部与侧栏会保留操作状态。Agent 搜索只筛选当前真实列表，详情中的未知状态不会被替换成成功。

- **Agent 管理**：安装前确认，只安装软件，不初始化 profile、密钥库或 Git 备份，也不自动应用配置。仅支持固定 npm 适配器和已检查的管理器/prefix，不执行任意注册表 shell 配方。检测区分已验证、PATH 中未发现、检测失败及无法检测；PATH 未发现不代表整台机器都未安装。目标 prefix 中已有的包或启动器即使不在 PATH，也会阻止安装；证据不足、冲突或目标变化时拒绝继续，不隐式重装或升级。安装器失败或安装后核验失败时，软件可能已发生改动，**不会自动回滚或卸载**。操作完成与列表刷新失败分别显示，状态过期时暂停写操作。
- **安装实例与来源证据**：分别显示 PATH 候选路径、文件真实路径、可证明的实际入口及当前活动实例的实测版本。重复的 PATH 别名不会冒充多套安装；不同实例或无法完整检查时明确提示，暂停配置操作。PATH 搜索不是整机扫描，无法检测到的其他安装不会被假定不存在。
- **npm 关联与卸载边界**：只有确切 npm 实例的全局 root/prefix、包 name/bin、链接或受支持的标准 Windows shim 与实际入口全部一致，才显示“npm 全局目录匹配”。这只是当前目录关联证据，**不证明安装历史，也不授权卸载**。自定义 wrapper、冲突或证据不足时仍为未知；不从注册表安装方式或 package.json 的版本推定来源与实测版本。自动卸载和 Agent 软件升级仍未开放，包括已匹配 npm 目录的实例，请使用原安装方式管理。
- **个性化与模型配置**：通过独立的“应用配置”操作确认后才应用默认 profile，不执行安装器。配置与个性化同步要求受支持的适配器、成功且无歧义的最新安装验证；在准备 profile/备份前检查，并在写入前复查。仅来源关联未知不会禁用一个已验证且无歧义的实例，但不授予卸载/升级权限。模板首次配置时初始化，保留用户已有模板。个性化使用受管理的 Markdown 区块，保留区块外原文；格式不兼容的目标不强行覆盖。跳过原因单独显示。当前尚无配置差异预览或本机改动三方对账，应用前请检查模板和现有配置。
- **记忆**：采集 Codex、Claude Code、OpenCode 的本地会话；搜索、手动选择和自动注入标记分别管理。撤销自动标记会立即刷新导出文件；显式手动选择会保留。导出位置为 `~/.agentdeck/memory/<agent>.md`，**外部 Agent 不保证自动读取此文件**，需将其作为上下文或配置引用。定时采集默认关闭，需明确开启后才每小时读取本地会话；可随时关闭。手动采集保持独立。采集有取消/时间预算和部分失败反馈。
- **密钥与备份**：vault 使用 AES-256-GCM，主密钥由 Windows DPAPI 保护。列表只显示脱敏值；复制密钥才取明文。Git 快照中的配置备份加密，随机 nonce，旧快照仍可解密。恢复保留主密钥及未管理文件。
- **MCP**：只修改注册表管理的服务器，保留用户自建服务器；批量变更验证完成后一次写入。HTTP 服务器需要自行启动，AgentDeck 不托管 RAGFlow。
- **DSH 声明盘点**：在新增工作区中输入明确的 profile 绝对目录路径，可另选 runtime `package.json`，点击后才读取最多两份有界 manifest。路径不持久化，不扫描 HOME，不创建目录，不运行 DSH/Node/包管理器，不读取凭据、`cordis.yml` 或插件代码，也不写配置。保存的声明不证明软件已安装或插件已启用；运行、认证和实际启用状态始终未知，所有管理修改均关闭。普通浏览器不可读取本地文件，显式演示仅返回合成示例。
- **在线更新**：只更新 Agent/MCP 注册表，不更新桌面二进制；通过内置公钥验签，校验所有文件后提交，持久化注册表版本。安装程序升级请下载新的 Release。

浏览器直接打开前端时不会模拟操作成功。仅 `?demo=1` 显式启用**只读演示**，有清晰标识，禁止安装、写配置和读取密钥。

实例与来源证据、安装计划及配置守卫见 [安装证据说明](docs/INSTALLATION-EVIDENCE.zh-CN.md)。各 Agent 的上游要求与关闭的适配器见 [注册表来源核验](docs/REGISTRY-SOURCES.zh-CN.md)：Pi/Hermes 配置写入保持关闭；OpenCode 的 MCP/模型写入关闭，旧的运行时注册表描述也不能重新启用这些写入；保留受支持的 Markdown 指令文件适配器，`opencode-ai` 安装途径只对应 V1，不表示 V2 安装或迁移支持。未经核实的 star 数、推荐标记及排名描述已从内置目录撤下，不构成产品排名。桌面路径选择及版本范围见 [DSH 被动盘点说明](docs/DSH-PASSIVE-INVENTORY.zh-CN.md)。

## 数据与恢复边界

数据在 `~/.agentdeck/`：`memory.db`、`memory/`、`vault/`、`profiles/default/`、`backups/`、`git-backup/`、`registry/`。

- 本地会话和记忆数据库为明文；配置写入前的本地恢复副本也可能含凭据。保护账户与磁盘，不要分享整个数据目录。只有进入 Git 快照的配置副本会加密；vault 的 API Key 不以明文落盘。
- 备份恢复针对 **AgentDeck 管理的 vault 与配置副本**，不是系统还原，也不会卸载/重装软件或自动将副本覆盖回外部 Agent 的配置路径。
- 不要删除 `vault/master.key`。DPAPI 密钥绑定 Windows 用户/机器；只复制 vault.json 或 Git 仓库不是跨机器可恢复备份。已有加密历史缺少主密钥时程序拒绝静默重建。
- 多文件配置/恢复有错误回滚，但不承诺断电原子性。遇到失败应保留数据目录并检查错误，不要删除恢复副本。
- v1.0.0 历史 Git 提交保留旧加密格式；新快照使用随机 nonce，不能消除已分享旧历史的风险。
- 技能分发没有适配器，目前明确报“不支持”，不会空实现成功。

## CLI

```powershell
cargo run --locked -p aisync-cli -- health
cargo run --locked -p aisync-cli -- agents
# DSH 静态盘点：仅显式选定的已有目录，不启动 DSH 或创建 profile
cargo run --locked -p aisync-cli -- dsh inspect --profile-dir "C:\DSH\profiles\web"
cargo run --locked -p aisync-cli -- deploy --agent codex --action verify
cargo run --locked -p aisync-cli -- deploy --agent codex --action install --yes
# 可选的独立配置操作，需要 Git 备份与受保护密钥库支持
cargo run --locked -p aisync-cli -- deploy --agent codex --action configure --yes
cargo run --locked -p aisync-cli -- sync --tools codex,claude-code --yes
cargo run --locked -p aisync-cli -- backup
cargo run --locked -p aisync-cli -- backups
cargo run --locked -p aisync-cli -- restore <timestamp> --full --yes
```

安装、配置、同步、恢复需要 `--yes`；安装不会隐式写配置，只执行通过目标核验的固定 npm 计划。`deploy --action verify` 输出实例与来源观察。配置及个性化同步在准备 profile 或备份前检查安装资格，并在写入前复查；CLI `sync` 先检查全部个性化目标，任一不合格都不会开始备份或写入。仅记忆导出不要求安装。自动卸载与 Agent 软件升级仍未开放，`--yes` 不会解除限制。受保护密钥/备份相关命令仅 Windows 可用，纯软件安装不依赖该备份流程。

`dsh inspect` 与桌面 DSH 工作区使用相同的被动读取器；它仅返回 JSON，不进入上述部署流程。它只读取明确选择的 profile 的 `package.json`，可通过 `--runtime-manifest` 另外选择一个 runtime 包的 `package.json`。不扫描 HOME，不调用 DSH/npm/pnpm，不读取凭据、`cordis.yml` 或插件代码。包依赖和 bundle selection 仅是保存的声明，版本仅是 manifest 报告值，不代表软件已安装、插件正在运行或已经认证。退出码 0 只表示成功产生报告；缺失、不支持或未知状态需检查 JSON。解析约定仅依据 `0.2.2-alpha.3` 快照审查；2026-11-06 固定核验的上游默认分支已声明 `0.2.2-rc.1`，不表示本分支新支持 rc.1。所有版本的管理写操作均未开放。详见 [DSH 被动盘点边界](docs/DSH-PASSIVE-INVENTORY.zh-CN.md)。

## 测试与发布

```powershell
npm ci --prefix web/ui
npm run typecheck --prefix web/ui
npm test --prefix web/ui
npm run build --prefix web/ui
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
python scripts/run-isolated.py cargo test --locked --workspace
```

Linux 核心检查：`python scripts/run-isolated.py cargo test --locked -p aisync-core -p aisync-cli`。

本分支为 Linux 桌面依赖 `glib 0.18.5` 回移了 `RUSTSEC-2024-0429` 的上游两行修复，保留原版本与告警。源码、实际依赖路径及18项真实 GLib 回归已在三档优化下验证通过；完整 Linux 桌面构建和图形界面仍未验证，也未消除其他维护警告。精确提交、CI证据和限制见 [中文说明](docs/GLIB-BACKPORT.zh-CN.md) / [English guide](docs/GLIB-BACKPORT.md)。

测试运行器设置 `AGENTDECK_TEST_HOME`，核心路径解析显式尊重此绝对路径（Windows Known Folder API 不受普通 HOME 变量影响）。

**所有测试使用隔离 HOME。不要运行 `--ignored` 或 `--include-ignored`**：8 项标记忽略的手动/真实环境测试可能安装/卸载第三方软件、访问会话和修改配置。常规 CI 只使用合成夹具。当前安装安全与 DSH 增量未通过真实 Agent 安装、桌面 GUI 或真实 DSH 执行验证；模拟 IPC、临时 manifest 和替身进程测试不能替代这些验证。

CI 验证前端、Linux 核心/CLI、Windows 全工作区（含 DPAPI），构建 NSIS 并记录 SHA256 和源码提交。PR 或合并 main 不会自动发布安装包；只有在 main 手动触发工作流并明确开启 `publish_release`，且全部验证门禁通过后，才会先上传草稿 Release，再下载核对安装包字节、SHA256、构建提交与实际标签提交，全部一致才公开发布。现有 Release（包括草稿）或指向其他提交的标签会阻止发布，需人工核查，不会覆盖或按成功跳过。

安装包发布与已签名注册表更新分开：当前注册表检查使用 GitHub 的 `latest` 下载入口，因此仅含安装包的 v1.2.0 发布不会标记为 Latest，保留既有 v1.0.0 已签名注册表入口，且不覆盖历史 Release。下载桌面新版请使用上方 v1.2.0 链接，不要把 Latest 标签当作最新桌面安装包。

升级时，仅当本地两份注册表与官方 v1.0.0 已签名资产的 SHA256 完全一致，且版本标记为 v1.0.0 或不存在，才改用新版内置注册表；原文件和版本标记保持不动。修改过、来源未知或版本更新的本地注册表继续保留并使用。核心包版本同时确定内置注册表版本下限，现为 1.2.0；这不代表已发布同版本的签名注册表。在线更新版本必须同时高于当前实际使用版本与 1.2.0 下限，不能用旧版或同版更新覆盖新版内置修正。签名公钥与外部更新源不变。

发布者注册表签名使用已有受保护私钥。本仓库不包含该私钥；构建安装包无需它。**没有私钥时不能发布新的可信注册表更新**，不要用新生成的密钥冒充已有信任链；详细验证范围见 [发布说明](docs/RELEASE-1.2.0.md)。
