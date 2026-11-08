# DSH passive inventory: desktop and CLI

English | [简体中文](DSH-PASSIVE-INVENTORY.zh-CN.md)

The development branch exposes one passive core reader through the new desktop
workspace, “DSH 声明盘点” (“DSH declaration inventory”), and a separate `aisync` CLI
command. This brings the desktop to six workspaces. DSH is now also listed in the main catalog. Its dedicated button opens this workspace without scanning or changing the current inputs. Generic installation/configuration remains disabled. This is an inventory
view, not a plugin manager: installation, configuration, plugin activation, update,
downgrade, removal, authentication and runtime connections remain unavailable.

These features are on the development branch and are not in the published v1.1.1 installer. The next installer is deferred until a larger validated update.
Confirm publication on the versioned Release page and compare its `BUILD_COMMIT.txt`
with the source revision. A version string alone does not identify the included
source. DSH itself does not need to be installed or started to inspect synthetic
or copied manifests.

## Desktop usage

In a source-built desktop app, open **DSH 声明盘点**. Enter the absolute path of the
existing profile directory you want to inspect. Optionally enter a separate
runtime `package.json` absolute path, then click **盘点所选声明** (“Inspect selected
declarations”). Opening the workspace or editing an input does not start a scan.
There are no default paths, and the selections are not persisted.

The desktop invokes the dedicated `inspect_dsh_profile` IPC command, which validates
both selections before reading either file and delegates to the same passive reader
as the CLI. It does not invoke the `aisync` or DSH CLI. Changing either path clears
the previous report; repeated submits are blocked while a read is in progress, and
responses for an older selection are discarded. Switching workspaces does not
require restarting the inspection.

The view separates file observations, saved dependencies and bundle selections
from runtime, authentication and actual-enabled states, which always remain
**unknown**. Every mutation capability is **disabled**. Incompatible responses
are rejected rather than presented as partial success. An ordinary browser cannot
read local paths; explicit `?demo=1` displays labeled synthetic declarations and
does not read the input paths or any local files.

## CLI usage

Installing an NSIS desktop package does not by itself install the `aisync` command.
With this repository and its documented Rust prerequisites, for example:

```powershell
cargo run --locked -p aisync-cli -- dsh inspect --profile-dir "C:\DSH\profiles\web"
# Optional, separately selected runtime manifest; no runtime discovery or execution
cargo run --locked -p aisync-cli -- dsh inspect --profile-dir "C:\DSH\profiles\web" --runtime-manifest "C:\DSH\runtime\package.json"
```

Replace the example paths with the existing directories you explicitly want to
inspect. Missing paths produce a report and are not created.

## Explicit input, static evidence

Use `aisync dsh inspect --profile-dir ABSOLUTE_DIRECTORY`. An optional
`--runtime-manifest ABSOLUTE_PACKAGE_JSON` selects a second manifest explicitly.
There is no default profile, HOME scan, PATH probe, automatic runtime discovery,
or directory creation. Relative paths and parent-directory components are not
accepted. This narrow local-file interface does not accept Windows UNC/device
paths.

Only the selected profile's `package.json` and the explicitly selected runtime
`package.json` are eligible inputs. The reader does not open `cordis.yml`,
credentials, environment files, npm configuration, lockfiles, plugin code, or
other profile files. It does not invoke a shell, DSH, Node, npm, pnpm, a DSH
profile loader, or a runtime API. It does not import JavaScript or parse YAML.

The report separates missing, corrupt, unreadable, unsupported and read
manifest observations. It limits input and collection sizes, omits arbitrary
manifest properties and raw parse errors, and does not return credential-bearing
dependency URLs as raw specs. Saved dependencies do not prove that their files
exist, and saved bundle selections do not prove that plugins are enabled or
active in a running host. Runtime, actual-enabled and authentication states stay
unknown. No configured defaults are substituted for missing declarations.

This is a conservative subset of the upstream manifest format. Unsupported names,
shapes or schema versions can reject a manifest instead of yielding a partial
inventory. Such rejection is not evidence that its packages or runtime are absent.
Upstream types bundle selections as strings; this slice emits only lowercase
npm-shaped package names. Other upstream strings may be legitimate but unsupported
here, rather than invalid DSH metadata. No path or export selector is resolved.

Current limits are two manifest files, 65,536 bytes per file, JSON depth 16,
4,096 JSON entries, 128 dependency declarations across supported sections and
64 saved bundle selections. Duplicate decoded JSON keys are rejected. Only
`dependencies`, `optionalDependencies` and `peerDependencies` are inventoried;
development dependencies and package contents are not resolved. Links, reparse
points, multiple hard links and nonregular files are rejected conservatively.
That can reject legitimate linked package layouts; it must not be reported as
not installed. Native file reading is supported only on Windows and the reviewed
Linux x86_64/aarch64 path; other targets return unsupported.

