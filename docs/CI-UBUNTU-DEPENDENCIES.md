# Bounded Ubuntu dependency setup in CI

The Linux core/GLib job and Linux WebView matrix use Ubuntu 24.04 amd64.
`install_ci_ubuntu_dependencies.py` installs only three fixed dependency groups
(`tracer`, `glib`, `webview`) on GitHub-hosted runners with this exact OS/architecture.
A new runner release or architecture requires review.

## Source selection and observed network failures

The helper uses the hosted runner's existing apt sources and trust configuration,
just as the original CI did. It does not replace sources, mirror selection,
keyrings, package-list paths, cache paths, DNS or system network settings. It does not
add a third-party mirror, disable authentication or relax TLS verification.
The Ubuntu archive keyring must still be present. Package names and existing
verification steps remain unchanged; this is not an AgentDeck or DSH installer.

PR #77 run `43036941206` attempts 1/2 failed when the runner's configured
`azure.archive.ubuntu.com` mirror could not resolve. An attempted replacement
with canonical `archive.ubuntu.com` and `security.ubuntu.com` sources also failed
DNS resolution in run `43055878985` attempts 1/2. Meanwhile, #78's main-based run
`43065200620`, Linux core job `130678174612`, fetched fresh Ubuntu indexes and
GLib packages through the runner's default mirror at 17:09–17:10 UTC on
2026-11-11 without DNS errors; both WebView dependency setups also passed.
That evidence supports restoring the working runner configuration, not forcing
the canonical hosts or claiming to repair DNS. Network failures still fail CI.

## Fail-closed update and bounds

Every invocation must finish `apt-get update` successfully before installation.
`APT::Update::Error-Mode=any` makes a partially failed update fatal, so apt cannot
proceed to install just because an older index remains available after an update
error. Existing lists and cached archives are not isolated or cleared; normal
signed-index and archive hash checks remain in force. Versions come from the
runner's configured sources, not a historic package pin, and existing package
version reporting/checks remain in CI.

apt performs at most two acquisition retries and uses 20-second HTTP/HTTPS
inactivity timeouts. A root-owned GNU `timeout` bounds update to 180 seconds and
install to 600 seconds, with a 15-second TERM-to-KILL grace period for each.
The workflow adds a 14-minute step limit. The helper never retries an entire
package transaction or continues after a failed/timed-out update or installation.
A killed install may leave this disposable runner's dpkg state incomplete; the
job fails instead of trying to repair or claim success. The normal job timeout
also remains in effect.

Offline tests mock subprocesses and prove fixed commands, reviewed context
guards, preservation of source/trust configuration and stop-on-error behavior.
They do not establish network reachability or native test success: exact-commit
CI must still run GLib and both WebView variants successfully. No isolation gate,
release gate or installer-publication behavior changes.
