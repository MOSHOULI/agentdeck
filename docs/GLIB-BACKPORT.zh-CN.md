# GLib 0.18.5 VariantStrIter 回移补丁

[English](GLIB-BACKPORT.md)

## 范围与当前状态

本变更针对 AgentDeck 的 Linux GTK 3 依赖链，为 [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) 回移一个范围明确的源码修复。它基于提交 `52ffa227e01fcf697ef3ee5d0ad888df1010e6c8` 重新构建，并非恢复此前丢失的 Git 提交。本变更不将 GLib 升级至 0.20，不迁移桌面技术栈，不修改 AgentDeck 版本，也不代表全部依赖均已安全。

**专项原生验证已于 2026-11-05 通过。** [CI 运行 42015765543](https://github.com/MOSHOULI/agentdeck/actions/runs/42015765543) 使用 Rust 1.99.1 和系统 GLib 2.80.0，测试提交 `e29e85192a820aae3edcbb06351ab89e2742784c`。18 项真实 GLib 测试在 test 优化等级 0、2 及 release 优化等级 3 全部通过，共执行 54 次，无忽略项。GLib Clippy、41 项校验器测试、官方归档校验、真实应用依赖图及新的保守审计均通过。审计有意保留 RUSTSEC-2024-0429 和四项维护警告；未修改的上游源码仍产生 34 条旧式编译警告，因此不能称为无警告构建或全部依赖均安全。

此前 Debian 13 云端准备环境缺少 GLib 开发元数据，包管理器访问失败；没有绕过该限制。原生测试通过项目正常的 GitHub 托管 Linux 任务运行，使用官方系统软件包。完整 Linux 桌面构建、GTK/WebKit 图形界面验证及 Linux 分发支持仍未验证；这些专项测试不能替代上述独立检查。

[此前的依赖审查](DEPENDENCY-SECURITY.md) 是历史背景，其中的 GLib 警告促成了本次回移。本变更应以下述证据和限制为准。

## 源码来源与精确变更

- 原始版本：[官方 `glib 0.18.5` crate 归档](https://static.crates.io/crates/glib/glib-0.18.5.crate)。
- 归档 SHA-256：`233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`。
- 上游修复：[`b5a4071e439bef2b5eea76c3aa25e5ae84839e34`](https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34)。
- 上游合并：[`05dff0ee696f9bcd8617cd48c4b812d046d440cb`](https://github.com/gtk-rs/gtk-rs-core/commit/05dff0ee696f9bcd8617cd48c4b812d046d440cb)。
- 内置源码：[`vendor/glib`](../vendor/glib)，保留归档中的全部 121 个原始文件，以及发布版本号、清单、`LICENSE` 和 `COPYRIGHT`。
- 可审查的补丁：[`vendor/glib-RUSTSEC-2024-0429.patch`](../vendor/glib-RUSTSEC-2024-0429.patch)。
- 机器可读的来源与文件哈希：[`vendor/glib-backport.json`](../vendor/glib-backport.json)。

与官方归档相比，仅修改 `vendor/glib/src/variant_iter.rs` 中 `VariantStrIter::impl_get` 的两行：

```diff
-            let p: *mut libc::c_char = std::ptr::null_mut();
+            let mut p: *mut libc::c_char = std::ptr::null_mut();
@@
-                &p,
+                &mut p,
```

C 函数通过该输出参数写回字符串指针。原实现传入了不可变指针变量的共享引用，C 通过该引用写入违反 Rust 的别名规则。编译优化可能因此使读取到的指针仍被视为空值，随后将它作为 C 字符串读取时发生未定义行为。回移补丁将变量声明为可变，并传入独占的可变引用，与上游修复一致。它不改变字符串所有权、生命周期、公开 API 或发布版本号。

## 确认应用实际使用补丁源码

工作区的 `[patch.crates-io]` 配置为现有的 `glib 0.18.5` 依赖选择 `vendor/glib`。单独添加另一个较新的 GLib 依赖，并不能修复仍在使用旧版的 GTK 3 路径。需要检查的应用路径是 `agentdeck` → Tauri/Wry/GTK 3 → `glib`。

[`scripts/verify-glib-backport.py`](../scripts/verify-glib-backport.py) 检查来源、完整的内置文件集合和精确的两行改动。提供完整 Cargo metadata 后，它还会检查解析后的应用依赖图是否到达 GTK 和已打补丁的 GLib 包。仅证明回归测试 crate 使用了补丁副本，并不足以证明应用也使用它。使用 `--no-deps` 生成的 metadata 不完整，会被拒绝。

在获准使用的 Linux 验证主机上，将官方归档保存到本地，然后执行：

```sh
python scripts/verify-glib-backport.py --archive /path/to/glib-0.18.5.crate
cargo metadata --locked --format-version 1 --filter-platform x86_64-unknown-linux-gnu > /tmp/agentdeck-glib-metadata.json
python scripts/verify-glib-backport.py --archive /path/to/glib-0.18.5.crate --metadata /tmp/agentdeck-glib-metadata.json
```

Metadata 只解析依赖图，不会编译或执行原生代码。记录验证时，应同时保存命令输出和实际受测的完整提交号。

## 真实 FFI 回归覆盖

[`crates/glib-regression`](../crates/glib-regression) 是禁止发布、使用 edition 2024 的工作区 crate。其精确版本开发依赖 `glib = "=0.18.5"` 仅对 Linux 启用。全部 18 个测试均位于 `#[cfg(all(test, target_os = "linux"))]` 中，因此非 Linux 工作区测试通过并不能证明覆盖了 GLib。

测试构造真实的 `glib::Variant` 字符串数组，调用 `Variant::array_iter_str`，没有用模拟实现代替 GLib。覆盖范围包括：

- `next`、`nth`、`last`、`next_back` 和 `nth_back`，每种入口均读取 ASCII、空字符串和 Unicode 字符串。
- 空数组及仅含一个空字符串的数组，保证二者不会混淆。
- 越界偏移、`usize::MAX`、前端索引非零时的加法溢出，以及反向减法下溢。
- 前后交替读取、混合跳过、部分消费后的 `last`，以及耗尽后反复调用。
- 消费前后的精确 `len`、`size_hint` 和 `count`，以及正向和反向收集结果。

所有测试数据都只存在于内存。该 crate 不读写真实的 AgentDeck 配置，不安装或调用 Agent，不访问网络，不启动进程，不初始化桌面界面，也不启用已忽略的真实环境测试。

在已授权、具有真实 GLib 开发库及 `pkg-config` 的 Linux 主机上，通过现有的隔离 HOME 脚本运行三个优化档位：

```sh
CARGO_PROFILE_TEST_OPT_LEVEL=0 python scripts/run-isolated.py cargo test --locked -p glib-regression --lib
CARGO_PROFILE_TEST_OPT_LEVEL=2 python scripts/run-isolated.py cargo test --locked -p glib-regression --lib
CARGO_PROFILE_RELEASE_OPT_LEVEL=3 python scripts/run-isolated.py cargo test --locked -p glib-regression --lib --release
cargo clippy --locked -p glib-regression --all-targets -- -D warnings
cargo fmt --all --check
```

Linux CI 验证在 GitHub 托管 runner 上安装 `libglib2.0-dev` 和 `pkg-config`，随后执行回归 crate 的 Clippy 检查及上述三个测试档位。这是在另一台获准使用的验证主机上执行，不是重试已被阻塞的云环境安装。对于后续提交，必须检查该提交自己的 CI 结果；上面的证据只对应记录的源码提交。

优化档位很重要，因为该缺陷涉及优化器对共享引用所作的假设。测试通过只能提供回归证据，不能证明未定义行为绝不可能发生。源码来源和精确的上游修复仍是必要证据。不得伪造 `pkg-config` 元数据，也不能将前置条件的访问失败视为改用其他安装途径的授权。

## 审计报告必须保留警告

回移补丁有意保留 `0.18.5` 版本号。RustSec 列出的上游修复版本线为 `glib >=0.20.0`，不会自动认可本仓库的源码审查。另一方面，**cargo-audit 0.22.2 会跳过路径依赖包**，因此应用 `[patch.crates-io]` 后 GLib 警告消失，并不是问题已经修复的证据。

为了继续展示这一风险，验证脚本可以生成独立、保守的审计锁文件。它保留当前 `Cargo.lock` 的全部内容，仅恢复内置 GLib 包原有的 crates.io 来源和校验和。它不修改实际构建使用的锁文件，不删除当前依赖，不升级包，也不屏蔽公告。输出路径必须尚不存在。这个仅用于审计的注册表身份会有意保留已发布版本 `0.18.5` 的公告，不是应用构建时使用的依赖来源。

```sh
set -eu
audit_dir=$(mktemp -d)
python scripts/verify-glib-backport.py --archive /path/to/glib-0.18.5.crate --audit-lock-output "$audit_dir/Cargo.lock"
audit_status=0
cargo audit --file "$audit_dir/Cargo.lock" --json > "$audit_dir/cargo-audit.json" || audit_status=$?
python scripts/verify-glib-backport.py --archive /path/to/glib-0.18.5.crate --audit-json "$audit_dir/cargo-audit.json"
# 即使本项门禁失败，CI 也必须发布 JSON 报告。
test "$audit_status" -eq 0
```

JSON 检查要求报告中存在 GLib 的 `RUSTSEC-2024-0429` 条目，而不是把它消失视为成功。不得添加忽略配置。应结合已审查的补丁及实际原生测试结果解释这份保守报告。其他漏洞、被撤回的包和维护状态警告仍需分别审查。默认审计命令返回成功并不代表完全安全；对这份保守报告执行严格的 `--deny unsound` 检查，仍会按设计失败。

## 尚待完成的验证与移除条件

1. 为最终提交记录成功的来源校验和完整应用依赖图检查。
2. 后续变更需在真实 Linux 环境中，以测试优化级别 0、2 和发布优化级别 3 重新运行回归套件并保留日志。记录的提交已通过全部三档测试。
3. 保存保守审计 JSON 及退出状态，确认 GLib 公告仍然可见。
4. 完整 Linux 桌面构建和运行验收属于独立工作，需要 GTK/WebKit 前置条件及相应授权。本 GLib 专项套件不能代替这些验证，也不能代替 Windows 原生验证。
5. 未来受支持的 GTK/Tauri 依赖迁移移除受影响的 GLib 版本线后，重新检查应用依赖图和测试，再一并移除内置补丁、来源检查和仅用于审计的身份恢复逻辑。不能仅因审计警告消失就移除它们。
