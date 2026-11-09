#!/usr/bin/env python3
"""Hosted Linux only: fixed official DSH root archive for real UI verification.
No extraction, CLI, npm, Node or package-content execution.
"""
import argparse
import hashlib
import json
import multiprocessing
import os
from pathlib import Path
import sys
import dsh_distribution_smoke as pinned


def guard(directory):
    pinned.guard._guard_destination(directory)
    if sys.platform != 'linux' or os.environ.get('RUNNER_OS') != 'Linux':
        raise pinned.Failure('requires_hosted_linux')


def worker(directory, sender):
    os.setsid()
    try:
        directory = Path(directory); guard(directory)
        raw, data, _ = pinned.fetch_package(directory)
        sender.send((True, {'state': 'pinned_dsh_root_downloaded_not_executed', 'bytes': len(data),
                            'sha256': pinned.SHA256, 'metadata_sha256': hashlib.sha256(raw).hexdigest()}))
    except Exception as error:
        sender.send((False, str(error) if isinstance(error, pinned.Failure) else 'download_failed'))
    finally: sender.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', type=Path, required=True)
    args = parser.parse_args()
    try:
        guard(args.destination)
        context = multiprocessing.get_context('spawn'); receiver, sender = context.Pipe(duplex=False)
        process = context.Process(target=worker, args=(str(args.destination), sender)); process.start(); sender.close()
        try: result = pinned.supervise(process, receiver)
        finally:
            receiver.close()
            if not process.is_alive(): process.close()
        print(json.dumps(result, sort_keys=True))
    except Exception as error:
        print('DSH UI download failed: ' + (str(error) if isinstance(error, pinned.Failure) else 'setup_failed'), file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__': sys.exit(main())
