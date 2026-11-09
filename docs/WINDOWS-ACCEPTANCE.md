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

## Qoder real-archive read-only verification / Qoder 真实归档只读校验

The installed UI still tests explicit-plan selection and synthetic mismatching
archive rejection. It now also hashes the pinned official **Linux amd64-baseline**
Qoder 1.1.85 archive while running on Windows, using actual UIAutomation actions.
Success must display the fixed SHA256-match message and exactly 78,587,775 bytes.
Original archive hash, write time and directory file count must remain unchanged.
Changing the selected target clears success; a subsequent successful read followed
by a path change must clear it again. A separate same-name/same-size copy with one
changed byte must then be rejected, proving the result is not reused from the
previous success. No archive is unpacked and no Qoder code is executed.

`scripts/fetch_qoder_ui_archive.py` is CI-only, guarded for a disposable GitHub-hosted
Windows runner and a NEW directory beneath RUNNER_TEMP. Its only network destination
is the exact version-pinned URL from the reviewed manifest; no URL/version/hash
CLI or environment overrides exist. TLS verification uses Python defaults and all
redirects are rejected. Status must be 200, one exact Content-Length is required,
content encoding must be absent/identity and transfer encoding is rejected. Streaming
is bounded to the expected size plus one overflow-detection byte, using 64 KiB
chunks, and the complete SHA256 must match. Per-socket timeout is 15 seconds; a
supervising process enforces a 180-second total worker deadline (up to 10 additional
seconds to terminate/kill). Existing destinations are never reused or overwritten.
Failure retains partial bytes, reports failure and blocks acceptance, never skips.

The helper rejects linked/reparse ancestor paths; exclusive creation prevents
reusing a leaf. The app's actual core reader independently applies its reviewed
no-follow/nonblocking, bounded-file validation. These are ordinary temporary CI
files with best-effort race checks, **not a security/isolation namespace or a
sandbox against a same-UID process**. Only result hashes, counts and accessibility
logs/screenshots are uploaded; the real archive and corrupted copy are outside the
evidence directory and are never uploaded as artifacts. Offline downloader tests
cover redirects, wrong headers, truncated/oversized bodies, wrong hashes, existing
destinations, timeout and worker failure without contacting the service.

This is real Windows **read/hash** UI evidence for a Linux archive, not validation
of a Windows-native Qoder package, installation, runtime, login or signatures.
Temporary CI hosts are discarded by the runner service; the script does not run
Qoder or recursively clean partial data. Network availability is a new explicit
acceptance dependency; failure must remain visible. Exact-commit CI must pass
before reporting the real installed flow verified.

现保留原计划/合成错误包门禁，并在Windows已安装界面真实点击校验固定官方Linux
baseline归档，必须显示摘要匹配与78587775字节。核对原包hash、写入时间、文件数
不变；改目标/路径均使成功结果失效，并用单字节损坏的同名同大小副本验证拒绝。
不解包、不运行Qoder，不等于Windows原生包的安装、运行或登录兼容性。

CI专用下载器仅允许全新RUNNER_TEMP目录和固定官方URL，TLS正常验签，拒绝所有
重定向、异常状态/长度/编码。64KiB流式读取、精确大小加一字节超限检测、固定SHA256；
socket超时15秒，独立监督进程限制180秒总下载时间，终止最多额外10秒。已有目标拒绝
覆盖，失败保留partial并阻断门禁，不跳过。离线测试不联网。

下载器拒绝父路径链接/reparse并独占创建文件，应用另有安全reader复查；这只是临时
CI文件及尽力竞态检查，不是隔离namespace或同UID攻击沙箱。只上传哈希、计数和UI证据，
不上传归档或损坏副本。新增网络依赖不可用时明确失败，只有精确提交CI通过才算真实
已安装流程验证，不以本地mock/AST替代。

### Qoder cache unsupported-host boundary / 缓存诊断平台边界

The installed Windows application now also receives an existing synthetic cache directory via actual UI input and click. Acceptance requires the distinct unsupported-host alert, preserves the fixture hash, directory member count and directory modification time, and records `qoder-cache-unsupported-host` evidence. No Linux success state is fabricated; this does not cover installed Linux GUI success. The cache observer has no desktop write, cleanup, repair or activation operation.

Windows 安装版新增真实 UI 缓存入口验收：输入已存在的合成目录并点击，必须显示独立的“不支持当前主机”提示，核对文件摘要、成员数量和目录修改时间未变，记录 `qoder-cache-unsupported-host` 证据。不能用不存在路径错误冒充平台拒绝，也不模拟 Linux 成功态；Linux 真实 GUI 成功仍未验收。没有缓存写入、清理、修复或激活操作。
