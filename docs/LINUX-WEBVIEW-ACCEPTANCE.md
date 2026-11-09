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

Python stdlib speaks only the local WebDriver protocol, with proxy discovery disabled and every HTTP redirect rejected before following; no extra client library or product test hook is needed. Requests have 30-second socket timeouts, wait loops have deadlines, responses are capped at 1 MiB, and the real UI step has an eight-minute workflow deadline. Only the process group started by the harness is stopped on completion. Offline harness tests check command construction, no script endpoint, fixture boundaries, guards, response/log limits; **they are not GUI acceptance**.

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
