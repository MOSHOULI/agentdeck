#!/usr/bin/env python3
"""Retry only the pinned Tauri DLL's observed transient download failure.
No custom downloader/cache/installer and no change to upstream hash checks.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
URL = 'https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll'
SHA1 = '75197FEE3C6A814FE035788D1C34EAD39349B860'
SOURCE_SHA256 = 'ce129133be31063c51ee3649f69f600b9397f6af1e24075dec3b0d54ae637a2c'
INTEGRITY = 'sha512-JI9bHfudSRJikGOrjbXEm6RjzzvmKuLc+JixB1NE8hNzQkNlCCIC3pTES3uypkR9lxHrsbxzcnSwhI2IxBQphw=='
NATIVE_PACKAGE = '@tauri-apps/cli-win32-x64-msvc'
NATIVE_URL = 'https://registry.npmjs.org/@tauri-apps/cli-win32-x64-msvc/-/cli-win32-x64-msvc-2.12.4.tgz'
NATIVE_INTEGRITY = 'sha512-DJhDeCqjLDNMWYFy7ZS8cfeAHX2RK0RbQMLiA3bMVVkXRDJ4NBYiHriGqsUhsr9iG41aTb+dJGkFPmPw1gWwVw=='
ATTEMPTS = 3
TAIL_LIMIT = 65536

def check_contract(root=ROOT):
    lock = json.loads((root / 'web/ui/package-lock.json').read_text())['packages']
    cli = lock['node_modules/@tauri-apps/cli']
    if (cli['version'], cli['resolved'], cli['integrity']) != ('2.12.4', 'https://registry.npmjs.org/@tauri-apps/cli/-/cli-2.12.4.tgz', INTEGRITY):
        raise ValueError('unreviewed CLI lock')
    for name, version in cli['optionalDependencies'].items():
        if version != '2.12.4' or lock['node_modules/' + name]['version'] != version:
            raise ValueError('unreviewed native CLI lock')
    native = lock['node_modules/' + NATIVE_PACKAGE]
    if (native['version'], native['resolved'], native['integrity']) != ('2.12.4', NATIVE_URL, NATIVE_INTEGRITY):
        raise ValueError('unreviewed Windows x64 native CLI lock')
    source = (root / 'docs/upstream/tauri/nsis-2.12.4.rs').read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError('source evidence changed')
    if f'const NSIS_TAURI_UTILS_URL: &str = "{URL}";'.encode() not in source or f'const NSIS_TAURI_UTILS_SHA1: &str = "{SHA1}";'.encode() not in source:
        raise ValueError('source contract disagreement')

def retryable(tail):
    # Exact final three process lines, not a historical URL/error somewhere in a
    # mixed log. Unknown/new formatting fails closed and needs explicit review.
    lines = [line.strip() for line in tail.decode('utf-8', errors='replace').splitlines() if line.strip()]
    if len(lines) < 3 or lines[-3] != 'Downloading ' + URL:
        return False
    match = re.fullmatch(r'failed to bundle project: `http status: (408|429|502|503|504)`', lines[-2])
    return bool(match and lines[-1] == 'Error ' + lines[-2])

def build_once():
    command = ['node', str(ROOT / 'web/ui/node_modules/@tauri-apps/cli/tauri.js'), 'build', '--bundles', 'nsis', '--ci', '--', '--locked']
    process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    tail = b''
    assert process.stdout is not None
    with process.stdout:
        while chunk := process.stdout.read1(16384):
            sys.stdout.buffer.write(chunk)
            sys.stdout.buffer.flush()
            tail = (tail + chunk)[-TAIL_LIMIT:]
    return process.wait(), tail

def run(build=build_once, sleep=time.sleep):
    for attempt in range(1, ATTEMPTS + 1):
        code, tail = build()
        if code == 0:
            return 0
        if not retryable(tail) or attempt == ATTEMPTS:
            return 1
        print(f'Pinned NSIS DLL transient HTTP failure; retry {attempt + 1}/{ATTEMPTS} after {attempt * 10}s', flush=True)
        sleep(attempt * 10)
    return 1

def main():
    if (os.environ.get('GITHUB_ACTIONS'), os.environ.get('RUNNER_ENVIRONMENT'), os.environ.get('RUNNER_OS')) != ('true', 'github-hosted', 'Windows'):
        raise ValueError('requires hosted Windows CI')
    if any(name in os.environ for name in ('TAURI_BUNDLER_TOOLS_GITHUB_MIRROR', 'TAURI_BUNDLER_TOOLS_GITHUB_MIRROR_TEMPLATE')):
        raise ValueError('mirror override forbidden')
    check_contract()
    installed = json.loads((ROOT / 'web/ui/node_modules/@tauri-apps/cli/package.json').read_text())
    if installed['version'] != '2.12.4':
        raise ValueError('installed CLI differs from reviewed lock')
    native_installed = json.loads((ROOT / 'web/ui/node_modules' / NATIVE_PACKAGE / 'package.json').read_text())
    if native_installed['name'] != NATIVE_PACKAGE or native_installed['version'] != '2.12.4':
        raise ValueError('installed native CLI differs from reviewed lock')
    return run()

if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError):
        print('NSIS build contract/launch failed; no retry', file=sys.stderr)
        sys.exit(1)
