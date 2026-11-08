# Read-only CLI synchronization plan

This development feature is not yet included in a published installer.

```sh
cargo run --locked -p aisync-cli -- sync --tools codex,claude-code --dry-run
```

`--dry-run` emits a JSON metadata plan for the explicit selection. It never runs an agent, npm, Git, or an installer; creates no profile, backup, vault, or memory database; and does not read configuration, profile-template, or memory contents. It does read catalog metadata. `--yes` and `--dry-run` conflict, so a preview cannot accidentally become an apply request.

The report lists the managed instruction-block destination and the local memory-export destination where those adapters are declared. Paths containing `~` are symbolic, not resolved. Non-simple template names have their source path withheld rather than inventing a path. Memory aliases and invalid targets use the same pure target-validation rules as the export adapter. A local memory file does not prove that the external agent consumes it.

## What the result means

- `batchStatus: unverified`: a declarative plan was produced, **not** a successful eligibility check or permission to apply.
- `batchStatus: unsupported`: at least one target has no supported operation or an invalid memory-export target. The report still exits 0 so callers can inspect it.
- `notChecked` explicitly lists installation eligibility, file contents, path safety/permissions, actual changes, memory entries and backup availability. There is no `ready` state.
- Unknown IDs or registry-read errors produce a nonzero exit and no partial JSON report. Invalid arguments exit 2. A report/output failure exits nonzero.
- This is neither a file diff nor a snapshot-bound approval token. Repeating it may yield a different plan if catalog metadata changes. No template content, credential, or memory text is shown.

An actual `sync --yes` still uses its existing validation and backup workflow, can initialize missing profile templates and the memory store, and may fail or partially complete. The dry run does not change those semantics or make the batch transactional. Model settings, MCP servers, skills, and software installation are outside the CLI `sync` operation. The desktop MCP preview is a separate feature with stale-preview checks.

## Read-only catalog handling

A fresh home can use the bundled catalog without creating `.agentdeck`. If an existing coordination lock is present, the dry run opens it read-only and takes a shared lock. It refuses a busy/unsafe lock, an existing runtime catalog without a lock, or a recovery journal. It never creates a missing lock or repairs an interrupted registry update. Normal application recovery remains a separate operation; do not delete lock or journal files to force a preview.

With that lock held, selection matches the normal catalog policy, including the narrowly identified official v1 catalog migration. The plan is still only metadata at the time of the read, not a guarantee about the later apply target.

## Verification and limits

Tests execute the real CLI binary in isolated temporary homes, compare files/directories before and after, place a synthetic executable on PATH, retain synthetic configuration/memory secrets, and exercise invalid selections, argument conflicts, unsupported targets, runtime metadata, absent/busy locks, and pending recovery. Pure tests cover memory alias validation and withheld template sources. These are synthetic tests, not real Agent installation, Windows GUI, profile application, or online MCP acceptance tests.
