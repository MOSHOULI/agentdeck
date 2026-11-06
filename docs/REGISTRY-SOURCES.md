# Registry source verification

English | [简体中文](REGISTRY-SOURCES.zh-CN.md)

Last focused review: **2026-11-06**. This review covers the Node requirements and authentication guidance for Gemini CLI and Claude Code, Pi's runtime/configuration layout, the source-only entries for Roo Code, Crush, and Oh-My-Pi, and the OpenCode V1/V2 installation/configuration boundary. The Hermes findings below retain their **2026-11-02** verification date. This is not certification of every catalog field or agent.

## What the catalog does and does not prove

- **Upstream documentation** describes the vendor's product. An upstream feature is not evidence of an AgentDeck adapter.
- **AgentDeck support** depends on the bundled `install` and `config_inject` metadata plus the implementation and its tests. `null` disables a write adapter; it does not mean the upstream product lacks the feature. A Node dependency string records an upstream requirement, not proof that a local runtime satisfies it.
- **Local observations** in this review are limited to parsing the bundled registry and checking static expectations. No agent, installer, login flow, provider request, or MCP server was run. No real user configuration or credential file was inspected or changed.
- `future_registry` contains source references, including archived projects. It is not an installation list or a delivery commitment. Moving a name into `agents` requires current source verification, an implemented adapter, and isolated tests.

The date in `registry-v3.json` records this edit, not a complete revalidation of all installation routes or existing authentication-file hints. A file-presence hint cannot establish a working login or a valid remote token.

**No popularity ranking is certified.** Every core and reference entry now has `stars: null` and `popular: false`. Null means no current verified count is supplied, not zero stars; false withdraws the legacy recommendation flag, not a negative assessment of the product. Unverified download/context/model-version counts, market rankings, and subjective superlatives have been removed from descriptions. The verified Roo Code archival date and explicit OpenCode V1/V2 boundary remain. Product descriptions are identification, not comparative recommendations or blanket capability certification.

The schema remains compatible: `stars` is optional, and sorting treats missing counts as zero while retaining installed-first behavior. Equal unranked entries use the core ID tie-breaker for stable display, not a certified popularity order. Separate UI labels, demo fixtures, and previously cached registries are not evidence of current verification and are not changed by this bundled-catalog edit.

## OpenCode: V1 installation route, bounded configuration support

