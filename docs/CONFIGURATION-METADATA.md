# Generic configuration writer: Linux extended metadata boundary

Personalization/model injection and the generic config writers preserve ordinary
mode bits, with backups/rollback retaining bytes, not arbitrary filesystem metadata. On Linux they now refuse
an existing file if its extended-attribute name list is nonempty. This includes
visible POSIX access ACLs and security labels as well as `user.*` attributes. Any
inspection error, including unsupported inspection, also refuses the operation.
Nothing automatically clears attributes or copies a potentially incomplete set.

The guard is compiled for **all `target_os = "linux"` architectures**, through
libc's `llistxattr` and `flistxattr`; it is not an x86-only implementation with a
silent no-op fallback on other Linux targets. Native verification was on Linux
x86_64. Attributes hidden by the filesystem or security policy are outside the
name-list observation's guarantee.

Checks occur before effective destination content reads, transaction snapshots,
and per-file backups, and again at replacement and rollback boundaries. Inactive
or skipped adapters do not inspect their destinations. Missing files can still
be created. Existing link refusal remains independent and unchanged.

Linux per-file backups use a newly created handle rather than `fs::copy`.
Fresh backup and replacement files are inspected **before changing permissions
or writing configuration bytes**, so inherited metadata cannot be silently
removed by chmod or carried into a supposedly ordinary replacement. Staging is
checked again after writing, and the replacement source and destination are
checked before rename. Parents are checked for links/file kinds, not rejected
merely for directory xattrs: inherited attributes are checked on the actual new
file. This can conservatively refuse a system-managed label even if it would be
safe to preserve. The original destination is not rewritten on a refusal before
replacement; empty staging artifacts/directories may remain if cleanup fails.

Rollback refuses to overwrite or delete a destination that has gained visible
extended metadata. A rollback failure is reported, not presented as successful
restoration. Ordinary rollback restores previous bytes or removes a new file;
it does not restore inode identity or timestamps. Earlier Git-backed backups
are separate and are **not** certified as extended-metadata-preserving here.

These are bounded observations, **not atomic compare-and-swap, cross-process
conflict protection, race-free pathname confinement, or crash recovery**. Another
process can change a file between checks, and replacement errors with uncertain
outcomes still require inspection. No post-rename fallible check is added that
would misreport a completed replacement as uncommitted.

Windows behavior is intentionally unchanged. Windows DACL preservation and
possible permission changes remain unverified; this change makes no Windows or
cross-platform metadata-protection claim. macOS behavior is also unchanged.
The separate Codex animations filesystem adapter is not modified by this guard.

Tests cover a real `user.audit` attribute through public `config::inject`,
all-format refusal before reads/backups, added metadata before replacement and
rollback, skipped adapters, ordinary 0600 backups/writes and missing-file
creation. An injected file creator exercises the shared inherited-staging guard
without pretending that a filesystem with unsupported POSIX ACL creation tested
real inherited ACLs. Run them with `scripts/run-isolated.py`; no Agent is executed.
