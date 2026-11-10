#!/usr/bin/env python3
"""Real pinned npm filesystem installation, never execute Codex or lifecycle code.

This is not AgentDeck adapter/runtime, login, model, or network-isolation acceptance.
Only Windows x64 is supported. No URL, version, hash or script-policy overrides.
"""
import argparse
import base64
from dataclasses import dataclass
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
import windows_job_process

VERSION = '0.167.0'
REGISTRY = 'https://registry.npmjs.org/'
ROOT_NAME = '@openai/codex'
NATIVE_NAME = '@openai/codex-win32-x64'
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
NATIVE = Pin(VERSION + '-win32-x64', 'duPCBwflc8qhQdLekbL8fDYF646tW4gj0/l9BDnsVhEzoqm6OwxMAJXDFwSOLUUJPCmjBeM+eHYAx4fP/hwfjg==', 47, 501483647, True)


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


def unsafe_reparse(metadata):
    return stat.S_ISLNK(metadata.st_mode) or bool(getattr(metadata, 'st_file_attributes', 0) & 0x400)


def reject_alternate_streams(path):
    if os.name != 'nt': return
    import ctypes
    from ctypes import wintypes as w
    class Stream(ctypes.Structure):
        _fields_ = [('size', ctypes.c_longlong), ('name', w.WCHAR * 296)]
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    k.FindFirstStreamW.argtypes = [w.LPCWSTR, ctypes.c_int, ctypes.POINTER(Stream), w.DWORD]; k.FindFirstStreamW.restype = w.HANDLE
    k.FindNextStreamW.argtypes = [w.HANDLE, ctypes.POINTER(Stream)]; k.FindNextStreamW.restype = w.BOOL
    k.FindClose.argtypes = [w.HANDLE]; k.FindClose.restype = w.BOOL
    data = Stream(); handle = k.FindFirstStreamW(str(path), 0, ctypes.byref(data), 0)
    if handle == ctypes.c_void_p(-1).value: raise Failure('file_streams_unavailable')
    try:
        if data.name != '::$DATA': raise Failure('alternate_stream_rejected')
        if k.FindNextStreamW(handle, ctypes.byref(data)): raise Failure('alternate_stream_rejected')
        if ctypes.get_last_error() != 38: raise Failure('file_streams_unavailable')
    finally: k.FindClose(handle)


def regular(path, limit):
    m = path.lstat()
    if unsafe_reparse(m) or not stat.S_ISREG(m.st_mode) or m.st_nlink != 1 or m.st_size > limit:
        raise Failure('unsafe_regular_file')
    reject_alternate_streams(path)
    return m


def open_read_no_follow(path):
    if os.name != 'nt':
        return os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    import ctypes
    import msvcrt
    from ctypes import wintypes
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p,
                                   wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
    kernel.CreateFileW.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]; kernel.CloseHandle.restype = wintypes.BOOL
    handle = kernel.CreateFileW(str(path), 0x80000000, 7, None, 3, 0x00200000, None)
    if handle == ctypes.c_void_p(-1).value: raise Failure('unsafe_regular_file')
    try: return msvcrt.open_osfhandle(handle, os.O_RDONLY | os.O_BINARY)
    except Exception:
        kernel.CloseHandle(handle); raise Failure('unsafe_regular_file') from None


