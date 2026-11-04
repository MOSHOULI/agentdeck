# DSH passive inventory: first slice

English | [简体中文](DSH-PASSIVE-INVENTORY.zh-CN.md)

This increment is based on the installation-evidence work in PR #2, commit
`7f43da1f654307241eee1558c920a79f80b1dd69`. It adds a core reader and a separate CLI
entry point. It does not add DSH to the generic deployment registry, provide a
desktop management screen, or enable installation, configuration, update,
downgrade, removal, authentication, or runtime connections.

This is source-branch functionality, not a claim about an already published
installer with the same version string. Check that installer's `BUILD_COMMIT.txt`
against the source revision before assuming the command is included. DSH itself
does not need to be installed or started to inspect synthetic or copied manifests.

This first slice is exposed through the source-built `aisync` CLI, not a desktop
button. Installing the NSIS desktop package does not by itself install this CLI
command. With this repository and its documented Rust prerequisites, for example:

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

The reviewed upstream snapshot is
[`8bd073392eef6f54e921a093b95076248eb08cbb`](https://github.com/deepseek-ai/deepseek-harness/commit/8bd073392eef6f54e921a093b95076248eb08cbb),
whose CLI is `0.2.2-alpha.3`. The default upstream branch was `master` when
reviewed. Preview interfaces can change; a later version needs fresh contract
review rather than a broad automatic compatibility assumption.

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

Windows-specific path, reparse and handle checks need native Windows validation;
passing portable Linux tests does not establish those checks have run. This slice
does not claim a Windows GUI or real DSH integration test.

Filesystem validation is conservative, but it is not an OS sandbox against a
same-privilege process replacing files concurrently, nor a guaranteed timeout
for every filesystem operation. Reading a regular file may update filesystem
access metadata. No configuration/content writes are requested.

Desktop profile ownership, authenticated runtime transport, actual plugin
activation, credential handling, and mutation plans remain separate future
work. A static manifest cannot authorize those operations. The existing glib
advisory is not modified or resolved by this feature.
