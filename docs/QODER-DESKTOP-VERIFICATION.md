# Qoder desktop plan and package verification / Qoder 桌面分发计划与包校验

The “Qoder 分发与校验” workspace exposes only two native IPC commands:
`qoder_distribution_plan` and `verify_qoder_package`. Both reuse the fixed core
[1.1.85 distribution contract](QODER-DISTRIBUTION.md). Choose one of eleven exact
target IDs explicitly; no host guessing or first-match variant selection occurs.
Changing inputs, clearing, cancelling results or unmounting invalidates pending
responses. Cancellation discards results, not a running filesystem read. Busy
coordination remains until that read settles; duplicate work is not dispatched.

The plan shows the fixed filename, version, public package URL and SHA256/manifest hash (plain text, no automatic download), runtime/variant and local fixed CPU,
libc or Windows-build conditions. Backend version, target, URL, SHA256, manifest
hash, filename, runtime, variant, build and capability fields are strictly checked
against the reviewed contract before rendering. Arbitrary requirements, paths,
errors and backend prose are not rendered. Verification additionally requires the
fixed success state and a positive safe-integer byte count <=256 MiB. Failed,
unsupported or incompatible responses never appear as successful verification.
Input paths necessarily remain in their user-editable field, not echoed in reports.

Native verification reads only the explicitly selected local archive, enforcing
core basename, bounded streaming and unsafe-file checks. Audited readers support
Windows and Linux x86_64/ARM64. A macOS distribution plan does NOT mean file
verification is available on a macOS host: other reader platforms fail closed.
Windows ARM64 remains a documentation/manifest contradiction, not a compatibility
claim. A digest match is not installation, execution, independent signature
verification, runtime compatibility or authorization to run. Reads may change
access time and are not atomic against same-user modification.

No download, staging, extraction, execution, installation, PATH/configuration
change or login is exposed in the desktop workflow. The separate controlled
staging/unpacking CLI capabilities are limited to Linux x86_64 and the pinned
baseline archive, and also do not install or execute. Browser/demo mode does not
invoke either command or simulate a successful package check.

Windows installed-app acceptance uses a separate CI-only downloader to obtain the
fixed official Linux baseline archive, then clicks the real installed UI to check
successful SHA256 verification and exact byte count. It asserts unchanged archive
hash, write time and directory file count, invalidation after target/path changes,
and rejection of a same-name, same-size copy with one byte changed. The existing
synthetic-invalid archive test also remains. No Qoder archive is unpacked/executed;
this validates Windows desktop read/hash behavior, **not Windows-native Qoder runtime
compatibility**. Until the exact Windows CI run passes, local mock tests and script
parsing are not proof that this installed-app flow succeeded.

## 中文

桌面新增“Qoder 分发与校验”，只接入固定 1.1.85 分发计划和显式本地归档校验两个
只读 IPC。11个目标必须明确选择，不探测平台、不按第一条猜变体。结果展示固定包名、官方URL与包/清单SHA256（纯文本，不自动下载）、
runtime/variant与CPU/libc/Windows build条件；先严格核目标、版本、官方URL/摘要、
manifest摘要、包名与权限字段，只投影固定说明，不渲染动态路径、错误或后端任意文本。
成功校验还须固定success状态和正安全整数字节数（最多256MiB）。输入框保留用户输入，
结果不重复泄露该路径。

更改输入、清除、取消或卸载界面丢弃迟到响应。取消不是中止文件读取，底层完成前
仍busy且不重复发起。只读验证沿用安全流式reader；审阅主机仅Windows与Linux
x86_64/ARM64，macOS可看计划不等于支持本机文件校验。ARM64 Windows文档矛盾
仍明确提示。摘要匹配不代表安装、运行、独立签名、兼容或执行授权。

桌面不提供下载、暂存、解包、执行、安装、PATH/配置修改或登录。独立CLI暂存/解包
只适用于Linux x86_64固定baseline，仍非安装。浏览器与demo模式不读取归档、不模拟
校验成功。Windows安装UI验收另用CI专用下载器获取固定官方Linux归档，真实点击验证摘要和精确字节数，
核原包hash/写入时间/文件数不变，切换目标/路径失效，并拒绝单字节损坏的同名同大小副本。
原合成错误包门禁保留，不解包、不运行Qoder；这是Windows桌面读取Linux归档的验收，
不是Windows原生Qoder运行兼容性。精确CI未通过前，不以mock或脚本语法检查冒充完成。

### Explicit target selection / 明确目标选择

All 11 fixed targets are individually labelled native radio controls in a responsive grid, with no default or automatic host selection. They are disabled in demo/unavailable mode. Changing a target invalidates results, including pending results; Clear deselects all targets. Windows installed acceptance selects the exact RadioButton through UI Automation SelectionItem and checks both its selected state and the displayed target, without keyboard-position guesses or direct IPC.

11 个固定目标使用带完整名称的原生单选项，紧凑网格随宽度换行，不默认选择、不自动检测主机；演示或后端不可用时禁用。改目标使已有及迟到结果失效，清除后全部取消选择。Windows 安装版验收按精确名称通过 UI Automation SelectionItem 选择 RadioButton，同时核对选中状态与显示目标，不猜键盘位置、不直接调用 IPC。

The Qoder desktop workspace now exposes [read-only cache inspection](QODER-CACHE-INSPECTION.md) on supported Linux x86_64 hosts, with explicit unsupported-host handling elsewhere. No desktop cleanup/activation. 桌面新增[缓存只读诊断](QODER-CACHE-INSPECTION.md)，仅 Linux x86_64 后端可用，其他主机明确拒绝；不开放清理或激活。

### Pending Linux final-clear diagnosis / Linux 最终清除待诊断

