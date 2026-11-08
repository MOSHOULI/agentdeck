# Windows installer acceptance / Windows 安装验收

The `windows-acceptance` job runs **only on a disposable GitHub-hosted Windows runner**, never on the user's computer. It downloads the NSIS artifact for the exact build commit, verifies its SHA256 and provenance, and checks the fixed published v1.1.1 baseline against that release's checksum. A published checksum is not an independent code signature. No release is published by this job.

`windows-acceptance` 仅在一次性 GitHub Windows runner 上运行，不使用用户电脑。校验本次精确提交的 NSIS 产物、SHA256 和提交标识，并以 v1.1.1 的发布校验文件核验升级基线。校验文件不等于独立签名。本任务不发布安装包。

## Assertions / 验收断言

- Install into a fresh path containing Chinese characters and spaces; assert installed product version.
- Launch the installed executable, not the build-tree executable. UI Automation must see the native-mode label and a returned Agent card, exercising WebView rendering and real `get_agents` IPC. Merely finding a process/window is insufficient.
- Close and restart, then reinstall the same version and launch again.
- Uninstall, install v1.1.1 without launching it, upgrade to the current build, launch, and uninstall.
- Preserve a synthetic file in the isolated application home across all operations.
- Keep screenshots, accessibility names, exact hashes and stage results as CI evidence; missing GUI access fails rather than silently skipping.

- 在含中文与空格的临时路径安装，校验程序版本。
- 启动安装后的程序，使用 UI Automation 核对本地模式及后端返回的 Agent 卡片，覆盖真实 WebView 与 `get_agents` IPC，不能只凭进程/窗口存在判通过。
- 关闭重启、同版重装并启动；卸载后安装 v1.1.1（不启动），升级到本次版本，再启动并卸载。
- 每阶段核对隔离应用目录中的合成用户文件保留；保存截图、可访问性名称、哈希及阶段结果。GUI 不可访问时失败，不跳过。

## Isolation and limits / 隔离与限制

The script refuses non-GitHub-hosted environments. It uses a unique synthetic home, a minimal system-only PATH and a fresh working directory. NSIS still writes the disposable runner's per-user registry/shortcuts; the runner is destroyed afterwards. There are no real credentials or Agent installations. Failure evidence remains in that ephemeral runner and uploaded artifacts.

This is smoke acceptance, not comprehensive manual GUI testing, real Agent installation, authentication, external MCP verification or recovery of arbitrary user configuration. The preserved file is a sentinel, not proof of migration compatibility for every database/config format. UI labels are intentionally strict and must be updated alongside legitimate UI changes. The baseline download is a network dependency; failure must be reported, not replaced with a same-version reinstall and called an upgrade.

脚本拒绝非 GitHub-hosted 环境，使用独立合成 home、仅系统 PATH 和空工作目录。NSIS 仍会写入一次性 runner 的当前用户注册表/快捷方式，runner 随后销毁。没有真实凭据或 Agent 安装。这是冒烟验收，不代表完整人工 GUI、真实 Agent/认证/MCP 验证，也不证明所有数据库格式都可迁移。基线下载失败必须报告，不能用同版重装冒充升级测试。
