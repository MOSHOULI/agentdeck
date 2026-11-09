# Official Qwen distribution bootstrap smoke / 官方 Qwen 分发包启动检查

## English

This is a separate opt-in engineering check, not an AgentDeck installation feature.
It downloads and extracts the actual official `@qwen-code/qwen-code@0.27.0`
registry artifact, verifies its pinned SHA-512 before extraction, and invokes its
unmodified `cli-entry.js --version` and `--help` under Node 24+.
No npm lifecycle scripts or optional dependencies are installed. It is **not** a
full `npm install -g` or a complete interactive installation acceptance test.

Sources checked:
- [Pinned official README](https://github.com/QwenLM/qwen-code/blob/6c3d5355de2be05b8d1ab61f4699f52c02c42068/README.md), which identifies the npm package.
- [Versioned registry metadata](https://registry.npmjs.org/@qwen-code%2fqwen-code/0.27.0), repository, bin and distribution integrity.
- SHA-512: `efZt0LUm/raGUjR/SNN6pp8cpuNHnmE0hQvcvXzWHtVX9oVtDDgukq2IZNwYDErFYqHRj1xq2NQ5Jjt2SHUr/Q==`.

The extracted manifest has no required dependencies or lifecycle scripts. Its
optional native/clipboard/audio/Bun packages are intentionally omitted. Review of
the distribution entry shows version/help use bootstrap fast paths, not the LLM
main entry. Hash matching establishes the reviewed bytes, not signature or
provenance-attestation verification.

```sh
# Linux with Node 24+, Python 3.12+ and bubblewrap; fails if namespace unavailable:
python scripts/test-qwen-distribution.py
# Use a pre-downloaded archive, still checked against the pinned hash:
python scripts/test-qwen-distribution.py --archive /absolute/qwen-code-0.27.0.tgz
# Offline harness unit tests (no download/execution of Qwen):
python scripts/test_qwen_distribution_harness.py
```

The default mode requires a bubblewrap network namespace during execution; it
never silently downgrades. Download occurs before the namespace. The root is
read-only in bubblewrap and the temporary synthetic HOME writable. Node permission
flags restrict JS filesystem reads to the extracted installation and synthetic
HOME, writes to HOME, and do not enable subprocesses, workers or native addons.
Only an explicit environment allowlist is passed, without the caller's API keys,
proxy settings, PATH or NODE_OPTIONS. Temporary HOME is also the working directory.
This is defense in depth, **not a general hostile-code sandbox**.

### Observed result and limits

In the dot cloud environment, Node `v24.22.1` returned exact `0.27.0` and expected
help text, both exit 0 with no stderr. The four offline harness tests passed.
The local kernel denies network namespace setup, so this run explicitly used
`--network-isolation none`; it had **no OS-level network isolation**. The default
bubblewrap mode is not claimed validated in this environment. To reproduce that
weaker diagnostic mode, add `--network-isolation none` explicitly. No credentials,
login, model prompt, real user configuration or optional native features are
used. Version/help do not exercise the full settings loader, editor, MCP, tools,
authentication or model traffic. This does not upgrade AgentDeck's discovery
candidates to verified installations and does not validate effective Vim mode.

## 中文

这是单独按需执行的工程检查，不是 AgentDeck 自动安装功能。下载真实官方固定
版本包，先校验 SHA-512，再解包并执行未修改的 `cli-entry.js --version/--help`。
不执行安装脚本，不安装可选原生、剪贴板、音频或 Bun 依赖，不等于完整全局 npm
安装验收。官方来源、哈希和复现命令见上；未验证签名或供应链证明。

子进程仅继承明确列出的最小环境，不继承真实 API Key、代理、PATH、NODE_OPTIONS。
临时 HOME 同时作为工作目录；Node 权限限制文件读写，不开放子进程、worker 或
原生扩展。默认要求 bubblewrap 网络命名空间，失败不会自动降级；下载在隔离前。
这不是可执行敌意代码的通用安全沙箱。

本次 dot 云端 Node 24.22.1 的版本和帮助检查成功，4 项脚本单测通过。但内核禁止
网络命名空间，因此本次明确使用 `--network-isolation none`，**没有操作系统级
网络隔离**，不声称默认隔离模式已通过。没有登录、模型调用或读取用户配置。
版本/帮助属于启动快速路径，不覆盖完整设置加载、交互界面、MCP、工具、认证或
Vim 实际生效，也不会将目录候选自动标记为已验证安装。尚未发布安装包。
