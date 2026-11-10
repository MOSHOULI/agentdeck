#!/usr/bin/env python3
"""Real pinned npm filesystem installation, never execute Codex or lifecycle code.

This is not AgentDeck adapter/runtime, login, model, or network-isolation acceptance.
Only Linux x86_64 is supported. No URL, version, hash or script-policy overrides.
"""
import argparse
import base64
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import selectors
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request

VERSION = '0.167.0'
REGISTRY = 'https://registry.npmjs.org/'
ROOT_NAME = '@openai/codex'
NATIVE_NAME = '@openai/codex-linux-x64'
ALIAS_TARGETS = ('linux-x64', 'win32-x64', 'darwin-x64', 'linux-arm64', 'win32-arm64', 'darwin-arm64')
OPTIONAL = {'@openai/codex-' + target: 'npm:@openai/codex@' + VERSION + '-' + target for target in ALIAS_TARGETS}
CHUNK = 65536
MAX_COMPRESSED = 256 * 1024 * 1024
MAX_EXPANDED = 512 * 1024 * 1024
MAX_MEMBER = 384 * 1024 * 1024
MAX_MEMBERS = 128
DOWNLOAD_SECONDS = 180
PROCESS_SECONDS = 300
OUTPUT_LIMIT = 65536


class Failure(Exception):
    """Fixed categories only. Never include package output, credentials or paths."""


@dataclass(frozen=True)
class Pin:
    version: str
    integrity: str
    files: int
    expanded: int
    native: bool = False

    @property
    def url(self):
        return REGISTRY + '@openai/codex/-/codex-' + self.version + '.tgz'


ROOT = Pin(VERSION, '1FSQgbTIKAdTub5Ln6z36NT4eXDmBRkinfigzycbgn9yLZdJ3UdkQZqr0N7jnl8/D1xxT/Pz3Fl2l7S2dmRaFA==', 3, 13570)
NATIVE = Pin(VERSION + '-linux-x64', 'FNc7danfUng9Sp6w+Fh7kofNHHZpA6VMyNKWsWrS3OwHly69gywTWeiuwrUPXpJSho8Df4xqRsHfHRPzj7zYQg==', 45, 458593658, True)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise Failure('download_redirect_rejected')


