#!/usr/bin/env python3
"""Run tests/CLI smoke checks without access to the caller's application home.

Usage: python scripts/run-isolated.py cargo test --workspace --locked
Real integration tests remain ignored: isolation does not authorize installers.
"""
import os
import pathlib
import subprocess
import sys
import tempfile


def main() -> int:
    if len(sys.argv) < 2:
        print("Usage: run-isolated.py COMMAND [ARGS...]", file=sys.stderr)
        return 2
    env = os.environ.copy()
    original_home = pathlib.Path.home()
    env.setdefault("CARGO_HOME", str(original_home / ".cargo"))
    env.setdefault("RUSTUP_HOME", str(original_home / ".rustup"))
    with tempfile.TemporaryDirectory(prefix="agentdeck-test-home-") as directory:
        root = pathlib.Path(directory)
        env.update({
            "HOME": directory,
            "AGENTDECK_TEST_HOME": directory,
            "USERPROFILE": directory,
            "APPDATA": str(root / "AppData" / "Roaming"),
            "LOCALAPPDATA": str(root / "AppData" / "Local"),
            "XDG_CONFIG_HOME": str(root / ".config"),
            "XDG_DATA_HOME": str(root / ".local" / "share"),
        })
        for name in ("APPDATA", "LOCALAPPDATA", "XDG_CONFIG_HOME", "XDG_DATA_HOME"):
            pathlib.Path(env[name]).mkdir(parents=True, exist_ok=True)
        return subprocess.run(sys.argv[1:], env=env, check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
