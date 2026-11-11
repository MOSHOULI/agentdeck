# Reviewed manual memory exports

The native desktop Memory panel can edit the **saved manual selection** for Codex,
Claude Code, or OpenCode. Select a target, read its saved selection, add or remove
existing entries across searches, preview the replacement, then confirm. Selected
entries remain visible and removable even when absent from the latest 50 search
results. Reset restores the loaded selection; cancel discards the draft. Neither
writes the export. Changing target or draft invalidates the preview.

The result is a local Markdown file at `~/.agentdeck/memory/<target>.md`. This is
**not evidence that an external agent loaded or used the file**. Agent runtime
configuration/context loading still needs separate verification. Browser/demo mode
does not dispatch editor writes or simulate success.

## Exact selection semantics

For target T, output is the union of:

1. Explicit manual IDs saved for T, including entries collected from other agents.
2. Rows whose source `agent_id` is T and whose automatic flag is enabled.

IDs are deduplicated. Output order is descending collection timestamp, then
numeric ID descending. Clearing the manual list preserves automatic rows/flags,
all database rows, other targets' selections/files, and collection settings. The
editor never collects sessions, discovers sources, runs agents, reads credentials,
or performs network requests. Existing collection and automatic-export controls
remain separate, explicit operations.

## Read, preview, checked replacement

- `read_memory_export` reads the saved manual and target automatic rows with a
  consistent SQLite read transaction. New DTOs omit source-session paths.
- `preview_memory_export` requires the loaded state fingerprint plus the proposed
  IDs. It fails when saved manual/automatic sets or relevant rows have changed.
- `replace_memory_export` requires the preview fingerprint and the same proposed
  IDs. It rechecks the target, previous manual/automatic rows, proposed rows and
  their rendered content/metadata before writing. Changed state requires a new
  read and review. Fingerprints are freshness checks, not authorization secrets.
- Unsupported targets, malformed/duplicate/missing IDs, invalid stored metadata,
  missing databases and oversized selections fail closed. No successful response
  silently truncates a selected set.
- Pending operations retain ownership when navigating between workspaces. Rapid
  repeated clicks dispatch once; failures invalidate confirmation and require
  refresh/review rather than blind retry.

The snapshot uses an existing read-only SQLite connection, not `MemoryStore::open`
(which initializes storage), and never uses `immutable=1`. Committed WAL data is
visible. Missing database reads create no application directory, schema, registry
lock, database, or export. Existing SQLite databases can still require WAL/SHM
coordination sidecars and ordinary access-time effects. This is not a promise of
zero filesystem effects. Registry validation uses its existing read-only path;
busy or recovery-required registry state fails without repairing it.

Existing database and export paths are checked for linked/nonregular files and
linked/non-directory parents. Unix symlinks/hardlinks and Windows reparse points/
hardlinks are refused; SQLite also receives `NOFOLLOW`. These path checks do not
provide protection against hostile concurrent filesystem replacement.

## Bounds

- At most 200 manual IDs per target and proposed selection.
- At most 500 unique rows in each current or proposed output union.
- At most 64 KiB UTF-8 **content** per row.
- At most 2 MiB summed UTF-8 JSON bytes of unique `ExportRow` records across the
  current manual, automatic, and proposed sets inspected together. Raw field sizes
  are also checked before loading content. This is a unique-row budget, **not** a
  full IPC response budget: repeated arrays can serialize the same row again.
- IDs are canonical positive signed 64-bit integers represented as strings;
  timestamps are JavaScript-safe integers; source agent identifiers are 1–64
  lowercase ASCII letters, digits or hyphens; automatic flags must be 0 or 1.

## Consistency and failure boundary

Supported in-process manual replacement, legacy injection, automatic flag/export,
and collection writes share a mutex. Desktop commands that may wait on it run on
blocking workers rather than the UI command thread. Read transactions prevent
mixed database snapshots.

Replacement remains **database first, then Markdown replacement**. If Markdown
writing fails, the prior manual database selection is restored when possible:

- `write_failed_rolled_back`: the database compensation succeeded; the error still
  includes the original file-write failure. Refresh/review is required.
- `write_failed_uncertain`: database replacement/worker outcome is uncertain, or
  compensation also failed. Both original and rollback errors are retained when
  available. Inspect and refresh before any further write.

The mutex is process-local. This is not an atomic database-plus-file transaction,
not cross-process compare-and-swap, and not crash recovery. External writers may
race between validation, database replacement and file replacement. A successful
receipt confirms the operation completed, not continuous agreement with external
processes. Clear or replace does not claim to unload agent runtime context.

## Verification boundary

Synthetic tests cover mixed-source/overlapping selections, outside-search rows,
exact union/clear and other-target preservation, invalid IDs and metadata, limits,
stale manual/automatic/content snapshots, actual committed/uncommitted WAL reads,
missing-home no-create behavior through the public API, compensation errors,
unsafe paths and competing supported in-process writers. Frontend tests cover
strict DTOs, navigation lifetime, interrupted/repeated flows and browser gating.

Linux WebKit/WebDriver and Windows WebView2/UIA acceptance scripts use authored
isolated databases and Markdown files. Their execution requires the corresponding
native CI hosts. Passing Rust/React tests or authoring these scripts alone does
not prove native GUI acceptance, real source collection, installation, actual
agent loading, or every application function. See CI results for the exact commit
before claiming the native scenarios passed. Normal tests intentionally leave
real-environment tests ignored; never use `--ignored` or `--include-ignored` for
this feature's validation.
