# Linux real WebView acceptance / Linux 真实 WebView 验收

## Official route and dependency evidence

Reviewed on 2026-11-09:
- https://v2.tauri.app/develop/tests/webdriver/manual-setup/
- https://v2.tauri.app/develop/tests/webdriver/ci/
- https://v2.tauri.app/develop/tests/webdriver/example/selenium/

These document the external `tauri-driver` → distro `WebKitWebDriver` route on Linux, with Xvfb for the display. We deliberately do not use embedded WDIO plugins, renderer-only browser mode, IPC mocks, `executeScript`, `browser.tauri.execute`, or a dev server. The normal debug native application embeds the built frontend and uses the real Rust IPC observer.

The official crates.io index and downloaded `tauri-driver` 2.1.0 crate agree on SHA256 `2f2e29bd6900fb718dfdc06f2e7a371d6087c085daaf83709847ef3e6b0c1953`. CI installs precisely `cargo install tauri-driver --version 2.1.0 --locked`. Source `src/webdriver.rs` sets `TAURI_WEBVIEW_AUTOMATION=true` / legacy `TAURI_AUTOMATION=true` for the native driver; this is official automation support, not a sandbox override. Source `src/server.rs` maps `tauri:options.application` to WebKit's native application capability. Both proxy and native interfaces bind loopback. CI uses the explicit `/usr/bin/WebKitWebDriver` path rather than accepting a shadow binary from PATH.

Ubuntu 24.04 official apt dependencies: GTK3/WebKit2GTK 4.1 development files, `libayatana-appindicator3-dev`, `webkit2gtk-driver`, `xvfb`, `xauth`, `dbus-x11`. Package versions are recorded in job logs. Native driver and WebKit come from the same distro update, avoiding an unrelated driver build.

## Scope and assertions

The hosted-runner-only harness creates a new temporary HOME and XDG directories plus private synthetic fixture directories. This separates test data from normal profiles; it is **not a new security namespace** and does not claim to defeat same-user attackers. It opens the actual Qoder tab, types each absolute cache path and clicks the real diagnosis button through standard W3C element commands. It requires fixed visible `unknown_ownership`, `incomplete`, and `conflict` messages via their local UI labels, as well as the no-deletion-authorization boundary. The unknown case is an existing empty private directory; partial state uses the fixed intent plus partial bytes; conflict adds an unpublished pending marker. No Qoder executable is downloaded or executed.

Changing the path must remove the previous result before the next click. Clear must remove the result and reset the input. Before and after all observations, the harness compares all fixture members, hashes, inode, owner, mode, link count, size, mtime and ctime. Access times are excluded because reading may update them. A synthetic content canary must never appear in the rendered body; private fixture files are never uploaded. Evidence is fixed assertion metadata plus capped (1 MiB) driver logs, scrubbed if the content canary appears.

Python stdlib `http.client` speaks only the fixed loopback WebDriver protocol, without proxy discovery and with every HTTP redirect rejected; no extra client library or product test hook is needed. Requests have 30-second socket timeouts, wait loops have deadlines, responses are capped at 1 MiB, and the real UI step has an eight-minute workflow deadline. Only the process group started by the harness is stopped on completion. Offline harness tests check command construction, no script endpoint, fixture boundaries, guards, response/log limits; **they are not GUI acceptance**.

The job is required by the release job. Missing drivers, unavailable sandbox/user namespaces, failed app launch, UI mismatch or unexpected fixture modification fail the job, never skip to green. No WebKit sandbox-disabling environment variables, sysctl changes, AppArmor changes or namespace bypasses are supplied. The harness explicitly refuses sandbox override variables. If the runner cannot start WebKit under its existing policy, retain the failure and arrange a compatible runner rather than relaxing isolation.

## Current validation boundary

The dot cloud has no WebKit development library, WebKitWebDriver or Xvfb, so only offline checks can run here. A read-only user-namespace probe succeeded; this is not proof that WebKit startup will succeed. The first actual GUI result must come from the new Linux CI job. Full staged/unpacked official-cache states remain covered by separate real Linux CLI acceptance, **not** this synthetic three-state GUI scope. This is a native debug-app WebView test, not Linux installation-package lifecycle acceptance.

## 中文说明

