#!/usr/bin/env python3
"""Pinned official Qwen distribution bootstrap smoke, never login or prompt.

Downloads only the pinned registry artifact (or uses --archive), checks SHA-512,
extracts without lifecycle scripts/optional dependencies, then runs version/help.
Default requires Linux bubblewrap network isolation. --network-isolation none
is an explicit weaker diagnostic mode; Node permissions are not a hostile-code sandbox.
"""
import argparse
import base64
import hashlib
import json
import pathlib
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

VERSION = '0.27.0'
URL = 'https://registry.npmjs.org/@qwen-code/qwen-code/-/qwen-code-0.27.0.tgz'
INTEGRITY = 'efZt0LUm/raGUjR/SNN6pp8cpuNHnmE0hQvcvXzWHtVX9oVtDDgukq2IZNwYDErFYqHRj1xq2NQ5Jjt2SHUr/Q=='

# Reviewed artifact: 1,400 entries / 156,300,038 bytes / largest file 12,142,871.
MAX_ARCHIVE_ENTRIES = 2000
MAX_UNPACKED_BYTES = 256 * 1024 * 1024
MAX_FILE_BYTES = 32 * 1024 * 1024


def verify_archive(path):
    with open(path, 'rb') as stream:
        digest = hashlib.file_digest(stream, 'sha512').digest()
    if base64.b64encode(digest).decode() != INTEGRITY:
        raise ValueError('Pinned Qwen archive integrity mismatch')


def bounded_members(archive):
    # Do not call getmembers(): stop iteration as soon as a limit is exceeded.
    # Validate the whole bounded inventory before creating extracted files.
    members = []
    total = 0
    for member in archive:
        if len(members) >= MAX_ARCHIVE_ENTRIES:
            raise ValueError('Archive entry count exceeds limit')
        parts = pathlib.PurePosixPath(member.name).parts
        if not parts or parts[0] != 'package' or '..' in parts or not (member.isfile() or member.isdir()):
            raise ValueError('Unexpected archive member')
        if member.size < 0 or member.size > MAX_FILE_BYTES:
            raise ValueError('Archive member size exceeds limit')
        total += member.size
        if total > MAX_UNPACKED_BYTES:
            raise ValueError('Archive expanded size exceeds limit')
        members.append(member)
    return members


def unpack(path, destination):
    verify_archive(path)
    with tarfile.open(path) as archive:
        members = bounded_members(archive)
        archive.extractall(destination, members=members, filter='data')
    package = destination / 'package'
    manifest = json.loads((package / 'package.json').read_text())
    if (manifest['name'], manifest['version'], manifest['bin']) != ('@qwen-code/qwen-code', VERSION, {'qwen': 'cli-entry.js'}):
        raise ValueError('Unexpected package identity/entry point')
    if manifest.get('dependencies') or manifest.get('scripts'):
        raise ValueError('Unexpected required dependencies/lifecycle scripts')
    return package


def environment(home):
    # Deliberately do not copy PATH, NODE_OPTIONS, proxy/auth or provider variables.
    return {'HOME': str(home), 'USERPROFILE': str(home), 'QWEN_HOME': str(home / '.qwen'),
            'XDG_CONFIG_HOME': str(home / '.config'), 'XDG_DATA_HOME': str(home / '.local/share'),
            'TMPDIR': str(home), 'TMP': str(home), 'TEMP': str(home),
            'NO_COLOR': '1', 'TERM': 'dumb', 'LANG': 'en_US.UTF-8'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=pathlib.Path)
    parser.add_argument('--network-isolation', choices=['bwrap', 'none'], default='bwrap')
    args = parser.parse_args()
    node = shutil.which('node')
    if not node:
        raise RuntimeError('Node 24+ required')
    node = str(pathlib.Path(node).resolve())
    with tempfile.TemporaryDirectory(prefix='agentdeck-qwen-distribution-') as temporary:
        root = pathlib.Path(temporary)
        home = root / 'home'
        home.mkdir()
        env = environment(home)
        version = subprocess.run([node, '--version'], env=env, cwd=home, check=True, capture_output=True, text=True).stdout.strip()
        if int(version.lstrip('v').split('.')[0]) < 24:
            raise RuntimeError('Node 24+ required for tested permission behavior')
        archive = args.archive
        if archive is None:
            archive = root / 'qwen.tgz'
            with urllib.request.urlopen(URL, timeout=60) as response, archive.open('wb') as output:
                # Bound both memory use and total download size.
                remaining = 64 * 1024 * 1024
                while chunk := response.read(min(1024 * 1024, remaining + 1)):
                    remaining -= len(chunk)
                    if remaining < 0:
                        raise ValueError('Archive exceeds size limit')
                    output.write(chunk)
        install = root / 'install'
        install.mkdir()
        package = unpack(archive, install)
        prefix = []
        if args.network_isolation == 'bwrap':
            bwrap = shutil.which('bwrap')
            if not bwrap:
                raise RuntimeError('bubblewrap required; no silent isolation downgrade')
            # Network namespace and read-only root; only the synthetic home is writable.
            prefix = [bwrap, '--unshare-net', '--die-with-parent', '--ro-bind', '/', '/',
                      '--bind', str(home), str(home)]
        checks = []
        for flag in ('--version', '--help'):
            command = prefix + [node, '--permission', '--allow-fs-read=' + str(install),
                                '--allow-fs-read=' + str(home), '--allow-fs-write=' + str(home),
                                str(package / 'cli-entry.js'), flag]
            result = subprocess.run(command, cwd=home, env=env, capture_output=True, text=True, timeout=30)
            if result.returncode or result.stderr:
                raise RuntimeError(f'{flag} failed: {result.stderr[:2000]}')
            if flag == '--version' and result.stdout.strip() != VERSION:
                raise AssertionError('Unexpected version output')
            if flag == '--help' and not all(text in result.stdout for text in ('Usage: qwen', '--help', '--version', '--prompt')):
                raise AssertionError('Unexpected help output')
            checks.append(flag)
        print(json.dumps({'package': '@qwen-code/qwen-code', 'version': VERSION,
                          'integrity': 'sha512-' + INTEGRITY, 'node': version,
                          'network_isolation': args.network_isolation, 'checks': checks,
                          'optional_dependencies_installed': False, 'authenticated': False,
                          'model_called': False, 'settings_loader_tested': False}, indent=2))


if __name__ == '__main__':
    main()
