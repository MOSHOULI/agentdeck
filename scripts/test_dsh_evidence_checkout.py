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
    'docs/upstream/dsh/native-evidence/artifacts.json': '4565645d30baa92db67511f155ad774b1ca4a3d21b1a0e27b37783a248052573',
    'docs/upstream/dsh/native-evidence/koffi/LICENSE.txt.txt': '22bcbbbacd739b026a8dc484f62a31484ce4c4e31f3408b60b29b4e41d62b290',
    'docs/upstream/dsh/native-evidence/koffi/cnoke.cjs.txt': 'd0547e34c211e4c01914418b514bba0f141359324fe1369734660bd75f7ffafc',
    'docs/upstream/dsh/native-evidence/koffi/metadata.json': '10daa36c7aba172d929aa632773419a72b575dfaed8dc680cbe757e1a5e843c7',
    'docs/upstream/dsh/native-evidence/koffi/package.json.txt': 'c0473aeaf572a612e643489f49ac8e6d1fe26092cf69802567332dc57fdca4c1',
    'docs/upstream/dsh/native-evidence/koffi/src__koffi__index.cjs.txt': 'cfd8708a7016740afb0e9f0a1c669485ccf8be98e3f2ef0fd6a4be76f4986f03',
    'docs/upstream/dsh/native-evidence/koffi-linux-x64/index.js.txt': 'f4f316a915fc895dc5e6e0b7c5d83dd5b97cdd8ea815276fef5578f92873703b',
    'docs/upstream/dsh/native-evidence/koffi-linux-x64/metadata.json': 'b471cb152f4254545bc6759af3a9db78142f468dab0b30d33fd4e53790334eb7',
    'docs/upstream/dsh/native-evidence/koffi-linux-x64/package.json.txt': '468c0748da5e9c74dfa58308dd23383c49505d29a911998bfac86edf9cba1d02',
    'docs/upstream/dsh/native-evidence/koffi-win32-x64/index.js.txt': '632ab5a01dfc1b519b06275f490d6790100a2f8d71d8f56957091d0fcc5b3e0b',
    'docs/upstream/dsh/native-evidence/koffi-win32-x64/metadata.json': 'bb7200748d1a4aa9010013c2dc677a345905ed920328c71360d17ffe979a0c60',
    'docs/upstream/dsh/native-evidence/koffi-win32-x64/package.json.txt': 'b38fcff8641d493656ef4153a37fd7389806ae621bb916aca536fd3bbc158690',
    'docs/upstream/dsh/native-evidence/node-pty/LICENSE.txt': 'a3d60eaf32d2fb09c9d82ef39f14bd6e2c0f3ca7de30daa827486c0d6f8b6e9f',
    'docs/upstream/dsh/native-evidence/node-pty/lib__unixTerminal.js.txt': '63b605fb25c4e6d237493cd1799b01a8d880d89c78850108aae1853d04b4cdf0',
    'docs/upstream/dsh/native-evidence/node-pty/lib__utils.js.txt': '807bc62c8b702377d591136862309718ba48fb2b9eca54f4b25fbe43838c8fc6',
    'docs/upstream/dsh/native-evidence/node-pty/metadata.json': '371717f3c9568b63681e11555eeb100e65dc51f02ee6e30e13f8444773bf13aa',
    'docs/upstream/dsh/native-evidence/node-pty/package.json.txt': 'f4a19bbc7cf4c2e35c081e6a18a3d11d21c54c2f3cc25bfe48142742a781b71d',
    'docs/upstream/dsh/native-evidence/node-pty/scripts__post-install.js.txt': '98b3f6379debdab20eea21f216e0d09fa38c7757ddc3411f1a743eae7f5be14a',
    'docs/upstream/dsh/native-evidence/node-pty/scripts__prebuild.js.txt': '7e604b10f7769d7dc95947d3481c00513b9e3bb6c561d5264506350a16a0381a',
    'docs/upstream/dsh/native-evidence/node-pty/src__unix__pty.cc.txt': '5809f87b15122f335017b0b3020071df4c6205c7827186a2c5a9e0edc9ef59b2',

    'docs/upstream/dsh/dependency-evidence/dsh-sdk-minimal-0.2.2-rc.1.json':
        'c38cf52c6f73815f3a5796afd7095ff8c2e680ce09aca60d158878dbc0717f24',
    'docs/upstream/dsh/dependency-evidence/subprocess-package-0.2.2-rc.1.json':
        '48d50b0e9d019954a8689b39af2bc20b03ce68abe5dde3351b2e9e27ee842b9f',
    'docs/upstream/dsh/dependency-evidence/ensure-spawn-helper-0.2.2-rc.1.mjs':
        'ca5509febf1e6ec1356df121835ebe5ed2f9cace4bdc2ba6d83d41c7e45e0f1b',
    'docs/upstream/dsh/subprocess-archive-layout-0.2.2-rc.1.json':
        '0ccccffe051afe15679f1ae92c4834df3d2891dd73d7d0b74c3f8363205f53e3',

    "docs/upstream/qoder/manifest-1.1.85.json":
        "a78234316fe2065cdae6fc17ac425bbc2ae11006d04b6ab5216fd6093597e4de",
    "docs/upstream/qoder/manifest-1.1.86.json":
        "f323905da3f75088b1e9ae4cf9b0fef6f117879f4f6942b609e92228a533fa78",
    "docs/upstream/qoder/manifest-1.1.87.json":
        "5bea07a5bb10187aac873a1b3c5f7362e0b18f464daf4612cdc804cde31cd8ba",
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
                    for folder in ("docs/upstream/dsh", "docs/upstream/qoder")
                    for path in (ROOT / folder).rglob("*")
                    if path.suffix in (".json", ".mjs", ".txt")}
        self.assertEqual(fixtures, set(PINNED), "Review and pin any added evidence fixtures")
        original, checked_out = self.checkout((ROOT / ".gitattributes").read_bytes())
        for name, expected in PINNED.items():
            with self.subTest(fixture=name):
                self.assertEqual(checked_out[name], original[name])
                self.assertEqual(hashlib.sha256(checked_out[name]).hexdigest(), expected)

    def test_missing_attributes_reproduce_dsh_and_qoder_multiline_digest_failures(self):
        original, checked_out = self.checkout(b"")
        for name in [
            "docs/upstream/dsh/native-evidence/artifacts.json",
            "docs/upstream/dsh/native-evidence/node-pty/scripts__prebuild.js.txt",
            "docs/upstream/dsh/native-evidence/koffi/cnoke.cjs.txt",
            'docs/upstream/dsh/dependency-evidence/subprocess-package-0.2.2-rc.1.json',
            'docs/upstream/dsh/dependency-evidence/ensure-spawn-helper-0.2.2-rc.1.mjs',
            'docs/upstream/dsh/subprocess-archive-layout-0.2.2-rc.1.json',
            "docs/upstream/qoder/manifest-1.1.85.json",
            "docs/upstream/qoder/manifest-1.1.86.json",
            "docs/upstream/qoder/manifest-1.1.87.json",
            "docs/upstream/dsh/root-0.2.2-rc.1.json",
            "docs/upstream/dsh/dependency-evidence/root-package-0.2.2-rc.1.json",
        ]:
            with self.subTest(fixture=name):
                self.assertIn(b"\n", original[name])
                self.assertEqual(checked_out[name], original[name].replace(b"\n", b"\r\n"))
                self.assertNotEqual(hashlib.sha256(checked_out[name]).hexdigest(), PINNED[name])


if __name__ == "__main__":
    unittest.main()
