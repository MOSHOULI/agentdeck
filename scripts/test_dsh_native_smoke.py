#!/usr/bin/env python3
"""Offline safety regressions, not evidence of native runtime compatibility."""
import email.message
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

import dsh_native_smoke as subject
import test_dsh_subprocess_smoke as shared_tests


class Response(io.BytesIO):
    status = 200

    def __init__(self, data, url):
        super().__init__(data)
        self.url = url
        self.headers = email.message.Message()
        self.headers['Content-Length'] = str(len(data))

    def geturl(self):
        return self.url


class Tests(unittest.TestCase):
    def test_review_exact_four_pins_and_no_overrides(self):
        self.assertEqual(tuple(a['id'] for a in subject.load_review()), subject.IDS)
        for value in ('latest', 'node-pty@1.2.0', ''):
            with self.assertRaises(subject.Failure):
                subject.artifact(value)
        with patch.object(Path, 'read_bytes', return_value=b'{}'), self.assertRaises(subject.Failure):
            subject.load_review()
        run = subprocess.run([sys.executable, '-B', subject.__file__, '--url', 'https://example.org'], capture_output=True)
        self.assertEqual(run.returncode, 2)

    def test_metadata_raw_snapshots_and_contract(self):
        for a in subject.load_review():
            raw = (subject.REVIEW_PATH.parent / a['id'] / 'metadata.json').read_bytes()
            subject.validate_metadata(raw, a['id'])
            with self.assertRaises(subject.Failure):
                subject.validate_metadata(raw + b' ', a['id'])
            good = json.loads(raw)
            for field, value in [('name', 'other'), ('version', 'latest'), ('dist', {}),
                                 ('scripts', {'install': 'evil'}), ('cpu', ['other']), ('dependencies', {'evil': '*'})]:
                with self.subTest(package=a['id'], field=field), self.assertRaises(subject.Failure):
                    subject.validate_metadata(json.dumps({**good, field: value}).encode(), a['id'])
            for raw in (b'[]', b'\xff', b'{"name":1,"name":2}'):
                with self.assertRaises(subject.Failure):
                    subject.validate_metadata(raw, a['id'])

    def test_closed_transport(self):
        a = subject.artifact('node-pty')
        for url in ('http://registry.npmjs.org/node-pty/1.2.0-beta.15', a['metadata_url'] + '?x=1', 'https://example.org'):
            with self.assertRaises(subject.Failure):
                subject.fetch(url, 262144)
        with self.assertRaises(subject.Failure):
            subject.fetch(a['archive_url'], a['archive_bytes'] + 1)
        opener = Mock()
        opener.open.return_value = Response(b'abc', a['metadata_url'])
        self.assertEqual(subject.fetch(a['metadata_url'], 262144, opener), b'abc')
        request = opener.open.call_args.args[0]
        self.assertEqual(request.get_header('Accept-encoding'), 'identity')
        self.assertEqual(opener.open.call_args.kwargs, {'timeout': 15})
        for mode in ('redirect', 'status', 'duplicate', 'missing', 'negative', 'unicode', 'huge', 'zero', 'mismatch', 'gzip', 'transfer', 'overflow', 'chunk'):
            response = Response(b'abc', a['metadata_url'])
            if mode == 'redirect': response.url += '/redirect'
            if mode == 'status': response.status = 302
            if mode == 'duplicate': response.headers['Content-Length'] = '3'
            if mode == 'missing': del response.headers['Content-Length']
            if mode in ('negative', 'unicode', 'huge', 'zero', 'mismatch'):
                response.headers.replace_header('Content-Length', {'negative': '-1', 'unicode': '٣', 'huge': '999999999', 'zero': '0', 'mismatch': '4'}[mode])
            if mode == 'gzip': response.headers['Content-Encoding'] = 'gzip'
            if mode == 'transfer': response.headers['Transfer-Encoding'] = 'chunked'
            if mode == 'overflow': response = Response(b'x' * 262145, a['metadata_url']); response.headers.replace_header('Content-Length', '262144')
            if mode == 'chunk': response.read1 = lambda count: b'x' * 16385
            opener.open.return_value = response
            with self.subTest(mode=mode), self.assertRaises(subject.Failure):
                subject.fetch(a['metadata_url'], 262144, opener)
        for ticks in ([0, 46], [0, 1, 46]):
            opener.open.return_value = Response(b'abc', a['metadata_url'])
            with self.assertRaisesRegex(subject.Failure, 'fetch_deadline'):
                subject.fetch(a['metadata_url'], 262144, opener, clock=Mock(side_effect=ticks))

    def test_default_fetch_forbids_redirects(self):
        a = subject.artifact('koffi')
        opener = Mock()
        opener.open.return_value = Response(b'abc', a['metadata_url'])
        with patch.object(subject.urllib.request, 'build_opener', return_value=opener) as factory:
            subject.fetch(a['metadata_url'], 262144)
        self.assertIsInstance(factory.call_args.args[0], subject.common.guard.NoRedirect)

    def test_integrity_rejects_wrong_size_and_same_length_bytes(self):
        for a in subject.load_review():
            for data in (b'', b'x' * a['archive_bytes'], b'x' * (a['archive_bytes'] + 1)):
                with self.assertRaises(subject.Failure):
                    subject.validate_archive(data, a['id'])

    def test_sha512_is_independently_required(self):
        a = subject.artifact('koffi')
        # Freeze only the manifest lookup and SHA256 result, leaving SHA512 real.
        # This proves matching size/SHA256 cannot bypass the independent SRI gate.
        with patch.object(subject, 'artifact', return_value=a), patch.object(subject.hashlib, 'sha256', return_value=Mock(hexdigest=lambda: a['archive_sha256'])):
            with self.assertRaisesRegex(subject.Failure, 'archive_integrity_mismatch'):
                subject.validate_archive(b'x' * a['archive_bytes'], a['id'])

    def test_supervisor_malformed_outcome_and_early_death_cleanup(self):
        for outcome in (None, [], (True,), (1, {}), (True, {}, 'extra')):
            process = Mock(pid=12345); process.is_alive.return_value = False
            receiver = Mock(); receiver.poll.return_value = True; receiver.recv.return_value = outcome
            with patch.object(subject.common.os, 'killpg', side_effect=ProcessLookupError), self.assertRaises(subject.Failure):
                subject.supervise(process, receiver)
            process.kill.assert_called_once_with()
            process.join.assert_called_once_with(5)

    def test_report_strict_keys_types_claims_and_declarations(self):
        for a in subject.load_review():
            good = {**subject.expected_report(a['id']), 'limitations': 'Static only; no runtime evidence.'}
            subject.validate_report(good, a['id'])
            for key, value in [('schema_version', True), ('bytes', True), ('regular_member_count', 0),
                               ('package_id', 'other'), ('declarations', {}), ('native_code_loaded', True),
                               ('install_ready', True), ('archive_verified_during_this_command', 1),
                               ('platform_compatibility', 'verified'), ('limitations', ''), ('limitations', ' '),
                               ('extra', True)]:
                with self.subTest(package=a['id'], key=key), self.assertRaises(subject.Failure):
                    subject.validate_report({**good, key: value}, a['id'])
            missing = dict(good); del missing['signature_verified']
            with self.assertRaises(subject.Failure): subject.validate_report(missing, a['id'])
        for raw in (b'{}{}', b'{"x":1,"x":2}', b'[]', b'\xff'):
            with self.assertRaises(subject.Failure): subject.decode_report(raw)
        self.assertFalse(subject.strict_equal({'a': [False]}, {'a': [0]}))

    def test_exclusive_private_files_and_complete_snapshot(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            sub = root / 'corrupt'; sub.mkdir()
            for index in range(8): subject.write_new((root if index < 4 else sub) / str(index), b'x' * 300000)
            before = subject.snapshot(root)
            self.assertEqual(len(before), 10)
            self.assertEqual((root / '0').stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError): subject.write_new(root / '0', b'overwrite')
            self.assertEqual(before, subject.snapshot(root))
            (sub / '7').write_bytes(b'y' * 300000)
            self.assertNotEqual(before, subject.snapshot(root))

    def test_snapshot_limits_links_and_special_files(self):
        for mode in ('symlink', 'hardlink', 'fifo', 'large', 'total', 'count', 'directories'):
            with tempfile.TemporaryDirectory() as d:
                root = Path(d)
                path = root / 'file'; subject.write_new(path, b'x')
                if mode == 'symlink': (root / 'link').symlink_to(path)
                if mode == 'hardlink': os.link(path, root / 'link')
                if mode == 'fifo': os.mkfifo(root / 'fifo')
                if mode == 'large':
                    with path.open('wb') as stream: stream.truncate(subject.MAX_ARCHIVE + 1)
                if mode == 'total':
                    for i in range(5):
                        with (root / str(i)).open('wb') as stream: stream.truncate(subject.MAX_ARCHIVE)
                if mode == 'count':
                    for i in range(16): subject.write_new(root / str(i), b'x')
                if mode == 'directories':
                    for i in range(33): (root / str(i)).mkdir()
                with self.subTest(mode=mode), self.assertRaises(subject.Failure): subject.snapshot(root)

    def test_cli_requires_trace_and_checks_mutation(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d); path = root / 'sentinel'; subject.write_new(path, b'original')
            def changed(*args):
                path.write_bytes(b'changed')
                return subprocess.CompletedProcess([], 0, b'{}', b'')
            with patch.object(subject, 'bounded_process', side_effect=changed), patch.object(subject, 'check_trace'), self.assertRaises(subject.Failure):
                subject.run_cli(Path('/built/aisync'), ['verify-native-package'], root)
            with patch.object(subject, 'bounded_process', return_value=subprocess.CompletedProcess([], 0, b'{}', b'')), self.assertRaises(subject.Failure):
                subject.run_cli(Path('/built/aisync'), ['verify-native-package'], root)

    def test_all_eight_calls_and_safe_corruption_contract(self):
        # Synthetic bytes exercise orchestration only, not archive verification.
        for bad_code, bad_stdout in ((1, b''), (0, b''), (2, b''), (-9, b''), (1, b'{}')):
            with tempfile.TemporaryDirectory() as d:
                root = Path(d) / 'fresh'
                calls = []
                def cli_run(cli, args, directory):
                    package_id = args[2]
                    archive = Path(args[4])
                    self.assertEqual(len(subject.snapshot(directory)), 10)
                    calls.append((package_id, archive.parent.name == 'corrupt'))
                    if archive.parent.name == 'corrupt':
                        self.assertEqual(archive.read_bytes(), b'`bc')
                        return subprocess.CompletedProcess([], bad_code, bad_stdout, b'rejected')
                    report = {**subject.expected_report(package_id), 'limitations': 'Static only'}
                    return subprocess.CompletedProcess([], 0, json.dumps(report).encode(), b'')
                with patch.object(subject, 'fetch', return_value=b'abc'), patch.object(subject, 'validate_metadata'), patch.object(subject, 'validate_archive'), patch.object(subject, 'run_cli', side_effect=cli_run):
                    if (bad_code, bad_stdout) == (1, b''):
                        result = subject.smoke(root, Path('/built/aisync'))
                        self.assertEqual(calls, [(p, corrupt) for p in subject.IDS for corrupt in (False, True)])
                        self.assertEqual(len(result['artifacts']), 4)
                        self.assertFalse(result['runtime_verified'])
                    else:
                        with self.assertRaises(subject.Failure): subject.smoke(root, Path('/built/aisync'))

    def test_no_files_written_before_integrity_and_no_directory_reuse(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d) / 'fresh'
            with patch.object(subject, 'fetch', return_value=b'bad'), patch.object(subject, 'validate_metadata'), self.assertRaises(subject.Failure):
                subject.smoke(root, Path('/built/aisync'))
            self.assertEqual(list(root.rglob('*.tgz')), [])
            with self.assertRaises(FileExistsError): subject.smoke(root, Path('/built/aisync'))

    def test_bounded_process_output_and_timeout(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaisesRegex(subject.Failure, 'cli_output_limit'):
                subject.bounded_process([sys.executable, '-c', 'import sys; sys.stdout.write("x"*70000)'], d, dict(os.environ))
            with self.assertRaisesRegex(subject.Failure, 'cli_timeout'):
                subject.bounded_process([sys.executable, '-c', 'import time; time.sleep(10)'], d, dict(os.environ), seconds=0.1)

    def test_shared_safety_primitives_are_not_replaced_or_weakened(self):
        for name in ('bounded_process', 'supervise', 'check_trace', 'validate_environment', 'write_new'):
            self.assertIs(getattr(subject, name), getattr(subject.common, name))
        self.assertEqual(subject.common.SIZE, 77649)
        self.assertEqual(subject.common.URL, 'https://registry.npmjs.org/@deepseek-ai/dsh-subprocess-local/-/dsh-subprocess-local-0.2.2-rc.1.tgz')
        with patch.dict(os.environ, {}, clear=True), self.assertRaises(subject.Failure):
            subject.validate_environment(Path('/tmp/test'), Path('/cli'))

    def test_ci_retains_old_lanes_and_only_uploads_summary(self):
        workflow = (subject.REVIEW_PATH.parents[4] / '.github/workflows/ci.yml').read_text()
        for name in ('dsh_distribution_smoke.py', 'dsh_subprocess_smoke.py', 'test_dsh_native_smoke.py', 'dsh_native_smoke.py'):
            self.assertIn(name, workflow)
        block = workflow.split('name: DSH-native-verification-', 1)[1].split('retention-days: 14', 1)[0]
        self.assertIn('path: ${{ runner.temp }}/dsh-native-evidence.json', block)
        self.assertNotIn('*.tgz', block)
        self.assertLess(workflow.index('sudo apt-get install --no-install-recommends -y strace'), workflow.index('python -B scripts/dsh_native_smoke.py'))


# These safety primitives are reused unchanged; execute their edge-case tests here
# too, including actual orphan cleanup and strict initial-exec-only trace parsing.
class SharedSafetyTests(unittest.TestCase):
    test_trace = shared_tests.Tests.test_exec_trace_exact_initial_cli_only
    test_supervisor_exited = shared_tests.Tests.test_supervisor_cleans_group_after_failed_worker_already_exited
    test_supervisor_edges = shared_tests.Tests.test_supervisor_cleans_on_crash_eof_timeout_and_success
    test_supervisor_real_cleanup = shared_tests.Tests.test_real_exited_worker_does_not_leave_live_group_child


if __name__ == '__main__':
    unittest.main()
