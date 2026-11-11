#!/usr/bin/env python3
"""Hosted-only fixed reviewed native archives. No extraction or package execution."""
import argparse
import json
import multiprocessing
import os
from pathlib import Path
import sys
import time
import dsh_native_smoke as pinned
import fetch_dsh_ui_archive as root_download


def prepare(directory):
    directory.mkdir(mode=0o700, exist_ok=False)
    for item in pinned.load_review():
        raw = pinned.fetch(item['metadata_url'], 262144)
        pinned.validate_metadata(raw, item['id'])
        data = pinned.fetch(item['archive_url'], item['archive_bytes'])
        pinned.validate_archive(data, item['id'])
        pinned.write_new(directory / item['archive_name'], data)
    return {'state': 'reviewed_native_archives_downloaded_not_executed', 'packages': list(pinned.IDS)}


def worker(directory, sender):
    os.setsid()
    try:
        directory = Path(directory)
        root_download.guard(directory)
        sender.send((True, prepare(directory)))
    except Exception as error:
        sender.send((False, str(error) if isinstance(error, pinned.Failure) else 'download_failed'))
    finally:
        sender.close()
        time.sleep(300)  # Supervisor owns session cleanup before reaping.


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', type=Path, required=True)
    args = parser.parse_args()
    try:
        root_download.guard(args.destination)
        context = multiprocessing.get_context('spawn')
        receiver, sender = context.Pipe(duplex=False)
        process = context.Process(target=worker, args=(str(args.destination), sender))
        process.start(); sender.close()
        try:
            result = pinned.supervise(process, receiver)
        finally:
            receiver.close()
            if not process.is_alive(): process.close()
        print(json.dumps(result, sort_keys=True))
    except Exception:
        print('DSH native UI source preparation failed.', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__': sys.exit(main())
