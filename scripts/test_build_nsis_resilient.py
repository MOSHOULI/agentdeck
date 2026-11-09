import unittest
import json
import tempfile
import subprocess
from pathlib import Path
from unittest.mock import Mock, patch
import build_nsis_resilient as subject


def failure(status=504, url=subject.URL):
    return f'Downloading {url}\nfailed to bundle project: `http status: {status}`\n       Error failed to bundle project: `http status: {status}`\n'.encode()

class RetryTests(unittest.TestCase):
    def test_exact_locked_source_contract(self):
        subject.check_contract()

    def test_lock_and_evidence_tampering_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'web/ui').mkdir(parents=True)
            (root / 'docs/upstream/tauri').mkdir(parents=True)
            lock = json.loads((subject.ROOT / 'web/ui/package-lock.json').read_text())
            source = (subject.ROOT / 'docs/upstream/tauri/nsis-2.12.4.rs').read_bytes()
            (root / 'docs/upstream/tauri/nsis-2.12.4.rs').write_bytes(source)
            lockpath = root / 'web/ui/package-lock.json'
            for field, bad in [('version', '2.12.5'), ('resolved', 'https://example.com/cli'), ('integrity', 'sha512-wrong')]:
                original = lock['packages']['node_modules/@tauri-apps/cli'][field]
                lock['packages']['node_modules/@tauri-apps/cli'][field] = bad
                lockpath.write_text(json.dumps(lock))
                with self.assertRaises(ValueError): subject.check_contract(root)
                lock['packages']['node_modules/@tauri-apps/cli'][field] = original
            native = lock['packages']['node_modules/' + subject.NATIVE_PACKAGE]
            for field, bad in [('version', '2.12.5'), ('resolved', 'https://example.com/native'), ('integrity', 'sha512-wrong')]:
                original = native[field]; native[field] = bad
                lockpath.write_text(json.dumps(lock))
                with self.assertRaises(ValueError): subject.check_contract(root)
                native[field] = original
            lockpath.write_text(json.dumps(lock))
            (root / 'docs/upstream/tauri/nsis-2.12.4.rs').write_bytes(source + b'changed')
            with self.assertRaises(ValueError): subject.check_contract(root)

    def test_snapshot_survives_real_autocrlf_checkout(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            relative = Path('docs/upstream/tauri/nsis-2.12.4.rs')
            (root / relative).parent.mkdir(parents=True)
            original = (subject.ROOT / relative).read_bytes()
            (root / relative).write_bytes(original)
            (root / '.gitattributes').write_bytes((subject.ROOT / '.gitattributes').read_bytes())
            def git(*args):
                subprocess.run(['git', '-C', str(root), *args], check=True, capture_output=True)
            git('init')
            git('config', 'core.autocrlf', 'true')
            git('add', '.gitattributes', str(relative))
            (root / relative).unlink()
            git('checkout-index', '--force', '--', str(relative))
            self.assertEqual((root / relative).read_bytes(), original)
            self.assertEqual(subject.hashlib.sha256((root / relative).read_bytes()).hexdigest(), subject.SOURCE_SHA256)

    def test_transient_statuses(self):
        for status in (408, 429, 502, 503, 504):
            self.assertTrue(subject.retryable(failure(status)))

    def test_permanent_or_unknown(self):
        for status in (400, 401, 403, 404, 500, 200):
            self.assertFalse(subject.retryable(failure(status)))

    def test_other_source(self):
        for url in (subject.URL + '?mirror=1', subject.URL.replace('github.com', 'example.com'), subject.URL.replace('0.5.3', '0.5.4')):
            self.assertFalse(subject.retryable(failure(url=url)))

    def test_hash_build_or_mixed_error_not_retried(self):
        for suffix in (b'Error hash mismatch\n', b'Error compilation failed\n', b'Downloading https://example.com/other\n', b'Error something else\n'):
            self.assertFalse(subject.retryable(failure() + suffix))
        self.assertFalse(subject.retryable(failure().replace(b'failed to bundle project:', b'hash mismatch:')))
        self.assertFalse(subject.retryable(b'old ' + failure()))
        self.assertFalse(subject.retryable(failure() + failure(url='https://example.com/other')))
        self.assertFalse(subject.retryable(failure().replace(b'\nfailed', b'\nInfo validating hash\nfailed')))

    def test_success_no_retry(self):
        build = Mock(return_value=(0, failure())); sleep = Mock()
        self.assertEqual(subject.run(build, sleep), 0)
        build.assert_called_once(); sleep.assert_not_called()

    def test_transient_then_success(self):
        build = Mock(side_effect=[(1, failure()), (0, b'ok')]); sleep = Mock()
        self.assertEqual(subject.run(build, sleep), 0)
        self.assertEqual(build.call_count, 2); sleep.assert_called_once_with(10)

    def test_exhaustion(self):
        build = Mock(return_value=(1, failure())); sleep = Mock()
        self.assertEqual(subject.run(build, sleep), 1)
        self.assertEqual(build.call_count, 3)
        self.assertEqual([c.args for c in sleep.call_args_list], [(10,), (20,)])

    def test_changed_error_stops(self):
        build = Mock(side_effect=[(1, failure()), (1, b'Error hash mismatch')]); sleep = Mock()
        self.assertEqual(subject.run(build, sleep), 1)
        self.assertEqual(build.call_count, 2)

    def test_installed_native_version_is_checked_before_build(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            main = root / 'web/ui/node_modules/@tauri-apps/cli/package.json'
            native = root / 'web/ui/node_modules' / subject.NATIVE_PACKAGE / 'package.json'
            main.parent.mkdir(parents=True); native.parent.mkdir(parents=True)
            main.write_text(json.dumps({'version': '2.12.4'}))
            env = {'GITHUB_ACTIONS': 'true', 'RUNNER_ENVIRONMENT': 'github-hosted', 'RUNNER_OS': 'Windows'}
            with patch.object(subject, 'ROOT', root), patch.object(subject, 'check_contract'), patch.object(subject, 'run') as run, patch.dict(subject.os.environ, env, clear=True):
                for name, version in [(subject.NATIVE_PACKAGE, '2.12.5'), ('other', '2.12.4')]:
                    native.write_text(json.dumps({'name': name, 'version': version}))
                    with self.assertRaises(ValueError): subject.main()
                    run.assert_not_called()
                native.write_text(json.dumps({'name': subject.NATIVE_PACKAGE, 'version': '2.12.4'}))
                run.return_value = 0
                self.assertEqual(subject.main(), 0)
                run.assert_called_once()

    def test_guard(self):
        with patch.dict(subject.os.environ, {}, clear=True), self.assertRaises(ValueError): subject.main()
        with patch.dict(subject.os.environ, {'GITHUB_ACTIONS':'true', 'RUNNER_ENVIRONMENT':'github-hosted', 'RUNNER_OS':'Windows', 'TAURI_BUNDLER_TOOLS_GITHUB_MIRROR':''}, clear=True), self.assertRaises(ValueError): subject.main()

if __name__ == '__main__': unittest.main()
