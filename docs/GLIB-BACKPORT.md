# GLib 0.18.5 VariantStrIter backport

[简体中文](GLIB-BACKPORT.zh-CN.md)

## Scope and current status

This is a narrowly scoped source backport for [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) in AgentDeck's Linux GTK 3 dependency graph. It is reconstructed on commit `52ffa227e01fcf697ef3ee5d0ad888df1010e6c8`; it is not a recovery of a missing earlier Git commit. It does not upgrade GLib to 0.20, migrate the desktop stack, change AgentDeck's version, or establish that all dependencies are secure.

**Native validation is not yet established.** As of this reconstruction on 2026-11-04, the GLib regression tests have not run against a real native GLib installation in the dot cloud environment. That Debian 13 environment lacks the required development metadata. Its `apt-get update` attempt failed with permission denied reading `/etc/apt/apt.conf.d/80-applied-apt-retries` and a missing `/var/lib/apt/lists/partial`, including with formally requested execution approval. This prerequisite failure has not been bypassed. No successful Linux desktop build, GTK/WebKit runtime check, or remote CI run is claimed here. CI configuration and written tests are not test results.

The [earlier dependency review](DEPENDENCY-SECURITY.md) is historical context. Its GLib warning motivated this backport; the evidence and limitations below govern this change.

## Source provenance and exact change

