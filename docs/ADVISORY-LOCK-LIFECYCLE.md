# Completed operation lock release

This source-only change extends the separately reproduced Codex lock-lifetime
finding in [the Codex regression](CODEX-ANIMATIONS-LOCK-REGRESSION.md) to four
independently audited acquisitions:

- Qwen owned Vim settings: apply, update and restore use the same exclusive lock.
- Registry recovery/read/update: exclusive lock, including recovery before the
  guard is returned to the caller.
- Registry read-only catalog inspection: shared lock on an existing lock file.
- Vault keystore: exclusive lock around the supplied operation.

On Linux, a concurrent unrelated `fork` inherits the parent's open-file
description. `CLOEXEC` closes the child descriptor only at exec, not at fork.
Closing the parent's descriptor alone therefore can retain a finished operation's
lock. The next Qwen operation can report `Busy`; registry/vault users can wait
until their existing deadlines. A retained shared registry lock blocks writers,
even though other shared readers can continue.

## Fix and unchanged boundaries

A private, non-cloneable `FileLock` owns an **already acquired** lock and explicitly
unlocks it on normal return, error return and panic unwind before closing its
file. It does not expose the file or change the operation's lock scope. Registry
recovery receives this ownership before any fallible recovery work. Shared
registry readers use the same guard without changing their lock mode.

Lock paths remain persistent. There is no unlink, stale-age stealing, timeout
increase, retry workaround, process-wide serialization or weakening of active
writer exclusion. Existing file checks, metadata handling, receipts, registry
recovery, vault encryption and supported-platform restrictions remain unchanged.
The previously fixed Codex guard is not refactored by this change.

Explicit Drop does not run after abrupt process termination. In that case cleanup
still depends on OS handle lifetime, including inherited handles. Advisory locks
coordinate participating operations only; this is not protection against hostile
same-user writers, a power-loss transaction, or a real Agent runtime test.

## Deterministic regression evidence

Four Linux x86_64 tests each run in an exact-filtered subprocess, so their forked
children cannot retain unrelated parallel tests' locks. A pipe holds the child
between fork and exec. Only async-signal-safe libc calls run in the child; cleanup
releases and reaps it even when a parent assertion fails. No timing sleep is used
to establish inheritance.

Each test covers success, error and unwind while the child remains alive, checks
writer exclusion during the operation and immediate writer reacquisition after
completion, and checks that the persistent lock file remains. The shared-registry
test also verifies that another shared reader can acquire the lock while active.

- Qwen forks during its existing test-only restore checkpoint after writing the
  restoring receipt. Failed/unwound restores preserve that receipt and can then
  complete, retaining the original field and unrelated synthetic content.
- Registry uses per-call test-only checkpoints after acquisition. Its exclusive
  error case exercises actual invalid-journal recovery before returning a guard;
  its shared error case refuses the pending journal. The journal remains intact.
- Vault uses a synthetic closure and never creates a master key. This does not
  enable or verify a non-Windows protected vault.

All four tests fail with `WouldBlock` against the original close-only code and
pass with explicit unlock. Run through the isolated HOME wrapper:

```sh
python scripts/run-isolated.py cargo test --locked -p aisync-core forked_child_exits
python scripts/run-isolated.py cargo test --locked -p aisync-core -p aisync-cli
```

Full exact-commit CI remains required, including Windows native/DPAPI coverage,
Linux/Windows application gates, frontend and dependency checks. No installer or
registry release is published by this change.
