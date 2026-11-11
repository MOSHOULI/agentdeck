#!/usr/bin/env python3
"""Hosted Linux CI: fetch pinned subprocess-local bytes and verify using the real Rust CLI.
No filesystem extraction, package manager, Node, DSH startup, configuration or PATH changes.
The built verifier parses the fixed archive in memory; both CLI calls require exec tracing.
"""
import argparse
import base64
import hashlib
import json
import multiprocessing
import os
from pathlib import Path
import subprocess
import selectors
import signal
import stat
import re
import resource
import tempfile
import sys
import time
import urllib.request
import fetch_qoder_ui_archive as guard

VERSION = '0.2.2-rc.1'
META_URL = 'https://registry.npmjs.org/@deepseek-ai%2fdsh-subprocess-local/0.2.2-rc.1'
URL = 'https://registry.npmjs.org/@deepseek-ai/dsh-subprocess-local/-/dsh-subprocess-local-0.2.2-rc.1.tgz'
NAME = 'dsh-subprocess-local-0.2.2-rc.1.tgz'
SIZE = 77649
SHA256 = 'ac2aae27078f34ccc068c62371672da84fb83bf9230bc26b24e1ecaee9d4ae97'
SRI = 'sha512-/fakFwvyt20BMZTD+Dm89uHyFFMHZzWPmvHWDj/m7Q8wpgeWxX2lFtS2nVFVaqZBmgGUWkUr4tPGPa6a0gLyMw=='
Failure = guard.DownloadFailure


def no_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result: raise Failure('duplicate_json_key')
        result[key] = value
    return result


def fetch(url, limit, opener=None, clock=time.monotonic):
    if url not in (META_URL, URL): raise Failure('source_not_allowed')
    client = opener or urllib.request.build_opener(guard.NoRedirect())
    deadline = clock() + 45
    request = urllib.request.Request(url, headers={'Accept-Encoding': 'identity', 'User-Agent': 'AgentDeck-CI-DSH-verifier'})
    with client.open(request, timeout=15) as response:
        if response.status != 200 or response.geturl() != url: raise Failure('http_response_rejected')
        if response.headers.get('Content-Encoding', 'identity').lower() != 'identity' or response.headers.get('Transfer-Encoding'):
            raise Failure('encoding_rejected')
        lengths = response.headers.get_all('Content-Length', [])
        if len(lengths) != 1 or not lengths[0].isascii() or not lengths[0].isdigit() or len(lengths[0]) > 8:
            raise Failure('length_rejected')
        expected = int(lengths[0])
        if not 0 < expected <= limit: raise Failure('length_rejected')
        data = bytearray()
        while True:
            if clock() >= deadline: raise Failure('fetch_deadline')
            block = response.read1(min(16384, limit - len(data) + 1))
            if clock() >= deadline: raise Failure('fetch_deadline')
            if not block: break
            if len(block) > 16384 or len(data) + len(block) > limit: raise Failure('body_limit')
            data.extend(block)
        if len(data) != expected: raise Failure('length_mismatch')
        return bytes(data)


def validate_metadata(raw):
    try: metadata = json.loads(raw, object_pairs_hook=no_duplicates)
    except (ValueError, UnicodeError, RecursionError): raise Failure('metadata_invalid') from None
    if not isinstance(metadata, dict): raise Failure('metadata_invalid')
    dist = metadata.get('dist')
    if (metadata.get('name') != '@deepseek-ai/dsh-subprocess-local' or metadata.get('version') != VERSION
        or metadata.get('scripts') != {'postinstall': 'node scripts/ensure-spawn-helper.mjs'}
        or not isinstance(dist, dict) or dist.get('tarball') != URL or dist.get('integrity') != SRI):
        raise Failure('metadata_contract_changed')


def validate_archive(data):
    if len(data) != SIZE or hashlib.sha256(data).hexdigest() != SHA256 or 'sha512-' + base64.b64encode(hashlib.sha512(data).digest()).decode() != SRI:
        raise Failure('archive_integrity_mismatch')


def write_new(path, data):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as output:
        output.write(data); output.flush(); os.fsync(output.fileno())