- Original release: [official `glib 0.18.5` crate archive](https://static.crates.io/crates/glib/glib-0.18.5.crate).
- Archive SHA-256: `233daaf6e83ae6a12a52055f568f9d7cf4671dabb78ff9560ab6da230ce00ee5`.
- Upstream fix: [`b5a4071e439bef2b5eea76c3aa25e5ae84839e34`](https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34).
- Upstream merge: [`05dff0ee696f9bcd8617cd48c4b812d046d440cb`](https://github.com/gtk-rs/gtk-rs-core/commit/05dff0ee696f9bcd8617cd48c4b812d046d440cb).
- Vendored source: [`vendor/glib`](../vendor/glib), all 121 original archive files, preserving the published package version, manifests, `LICENSE`, and `COPYRIGHT`.
- Reviewable patch: [`vendor/glib-RUSTSEC-2024-0429.patch`](../vendor/glib-RUSTSEC-2024-0429.patch).
- Machine-readable provenance and file hashes: [`vendor/glib-backport.json`](../vendor/glib-backport.json).

Only two lines in `vendor/glib/src/variant_iter.rs`, inside `VariantStrIter::impl_get`, differ from the official archive:

```diff
-            let p: *mut libc::c_char = std::ptr::null_mut();
+            let mut p: *mut libc::c_char = std::ptr::null_mut();
@@
-                &p,
+                &mut p,
```

The C function writes the returned string pointer through this out parameter. The original code supplied a shared reference to an immutable pointer variable; a C write through that reference violates Rust's aliasing requirements. Optimizations can consequently leave the apparent value null, followed by undefined behavior when it is read as a C string. The backport makes the variable mutable and passes an exclusive mutable reference, matching upstream's correction. It does not change string ownership, lifetime, the public API, or the published package version.

## Ensuring the application uses the patched source

The workspace's `[patch.crates-io]` entry selects `vendor/glib` for the existing `glib 0.18.5` dependency. Merely adding a separate newer GLib dependency would leave the GTK 3 path affected. The application path to inspect is `agentdeck` → Tauri/Wry/GTK 3 → `glib`.

[`scripts/verify-glib-backport.py`](../scripts/verify-glib-backport.py) checks provenance, the complete vendor file set, and the exact two-line change. With full Cargo metadata, it also checks the resolved application graph reaches GTK and the patched GLib package. A regression crate depending on the patched copy is not, by itself, proof that the application does. Metadata created with `--no-deps` is insufficient and is rejected.

On a permitted Linux validation host, save the official archive locally and run:

```sh
python scripts/verify-glib-backport.py --archive /path/to/glib-0.18.5.crate
cargo metadata --locked --format-version 1 --filter-platform x86_64-unknown-linux-gnu > /tmp/agentdeck-glib-metadata.json
python scripts/verify-glib-backport.py --archive /path/to/glib-0.18.5.crate --metadata /tmp/agentdeck-glib-metadata.json
```

Metadata resolves the graph; it does not compile or execute the native code. Preserve the command output together with the exact tested commit when recording validation.

## Real FFI regression coverage

[`crates/glib-regression`](../crates/glib-regression) is an unpublished, edition-2024 workspace crate. Its exact `glib = "=0.18.5"` dev dependency is Linux-only. All 18 tests are inside `#[cfg(all(test, target_os = "linux"))]`, so a successful non-Linux workspace run does not establish GLib coverage.

The tests construct real `glib::Variant` string arrays and call `Variant::array_iter_str`; they do not replace GLib with a mock. They cover:

- `next`, `nth`, `last`, `next_back`, and `nth_back`, each reading ASCII, empty, and Unicode strings.
- Empty arrays and a singleton empty string, which must not be confused with an empty array.
- Out-of-bounds offsets, `usize::MAX`, a nonzero front index before overflow, and reverse underflow.
- Alternating ends, mixed skips, last after partial consumption, and repeated operations after exhaustion.
- Exact `len`, `size_hint`, and `count` before and after consumption, plus forward/reverse collection.

Every fixture exists only in memory. The crate does not read or modify real AgentDeck configuration, install or invoke an agent, access a network, spawn a process, initialize a desktop UI, or enable ignored real-environment tests.

On an authorized Linux host with real GLib development libraries and `pkg-config`, run all three profiles through the existing isolated-HOME harness:

```sh
CARGO_PROFILE_TEST_OPT_LEVEL=0 python scripts/run-isolated.py cargo test --locked -p glib-regression --lib
CARGO_PROFILE_TEST_OPT_LEVEL=2 python scripts/run-isolated.py cargo test --locked -p glib-regression --lib
CARGO_PROFILE_RELEASE_OPT_LEVEL=3 python scripts/run-isolated.py cargo test --locked -p glib-regression --lib --release
cargo clippy --locked -p glib-regression --all-targets -- -D warnings
cargo fmt --all --check
```

The planned Linux CI validation installs `libglib2.0-dev` and `pkg-config` on its GitHub-hosted runner, then runs the regression crate's Clippy check and these three test profiles. This is a separate permitted validation host, not a retry of the blocked cloud installation. The workflow must still be reviewed, run, and checked for the exact final commit before its result can be reported as passed.

The optimized profiles matter because this defect involves the assumptions an optimizer may make about a shared reference. Passing tests would provide regression evidence, not a proof that undefined behavior is impossible. Source provenance and the exact upstream fix remain essential evidence. Do not fabricate `pkg-config` metadata or use a prerequisite access failure as permission to change installation routes.

## Audit reporting must retain the warning

The backport intentionally retains version `0.18.5`. RustSec lists the upstream fixed release line as `glib >=0.20.0`; it does not automatically recognize this repository's source review. Conversely, **cargo-audit 0.22.2 omits path packages from its audit**, so losing the GLib warning after applying `[patch.crates-io]` is not evidence of remediation.

To keep the risk visible, the verification script can create a separate conservative audit lockfile. It uses the full current `Cargo.lock` and restores only the vendored GLib package's original crates.io source and checksum. It does not change the real build lockfile, remove current dependencies, upgrade packages, or suppress advisories. The output path must not already exist. That audit-only registry identity deliberately causes the published `0.18.5` advisory to remain visible; it is not the dependency source used to build the application.

```sh
set -eu
audit_dir=$(mktemp -d)
python scripts/verify-glib-backport.py --archive /path/to/glib-0.18.5.crate --audit-lock-output "$audit_dir/Cargo.lock"
audit_status=0
cargo audit --file "$audit_dir/Cargo.lock" --json > "$audit_dir/cargo-audit.json" || audit_status=$?
python scripts/verify-glib-backport.py --archive /path/to/glib-0.18.5.crate --audit-json "$audit_dir/cargo-audit.json"
# CI must publish the JSON even if this gate fails.
test "$audit_status" -eq 0
```

The JSON check requires the GLib `RUSTSEC-2024-0429` finding instead of treating its disappearance as success. Do not add an ignore entry. Interpret the conservative report together with the reviewed patch and actual native test results. Other vulnerabilities, yanked packages, and maintenance warnings still need their own review. A default successful audit exit is not an all-clear, and a strict `--deny unsound` check of this conservative report continues to fail by design.

## Remaining validation and removal criteria

1. Record successful provenance and full application-graph checks for the final commit.
2. Run the real Linux regression suite at test optimization levels 0 and 2 and release optimization level 3, and preserve the logs. Until then, the native result remains **not run**.
3. Preserve the conservative audit JSON and its exit status; confirm the GLib finding remains visible.
4. Treat a full Linux desktop build and runtime acceptance as separate work requiring the GTK/WebKit prerequisites and appropriate authorization. This focused GLib suite does not substitute for them or for Windows-native validation.
5. When a supported GTK/Tauri dependency migration removes the affected GLib line, verify the application graph and tests again, then remove the vendor patch, provenance checks, and audit-only identity restoration together. Do not remove them merely because an audit warning disappears.
