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
