# AgentDeck（AI 中枢）

在一个桌面界面中管理本机 AI Agent：发现与安装、配置、可选择的对话记忆导出、加密密钥、MCP 和配置备份。

**当前支持的完整桌面平台是 Windows 10/11 x64。** React + Tauri 2 + Rust 架构；Linux 可运行非密钥相关的核心/CLI，尚不提供 Linux/macOS 的受保护密钥库或完整安装包。

## 安装与运行

1. 从 [AgentDeck v1.1.0 安装包发布页](https://github.com/MOSHOULI/agentdeck/releases/tag/v1.1.0) 下载 `AgentDeck_1.1.0_x64-setup.exe`，用同一发布中的 `SHA256SUMS.txt` 验证文件。发布须通过下述 CI 门禁；若页面尚不存在，安装包尚未发布。
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

- **Agent 广场**：安装/卸载前确认；不具备自动安装或验证能力的 Agent 显示手动说明或未知状态，不伪报成功。安装命令有超时；配置失败不卸载已有软件。认证状态仅检查本地配置证据，不代表远端凭据仍有效。
- **个性化与模型配置**：默认模板首次使用时初始化，保留用户已有模板。个性化使用受管理的 Markdown 区块，保留区块外原文；格式不兼容的目标不强行覆盖。配置跳过原因会显示为安装警告。
- **记忆**：采集 Codex、Claude Code、OpenCode 的本地会话；搜索、手动选择和自动注入标记分别管理。撤销自动标记会立即刷新导出文件；显式手动选择会保留。导出位置为 `~/.agentdeck/memory/<agent>.md`，**外部 Agent 不保证自动读取此文件**，需将其作为上下文或配置引用。定时采集默认关闭，需明确开启后才每小时读取本地会话；可随时关闭。手动采集保持独立。采集有取消/时间预算和部分失败反馈。
- **密钥与备份**：vault 使用 AES-256-GCM，主密钥由 Windows DPAPI 保护。列表只显示脱敏值；复制密钥才取明文。Git 快照中的配置备份加密，随机 nonce，旧快照仍可解密。恢复保留主密钥及未管理文件。
- **MCP**：只修改注册表管理的服务器，保留用户自建服务器；批量变更验证完成后一次写入。HTTP 服务器需要自行启动，AgentDeck 不托管 RAGFlow。
- **在线更新**：只更新 Agent/MCP 注册表，不更新桌面二进制；通过内置公钥验签，校验所有文件后提交，持久化注册表版本。安装程序升级请下载新的 Release。

浏览器直接打开前端时不会模拟操作成功。仅 `?demo=1` 显式启用**只读演示**，有清晰标识，禁止安装、写配置和读取密钥。

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
cargo run --locked -p aisync-cli -- deploy --agent codex --action verify
cargo run --locked -p aisync-cli -- deploy --agent codex --action install --yes
cargo run --locked -p aisync-cli -- sync --tools codex,claude-code --yes
cargo run --locked -p aisync-cli -- backup
cargo run --locked -p aisync-cli -- backups
cargo run --locked -p aisync-cli -- restore <timestamp> --full --yes
```

安装、卸载、同步、恢复需要 `--yes`；不会假装完成。受保护密钥/备份相关命令仅 Windows 可用。

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

测试运行器设置 `AGENTDECK_TEST_HOME`，核心路径解析显式尊重此绝对路径（Windows Known Folder API 不受普通 HOME 变量影响）。

**所有测试使用隔离 HOME。不要运行 `--ignored` 或 `--include-ignored`**：标记忽略的真实环境测试可能安装/卸载第三方软件、访问会话和修改配置。常规 CI 只使用合成夹具。

CI 验证前端、Linux 核心/CLI、Windows 全工作区（含 DPAPI），构建 NSIS 并记录 SHA256 和源码提交。主分支全部门禁通过后先上传草稿 Release，再下载核对安装包字节、SHA256、构建提交与实际标签提交，全部一致才公开发布。现有 Release（包括草稿）或指向其他提交的标签会阻止发布，需人工核查，不会覆盖或按成功跳过。

安装包发布与已签名注册表更新分开：当前注册表检查使用 GitHub 的 `latest` 下载入口，因此仅含安装包的 v1.1.0 发布不会标记为 Latest，保留既有已签名注册表入口。下载桌面新版请使用上方 v1.1.0 链接，不要把 Latest 标签当作最新桌面安装包。

发布者注册表签名使用已有受保护私钥。本仓库不包含该私钥；构建安装包无需它。**没有私钥时不能发布新的可信注册表更新**，不要用新生成的密钥冒充已有信任链；详细验证范围见 [发布说明](docs/RELEASE-1.1.0.md)。