def handle_fingerprint(fd):
    if os.name != 'nt':
        m = os.fstat(fd)
        return tuple(getattr(m, key) for key in ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_nlink', 'st_mtime_ns', 'st_ctime_ns'))
    import ctypes
    import msvcrt
    from ctypes import wintypes as w
    class Information(ctypes.Structure):
        _fields_ = [('attributes', w.DWORD), ('creation', w.FILETIME), ('access', w.FILETIME),
                    ('write', w.FILETIME), ('volume', w.DWORD), ('size_high', w.DWORD),
                    ('size_low', w.DWORD), ('links', w.DWORD), ('index_high', w.DWORD), ('index_low', w.DWORD)]
    class BasicInformation(ctypes.Structure):
        _fields_ = [('creation', ctypes.c_longlong), ('access', ctypes.c_longlong),
                    ('write', ctypes.c_longlong), ('change', ctypes.c_longlong), ('attributes', w.DWORD)]
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    k.GetFileInformationByHandleEx.argtypes = [w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD]
    k.GetFileInformationByHandleEx.restype = w.BOOL
    k.GetFileInformationByHandle.argtypes = [w.HANDLE, ctypes.POINTER(Information)]
    k.GetFileInformationByHandle.restype = w.BOOL
    info = Information()
    if not k.GetFileInformationByHandle(msvcrt.get_osfhandle(fd), ctypes.byref(info)):
        raise Failure('file_identity_unavailable')
    basic = BasicInformation()
    if not k.GetFileInformationByHandleEx(msvcrt.get_osfhandle(fd), 0, ctypes.byref(basic), ctypes.sizeof(basic)):
        raise Failure('file_identity_unavailable')
    if info.links != 1 or info.attributes & (0x400 | 0x10): raise Failure('unsafe_regular_file')
    if basic.attributes != info.attributes: raise Failure('file_changed')
    # Native identity/attributes avoid Python's path-extension-dependent st_mode.
    # Exclude access time: this verifier itself reads the file.
    return (info.volume, info.index_high, info.index_low, info.attributes, info.links,
            info.size_high, info.size_low, info.creation.dwHighDateTime, info.creation.dwLowDateTime,
            info.write.dwHighDateTime, info.write.dwLowDateTime, basic.creation, basic.write, basic.change)


def metadata_families_match(path_stat, handle_stat, windows):
    # Python 3.12 Windows path stat reports ctime as creation time while fstat
    # can report last-write time. Compare the explicit birthtime across families;
    # each family's ctime still must remain unchanged across the read below.
    keys = ('st_dev', 'st_ino', 'st_size', 'st_nlink', 'st_mtime_ns') + (
        ('st_birthtime_ns',) if windows else ('st_ctime_ns',))
    if any(not hasattr(item, key) for item in (path_stat, handle_stat) for key in keys): return False
    mode_mask = ~0o111 if windows else ~0
    return (path_stat.st_mode & mode_mask) == (handle_stat.st_mode & mode_mask) and all(
        getattr(path_stat, key) == getattr(handle_stat, key) for key in keys)


def file_digest(path, limit, algorithm='sha256'):
    before = regular(path, limit)
    fd = open_read_no_follow(path)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        if unsafe_reparse(opened) or opened.st_nlink != 1 or not metadata_families_match(before, opened, os.name == 'nt'): raise Failure('file_changed')
        pinned = handle_fingerprint(stream.fileno())
        named = open_read_no_follow(path)
        try:
            if handle_fingerprint(named) != pinned: raise Failure('file_changed')
        finally: os.close(named)
        digest = hashlib.new(algorithm)
        count = 0
        while block := stream.read(CHUNK):
            count += len(block)
            if count > limit: raise Failure('file_size_exceeded')
            digest.update(block)
        after = os.fstat(stream.fileno())
        if handle_fingerprint(stream.fileno()) != pinned: raise Failure('file_changed')
        named = open_read_no_follow(path)
        try:
            if handle_fingerprint(named) != pinned: raise Failure('file_changed')
        finally: os.close(named)
    final = path.lstat()
    keys = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_nlink', 'st_mtime_ns', 'st_ctime_ns')
    if os.name == 'nt': keys += ('st_birthtime_ns',)
    # Compare each Python metadata family with itself: on Windows path stat can
    # synthesize executable mode from '.cmd', while fstat has no filename.
    # Cross-family identity remains checked above, with native handle snapshots.
    if count != before.st_size or count != opened.st_size or any(
            getattr(opened, k) != getattr(after, k) or getattr(before, k) != getattr(final, k) for k in keys):
        raise Failure('file_changed')
    return digest.digest()


def download(pin, destination, opener=None, clock=time.monotonic):
    # Production calls only the two constants; no CLI/user-supplied URL.
    if pin not in (ROOT, NATIVE): raise Failure('download_pin_rejected')
    client = opener or urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    deadline = clock() + DOWNLOAD_SECONDS
    request = urllib.request.Request(pin.url, headers={'Accept-Encoding': 'identity', 'User-Agent': 'AgentDeck-pinned-install-verifier'})
    fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), 0o600)
    with os.fdopen(fd, 'wb') as out, client.open(request, timeout=15) as response:
        if response.status != 200 or response.geturl() != pin.url: raise Failure('download_response_rejected')
        if response.headers.get('Content-Encoding', 'identity').lower() != 'identity' or response.headers.get('Transfer-Encoding'):
            raise Failure('download_encoding_rejected')
        lengths = response.headers.get_all('Content-Length', [])
        if len(lengths) != 1 or not lengths[0].isascii() or not lengths[0].isdigit() or len(lengths[0]) > 10:
            raise Failure('download_length_rejected')
        expected = int(lengths[0])
        if expected != (181015521 if pin.native else 4992): raise Failure('download_length_rejected')
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
        if manifest.get('os') != ['win32'] or manifest.get('cpu') != ['x64'] or manifest.get('bin') not in (None, {}):
            raise Failure('native_platform_mismatch')
        if manifest.get('optionalDependencies') not in (None, {}): raise Failure('native_dependencies_changed')
    else:
        if manifest.get('bin') != {'codex': 'bin/codex.js'} or manifest.get('optionalDependencies') != OPTIONAL:
            raise Failure('root_entry_or_dependencies_changed')


