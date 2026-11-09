# AgentDeck (AI Hub)

[简体中文](README.md) | English

This document describes the ongoing development source: installation-instance and source evidence, restricted npm installation, catalog corrections, and desktop/CLI DSH passive inventory. The published v1.1.1 installer does not include these increments. Confirm publication on the versioned Release page and compare `BUILD_COMMIT.txt` with the successful release CI commit. v1.2.0 was not released separately. The next installer will follow a larger, verified update; its version is not yet selected. See the historical [bilingual v1.2.0 preparation notes](docs/RELEASE-1.2.0.md).

This development branch also includes the [MCP toggle preflight and backup-order fix](docs/MCP-SAFETY.md), which has not shipped in a public installer. Validation uses synthetic fixtures only, not real agents or the desktop GUI.

**Next-development-version note:** This branch adds Qwen Code manual-install guidance and passive PATH candidate observation, not yet shipped in a public installer. A candidate does not establish a verified installation or login; Qwen Code automatic installation, runtime/source verification and general configuration writes remain unavailable; only the experimental CLI Vim preference apply/restore described below is available. See the [catalog source review](docs/REGISTRY-SOURCES.md#qwen-code-manual-installation-and-passive-path-discovery-only).

The catalog adds six [official manual-guidance entries](docs/MAINSTREAM-CATALOG.md) for Cursor, GitHub Copilot CLI, Kimi Code and Qoder CLI, separating CLI and desktop surfaces. Its 16 entries do not imply 16 managed integrations. New entries execute no probes and write no configuration; they have not shipped in a public installer.

Manage local AI agents from one desktop interface: discovery and installation, configuration, optional conversation-memory exports, encrypted keys, MCP, and configuration backups.

**Windows 10/11 x64 is currently the fully supported desktop platform.** AgentDeck uses React + Tauri 2 + Rust. Linux can run the core/CLI features that do not involve keys; a protected vault and complete installers are not yet available for Linux/macOS.

The catalog can filter currently available automatic installation, manual-only entries and observations needing confirmation. See [catalog filter guidance](docs/CATALOG-FILTERS.md).

Desktop MCP toggles now offer a [change preview](docs/MCP-PREVIEW.md): review the target and action before confirming backup/write; stale previews are rejected. This summary is not a full configuration diff and has not shipped in a public installer.

## Installation and startup

1. Select a published version and download its installer from the [AgentDeck releases](https://github.com/MOSHOULI/agentdeck/releases), and verify it against `SHA256SUMS.txt` from the same release. Publication requires the CI gates described below. CI test artifacts are not public releases.
2. Install [Git for Windows](https://git-scm.com/downloads/win) and ensure `git` is on PATH; encrypted backups require it. npm-based agents also require Node.js/npm.
3. The installer is currently **not Windows Authenticode-signed**. SHA256 verifies integrity; it is not a signature establishing publisher identity.

### Build from source (Windows)

Requirements: Rust stable (MSVC), Node.js 24 LTS, Git, Microsoft C++ Build Tools, and WebView2.

```powershell
git clone https://github.com/MOSHOULI/agentdeck.git
cd agentdeck
npm ci --prefix web/ui
npm exec --prefix web/ui -- tauri dev
# The NSIS installer is written to target/release/bundle/nsis/
npm exec --prefix web/ui -- tauri build --bundles nsis --ci -- --locked
```

A global Tauri CLI installation is not required. Both Rust and npm dependency lockfiles are committed to the repository.

## Usage

The v1.2.0 source has six desktop workspaces: agent management, memory management, MCP services, keys and backups, DSH declaration inventory, and registry updates. The sidebar is collapsible, and the theme can be light, dark, or system-controlled. Navigation does not interrupt ongoing operations; their status remains visible in the top bar and sidebar. Agent search filters only the actual current list, and unknown states in the details view are never replaced with success.

- **Agent management:** Installation requires confirmation and installs only software, without initializing a profile, vault or Git backup, or applying configuration. Only fixed npm adapters using an inspected manager/prefix are supported; arbitrary registry shell recipes are not executed. Detection distinguishes verified, not found on PATH, probe failure and unable to probe. PATH absence does not mean whole-machine absence. Existing packages or launchers in the target prefix block installation even when off PATH. Incomplete, conflicting or changed evidence blocks the operation rather than causing an implicit reinstall or upgrade. If the installer or post-install verification fails, software may already have changed; **there is no automatic rollback or uninstall**. Operation completion and subsequent list-refresh failures are reported separately; stale status blocks write operations.
- **Installation instances and source evidence:** PATH candidates, canonical file paths, provable actual entrypoints, and the measured version of the active instance are shown separately. Duplicate PATH aliases are not counted as separate installations. Distinct instances or incomplete inspection are explicitly reported and block configuration. PATH search is not a whole-machine scan; undetected installations are not assumed absent.
- **npm association and uninstall boundaries:** The UI shows “npm 全局目录匹配” (“npm global directory match”) only when the exact npm instance's global root/prefix, package name/bin, link or supported standard Windows shim, and actual entrypoint all agree. This is current directory-association evidence only. It **does not prove installation history or authorize uninstalling**. Custom wrappers, conflicting observations, or insufficient evidence remain unknown. Neither the registry's installation recipe nor the version in package.json is used to infer provenance or a measured version. Automatic uninstall and agent software upgrades remain unavailable, including for instances with a matched npm directory. Use the original installation method to manage them.
- **Personalization and model configuration:** The default profile is applied only after confirmation through the separate “应用配置” (“Apply configuration”) action, which does not run an installer. Configuration and personalization sync require a supported adapter and fresh, successful, unambiguous installation verification before preparing a profile/backup, with another check before writing. Unknown package association alone does not disable a verified, unambiguous instance, or grant uninstall/upgrade permission. Templates are initialized on first configuration, preserving existing user templates. Personalization uses managed Markdown blocks and preserves text outside those blocks; incompatible target formats are not forcibly overwritten. Reasons for skipped items are reported separately. Configuration diff previews and three-way reconciliation of local changes are not yet available. Review templates and existing configuration before applying them.
- **Memory:** Collect local sessions from Codex, Claude Code, and OpenCode. Search, manual selection, and automatic-injection markers are managed separately. Revoking automatic markers immediately refreshes the export; explicit manual selections are retained. Exports are written to `~/.agentdeck/memory/<agent>.md`. **External agents are not guaranteed to read this file automatically**; reference it in their context or configuration. Scheduled collection is off by default and reads local sessions hourly only after explicit opt-in. It can be disabled at any time, and manual collection remains independent. Collection supports cancellation, a time budget, and partial-failure reporting.
- **Keys and backups:** The vault uses AES-256-GCM, with its master key protected by Windows DPAPI. Lists show only masked values; plaintext is retrieved only when copying a key. Configuration backups in Git snapshots are encrypted with random nonces, and older snapshots remain decryptable. Restoration preserves the master key and unmanaged files.
- **MCP:** Only registry-managed servers are changed; user-created servers are preserved. Batch changes are validated before a single write. HTTP servers must be started separately; AgentDeck does not host RAGFlow.
- **DSH declaration inventory:** In the new workspace, enter an explicit absolute profile-directory path and optionally a runtime `package.json` path. Only clicking Inspect reads the selected bounded manifests, at most two files. Paths are not persisted. Inspection does not scan HOME, create directories, run DSH/Node/package managers, read credentials, `cordis.yml` or plugin code, or write configuration. Saved declarations do not prove installed software or enabled plugins. Runtime, authentication and actual-enabled states always remain unknown; all management mutations are disabled. Ordinary browsers cannot read local files, and explicit demo mode returns synthetic examples only.
- **Online updates:** Updates apply only to the agent/MCP registries, not the desktop binary. Signatures are verified using the built-in public key, all files are validated before committing the update, and the registry version is persisted. Download a new Release to upgrade the application.

Opening the frontend directly in a browser does not simulate successful operations. Only `?demo=1` explicitly enables a clearly labeled **read-only demo**, which prohibits installation, configuration writes, and key access.

See [Installation evidence and management boundaries](docs/INSTALLATION-EVIDENCE.md) for observations, install plans and configuration guards. [Registry source verification](docs/REGISTRY-SOURCES.md) covers upstream requirements and disabled adapters: Pi/Hermes configuration writes remain disabled; OpenCode MCP/model writes are disabled even when an older retained runtime registry describes those adapters; its supported Markdown instruction-file adapter remains. `opencode-ai` is a V1 installation route, not V2 installation or migration support. Unverified star counts, recommendation flags and ranking claims have been withdrawn from the bundled catalog; it does not certify a product ranking. See [DSH passive inventory](docs/DSH-PASSIVE-INVENTORY.md) for desktop path selection and version limits.

## Data and recovery boundaries

Data lives in `~/.agentdeck/`: `memory.db`, `memory/`, `vault/`, `profiles/default/`, `backups/`, `git-backup/`, and `registry/`.

- Local sessions and the memory database are plaintext. Local recovery copies made before configuration writes may also contain credentials. Protect your account and disk, and do not share the entire data directory. Only configuration copies included in Git snapshots are encrypted; vault API keys are not written to disk in plaintext.
- Backup restoration covers **the vault and configuration copies managed by AgentDeck**. It is not a system restore, does not uninstall/reinstall software, and does not automatically copy backups back over external agents' configuration paths.
- Do not delete `vault/master.key`. DPAPI keys are bound to the Windows user/machine; copying only vault.json or the Git repository does not provide a recoverable cross-machine backup. If encrypted history exists but the master key is missing, the application refuses to silently recreate it.
- Multi-file configuration changes and restoration support rollback on error, but do not guarantee atomicity during a power loss. If an operation fails, preserve the data directory and inspect the error; do not delete recovery copies.
- Historical v1.0.0 Git commits retain the old encryption format. New snapshots use random nonces, but cannot remove the risk from old history that has already been shared.
- Skill distribution has no adapter and explicitly reports “不支持” (“unsupported”); it does not report success for an empty implementation.

## CLI

```powershell
cargo run --locked -p aisync-cli -- health
cargo run --locked -p aisync-cli -- agents
# DSH static inventory: only an explicitly selected existing directory; no DSH startup or profile creation
cargo run --locked -p aisync-cli -- dsh inspect --profile-dir "C:\DSH\profiles\web"
cargo run --locked -p aisync-cli -- deploy --agent codex --action verify
cargo run --locked -p aisync-cli -- deploy --agent codex --action install --yes
# Optional separate configuration action; requires Git backups and protected-vault support
cargo run --locked -p aisync-cli -- deploy --agent codex --action configure --yes
cargo run --locked -p aisync-cli -- sync --tools codex,claude-code --dry-run
cargo run --locked -p aisync-cli -- sync --tools codex,claude-code --yes
cargo run --locked -p aisync-cli -- backup
cargo run --locked -p aisync-cli -- backups
cargo run --locked -p aisync-cli -- restore <timestamp> --full --yes
```

Installation, configuration, synchronization and restoration require `--yes`. Installation does not implicitly write configuration and only executes a fixed npm plan whose target checks pass. `deploy --action verify` reports instance and source observations. Configuration and personalization sync check installation eligibility before preparing a profile/backup and again before writing. CLI `sync` checks every personalization target first; any ineligible target prevents backup and writes from starting. Memory-only exports do not require installation. Automatic uninstall and agent software upgrade remain unavailable; `--yes` does not override that boundary. Protected-key and backup commands are Windows-only; software-only installation does not depend on that backup workflow.

`dsh inspect` uses the same passive reader as the desktop DSH workspace, returns only JSON and does not enter the deployment workflow above. It reads only the selected profile's `package.json`; `--runtime-manifest` can explicitly select an additional runtime package's `package.json`. It does not scan HOME, invoke DSH/npm/pnpm, or read credentials, `cordis.yml`, or plugin code. Package dependencies and bundle selections are saved declarations only; versions are manifest-reported values. They do not establish that software is installed, plugins are running, or authentication has succeeded. Exit code 0 means only that a report was produced; inspect the JSON for missing, unsupported, or unknown states. The parser subset was reviewed against fixed `0.2.2-alpha.3` and `0.2.2-rc.1` snapshots; exact name/version recognition is not runtime compatibility or npm publication verification. Management writes remain disabled for all versions. See [DSH passive inventory boundaries](docs/DSH-PASSIVE-INVENTORY.md).

The CLI supports a [read-only sync plan](docs/SYNC-DRY-RUN.md): `sync --tools codex,claude-code --dry-run` reports target/action metadata without executing probes, initializing data, or modifying files. It is not a diff or verified apply readiness.

## Tests and releases

```powershell
npm ci --prefix web/ui
npm run typecheck --prefix web/ui
npm test --prefix web/ui
npm run build --prefix web/ui
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
python scripts/run-isolated.py cargo test --locked --workspace
```

Linux core checks: `python scripts/run-isolated.py cargo test --locked -p aisync-core -p aisync-cli`.

The reviewed two-line GLib backport, official source checks and three-profile native regressions remain part of this branch. This does not establish full Linux desktop or GUI support, or remove other maintenance warnings. See the [English GLib guide](docs/GLIB-BACKPORT.md) / [中文说明](docs/GLIB-BACKPORT.zh-CN.md).

The test runner sets `AGENTDECK_TEST_HOME`, and core path resolution explicitly respects this absolute path. The Windows Known Folder API is not affected by ordinary HOME variables.

**All tests use an isolated HOME. Do not run `--ignored` or `--include-ignored`.** The eight ignored manual/real-environment tests may install/uninstall third-party software, access sessions, or modify configuration. Normal CI uses only synthetic fixtures. The current installation-safety and DSH increments have not been validated through a real agent installation, desktop GUI workflow or real DSH execution; mocked IPC, temporary manifests and fake-process tests do not replace those checks.

CI validates the frontend, Linux core/CLI, and the full Windows workspace (including DPAPI), builds the NSIS installer, and records its SHA256 and source commit. Pull requests and merges to main do not automatically publish an installer. Publication requires manually triggering the workflow on main with `publish_release` explicitly enabled and passing all validation gates. CI then uploads a draft Release, downloads it again, and verifies the installer bytes, SHA256, build commit, and actual tag commit. It publishes only if all match. An existing Release, including a draft, or a tag pointing to another commit blocks publication for manual review; it is not overwritten or skipped as a successful publication.

Installer releases and signed registry updates are separate. Registry checks currently use GitHub's `latest` download endpoint, so the installer-only v1.2.0 release is not marked Latest, preserving the existing signed v1.0.0 registry feed and historical releases. Use the v1.2.0 link above for the newer desktop installer; do not treat the Latest label as the newest desktop binary.

On upgrade, the application switches to its newer bundled registries only if both local registry files exactly match the SHA256 fingerprints of the official signed v1.0.0 assets and the version marker is v1.0.0 or absent. The original files and version marker are left unchanged. Modified, unknown-source, or newer local registries are retained and remain in use. The core package version also sets the bundled registry floor, now 1.2.0; this does not mean a signed registry with that version has been published. An online update must be newer than both the effective current version and the 1.2.0 floor. Older or equal-version updates cannot overwrite the bundled fixes. The signing public key and external update feed are unchanged.

Publisher registry signing uses the existing protected private key. That key is not included in this repository and is not needed to build an installer. **Without the private key, a new trusted registry update cannot be published.** Do not generate a new key and present it as part of the existing trust chain. See the [bilingual release notes](docs/RELEASE-1.2.0.md) for the detailed validation scope.

### Qwen settings inspection

Explicit, version-gated read-only CLI inspection is documented in [Qwen configuration inspection](docs/QWEN-CONFIG-INSPECTION.md). It is not configuration switching or restoration. A separate [experimental owned Vim preference](docs/QWEN-OWNED-SETTING.md) supports one field with explicit paths and an exact version; full runtime effectiveness is not verified.
