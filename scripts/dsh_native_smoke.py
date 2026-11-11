#!/usr/bin/env python3
"""Hosted Linux static acceptance for four reviewed DSH native dependency archives.
Only fetch and inspect bytes. Never extract, import, load or execute package contents.
No URL, version, digest, registry, tracer or execution-policy overrides are supported.
"""
import argparse
import base64
import hashlib
import json
import multiprocessing
import os
from pathlib import Path
import stat
import sys
import tempfile
import time
import urllib.request

import dsh_subprocess_smoke as common

Failure = common.Failure
write_new = common.write_new
bounded_process = common.bounded_process
check_trace = common.check_trace
decode_report = common.decode_report
supervise = common.supervise
validate_environment = common.validate_environment
REVIEW_SHA256 = '4565645d30baa92db67511f155ad774b1ca4a3d21b1a0e27b37783a248052573'
REVIEW_PATH = Path(__file__).resolve().parents[1] / 'docs/upstream/dsh/native-evidence/artifacts.json'
IDS = ('node-pty', 'koffi', 'koffi-linux-x64', 'koffi-win32-x64')
MAX_ARCHIVE = 7150506
MAX_TOTAL = 32 * 1024 * 1024


def load_review():
    raw = REVIEW_PATH.read_bytes()
    if hashlib.sha256(raw).hexdigest() != REVIEW_SHA256:
        raise Failure('review_manifest_changed')
    review = json.loads(raw, object_pairs_hook=common.no_duplicates)
    artifacts = review['artifacts']
    if tuple(a['id'] for a in artifacts) != IDS:
        raise Failure('review_manifest_changed')
    return artifacts


def artifact(package_id):
    for item in load_review():
        if item['id'] == package_id:
            return item
    raise Failure('package_not_allowed')


def fetch(url, limit, opener=None, clock=time.monotonic):
    allowed = {a[key]: (262144 if key == 'metadata_url' else a['archive_bytes'])
               for a in load_review() for key in ('metadata_url', 'archive_url')}
    if url not in allowed or type(limit) is not int or limit != allowed[url]:
        raise Failure('source_not_allowed')
    client = opener or urllib.request.build_opener(common.guard.NoRedirect())
    deadline = clock() + 45
    request = urllib.request.Request(url, headers={'Accept-Encoding': 'identity', 'User-Agent': 'AgentDeck-CI-DSH-verifier'})
    with client.open(request, timeout=15) as response:
        if response.status != 200 or response.geturl() != url:
            raise Failure('http_response_rejected')
        if response.headers.get('Content-Encoding', 'identity').lower() != 'identity' or response.headers.get('Transfer-Encoding'):
            raise Failure('encoding_rejected')
        lengths = response.headers.get_all('Content-Length', [])
        if len(lengths) != 1 or not lengths[0].isascii() or not lengths[0].isdigit() or len(lengths[0]) > 8:
            raise Failure('length_rejected')
        expected = int(lengths[0])
        if not 0 < expected <= limit:
            raise Failure('length_rejected')
        data = bytearray()
        while True:
            if clock() >= deadline:
                raise Failure('fetch_deadline')
            block = response.read1(min(16384, limit - len(data) + 1))
            if clock() >= deadline:
                raise Failure('fetch_deadline')
            if not block:
                break
            if len(block) > 16384 or len(data) + len(block) > limit:
                raise Failure('body_limit')
            data.extend(block)
        if len(data) != expected:
            raise Failure('length_mismatch')
        return bytes(data)


def strict_equal(value, expected):
    if type(value) is not type(expected):
        return False
    if isinstance(expected, dict):
        return value.keys() == expected.keys() and all(strict_equal(value[k], v) for k, v in expected.items())
    if isinstance(expected, list):
        return len(value) == len(expected) and all(strict_equal(a, b) for a, b in zip(value, expected))
    return value == expected


