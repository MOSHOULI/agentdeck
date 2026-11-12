#!/usr/bin/env python3
"""Real fixed-release mixed cache rejection; never execute a Qoder payload."""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import qoder_complete_fixtures as complete
import qoder_private_deploy_acceptance as deployment


# Exact observations for fixed whole foreign caches. The core checks selected
# archive/payload size ceilings before reading the intent version. A smaller
# foreign cache reaches the mismatched intent (unknown_ownership); a larger
# member fails the ceiling first (conflict). Neither state authorizes mutation.
FOREIGN_STATES = {
    ('1.1.85', '1.1.86'): 'unknown_ownership',
    ('1.1.85', '1.1.87'): 'unknown_ownership',
    ('1.1.86', '1.1.85'): 'conflict',
    ('1.1.86', '1.1.87'): 'conflict',
    ('1.1.87', '1.1.85'): 'conflict',
    ('1.1.87', '1.1.86'): 'unknown_ownership',
}


def trace_command(cli, action, cache, trace, destination=None):
    command = ['/usr/bin/strace', '-f', '-qq', '-s', '4096', '-o', str(trace),
               '-e', 'trace=execve,execveat', str(cli), 'qoder', action,
               '--cache', str(cache)] + complete.release_args()
    if destination is not None: command += ['--destination', str(destination)]
    return command


def reject_fresh_preview(cli, cache, destination, env, trace):
    command = trace_command(cli, 'deploy-preview', cache, trace, destination)
    process = subprocess.Popen(command, env=env, stdout=subprocess.DEVNULL,
                               stderr=subprocess.DEVNULL, start_new_session=True)
    try:
        try: code = process.wait(timeout=120)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL); process.wait(timeout=5)
            raise deployment.Failure('fresh_preview_timeout') from None
        if code != 1: raise deployment.Failure('fresh_preview_not_rejected')
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL); process.wait(timeout=5)
    deployment.check_trace(trace, cli)


def inspect_state(cli, cache, env, trace, expected):
    report = complete.run_cli(trace_command(cli, 'inspect-cache', cache, trace), env)
    deployment.check_trace(trace, cli)
    if (not isinstance(report, dict) or report.get('version') != complete.pinned.VERSION
            or report.get('state') != expected or report.get('installed') is not False
            or report.get('deletion_authorized') is not False):
        raise deployment.Failure('unexpected_cross_release_inspection')