def json_object(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result: raise Failure('duplicate_json_key')
            result[key] = value
        return result
    try:
        result = json.loads(raw, object_pairs_hook=pairs)
        if not isinstance(result, dict): raise Failure('manifest_not_object')
        return result
    except (ValueError, UnicodeError, RecursionError):
        raise Failure('manifest_invalid') from None


def regular(path, limit):
    m = path.lstat()
    if not stat.S_ISREG(m.st_mode) or m.st_nlink != 1 or m.st_size > limit or m.st_uid != os.geteuid():
        raise Failure('unsafe_regular_file')
    return m


def file_digest(path, limit, algorithm='sha256'):
    before = regular(path, limit)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        if (opened.st_dev, opened.st_ino) != (before.st_dev, before.st_ino): raise Failure('file_changed')
        digest = hashlib.new(algorithm)
        count = 0
        while block := stream.read(CHUNK):
            count += len(block)
            if count > limit: raise Failure('file_size_exceeded')
            digest.update(block)
        after = os.fstat(stream.fileno())
    final = path.lstat()
    keys = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_nlink', 'st_mtime_ns', 'st_ctime_ns')
    if count != before.st_size or any(getattr(before, k) != getattr(after, k) or getattr(before, k) != getattr(final, k) for k in keys):
        raise Failure('file_changed')
    return digest.digest()


def download(pin, destination, opener=None, clock=time.monotonic):
    # Production calls only the two constants; no CLI/user-supplied URL.
    if pin not in (ROOT, NATIVE): raise Failure('download_pin_rejected')
    client = opener or urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    deadline = clock() + DOWNLOAD_SECONDS
    request = urllib.request.Request(pin.url, headers={'Accept-Encoding': 'identity', 'User-Agent': 'AgentDeck-pinned-install-verifier'})
    fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as out, client.open(request, timeout=15) as response:
        if response.status != 200 or response.geturl() != pin.url: raise Failure('download_response_rejected')
        if response.headers.get('Content-Encoding', 'identity').lower() != 'identity' or response.headers.get('Transfer-Encoding'):
            raise Failure('download_encoding_rejected')
        lengths = response.headers.get_all('Content-Length', [])
        if len(lengths) != 1 or not lengths[0].isascii() or not lengths[0].isdigit() or len(lengths[0]) > 10:
            raise Failure('download_length_rejected')
        expected = int(lengths[0])
        if not 0 < expected <= MAX_COMPRESSED: raise Failure('download_length_rejected')
        count = 0
        while True:
            if clock() >= deadline: raise Failure('download_deadline')
            block = response.read1(min(CHUNK, MAX_COMPRESSED - count + 1))
            if clock() >= deadline: raise Failure('download_deadline')
            if not block: break
            count += len(block)
            if len(block) > CHUNK or count > MAX_COMPRESSED: raise Failure('download_size_exceeded')
            out.write(block)
        if count != expected: raise Failure('download_length_mismatch')
    if base64.b64encode(file_digest(destination, MAX_COMPRESSED, 'sha512')).decode() != pin.integrity:
        raise Failure('archive_integrity_mismatch')


def validate_manifest(manifest, pin):
    if manifest.get('name') != ROOT_NAME or manifest.get('version') != pin.version:
        raise Failure('package_identity_mismatch')
    if any(manifest.get(key) not in (None, {}, []) for key in ('scripts', 'dependencies', 'peerDependencies', 'bundleDependencies', 'bundledDependencies')):
        raise Failure('package_dependency_or_script_changed')
    if pin.native:
        if manifest.get('os') != ['linux'] or manifest.get('cpu') != ['x64'] or manifest.get('bin') not in (None, {}):
            raise Failure('native_platform_mismatch')
        if manifest.get('optionalDependencies') not in (None, {}): raise Failure('native_dependencies_changed')
    else:
        if manifest.get('bin') != {'codex': 'bin/codex.js'} or manifest.get('optionalDependencies') != OPTIONAL:
            raise Failure('root_entry_or_dependencies_changed')


def inventory(archive_path, pin):
    if base64.b64encode(file_digest(archive_path, MAX_COMPRESSED, 'sha512')).decode() != pin.integrity:
        raise Failure('archive_integrity_mismatch')
    records = {}; seen = set(); total = 0; manifest = None
    # Streaming traversal never extracts or executes the archive.
    with tarfile.open(archive_path, mode='r|gz') as archive:
        for member in archive:
            parts = PurePosixPath(member.name).parts
            if (len(seen) >= MAX_MEMBERS or not parts or parts[0] != 'package'
                or member.name.startswith('/') or '..' in parts or '\\' in member.name
                or any(ord(c) < 32 or ord(c) == 127 for c in member.name)
                or not (member.isfile() or member.isdir()) or member.mode & 0o7022):
                raise Failure('archive_member_rejected')
            relative = '/'.join(parts[1:])
            canonical = 'package' + ('/' + relative if relative else '')
            if member.name.rstrip('/') != canonical or canonical in seen:
                raise Failure('archive_duplicate_or_noncanonical_path')
            seen.add(canonical)
            if not relative and not member.isdir(): raise Failure('archive_member_rejected')
            if member.isdir():
                if member.size: raise Failure('archive_member_rejected')
                continue
            if not 0 <= member.size <= MAX_MEMBER: raise Failure('archive_member_size_exceeded')
            total += member.size
            if total > MAX_EXPANDED: raise Failure('archive_expanded_size_exceeded')
            stream = archive.extractfile(member)
            if stream is None: raise Failure('archive_member_unreadable')
            digest = hashlib.sha256(); count = 0; manifest_data = bytearray()
            while block := stream.read(CHUNK):
                count += len(block)
                if count > member.size: raise Failure('archive_member_size_mismatch')
                digest.update(block)
                if relative == 'package.json':
                    if count > 65536: raise Failure('manifest_size_exceeded')
                    manifest_data.extend(block)
            if count != member.size: raise Failure('archive_member_size_mismatch')
            records[relative] = {'size': count, 'sha256': digest.hexdigest(), 'executable': bool(member.mode & 0o111)}
            if relative == 'package.json': manifest = json_object(manifest_data)
    if len(records) != pin.files or total != pin.expanded: raise Failure('archive_inventory_changed')
    if manifest is None: raise Failure('manifest_missing')
    validate_manifest(manifest, pin)
    for name in records:
        if any(str(parent) in records for parent in PurePosixPath(name).parents if str(parent) != '.'):
            raise Failure('archive_file_directory_collision')
    return records


def prepare(root):
    root.mkdir(mode=0o700, exist_ok=False)
    paths = {name: root / name for name in ('home', 'prefix', 'cache', 'tmp', 'config', 'data', 'state')}
    for path in paths.values(): path.mkdir(mode=0o700)
    for name in ('userconfig', 'globalconfig'):
        path = root / name
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        os.close(fd); paths[name] = path
    return paths


def clean_environment(paths, node):
    # Intentionally do not copy os.environ. No auth, proxies, npm config,
    # NODE_OPTIONS, model provider variables, or personal runtime locations.
    return {'HOME': str(paths['home']), 'USERPROFILE': str(paths['home']),
            'XDG_CONFIG_HOME': str(paths['config']), 'XDG_DATA_HOME': str(paths['data']),
            'XDG_CACHE_HOME': str(paths['cache']), 'XDG_STATE_HOME': str(paths['state']),
            'TMPDIR': str(paths['tmp']), 'TMP': str(paths['tmp']), 'TEMP': str(paths['tmp']),
            'PATH': str(node.parent) + ':/usr/bin:/bin', 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8',
            'NO_COLOR': '1', 'CI': 'true'}


def npm_command(node, npm, paths, root_archive):
    return [str(node), str(npm), 'install', '--global', '--prefix', str(paths['prefix']),
            '--cache', str(paths['cache']), '--userconfig', str(paths['userconfig']),
            '--globalconfig', str(paths['globalconfig']), '--registry=' + REGISTRY, '--@openai:registry=' + REGISTRY,
            '--ignore-scripts', '--include=optional', '--no-audit', '--no-fund', '--no-progress',
            '--package-lock=false', '--update-notifier=false', '--fetch-retries=0',
            '--fetch-timeout=30000', '--logs-max=0', '--force=false', str(root_archive)]


def bounded_process(argv, cwd, env, seconds=PROCESS_SECONDS):
    process = subprocess.Popen(argv, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True, umask=0o077)
    buffers = [bytearray(), bytearray()]; deadline = time.monotonic() + seconds
    try:
        with selectors.DefaultSelector() as selector:
            for index, stream in enumerate((process.stdout, process.stderr)):
                os.set_blocking(stream.fileno(), False); selector.register(stream, selectors.EVENT_READ, index)
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0: raise Failure('npm_process_timeout')
                for key, _ in selector.select(min(remaining, 0.1)):
                    block = os.read(key.fileobj.fileno(), 8192)
                    if not block: selector.unregister(key.fileobj); continue
                    target = buffers[key.data]
                    if len(target) + len(block) > OUTPUT_LIMIT: raise Failure('npm_output_limit')
                    target.extend(block)
            remaining = deadline - time.monotonic()
            if remaining <= 0: raise Failure('npm_process_timeout')
            try: result = process.wait(timeout=remaining)
            except subprocess.TimeoutExpired: raise Failure('npm_process_timeout') from None
        if result != 0: raise Failure('npm_process_failed')
        return bytes(buffers[0]), bytes(buffers[1])
    finally:
        # Kill the invocation's process group even if its leader exited early.
        # No retry after uncertain execution.
        try: os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError: pass
        process.wait(timeout=5)
        process.stdout.close(); process.stderr.close()


def expected_layout(root_records, native_records):
    package = PurePosixPath('lib/node_modules/@openai/codex')
    native = package / 'node_modules/@openai/codex-linux-x64'
    files = {str(package / name): record for name, record in root_records.items()}
    files.update({str(native / name): record for name, record in native_records.items()})
    directories = {'bin'}
    for name in files:
        directories.update(str(parent) for parent in PurePosixPath(name).parents if str(parent) != '.')
    return files, directories


def verify_install(paths, root_records, native_records):
    prefix = paths['prefix']; files, directories = expected_layout(root_records, native_records)
    if not stat.S_ISDIR(prefix.lstat().st_mode): raise Failure('installed_prefix_replaced')
    observed_files = set(); observed_dirs = set(); launchers = set()
    # Bound traversal and never follow directory links.
    pending = [prefix]; count = 0
    while pending:
        directory = pending.pop()
        for path in directory.iterdir():
            count += 1
            if count > 256: raise Failure('installed_tree_limit')
            relative = path.relative_to(prefix).as_posix(); m = path.lstat()
            if m.st_uid != os.geteuid(): raise Failure('installed_permissions_rejected')
            if not stat.S_ISLNK(m.st_mode) and m.st_mode & 0o7022: raise Failure('installed_permissions_rejected')
            if stat.S_ISLNK(m.st_mode):
                if relative != 'bin/codex' or os.readlink(path) != '../lib/node_modules/@openai/codex/bin/codex.js':
                    raise Failure('installed_link_rejected')
                launchers.add(relative)
            elif stat.S_ISDIR(m.st_mode):
                if relative not in directories: raise Failure('unexpected_installed_directory')
                observed_dirs.add(relative); pending.append(path)
            elif stat.S_ISREG(m.st_mode):
                expected = files.get(relative)
                if expected is None: raise Failure('unexpected_installed_file')
                if m.st_size != expected['size'] or bool(m.st_mode & 0o111) != expected['executable']:
                    raise Failure('installed_file_metadata_mismatch')
                if file_digest(path, MAX_MEMBER).hex() != expected['sha256']:
                    raise Failure('installed_file_digest_mismatch')
                observed_files.add(relative)
            else: raise Failure('installed_type_rejected')
    if observed_files != set(files) or observed_dirs != directories or launchers != {'bin/codex'}:
        raise Failure('installed_layout_incomplete')
    # npm caches/temp are expected, but no agent profile, vault, HOME state or
    # user/global npm config may be created by the install.
    for name in ('home', 'config', 'data', 'state'):
        if not stat.S_ISDIR(paths[name].lstat().st_mode) or any(paths[name].iterdir()): raise Failure('configuration_side_effect')
    for name in ('userconfig', 'globalconfig'):
        if regular(paths[name], 0).st_size != 0: raise Failure('npm_config_changed')
    return {'root_files_verified': len(root_records), 'native_files_verified': len(native_records),
            'installed_bytes_verified': sum(x['size'] for x in files.values()),
            'launcher_symlink_verified': True, 'no_agent_configuration_created': True}


def toolchain(node_path, npm_path):
    for path in (node_path, npm_path):
        if not path.is_absolute(): raise Failure('tool_path_not_absolute')
    node = node_path.resolve(strict=True); npm = npm_path.resolve(strict=True)
    if not node.is_file() or not os.access(node, os.X_OK) or not npm.is_file() or npm.name != 'npm-cli.js':
        raise Failure('toolchain_invalid')
    with node.open('rb') as stream:
        if stream.read(4) != b'\x7fELF': raise Failure('node_not_native_elf')
    metadata = json_object((npm.parent.parent / 'package.json').read_bytes())
    if metadata.get('name') != 'npm': raise Failure('npm_identity_invalid')
    # Supplied local Node/npm are trusted developer dependencies, not a publisher
    # signature or an attestation about arbitrary user-selected executables.
    return node, npm


def run(node_path, npm_path):
    if sys.platform != 'linux' or platform.machine() != 'x86_64': raise Failure('requires_linux_x86_64')
    node, npm = toolchain(node_path, npm_path)
    with tempfile.TemporaryDirectory(prefix='agentdeck-codex-real-install-') as temporary:
        root = Path(temporary); paths = prepare(root / 'isolated')
        root_archive = root / 'root.tgz'; native_archive = root / 'native.tgz'
        download(ROOT, root_archive); download(NATIVE, native_archive)
        root_records = inventory(root_archive, ROOT); native_records = inventory(native_archive, NATIVE)
        env = clean_environment(paths, node)
        bounded_process(npm_command(node, npm, paths, root_archive), paths['home'], env)
        return verify_install(paths, root_records, native_records)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', required=True, type=Path, help='Trusted official local Node executable')
    parser.add_argument('--npm', required=True, type=Path, help='Trusted official npm CLI (symlink permitted)')
    parser.add_argument('--evidence', required=True, type=Path, help='New JSON result, never overwritten')
    args = parser.parse_args()
    report = {'passed': False, 'package': ROOT_NAME, 'version': VERSION, 'platform': 'linux-x64',
              'root_integrity': 'sha512-' + ROOT.integrity, 'native_integrity': 'sha512-' + NATIVE.integrity,
              'real_npm_filesystem_install_verified': False, 'lifecycle_scripts_enabled': False,
              'codex_executed': False, 'agentdeck_adapter_verified': False, 'network_isolated': False,
              'login_or_model_verified': False, 'release_published': False}
    # Reserve output first; never start installation when evidence cannot be saved.
    fd = os.open(args.evidence, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    code = 1
    with os.fdopen(fd, 'w', encoding='utf-8') as evidence:
        try:
            report.update(run(args.node, args.npm)); report['passed'] = True
            report['real_npm_filesystem_install_verified'] = True; code = 0
        except Exception as error:
            report['failure_code'] = str(error) if isinstance(error, Failure) else 'installation_verification_failed'
        json.dump(report, evidence, indent=2); evidence.write('\n'); evidence.flush(); os.fsync(evidence.fileno())
    print('Pinned Codex filesystem installation verified.' if code == 0 else 'Pinned Codex installation verification failed; inspect bounded JSON evidence.')
    return code


if __name__ == '__main__':
    raise SystemExit(main())