采用 Tauri 官方外置 `tauri-driver` 2.1.0 → 系统 `WebKitWebDriver`，在 Ubuntu 24.04 的 DBus/Xvfb 下启动真实原生调试应用，嵌入构建后的前端并走实际 Rust IPC。不加入测试插件，不直接调用 IPC，不执行 JavaScript，不用浏览器 mock 或开发服务器冒充原生验收。

仅验证三种合成缓存态：未知归属、未完成和冲突。真实输入路径/点击按钮，核对固定状态文案和不授权删除边界；换路径使旧结论消失，清除使结果和输入归零。前后全目录成员、SHA256、inode/owner/mode/nlink/size/mtime/ctime 不变，atime 可因读取改变。新建临时 HOME/XDG 只是测试数据分离，不声称安全沙盒。合成文件内容金丝雀不得出现在页面或上传日志；不上传缓存、不运行 Qoder。

CI 安装官方 GTK/WebKit2GTK 4.1、匹配原生 driver 和 Xvfb，固定 crates.io driver 版本；日志与响应有界，步骤限时。禁止禁用 WebKit sandbox、修改 sysctl/AppArmor 或绕过 namespace 限制；启动失败必须红灯，不跳过成绿。发布依赖此门禁。当前 dot 云端缺 WebKit/Xvfb，离线测试不是 Linux GUI 成功证据，必须等待 CI 实跑。完整真实缓存已有 CLI 验收但不包含在本轮 GUI 范围，也不声称 Linux 安装包生命周期已验收。

### Failure diagnostics / 失败定位

Failures retain a fixed phase and sanitized failure code. Before closing an established session, standard read-only WebDriver body-text and page-source endpoints capture bounded diagnostic strings (at most 64 KiB each, subject to the existing 1 MiB response cap). The complete bounded response is checked for the synthetic private-content canary before any truncation; matching evidence is suppressed. No screenshot/JavaScript or direct IPC is used, no raw exception messages are retained, and diagnostics cannot turn a failure green. Initial CI showed an actual application launch with AT-SPI/DRI3 warnings but no confirmed sandbox denial; this diagnostic increment does not guess at or disable rendering/security settings.

失败证据新增固定阶段及脱敏错误码；关闭会话前仅通过标准只读 WebDriver 读取页面文本和源码，各最多保留 64 KiB，并在截断前对完整有界响应检查内容金丝雀，命中即抑制。没有 JavaScript、直接 IPC、截图或原始异常消息，不会把失败变成成功。首次 CI 已启动真实应用，仅有 AT-SPI/DRI3 警告，未证实 sandbox 拒绝；本次只补定位，不猜测修改图形或安全设置。

Typing is followed by a standard WebDriver `property/value` exact readback, and clicking inspection waits for the native `enabled` response to be true. A timeout in either precondition fails before clicking. Failure diagnostics retain only input agreement/type/length-bound booleans and button-enabled state, never an arbitrary input value. This distinguishes React/input readiness from IPC/result failures without JavaScript.

输入后通过标准 WebDriver `property/value` 精确读回，诊断按钮必须由原生 `enabled` 确认为 true 才点击；任一前置等待超时即失败。诊断只记录输入是否相符、类型/长度边界和按钮启用布尔值，不转储任意输入，便于区分输入状态就绪问题与 IPC/结果问题。

Failure classification also records a whitelisted Python exception class and the input substep (`locate_input`, `clear_input`, `send_keys`, `send_keys_returned`). The last response is summarized only by fixed operation, HTTP status, empty/size flags and JSON root/value types, never arbitrary keys or response data; this snapshot is saved before failure DOM reads overwrite the client's latest operation. Empty/non-JSON bodies remain failures, not successful clear/send operations.

Protocol review: WebKit's official `Source/WebDriver/WebDriverService.cpp` implementation of `elementSendKeys` requires the `text` string; the existing request supplies that field (with a legacy `value` array that the implementation does not reject). Responses are wrapped in a JSON `value` field; a successful clear can return `value: null`, not an empty HTTP body. The diagnostic increment does not speculate by changing key input or sandbox settings before the concrete exception is known.

失败补充白名单 Python 异常类型和输入子步骤；最近响应仅记录固定操作/HTTP 状态、空值及大小标记、JSON 结构类型，不记录原始字段或内容，且在诊断读页面前保存快照。空响应/非 JSON 仍失败。已核对官方 WebKit SendKeys 必须提供 text，当前请求符合，成功清空返回 value:null 而非空正文；没有在明确异常前猜改输入或 sandbox。

### Confirmed startup blocker and product fix / 已证启动阻塞及产品修复