def run(cli, archives, evidence):
    if len(archives) < 2 or not set(archives).issubset(complete.pinned.releases.RELEASES):
        raise deployment.Failure('unsupported_cross_release_selection')
    pairs = [(release, other) for release in archives for other in archives if release != other]
    for archive in archives.values(): deployment.guard(cli, archive, evidence)
    evidence.mkdir(mode=0o700, exist_ok=False)
    root = Path(tempfile.mkdtemp(prefix='qoder-cross-release-', dir=os.environ['RUNNER_TEMP']))
    home = root / 'home'; home.mkdir(mode=0o700)
    env = {'HOME': str(home), 'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8'}
    report = {'passed': False, 'releases': list(archives),
              'checks': [], 'whole_cache_checks': [], 'same_release_checks': [], 'harness_invokes_qoder': False,
              'boundary': 'Real fixed archive/payload caches; each rejected deploy traced. Preparation does not invoke Qoder but is not traced here. No runtime or authentication acceptance.'}
    try:
        prepared = {}
        for release, archive in archives.items():
            complete.select_release(release)
            parent = root / release; parent.mkdir(mode=0o700)
            prepared[release], _ = complete.prepare(cli, archive, parent, env)
        originals = {release: deployment.snapshot(paths['unpacked']) for release, paths in prepared.items()}
        home_before = deployment.snapshot(home)
        for release, other in pairs:
            complete.select_release(release)
            own = prepared[release]['unpacked']
            matching_target = root / f'matching-{release}-{other}'
            deployment.run_preview(cli, own, matching_target, env, root / f'matching-{release}-{other}.trace')
            inspect_state(cli, own, env, root / f'matching-inspect-{release}-{other}.trace', 'unpacked_payload_verified')
            if os.path.lexists(matching_target): raise deployment.Failure('matching_preview_wrote_target')
            report['same_release_checks'].append({'release': release, 'foreign_release': other, 'preview_success': True, 'verified_inspection': True})
            foreign = prepared[other]['unpacked']
            foreign_target = root / f'foreign-{release}-{other}'
            reject_fresh_preview(cli, foreign, foreign_target, env, root / f'foreign-preview-{release}-{other}.trace')
            foreign_state = FOREIGN_STATES[(release, other)]
            inspect_state(cli, foreign, env, root / f'foreign-inspect-{release}-{other}.trace', foreign_state)
            if os.path.lexists(foreign_target): raise deployment.Failure('foreign_preview_wrote_target')
            if any(deployment.snapshot(paths['unpacked']) != originals[v] for v, paths in prepared.items()) or deployment.snapshot(home) != home_before:
                raise deployment.Failure('foreign_cache_check_changed_state')
            report['whole_cache_checks'].append({'selected_release': release, 'cache_release': other, 'fresh_preview_rejected': True, 'inspection': foreign_state, 'source_unchanged': True, 'destination_absent': True})
            for member, category in [('complete.json', 'receipt'), ('archive.tar.gz', 'archive'), ('qodercli', 'payload')]:
                cache = root / f'mixed-{release}-{other}-{category}'
                shutil.copytree(prepared[release]['unpacked'], cache)
                destination = root / f'destination-{release}-{other}-{category}'
                token = deployment.run_preview(cli, cache, destination, env, root / f'preview-{release}-{other}-{category}.trace')
                # Harness-authored mutation after a valid preview. Snapshot AFTER
                # mixing distinguishes our intentional setup from product writes.
                shutil.copyfile(prepared[other]['unpacked'] / member, cache / member)
                before = deployment.snapshot(cache)
                reject_fresh_preview(cli, cache, destination, env, root / f'fresh-{release}-{other}-{category}.trace')
                inspect_state(cli, cache, env, root / f'inspect-{release}-{other}-{category}.trace', 'conflict')
                deployment.run_deploy(cli, cache, destination, token, env, root / f'reject-{release}-{other}-{category}.trace', reject=True)
                if os.path.lexists(destination) or deployment.snapshot(cache) != before:
                    raise deployment.Failure('mixed_release_rejection_changed_state')
                if deployment.snapshot(home) != home_before:
                    raise deployment.Failure('mixed_release_rejection_changed_home')
                for version, paths in prepared.items():
                    if deployment.snapshot(paths['unpacked']) != originals[version]:
                        raise deployment.Failure('mixed_release_original_changed')
                report['checks'].append({'release': release, 'foreign_release': other, 'member': category,
                                        'rejected': True, 'fresh_preview_rejected': True, 'inspection': 'conflict', 'destination_absent': True, 'source_unchanged': True,
                                        'preview_exec_count': 1, 'rejection_exec_count': 1})
        if len(report['checks']) != len(pairs) * 3 or len(report['whole_cache_checks']) != len(pairs) or len(report['same_release_checks']) != len(pairs): raise deployment.Failure('missing_cross_release_coverage')
        report['passed'] = True
    except Exception as error:
        report['failure'] = str(error) if isinstance(error, (deployment.Failure, complete.PreparationFailure)) else 'unexpected_cross_release_failure'
    finally:
        (evidence / 'result.json').write_text(json.dumps(report, indent=2) + '\n')
    return report['passed']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--legacy-archive', type=Path, required=True)
    parser.add_argument('--current-archive', type=Path, required=True)
    parser.add_argument('--additional-archive', type=Path, help='Fixed reviewed 1.1.87 archive; adds all bidirectional pairs')
    parser.add_argument('--evidence', type=Path, required=True)
    args = parser.parse_args()
    archives = {'1.1.85': args.legacy_archive, '1.1.86': args.current_archive}
    if args.additional_archive is not None: archives['1.1.87'] = args.additional_archive
    try: return 0 if run(args.cli, archives, args.evidence) else 1
    except Exception:
        print('Qoder cross-release acceptance could not start')
        return 1

if __name__ == '__main__': raise SystemExit(main())