def snapshot(directory):
    result = {}
    paths = [directory, *sorted(directory.rglob('*'))]
    if len(paths) > 12: raise Failure('unexpected_filesystem_changes')
    for path in paths:
        m = path.lstat()
        if stat.S_ISLNK(m.st_mode) or not (stat.S_ISREG(m.st_mode) or stat.S_ISDIR(m.st_mode)):
            raise Failure('unsafe_local_file')
        if m.st_uid != os.geteuid() or (stat.S_ISREG(m.st_mode) and (m.st_nlink != 1 or m.st_size > 262144)):
            raise Failure('unsafe_local_file')
        digest = None
        if stat.S_ISREG(m.st_mode):
            with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
                opened = os.fstat(stream.fileno())
                if (opened.st_dev, opened.st_ino, opened.st_mode, opened.st_size, opened.st_nlink) != (m.st_dev, m.st_ino, m.st_mode, m.st_size, m.st_nlink):
                    raise Failure('unsafe_local_file')
                data = stream.read(262145)
                if len(data) != m.st_size or any(getattr(os.fstat(stream.fileno()), key) != getattr(opened, key) or getattr(path.lstat(), key) != getattr(m, key) for key in ('st_dev','st_ino','st_mode','st_size','st_nlink','st_mtime_ns','st_ctime_ns')):
                    raise Failure('unsafe_local_file')
                digest = hashlib.sha256(data).hexdigest()
        result[str(path.relative_to(directory))] = (m.st_ino, m.st_mode, m.st_size, m.st_mtime_ns, m.st_ctime_ns, digest)
    return result


def bounded_process(argv, directory, env, seconds=30):
    def file_limit():
        resource.setrlimit(resource.RLIMIT_FSIZE, (65536, 65536))
    process = subprocess.Popen(argv, cwd=directory, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, preexec_fn=file_limit)
    buffers = [bytearray(), bytearray()]
    deadline = time.monotonic() + seconds
    try:
        with selectors.DefaultSelector() as selector:
            for index, stream in enumerate((process.stdout, process.stderr)):
                os.set_blocking(stream.fileno(), False); selector.register(stream, selectors.EVENT_READ, index)
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0: raise Failure('cli_timeout')
                for key, _ in selector.select(min(remaining, 0.1)):
                    block = os.read(key.fileobj.fileno(), 8192)
                    if not block: selector.unregister(key.fileobj); continue
                    target = buffers[key.data]
                    if len(target) + len(block) > 65536: raise Failure('cli_output_limit')
                    target.extend(block)
            remaining = deadline - time.monotonic()
            if remaining <= 0: raise Failure('cli_timeout')
            try: code = process.wait(timeout=remaining)
            except subprocess.TimeoutExpired: raise Failure('cli_timeout') from None
        return subprocess.CompletedProcess(argv, code, bytes(buffers[0]), bytes(buffers[1]))
    finally:
        if process.poll() is None:
            process.kill(); process.wait(timeout=5)
        process.stdout.close(); process.stderr.close()


def run_cli(cli, args, directory):
    env = dict(os.environ, HOME=str(directory), XDG_CONFIG_HOME=str(directory), XDG_DATA_HOME=str(directory), XDG_CACHE_HOME=str(directory))
    before = snapshot(directory)
    trace_root = Path(tempfile.mkdtemp(prefix='dsh-exec-trace-', dir=directory.parent))
    trace = trace_root / 'exec.trace'
    write_new(trace, b'')
    result = bounded_process(['/usr/bin/strace', '-f', '-qq', '-s', '4096', '-o', str(trace),
                              '-e', 'trace=execve,execveat', str(cli), 'dsh', *args], directory, env)
    check_trace(trace, cli)
    if snapshot(directory) != before: raise Failure('cli_changed_fixture')
    return result


def check_trace(path, cli):
    if path.stat().st_size > 65536: raise Failure('execution_trace_oversized')
    lines = path.read_text().splitlines()
    if len(lines) != 1: raise Failure('unexpected_execution_trace')
    match = re.fullmatch(r'\d+\s+execve\("([^"\\]*)", .*\)\s+= 0', lines[0])
    if match is None or match[1] != str(cli): raise Failure('unexpected_execution_trace')


def decode_report(raw):
    try: value = json.loads(raw, object_pairs_hook=no_duplicates)
    except (ValueError, UnicodeError, RecursionError): raise Failure('cli_report_invalid') from None
    if not isinstance(value, dict): raise Failure('cli_report_invalid')
    return value


def validate_report(report):
    expected = {'state': 'subprocess_archive_static_verified', 'package': '@deepseek-ai/dsh-subprocess-local',
                'version': VERSION, 'archive_name': NAME, 'bytes': SIZE, 'sha256': SHA256, 'integrity': SRI,
                'archive_verified_during_this_command': True, 'manifest_snapshot_matched': True,
                'postinstall_snapshot_matched': True, 'metadata_archive_agreement': 'reviewed_declaration_fields_match',
                'regular_member_count': 35, 'closure_complete': False, 'install_ready': False,
                'dependencies_resolved': False, 'runtime_verified': False, 'signature_verified': False}
    if not isinstance(report, dict) or set(report) != set(expected) | {'limitations'} or not isinstance(report.get('limitations'),str) or not report['limitations'] or any(type(report.get(k)) is not type(v) or report.get(k) != v for k,v in expected.items()):
        raise Failure('cli_report_invalid')