Re-reading the complete failure source identified `进行中：注册表更新` and the registry tab's active-operation label. The input path had reached the native DOM correctly; the Qoder button was legitimately disabled by another workspace's pending registry check. This is not evidence of a React input-event defect or a sandbox failure. The update component previously mounted in `checking` state and automatically invoked `check_updates`. The backend fetches manifest and signature sequentially with existing 60-second HTTP request timeouts; a slow check can therefore outlast the harness's 20-second readiness wait. It is not an unbounded network call.

The product now starts registry updates in an explicit **not checked** state. Mounting or showing the workspace does not start network checks or claim the registry is current. The user can click Check updates when wanted. Once explicitly started, check/apply operations retain their existing busy and duplicate guards until their real IPC promises settle, including when the workspace is hidden. No Promise race, fake cancellation, shortened write lifetime, or removal of cross-workspace busy coordination is introduced. Existing signature and payload verification is unchanged.

Real GUI acceptance now asserts the registry tab is idle at startup before touching caches, while retaining exact path readback and the enabled-button gate. No Tab workaround or 150-second blind wait was added. Offline component regressions cover startup/rerender no network, not-checked messaging, explicit checks, hidden pending check/apply ownership, errors and duplicate guards. The next real CI run must verify this product fix.

完整失败源码明确显示“进行中：注册表更新”，输入路径已正确进入原生 DOM，Qoder 按钮因另一工作区正在检查注册表而合法禁用，并非已证明 React 输入事件或 sandbox 出错。原组件挂载即自动检查；manifest 与签名串行 HTTP 请求各已有 60 秒超时，可能超过脚本 20 秒等待，不能称为无限挂死。

产品改为启动“尚未检查”，挂载/展示不自动联网，也不声称最新；需要时由用户显式检查。一旦开始，检查/应用仍保持真实忙碌及重复操作防护，隐藏工作区不释放，直到 IPC 真正结算。没有假取消、Promise 超时抢先解锁、缩短写入寿命或绕跨工作区忙碌协调；签名和载荷校验不变。真实 GUI 新增注册表初始空闲断言，并保留精确路径/启用门禁，没有 Tab 猜测修复或盲等 150 秒。效果待下一次 CI 实跑确认。

### Native proxy transport compatibility / 原生代理传输兼容

After the startup fix, one real Linux run completed all three state assertions and zero-write checks, then failed before receiving a response to the final clear click. The proxy logged a native `SendRequest` connection reset (errno 104). The failed click was not replayed, and the job remained failed; partial checks are not complete acceptance.

Review of official `tauri-driver` 2.1.0 `src/server.rs` shows a shared hyper upstream connection pool, `retry_canceled_requests(false)`, and forwarding of request headers without removing `Connection`. Python's `urllib.request.AbstractHTTPHandler.do_open` unconditionally adds `Connection: close` and closes its socket after each request. That interaction is a plausible connection-lifetime compatibility cause, not a proven diagnosis of every reset.

The harness now uses one standard `http.client.HTTPConnection('127.0.0.1', 4444, timeout=30)`, sends an explicit Content-Length without `Connection: close`, fully consumes each bounded response before reuse, and rejects all 3xx, non-2xx, malformed JSON/schema and over-limit responses. It never follows redirects or discovers a proxy. Any failure closes the connection and propagates without replaying the unknown command. Explicit later read-only diagnostics may establish a fresh connection but cannot erase the original failure. Cleanup closes the connection after attempting session deletion, even when deletion fails. No Tauri/WebKit source, key-event payload, product gate, Cargo dependency or sandbox setting is changed.

Offline tests use a real local HTTP/1.1 server to verify two requests share the same TCP connection, no close header is sent, and Content-Length matches. A disconnected click is sent exactly once; response-size, JSON and redirect failures close transport. The next real CI run must confirm compatibility; tests do not manufacture GUI success.

启动修复后的一次真实运行已完成三态点击/IPC/零写，但最终清除点击在收到响应前遭原生代理连接重置，仍然失败，未重放未知结果的点击。官方 tauri-driver 2.1.0 使用共享 hyper 上游连接池、禁用取消请求自动重试，却原样转发 Connection 头；Python urllib 每次强制 Connection: close 并关闭连接，存在连接生命周期兼容嫌疑，但不能断言全部 reset 根因。