def windows_component(part):
    if not part or part.endswith((' ', '.')) or any(c in '<>:"|?*' for c in part): return False
    base = part.split('.')[0].upper()
    return base not in {'CON', 'PRN', 'AUX', 'NUL', 'CONIN$', 'CONOUT$'} and not (len(base) == 4 and base[:3] in ('COM', 'LPT') and base[3] in '0123456789¹²³')


def inventory(archive_path, pin):
    if base64.b64encode(file_digest(archive_path, MAX_COMPRESSED, 'sha512')).decode() != pin.integrity:
        raise Failure('archive_integrity_mismatch')
    records = {}; seen = set(); folded = set(); spellings = {}; total = 0; manifest = None
    # Streaming traversal never extracts or executes the archive.
    with tarfile.open(archive_path, mode='r|gz') as archive:
        for member in archive:
            parts = PurePosixPath(member.name).parts
            if (len(seen) >= MAX_MEMBERS or not parts or parts[0] != 'package'
                or member.name.startswith('/') or '..' in parts or '\\' in member.name
                or any(ord(c) < 32 or ord(c) == 127 for c in member.name)
                or not (member.isfile() or member.isdir()) or member.issparse() or member.mode & 0o7022
                or any(not windows_component(part) for part in parts)):
                raise Failure('archive_member_rejected')
            relative = '/'.join(parts[1:])
            canonical = 'package' + ('/' + relative if relative else '')
            if member.name.rstrip('/') != canonical or canonical.casefold() in folded:
                raise Failure('archive_duplicate_or_noncanonical_path')
            for depth in range(1, len(parts) + 1):
                prefix = '/'.join(parts[:depth]); key = prefix.casefold()
                if key in spellings and spellings[key] != prefix: raise Failure('archive_case_alias')
                spellings[key] = prefix
            seen.add(canonical); folded.add(canonical.casefold())
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
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), 0o600)
        os.close(fd); paths[name] = path
    return paths


def windows_directory():
    import ctypes
    buffer = ctypes.create_unicode_buffer(32768)
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.GetWindowsDirectoryW.argtypes = [ctypes.c_wchar_p, ctypes.c_uint]
    kernel.GetWindowsDirectoryW.restype = ctypes.c_uint
    count = kernel.GetWindowsDirectoryW(buffer, len(buffer))
    if not 0 < count < len(buffer): raise Failure('system_directory_unavailable')
    return buffer.value


def clean_environment(paths, node):
    # Empty base: no auth, proxy, NODE_OPTIONS/NODE_PATH, npm or provider settings.
    system = windows_directory()
    return {'SystemRoot': system, 'WINDIR': system,
            'HOME': str(paths['home']), 'USERPROFILE': str(paths['home']),
            'APPDATA': str(paths['config']), 'LOCALAPPDATA': str(paths['data']),
            'XDG_CONFIG_HOME': str(paths['config']), 'XDG_DATA_HOME': str(paths['data']),
            'XDG_CACHE_HOME': str(paths['cache']), 'XDG_STATE_HOME': str(paths['state']),
            'TMPDIR': str(paths['tmp']), 'TMP': str(paths['tmp']), 'TEMP': str(paths['tmp']),
            'PATH': str(node.parent), 'NO_COLOR': '1', 'CI': 'true'}


def npm_command(node, npm, paths, root_archive):
    return [str(node), str(npm), 'install', '--global', '--prefix', str(paths['prefix']),
            '--cache', str(paths['cache']), '--userconfig', str(paths['userconfig']),
            '--globalconfig', str(paths['globalconfig']), '--registry=' + REGISTRY, '--@openai:registry=' + REGISTRY,
            '--ignore-scripts', '--include=optional', '--no-audit', '--no-fund', '--no-progress',
            '--package-lock=false', '--update-notifier=false', '--fetch-retries=0',
            '--fetch-timeout=30000', '--logs-max=0', '--force=false', str(root_archive)]


def bounded_process(argv, cwd, env, seconds=PROCESS_SECONDS):
    try: return windows_job_process.run(argv, cwd, env, seconds, OUTPUT_LIMIT)
    except windows_job_process.ProcessFailure as error: raise Failure(str(error)) from None


def expected_layout(root_records, native_records):
    package = PurePosixPath('node_modules/@openai/codex')
    native = package / 'node_modules/@openai/codex-win32-x64'
    files = {str(package / name): record for name, record in root_records.items()}
    files.update({str(native / name): record for name, record in native_records.items()})
    directories = set()
    for name in files:
        directories.update(str(parent) for parent in PurePosixPath(name).parents if str(parent) != '.')
    return files, directories


