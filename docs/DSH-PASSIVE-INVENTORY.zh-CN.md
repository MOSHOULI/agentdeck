# DSH 被动盘点：首个实现阶段

[English](DSH-PASSIVE-INVENTORY.md) | 简体中文

本次增量基于 PR #2 的安装证据工作，基准提交为
`7f43da1f654307241eee1558c920a79f80b1dd69`。它新增核心读取器和独立 CLI
入口，不会将 DSH 加入通用部署注册表，不提供桌面管理页面，也不开放安装、配置、
更新、降级、移除、认证或运行时连接。

这是源码分支中的功能，并不表示版本字符串相同的已发布安装包也包含该功能。
判断安装包是否含有此命令前，请将其 `BUILD_COMMIT.txt` 与源码修订版本核对。
盘点合成或复制的 manifest，不需要安装或启动 DSH 本身。

本阶段通过从源码构建的 `aisync` CLI 提供功能，没有桌面按钮。安装 NSIS 桌面包本身
不会安装该 CLI 命令。在本仓库中满足文档所列 Rust 前提条件后，例如可以运行：

```powershell
cargo run --locked -p aisync-cli -- dsh inspect --profile-dir "C:\DSH\profiles\web"
# 可选：单独选择 runtime manifest；不自动发现或执行 runtime
cargo run --locked -p aisync-cli -- dsh inspect --profile-dir "C:\DSH\profiles\web" --runtime-manifest "C:\DSH\runtime\package.json"
```

请将示例路径替换为你明确选择的已有目录。路径缺失时会生成报告，不会创建路径。

## 显式输入与静态证据

使用 `aisync dsh inspect --profile-dir ABSOLUTE_DIRECTORY`。可选参数
`--runtime-manifest ABSOLUTE_PACKAGE_JSON` 用于显式选择第二份 manifest。
没有默认 profile，不扫描 HOME，不探测 PATH，不自动发现 runtime，也不创建目录。
不接受相对路径或包含父目录分量的路径。这个范围受限的本地文件接口不接受
Windows UNC 路径或设备路径。

唯一允许的输入是所选 profile 的 `package.json`，以及显式选择的 runtime
`package.json`。读取器不会打开 `cordis.yml`、凭据、环境变量文件、npm 配置、
lockfile、插件代码或其他 profile 文件；不会调用 shell、DSH、Node、npm、pnpm、
DSH profile 加载器或运行时 API；也不会导入 JavaScript 或解析 YAML。

报告分别呈现 manifest 缺失、损坏、不可读、不支持和已读取的观察结果。输入与集合
大小均有限制；报告不返回任意 manifest 属性或原始解析错误，也不会原样返回可能
带有凭据的依赖 URL。已保存的依赖声明不能证明对应文件存在，已保存的 bundle
选择也不能证明插件已在运行中的宿主里启用或处于活动状态。运行时状态、实际启用
状态和认证状态仍为未知。声明缺失时，不会用配置默认值填补。

本实现保守地支持上游 manifest 格式的一个子集。名称、结构或 schema 版本不受支持时，
可能拒绝整份 manifest，而不是生成部分盘点结果。拒绝读取不代表对应的软件包或
runtime 不存在。
上游将 bundle 选择定义为字符串；本阶段只返回符合小写 npm 包名形态的名称。
其他字符串在上游可能合法，但不在本实现支持范围内，不能据此认定 DSH 元数据无效。
本实现不会解析路径或导出选择器。

当前限制为两份 manifest 文件、每个文件 65,536 字节、JSON 深度 16、4,096 个 JSON
条目、受支持区段中合计 128 条依赖声明，以及 64 项已保存的 bundle 选择。解码后重复的
JSON 键会被拒绝。只盘点 `dependencies`、`optionalDependencies` 和
`peerDependencies`，不解析开发依赖和包内容。链接、重解析点、多重硬链接和非普通
文件均被保守拒绝。这可能拒绝合法的链接包目录布局，不能将这种拒绝报告为未安装。
原生文件读取仅支持 Windows 和已审查的 Linux x86_64/aarch64 实现路径；其他目标
平台返回不支持。