PR #38 first Linux GUI run recorded all five read-only states and all five
desktop write checks, including genuine pinned-archive staging, unpacking, and
existing-destination rejection. The final top-level Clear click returned W3C
HTTP 400; the run remains failed, not complete acceptance. Its retained response
shape did not identify the W3C error, so a scrolling, obstruction, stale-element,
or other cause is **not established**.

The harness now retains only a fixed allowlist of W3C error names, never raw
error messages or stack traces. Failure diagnostics read Clear enabled/displayed
booleans and bounded finite element/window geometry through standard read-only
WebDriver endpoints. Final Clear waits for enabled before its single click.
No failed click is replayed, no JavaScript click/scroll is injected, and every
original acceptance gate remains required. A fresh isolated real GUI run is
needed to establish the cause and validate any eventual fix.

首轮已记录五种只读状态及五项写入检查，但最终顶部清除按钮返回 HTTP 400，
因此整体验收仍失败。滚动、遮挡或元素失效均未证实。新增诊断仅保留白名单
错误类别、严格布尔值和有限范围几何，不保留原始错误消息；不重放失败点击，
不注入 JS，继续保留全部门禁，待新一轮真实 GUI 结果。

### Keyboard scroll candidate / 真实键盘滚动候选

The diagnostic run established `element not interactable` for final Clear while
its enabled/displayed values were true. This does not establish the exact layout
cause. The next isolated run uses standard element Send Keys on the existing
focusable `main#workspace-content` (`tabIndex=-1`): NULL first, verify the active
WebDriver element is that main, then Control + Home + NULL to release modifiers.
A read-only `scrollTop` poll must reach zero; all source/destination/cache input
values must remain unchanged. Only then is the original Clear clicked once and
all original clear/zero-write assertions run. No coordinate click, script
injection, alternate Clear, or failed-click retry is used.

Protocol basis (not proof of this runner's outcome):
- [W3C Element Send Keys and keyboard codes](https://w3c.github.io/webdriver/#element-send-keys)
- [Official WebKit Session.cpp](https://github.com/WebKit/WebKit/blob/main/Source/WebDriver/Session.cpp): `elementSendKeys` focuses the target before dispatch and rejects a target that cannot receive focus.
- [Official WebKit GTK keyboard mapping](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/Automation/gtk/WebAutomationSessionGtk.cpp): Control and Home map to GTK keyboard events.

诊断已确认最终按钮为 `element not interactable`，尚不能确定底层布局原因。
候选方案使用标准键盘输入聚焦原滚动容器，核验焦点后发送 Ctrl+Home 并释放修饰键；
等待滚动位置归零且输入未变，再点击原清除按钮。离线协议测试不是此 GTK/WebView
真实运行成功证明，仍须完整真实 GUI 门禁通过。

### Bounded wheel candidate / 有界真实滚轮候选

The keyboard candidate completed focus/Send Keys but timed out because the
observed main scrollTop did not reach zero. It is not retained as a fallback
or rerun unchanged. The replacement sends W3C wheel actions with the real main
element as origin and zero center offsets, deltaY=-600 and duration=200 ms.
At most 12 successful steps are allowed; each must show strictly decreasing,
finite bounded scrollTop within three seconds before another step. Zero is
required before the unchanged original Clear click. A failed wheel is never
replayed. DELETE /actions only releases input state; cleanup cannot convert
failure to success or replace the original failure evidence.

Official upstream support evidence, inspected 2026-11-09 (main branch, not a
claim about the exact runner binary):
- [OptionsGTK.cmake](https://github.com/WebKit/WebKit/blob/main/Source/cmake/OptionsGTK.cmake) enables WEBDRIVER_WHEEL_INTERACTIONS.
- [Session.cpp](https://github.com/WebKit/WebKit/blob/main/Source/WebDriver/Session.cpp) forwards Wheel/Scroll with Element origin and node handle.
- [WebAutomationSessionGtk.cpp](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/Automation/gtk/WebAutomationSessionGtk.cpp) maps wheel interaction to GTK wheel events.

键盘方案已真实失败：焦点与发送协议通过，但滚动未归零。改为官方协议滚轮，
元素中心零偏移不是猜测屏幕坐标；每步必须下降、最多 12 步，异常不重放。
上游源码支持不等于当前 runner 成功，仍需完整真实 GUI 验收。

### Recovery-guidance scroll diagnostics / 恢复指引验收滚动诊断

The first #39 Linux run (42654567086) completed the new immutable-attempt,
recovery-guidance, and zero-write checks for both staged and unpacked caches.
Final Clear was not reached: a successful numeric scrollTop read failed the
bounded wheel-progress wait. The retained evidence does not reveal the numeric
value, step, or nested scroll state; it cannot establish fractional residue or
nested scrolling as the cause. The whole acceptance remains failed.

Diagnostic-only additions retain the initial scrollTop and at most 12 wheel
steps (index, before/last bounded numeric values, bounded poll count, posted and
progressed booleans). Failure-only reads capture scrollTop/scrollHeight/clientHeight
for five fixed known elements: workspace, body, and the three path inputs.
Values outside [0, 1,000,000], non-numbers, booleans, NaN and infinities are
rejected; no arbitrary property strings, paths, or backend messages are retained.
These observations cannot by themselves identify which descendant received a
wheel event. No scroll/click is replayed, no zero/progress tolerance is added,
and all original gates remain unchanged pending fresh diagnostic evidence.

首轮 #39 新增恢复检查已通过，但最终清除前滚轮进度等待失败；未记录数值，
不能断言是小数残留或嵌套滚动。此次仅增加有界数值和固定元素只读诊断，
不放宽归零或进度门禁，不重放输入，整体真实验收仍待通过。
