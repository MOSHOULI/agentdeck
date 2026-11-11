# Linked configuration destinations

Profile configuration refuses an existing instruction/configuration destination
that is a symbolic link, a Windows reparse point, a nonregular file, or a file
with more than one hardlink. Existing parent components must be ordinary
unlinked directories. Missing destinations and missing parent directories remain
supported. This deliberately rejects linked home/configuration directories too;
AgentDeck does not resolve or detach them automatically.

The refusal explains that the owner can manage the file with the tool that
created the link, or prepare a separate regular file in an unlinked directory
before trying again. No automatic unlink, copy, link conversion or force action
is provided. A shared dotfile remains shared rather than silently becoming an
independent file after replacement.

Checks run before effective destination content reads, per-file injection
backups and journal snapshots, and again at writer/rollback mutation boundaries.
Windows uses a metadata-only handle with OPEN_REPARSE_POINT to check hardlink
count without reading configuration bytes. Skipped adapters, missing instruction
templates and unsupported formats do not cause destination checks.

These are pathname metadata checks, not atomic path confinement or a
cross-process transaction. Another process can change paths between checks.
Earlier targets may already have been written when a later target is rejected;
the existing rollback journal is retained. Rollback refuses a target that is now
linked instead of unlinking or replacing it, and reports a rollback failure.
The desktop command's earlier Git safety backup and profile initialization can
still happen before a destination refusal. The error is not a claim that no
other files changed. This change does not enable the blocked native Codex bulk
configuration action or add new model/MCP adapters.

## Verification

Core tests cover valid Claude/Gemini/Codex instruction hardlinks, direct writer
hardlinks, leaf/dangling/parent symlinks on Unix, skipped linked targets, and both
restore-existing and remove-new rollback branches after a hardlink appears.
Existing malformed-marker preservation and later-target rollback tests remain.

Windows installed-app acceptance extends the existing configuration-failure
stage after its two malformed-marker tests. With the App stopped, the harness
authors valid instruction files and creates one hardlink alias per Claude/Gemini
file. It then uses the installed AgentDeck WebView's actual Configure action and
native confirmation once per Agent, requires the fixed refusal/recovery text,
and compares both aliases' bytes, file identity, link count, timestamps,
attributes, mode and ACL before discovery, after refusal and after App close.
Hardlinks require no Windows symbolic-link privilege. The authored packages only
answer version probes; this verifies installed AgentDeck behavior, not official
Claude/Gemini execution. Actual Windows execution must pass separately; offline
fixture and PowerShell syntax tests are not installed-app evidence. Windows
symbolic-link/junction handling is implemented via reparse checks but this GUI
stage proves the hardlink case only.
