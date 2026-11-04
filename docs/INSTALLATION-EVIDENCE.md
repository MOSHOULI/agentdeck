# Installation evidence and management boundaries

English | [简体中文](INSTALLATION-EVIDENCE.zh-CN.md)

This increment adds read-only observations. It does not create an installation
receipt, prove who originally installed a program, or grant permission to remove
or upgrade a package. Registry installation recipes remain separate from facts
observed on the current machine.

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

Software installation remains separate from configuration and only proceeds from
a reliable `not_detected` result. Configuration checks supported adapters and fresh
instance ambiguity before preparing a profile or backup; the core repeats its
check before applying configuration. Stale frontend data remains non-actionable.

Automatic uninstall remains disabled at every layer, even for a matched npm
directory. Future same-source management needs a separately confirmed instance
and a backend plan that revalidates its manager, prefix, package, launcher and
entrypoint before acting. The observation DTO is not such a plan. Upgrade support,
durable installation receipts and protection against every concurrent external
change are not implemented here.

The source adapter boundary can later add native agents or DSH plugins without
claiming that every agent is an npm package. Native-plugin capability negotiation,
configuration reconciliation and package lifecycle management remain separate
design work, not fields inferred from the current registry.
