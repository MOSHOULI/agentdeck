"""Offline safety tests; never fetch a package or run its contents."""
import email.message
import hashlib
import json
import multiprocessing
import time
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import fetch_qoder_ui_archive as download

def _stall_worker():
    time.sleep(60)

class Response:
    def __init__(self, data=b'fixture', status=200, url=None, length='7', encoding=None, transfer=None):
        self.data = data
        self.status = status
        self.url = download.URL if url is None else url
        self.headers = email.message.Message()
        if length is not None: self.headers['Content-Length'] = length
        if encoding: self.headers['Content-Encoding'] = encoding
        if transfer: self.headers['Transfer-Encoding'] = transfer
    def geturl(self): return self.url
    def read1(self, maximum):
        result, self.data = self.data[:maximum], self.data[maximum:]
        return result
    def __enter__(self): return self
    def __exit__(self, *args): pass
class Opener:
    def __init__(self, response): self.response = response
    def open(self, request, timeout):
        assert request.full_url == download.URL
        assert timeout == download.SOCKET_SECONDS
        return self.response

class FetchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.parent = Path(self.temp.name)
        self.dest = self.parent / 'new'
        self.patch = patch.multiple(download, SIZE=7, SHA256=hashlib.sha256(b'fixture').hexdigest())
        self.patch.start(); self.addCleanup(self.patch.stop)
    def fetch(self, response, **kwargs): return download._fetch(self.dest, Opener(response), **kwargs)
    def test_valid_exact_response_and_no_extra_files(self):
        self.assertEqual(self.fetch(Response())['bytes'], 7)
        self.assertEqual((self.dest / download.NAME).read_bytes(), b'fixture')
        self.assertEqual([p.name for p in self.dest.iterdir()], [download.NAME])
    def test_redirect_handler_and_final_url_rejected(self):
        with self.assertRaises(download.DownloadFailure): download.NoRedirect().redirect_request(None, None, 302, '', {}, download.URL)
        with self.assertRaises(download.DownloadFailure): self.fetch(Response(url='https://evil.invalid/archive'))
    def test_wrong_status_length_encoding_transfer_or_duplicate_length(self):
        duplicate = Response(); duplicate.headers['Content-Length'] = '7'
        for response in [Response(status=302), Response(status=206), Response(length=None), Response(length='8'), Response(length='999999999999'), Response(encoding='gzip'), Response(transfer='chunked'), duplicate]:
            with self.subTest(response=response):
                self.dest = self.parent / f'case-{len(list(self.parent.iterdir()))}'
                with self.assertRaises(download.DownloadFailure): self.fetch(response)
                self.assertEqual((self.dest / download.NAME).read_bytes(), b'')
    def test_short_oversized_and_digest_mismatch_fail(self):
        for body in [b'', b'fixtur', b'fixtureX', b'changed']:
            self.dest = self.parent / f'case-{len(list(self.parent.iterdir()))}'
            with self.assertRaises(download.DownloadFailure): self.fetch(Response(data=body))
            self.assertLessEqual((self.dest / download.NAME).stat().st_size, 7)
    def test_existing_directory_is_not_reused_or_overwritten(self):
        self.dest.mkdir(); sentinel = self.dest / download.NAME; sentinel.write_bytes(b'keep')
        with self.assertRaises(FileExistsError): self.fetch(Response())
        self.assertEqual(sentinel.read_bytes(), b'keep')
    def test_deadline_failure_retains_only_partial_data(self):
        times = iter([0, download.TOTAL_SECONDS + 1])
        with self.assertRaises(download.DownloadFailure): self.fetch(Response(), clock=lambda: next(times))
        self.assertEqual((self.dest / download.NAME).read_bytes(), b'')
    def test_network_error_does_not_overwrite_anything(self):
        class Broken:
            def open(self, *args, **kwargs): raise OSError('SECRET transport error')
        with self.assertRaises(OSError): download._fetch(self.dest, Broken())
        self.assertEqual((self.dest / download.NAME).read_bytes(), b'')

class GuardAndSupervisorTests(unittest.TestCase):
    def test_normal_user_execution_is_rejected(self):
        with patch.dict('os.environ', {}, clear=True):
            with self.assertRaises(download.DownloadFailure): download._guard_destination(Path('/tmp/new'))
    def test_destination_guard_rejects_outside_traversal_and_linked_parent(self):
        with tempfile.TemporaryDirectory() as root:
            base = Path(root)
            environment = {'GITHUB_ACTIONS': 'true', 'RUNNER_ENVIRONMENT': 'github-hosted', 'RUNNER_OS': 'Windows', 'RUNNER_TEMP': str(base)}
            with patch.dict('os.environ', environment, clear=True):
                download._guard_destination(base / 'new')
                for dest in [base, base.parent / 'outside', base / '..' / 'new']:
                    with self.assertRaises(download.DownloadFailure): download._guard_destination(dest)
                for metadata in [SimpleNamespace(st_mode=0o120777, st_file_attributes=0), SimpleNamespace(st_mode=0o40700, st_file_attributes=0x400)]:
                    with patch.object(Path, 'lstat', return_value=metadata):
                        with self.assertRaises(download.DownloadFailure): download._guard_destination(base / 'new')
    def test_worker_failure_and_missing_report_do_not_count_as_success(self):
        class Finished:
            exitcode = 1
            def join(self, timeout): pass
            def is_alive(self): return False
        with self.assertRaises(download.DownloadFailure): download._supervise(Finished(), None)
        class NoReport:
            def poll(self): return False
        process = Finished(); process.exitcode = 0
        with self.assertRaises(download.DownloadFailure): download._supervise(process, NoReport())
    def test_fixed_contract_matches_reviewed_manifest(self):
        manifest = json.loads((Path(__file__).resolve().parents[1] / 'docs/upstream/qoder/manifest-1.1.85.json').read_text())
        entry = next(e for e in manifest['files'] if e['os'] == 'linux' and e['arch'] == 'amd64-baseline')
        self.assertEqual(download.URL, entry['url']); self.assertEqual(download.SHA256, entry['sha256']); self.assertEqual(download.SIZE, 78587775)
    def test_wall_clock_supervisor_stops_a_real_child(self):
        process = multiprocessing.get_context('spawn').Process(target=_stall_worker)
        process.start()
        try:
            with patch.object(download, 'TOTAL_SECONDS', 0.05):
                with self.assertRaises(download.DownloadFailure): download._supervise(process, None)
            self.assertFalse(process.is_alive())
        finally:
            if process.is_alive(): process.kill(); process.join(5)
            process.close()
    def test_worker_does_not_report_raw_transport_errors(self):
        class Sender:
            result = None
            def send(self, result): self.result = result
            def close(self): pass
        sender = Sender()
        with patch.object(download, '_guard_destination'), patch.object(download, '_fetch', side_effect=OSError('SECRET proxy error')):
            download._worker('/unused', sender)
        self.assertEqual(sender.result, (False, 'download_or_filesystem_failed'))
    def test_supervisor_terminates_stalled_worker(self):
        class Stalled:
            exitcode = 0
            alive = True
            terminated = False
            def join(self, timeout): pass
            def is_alive(self): return self.alive
            def terminate(self): self.terminated = True; self.alive = False
        process = Stalled()
        with self.assertRaises(download.DownloadFailure): download._supervise(process, None)
        self.assertTrue(process.terminated)

if __name__ == '__main__': unittest.main()
