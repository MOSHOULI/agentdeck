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

Windows installed-app acceptance clicks the real workspace, selects an explicit
plan and checks rejection of a deliberately invalid synthetic archive with
unchanged file bytes/count. It does not download a real Qoder archive or establish
successful hashing on Windows. Frontend mocked success tests and native synthetic
failure tests are not substitutes for real package/platform acceptance.

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
校验成功。Windows安装UI验收只证明真实计划与合成错误包拒绝/零写，不证明真实包
在Windows校验成功；mock成功测试不能冒充实际平台验收。
