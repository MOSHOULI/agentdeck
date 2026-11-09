#!/usr/bin/env python3
"""Offline regression tests for the distribution smoke harness itself."""
import importlib.util
import io
import json
import os
import pathlib
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('distribution', pathlib.Path(__file__).with_name('test-qwen-distribution.py'))
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


class HarnessTests(unittest.TestCase):
    def test_environment_does_not_inherit_secrets_or_loader_options(self):
        with patch.dict(os.environ, {'OPENAI_API_KEY': 'SYNTHETIC', 'NODE_OPTIONS': '--require=bad', 'HTTPS_PROXY': 'SYNTHETIC'}):
            env = harness.environment(pathlib.Path('/synthetic'))
        self.assertFalse({'OPENAI_API_KEY', 'NODE_OPTIONS', 'HTTPS_PROXY', 'PATH'} & env.keys())
        self.assertEqual(env['HOME'], '/synthetic')
        self.assertEqual(env['QWEN_HOME'], '/synthetic/.qwen')

    def test_modified_archive_rejected_before_extraction(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / 'bad.tgz'
            path.write_bytes(b'not the pinned distribution')
            with self.assertRaisesRegex(ValueError, 'integrity'):
                harness.unpack(path, pathlib.Path(directory))

    def archive(self, root, name, payload=b'{}', link=False):
        path = root / 'input.tar'
        with tarfile.open(path, 'w') as archive:
            info = tarfile.TarInfo(name)
            if link:
                info.type = tarfile.SYMTYPE
                info.linkname = '/outside'
            else:
                info.size = len(payload)
            archive.addfile(info, io.BytesIO(payload) if not link else None)
        return path

    def test_unsafe_members_rejected(self):
        for name, link in [('package/../../escape', False), ('/absolute', False), ('other/file', False), ('package/link', True)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                path = self.archive(root, name, link=link)
                with patch.object(harness, 'verify_archive'), self.assertRaisesRegex(ValueError, 'member'):
                    harness.unpack(path, root / 'output')
                self.assertFalse((root / 'output').exists())

    def test_unexpected_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            path = self.archive(root, 'package/package.json', json.dumps({'name': '@other/agent', 'version': '0.27.0', 'bin': {'qwen':'cli-entry.js'}}).encode())
            with patch.object(harness, 'verify_archive'), self.assertRaisesRegex(ValueError, 'identity'):
                harness.unpack(path, root / 'output')


if __name__ == '__main__':
    unittest.main()