依赖规格类型仅按文本形态分类，不验证完整的 npm semver 语法，不证明版本范围能够
解析，也不检查插件兼容性。报告中的 Node engine 文本同样是未经验证的声明。

版本识别针对的是 manifest 声明，不是执行命令测得的版本，不是经过验证的发布者
身份，也不能证明某个 runtime 使用了这个 profile。未知或较新的版本不会获得写入
能力。本阶段中，即使版本已被识别，所有修改能力也仍然关闭。

退出码 0 仅表示已生成盘点报告，包括含有缺失或未知证据的报告；不表示已安装、
健康或已认证。无效 CLI 参数会在盘点开始前报错；输出失败也属于错误。

## 固定的上游参考版本

已审查的上游快照为
[`8bd073392eef6f54e921a093b95076248eb08cbb`](https://github.com/deepseek-ai/deepseek-harness/commit/8bd073392eef6f54e921a093b95076248eb08cbb)，
其中 CLI 版本为 `0.2.2-alpha.3`。审查时，上游默认分支为 `master`。
预览版接口可能变化；较新的版本需要重新审查接口约定，不能直接作宽泛的兼容性假设。

- profile manifest 可以只包含部分字段，不一定有包版本。
- `dsh.manifestVersion` 如存在，描述的是 manifest schema，与 runtime/包版本和已保存的会话版本相互独立。
- `dsh.profile.bundles` 是已保存的有序列表，不是运行时状态报告。
- CLI 的版本实现读取 app-boot 包的 manifest。在这里读取包文件仍然只是获得静态声明，不是实测 CLI 版本。

来源：[manifest 类型](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/packages/util/package-manifest/src/types.ts#L7-L84)
和 [profile 初始化](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/packages/boot/app-boot/src/profile.ts#L198-L216)。

## 为什么不调用列表命令？

在已审查的快照中，`dsh plugin list` 可能初始化 profile、创建操作日志并调整 bundle
选择，使其与当前状态一致。配置导出会准备 profile 并重写配置；schema 导出可能导入
插件代码。即使原生插件管理器的列表操作，也可能进入会移除已退役 bundle 的 profile
加载器。这些操作不能替代零写入的被动读取器。本实现不会调用它们。

来源：[插件命令准备](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/packages/boot/plugin-manager/src/operations.ts#L565-L577)、
[profile 准备](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/apps/cli/src/profile-boot.ts#L194-L198)
和 [原生列表操作](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/packages/boot/plugin-manager/src/index.ts#L465-L480)。

## 验证与剩余边界

测试使用合成 manifest 和临时目录，不使用真实 DSH 安装、不执行包管理器，也不使用
用户配置。读取边界测试会拒绝未获准的文件名、格式错误或超大输入，以及不安全的
路径类型。测试检查只请求读取允许的 manifest，并确认文件内容和修改时间保持不变。
生产读取器不包含进程执行器或写入器。CLI 测试在 PATH 中放置替身哨兵程序，检测意外的
DSH/Node/包管理器调用；这不等于覆盖所有系统调用的跟踪，也不是操作系统沙箱。
已有真实环境测试仍保持忽略。

Windows 特有的路径、重解析点和句柄检查，需要在原生 Windows 上验证；可移植的
Linux 测试通过不代表这些检查已经运行。本阶段不声称完成 Windows GUI 测试或真实
DSH 集成测试。

文件系统验证采取保守策略，但不是用于防御同权限进程并发替换文件的操作系统沙箱，
也不保证每次文件系统操作都有超时。读取普通文件可能更新文件系统的访问元数据。
本实现不请求写入配置或文件内容。

桌面 profile 归属、已认证的运行时传输、插件实际激活、凭据处理和修改计划，均属于
后续独立工作。静态 manifest 不能授权这些操作。本功能没有修改或解决现有 glib
安全通告涉及的问题。