改用固定 loopback 的标准 http.client 单持久连接、明确 Content-Length，不发送 close，完整读取有界响应后复用。所有跳转/非成功/异常 JSON/超限均关连接并失败，不自动重放任何未知命令；之后只读诊断可新建连接但保留原失败。正常清理也关闭连接。离线真实 HTTP/1.1 服务测试验证复用与头信息，断连点击恰好发送一次；未改 driver、按键、产品门禁、Cargo 或 sandbox，实际兼容效果仍待 CI。

## Complete official cache extension / 完整官方缓存扩展

The Linux hosted x86_64 job now downloads the same reviewed 1.1.85 baseline archive used by Windows read-only acceptance: 78,587,775 bytes, SHA256 `a2fd8cc04711603f9637f54b63097380efc79939739967985cd6193507472dc4`. The downloader retains its fixed official URL, no redirects, streaming size/hash bounds, 180-second subprocess deadline, new-file-only creation and private RUNNER_TEMP destination. The existing Windows-hosted guard is unchanged; the additional branch requires hosted Linux **and actual Linux x86_64**, not a general-purpose bypass. Download failure blocks the job.

CI builds `aisync` from the same exact checkout using `--locked`. Under the GUI harness's new temporary HOME/XDG, it invokes only `qoder stage-package --target linux-amd64-baseline ... --yes` and `qoder unpack-cache ... --yes`, each with a 120-second subprocess deadline and bounded output. Destinations are new private directories, never reused. Nonzero exit, timeout, malformed/incorrect report or invalid member set fails before starting the GUI; there is no fallback to only synthetic states. Reports are strictly checked for fixed fields/types, non-installation and non-execution flags. Core stage/unpack independently check archive/payload content and retain partial failures. No Qoder payload is executed or given executable permissions.

Actual stage/unpack member sets, directory 0700/file 0600 modes, ownership, regular single links, fixed archive and payload sizes/hashes are checked before GUI interaction. The payload is 182,543,840 bytes with SHA256 `8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b`. Snapshot hashing is streamed in 64 KiB blocks through no-follow handles with before/after metadata/identity checks. Only the three fixed large artifact paths get their exact reviewed size/hash contracts; other records retain a 64 KiB ceiling. No blanket unbounded read was introduced.

The actual WebView then inspects all five paths: the three original synthetic states plus the genuine staged and unpacked caches. Complete states must display their exact local label and “retain cache, take no action”, alongside the no-deletion-authorization boundary. Input-change invalidation, enabled gating, final Clear and before/after member/hash/metadata checks remain mandatory. Complete-result waits are bounded at 90 seconds to accommodate debug Rust hashing of large artifacts; this does not fake completion or release busy early. The existing eight-minute GUI job-step budget is unchanged. Evidence contains fixed hashes/sizes/required modes and observed assertion booleans, never archives, payloads, fixture files or HOME contents.

This extension is not a runtime or installer test for Qoder. Local real CLI/snapshot checks are separate from actual GUI success; `complete_cache_gui_verified` is true only after all five GUI assertions and cleanup finish successfully. The earlier three-state scope descriptions above describe the original gate and are extended, not replaced or weakened, by this section.

Linux hosted x86_64 CI 新增下载同一固定官方包，Windows 原 guard 不变；固定 URL/大小/SHA、无跳转、流式上限/180 秒总时限及新文件私有目录规则保持。Linux 分支同时核实际 Linux x86_64。任意下载失败直接阻断。

同 checkout `--locked` 编译 CLI，在临时 HOME/XDG 中对两个全新目录真实运行 stage/unpack，各限时 120 秒且输出有界。必须验证退出码、严格报告、成员集、owner/链接/0700/0600、固定归档与载荷摘要后才启动 GUI；不能伪造 complete 记录或在失败时退化为仅三态通过。Qoder 不执行，载荷不赋执行位。大文件采用 64 KiB 流式 no-follow 读取与前后身份/metadata 核验，仅固定三条大文件路径按精确大小/SHA 放宽，其他记录仍限 64 KiB。

真实 WebView 保留原三态并追加完整暂存/解包两态，必须显示精确文案、“保留缓存，不执行操作”和不授权删除；换路径失效、按钮启用、清除与完整零写检查全部保留。完整态结果等待上限 90 秒只为有界真实哈希留余量，不伪造完成。证据只含固定摘要/大小/要求权限和断言布尔，不上传包、载荷、缓存或 HOME。五态及清理全部完成后才标记完整 GUI 已验证，本地 CLI/snapshot 测试不能替代 GUI，也不证明 Qoder 运行兼容。

