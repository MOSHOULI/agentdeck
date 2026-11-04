# AgentDeck 1.1.0

This release hardens and completes the existing Windows desktop workflow without replacing the React/Tauri/Rust architecture.

## Changes

- Key-preserving, authenticated backup restoration; portable paths, deleted-file restoration, random backup nonces and legacy compatibility.
- Atomic configuration writes with rollback, preserved user instruction blocks, bounded installers and honest capability/warning states.
- Transactional signed registry updates, runtime registry loading and persisted installed version.
- Revocable memory exports, explicit manual selections, opt-in automatic collection (off by default), cancellation and partial-error reporting.
- Atomic managed-only MCP synchronization, working CLI commands and explicit read-only browser demo.
- Locked dependencies, isolated-home test runner, Windows-native tests and reproducible NSIS build artifacts.
- Software installation is separate from optional profile configuration. Probe failures remain errors rather than absence; stale list data blocks further mutations. Unverified installation provenance blocks automatic uninstall in the UI and core.

## Verify this download

After the release gates pass, download the installer from the explicit [AgentDeck v1.1.0 release page](https://github.com/MOSHOULI/agentdeck/releases/tag/v1.1.0). A missing page means this installer has not yet been published.

`SHA256SUMS.txt` identifies installer bytes. `BUILD_COMMIT.txt` identifies the exact source revision used by the release build. Compare both with the successful CI run for the release commit.

CI first uploads a draft release, downloads its assets again, and verifies the installer bytes, checksums, build commit and actual Git tag commit before publishing. An existing release (including a draft) or a tag pointing to another commit stops publication for manual review; no existing assets are overwritten. A failed upload or verification can leave an unpublished draft that must be inspected before any retry.

**The Windows installer is not Authenticode-signed.** Checksums are integrity evidence, not publisher certificate validation.

## Installer and registry release channels

This installer-only release is published with `--latest=false`. The application's registry update check uses GitHub's `latest` download endpoint, which must continue to serve the existing signed registry manifest and signature. Use the explicit v1.1.0 link above for the desktop installer; the Latest label identifies the preserved registry feed, not the newest desktop binary. No new signed registry update is included because the original signing private key is unavailable. Signature and download failures remain errors, and the existing trust anchor is unchanged.

On upgrade, the application uses its improved bundled registries only when both retained runtime files exactly match the SHA256 fingerprints of the original signed v1.0.0 assets and their version marker is absent or `1.0.0`. This changes the selected source without deleting or rewriting those files or their marker. Custom, unknown and newer runtime registries remain in use. Signed updates must exceed both the effective installed registry version and the bundled version, so the old feed cannot reintroduce superseded metadata. Invalid version metadata, incomplete versioned installations and recovery failures remain errors.

## Scope and known limitations

- Windows 10/11 x64 is the complete desktop target. Git is required for backups; npm-based agents need Node.js/npm.
- No installation-origin ledger exists yet, including for new installations. Automatic uninstall is therefore disabled until provenance can be verified. Agent software upgrades are not implemented. A missing executable on PATH does not prove absence elsewhere on the machine.
- Applying a profile is a separate confirmed action, but configuration diff previews and three-way conflict reconciliation are not implemented. Existing rollback does not protect against every concurrent external edit.
- Linux core/CLI tests do not establish a functioning Linux desktop or secure non-Windows vault.
- Automated Windows tests and packaging do not constitute manual native GUI acceptance or testing every third-party installer/network/login.
- Real-environment integration tests are intentionally ignored; no real account secrets or personal conversations are used by ordinary CI.
- Memory exports are local Markdown; external agents require explicit context/configuration integration. Skill distribution is explicitly unsupported.
- Local memory databases and pre-write configuration recovery copies can contain plaintext. Git snapshots encrypt configuration copies; DPAPI vault keys are user/machine-bound.
- Legacy backup history retains its original deterministic ciphertext. Multiple-file rollback is not a power-loss journal.
- The original registry signing private key is not available in CI. No replacement key is generated, no new signed registry feed is claimed, and the existing trust anchor is preserved. Desktop binary upgrades are distributed by installer.

Dependency audit/remediation and remaining Linux-only upstream findings are documented in [DEPENDENCY-SECURITY.md](DEPENDENCY-SECURITY.md).
