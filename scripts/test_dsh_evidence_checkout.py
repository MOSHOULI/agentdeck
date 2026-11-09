#!/usr/bin/env python3
"""Offline regression: Git Windows-style checkouts retain pinned evidence bytes.

No package runtime, downloads, fixture normalization or modifications to the
source checkout. Each test uses an isolated temporary Git index and worktree.
"""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
PINNED = {
    "docs/upstream/dsh/root-0.2.2-rc.1.json":
        "e23a5772276c9a7709ffb2daeaa5b2d6099a431a4bb9f48cf8c87c5a2517539e",
    "docs/upstream/dsh/dependency-evidence/root-package-0.2.2-rc.1.json":
        "3e14837ceb3bbc0ba419ed08e9b5ba164338369b1268b93c3c25d89f7b1c9953",
    "docs/upstream/dsh/dependency-evidence/dsh-subprocess-local-0.2.2-rc.1.json":
        "4f7bf72dcfdc4769c0c63648d5c1c7276089e39d016a9af305455c816dae7de8",
    "docs/upstream/dsh/dependency-evidence/node-pty-1.2.0-beta.15.json":
        "371717f3c9568b63681e11555eeb100e65dc51f02ee6e30e13f8444773bf13aa",
    "docs/upstream/dsh/dependency-evidence/koffi-3.1.1.json":
        "10daa36c7aba172d929aa632773419a72b575dfaed8dc680cbe757e1a5e843c7",
}


class EvidenceCheckoutTests(unittest.TestCase):
    def checkout(self, attributes):
        with tempfile.TemporaryDirectory(prefix="dsh-evidence-checkout-") as temporary:
            root = Path(temporary)
            home = root / "home"
            home.mkdir()
            repository = root / "repository"
            # Do not inherit user/global Git settings, alternate indexes or paths.
            env = {key: value for key, value in os.environ.items()
                   if not key.upper().startswith("GIT_")}
            env.update({
                "GIT_CONFIG_NOSYSTEM": "1",
                "GIT_ATTR_NOSYSTEM": "1",
                "GIT_CONFIG_GLOBAL": os.devnull,
                "HOME": str(home),
                "USERPROFILE": str(home),
                "XDG_CONFIG_HOME": str(home),
            })

            def git(*arguments):
                return subprocess.run(
                    ["git", "-C", str(repository), *arguments], env=env,
                    check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                    timeout=30,
                ).stdout

            repository.mkdir()
            git("init", "--quiet", "--template=")
            (repository / ".gitattributes").write_bytes(attributes)
            original = {}
            for name, expected in PINNED.items():
                data = (ROOT / name).read_bytes()
                self.assertEqual(hashlib.sha256(data).hexdigest(), expected, name)
                original[name] = data
                target = repository / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
            # This deliberately unprotected file proves conversion is active.
            control = repository / "conversion-control.txt"
            control.write_bytes(b"first\nsecond\n")
            git("-c", "core.autocrlf=false", "add", "--", ".gitattributes",
                "conversion-control.txt", *PINNED)
            for name in PINNED:
                (repository / name).unlink()
            control.unlink()
            # checkout-index uses the same checkout conversion as git checkout.
            git("-c", "core.autocrlf=true", "-c", "core.eol=crlf",
                "checkout-index", "--force", "--", *PINNED, "conversion-control.txt")
            self.assertEqual(control.read_bytes(), b"first\r\nsecond\r\n")
            return original, {name: (repository / name).read_bytes() for name in PINNED}

    def test_windows_style_checkout_preserves_every_pinned_snapshot(self):
        fixtures = {path.relative_to(ROOT).as_posix()
                    for path in (ROOT / "docs/upstream/dsh").rglob("*.json")}
        self.assertEqual(fixtures, set(PINNED), "Review and pin any added evidence fixtures")
        original, checked_out = self.checkout((ROOT / ".gitattributes").read_bytes())
        for name, expected in PINNED.items():
            with self.subTest(fixture=name):
                self.assertEqual(checked_out[name], original[name])
                self.assertEqual(hashlib.sha256(checked_out[name]).hexdigest(), expected)

    def test_missing_attributes_reproduce_both_multiline_digest_failures(self):
        original, checked_out = self.checkout(b"")
        for name in [
            "docs/upstream/dsh/root-0.2.2-rc.1.json",
            "docs/upstream/dsh/dependency-evidence/root-package-0.2.2-rc.1.json",
        ]:
            with self.subTest(fixture=name):
                self.assertIn(b"\n", original[name])
                self.assertEqual(checked_out[name], original[name].replace(b"\n", b"\r\n"))
                self.assertNotEqual(hashlib.sha256(checked_out[name]).hexdigest(), PINNED[name])


if __name__ == "__main__":
    unittest.main()
