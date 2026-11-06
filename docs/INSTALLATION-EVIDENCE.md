# Installation evidence and management boundaries

English | [简体中文](INSTALLATION-EVIDENCE.zh-CN.md)

The development branch provides read-only installation observations and a separate,
restricted install-only npm adapter. Neither creates a durable installation receipt,
proves who originally installed a program, or grants permission to remove or upgrade
a package. Registry recipes remain separate from facts observed on this machine.

These changes are unreleased and are not in the published v1.1.1 installer. A shared
version string does not identify the code included in a build. Compare the release's
`BUILD_COMMIT.txt` with the source revision; see the [README](../README.en.md).

## What is reported

`inspect_installation` returns the existing installation probe plus candidate
instances, an ambiguity flag and an optional explanation. Each instance separates:

- `path`: the absolute candidate found through the application's command search.
- `resolvedPath`: the canonical discovered file. A Windows batch launcher still
  resolves to the batch file, not automatically to the program it launches.
- `entrypointPath`: an actual package entrypoint, only when a supported link or
  launcher mapping has been verified.
- `version`: parsed from a bounded command run for the uniquely selected active
  instance. Registry and package-manifest version strings are not substitutes.
- `source`: either a reason for unknown association, or the observed npm package,
  package root, global root, prefix and exact manager entrypoint.

The list is a PATH observation, not a complete machine inventory. Duplicate
aliases to one target do not constitute distinct installations. Other candidates
are displayed without running their version commands. Different targets, failed
inspection and inconsistent observations must not silently select a write target.

Custom registry shell verification can retain its bounded legacy status result,
but does not produce inferred instance paths, measured versions or npm ownership.
Unsupported Windows launchers are not interpolated into a shell to obtain a
convenient answer.

## npm association

The supported adapter uses fixed package identifiers and command names. A matching
install recipe, a package name reported by a package manager, or a directory named
`node_modules` is insufficient. Association requires a consistent chain:

1. Resolve a particular supported npm manager and its actual invocation.
2. Query its global prefix and root, and verify their canonical layout.
3. Read a bounded package manifest with the exact expected name and bin mapping.
4. Reject linked packages and entrypoints escaping the package directory.
5. Bind the selected executable to that exact bin, using a canonical link target
   or the complete supported Windows launcher template.

Windows npm prefix redirection must follow the launcher semantics, rather than
assuming that an adjacent npm CLI is always the one the launcher selects.
Unknown templates, extra launcher commands, inconsistent roots and conflicting
evidence remain unknown. A successful safe version probe is not erased merely
because npm association cannot be established.

The UI calls a successful result “npm 全局目录匹配，安装历史未知”. It does not say
that npm or AgentDeck historically installed the software. This is a consistency
check against accidental misidentification, not cryptographic publisher identity
or a sandbox against another process running with the same user's permissions.

## Execution and write boundaries

Known-instance probes use a concrete program and separate arguments. Their output,
input-file sizes and runtime are bounded; raw stderr is not returned as evidence.
AgentDeck does not parse or return credential or npm configuration contents as
source evidence; npm itself may consult its configuration. Tests use temporary
directories, synthetic launchers and injected manager results.

Read-only describes AgentDeck's requested operations: it does not install,
upgrade, uninstall, or write agent configuration during inspection. It does execute
local version and npm query programs. Those programs may read their own
configuration, update caches or logs, or have other side effects; the probe is not
an OS security sandbox. Limits apply to each subprocess and inspected input/output,
not a guaranteed deadline for every filesystem operation or the entire refresh.

## Install-only execution

Installing software and applying configuration are separate actions. Installation
requires a supported fixed npm adapter, not just a registry shell command or a
`not_detected` PATH result. The current allowlist binds Codex, Claude Code, OpenCode,
Gemini CLI and Pi to their exact package and verification-command mappings. The
`opencode-ai` recipe is the documented OpenCode V1 route, not a claim of V2 support. The
registry's method and recipe must also match the adapter. Manual entries and
arbitrary shell recipes are not executed by this install path. See the
[catalog source review](REGISTRY-SOURCES.md) for per-agent limits and upstream
requirements; catalog inclusion is not proof that an adapter is usable locally.

