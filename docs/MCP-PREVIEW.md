# MCP change preview

English | [简体中文](MCP-PREVIEW.zh-CN.md)

Development feature, not yet shipped in a public installer.

The desktop MCP panel now separates reviewing a change from applying it. Clicking
an agent checkbox prepares a preview; it does not immediately back up or write the
agent's configuration. Review the server, agent, target file, format, requested
inclusion/removal, file existence and whether bytes would change, then explicitly
confirm. Cancel leaves the configuration unchanged.

The preview is a **change summary, not a full configuration diff**. It deliberately
omits configuration contents, credentials and rendered output. JSON/YAML writers
may reserialize the file and affect formatting or comments; the summary does not
show those differences. Enabling adds or replaces the entire same-named server
block from the registry definition, so custom fields in that block may be lost.
Profile application and batch CLI synchronization do not
gain a preview in this increment.

## Freshness and safety

The preview carries an opaque revision for the proposed operation. The desktop
sends it back when applying. Under the shared in-process configuration writer
lock, the backend prepares the operation again and rejects a changed destination,
configuration or relevant registry definition before starting backup. A stale
preview must be reviewed again; it is never silently applied to the newer state.
The revision is a freshness check, not an authentication or authorization token.
Do not log or publish it. Existing direct toggle APIs remain available.

Only an explicitly confirmed, matching operation proceeds through the existing
backup and write flow. Backup failure prevents the configuration write, but the
backup itself may already have created files. A second destination check detects
changes during backup. This is not a cross-process transaction: an external writer
can still race the last check and atomic replacement.

Preview does not run Git backup, initialize a vault, create a local MCP recovery
copy, write the destination, or execute/contact a server. Registry loading can
still create a registry lock and recover an interrupted registry update; preview
therefore does not promise zero filesystem activity. Success describes configuration,
not server health, authentication or connectivity.

## Verification scope

Tests use synthetic configurations and mocked IPC. Real agent configurations,
live MCP services and manual Windows desktop acceptance are not established by
those tests. See [MCP backup boundaries](MCP-SAFETY.md) for existing recovery and
concurrency limits.
