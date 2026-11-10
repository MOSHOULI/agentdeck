#!/usr/bin/env python3
"""Offline verifier regressions, not evidence of a real npm install."""
import base64
import email.message
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import Mock, patch

import codex_official_install as subject


class Response(io.BytesIO):
    status = 200
    def __init__(self, data, url):
        super().__init__(data); self.url = url
        self.headers = email.message.Message(); self.headers['Content-Length'] = str(len(data))
    def geturl(self): return self.url


class Tests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.serial = 0

    def manifest(self, native=False):
        value = {'name': '@openai/codex', 'version': subject.VERSION + ('-linux-x64' if native else '')}
        value.update({'os': ['linux'], 'cpu': ['x64']} if native else {'bin': {'codex': 'bin/codex.js'}, 'optionalDependencies': subject.OPTIONAL})
        return value

    def archive(self, native=False, extra=None, manifest=None, entries=None):
        self.serial += 1; path = self.root / ('archive-' + str(self.serial) + '.tgz')
        if entries is None:
            values = {'package.json': json.dumps(manifest or self.manifest(native)).encode(),
                      ('vendor/target/codex/codex' if native else 'bin/codex.js'): b'\x7fELFsynthetic-never-executed' if native else b'#!/usr/bin/env node\n'}
            values.update(extra or {})
            entries = [('package/' + name, data, 0o755 if name != 'package.json' else 0o644, tarfile.REGTYPE) for name, data in values.items()]
        with tarfile.open(path, 'w:gz') as archive:
            for name, data, mode, kind in entries:
                member = tarfile.TarInfo(name); member.mode = mode; member.type = kind
                if kind in (tarfile.REGTYPE, tarfile.AREGTYPE): member.size = len(data)
                if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE): member.linkname = '/tmp/outside'
                archive.addfile(member, io.BytesIO(data) if member.isfile() else None)
        pin = subject.Pin(subject.VERSION + ('-linux-x64' if native else ''),
                          base64.b64encode(hashlib.sha512(path.read_bytes()).digest()).decode(),
                          sum(kind in (tarfile.REGTYPE, tarfile.AREGTYPE) for _, _, _, kind in entries),
                          sum(len(data) for _, data, _, kind in entries if kind in (tarfile.REGTYPE, tarfile.AREGTYPE)), native)
        return path, pin

    def installed(self):
        rp, rpin = self.archive(); np, npin = self.archive(True)
        root_records = subject.inventory(rp, rpin); native_records = subject.inventory(np, npin)
        paths = subject.prepare(self.root / 'install')
        root_package = paths['prefix'] / 'lib/node_modules/@openai/codex'
        native_package = root_package / 'node_modules/@openai/codex-linux-x64'
        # Authored local fixture only. Never execute its bytes.
        for archive_path, destination in [(rp, root_package), (np, native_package)]:
            with tarfile.open(archive_path) as archive:
                for item in archive:
                    path = destination / item.name.removeprefix('package/')
                    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
                    path.write_bytes(archive.extractfile(item).read()); path.chmod(0o700 if item.mode & 0o111 else 0o600)
        (paths['prefix'] / 'bin').mkdir(mode=0o700)
        (paths['prefix'] / 'bin/codex').symlink_to('../lib/node_modules/@openai/codex/bin/codex.js')
        return paths, root_records, native_records

    def test_pins_are_fixed_platform_and_version(self):
        self.assertEqual(subject.ROOT.version, '0.167.0')
        self.assertEqual(subject.NATIVE.version, '0.167.0-linux-x64')
        self.assertTrue(subject.ROOT.url.startswith('https://registry.npmjs.org/@openai/codex/-/'))
        self.assertEqual((subject.ROOT.files, subject.NATIVE.files), (3, 45))
        self.assertEqual(len(subject.OPTIONAL), 6)

    def test_valid_archive_identity_inventory_and_corruption(self):
        for native in (False, True):
            path, pin = self.archive(native)
            records = subject.inventory(path, pin)
            self.assertEqual(len(records), 2)
            with path.open('ab') as stream: stream.write(b'x')
            with self.assertRaisesRegex(subject.Failure, 'archive_integrity_mismatch'): subject.inventory(path, pin)

    def test_manifest_rejects_scripts_dependencies_and_wrong_targets(self):
        for native in (False, True):
            original = self.manifest(native)
            for key, value in [('name', 'other'), ('version', 'latest'), ('scripts', {'install': 'do-not-run'}),
                               ('dependencies', {'other': '*'}), ('peerDependencies', {'other': '*'}),
                               ('bundleDependencies', ['other']), ('optionalDependencies', {'other': '*'})]:
                with self.subTest(native=native, key=key):
                    path, pin = self.archive(native, manifest={**original, key: value})
                    with self.assertRaises(subject.Failure): subject.inventory(path, pin)
        with self.assertRaises(subject.Failure): subject.json_object(b'{"name":"x","name":"y"}')

    def test_archive_rejects_links_paths_duplicates_modes_and_limits(self):
        cases = [[('../outside', b'x', 0o644, tarfile.REGTYPE)],
                 [('/package/file', b'x', 0o644, tarfile.REGTYPE)],
                 [('package/../outside', b'x', 0o644, tarfile.REGTYPE)],
                 [('package/a//b', b'x', 0o644, tarfile.REGTYPE)],
                 [('package/link', b'', 0o755, tarfile.SYMTYPE)],
                 [('package/link', b'', 0o755, tarfile.LNKTYPE)],
                 [('package/fifo', b'', 0o600, tarfile.FIFOTYPE)],
                 [('package/file', b'x', 0o666, tarfile.REGTYPE)],
                 [('package/file', b'x', 0o4755, tarfile.REGTYPE)],
                 [('package/file', b'x', 0o644, tarfile.REGTYPE)] * 2]
        for entries in cases:
            path, pin = self.archive(entries=entries)
            with self.subTest(entries=entries), self.assertRaises(subject.Failure): subject.inventory(path, pin)
        path, pin = self.archive()
        for attr, value in [('MAX_MEMBER', 1), ('MAX_MEMBERS', 1), ('MAX_EXPANDED', 1)]:
            with patch.object(subject, attr, value), self.assertRaises(subject.Failure): subject.inventory(path, pin)

    def test_fetch_rejects_response_changes_and_never_overwrites(self):
        # Small synthetic payload is accepted only with a patched test pin.
        data = b'fixture'; pin = subject.Pin(subject.VERSION, base64.b64encode(hashlib.sha512(data).digest()).decode(), 0, 0)
        for kind in ['good', 'redirect', 'status', 'length', 'encoding', 'transfer', 'short']:
            response = Response(data, pin.url)
            if kind == 'redirect': response.url = 'https://example.invalid/'
            if kind == 'status': response.status = 302
            if kind == 'length': response.headers['Content-Length'] = str(len(data))
            if kind == 'encoding': response.headers['Content-Encoding'] = 'gzip'
            if kind == 'transfer': response.headers['Transfer-Encoding'] = 'chunked'
            if kind == 'short': response.headers.replace_header('Content-Length', '99')
            opener = Mock(); opener.open.return_value = response
            destination = self.root / kind
            with patch.object(subject, 'ROOT', pin):
                if kind == 'good': subject.download(pin, destination, opener)
                else:
                    with self.assertRaises(subject.Failure): subject.download(pin, destination, opener)
        with self.assertRaises(FileExistsError): subject.download(subject.ROOT, self.root / 'good', Mock())
        with self.assertRaises(subject.Failure): subject.NoRedirect().redirect_request(None, None, 302, '', {}, pin.url)

    def test_environment_is_scrubbed_and_install_arguments_are_constrained(self):
        paths = subject.prepare(self.root / 'clean'); node = Path('/trusted/bin/node')
        with patch.dict(os.environ, {'NPM_TOKEN': 'secret', 'NODE_OPTIONS': '--require=evil', 'npm_config_registry': 'evil', 'HTTPS_PROXY': 'secret'}):
            env = subject.clean_environment(paths, node)
        for key in ('NPM_TOKEN', 'NODE_OPTIONS', 'npm_config_registry', 'HTTPS_PROXY'): self.assertNotIn(key, env)
        command = subject.npm_command(node, Path('/trusted/npm/bin/npm-cli.js'), paths, self.root / 'root.tgz')
        for flag in ('--ignore-scripts', '--include=optional', '--fetch-retries=0', '--force=false', '--registry=https://registry.npmjs.org/'): self.assertIn(flag, command)
        self.assertEqual(command[-1], str(self.root / 'root.tgz'))
        self.assertEqual(command[:3], ['/trusted/bin/node', '/trusted/npm/bin/npm-cli.js', 'install'])

    def test_complete_install_inventory_and_no_config(self):
        paths, root_records, native_records = self.installed()
        report = subject.verify_install(paths, root_records, native_records)
        self.assertEqual(report['native_files_verified'], 2)
        self.assertTrue(report['launcher_symlink_verified'])
        (paths['home'] / '.codex').mkdir()
        with self.assertRaisesRegex(subject.Failure, 'configuration_side_effect'): subject.verify_install(paths, root_records, native_records)

    def test_install_rejects_missing_changed_extra_and_linked_payloads(self):
        paths, roots, native = self.installed()
        target = paths['prefix'] / 'lib/node_modules/@openai/codex/bin/codex.js'
        data = target.read_bytes()
        target.write_bytes(b'x' * len(data))
        with self.assertRaisesRegex(subject.Failure, 'installed_file_digest_mismatch'): subject.verify_install(paths, roots, native)
        target.unlink()
        with self.assertRaises(subject.Failure): subject.verify_install(paths, roots, native)
        target.symlink_to('/tmp/outside')
        with self.assertRaises(subject.Failure): subject.verify_install(paths, roots, native)
        target.unlink(); target.write_bytes(data); target.chmod(0o700)
        (paths['prefix'] / 'extra').write_text('x')
        with self.assertRaises(subject.Failure): subject.verify_install(paths, roots, native)

    def test_process_output_deadline_and_nonzero_are_failures(self):
        env = {'PATH': '/usr/bin:/bin'}
        out, err = subject.bounded_process([sys.executable, '-c', 'print("fixture")'], self.root, env, 5)
        self.assertEqual((out, err), (b'fixture\n', b''))
        for code, seconds, expected in [('raise SystemExit(2)', 5, 'npm_process_failed'),
                                        ('import time; time.sleep(2)', 0.05, 'npm_process_timeout'),
                                        ('print("x"*100000)', 5, 'npm_output_limit')]:
            with self.assertRaisesRegex(subject.Failure, expected):
                subject.bounded_process([sys.executable, '-c', code], self.root, env, seconds)

    def test_main_failure_evidence_is_fixed_and_does_not_claim_install(self):
        evidence = self.root / 'result.json'
        argv = ['codex_official_install.py', '--node', '/node', '--npm', '/npm', '--evidence', str(evidence)]
        with patch.object(sys, 'argv', argv), patch.object(subject, 'run', side_effect=RuntimeError('SECRET-OUTPUT')):
            self.assertEqual(subject.main(), 1)
        result = json.loads(evidence.read_text())
        self.assertEqual(result['failure_code'], 'installation_verification_failed')
        self.assertFalse(result['real_npm_filesystem_install_verified'])
        self.assertFalse(result['codex_executed'])
        self.assertNotIn('SECRET-OUTPUT', evidence.read_text())
        with patch.object(sys, 'argv', argv), patch.object(subject, 'run') as run:
            with self.assertRaises(FileExistsError): subject.main()
            run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