# Exact cmd-shim 8 template bytes for '#!/usr/bin/env node' and this fixed target.
# Never execute the POSIX, cmd, or PowerShell launcher to check it.
SHIMS = {
    'codex': (421, '6ee176b43f96b2294c8e2265d599f829e161b2c2ad34cb2b8fbf5f836fd34b8c'),
    'codex.cmd': (341, 'c54db6755e710c39703f7c37512f9e35ed41042d8080558d2b84b8d2694323c3'),
    'codex.ps1': (869, '0c149db80ed0bf442c810146b0ad0163b74982fe4542d673f56c354d7b8229cb'),
}


def verify_install(paths, root_records, native_records):
    prefix = paths['prefix']; files, directories = expected_layout(root_records, native_records)
    if unsafe_reparse(prefix.lstat()) or not stat.S_ISDIR(prefix.lstat().st_mode): raise Failure('installed_prefix_replaced')
    observed_files = set(); observed_dirs = set(); launchers = set()
    pending = [prefix]; count = 0
    while pending:
        directory = pending.pop()
        for path in directory.iterdir():
            count += 1
            if count > 256: raise Failure('installed_tree_limit')
            relative = path.relative_to(prefix).as_posix(); m = path.lstat()
            if unsafe_reparse(m): raise Failure('installed_link_rejected')
            if stat.S_ISDIR(m.st_mode):
                if relative not in directories: raise Failure('unexpected_installed_directory')
                observed_dirs.add(relative); pending.append(path)
            elif stat.S_ISREG(m.st_mode):
                if relative in SHIMS:
                    size, digest = SHIMS[relative]
                    if m.st_size != size or file_digest(path, 4096).hex() != digest: raise Failure('launcher_template_changed')
                    launchers.add(relative); continue
                expected = files.get(relative)
                if expected is None: raise Failure('unexpected_installed_file')
                if m.st_size != expected['size']: raise Failure('installed_file_metadata_mismatch')
                if file_digest(path, MAX_MEMBER).hex() != expected['sha256']: raise Failure('installed_file_digest_mismatch')
                observed_files.add(relative)
            else: raise Failure('installed_type_rejected')
    if observed_files != set(files) or observed_dirs != directories or launchers != set(SHIMS): raise Failure('installed_layout_incomplete')
    for name in ('home', 'config', 'data', 'state'):
        if unsafe_reparse(paths[name].lstat()) or not paths[name].is_dir() or any(paths[name].iterdir()): raise Failure('configuration_side_effect')
    for name in ('userconfig', 'globalconfig'):
        if regular(paths[name], 0).st_size != 0: raise Failure('npm_config_changed')
    return {'root_files_verified': len(root_records), 'native_files_verified': len(native_records),
            'installed_bytes_verified': sum(x['size'] for x in files.values()),
            'launcher_shims_verified': True, 'no_agent_configuration_created': True}


def toolchain(node_path, npm_path):
    for path in (node_path, npm_path):
        if not path.is_absolute(): raise Failure('tool_path_not_absolute')
    node = node_path.resolve(strict=True); npm = npm_path.resolve(strict=True)
    if not node.is_file() or node.name.lower() != 'node.exe' or not npm.is_file() or npm.name != 'npm-cli.js':
        raise Failure('toolchain_invalid')
    with node.open('rb') as stream:
        if stream.read(2) != b'MZ': raise Failure('node_not_native_pe')
    metadata = json_object((npm.parent.parent / 'package.json').read_bytes())
    if metadata.get('name') != 'npm': raise Failure('npm_identity_invalid')
    # Supplied local Node/npm are trusted developer dependencies, not a publisher
    # signature or an attestation about arbitrary user-selected executables.
    return node, npm


def run(node_path, npm_path):
    if sys.platform != 'win32' or platform.machine().lower() not in ('amd64', 'x86_64'): raise Failure('requires_windows_x64')
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
    report = {'buildCommit': os.environ.get('BUILD_SHA', ''), 'passed': False, 'package': ROOT_NAME, 'version': VERSION, 'platform': 'win32-x64',
              'root_integrity': 'sha512-' + ROOT.integrity, 'native_integrity': 'sha512-' + NATIVE.integrity,
              'real_npm_filesystem_install_verified': False, 'lifecycle_scripts_enabled': False,
              'codex_executed': False, 'agentdeck_adapter_verified': False, 'network_isolated': False,
              'login_or_model_verified': False, 'release_published': False}
    # Reserve output first; never start installation when evidence cannot be saved.
    fd = os.open(args.evidence, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), 0o600)
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
