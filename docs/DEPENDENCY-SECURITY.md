# Rust dependency security review

Reviewed **2026-11-02** against the workspace `Cargo.lock`, using **cargo-audit 0.22.2** and RustSec database commit `263a4d951a0867d4bddc29dddd6d59427c78f9a7` (1,402 advisories; database timestamp `2026-11-02T12:45:44+01:00`). The post-remediation lockfile contains 520 dependencies across all supported target platforms.

## Result and remediation

The initial audit found **1 vulnerability and 6 informational warnings**. The final audit exits successfully with **0 reported vulnerabilities and 5 informational warnings**. Informational does **not** mean harmless: one remaining warning describes memory unsoundness in the Linux desktop dependency graph. This is not an all-platform clean-security claim.

The core YAML dependency was changed from `serde_yml 0.0.13` to `serde_norway 0.9.42`, retaining the `serde_yml` dependency alias to preserve existing call sites. Its parser dependency is `unsafe-libyaml-norway 0.2.15`. Both old `serde_yml` and `noyalib` packages are absent from the resulting lockfile. No application version was changed by this remediation.

Removed findings:

| Advisory | Previous dependency path | Classification and remediation |
| --- | --- | --- |
| [RUSTSEC-2026-0333](https://rustsec.org/advisories/RUSTSEC-2026-0333.html) | `agentdeck` / `aisync-cli` → `aisync-core` → `serde_yml 0.0.13` → `noyalib 0.0.5` | Denial-of-service/resource-budget enforcement vulnerability in typed YAML deserialization. Upstream fixes noyalib at `>=0.0.53`; this project instead removed that dependency chain. Default document-length, depth, and alias caps were not bypassed, but configured additional budgets were. |
| [RUSTSEC-2025-0068](https://rustsec.org/advisories/RUSTSEC-2025-0068.html) | `aisync-core` → `serde_yml 0.0.13` | RustSec unsoundness/unmaintained warning; the audit database lists no patched versions. Although the transitional package claims to replace its old implementation, it was still flagged by the actual audit. Replaced with a maintained alternative recommended by the advisory. |

Validation after the replacement: `cargo check -p aisync-core`; `cargo test -p aisync-core --locked` (**81 passed, 8 explicitly ignored manual/real-environment tests**); and `cargo clippy -p aisync-core --lib --locked -- -D warnings` all passed in an isolated cloud HOME. Tests include config YAML merge, MCP YAML toggle, and update validation. These are portable Linux test results, not a substitute for Windows-native CI.

## Remaining upstream findings

All five remaining packages are present in the all-platform lockfile but absent from the resolved `x86_64-pc-windows-msvc` application dependency graph. They remain relevant to Linux desktop builds.

| Advisory | Package | Dependency path / risk | Remediation direction |
| --- | --- | --- | --- |
| [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) | `glib 0.18.5` | `agentdeck` → Tauri / Wry / GTK 3 → `glib`. **Unsoundness** in `VariantStrIter` iterator implementations can cause undefined behavior/null-pointer crashes. | Fixed upstream at `glib >=0.20.0`. Current GTK 3/Tauri dependencies require the older GLib family; merely adding a second newer GLib package does not fix this path. Requires a compatible upstream desktop stack migration or an explicitly reviewed backport. **Unresolved for Linux.** |
| [RUSTSEC-2024-0370](https://rustsec.org/advisories/RUSTSEC-2024-0370.html) | `proc-macro-error 1.0.4` | Linux GTK macro build dependencies: `glib-macros` / `gtk3-macros` → `proc-macro-error`. **Unmaintained**. | Upstream GTK macro migration to maintained diagnostic tooling. No patched release is listed. |
| [RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html) | `zbus_names 4.3.5` | `agentdeck` → `tauri-plugin-opener 2.7.2` → `zbus 5.19.0` / `zbus_macros` → `zbus_names`. **Unmaintained standalone crate**. | Upstream opener migration to `zbus >=6`, where name types are integrated. |
| [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html) | `zvariant 5.15.1` | Same Linux opener/D-Bus graph. **Unmaintained standalone crate**. | Upstream opener migration to `zbus >=6`, where the wire module is integrated (or `zgvariant` for GVariant users). |
| [RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html) | `zvariant_derive 5.15.1` | Same Linux opener/D-Bus graph, through `zvariant` / macros. **Unmaintained standalone crate**. | Upstream `zbus >=6` migration. |

No advisory ignore list or vendored security patch was added. The Windows graph exclusion was checked with `cargo tree --locked --target x86_64-pc-windows-msvc -i PACKAGE` for **each** of the five packages; every command reported “nothing to print.” Also inspect `--target all` when reviewing Linux or another release target. This demonstrates dependency selection, not proof that a built binary has no vulnerabilities.

## Reproducible CI commands

```sh
cargo install cargo-audit --version 0.22.2 --locked
cargo audit --json > cargo-audit.json
```

Keep the audit command's exit code: it fails on known vulnerabilities. Publish the JSON report even on failure, and review informational warnings rather than suppressing them. A stricter Linux release gate can use `cargo audit --deny unsound`; it **currently fails** because of the unresolved GLib warning above. `--deny warnings` also fails on the remaining maintenance warnings. Do not describe the default successful exit as “no advisories.”

The tool additionally supports advisory applicability filters:

```sh
cargo audit --target-os windows --target-arch x86_64 --json > cargo-audit-windows.json
```

**Important:** these options filter advisories by their declared OS/architecture applicability; they do **not** resolve or prune Cargo's platform-specific dependency graph. Advisories without an OS restriction can still appear for packages unused on Windows. Use the independent `cargo tree --target x86_64-pc-windows-msvc` evidence above; do not globally ignore those advisories to make a Windows report appear empty.

The advisory database and dependency graph change over time. Re-run the audit against the committed lockfile for every release, especially before adding Linux distribution artifacts.
