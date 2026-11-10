"""Offline harness regressions only; not a real native WebView result."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import linux_codex_config_webview as subject
import linux_qoder_webview as harness


def fixture(root):
    package = root / 'package'; (package / 'bin').mkdir(parents=True)
    program = package / 'bin/codex.js'
    program.write_bytes(b"#!/usr/bin/env node\n'use strict';\nif (JSON.stringify(process.argv.slice(2)) !== '[\"--version\"]') process.exit(64);\nconsole.log('codex-cli 1.2.3');\n")
    program.chmod(0o700)
    queries = root / 'queries'; queries.write_bytes(b'pr')
    return {'root': root, 'package': package, 'queries': queries}


class Driver:
    def __init__(self, home, f, fault=None):
        self.home = home; self.fixture = f; self.fault = fault; self.show = False; self.calls = []
    def elements(self, selector):
        if selector == subject.PREVIEW: return [1, 2] if self.fault == 'ambiguous' else [1]
        if selector == subject.CLEAR or selector == subject.RESULT or '配置目标' in selector:
            return [1] if self.show else []
        if 'contains(text()' in selector: return [1] if self.fault == 'apply' else []
        return []
    def enabled(self, selector): return True
    def one(self, selector): return 'preview' if selector == subject.PREVIEW else 'clear'
    def wait(self, predicate):
        if not predicate(): raise harness.Failure('ui_wait_expired')
    def command(self, method, path, data=None):
        if method == 'GET': return subject.CANARY if self.fault == 'canary' else 'redacted source'
        self.calls.append(path)
        if self.fault == 'transport': raise harness.Failure('webdriver_command_failed')
        if '/preview/' in path:
            self.show = True
            if self.fault == 'write': (self.home / '.codex').mkdir(exist_ok=True)
            if self.fault == 'probe': (self.fixture['root'] / 'codex-config-process-marker').write_bytes(b'executed')
            if self.fault == 'query': self.fixture['queries'].write_bytes(b'p')
        else:
            self.show = self.fault == 'clear'
    def text(self, selector):
        if not self.show: return ''
        absent = not (self.home / '.codex').exists()
        text = ('默认 profile 缺失，使用内置模板计算；未创建 profile。冲突总数：0'
                + '文件状态：不存在；计划：拟创建。' * 2 if absent else
                '使用现有默认 profile；未修改源文件。冲突总数：2' + '文件状态：已存在；计划：存在冲突，不可应用。' * 2
                + '受管块标记不完整或重复现有配置值与模板不同')
        text += '应用功能尚未开放。'
        text += str(self.home / '.codex/AGENTS.md') + str(self.home / '.codex/config.toml')
        if self.fault == 'wrong_path': text = text.replace(str(self.home), '/wrong')
        if self.fault == 'raw_key': text += 'synthetic_fixture'
        return text


class Tests(unittest.TestCase):
    def test_snapshot_absence_bytes_metadata_and_unsafe_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp); before = subject.snapshot(home)
            self.assertEqual(before, {'.agentdeck': None, '.codex': None})
            root = home / '.codex'; root.mkdir(); path = root / 'AGENTS.md'; path.write_bytes(b'a')
            original = subject.snapshot(home); self.assertNotEqual(before, original)
            path.write_bytes(b'b'); self.assertNotEqual(original, subject.snapshot(home))
            path.unlink(); path.symlink_to(home / '.missing')
            with self.assertRaises(subject.CodexConfigFailure): subject.snapshot(home)

    def test_snapshot_rejects_hardlink_fifo_and_oversize(self):
        for kind in ('hardlink', 'fifo', 'oversize'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as tmp:
                home = Path(tmp); root = home / '.codex'; root.mkdir(); path = root / 'config.toml'
                if kind == 'hardlink':
                    other = home / 'other'; other.write_bytes(b''); os.link(other, path)
                elif kind == 'fifo': os.mkfifo(path)
                else: path.write_bytes(b'x' * 2_097_153)
                with self.assertRaises(subject.CodexConfigFailure): subject.snapshot(home)

    def test_sentinel_rejects_unrecognized_program(self):
        with tempfile.TemporaryDirectory() as tmp:
            f = fixture(Path(tmp)); (f['package'] / 'bin/codex.js').write_bytes(b'not fixture')
            with self.assertRaises(subject.CodexConfigFailure): subject.install_sentinel(f)

    def test_lifecycle_and_failure_detection(self):
        for fault in (None, 'canary', 'write', 'probe', 'query', 'apply', 'wrong_path', 'raw_key', 'clear'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); home = root / 'home'; home.mkdir(); f = fixture(root)
                records = []; driver = Driver(home, f, fault)
                if fault is None:
                    subject.exercise(driver, home, f, records, harness.write_private)
                    subject.assert_coverage(records)
                    self.assertEqual(driver.calls, ['/element/preview/value', '/element/clear/value'] * 2)
                else:
                    with self.assertRaises((subject.CodexConfigFailure, harness.Failure)):
                        subject.exercise(driver, home, f, records, harness.write_private)
                    self.assertLess(len(records), len(subject.STAGES))

    def test_ambiguous_and_transport_errors_not_replayed(self):
        for fault, count in [('ambiguous', 0), ('transport', 1)]:
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); home = root / 'home'; home.mkdir(); f = fixture(root)
                driver = Driver(home, f, fault); records = []
                with self.assertRaises((subject.CodexConfigFailure, harness.Failure)):
                    subject.exercise(driver, home, f, records, harness.write_private)
                self.assertEqual(len(driver.calls), count); self.assertEqual(records, [])

    def test_coverage_and_failure_redaction(self):
        records = [{'stage': name, 'verified': True} for name in subject.STAGES]
        subject.assert_coverage(records)
        for value in ([], records[:-1], records[::-1], records + records[:1]):
            with self.assertRaises(subject.CodexConfigFailure): subject.assert_coverage(value)
        self.assertEqual(harness.failure_code(subject.CodexConfigFailure(subject.CANARY)), 'codex_config_gui_acceptance_failed')

    def test_new_exercise_is_after_unchanged_install_gate(self):
        source = Path(harness.__file__).read_text()
        self.assertLess(source.index('install_gui.assert_coverage(install_records)'), source.index('codex_config.exercise('))
        self.assertIn("'codex_config_plan_checks': codex_config_records", source)
        self.assertIn('no_configuration', __import__('linux_install_webview').STAGES)
        self.assertIn("CODEX_HOME=str(home / '.codex')", source)
        self.assertEqual(subject.CANARY, harness.CANARY)



class AnimationsDriver:
    """Offline UI/filesystem model, not evidence of native IPC behavior."""
    def __init__(self, home, f, fault=None):
        self.home = home; self.fixture = f; self.fault = fault; self.status = None
        self.target = home / '.codex/config.toml'
        self.receipt = self.target.parent / '.agentdeck-codex-animations.json'
        self.calls = []; self.preview_bytes = None
    def elements(self, selector):
        if 'result' in selector:
            if 'data-status=' in selector:
                return [1] if "data-status='" + str(self.status) + "'" in selector else []
            return [1] if self.status else []
        return [1]
    def one(self, selector): return selector.split("data-testid='")[1].split("'")[0]
    def enabled(self, selector): return True
    def wait(self, predicate):
        if not predicate(): raise harness.Failure('ui_wait_expired')
    def text(self, selector): return subject.CANARY if self.fault == 'leak' else 'redacted'
    def command(self, method, path, data=None):
        if path.endswith('/selected'): return False
        if method == 'GET': return 'redacted'
        self.calls.append(path)
        if self.fault == 'transport': raise harness.Failure('webdriver_command_failed')
        name = path.split('/')[2].replace('codex-animations-', '')
        if name == 'preview':
            self.preview_bytes = self.target.read_bytes(); self.status = 'preview'
            if self.fault == 'preview_write': self.target.write_bytes(b'modified')
        elif name == 'cancel': self.status = None
        elif name == 'confirm-apply':
            if self.target.read_bytes() != self.preview_bytes:
                self.status = 'error'
            else:
                self.original = self.preview_bytes
                self.owned = self.original.replace(b'animations = true', b'animations = false')
                self.target.write_bytes(self.owned)
                self.receipt.write_text(json.dumps({'phase': 'applied', 'field': 'tui.animations',
                    'before': True, 'after': False, 'before_digest': subject.digest(self.original),
                    'after_digest': subject.digest(self.owned)})); self.receipt.chmod(0o600)
                self.status = 'applied'
                if self.fault == 'receipt': self.receipt.write_text('{}')
        elif name == 'inspect': self.status = 'recovery'
        elif name == 'confirm-restore':
            if self.target.read_bytes() != self.owned:
                self.status = 'error'
                if self.fault == 'lost_evidence': self.receipt.unlink()
            else:
                self.target.write_bytes(self.original); self.receipt.unlink(); self.status = 'restored'
        if self.fault == 'process':
            (self.fixture['root'] / 'codex-config-process-marker').write_bytes(b'executed')


class AnimationTests(unittest.TestCase):
    def test_lifecycle_and_guards(self):
        for fault in (None, 'preview_write', 'receipt', 'lost_evidence', 'leak', 'process', 'transport'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); home = root / 'home'; home.mkdir(); f = fixture(root)
                subject.exercise(Driver(home, f), home, f, [], harness.write_private)
                driver = AnimationsDriver(home, f, fault); records = []
                if fault is None:
                    subject.exercise_animations(driver, home, f, records, harness.write_private)
                    subject.assert_animation_coverage(records)
                else:
                    with self.assertRaises((subject.CodexConfigFailure, harness.Failure)):
                        subject.exercise_animations(driver, home, f, records, harness.write_private)
                    self.assertLess(len(records), len(subject.ANIMATION_STAGES))
                if fault == 'transport': self.assertEqual(len(driver.calls), 1)

    def test_distinct_complete_coverage_after_read_only_gate(self):
        records = [{'stage': name, 'verified': True} for name in subject.ANIMATION_STAGES]
        subject.assert_animation_coverage(records)
        for bad in ([], records[:-1], records[::-1], records + records[:1]):
            with self.assertRaises(subject.CodexConfigFailure): subject.assert_animation_coverage(bad)
        source = Path(harness.__file__).read_text()
        self.assertLess(source.index('codex_config.assert_coverage(codex_config_records)'),
                        source.index('codex_config.exercise_animations('))


if __name__ == '__main__': unittest.main()
