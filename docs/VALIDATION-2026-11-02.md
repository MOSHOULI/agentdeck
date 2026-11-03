# Validation checkpoint — 2026-11-02

This is a cloud-only refactor checkpoint, **not a claim that a Windows release has already been built or published**.

## Passed against the final checkpoint source

- `cargo fmt --all --check`
- `cargo clippy --locked -p aisync-core -p aisync-cli --all-targets -- -D warnings`
- `python scripts/run-isolated.py cargo test --locked -p aisync-core -p aisync-cli`: **93 core + 3 CLI tests passed**, 8 intentionally ignored integration/diagnostic tests.
- Clean `npm ci --prefix web/ui`; TypeScript checking, **36 frontend tests**, production Vite build.
- Full `npm audit`: **0 vulnerabilities**.
- `cargo audit`: **0 vulnerabilities, 5 upstream Linux desktop warnings**. See [dependency report](DEPENDENCY-SECURITY.md), especially unresolved GLib unsoundness. Warning packages are absent from the Windows dependency graph.
- Isolated CLI `health`, `agents`, and unsupported-sync rejection checks.
- Actual root Tauri build invocation: Rust/JavaScript version compatibility and frontend build hook passed.
- YAML workflow parsing and `git diff --check` passed.

Regression coverage includes key-preserving restore, partial deletion, portable paths, symlink/traversal rejection, legacy encryption migration, failed-write rollback, update signature/integrity/schema checks, crash recovery, concurrent registry readers, bounded subprocesses, managed instruction preservation, memory revocation/manual selection/opt-in persistence and truthful UI failures.

## Not established

- Full native Tauri/IPC compilation, Windows DPAPI execution, controlled Windows timeout tests, and NSIS installer output require Windows CI.
- Cloud Linux native build stopped at missing `glib-2.0` development metadata (also lacks other Tauri desktop development prerequisites). Package prerequisite discovery encountered an environment permission denial; it was not bypassed.
- No native desktop GUI acceptance test, actual third-party installation/uninstallation or real-account authentication was performed. Browser screenshot launch was blocked by the environment.
- GitHub branch publication failed with HTTP 403 “Resource not accessible by integration”; CLI Git had no credentials. No PR, merge, remote CI result, installer or release is claimed at this checkpoint. Account-level admin metadata does not establish integration write access.
- Existing registry signing private key was not available and was not replaced. The installer release workflow does not create a newly signed registry manifest.

## Remaining release gate

Restore authorized GitHub write/workflow access, publish the checkpoint branch and draft PR, run all checks for the exact head commit, repair any native-only failures, then merge. Main CI repeats frontend/core/audit/Windows tests and builds NSIS. Release publication requires all jobs to pass, matching `BUILD_COMMIT.txt`, and verified installer SHA256. Verify the actual public release assets before declaring delivery complete.

Never run ignored real-environment tests as routine validation. Test-home isolation does not authorize third-party installers.
