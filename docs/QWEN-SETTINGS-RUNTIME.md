# Real Qwen settings snapshot test / 真实 Qwen 设置快照测试

## English

**Status: implemented, but runtime assertions have NOT passed in the dot cloud.**
The namespace preflight is denied by the kernel before official settings code is
imported. Do not infer settings-loader or interactive Vim acceptance from this
script's existence or from the previous version/help smoke.

Static inspection of the SHA-512-pinned Qwen 0.27.0 distribution identifies
`chunks/chunk-E6R76DS6.js` exporting `readSettingsSnapshot`. It calls the same
`readSettingsLayers` as `loadSettings`, with an explicit environment, no `.env`
loading, no on-disk migrations and explicit workspace trust. This is not an
extracted/reimplemented parser: the test imports the unmodified package chunk.
Its dependency graph is broad and includes network-capable code. Unlike the
small version/help fast paths, it must **not** be run using the weaker diagnostic
mode from [the distribution smoke](QWEN-DISTRIBUTION-SMOKE.md).

```sh
# Requires Linux bubblewrap network namespaces, Node 24+, Python 3.12+,
# the official archive (same pinned hash as distribution smoke), and built aisync:
python scripts/test-qwen-settings-runtime.py \
  --archive /absolute/qwen-code-0.27.0.tgz --aisync /absolute/aisync
# Offline fail-closed checks only:
python scripts/test_qwen_settings_runtime_harness.py
```

The mandatory namespace is tested before package extraction/import. There is no
fallback flag. Execution has an allowlisted synthetic environment and HOME,
explicit synthetic system settings/defaults paths, and a synthetic workspace.
Node permissions permit reading the package, synthetic data and test runner;
no file writes, subprocesses, workers or native addons are granted to the Qwen
snapshot process. Bubblewrap blocks networking. The AgentDeck CLI writes only
its selected synthetic user configuration outside that snapshot process.

Planned assertions first observe the default true value with no user file, then
exercise user-over-default precedence, preview/apply followed
by a fresh official snapshot, trusted-workspace override, untrusted-workspace
exclusion, and field restore followed by another snapshot. Snapshots must leave
synthetic files unchanged. These assertions remain **unverified** until the full
script passes on a supported runner. Even a pass establishes settings-layer
resolution only, not interactive editor key behavior, authentication, MCP, tools,
optional native integrations, model requests or all configuration permutations.

Observed local failure: `bwrap: loopback: Failed to create NETLINK_ROUTE socket:
Operation not permitted`. No security settings were changed to bypass this.

### CI gate

A separate Linux CI job installs bubblewrap from the runner's official apt source,
builds aisync and downloads the fixed official archive. The runner checks its
SHA-512 before extraction and imports only inside the required network namespace.
Namespace denial fails the job; no host sysctl, AppArmor or permission settings
are relaxed. The release job depends on this gate. The ordinary Linux job also
runs the offline fail-closed harness tests. CI results must still be observed
before changing the unverified status above.

## 中文

**当前状态：脚本已实现，但真实设置加载断言尚未通过。** dot 云端内核在网络隔离
预检阶段拒绝命名空间，尚未导入官方设置代码，不能把脚本存在当成运行验证完成。

固定 SHA-512 的官方 0.27.0 包导出 `readSettingsSnapshot`，使用与 `loadSettings`
相同的分层加载器，明确传入合成环境和信任状态，不加载 `.env`，不持久化迁移。
测试直接导入未修改的官方分块，不拼装替代解析器。依赖图含网络能力，因此此
测试强制要求 bubblewrap 网络隔离，**没有弱模式或自动降级**。

只使用临时 HOME、工作区、系统默认和用户设置；不读取真实配置、凭据或登录。
计划先在用户文件不存在时验证默认值 true，再验证用户层覆盖默认、应用后的新进程加载、受信工作区覆盖、不信任层排除、
撤销后的加载，并确认只读快照没有改文件。当前只验证预检失败会停止；完整断言
需要在允许网络命名空间的 runner 上执行。即便通过，也仅证明设置层解析，不
证明交互式 Vim 按键、完整登录、模型、MCP 或工具运行效果。未修改安全设置绕过限制。

独立 Linux CI 任务通过官方 apt 安装 bubblewrap、构建 aisync、下载固定官方包，
校验哈希并强制在网络隔离内验证。隔离失败即任务失败，不调整主机 sysctl、
AppArmor 或权限；发布任务依赖此门禁。普通 Linux 任务也运行离线防降级测试。
目前尚未观察到这项 CI 的运行结果，不能据此改为“已验证”。
