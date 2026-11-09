#!/usr/bin/env python3
"""Offline checks: namespace denial must stop before importing Qwen."""
import importlib.util
import pathlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('runtime', pathlib.Path(__file__).with_name('test-qwen-settings-runtime.py'))
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


class HarnessTests(unittest.TestCase):
    def test_namespace_denial_has_no_fallback_or_extraction(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = pathlib.Path(directory) / 'aisync'
            binary.write_text('not executed')
            with patch('sys.argv', ['test', '--archive', '/unused', '--aisync', str(binary)]), \
                 patch.object(harness.shutil, 'which', side_effect=lambda name: '/usr/bin/' + name), \
                 patch.object(harness.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1, '', 'namespace denied')) as run, \
                 patch.object(harness.distribution, 'unpack') as unpack:
                with self.assertRaisesRegex(RuntimeError, 'namespace denied'):
                    harness.main()
                self.assertEqual(run.call_count, 1)
                self.assertIn('--unshare-net', run.call_args.args[0])
                unpack.assert_not_called()

    def test_subprocess_timeout_is_bounded(self):
        with patch.object(harness.subprocess, 'run', side_effect=subprocess.TimeoutExpired('node', 45)) as run:
            with self.assertRaises(subprocess.TimeoutExpired):
                harness.run(['node'], '/synthetic', {})
            self.assertEqual(run.call_args.kwargs['timeout'], 45)

    def test_no_weak_mode_accepted(self):
        with patch('sys.argv', ['test', '--archive', '/unused', '--aisync', '/unused', '--network-isolation', 'none']):
            with self.assertRaises(SystemExit):
                harness.main()


if __name__ == '__main__':
    unittest.main()