- [Official V1 installation guide](https://opencode.ai/docs/)
- [Official V2 installation guide](https://opencode.ai/v2/docs)
- [V1-to-V2 compatibility and migration](https://opencode.ai/v2/docs/migrate-v1/)
- [Native V2 MCP format](https://opencode.ai/v2/docs/mcp-servers/)
- [V2 instruction-file locations](https://opencode.ai/v2/docs/instructions/)

The retained `npm install -g opencode-ai` recipe is the official **V1** route, not V2 support. V2 documents `@opencode/cli`, does not support Windows package managers, and offers standalone binaries. AgentDeck does not silently substitute that package or perform migration. Both versions share the `opencode` command and can replace or conflict with each other. Install preflight guards the instances it observes; it does not promise discovery of every installation on the machine.

V2 still reads `~/.config/opencode/opencode.json(c)` and supports documented V1 fields. It is inaccurate to describe every old path or field as incompatible. Plugins, server APIs, and terminal-client configuration have distinct migration boundaries, and nested native shapes can differ.

AgentDeck's current MCP adapter reads and edits `mcp.<name>`, whereas native V2 stores entries under `mcp.servers.<name>`. It cannot reliably list or disable a native V2 server. The generic model-profile merge does not validate V1/V2 nested schemas; its empty bundled profile is not proof that user-supplied profiles are compatible. Therefore **MCP and model-config adapters are disabled**, without inventing a new path or enabling a V2 adapter.

Personalization remains enabled: the V2 instructions reference explicitly retains `~/.config/opencode/AGENTS.md`, and AgentDeck merges a managed Markdown block there. This narrowly supported path does not certify a real-agent end-to-end run. Skills, memory flags, and the session collector are unchanged and have not been recertified for V2 here. Future V2 support needs explicit version/shape-aware adapters and isolated tests before write capabilities return.

## Gemini CLI

- [Official installation requirements](https://geminicli.com/docs/get-started/installation/)
- [Official authentication guide](https://geminicli.com/docs/get-started/authentication/)

The documented runtime is **Node.js 20.0.0 or newer**. The Google account flow starts with `gemini`, followed by **Sign in with Google** and the browser prompts. The old `gemini login` instruction has been removed. This does not validate the existing `completion_check` path or test OAuth. Existing AgentDeck adapter flags are unchanged.

## Claude Code

- [Official setup guide, npm installation](https://code.claude.com/docs/en/setup#install-with-npm)
- [Official CLI reference](https://code.claude.com/docs/en/cli-reference)

The **npm package** requires **Node.js 22 or newer**. That requirement is specific to AgentDeck's existing npm route, not a claim that native Claude Code needs Node at runtime. The documented account command is `claude auth login`; the old `claude login` instruction has been removed. The install recipe and existing adapters are unchanged. Authentication storage and the existing `completion_check` path were not validated in this review.

## Pi

- [Official relocation/package announcement, 2026-05-07](https://pi.dev/changelog/2026/5/7/pi-has-a-new-home)
- Repository: [earendil-works/pi](https://github.com/earendil-works/pi)
- Homepage: [pi.dev](https://pi.dev)
- [Current package manifest](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/package.json)
- [Configuration layout and instruction files](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/configuration.md)
- [Path resolution implementation](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/src/config.ts)
- [Upstream MCP support](https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/mcp.md)

The current package is `@earendil-works/pi-coding-agent`; the old `@mariozechner` scope is deprecated. The bundled install and uninstall commands now address the current package. The CLI remains `pi`, so version detection remains `pi --version`. Uninstall does not remove a separately installed old-scope package.

The manifest requires **Node.js >=22.19.0**. User-level configuration defaults to `~/.pi/agent`, with `PI_CODING_AGENT_DIR` as an override. Pi separates settings (`settings.json`), provider/model definitions (`models.json`), saved credentials (`auth.json`), and MCP servers (`mcp.json`). Global instruction files, including `AGENTS.md`, also live in the selected agent directory; project configuration and context files have their own rules.

AgentDeck's former model target, `~/.pi/config.json`, does not match this layout. Its `model_config` adapter is now **disabled**, not redirected to another file. A path correction alone would not establish correct schemas, credential handling, profile selection, or merge behavior. Personalization was already `null` and stays disabled, so there is no active instruction-file target to migrate. MCP stays `null`; skills and memory stay `false`, and there is no memory collector. Upstream MCP support does not enable AgentDeck MCP injection.

The bundled Pi recipe now matches the current upstream npm alternative: `npm install -g --ignore-scripts @earendil-works/pi-coding-agent`. The fixed npm adapter uses this same argument list. Upstream says normal Pi npm installation does not need dependency lifecycle scripts; suppressing those scripts does not certify every dependency or make the package sandboxed. The upstream README also notes that direct npm installation does not pin transitive dependencies. This review did not execute the recipe.

For repeatable source identification, the GitHub reads returned these **file blob SHAs**, not repository commit IDs:

- Package manifest: `13e6335774d9809f74a1ccd2dd690770aad51473`.
- Configuration reference: `b16fd001d836f764a3e50f9cdbde7e72ccbdef41`.
- Path implementation: `5a9552d5de451817ab06e99f2626e931d15c3384`.

## Source-only and archived entries

- [Roo Code](https://github.com/RooCodeInc/Roo-Code): the repository was archived on **2026-05-15**, and its README says the extension shut down on May 15. The catalog retains the entry only as historical reference, with `status: archived`. It is not an upcoming recommendation or supported installation. Its historical extension method is not an executable recipe.
- [Crush](https://github.com/charmbracelet/crush): the source is `charmbracelet/crush`, replacing `charms/crush`. It remains outside `agents`, manual/source-only, without a configuration adapter.
- [Oh-My-Pi](https://github.com/can1357/oh-my-pi): the source is `can1357/oh-my-pi`, replacing `oh-my-pi/oh-my-pi`. It remains outside `agents`, manual/source-only, without a configuration adapter.

## Hermes Agent (verified 2026-11-02)

- [Official installation guide](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/getting-started/installation.md)
- [Official platform support matrix](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/getting-started/platform-support.md)
- [Official download site](https://hermes-agent.nousresearch.com/)

The platform support matrix explicitly excludes PyPI installation. AgentDeck therefore does **not** run `pip install hermes-agent` or a corresponding pip uninstall. Hermes is a manual-install entry linked to the official site; `hermes --version` remains available for detection. Users select the official package or platform installer themselves.

Native Windows defaults to `%LOCALAPPDATA%\hermes`; POSIX defaults to `~/.hermes`. `HERMES_HOME` can override the user-data location. AgentDeck does not yet resolve and validate that platform/profile selection, so the bundled Hermes MCP, personalization, and model-config write adapters are **disabled**, rather than writing a potentially unrelated `~/.hermes/config.yaml`. Configure Hermes directly through its own tools. In particular, a Markdown instruction template must never overwrite its YAML configuration.

Registry metadata may change upstream; future revisions should recheck these sources before re-enabling automatic installation or configuration.

## Isolated catalog regression checks

`crates/aisync-core/tests/catalog_metadata.rs` parses the actual bundled schema-v3 registry and the dated expectations in `tests/fixtures/catalog-verification-2026-11-06.json`. It checks the Node/auth corrections, disabled Pi/Hermes write adapters, the explicit OpenCode V1 route with disabled MCP/model writes and retained instruction target, archived/source-only status, reviewed fixed install recipes, the existing nine core IDs, and withdrawal of unverified popularity metadata across all core/reference entries. A focused ordering check preserves installed-first behavior without inventing a ranking. It never executes a catalog command, reads a user's home, or contacts a provider.

Run through the repository's isolated-home wrapper:

```sh
python scripts/run-isolated.py cargo test --locked -p aisync-core --test catalog_metadata
```

These tests detect catalog regressions. They do not re-fetch upstream sources or demonstrate a real installation, login, or configuration round trip.
