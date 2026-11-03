# AgentDeck 1.1.0

This release hardens and completes the existing Windows desktop workflow without replacing the React/Tauri/Rust architecture.

## Changes

- Key-preserving, authenticated backup restoration; portable paths, deleted-file restoration, random backup nonces and legacy compatibility.
- Atomic configuration writes with rollback, preserved user instruction blocks, bounded installers and honest capability/warning states.
- Transactional signed registry updates, runtime registry loading and persisted installed version.
- Revocable memory exports, explicit manual selections, opt-in automatic collection (off by default), cancellation and partial-error reporting.
- Atomic managed-only MCP synchronization, working CLI commands and explicit read-only browser demo.
- Locked dependencies, isolated-home test runner, Windows-native tests and reproducible NSIS build artifacts.

## Verify this download

`SHA256SUMS.txt` identifies installer bytes. `BUILD_COMMIT.txt` identifies the exact source revision used by the release build. Compare both with the successful CI run for the release commit.

**The Windows installer is not Authenticode-signed.** Checksums are integrity evidence, not publisher certificate validation.

## Scope and known limitations

- Windows 10/11 x64 is the complete desktop target. Git is required for backups; npm-based agents need Node.js/npm.
- Linux core/CLI tests do not establish a functioning Linux desktop or secure non-Windows vault.
- Automated Windows tests and packaging do not constitute manual native GUI acceptance or testing every third-party installer/network/login.
- Real-environment integration tests are intentionally ignored; no real account secrets or personal conversations are used by ordinary CI.
- Memory exports are local Markdown; external agents require explicit context/configuration integration. Skill distribution is explicitly unsupported.
- Local memory databases and pre-write configuration recovery copies can contain plaintext. Git snapshots encrypt configuration copies; DPAPI vault keys are user/machine-bound.
- Legacy backup history retains its original deterministic ciphertext. Multiple-file rollback is not a power-loss journal.
- The original registry signing private key is not available in CI. No replacement key is generated, no new signed registry feed is claimed, and the existing trust anchor is preserved. Desktop binary upgrades are distributed by installer.

Dependency audit/remediation and remaining Linux-only upstream findings are documented in [DEPENDENCY-SECURITY.md](DEPENDENCY-SECURITY.md).
