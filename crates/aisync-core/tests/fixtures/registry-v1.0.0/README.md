# Original public v1.0.0 registry fixtures

These are the unchanged public assets from
<https://github.com/MOSHOULI/agentdeck/releases/tag/v1.0.0>.
The release's original `manifest.json` and `manifest.json.sig` were checked on
2026-11-03: the Ed25519 signature verifies against the application's unchanged
embedded public key (fingerprint `dc83b35677939e36`).

- `registry-v3.json`: 14,724 bytes; SHA256 `3a30c35b56d04b685671f24fd36f2222d5a129930d3ab043d4bab60ee8dd461a`.
- `mcp-registry.json`: 884 bytes; SHA256 `c35b683d4810fcd3ce3ccd41b8d032923b1ab26d88e5a14c54eb9879e9f36c9f`.

Regression tests use these exact bytes to recognize the historical stock pair.
Do not reformat them. The local `.gitattributes` disables line-ending conversion
so Windows checkout preserves their signed bytes. They contain public product
metadata, not user data.