Before running the installer, the backend:

1. Requires a complete executable search with no existing candidate. An existing,
   uninspectable or ambiguous candidate blocks installation.
2. Resolves a supported npm entrypoint and the specific Node executable, checks
   npm's package/bin mapping, and records their file digests. Unknown manager
   wrappers are rejected.
3. Queries that manager's global prefix and root, verifies the supported layout,
   and requires an existing inspectable prefix. Destination directories cannot
   redirect through links or unusable components.
4. Requires the package and launcher destinations to be absent, **including
   packages or launchers outside PATH**. An unreadable destination is not treated
   as absent. On Windows this covers npm launchers and executable-name collisions,
   not only the current PATHEXT result.
5. Rebuilds the plan immediately before execution and rejects changed evidence.

The command uses separate arguments, the inspected manager and an explicit
`--prefix`; it does not execute the registry command through a shell or create a
different installation prefix. It disables force, audit, funding and progress
options. Pi also uses `--ignore-scripts`; that flag is not applied to every agent.
The install subprocess has a five-minute runtime limit and bounded captured output.
Only one installation runs at a time within this process. This is not a lock
against another program, a publisher-signature check, or a sandbox for npm and
package lifecycle scripts. npm may download dependencies and write its own files.

After npm finishes, the backend rechecks the manager and requires one active,
versioned instance whose entrypoint, package, global root, prefix and manager match
the plan. A successful npm exit alone is not installation success. If the installer
fails, times out, or finishes but this postcheck fails, **software may already be
installed or partially changed**. AgentDeck reports the uncertainty and performs
**no automatic rollback or uninstall**. Refresh and inspect before retrying; an
existing package will block a fresh install rather than silently becoming an upgrade.

Installation does not initialize a profile, vault or Git backup and does not apply
configuration. The desktop action requires confirmation; the CLI requires `--yes`.
The ephemeral backend plan is rebuilt locally, not accepted from the UI as a
reusable authorization token or retained as proof of historical ownership.

## Configuration and synchronization

“Apply configuration” and `deploy --action configure --yes` never run an installer.
They require an existing supported profile adapter, successful fresh installation
verification and no instance ambiguity before preparing a profile or backup. The
core checks again before applying configuration. Missing, failed, unsupported or
ambiguous installation observations cannot authorize configuration. A unique,
verified instance may still have an unknown package association: that alone does
not disable a supported configuration adapter or prove an installation source.

The same eligibility rules guard personalization synchronization before profile
preparation and again before writing. CLI `sync` preflights every selected
personalization target before creating its backup or writing to any target.
Memory-only exports do not acquire an installation requirement. MCP's independent
offline configuration behavior is unchanged. Stale frontend data cannot bypass
backend checks. Disabled catalog adapters remain disabled; neither a source match
nor a successful installation enables them.

## Unavailable management and validation limits

Automatic uninstall and software upgrade remain unavailable, even for a matched
npm directory and even with CLI `--yes`. Use the original installation method for
those operations. Future same-source management requires its own reviewed plan;
the observation DTO and install-only plan do not authorize it. Durable installation
receipts and protection against every concurrent external change are not implemented.

Installation/source and configuration-guard tests use isolated temporary homes,
synthetic packages and launchers, and injected process results or installer callbacks.
They do not install real agents, validate a real login/provider, or establish a
Windows desktop GUI round trip. Do not run the ignored real-environment tests to
validate this increment. Platform-specific checks still require their native
platform; a passing Linux test does not establish Windows execution.

The source adapter boundary can later add native agents or DSH plugins without
claiming that every agent is an npm package. Native-plugin capability negotiation,
configuration reconciliation and package lifecycle management remain separate
design work, not fields inferred from the current registry.
