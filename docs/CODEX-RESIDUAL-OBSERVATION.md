# Selected Codex locations / 显式选定 Codex 位置检查

This is a bounded metadata observation of fixed Codex npm locations under an
explicitly selected Windows prefix. It is not an installation scan, package
verification, provenance statement, ownership receipt or recovery operation.
It never runs npm, Node or Codex, reads package contents, recursively enumerates
folders, writes configuration, retries installation or removes files.

A missing location is not permission to install. A present directory or launcher
is not proof of a complete installation, its origin or who owns it. A blocked or
unavailable observation must not be treated as absence. Installation still uses
its separate preview, explicit confirmation and fresh preflight.

The scope is the package directory `node_modules/@openai/codex` and the fixed
launchers `codex`, `codex.cmd` and `codex.ps1`. No sibling package names or file
contents are returned. Windows absolute paths on a local fixed drive are required. Reparse
points, unsafe ancestors and unsupported path forms are refused; other operating
systems do not gain this capability by analogy.

Results describe observations during this call, not an atomic filesystem
snapshot or a durable fact. They cannot authorize cleanup, reinstall, upgrade or
uninstall. The session's historical reviewed targets remain historical. This
CLI/core slice does not add a desktop scan button or persistent recovery state.

## 中文

这是一项 Windows 固定位置元数据观察，只检查显式选定 npm prefix 下的 Codex
包目录和三个启动文件位置。它不是全盘扫描、完整安装核验、来源认证、所有权证明
或恢复操作。不会运行 npm、Node 或 Codex，不读取包内容、不递归列出文件，
不修改配置、不重试安装、不删除残留。

位置缺失不代表允许安装；目录或启动文件存在不代表安装完整，也不能证明由谁安装。
不可检查或被拒绝的路径不能当成缺失。安装仍必须通过独立的预览、明确确认和重新检查。

结果仅表示本次调用期间的观察，不是原子文件系统快照，也不能授权清理、重装、升级
或卸载。会话中的已审核目标仍是历史快照。本批只有 CLI/核心能力，没有新增桌面扫描
入口或持久恢复记录。测试使用临时合成文件，不代表运行过官方 Agent。

## Command / 命令

On Windows, explicitly select an existing local prefix:

```text
aisync codex-residual --prefix "C:\Users\Example\AppData\Roaming\npm"
```

The JSON contains four fixed slot identifiers and `missing`, `file`, `directory`
or `blocked` states. Global path/access/change failures return a nonzero status
with a fixed message, not a raw filesystem error. No path is discovered from
PATH, npm configuration or a session token. The caller must choose the prefix;
this command does not recover it from an earlier failed request.

Windows 上须明确传入已存在的本地固定盘 prefix。JSON 仅包含四个固定位置标识及状态。
整体路径、访问或变化错误以非零退出码和固定消息返回，不回传原始系统错误。
不会从 PATH、npm 配置或会话令牌发现位置，也不会自动恢复旧请求的路径。

During inspection, read-capable handles, used only for metadata queries, retain existing ancestors and selected
entries without sharing write/delete access. An already open incompatible writer
can make the check unavailable; inspection can briefly prevent competing writes
or renames. This is not a background watcher. Directory identity/type/attributes
are checked rather than treating unrelated sibling activity as a global snapshot.

检查期间保留只查询属性但带读取访问权的句柄，不共享写入和删除权限。已有不兼容写句柄可能令检查
不可用；检查也可能短暂阻止竞争写入或重命名。它不是后台监控器，也不把兄弟文件的
活动当作全局快照。

The handle read-access bit is required for Windows write-sharing exclusion; no file-content read API is called. Access-denied results stay unavailable, not missing.
Windows 的写共享排斥需要句柄的读取访问位，但实现不调用文件内容读取 API。访问被拒绝仍表示不可检查，不表示缺失。

## Desktop explicit inspection / 桌面显式只读检查

Windows 桌面在本会话 Codex 安装请求失败（完成状态未确认）后，展开安装记录可点击「只读检查固定残留槽位」。仅后端仍保留对应已审核目标时可用；没有自动检查、恢复、清理、重试或卸载。四个槽位仅返回 missing / file / directory / blocked，未执行 npm 或 Agent、未读取文件内容。结果不证明安装成功、真实性、归属或清理权限。检查仍受上文固定本地盘、句柄权限和非原子观察等限制。

后端仅在内存保存最近预览的 prefix、revision 与独立 UUID 引用；只有失败的安装请求匹配该已审核引用和 revision，才获得检查资格；引用不改变原有安装审核条件，缺失引用的请求仍须经过 core 安装检查。IPC 只接受引用，不接受前端路径。新的预览或安装使旧引用失效，成功与重启清除资格。后端互斥门禁防止此桌面入口的预览、安装和检查同时执行；其他进程仍可能改变磁盘。缓存的规范 DOS verbatim 路径仅在后端转换为对应盘符路径，再经 core 全部路径检查；UNC 和其他设备命名空间不转换。

结果仅存在当前面板内存。关闭结果、折叠记录、导航离开、切换安装会话或开始新操作会丢弃结果与迟到响应。关闭不会取消已开始的文件系统元数据查询，也不承诺 I/O 截止时间。无日志或持久化路径、引用或检查结果。浏览器演示不合成这项真实检查。

On Windows desktop, expand an unconfirmed Codex installation record and explicitly choose “只读检查固定残留槽位”. The backend must still hold that reviewed target. No automatic inspection, recovery, cleanup, retry or uninstall occurs. Only four fixed slot kinds are returned; npm and agents are not executed, and file contents are not read. These observations do not establish installation success, authenticity, ownership or permission to clean up.

The backend keeps only the latest preview prefix, revision and a separate UUID reference in process memory. Inspection eligibility requires the failed installation request to match the reviewed reference and revision. The reference does not change installation authorization; requests without it still undergo the existing core installation checks. IPC accepts the reference, never a frontend path. A new preview/installation invalidates old references; success and restart remove eligibility. A nonblocking mutex excludes overlapping desktop preview, installation and inspection operations, not changes by other processes. Only canonical DOS disk verbatim paths are converted on the backend before all core path checks; UNC and other device namespaces are not converted.

Results live only in the open panel. Closing, collapsing, navigating away, changing session or starting another operation discards results and late responses. Closing does not cancel already-started filesystem metadata queries or impose an I/O deadline. Paths, references and reports are not logged or persisted. Browser demo mode does not fabricate an inspection. Existing fixed-local-disk, permission and non-atomic-sampling limitations still apply. Synthetic UI/fixture tests are not a real Codex installation acceptance test.
