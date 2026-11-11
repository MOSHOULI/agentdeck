# Frozen installer Node minimum policy

The four existing fixed npm installation adapters now measure the **installer's**
frozen native Node executable with the direct argv `--version`. This is a bounded
local process probe, not a shell command, npm query or arbitrary registry dependency
recipe. The existing shared runner timeout/output limits and inherited Node preload
variable removal apply. A strict stable `vMAJOR.MINOR.PATCH` output is required;
malformed, prerelease, overflowing, failed or timed-out measurements block installation.

AgentDeck's fixed minimum policy is:

- Codex (`@openai/codex`): 18.0.0.
- OpenCode V1 (`opencode-ai`): 18.0.0.
- Gemini CLI (`@google/gemini-cli`): 20.0.0.
- Pi (`@earendil-works/pi-coding-agent`): 22.19.0.

These are **AgentDeck installer policy minima**, independent of user-editable
registry strings. They are not a claim that the selected package, npm itself,
transitive dependencies, native libraries or the eventual Agent runtime are compatible.
An exact policy minimum passing the check is not an endorsement of an EOL Node release.
The installer still does not pin the package version or resolve its dependency closure.
No Node download, upgrade or PATH change is performed automatically.

## Evidence and source distinction

Source review on 2026-11-11 found:

- The published [Codex 0.167.1 metadata](https://registry.npmjs.org/@openai/codex/0.167.1)
  declares Node `>=16`. AgentDeck's 18.0.0 installer policy is deliberately stricter
  than that declaration; it is not the exact upstream minimum.
- The published [OpenCode V1 1.19.5 metadata](https://registry.npmjs.org/opencode-ai/1.19.5)
  has no `engines` declaration. AgentDeck's 18.0.0 is its own policy, not a verified
  upstream minimum. The current OpenCode docs describe V2 `@opencode/cli` and must
  not be used to claim V1 compatibility.
- The [Gemini official source manifest](https://github.com/google-gemini/gemini-cli/blob/main/package.json)
  declares Node `>=20.0.0` (reviewed main identified itself as
  `0.70.0-nightly.20261110.g1603e9bab`). This source branch is mutable evidence,
  not a pin on the package an installation will choose.
- The [Pi official source manifest](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/package.json)
  declares Node `>=22.19.0` (reviewed version 1.5.2). This source branch is mutable
  evidence, not a package pin.
- [npm's engines documentation](https://docs.npmjs.com/cli/v11/configuring-npm/package-json/#engines)
  describes the declaration as advisory unless `engine-strict` is enabled.
  AgentDeck does not rely on that setting for its fixed minimum policy.

This is metadata review, not installation or runtime acceptance of those releases.

## Ordering and review binding

After manager resolution, the version probe precedes ordinary npm global prefix/root
queries and package mutation. Existing Windows distributed npm resolution may itself
invoke `npm-prefix.js` before this probe, so rejection does not promise that no npm
code has run or that no npm cache/log was touched.

The desktop preview displays `installerNodeVersion` and `installerNodeMinimum`.
Both are included in the serialized preview revision and the plan equality check.
Direct CLI installation and desktop confirmed installation rebuild and compare the
plan immediately before the package-mutating subprocess. A changed measurement,
manager, prefix or occupied destination blocks execution. Invalid or stale previews
are not reusable authorization. Existing same-user race and sandbox limitations remain.

On Windows, a future launcher may select an adjacent `node.exe` in the installation
prefix rather than the installer Node beside npm. The new measurement does not
validate that future Node's version. Native headers and hashes do not authenticate
Node's publisher or establish a general execution sandbox.

## Verification scope

Core regressions model all four boundaries, strict parsing, process failure,
changed measurements, unchanged direct argv and install-callback non-invocation.
Frontend regressions validate exact policy projection and display, rejecting missing,
malformed, prerelease and below-minimum evidence. Existing cancellation and stale-response
coverage remains in force.

A separate exact-name, isolated `trusted_installer_node_version_smoke` test uses
an explicitly supplied trusted native Node with the production bounded runner and
strict parser. It invokes only `--version`, never npm, an Agent or an installer.
The ordinary test suite keeps it ignored; CI explicitly runs only this named smoke.

The authored Linux and Windows npm fixtures use a separately supplied real native
Node. Their preparation independently measures that Node; GUI acceptance compares
its version and the 18.0.0 Codex policy against the native IPC review dialog. The
fixture does not implement or spoof Node's version output. These remain **authored
npm/Codex fixtures**, not official package installation or runtime acceptance. Offline
fixture tests are not GUI acceptance. Native GUI lanes must run separately on supported
hosts with their existing isolation and WebView sandbox retained.

No manual-only adapter is enabled, no DSH lock or runtime claim is extended, no #22
isolation requirement is relaxed, and no installer publication is authorized here.
