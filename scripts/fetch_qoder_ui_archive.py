#!/usr/bin/env python3
"""CI-only pinned, bounded archive download. Never unpack or execute content."""
import argparse
import hashlib
import json
import multiprocessing
import os
import platform
from pathlib import Path
import stat
import sys
import time
import urllib.request

URL = 'https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.85/qodercli-linux-x64-baseline.tar.gz'
NAME = 'qodercli-linux-x64-baseline.tar.gz'
SIZE = 78_587_775
SHA256 = 'a2fd8cc04711603f9637f54b63097380efc79939739967985cd6193507472dc4'
CHUNK = 64 * 1024
TOTAL_SECONDS = 180
SOCKET_SECONDS = 15

class DownloadFailure(Exception):
    """Fixed error category: never dump remote content or raw transport errors."""

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise DownloadFailure('redirect_rejected')

def _guard_destination(directory):
    hosted = (os.environ.get('GITHUB_ACTIONS'), os.environ.get('RUNNER_ENVIRONMENT')) == ('true', 'github-hosted')
    windows = os.environ.get('RUNNER_OS') == 'Windows'
    linux = os.environ.get('RUNNER_OS') == 'Linux' and sys.platform == 'linux' and platform.machine() == 'x86_64'
    if not hosted or not (windows or linux):
        raise DownloadFailure('requires_hosted_windows_or_linux_x64_runner')
    root = os.environ.get('RUNNER_TEMP')
    if not root or not directory.is_absolute():
        raise DownloadFailure('requires_absolute_runner_temp_destination')
    try:
        directory.relative_to(Path(root))
    except ValueError:
        raise DownloadFailure('destination_outside_runner_temp') from None
    # Reject all existing linked/reparse ancestors and lexical traversal. This is
    # best effort against same-UID races, not a security/isolation namespace.
    if '..' in directory.parts or directory == Path(root):
        raise DownloadFailure('invalid_destination')
    for parent in directory.parents:
        metadata = parent.lstat()
        if not stat.S_ISDIR(metadata.st_mode) or stat.S_ISLNK(metadata.st_mode) or getattr(metadata, 'st_file_attributes', 0) & 0x400:
            raise DownloadFailure('unsafe_destination_parent')

def _fetch(directory, opener=None, clock=time.monotonic):
    """Private test seam; production has no URL/hash/size overrides or environment bypass."""
    directory.mkdir(mode=0o700, exist_ok=False)
    target = directory / NAME
    # Creation is exclusive even if an existing destination is a dangling link.
    fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, 'O_NOFOLLOW', 0), 0o600)
    with os.fdopen(fd, 'wb') as output:
        client = opener or urllib.request.build_opener(NoRedirect())
        request = urllib.request.Request(URL, headers={'Accept-Encoding': 'identity', 'User-Agent': 'AgentDeck-CI-Qoder-archive-verifier'})
        deadline = clock() + TOTAL_SECONDS
        with client.open(request, timeout=SOCKET_SECONDS) as response:
            if response.status != 200 or response.geturl() != URL:
                raise DownloadFailure('unexpected_http_response')
            if response.headers.get('Content-Encoding', 'identity').lower() != 'identity':
                raise DownloadFailure('content_encoding_rejected')
            lengths = response.headers.get_all('Content-Length', [])
            if len(lengths) != 1 or lengths[0] != str(SIZE):
                raise DownloadFailure('content_length_mismatch')
            if response.headers.get('Transfer-Encoding'):
                raise DownloadFailure('transfer_encoding_rejected')
            digest = hashlib.sha256()
            count = 0
            while True:
                if clock() >= deadline:
                    raise DownloadFailure('download_deadline_exceeded')
                block = response.read1(min(CHUNK, SIZE - count + 1))
                if clock() >= deadline:
                    raise DownloadFailure('download_deadline_exceeded')
                if not block:
                    break
                count += len(block)
                if len(block) > CHUNK or count > SIZE:
                    raise DownloadFailure('archive_size_exceeded')
                output.write(block)
                digest.update(block)
            if count != SIZE or digest.hexdigest() != SHA256:
                raise DownloadFailure('archive_integrity_mismatch')
        output.flush()
        os.fsync(output.fileno())
    return {'state': 'pinned_archive_downloaded_not_executed', 'bytes': SIZE, 'sha256': SHA256, 'target': 'linux-amd64-baseline', 'version': '1.1.85'}

def _worker(directory, sender):
    try:
        _guard_destination(Path(directory))
        sender.send((True, _fetch(Path(directory))))
    except Exception as error:
        category = str(error) if isinstance(error, DownloadFailure) else 'download_or_filesystem_failed'
        sender.send((False, category))
    finally:
        sender.close()

def _supervise(process, receiver):
    # A separate process imposes a true wall-clock bound even on slow headers,
    # read trickles, stalled TLS or filesystem calls; socket timeouts alone do not.
    process.join(TOTAL_SECONDS)
    if process.is_alive():
        process.terminate()
        process.join(5)
        if process.is_alive():
            process.kill()
            process.join(5)
        raise DownloadFailure('download_worker_deadline_exceeded_partial_may_remain')
    if process.exitcode != 0 or not receiver.poll():
        raise DownloadFailure('download_worker_failed_partial_may_remain')
    success, result = receiver.recv()
    if not success:
        raise DownloadFailure(result)
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', type=Path, required=True, help='NEW directory beneath RUNNER_TEMP; no existing directory reused')
    args = parser.parse_args()
    try:
        _guard_destination(args.destination)
        context = multiprocessing.get_context('spawn')
        receiver, sender = context.Pipe(duplex=False)
        process = context.Process(target=_worker, args=(str(args.destination), sender))
        process.start()
        sender.close()
        try:
            result = _supervise(process, receiver)
        finally:
            receiver.close()
            if not process.is_alive():
                process.close()
        print(json.dumps(result, sort_keys=True))
    except Exception as error:
        category = str(error) if isinstance(error, DownloadFailure) else 'download_setup_failed'
        print(f'Qoder CI download failed: {category}; partial data is retained, never executed or overwritten.', file=sys.stderr)
        return 1
    return 0

if __name__ == '__main__':
    sys.exit(main())
