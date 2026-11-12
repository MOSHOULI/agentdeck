#!/usr/bin/env python3
"""Hosted Linux CLI/private-deployment acceptance. Never execute Qoder.

Real fixed archive and payload hashes, source/config snapshots and exec tracing
are independent gates. No local bypass, weakened trace mode or hash override.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import signal
import stat
import subprocess
import tempfile

import qoder_complete_fixtures as complete


class Failure(Exception):
    """Fixed categories only; no paths, payload bytes or raw CLI diagnostics."""


def identity(m):
    return (m.st_dev, m.st_ino, m.st_uid, m.st_gid, m.st_mode, m.st_nlink,
            m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def file_hash(path, mode, size=None, expected=None):
    before = path.lstat()
    if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
            or before.st_uid != os.geteuid() or stat.S_IMODE(before.st_mode) != mode
            or before.st_size > max(c[2] for c in complete.pinned.releases.RELEASES.values()) or (size is not None and before.st_size != size)):
        raise Failure('unsafe_file')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        if identity(os.fstat(stream.fileno())) != identity(before): raise Failure('file_changed')
        digest = hashlib.sha256()
        count = 0
        while block := stream.read(65536):
            count += len(block)
            if count > before.st_size: raise Failure('file_changed')
            digest.update(block)
        if identity(os.fstat(stream.fileno())) != identity(before): raise Failure('file_changed')
    if count != before.st_size or identity(path.lstat()) != identity(before): raise Failure('file_changed')
    result = digest.hexdigest()
    if expected is not None and result != expected: raise Failure('hash_mismatch')
    return result


def snapshot(root):
    result = {}
    paths = [root]
    while paths:
        path = paths.pop()
        if len(result) > 64: raise Failure('too_many_entries')
        m = path.lstat()
        if m.st_uid != os.geteuid(): raise Failure('wrong_owner')
        if stat.S_ISDIR(m.st_mode):
            if stat.S_IMODE(m.st_mode) != 0o700: raise Failure('unsafe_directory')
            paths.extend(path.iterdir()); digest = None
        else:
            mode = stat.S_IMODE(m.st_mode)
            if mode not in (0o600, 0o700): raise Failure('unsafe_file_mode')
            digest = file_hash(path, mode)
        result[str(path.relative_to(root))] = (*identity(m), digest)
    return result


def check_trace(path, cli):
    if path.stat().st_size > 65536: raise Failure('execution_trace_oversized')
    lines = path.read_text().splitlines()
    # A single successful initial exec is required. Any extra exec attempt,
    # execveat, unfinished/truncated line or tracer failure is rejected.
    if len(lines) != 1: raise Failure('unexpected_execution_trace')
    match = re.fullmatch(r'\d+\s+execve\("([^"\\]*)", .*\)\s+= 0', lines[0])
    if match is None or match[1] != str(cli): raise Failure('unexpected_execution_trace')
    return 1


def run_preview(cli, cache, destination, env, trace):
    command = ['/usr/bin/strace', '-f', '-qq', '-s', '4096', '-o', str(trace),
               '-e', 'trace=execve,execveat', str(cli), 'qoder', 'deploy-preview',
               '--cache', str(cache), '--destination', str(destination)] + complete.release_args()
    result = complete.run_cli(command, env)
    check_trace(trace, cli)
    expected = {'version': complete.pinned.VERSION, 'target': 'linux-amd64-baseline',
                'payload_bytes': complete.PAYLOAD_BYTES, 'payload_sha256': complete.PAYLOAD_SHA256,
                'executable_mode': '0700'}
    if not isinstance(result, dict) or not isinstance(result.get('token'), str) or re.fullmatch('[0-9a-f]{64}', result['token']) is None:
        raise Failure('invalid_preview_token')
    exact_fields({key: value for key, value in result.items() if key != 'token'}, expected, limitations=True)
    return result['token']


def run_deploy(cli, cache, destination, token, env, trace, *, reject=False):
    command = ['/usr/bin/strace', '-f', '-qq', '-s', '4096', '-o', str(trace),
               '-e', 'trace=execve,execveat', str(cli), 'qoder', 'deploy-private',
               '--cache', str(cache), '--destination', str(destination), '--preview-token', token, '--yes'] + complete.release_args()
    if not reject:
        result = complete.run_cli(command, env)
    else:
        process = subprocess.Popen(command, env=env, stdout=subprocess.DEVNULL,
                                   stderr=subprocess.DEVNULL, start_new_session=True)
        try:
            try: code = process.wait(timeout=120)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL); process.wait(timeout=5)
                raise Failure('rejection_timeout') from None
            if code != 1: raise Failure('repeat_not_rejected')
        finally:
            if process.poll() is None:
                os.killpg(process.pid, signal.SIGKILL); process.wait(timeout=5)
        result = None
    check_trace(trace, cli)
    return result


def run_inspect(cli, destination, env, trace, state):
    command = ['/usr/bin/strace', '-f', '-qq', '-s', '4096', '-o', str(trace),
               '-e', 'trace=execve,execveat', str(cli), 'qoder', 'inspect-deployment',
               '--deployment', str(destination)] + complete.release_args()
    result = complete.run_cli(command, env)
    check_trace(trace, cli)
    expected = {'version': complete.pinned.VERSION, 'target': 'linux-amd64-baseline',
                'state': state, 'payload_bytes': complete.PAYLOAD_BYTES,
                'payload_sha256': complete.PAYLOAD_SHA256, 'execution_verified': False,
                'activated': False, 'ownership_proven': False, 'deletion_authorized': False,
                'repair_authorized': False, 'manual_review_required': True}
    exact_fields(result, expected, limitations=True)
    return result


def alter_authored_payload(destination):
    # Only this harness's previously validated, disposable deployment. Preserve
    # size/mode, flip one byte and retain the changed file; never execute it.
    payload = destination / 'qodercli'
    before = payload.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_size < 1:
        raise Failure('unsafe_tamper_fixture')
    fd = os.open(payload, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'r+b') as stream:
        if identity(os.fstat(stream.fileno())) != identity(before): raise Failure('file_changed')
        original = stream.read(1)
        stream.seek(0); stream.write(bytes([original[0] ^ 1])); stream.flush(); os.fsync(stream.fileno())
    after = payload.lstat()
    if (after.st_dev, after.st_ino, after.st_mode, after.st_size) != (before.st_dev, before.st_ino, before.st_mode, before.st_size):
        raise Failure('tamper_fixture_replaced')


# Product report and directory validators follow the fixed reviewed CLI contract.
OUTCOME = {'state': 'private_executable_deployed_not_activated', 'version': complete.pinned.VERSION,
           'target': 'linux-amd64-baseline', 'executable_mode': '0700',
           'execution_verified': False, 'activated': False}


def exact_fields(value, expected, *, limitations=False):
    keys = set(expected) | ({'limitations'} if limitations else set())
    if not isinstance(value, dict) or set(value) != keys: raise Failure('invalid_report_fields')
    for key, expected_value in expected.items():
        if type(value[key]) is not type(expected_value) or value[key] != expected_value:
            raise Failure('invalid_report_value')
    if limitations and (not isinstance(value['limitations'], str) or not value['limitations']):
        raise Failure('missing_limitations')


def validate_report(report):
    exact_fields(report, {**OUTCOME, 'version': complete.pinned.VERSION}, limitations=True)


def load_record(path):
    file_hash(path, 0o600)
    if path.stat().st_size > 65536: raise Failure('record_oversized')
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result: raise Failure('duplicate_record_key')
            result[key] = value
        return result
    try: return json.loads(path.read_bytes(), object_pairs_hook=unique)
    except (ValueError, UnicodeError): raise Failure('invalid_record_json') from None


def validate_destination(destination):
    state = snapshot(destination)
    if set(state) != {'.', 'qodercli', 'deployment.intent.json', 'deployment.complete.json'}:
        raise Failure('unexpected_deployment_members')
    file_hash(destination / 'qodercli', 0o700, complete.PAYLOAD_BYTES, complete.PAYLOAD_SHA256)
    exact_fields(load_record(destination / 'deployment.intent.json'),
                 {'format': 1, 'state': 'private_deployment_started', 'version': complete.pinned.VERSION,
                  'target': 'linux-amd64-baseline', 'execution_verified': False, 'activated': False})
    exact_fields(load_record(destination / 'deployment.complete.json'),
                 {**OUTCOME, 'version': complete.pinned.VERSION, 'format': 1, 'payload_bytes': complete.PAYLOAD_BYTES,
                  'payload_sha256': complete.PAYLOAD_SHA256}, limitations=True)


def guard(cli, archive, evidence):
    if ((os.environ.get('GITHUB_ACTIONS'), os.environ.get('RUNNER_ENVIRONMENT'), os.environ.get('RUNNER_OS'))
            != ('true', 'github-hosted', 'Linux') or platform.machine() != 'x86_64'):
        raise Failure('requires_hosted_linux_x64')
    if not cli.is_absolute() or not cli.is_file() or not os.access(cli, os.X_OK):
        raise Failure('explicit_cli_required')
    if not Path('/usr/bin/strace').is_file(): raise Failure('tracer_required')
    complete.pinned._guard_destination(evidence)
    complete.pinned._guard_destination(archive.parent)


def private_file(path, data):
    fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as stream: stream.write(data)


def run(cli, archive, evidence):
    guard(cli, archive, evidence)
    evidence.mkdir(mode=0o700, exist_ok=False)
    root = Path(tempfile.mkdtemp(prefix='agentdeck-private-deploy-', dir=os.environ['RUNNER_TEMP']))
    home = root / 'home'; home.mkdir(mode=0o700)
    for name in ('.qoder', '.agentdeck', 'config', 'cache', 'data', 'state'):
        directory = home / name; directory.mkdir(mode=0o700)
        private_file(directory / 'sentinel.json', b'{"synthetic":"preserve-private-config"}\n')
    env = {'HOME': str(home), 'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8',
           'XDG_CONFIG_HOME': str(home / 'config'), 'XDG_CACHE_HOME': str(home / 'cache'),
           'XDG_DATA_HOME': str(home / 'data'), 'XDG_STATE_HOME': str(home / 'state')}
    cache_root = root / 'caches'; cache_root.mkdir(mode=0o700)
    report = {'passed': False, 'version': complete.pinned.VERSION, 'target': 'linux-amd64-baseline',
              'latest_version_claimed': False, 'harness_invokes_qoder': False, 'traced_steps_qoder_executed': None,
              'boundary': 'Trace covers stage, unpack, preview, deploy, repeated deploy and missing/verified/tampered read-only inspection; configuration comparison covers synthetic HOME. Tamper case flips one byte in the harness-owned disposable payload and retains it; inspection must not repair it. Fixed private deployment, not runtime, login, PATH activation, service validation or proof of no other system side effects.',
              'source_unchanged': False, 'configuration_unchanged': False,
              'repeat_rejected_unchanged': False, 'preview_unchanged': False, 'preview_exec_count': 0, 'deploy_exec_count': 0, 'repeat_exec_count': 0, 'inspection_exec_count': 0, 'inspection_missing_unchanged': False,
              'inspection_verified_unchanged': False, 'inspection_tampered_rejected_unchanged': False}
    try:
        before_home = snapshot(home)
        before_archive = (identity(archive.lstat()), complete.file_digest(archive, complete.pinned.SIZE, (complete.pinned.SIZE, complete.pinned.SHA256)))
        preparation_traces = []
        def traced_preparation(command, environment):
            trace = root / f'preparation-{len(preparation_traces)}.trace'
            result = complete.run_cli(['/usr/bin/strace', '-f', '-qq', '-s', '4096', '-o', str(trace),
                                       '-e', 'trace=execve,execveat'] + command, environment)
            check_trace(trace, cli)
            preparation_traces.append(1)
            return result
        caches, _ = complete.prepare(cli, archive, cache_root, env, runner=traced_preparation)
        if preparation_traces != [1, 1]: raise Failure('missing_preparation_traces')
        report.update(stage_exec_count=1, unpack_exec_count=1)
        before_cache = snapshot(cache_root)
        if snapshot(home) != before_home: raise Failure('preparation_changed_config')
        destination = root / 'private-deployment'
        run_inspect(cli, destination, env, root / 'inspect-missing.trace', 'missing')
        if os.path.lexists(destination) or snapshot(cache_root) != before_cache or snapshot(home) != before_home:
            raise Failure('missing_inspection_changed_state')
        report.update(inspection_missing_unchanged=True, inspection_exec_count=1)
        token = run_preview(cli, caches['unpacked'], destination, env, root / 'preview.trace')
        if os.path.lexists(destination) or snapshot(cache_root) != before_cache or snapshot(home) != before_home:
            raise Failure('preview_changed_state')
        report.update(preview_unchanged=True, preview_exec_count=1)
        result = run_deploy(cli, caches['unpacked'], destination, token, env, root / 'deploy.trace')
        validate_report(result)
        validate_destination(destination)
        report['deploy_exec_count'] = 1
        deployed = snapshot(destination)
        run_inspect(cli, destination, env, root / 'inspect-verified.trace', 'payload_verified')
        if snapshot(destination) != deployed or snapshot(cache_root) != before_cache or snapshot(home) != before_home:
            raise Failure('verified_inspection_changed_state')
        report.update(inspection_verified_unchanged=True, inspection_exec_count=2)
        if snapshot(cache_root) != before_cache: raise Failure('source_cache_changed')
        if snapshot(home) != before_home: raise Failure('configuration_changed')
        run_deploy(cli, caches['unpacked'], destination, token, env, root / 'repeat.trace', reject=True)
        report['repeat_exec_count'] = 1
        if snapshot(destination) != deployed: raise Failure('repeated_deployment_changed_output')
        if snapshot(cache_root) != before_cache: raise Failure('source_cache_changed')
        if snapshot(home) != before_home: raise Failure('configuration_changed')
        alter_authored_payload(destination)
        changed = snapshot(destination)
        if changed == deployed: raise Failure('tamper_fixture_not_changed')
        run_inspect(cli, destination, env, root / 'inspect-tampered.trace', 'unsupported_or_changed')
        if snapshot(destination) != changed or snapshot(cache_root) != before_cache or snapshot(home) != before_home:
            raise Failure('tampered_inspection_changed_state')
        report.update(inspection_tampered_rejected_unchanged=True, inspection_exec_count=3)
        after_archive = (identity(archive.lstat()), complete.file_digest(archive, complete.pinned.SIZE, (complete.pinned.SIZE, complete.pinned.SHA256)))
        if before_archive != after_archive: raise Failure('source_archive_changed')
        report.update(passed=True, traced_steps_qoder_executed=False, source_unchanged=True, configuration_unchanged=True,
                      repeat_rejected_unchanged=True, payload_sha256=complete.PAYLOAD_SHA256,
                      payload_bytes=complete.PAYLOAD_BYTES, directory_mode='0700', payload_mode='0700')
    except (Failure, complete.PreparationFailure, complete.pinned.DownloadFailure) as error:
        # Only our fixed categories are persisted. Traces and snapshots stay private.
        report['failure'] = str(error)
    except Exception:
        report['failure'] = 'unexpected_acceptance_failure'
    finally:
        (evidence / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    return report['passed']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--release', choices=tuple(complete.pinned.releases.RELEASES), default=complete.pinned.releases.DEFAULT)
    args = parser.parse_args()
    complete.select_release(args.release)
    try: return 0 if run(args.cli, args.archive, args.evidence) else 1
    except Exception:
        print('Qoder private deployment acceptance could not start')
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
