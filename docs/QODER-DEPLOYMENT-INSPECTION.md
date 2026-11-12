# Qoder deployment inspection / Qoder 部署只读检查

## English

The desktop Qoder distribution page now has a separate **read-only private deployment inspection** panel. Select a reviewed release (1.1.85 or 1.1.86), enter an explicit existing deployment directory, then request inspection. It requires the native Linux x86_64 backend. Browser/demo mode neither reads local files nor simulates success.

The fixed states are `payload_verified`, `missing`, `incomplete`, and `unsupported_or_changed`. `incomplete` means required members are missing; it does not establish provenance or validate the remaining contents. A verified result observes the selected fixed payload, deployment records and private metadata, not execution, activation, ownership, signatures or runtime compatibility. No state authorizes deletion, repair, overwriting or launch. Preserve unknown or incomplete output for manual review. This is a current observation, not a durable guarantee against subsequent changes.

Path edits, release changes, clearing, cancellation and leaving the workspace discard the result. They do not stop an in-flight filesystem read. Other workspace operations remain blocked until the request settles; late responses do not revive a discarded result. The native command also shares the existing in-memory Qoder write exclusion guard; it creates no filesystem lock and does not exclude unrelated external writers. Backend prose, paths and unexpected response fields are not presented as results.

Automated component/contract tests cover response validation, invalidation, late replies and concurrency. The Linux WebView acceptance harness additionally checks real IPC against a deployed fixed official payload and an authored conflicting directory, including unchanged file snapshots. Offline tests do not establish real GUI acceptance: the exact-commit CI artifact must pass. Neither harness executes Qoder.

## 中文

桌面 Qoder 分发页面新增独立的“重新检查私有部署（只读）”面板。显式选择已审阅版本 1.1.85 或 1.1.86，输入部署绝对目录，再发起检查。仅支持 Linux x86_64 原生后端；演示模式不读取本地文件、不模拟成功。

结果分为：固定载荷匹配、目录缺失、固定成员不完整、不受支持或已变化。固定成员不完整不代表剩余内容已验证，也不证明该目录曾由部署操作创建。匹配仅表示本次读取中固定版本的载荷、部署记录及私有元数据满足契约，不证明运行、激活、所有权、签名或兼容性。任何状态都不授权删除、修复、覆盖或启动；未知和未完成文件应保留供人工检查。检查不是对未来文件状态的保证。

修改路径或版本、清除、取消、离开页面会使结果作废，但不会停止底层读取。请求实际返回前继续阻止其他工作区操作，迟到响应不会恢复已作废结果。原生入口与 Qoder 写入共用内存互斥锁，不创建文件锁，也不排除外部程序修改。界面只呈现固定标签，不展示后端动态文本或路径。

组件与契约测试覆盖响应校验、作废、迟到响应与并发；Linux WebView 验收脚本通过真实 IPC 检查已部署官方固定载荷及测试构造的冲突目录，并校验文件快照不变。离线通过不能代替精确提交的 GUI CI 证据；两个验收脚本均不执行 Qoder。

## Release boundary / 版本边界

Read-only upstream checks on 2026-11-12 observed Qoder rolling `1.1.87`
(published_at `2026-11-11T12:07:29.788Z`) at
<https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/channels/manifest.json>.
This increment deliberately reinspects only the previously reviewed `1.1.85`
and `1.1.86` payloads; it neither claims they are latest nor adopts a new release
based on a moving manifest. New release hashes, archives and compatibility still
require their own review and acceptance.

2026-11-12 只读查询已发现上游 rolling 为 1.1.87。本批只复核此前审查的
1.1.85/1.1.86 私有部署，不声称最新，不根据滚动清单自动安装或扩大版本支持。
