# Mainstream product guidance (2026-11-08)

[简体中文](MAINSTREAM-CATALOG.zh-CN.md)

The bundled catalog adds six **manual-guidance surfaces** from four product families: Cursor Desktop, Cursor Agent CLI, GitHub Copilot CLI, Kimi Code CLI, Kimi Code Desktop and Qoder CLI. The total is 17 catalog entries, not 17 end-to-end integrations. CLI and desktop installations are separate; finding one would not establish the other.

All six entries have null install/verify/uninstall commands and authentication checks. Skills, MCP/configuration writes, memory export and memory collectors are disabled. They neither scan PATH nor execute an agent, npm, installer, migration or login. Their installation, version, source and authentication remain unknown. The UI includes them under manual installation and uncertain status, never the installed or automatic-install lists. This changes bundled metadata only; it does not overwrite a preserved custom runtime registry or publish a new registry feed.

## Cursor

- [Desktop downloads](https://cursor.com/download): Windows x64/ARM64, macOS Intel/Apple Silicon, and Linux DEB/RPM/AppImage builds. Choose the official package for the user's platform.
- [Current CLI installation](https://cursor.com/docs/cli/installation): macOS, Linux, WSL and native Windows. Current documentation uses `agent`, not the older `cursor-agent`. Shell/PowerShell installers are user-run instructions only.
- [CLI authentication](https://cursor.com/docs/cli/reference/authentication): browser login with `agent login`, or `CURSOR_API_KEY`.

`agent` is a generic executable name. This catalog does not treat a same-name file as Cursor identity evidence. The older docs.cursor.com pages can differ from current cursor.com documentation. Desktop and CLI are separate entries; no desktop installation or sign-in is inferred from CLI state. No open-source license is asserted for these entries.

## GitHub Copilot CLI

- [Official installation](https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/install-copilot-cli): standalone `copilot`, with npm package `@github/copilot` (Node.js 22+), WinGet for Windows, Homebrew for macOS/Linux and an official macOS/Linux script. Windows requires PowerShell 6+.
- [Authentication](https://docs.github.com/en/copilot/how-tos/copilot-cli/set-up-copilot-cli/authenticate-copilot-cli): `copilot login` provides OAuth; other documented routes include supported environment tokens and GitHub CLI fallback. BYOK use can avoid GitHub authentication. Entitlements and organization policy still depend on the selected mode.
- [Official repository](https://github.com/github/copilot-cli) and [custom license](https://github.com/github/copilot-cli/blob/main/LICENSE.md): a public repository does not make this binary MIT/Apache-licensed open source.

This is not the old `gh-copilot` extension or a Copilot IDE-extension entry. Official npm setup requires lifecycle scripts; the catalog does not silently apply another adapter's `--ignore-scripts` policy or claim that such an installation works.

## Kimi Code

- [Current CLI repository](https://github.com/MoonshotAI/kimi-code): TypeScript/Node.js, MIT. [Getting started](https://www.kimi.com/code/docs/en/kimi-code-cli/guides/getting-started) documents `@moonshot-ai/kimi-code` with Node.js 22.19.0+ for npm, or official standalone installers without a preinstalled Node requirement. Windows needs Git for Windows/Git Bash. `/login` offers Kimi Code OAuth or a Kimi Platform API key.
- [Legacy kimi-cli repository](https://github.com/MoonshotAI/kimi-cli) is archived. Both generations use `kimi`; a filename is not version identity. Current data defaults to `~/.kimi-code` with `KIMI_CODE_HOME`, rather than the legacy `~/.kimi`. [Migration](https://www.kimi.com/code/docs/en/kimi-code-cli/guides/migration.html) is user-controlled; OAuth credentials and MCP authorizations are not migrated. AgentDeck never runs it.
- [Current Desktop guide](https://www.kimi.com/code/docs/en/kimi-code-desktop/getting-started.html) and [installer FAQ](https://www.kimi.com/code/docs/en/kimi-code-desktop/help.html): macOS 13+, Windows 10+ 64-bit, x86_64 Linux. The older general help-center page still lists macOS 12 and omits Linux; the linked team-maintained guide is used here. Desktop OAuth/account entitlements or third-party provider setup remain user-controlled. The CLI repository's MIT license is not asserted for the desktop binary.

## Validation boundary

This is a point-in-time official documentation review, not a package, version or publisher signature verification. No installer, npm publication, real login, platform GUI or agent runtime was tested. Regression tests cover the bundled metadata, no probe callbacks, absence of executable/configuration capabilities, and UI manual/unknown-state rendering. No automatic adapters or credentials were added. New official releases or changed documentation require a new review before expanding capabilities.

## Existing and reference entries reviewed on 2026-11-08

- [Claude Code](https://github.com/anthropics/claude-code/blob/0ed94698aa0aeee471465fc9c867c2e8434c4d68/README.md): npm installation is deprecated; native installers, Homebrew or WinGet are now the official alternatives. The bundled entry links the manual setup guide instead of automatically running the obsolete npm recipe. Existing configuration adapters remain, but no new native-package adapter or runtime validation is implied. The unverified credentials-file login hint is removed.
- [Gemini CLI](https://github.com/google-gemini/gemini-cli/blob/62bab5b232c15b925d31e655ce844ab939c882b7/README.md): the current README limits its intended audience to enterprise Code Assist and legacy paid API/Vertex workflows. Personal free/Google AI Pro/Ultra access moved to Antigravity CLI on 2026-06-18. The existing CLI recipe is retained for compatibility, not recommended as the personal successor; Antigravity is not integrated. The unverified auth.json login hint is removed rather than replaced by another file-presence guess.
- [Qwen Code](https://github.com/QwenLM/qwen-code): upstream product surfaces extend beyond CLI. AgentDeck still offers only the previously documented manual CLI/passive-discovery boundary, not desktop/IDE/WebUI management.
- [OpenCode](https://opencode.ai/docs/migrate-v1): retain the explicit V1 package/compatibility boundary; no V2 installation or model/MCP management is claimed.
- [Kilo](https://github.com/Kilo-Org/kilocode) and [Aider](https://github.com/Aider-AI/aider): candidates for a separate adapter review, not newly enabled automatic management or a popularity ranking.
- [Continue](https://github.com/continuedev/continue): README states the repository is read-only and no longer actively maintained, with a final 2.0.0 release. Do not recommend it as an actively maintained default.
- [Plandex](https://github.com/plandex-ai/plandex): cloud service is winding down and closed to new users; self-host/local instructions remain. This is not evidence that all local code is unusable.
- [Roo Code](https://github.com/RooCodeInc/Roo-Code): archived May 15, 2026; README says the extension shut down. Retain historical reference rather than promote as a current default.

Existing IDs and reference entries are preserved for compatibility. These distinctions concern upstream product status and AgentDeck support separately; stars and adoption claims are not used as evidence of fitness.

Claude retains existing read-only installation inspection and configuration adapters; native-package recognition was not newly validated. The fixed installer rejects the deprecated Claude npm channel even when an older runtime registry still supplies its recipe.

## Qoder CLI

[Official installation](https://docs.qoder.com/cli/installation) prefers a standalone native installer. `@qoder-ai/qodercli` is a legacy npm compatibility route requiring Node.js 20+, not the recommended new installer. The command is `qoder`; Windows ARM64 is unsupported. [Custom models](https://docs.qoder.com/cli/custom-models) use the `/model` wizard. This entry only links official instructions: no executable detection, credential injection, installation, configuration or desktop coverage is enabled.

## Other existing entries: dated maintenance evidence

These are upstream release/source observations on 2026-11-08, not installed-version measurements or popularity endorsements.

| Entry | Official maintenance/source evidence | AgentDeck boundary |
| --- | --- | --- |
| Codex | [0.167.0 release](https://github.com/openai/codex/releases/tag/rust-v0.167.0), 2026-11-06 | Existing CLI adapter; IDE/desktop is not implied by CLI evidence. |
| Cline | [4.1.26 release](https://github.com/cline/cline/releases/tag/v4.1.26), 2026-10-26 | Existing extension entry; upstream CLI/Desktop/JetBrains/SDK are not new managed surfaces. |
| Pi | [1.5.0 release](https://github.com/earendil-works/pi/releases/tag/v1.5.0), 2026-11-02; [official migration](https://pi.dev/changelog/2026/5/7/pi-has-a-new-home) | Retain earendil-works package, not obsolete ownership. |
| Hermes | [0.23.0 release](https://github.com/NousResearch/hermes-agent/releases/tag/v0.23.0), 2026-11-05 | Existing manual boundary; Windows native paths must not be inferred from Unix paths. |
| OpenClaw | [2026.11.2 release](https://github.com/openclaw/openclaw/releases/tag/v2026.11.2), 2026-11-08 | Gateway/channel/plugin assistant, manual guidance; not interchangeable with a terminal coding agent. |
| ZCode | [Official changelog](https://zcode.z.ai/cn/changelog), 3.17.2 on 2026-11-03 | Desktop product; keep manual guidance, not inferred CLI management. |

## DSH belongs to its existing inventory workspace

DSH already has a separate [desktop/CLI passive declaration inventory](DSH-PASSIVE-INVENTORY.md); its catalog card now links directly to that workspace. It reads explicitly selected profile/runtime manifests without starting DSH. Saved dependencies and selected bundles are declarations, not verified installation, authentication or active-plugin state. Keep installation, updates, activation and configuration disabled; the catalog card is not an automatic management adapter.

Reference-only entries keep their IDs: Continue is `unmaintained`, Plandex is `cloud_winding_down`, Roo Code is `archived`, and the others are `not_integrated`. These statuses do not promise a release version. Kilo and Aider point to their current organizations.