Dependency specification kinds are only lexical classifications. They do not
validate full npm semver syntax, establish that a range resolves, or check plugin
compatibility. Reported Node engine text is likewise an unverified declaration.

Version recognition refers to a manifest declaration, not an executed version
probe, verified publisher identity, or proof that a runtime uses this profile.
An unknown or newer version cannot grant write capabilities. Recognized versions
also have every mutation capability disabled in this first slice.

Exit code 0 means that an inspection report was produced, including a report
containing missing or unknown evidence. It does not mean installed, healthy, or
authenticated. Invalid CLI arguments fail before inspection. Output failures
are errors.

## Fixed upstream reference

The supported schema subset was reviewed against upstream snapshot
[`8bd073392eef6f54e921a093b95076248eb08cbb`](https://github.com/deepseek-ai/deepseek-harness/commit/8bd073392eef6f54e921a093b95076248eb08cbb),
whose CLI is `0.2.2-alpha.3`. That runtime manifest version, under the expected
`@deepseek-ai/dsh` or `@deepseek-ai/dsh-app-boot` name, receives the “reviewed manifest
version” label. This is still declaration recognition, not a compatibility test.

A separate **2026-11-06** check of the official default branch, pinned at
[`fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159`](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/cli/package.json),
found CLI `0.2.2-rc.1` and Node `>=24.9.0` in its manifest. The **2026-11-08** review additionally checked the same `dsh.manifestVersion: 1`,
partial profile initialization and ordered bundle declaration subset at this fixed
commit, plus both CLI and app-boot package names/versions. Exact `0.2.2-rc.1`
declarations now receive the reviewed-manifest label; the parser is not widened.
This does not verify the published npm artifact, installed runtime, profile ownership
or plugin compatibility. Later versions remain unrecognized; no version enables
management operations. The desktop bundles its own runtime: do not replace its CLI
registration with a global npm install.

- A profile manifest is partial and need not have a package version.
- `dsh.manifestVersion`, when present, describes the manifest schema and is
  independent of runtime/package and stored-session versions.
- `dsh.profile.bundles` is a saved ordered list, not a runtime status report.
- The CLI version implementation reads the app-boot package's manifest. Reading
  a package file here remains a static declaration, not a measured CLI version.

Sources: [manifest types](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/packages/util/package-manifest/src/types.ts#L7-L84)
and [profile initialization](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/packages/boot/app-boot/src/profile.ts#L198-L216).

## Why not call a list command?

At the reviewed snapshot, `dsh plugin list` can initialize a profile, create
operation logs and reconcile bundle selections. Configuration dumps prepare
profiles and rewrite configuration; schema dumps can import plugin code.
Even the native plugin-manager list operation can enter a profile loader that
removes retired bundles. These are not substitutes for a zero-write passive
reader. This implementation does not call them.

Sources: [plugin command setup](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/packages/boot/plugin-manager/src/operations.ts#L565-L577),
[profile preparation](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/apps/cli/src/profile-boot.ts#L194-L198)
and [native list](https://github.com/deepseek-ai/deepseek-harness/blob/8bd073392eef6f54e921a093b95076248eb08cbb/packages/boot/plugin-manager/src/index.ts#L465-L480).

## Validation and remaining boundaries

Tests use synthetic manifests and temporary directories, with no real DSH
installation, package-manager execution, or user configuration. Read-boundary
tests reject unapproved filenames, malformed and oversized inputs, and unsafe
path kinds. They check that only the allowed manifests are requested and that
content and modification times stay unchanged. The production reader has no
process runner or writer. CLI tests use fake PATH sentinels to detect accidental
DSH/Node/package-manager calls; these are not syscall-wide tracing or an OS
sandbox. Existing real-environment tests remain ignored.

Desktop IPC tests use temporary manifests; frontend tests use synthetic reports
and mocked IPC to cover explicit submission, repeated clicks, input changes,
stale responses, navigation, errors and demo/browser boundaries. They do not
establish that a real desktop GUI was launched or that real DSH was integrated.
Windows-specific path, reparse and handle checks need native Windows validation;
passing portable Linux tests does not establish those checks have run.

Filesystem validation is conservative, but it is not an OS sandbox against a
same-privilege process replacing files concurrently, nor a guaranteed timeout
for every filesystem operation. Reading a regular file may update filesystem
access metadata. No configuration/content writes are requested.

The no-content-write boundary applies to this inspection, not to every operation
or startup activity of the desktop application. This IPC path does not load the
registry, initialize AgentDeck's home, create profiles or prepare configuration.

Profile ownership, authenticated runtime transport, actual plugin activation,
credential handling and mutation plans remain future work. A static manifest cannot
authorize those operations. This feature does not change the GLib backport or its
remaining advisory/validation limits; see the [GLib guide](GLIB-BACKPORT.md).
