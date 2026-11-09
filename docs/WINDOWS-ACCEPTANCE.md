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

## Installed Qwen field lifecycle / 已安装桌面的 Qwen 字段生命周期

After clean installation, `windows-qwen-ui-acceptance.ps1` drives the installed WebView using native UI Automation Invoke, Value, Toggle and SelectionItem patterns. Controls are selected by the app process, exact accessible name and control type; ambiguous or missing controls fail. It does not evaluate JavaScript or invoke IPC directly.

The fixture contains only synthetic `settings.json` and a package manifest declaring Qwen 0.27.0; it is **not a Qwen installation**. Assertions cover:

- Read-only preview, cancel and clearing selected paths: unchanged settings bytes and no receipt/lock creation.
- External byte changes after preview: apply rejects the stale token without replacing those bytes or creating a receipt.
- Confirmed apply: the actual backend changes `general.vimMode`, preserves every unrelated fixture field and the manifest, and creates an ownership receipt without copying the synthetic secret canary.
- Closing/restarting the installed app after apply; cancelling restore preserves settings and receipt bytes.
- External managed-field drift after opening restore confirmation: restore rejects it and preserves both settings and receipt.
- Resetting only the fixture to its recorded applied bytes, then confirming restore: original Boolean and unrelated values are preserved and the receipt is removed. Formatting need not match the original bytes.
- Accessibility evidence is checked for the synthetic secret canary before upload. UI completion messages and on-disk assertions must both pass; neither alone is sufficient.

The fixture reset is test setup, not an automatic conflict-resolution feature. This does not run Qwen, test real loader behavior, simulate process death during a write, or guarantee all GUI interactions. Linux/cloud static checks do not prove Windows acceptance; the exact Windows CI run must pass before claiming it verified.

全新安装后，通过原生 UI Automation 的 Invoke、Value、Toggle、SelectionItem 操作真实安装程序的 WebView，按进程、精确可访问名称及控件类型定位，缺失或重名即失败，不注入 JavaScript、不直接调用 IPC。

测试只使用合成 settings.json 和声明 Qwen 0.27.0 的 package.json，**不是真实 Qwen 安装**。覆盖只读预览、取消、清除路径零写入；预览后外部改字节导致令牌失效且禁止应用；确认应用后实际字段写入、无关值及 manifest 保留、所有权记录生成；关闭并重启应用后取消撤销零改动；确认撤销前外部修改受管字段时拒绝覆盖并保留记录；仅将测试夹具重置为已记录的应用后字节，再确认撤销，恢复原布尔值并移除记录。UI 完成状态与磁盘断言必须同时通过，可访问性证据上传前检查合成秘密标记未泄露。

夹具重置不是自动解决用户配置冲突的功能。本测试不执行 Qwen、不验证真实运行时加载、不模拟写入中途进程崩溃，也不覆盖所有交互。云端 Linux 静态检查不能替代精确提交的 Windows CI 验收通过。

The Qwen lifecycle also clicks read-only recovery observation for all four states
(no receipt, field restoration, receipt-only cleanup, conflict), checks that the
entire synthetic directory's file names and byte hashes remain unchanged, and
ensures no restore/apply confirmation is opened by inspection. This tests the
installed UI/IPC route, not real Qwen behavior; Windows CI must actually pass.

Qwen 生命周期还通过真实点击检查四种恢复观察状态，核对整个合成目录文件名与
字节哈希均不变，诊断不打开应用/撤销确认。此项验证已安装 UI/IPC，不代表真实
Qwen 运行效果，仍须以 Windows CI 实际通过为准。

## Qoder read-only workspace / Qoder 只读工作区

Installed UI acceptance selects the explicit Linux baseline plan through native
keyboard/UIAutomation, checks its fixed official archive filename, then verifies
that a synthetic mismatching archive is rejected through real IPC with unchanged
bytes and file count. Clearing the selected path is also checked. It performs no
Qoder download, extraction, execution or installation; real matching archive success
on Windows remains unverified by this smoke test.

已安装UI通过原生键盘/UIAutomation选择明确Linux baseline计划并核固定包名，
再经真实IPC拒绝合成摘要错误包，核对文件字节及数量不变，并测试清除路径。不下载、
解包、执行或安装Qoder；此冒烟不证明真实匹配包在Windows上的成功验收。