def validate_metadata(raw, package_id):
    a = artifact(package_id)
    try:
        metadata = json.loads(raw, object_pairs_hook=common.no_duplicates)
    except (ValueError, UnicodeError, RecursionError):
        raise Failure('metadata_invalid') from None
    if not isinstance(metadata, dict):
        raise Failure('metadata_invalid')
    dist = metadata.get('dist')
    if (metadata.get('name') != a['package'] or metadata.get('version') != a['version']
            or not isinstance(dist, dict) or dist.get('tarball') != a['archive_url'] or dist.get('integrity') != a['integrity']
            or not strict_equal({k: metadata.get(k) for k in a['declarations']}, a['declarations'])):
        raise Failure('metadata_contract_changed')
    # Raw bytes are deliberately pinned too: registry changes require review, not silent acceptance.
    if hashlib.sha256(raw).hexdigest() != a['metadata_sha256']:
        raise Failure('metadata_snapshot_changed')


def validate_archive(data, package_id):
    a = artifact(package_id)
    if (len(data) != a['archive_bytes'] or hashlib.sha256(data).hexdigest() != a['archive_sha256']
            or 'sha512-' + base64.b64encode(hashlib.sha512(data).digest()).decode() != a['integrity']):
        raise Failure('archive_integrity_mismatch')


def snapshot(directory):
    """Bounded whole-fixture walk, without following links or trusting file size alone."""
    result = {}
    pending = [directory]
    file_count = total = 0
    keys = ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_size', 'st_nlink', 'st_mtime_ns', 'st_ctime_ns')
    while pending:
        path = pending.pop()
        m = path.lstat()
        if len(result) >= 32 or m.st_uid != os.geteuid():
            raise Failure('unsafe_local_file')
        digest = None
        if stat.S_ISDIR(m.st_mode):
            # Stop enumeration before an attacker can force an unbounded list.
            with os.scandir(path) as entries:
                for entry in entries:
                    if len(pending) + len(result) >= 31:
                        raise Failure('unexpected_filesystem_changes')
                    pending.append(Path(entry.path))
        elif stat.S_ISREG(m.st_mode):
            file_count += 1
            total += m.st_size
            if file_count > 16 or m.st_nlink != 1 or m.st_size > MAX_ARCHIVE or total > MAX_TOTAL:
                raise Failure('unsafe_local_file')
            with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
                opened = os.fstat(stream.fileno())
                if any(getattr(opened, k) != getattr(m, k) for k in keys):
                    raise Failure('unsafe_local_file')
                data = stream.read(m.st_size + 1)
                if len(data) != m.st_size or any(getattr(os.fstat(stream.fileno()), k) != getattr(m, k)
                                               or getattr(path.lstat(), k) != getattr(m, k) for k in keys):
                    raise Failure('unsafe_local_file')
                digest = hashlib.sha256(data).hexdigest()
        else:
            raise Failure('unsafe_local_file')
        result[str(path.relative_to(directory))] = (*[getattr(m, k) for k in keys], digest)
    return result


def run_cli(cli, args, directory):
    env = dict(os.environ, HOME=str(directory), XDG_CONFIG_HOME=str(directory), XDG_DATA_HOME=str(directory), XDG_CACHE_HOME=str(directory))
    before = snapshot(directory)
    with tempfile.TemporaryDirectory(prefix='dsh-native-exec-trace-', dir=directory.parent) as trace_root:
        trace = Path(trace_root) / 'exec.trace'
        write_new(trace, b'')
        result = bounded_process(['/usr/bin/strace', '-f', '-qq', '-s', '4096', '-o', str(trace),
                                  '-e', 'trace=execve,execveat', str(cli), 'dsh', *args], directory, env)
        check_trace(trace, cli)
        if snapshot(directory) != before:
            raise Failure('cli_changed_fixture')
    return result


def expected_report(package_id):
    a = artifact(package_id)
    return {'schema_version': 1, 'state': 'native_archive_static_verified', 'package_id': package_id,
            'package': a['package'], 'version': a['version'], 'archive_name': a['archive_name'],
            'bytes': a['archive_bytes'], 'sha256': a['archive_sha256'], 'integrity': a['integrity'],
            'regular_member_count': len(a['members']), 'archive_verified_during_this_command': True,
            'reviewed_manifest_and_sources_matched': True,
            'metadata_archive_agreement': 'reviewed_declaration_fields_match', 'declarations': a['declarations'],
            'lifecycle_executed': False, 'native_code_loaded': False, 'closure_complete': False,
            'install_ready': False, 'dependencies_resolved': False, 'runtime_verified': False,
            'signature_verified': False, 'platform_compatibility': 'unverified'}


