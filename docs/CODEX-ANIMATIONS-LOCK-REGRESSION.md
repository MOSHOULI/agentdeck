# Codex animations: fork-safe lock release

## Evidence and cause

The parallel core suite intermittently failed
`restore_cleanup_final_state_cannot_hide_foreign_changes`: the receipt still
contained the applied state instead of the test's injected foreign bytes.
Instrumentation showed `try_lock` returned `WouldBlock`, surfaced as `Busy`,
before **any** restore hook ran. The later mutex-poison failures were secondary.

On Linux, `flock` belongs to an open-file description shared across `fork`.
`O_CLOEXEC` closes the inherited descriptor at **exec**, not at fork. If another
thread forks while apply owns its lock, dropping only the parent's `File` can
leave the completed operation locked until that child execs or exits.

A pipe-gated child reproduces the same `Busy` and empty hook trace without
sleeps, retries, or a competing configuration writer. The regression fails when
explicit unlocking is removed and passes with `LockGuard`. This establishes the
mechanism; the particular child from the original historical failure was not
identified.

## Required behavior

- Construct `LockGuard` immediately after successful acquisition, before later
  inode validation. Explicitly unlock on return, error, and unwind.
- Keep active-owner contention nonblocking and exclusive. Never unlink the lock
  file or bypass identity, permission, link, receipt, or conflict checks.
- Run the gated-fork regression in an exact-filtered subprocess so it cannot
  retain unrelated parallel tests' descriptors. It checks successful, failed,
  and unwound apply, active-owner `Busy`, immediate restore, original bytes,
  receipt cleanup, and the persistent lock file.
- The foreign-change regression must reach `CleanupSynced` and return `Conflict`;
  an unrelated early error must not count as successful conflict detection.

This changes normal scope-exit release, not the protocol's durability or
same-user-writer guarantees. On abrupt process death, release still depends on
the kernel closing the last inherited descriptor.

```sh
python scripts/run-isolated.py cargo test --locked -p aisync-core --lib config::codex_animations_fs::native::tests
python scripts/run-isolated.py cargo test --locked -p aisync-core -p aisync-cli
```

## 中文说明

并行测试曾在恢复后的外部修改检查中失败：回执仍是 `applied`，没有执行测试注入。
补充错误与阶段记录后确认，恢复在任何 hook 之前就因 `try_lock` 的 `WouldBlock`
返回 `Busy`；后续 mutex 中毒只是连带失败。

Linux 的 `flock` 绑定到可由 `fork` 继承的打开文件描述，而不是单个文件描述符。
`O_CLOEXEC` 只在 `exec` 时关闭继承描述符。另一线程在 apply 持锁时 fork，
父进程仅关闭 `File`，可能无法立即释放已结束操作的锁。

回归测试使用管道暂停 fork 子进程，确定性复现同样的 `Busy` 和空阶段记录，
没有睡眠、重试或真实竞争写入者。删掉显式 unlock 后测试必败，保留修复则通过；
原始历史失败中的具体子进程未被识别。

必须在成功加锁后立即创建 guard，并在正常返回、错误和栈展开时显式解锁。
真实持锁竞争仍返回 `Busy`，锁文件不删除，所有身份、权限、链接、回执与冲突检查不变。
管道测试在单独的精确筛选进程内运行，避免继承其他并行测试的锁。外部修改测试
必须确认到达 `CleanupSynced` 且返回 `Conflict`，不能把任意提前报错当作验证成功。
进程被强制终止时仍由内核在最后一个继承描述符关闭后释放锁；此修复不扩大持久性
或对同用户外部写入者的保证。