## Explicit desktop cache writes / 桌面显式缓存写入

The native Qoder workspace now offers **stage archive** and **unpack cache** for the fixed 1.1.85 `linux-amd64-baseline` contract, only on Linux x86_64. These are cache operations, not installation, activation, execution or PATH/configuration changes. A preview shows the selected operation, fixed target/version, source and new destination. Changing inputs discards confirmation. Only the explicit confirmation button invokes the native write command, whose bounded absolute paths, operation/target and `confirmed` flag are checked before filesystem work. Other hosts fail closed.

A native Qoder-only `try_lock` lives inside the blocking worker until filesystem work finishes, even if its async caller disappears. This is not an application-wide atomic lock. Inputs/clear/cancel are unavailable during a write; the UI's separate busy owner survives component unmount until the IPC promise settles. No failed write is automatically retried, overwritten or cleaned up. Failure or uncertain completion can leave private partial output, requiring manual inspection and a new destination rather than in-place retry. Same-user hostile races, filesystem I/O deadlines and crash recovery remain core limitations; successful records do not grant execution or deletion authority.

Linux x86_64 原生 Qoder 工作区新增固定 1.1.85 `linux-amd64-baseline` 的“暂存归档”和“解包缓存”。这不是安装、激活或执行，不改 PATH/配置。预览明确显示操作、固定目标/版本、来源与全新目标目录；更改输入即废弃确认。只有显式确认才调用写入 IPC，后端在访问文件前检查确认标志、固定操作/目标与有界绝对路径；其他主机失败关闭。

原生锁仅协调 Qoder 缓存写入，位于实际阻塞任务内部，调用方消失也不会提前释放，不代表整个应用的原子锁。写入中不可更改输入、清除或取消；独立忙碌标记在组件卸载后仍保留至 IPC 真实结算。不自动重试、覆盖或清理失败结果。失败或完成状态不明时可能残留私有部分文件，应保留现场人工检查，不能原地重试。同用户恶意竞争、文件系统 I/O 时限和崩溃恢复仍有核心边界；记录不构成执行或删除授权。

## Read-only guidance after a submitted write / 提交写入后的只读定位与指引

The UI keeps the most recent actually confirmed submission's operation, source, destination and outcome in component memory. Later form edits do not relabel that record. Completion is historical, not a fresh assertion about the directory; an unconfirmed outcome can mean no directory or partial output. Nothing is persisted to disk by this UI record, and global Clear or unmount removes it.

“Fill the last submitted destination into read-only diagnosis” only copies that captured destination into the diagnosis field. It does not call IPC, retry a write, clean up, or automatically diagnose. The user must separately invoke diagnosis. Every result displays the exact directory captured at that request; input changes, cancellation, Clear and unmount invalidate pending or displayed observations. Missing/unreadable/unsupported diagnosis yields no integrity, emptiness, deletion or cleanup conclusion.

All five recovery hints are fixed local text. A verified staged cache may be chosen manually as a later unpack source with a **different new destination**, fresh confirmation and complete core revalidation. An unpacked cache is retained only and is not offered as a staged-source substitute. Incomplete, conflicting and unknown-ownership directories require preservation and manual review, never automatic deletion, repair, reuse or in-place retry. No new backend capability, execution permission or automatic operation is introduced.

界面仅在组件内存保存最近一次实际确认提交的操作、来源、目标与结算状态；之后修改表单不会篡改该记录。曾报告完成不代表目录当前状态；未确认完成可能意味着目录不存在，也可能残留部分文件。记录本身不写磁盘，通用清除或组件卸载会移除记录。

“将上次提交目标填入只读诊断”仅填写捕获的原目标，不调用 IPC、不重试、不清理、不自动诊断。用户仍须另行点击只读诊断。每份结果显示请求时的精确目录；修改输入、取消、清除或卸载都会废弃迟到响应及旧结论。目录缺失、不可读或平台不支持时，没有完整性、空目录、安全删除或已清理结论。

五种恢复提示均为本地固定文案：完整暂存目录可以手动选择为后续解包来源，但必须另选全新目标、重新确认并经核心完整复验；解包缓存仅保留，不能作为暂存来源替代。不完整、冲突或归属未知时保留现场人工排查，不自动删除、修复、复用或原地重试。本增量没有新增后端能力、执行权限或自动操作。