def validate_report(report, package_id):
    expected = expected_report(package_id)
    if (not isinstance(report, dict) or set(report) != set(expected) | {'limitations'}
            or not isinstance(report.get('limitations'), str) or not report['limitations'].strip()
            or not strict_equal({k: report.get(k) for k in expected}, expected)):
        raise Failure('cli_report_invalid')


def smoke(directory, cli):
    directory.mkdir(mode=0o700, exist_ok=False)
    # Prepare the entire chosen fixture before any CLI invocation, so every check
    # compares all eight archives and all directories, not just its own input.
    items = []
    bad_directory = directory / 'corrupt'
    bad_directory.mkdir(mode=0o700)
    for a in load_review():
        package_id = a['id']
        raw = fetch(a['metadata_url'], 262144)
        validate_metadata(raw, package_id)
        data = fetch(a['archive_url'], a['archive_bytes'])
        validate_archive(data, package_id)
        target = directory / a['archive_name']
        write_new(target, data)
        bad = bad_directory / a['archive_name']
        write_new(bad, bytes([data[0] ^ 1]) + data[1:])
        items.append((a, target, bad))
    baseline = snapshot(directory)
    evidence = []
    for a, target, bad in items:
        args = ['verify-native-package', '--package', a['id'], '--archive']
        success = run_cli(cli, [*args, str(target)], directory)
        if success.returncode != 0 or success.stderr:
            raise Failure('cli_valid_archive_rejected')
        validate_report(decode_report(success.stdout), a['id'])
        rejected = run_cli(cli, [*args, str(bad)], directory)
        if rejected.returncode != 1 or rejected.stdout:
            raise Failure('cli_corrupt_archive_not_safely_rejected')
        evidence.append({'package_id': a['id'], 'version': a['version'], 'bytes': a['archive_bytes'],
                         'sha256': a['archive_sha256'], 'metadata_sha256': a['metadata_sha256'],
                         'real_cli_valid_and_corrupt_checks': True, 'valid_cli_exec_count': 1,
                         'corrupt_cli_exec_count': 1})
    if snapshot(directory) != baseline:
        raise Failure('cli_changed_fixture')
    return {'schema_version': 1, 'state': 'pinned_native_archives_verified_not_installed',
            'review_sha256': REVIEW_SHA256, 'artifacts': evidence, 'fixture_zero_write': True,
            'exec_trace_verified': True, 'dsh_executed': False, 'lifecycle_executed': False,
            'native_code_loaded': False, 'closure_complete': False, 'install_ready': False,
            'dependencies_resolved': False, 'runtime_verified': False, 'signature_verified': False,
            'platform_compatibility': 'unverified'}


def worker(directory, cli, sender):
    os.setsid()
    try:
        try:
            directory, cli = Path(directory), Path(cli)
            validate_environment(directory, cli)
            outcome = (True, smoke(directory, cli))
        except Exception as error:
            outcome = (False, str(error) if isinstance(error, Failure) else 'dsh_native_smoke_failed')
        sender.send(outcome)
        sender.recv()  # Remain session leader until supervisor terminates the whole group.
    except EOFError:
        pass
    finally:
        sender.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', type=Path, required=True)
    parser.add_argument('--cli', type=Path, required=True)
    args = parser.parse_args()
    try:
        validate_environment(args.destination, args.cli)
        context = multiprocessing.get_context('spawn')
        receiver, sender = context.Pipe(duplex=True)
        process = context.Process(target=worker, args=(str(args.destination), str(args.cli), sender))
        process.start()
        sender.close()
        try:
            result = supervise(process, receiver)
        finally:
            receiver.close()
            if not process.is_alive():
                process.close()
        print(json.dumps(result, sort_keys=True))
    except Exception as error:
        print('DSH native smoke failed: ' + (str(error) if isinstance(error, Failure) else 'setup_failed'), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
