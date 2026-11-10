# Pinned Codex filesystem install / 固定 Codex 官方包安装验收

Development verification only. This does not enable a new adapter or publish an installer.

## English

`scripts/codex_official_install.py` verifies a real npm global installation of the
fixed official `@openai/codex` **0.167.0** package and its Linux x86_64 native
optional package (`@openai/codex@0.167.0-linux-x64`, installed under the alias
`@openai/codex-linux-x64`). This is not the synthetic npm/Codex substitute used by
the separate WebView test.

Before invoking npm, the harness downloads the two fixed official registry
archives, verifies their pinned SHA-512 integrity, and reads a bounded archive
inventory without extraction or execution. It checks the exact package identity,
entry point, six fixed optional platform aliases and absence of required/peer/
bundled dependencies and lifecycle scripts. Native platform identity must match.
The expected inventories are 3 root files (13,570 bytes) and 45 native files
(458,593,658 bytes). Integrity is not an independent publisher signature check.

The trusted preinstalled Node/npm runs once with a fresh private HOME, global
prefix, cache, temporary directory, and empty user/global npm configuration.
The environment does not inherit credentials, provider variables, proxies,
`NODE_OPTIONS` or npm configuration. Both default and `@openai` registry are
fixed to `https://registry.npmjs.org/`; `--ignore-scripts` is mandatory, automatic
fetch retries and audit/fund/update requests are disabled. npm installs the
already verified root archive and resolves its fixed optional alias online.
The separately downloaded native archive is the post-install byte authority.

After npm exits successfully, every installed root and native package file must
match the pinned archive inventory (size, SHA-256 and executable-bit state).
Unexpected files, packages, directories, links, hard links, writable-by-others
files, missing native payloads and a changed launcher are rejected. The only
launcher is the expected `bin/codex` symlink. HOME/config/data/state stay empty,
and the empty npm configuration files must remain unchanged. npm's scoped
cache/log/temp activity is allowed. The temporary verification tree is removed
afterward; this does not install into the person's actual global prefix.

Downloads have fixed source, length, size and duration checks. npm has a
five-minute process-group deadline and bounded captured output. Failures have
fixed classifications; package output is not published. No uncertain operation
is retried. A newly reserved `--evidence` JSON records success or failure, always
keeping the unverified boundaries explicit. A failed check may have involved a
partial temporary installation before cleanup; it is not a success claim.

Run with trusted official developer tools and a **new** evidence path:

```sh
python -B scripts/codex_official_install.py \
  --node "$(command -v node)" --npm "$(command -v npm)" \
  --evidence /tmp/codex-install-evidence.json
```

Offline regression checks:

```sh
python -B scripts/test_codex_official_install.py
```

### Not verified

- Codex is never executed, including `--version` or `--help`.
- No lifecycle script runs; this does not establish script compatibility.
- The AgentDeck install adapter is not invoked, because it executes the installed
  agent for version/provenance detection. Its separate synthetic GUI acceptance
  is not promoted into real-package acceptance by this test.
- No login, model call, personal configuration, automatic upgrade or uninstall.
- No runtime network isolation claim: downloads and npm contact the official
  registry. This is not a hostile-code sandbox or protection against same-user
  concurrent filesystem substitution. Existing blocked runtime isolation gates
  remain blocked, not bypassed.
- Other versions, operating systems, architectures and npm layouts are not
  implied supported. No installer or release is published.

## 中文

本验收通过真实 npm，把固定官方 Codex **0.167.0** 根包及 Linux x86_64 平台包
安装到一次性的私有目录。不是界面验收中的合成替身，也不会写入用户实际的全局安装目录。

执行 npm 前，先下载两份官方归档，核对固定 SHA-512，限制归档数量、大小、路径、
类型及权限，逐文件记录摘要。根包必须只有固定的平台可选依赖，不能出现生命周期脚本、
额外必需依赖或改变的入口；平台包身份也必须一致。摘要匹配不等于独立验证发布者签名。

npm 使用空 HOME、独立 prefix/cache/tmp、空 user/global npmrc，不继承密钥、代理、
模型服务环境变量或 NODE_OPTIONS。默认与 @openai 作用域均固定为官方 registry，
强制禁用脚本，禁用自动重试和审计等附带请求。安装完成后，核对根包与平台包全部文件
的字节、大小和执行位；缺文件、多出包、异常链接、来源不符或写入配置均判定失败。
缓存与临时文件允许写入。验收结束清理自身临时目录，不进行用户软件卸载。

只验证真实 npm 的文件安装与目录关联，不运行 Codex，包括不运行 --version/--help，
不验证登录、模型调用或真实 AgentDeck 安装适配器。后者会运行已安装程序来检测版本，
仍需要独立的运行隔离验收。本测试需要联网下载，不能当作“已实现网络隔离”；原本
受阻的运行时隔离要求保持不变。无新安装包发布，其他平台、版本和 npm 布局不自动算通过。
