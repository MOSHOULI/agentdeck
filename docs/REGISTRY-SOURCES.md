# Registry source verification

Verified **2026-11-02** against the official sources below. This is a focused correction of Pi and Hermes installation metadata, not certification of every registry entry. No third-party installer was executed during verification.

## Pi

- [Official relocation/package announcement, 2026-05-07](https://pi.dev/changelog/2026/5/7/pi-has-a-new-home)
- Repository: [earendil-works/pi](https://github.com/earendil-works/pi)
- Homepage: [pi.dev](https://pi.dev)

The current package is `@earendil-works/pi-coding-agent`; the old `@mariozechner` scope is deprecated. The bundled install and uninstall commands now address the current package. The CLI remains `pi`, so version detection remains `pi --version`. Uninstall does not remove a separately installed old-scope package.

## Hermes Agent

- [Official installation guide](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/getting-started/installation.md)
- [Official platform support matrix](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/getting-started/platform-support.md)
- [Official download site](https://hermes-agent.nousresearch.com/)

The platform support matrix explicitly excludes PyPI installation. AgentDeck therefore does **not** run `pip install hermes-agent` or a corresponding pip uninstall. Hermes is a manual-install entry linked to the official site; `hermes --version` remains available for detection. Users select the official package or platform installer themselves.

Native Windows defaults to `%LOCALAPPDATA%\hermes`; POSIX defaults to `~/.hermes`. `HERMES_HOME` can override the user-data location. AgentDeck does not yet resolve and validate that platform/profile selection, so the bundled Hermes MCP, personalization, and model-config write adapters are **disabled**, rather than writing a potentially unrelated `~/.hermes/config.yaml`. Configure Hermes directly through its own tools. In particular, a Markdown instruction template must never overwrite its YAML configuration.

Registry metadata may change upstream; future revisions should recheck these sources before re-enabling automatic installation or configuration.
