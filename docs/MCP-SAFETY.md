# MCP toggle validation and backup boundaries

English | [简体中文](MCP-SAFETY.zh-CN.md)

This is a development-only fix, **not part of the frozen, pending v1.2.0 release**.
It changes the desktop MCP toggle's backup ordering, without changing registry
contents, package versions or release artifacts. Batch MCP synchronization
(`mcp::sync`) is unchanged.

## What changes

Previously, `toggle_mcp_for_agent` invoked Git backup before the core validated
the requested toggle. An unsupported target or invalid configuration could
therefore be rejected after backup, vault or repository initialization had begun.

The desktop command now calls `mcp::toggle_with_backup`. While holding the shared
**in-process** configuration writer lock, the core:

1. Resolves the agent target and, when enabling, an active server definition.
   It validates the destination path, reads its existing text or records its
   absence, and runs the existing parser/transformation to prepare the result.
2. Invokes the backup callback once only after preparation succeeds. A valid
   no-op still invokes it, preserving the previous desktop behavior.
3. Rechecks destination existence, content and path types. A detected conflict
   returns an error without overwriting the external change.
4. Saves the already prepared result, retaining the existing local recovery-copy
   behavior. It does not reload registries or render the change again after backup.

The private `PreparedToggle` binds the destination, original state and output.
The backup callback must not reenter configuration writers because the lock is
held throughout. The core `mcp::toggle` API shares this preparation but does not
perform a Git backup itself.

## Failure and compatibility boundaries

- **Preflight rejection:** No Git backup callback, local MCP recovery copy or
  destination write occurs. This covers unsupported targets (including OpenCode),
  enable requests for missing/disabled servers or disallowed commands, unsupported
  formats, parser/transformation errors, and rejected destination paths. Paths
  must resolve to an absolute filename without `..` components; existing
  components cannot be symlinks, an existing destination must be a regular file,
  and existing parents must be directories. This rejects unsafe/unresolved paths,
  not general path normalization.
- These are the existing format parser/transformation checks plus destination
  checks, not complete validation of every external agent's configuration schema.
  Disabling still allows removal by name when the registry entry is disabled or
  has been removed; it does not require an active server definition.
- **Backup failure:** The error stops the destination write and local MCP
  recovery-copy step. The backup operation itself may already have left files.
- **Conflict after backup:** A completed backup may remain. The check distinguishes
  an absent file from a newly created empty file and rejects detected content or
  path-type changes. It is not a cross-process lock or atomic compare-and-swap;
  an external writer can still race the final check and file replacement.
- Registry reads already create/use `registry-update.lock` and may recover an
  interrupted registry update. Preflight rejection therefore does **not** promise
  zero filesystem activity or absence of `~/.agentdeck/`.

A toggle edits configuration only. It does not install, start, stop or contact
an MCP server, and success does not establish that a live server works. Local
recovery copies may contain credentials; see the [data and recovery boundaries](../README.en.md#data-and-recovery-boundaries).

## Validation scope

Regression tests use isolated synthetic inputs; core backup-order tests inject
fake backup callbacks. The original ordering was reproduced with a fake backup
marker, not a real vault/Git backup. Real user configurations, external agents,
live MCP servers and the desktop GUI have not been exercised for this increment.
Synthetic tests do not establish end-to-end desktop or Windows protected-backup
behavior.