def fetch_package(directory):
    directory.mkdir(mode=0o700, exist_ok=False)  # No cache trust or reuse.
    raw = fetch(META_URL, 262144); validate_metadata(raw)
    data = fetch(URL, SIZE); validate_archive(data)
    target = directory / NAME; write_new(target, data)
    return raw, data, target


def smoke(directory, cli):
    raw, data, target = fetch_package(directory)
    success = run_cli(cli, ['verify-subprocess-package', '--archive', str(target)], directory)
    if success.returncode != 0: raise Failure('cli_valid_archive_rejected')
    report = decode_report(success.stdout)
    validate_report(report)
    bad_directory = directory / 'corrupt'; bad_directory.mkdir(mode=0o700)
    bad = bad_directory / NAME; write_new(bad, bytes([data[0] ^ 1]) + data[1:])
    rejected = run_cli(cli, ['verify-subprocess-package', '--archive', str(bad)], directory)
    if rejected.returncode != 1 or rejected.stdout:
        raise Failure('cli_corrupt_archive_not_safely_rejected')
    validate_archive(target.read_bytes())
    return {'state': 'pinned_subprocess_package_verified_not_installed', 'version': VERSION, 'bytes': SIZE,
            'sha256': SHA256, 'metadata_sha256': hashlib.sha256(raw).hexdigest(),
            'real_cli_valid_and_corrupt_checks': True, 'fixture_zero_write': True,
            'dsh_executed': False, 'lifecycle_executed': False, 'dependencies_resolved': False,
            'valid_cli_exec_count': 1, 'corrupt_cli_exec_count': 1, 'exec_trace_verified': True}


def validate_environment(directory, cli):
    guard._guard_destination(directory)
    if not Path('/usr/bin/strace').is_file(): raise Failure('tracer_required')
    if sys.platform != 'linux' or os.environ.get('RUNNER_OS') != 'Linux': raise Failure('requires_hosted_linux')
    if not cli.is_absolute() or not cli.is_file() or cli.is_symlink() or not os.access(cli, os.X_OK): raise Failure('explicit_cli_required')


def worker(directory, cli, sender):
    os.setsid()  # Own group; traced CLI/tracees stay in it.
    try:
        try:
            directory, cli = Path(directory), Path(cli)
            validate_environment(directory, cli)
            outcome = (True, smoke(directory, cli))
        except Exception as error:
            outcome = (False, str(error) if isinstance(error, Failure) else 'dsh_subprocess_smoke_failed')
        sender.send(outcome)
        # Keep the session leader alive until the parent has consumed the result
        # and kills the whole group. Never exit first and strand a detached tracee.
        # Parent never sends an acknowledgement: cleanup terminates this wait.
        sender.recv()
    except EOFError:
        pass
    finally:
        sender.close()


def supervise(process, receiver):
    # Do not join/poll Process/inspect exitcode before group cleanup: an abruptly
    # dead worker stays unreaped, reserving its PID against reuse. Normal workers
    # deliberately remain alive after sending their outcome. Thus killpg targets
    # our still-owned session, including tracees left behind if strace died.
    try:
        if not receiver.poll(240): raise Failure('worker_deadline_partial_retained')
        try: outcome = receiver.recv()
        except EOFError: raise Failure('worker_failed') from None
        if (not isinstance(outcome, tuple) or len(outcome) != 2
                or type(outcome[0]) is not bool): raise Failure('worker_failed')
        success, result = outcome
        if not success: raise Failure(result)
        return result
    finally:
        try: os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            # Worker may have died before setsid. Its PID has not been reaped.
            try: process.kill()
            except ProcessLookupError: pass
        process.join(5)
        if process.is_alive():
            process.kill(); process.join(5)
            raise Failure('worker_cleanup_failed')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', type=Path, required=True)
    parser.add_argument('--cli', type=Path, required=True)
    args = parser.parse_args()
    try:
        validate_environment(args.destination, args.cli)
        context = multiprocessing.get_context('spawn'); receiver, sender = context.Pipe(duplex=True)
        process = context.Process(target=worker, args=(str(args.destination), str(args.cli), sender)); process.start(); sender.close()
        try: result = supervise(process, receiver)
        finally:
            receiver.close()
            if not process.is_alive(): process.close()
        print(json.dumps(result, sort_keys=True))
    except Exception as error:
        print('DSH smoke failed: ' + (str(error) if isinstance(error, Failure) else 'setup_failed'), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__': sys.exit(main())
