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

During inspection, metadata-only handles retain existing ancestors and selected
entries without sharing write/delete access. An already open incompatible writer
can make the check unavailable; inspection can briefly prevent competing writes
or renames. This is not a background watcher. Directory identity/type/attributes
are checked rather than treating unrelated sibling activity as a global snapshot.

检查期间保留仅访问属性的句柄，不共享写入和删除权限。已有不兼容写句柄可能令检查
不可用；检查也可能短暂阻止竞争写入或重命名。它不是后台监控器，也不把兄弟文件的
活动当作全局快照。
