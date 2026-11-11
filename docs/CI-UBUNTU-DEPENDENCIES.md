# Signed Ubuntu dependency setup in CI

The Linux core/GLib job and Linux WebView matrix use Ubuntu 24.04 amd64.
`install_ci_ubuntu_dependencies.py` installs only three fixed dependency groups
(`tracer`, `glib`, `webview`) on GitHub-hosted runners with this exact OS/architecture.
A new runner release or architecture requires review rather than reusing Noble packages.

Two attempts of PR #77 CI run `43036941206` failed because the runner's configured
`azure.archive.ubuntu.com` mirror could not resolve. The helper uses canonical
`https://archive.ubuntu.com/ubuntu` and `https://security.ubuntu.com/ubuntu` directly,
with the installed Ubuntu archive keyring explicitly required for both sources.
It never edits `/etc/apt`, replaces keys, relaxes signature/TLS checks, changes DNS,
or substitutes a third-party mirror. Package names and existing verification
steps remain unchanged; this is not an AgentDeck or DSH installer.

Each invocation creates its own sources and initially empty package-list directory.
Only those sources/lists are used by its apt commands; APT binary metadata caches
are disabled. Any reused `.deb` archive still must match the new signed index's
hash. `APT::Update::Error-Mode=any` makes a partially failed update fatal,
so old indexes cannot silently allow an install. apt's normal authenticated archive
hash checks remain in force. This does not pin packages to a historic version;
versions come from the current signed Noble indexes and retain existing package
version checks/reporting in CI.

apt performs at most two acquisition retries and uses 20-second HTTP/HTTPS
inactivity timeouts. A root-owned GNU `timeout` bounds update to 180 seconds and
install to 600 seconds, with a 15-second TERM-to-KILL grace period for each.
The workflow adds a 14-minute step limit. The helper never retries an entire
package transaction or continues after a failed/timed-out update or installation.
A killed install may leave this disposable runner's dpkg state incomplete; the
job fails instead of trying to repair or claim success. The normal job timeout
also remains in effect.

The unique temporary apt directory is retained for the ephemeral runner's lifetime
because apt owns some generated list files. No privileged recursive cleanup is
introduced. Offline tests mock subprocesses and prove the fixed commands, reviewed
context guards, source/authentication contract, fresh index setup and stop-on-error
behavior. They do not establish network reachability or native test success:
exact-commit CI must still run GLib and both WebView variants successfully.
No isolation gate, release gate, or installer-publication behavior changes.
