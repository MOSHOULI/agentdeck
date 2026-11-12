# Windows generic configuration DACL preservation

The generic configuration writer previously retained only `fs::Permissions`
(the Windows read-only attribute) when replacing a file with a staged sibling.
A replacement could therefore inherit a different directory DACL. The Windows
regression suite checks replacement, per-file backup, and transactional rollback
separately; the old `fs::copy` backup behavior must not be inferred from the
replacement result.

The test-only baseline `e71783517d86a4d50d367fe1304376bf1f4a962b`
([native Windows run 43119905515](https://github.com/MOSHOULI/agentdeck/actions/runs/43119905515))
failed all three DACL assertions: successful replacement, per-file backup, and
transactional rollback. In each synthetic fixture the original protected
owner-only DACL became an unprotected parent-derived DACL. The native core
library suite reached 410 passed / 3 failed / 10 ignored; these were metadata
assertion failures, not compilation or fixture setup failures.

The next diagnostic run at `e8160e9f`
([43123240254](https://github.com/MOSHOULI/agentdeck/actions/runs/43123240254))
identified a separate refusal: the installed application's protected current-user
fixture had `AUTO_INHERITED` set, while legacy staging lost that control bit.
The reported category was `staging-dacl-verification-control-flags`. Original
bytes, DACL controls, owner and group remained unchanged; no backup survived.
This was a fail-closed compatibility issue, not a successful preservation result.

## Implemented boundary

For existing regular files the writer reads the DACL and owner/group SIDs. It
refuses failed inspection and absent, null, or defaulted DACLs. Only ordinary
allow/deny ACEs (types 0/1 with ordinary inheritance flags and a complete SID)
are supported. Callback/conditional, object, audit and all other ACE types,
unknown ACE flags/payloads, and `SE_DACL_UNTRUSTED`/`SE_SERVER_SECURITY` controls
are refused before content reads/backups. Such ACLs can depend on uncopied SACL
resource attributes or semantics outside this preservation boundary. Existing linked
file and ancestor refusal remains in force. Before any configuration bytes are
written into replacement or backup staging, it checks that the staging owner
and group match, applies the original DACL, and verifies the effective ACL bytes
and protection/auto-inheritance control flags. It does not force every file to
become protected. Unsupported or differing metadata fails closed.

Staging handles request the rights needed to set and read the DACL.
Descriptors carrying `SE_DACL_AUTO_INHERITED` use handle-based
[`SetSecurityInfo(SE_FILE_OBJECT)`](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setsecurityinfo),
with the captured protected/unprotected flag. This includes protected+AI files
created through the modern .NET file-security APIs. Exact readback remains
mandatory, including AI/AR controls; no normalization discards a mismatch.

For legacy non-AI descriptors, the existing `SetKernelObjectSecurity` path is
retained solely as bounded compatibility. [Microsoft discourages that API for
filesystem objects](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-setkernelobjectsecurity),
but converting the descriptor to the modern inheritance model would change its
captured controls. This path touches only our owned empty staging file handle,
never an ancestor/directory or the original file; exact readback is required
before bytes. No new pathname-based security setter or directory propagation
is introduced. Native tests distinguish protected/no-AI, protected/AI and
ordinary unprotected/AI fixtures; unsupported transformations are refused.

Source and staging security are rechecked before replacement; backup source/output handles
are also checked. Creation without an existing destination uses normal parent
inheritance. No permissions of an existing user file are weakened or repaired.

The transaction journal retains original bytes/security and the expected
committed security, captured from the staging handle **before** rename. Rename
remains the last fallible write step. Rollback checks committed security before
restoring bytes or deleting a newly created file. Security drift refuses rollback
and reports a rollback failure instead of overwriting the intervening change.

## Verification and limits

Native Windows tests exercise protected owner-only fixtures, ordinary inherited
DACLs, per-file backup, successful writes, actual completed journal entries,
restore/delete conflict refusal, and empty staging before content. They use
synthetic temporary files only. Linux tests and cross-target compilation are not
native Windows execution evidence. Exact native CI results are recorded in the
pull request, not inferred here.

This is DACL preservation, **not** full Windows security-descriptor preservation.
Owner/group changes are refused rather than copied; SACLs, mandatory integrity
labels, alternate data streams, encryption, compression and other extended
metadata are not preserved or certified by this change. There is no atomic CAS,
protection against concurrent hostile path/ACL mutation, crash recovery, or
claim that Git-level backups preserve filesystem metadata. Empty staging is
created with ordinary inheritance before its ACL is secured: an already-opened
external handle is not revoked by changing the DACL. This is not an at-creation
confidentiality or hostile concurrent-access guarantee. Backup errors may
leave empty directories or an already secured partial backup if cleanup fails.
Official Agent runtime and model/network calls are not part of this acceptance.
