"""Offline fixture/harness assertions. These do NOT certify native GUI or IPC."""
import json
import base64
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import Mock

import linux_install_webview as subject
import linux_qoder_webview as harness


class OfflineDriver:
    """Assertion model only, not evidence of a browser/native execution."""
    def __init__(self, fixture, install, fault=None):
        self.fixture = fixture; self.install = install; self.fault = fault
        self.dialog = False; self.confirmations = 0; self.clicks = []; self.commands = []; self.events = []
    def click(self, selector):
        self.clicks.append(selector)
        self.events.append(('click', selector))
        if selector == subject.PREVIEW: self.dialog = True
        if selector == subject.CANCEL: self.dialog = False
        if selector == subject.CONFIRM:
            self.dialog = False; self.confirmations += 1
            if self.fault == 'timeout': raise harness.Failure('ui_wait_expired')
            if self.confirmations == 2: self.install()
    def wheel_scroll_workspace_top(self):
        self.events.append(('wheel_top', None))
    def one(self, selector):
        if selector != "//*[@id='agent-search']": raise AssertionError('unexpected input')
        return 'search-element'
    def command(self, *args): self.commands.append(args)
    def type(self, *_): pass
    def enabled(self, *_): return True
    def value(self, *_): return 'Codex'
    def elements(self, selector):
        return ([{}] if self.dialog else []) if selector == subject.DIALOG else [{}]
    def wait(self, predicate, **_):
        if not predicate(): raise harness.Failure('ui_wait_expired')
    def text(self, selector):
        f = self.fixture
        fields = {'npm 包': '@openai/codex', '启动命令': 'codex', 'npm prefix': str(f['prefix']),
                  'npm 全局目录': str(f['global']), '包目录': str(f['package']),
                  '启动文件目录': str(f['bin']), 'npm 规范入口': str(f['manager']), '安装器 Node': str(f['node'])}
        if '/dd' in selector:
            return fields[selector.split("dt[text()='")[1].split("']")[0]]
        if selector == subject.DIALOG:
            return '版本由当前 npm 配置选择 生命周期脚本：遵循当前 npm 配置 仅安装软件，不应用 profile'
        if "@role='alert'" in selector:
            return '安装失败 ' + ('wrong failure' if self.fault == 'wrong_stale' else 'Installation target changed since confirmation')
        if "@role='status'" in selector: return '软件安装操作已完成，未应用配置'
        return 'npm 全局目录匹配，安装历史未知 实测版本 1.2.3 @openai/codex ' + str(f['package']) + ' ' + str(f['manager'])


class FixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.search = self.root / 'search'
        self.search.mkdir(mode=0o700)
        node = shutil.which('node')
        self.assertIsNotNone(node, 'Offline fixture contract requires official Node installed by CI')
        (self.search / 'node').symlink_to(Path(node).resolve())
        self.f = subject.prepare(self.root / 'fixture', str(self.search))

    def run_fixture(self, args):
        return subprocess.run([str(self.f['node']), str(self.f['manager']), *args],
                              capture_output=True, text=True, timeout=5,
                              env={'HOME': str(self.root), 'PATH': self.f['path']})

    def install_args(self):
        return ['install', '--global', '--prefix', str(self.f['prefix']), '--force=false',
                '--no-audit', '--no-fund', '--no-progress', '@openai/codex']

    def test_queries_are_readonly_install_is_exact_and_single_use(self):
        for query, key in [('prefix', 'prefix'), ('root', 'global')]:
            result = self.run_fixture([query, '--global'])
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, str(self.f[key]) + '\n')
            subject.assert_absent(self.f)
        self.assertEqual(subject.query_counts(self.f), {'prefix_queries': 1, 'root_queries': 1})
        result = self.run_fixture(self.install_args())
        self.assertEqual(result.returncode, 0, result.stderr)
        subject.assert_installed(self.f)
        result = subprocess.run([str(self.f['bin'] / 'codex'), '--version'],
                                env={'PATH': self.f['path']}, capture_output=True, text=True, timeout=5)
        self.assertEqual((result.returncode, result.stdout), (0, 'codex-cli 1.2.3\n'))
        self.assertEqual(self.run_fixture(self.install_args()).returncode, 64)
        subject.assert_installed(self.f)

    def test_rejects_malformed_arguments_without_side_effects(self):
        cases = [[], ['--version'], ['prefix'], ['root', '-g'], ['install', '-g', '@openai/codex'],
                 self.install_args() + ['--ignore-scripts'], self.install_args()[:-1] + ['other'],
                 ['install', '--global', '--prefix', '/tmp/escape', *self.install_args()[4:]]]
        for args in cases:
            with self.subTest(args=args):
                self.assertEqual(self.run_fixture(args).returncode, 64)
                subject.assert_absent(self.f)

    def test_scope_symlink_escape_is_rejected(self):
        outside = self.root / 'outside'; outside.mkdir(mode=0o700)
        (self.f['global'] / '@openai').symlink_to(outside, target_is_directory=True)
        self.assertEqual(self.run_fixture(self.install_args()).returncode, 64)
        self.assertEqual(list(outside.iterdir()), [])
        subject.assert_absent(self.f)

    def test_prefix_symlink_escape_is_rejected(self):
        saved = self.f['prefix'].with_name('saved-prefix')
        self.f['prefix'].rename(saved)
        self.f['prefix'].symlink_to(saved, target_is_directory=True)
        self.assertEqual(self.run_fixture(self.install_args()).returncode, 64)
        subject.assert_absent(self.f)

    def test_no_external_process_network_or_registry_contract(self):
        source = self.f['manager'].read_text()
        # Executed fixture has only two built-in imports and exact argv. This is
        # authored source inspection, not a claim to sandbox arbitrary JS.
        imports = [line for line in source.splitlines() if 'require(' in line]
        self.assertEqual(imports, ["const fs = require('node:fs');", "const path = require('node:path');"])
        for forbidden in ('fetch(', 'http:', 'https:', 'child_process', 'eval(', 'import('):
            self.assertNotIn(forbidden, source)
        self.assertEqual((self.f['manager_bin'] / 'node').resolve(), self.f['node'])
        self.assertEqual((self.f['manager_bin'] / 'npm').resolve(), self.f['manager'])
        for path in self.f['root'].rglob('*'):
            if path.is_dir(): self.assertEqual(path.stat().st_mode & 0o777, 0o700)

    def test_preexisting_candidates_including_broken_links_block(self):
        for kind in ('regular', 'broken'):
            candidate = self.search / 'codex'
            if kind == 'regular': candidate.write_text('not executable')
            else: candidate.symlink_to(self.root / 'missing')
            with self.assertRaises(subject.InstallFailure): subject.reject_existing_codex(str(self.search))
            candidate.unlink()
        for value in ('', '.', str(self.search) + ':'):
            with self.assertRaises(subject.InstallFailure): subject.reject_existing_codex(value)

    def test_fixed_failure_mapping_and_coverage(self):
        self.assertEqual(harness.failure_code(subject.InstallFailure('private contents')),
                         'reviewed_install_gui_acceptance_failed')
        subject.assert_coverage([{'stage': stage, 'verified': True} for stage in subject.STAGES])
        for invalid in ([], [{'stage': stage, 'verified': True} for stage in reversed(subject.STAGES)]):
            with self.assertRaises(subject.InstallFailure): subject.assert_coverage(invalid)

    def test_configuration_guard_allows_registry_not_configuration(self):
        home = self.root / 'home'; home.mkdir()
        product = home / '.agentdeck'; product.mkdir()
        (product / 'registry-update.lock').touch()
        subject.assert_no_configuration(home)
        for name in ('profiles', 'vault', 'backups', 'config.json'):
            (product / name).touch()
            with self.assertRaises(subject.InstallFailure): subject.assert_no_configuration(home)
            (product / name).unlink()
        (home / '.codex').mkdir()
        with self.assertRaises(subject.InstallFailure): subject.assert_no_configuration(home)

    def test_offline_assertion_model_covers_success_not_native_gui(self):
        records = []; trace = []; phases = []
        def install(): self.assertEqual(self.run_fixture(self.install_args()).returncode, 0)
        driver = OfflineDriver(self.f, install)
        subject.exercise(driver, self.f, self.root / 'empty-home', records, phases.append, trace)
        subject.assert_coverage(records)
        self.assertEqual(driver.clicks.count(subject.CONFIRM), 2)
        self.assertEqual(driver.events.count(('wheel_top', None)), 3)
        for index, event in enumerate(driver.events):
            if event == ('click', subject.PREVIEW):
                self.assertEqual(driver.events[index - 1], ('wheel_top', None))
        self.assertEqual(driver.clicks.count(subject.CANCEL), 1)
        self.assertNotIn("//*[@id='workspace-panel-agents']//h1", driver.clicks)
        self.assertEqual(driver.commands, [('POST', '/element/search-element/value', {'text': '\ue004', 'value': ['\ue004']})])
        self.assertLess(phases.index('install_search_tab'), phases.index('install_initial_preview_click'))
        self.assertIn(subject.DETAILS, driver.clicks)
        self.assertNotIn('Codex', subject.DETAILS)
        self.assertEqual([entry['stage'] for entry in trace], ['install_initial_preview', 'install_stale_preview', 'install_fresh_preview'])
        self.assertIn('install_initial_preview_wait_dialog', phases)
        self.assertIn('install_cancel_wait_closed', phases)

    def test_query_evidence_rejects_corrupt_or_linked_file(self):
        self.f['queries'].write_text('not-enum')
        with self.assertRaises(subject.InstallFailure): subject.query_counts(self.f)
        self.f['queries'].unlink()
        outside = self.root / 'outside'; outside.write_text('')
        self.f['queries'].symlink_to(outside)
        with self.assertRaises(subject.InstallFailure): subject.query_counts(self.f)
        self.assertEqual(self.run_fixture(['prefix', '--global']).returncode, 64)
        self.assertEqual(outside.read_text(), '')

    def test_failed_dialog_wait_preserves_query_evidence_without_reclick(self):
        trace = []
        driver = OfflineDriver(self.f, lambda: self.fail('must not install'))
        original = driver.elements
        driver.elements = lambda selector: [] if selector == subject.DIALOG else original(selector)
        with self.assertRaises(harness.Failure):
            subject.exercise(driver, self.f, self.root, [], lambda _: None, trace)
        self.assertEqual(driver.clicks.count(subject.PREVIEW), 1)
        self.assertEqual(driver.clicks.count(subject.CONFIRM), 0)
        self.assertEqual(trace, [{'stage': 'install_initial_preview',
                                 'before': {'prefix_queries': 0, 'root_queries': 0},
                                 'after': {'prefix_queries': 0, 'root_queries': 0}}])

    def test_failed_native_tab_is_not_replayed_or_followed_by_preview(self):
        driver = OfflineDriver(self.f, lambda: self.fail('must not install'))
        def fail_tab(*args):
            driver.commands.append(args)
            raise harness.Failure('driver_http_error')
        driver.command = fail_tab
        with self.assertRaises(harness.Failure):
            subject.exercise(driver, self.f, self.root, [], lambda _: None)
        self.assertEqual(len(driver.commands), 1)
        self.assertEqual(driver.clicks.count(subject.PREVIEW), 0)
        self.assertEqual(driver.clicks.count(subject.CONFIRM), 0)
        subject.assert_absent(self.f)

    def test_geometry_is_bounded_readonly_and_screenshot_canary_suppressed(self):
        driver = Mock()
        driver.one.return_value = 'synthetic-element'
        driver.last_response = {'operation': 'click'}
        png = b'\x89PNG\r\n\x1a\nsynthetic-placeholder-not-rendered'
        def read(method, endpoint):
            self.assertEqual(method, 'GET')
            self.assertNotIn('script', endpoint)
            if endpoint.endswith('/rect'): return {'x': 1, 'y': 2, 'width': 65, 'height': 32}
            if endpoint.endswith(('/enabled', '/displayed')): return True
            if '/property/' in endpoint: return 32
            if '/css/' in endpoint: return 'visible'
            if endpoint == '/element/active': return {harness.ELEMENT: 'synthetic-element'}
            if endpoint == '/source': return '<html>synthetic</html>'
            if endpoint == '/screenshot': return base64.b64encode(png).decode()
            self.fail('unexpected endpoint')
        driver.command.side_effect = read
        evidence = self.root / 'evidence'; evidence.mkdir()
        report = subject.capture_geometry(driver, evidence, 'install_initial_preview_before')
        self.assertEqual(report['button_rect']['width'], 65)
        self.assertEqual(report['active_element'], 'button')
        self.assertEqual((evidence / report['screenshot']).read_bytes(), png)
        self.assertEqual(driver.last_response, {'operation': 'click'})
        driver.command.side_effect = lambda method, endpoint: harness.CANARY if endpoint == '/source' else read(method, endpoint)
        report = subject.capture_geometry(driver, evidence, 'install_initial_preview_settled')
        self.assertEqual(report['screenshot'], 'unavailable_or_suppressed')
        self.assertFalse((evidence / 'install_initial_preview_settled.png').exists())
        with self.assertRaises(subject.InstallFailure): subject.capture_geometry(driver, evidence, '../escape')

    def test_failed_scroll_preflight_never_clicks_preview_or_confirms(self):
        driver = OfflineDriver(self.f, lambda: self.fail('must not install'))
        def fail_scroll():
            driver.events.append(('wheel_top', None))
            raise harness.Failure('ui_wait_expired')
        driver.wheel_scroll_workspace_top = fail_scroll
        phases = []
        with self.assertRaises(harness.Failure):
            subject.exercise(driver, self.f, self.root, [], phases.append)
        self.assertEqual(driver.events.count(('wheel_top', None)), 1)
        self.assertEqual(driver.clicks.count(subject.PREVIEW), 0)
        self.assertEqual(driver.clicks.count(subject.CONFIRM), 0)
        self.assertEqual(phases[-1], 'install_initial_preview_scroll_top')
        subject.assert_absent(self.f)

    def test_uncertain_confirm_never_replayed(self):
        driver = OfflineDriver(self.f, lambda: self.fail('must not install'), fault='timeout')
        with self.assertRaises(harness.Failure):
            subject.exercise(driver, self.f, self.root, [], lambda _: None)
        self.assertEqual(driver.clicks.count(subject.CONFIRM), 1)
        subject.assert_absent(self.f)

    def test_wrong_failure_does_not_count_as_stale_acceptance(self):
        records = []
        driver = OfflineDriver(self.f, lambda: self.fail('must not install'), fault='wrong_stale')
        with self.assertRaises(subject.InstallFailure):
            subject.exercise(driver, self.f, self.root, records, lambda _: None)
        self.assertEqual(records, [{'stage': 'preview_cancel', 'verified': True}])
        self.assertEqual(driver.clicks.count(subject.CONFIRM), 1)
        subject.assert_absent(self.f)

    def test_plan_mismatch_and_click_failure_never_replayed(self):
        driver = Mock()
        driver.text.return_value = 'wrong'
        with self.assertRaises(subject.InstallFailure): subject.assert_plan(driver, self.f)
        driver.click.side_effect = harness.Failure('ui_wait_expired')
        with self.assertRaises(harness.Failure): subject.exercise(driver, self.f, self.root, [], lambda _: None)
        self.assertEqual(driver.click.call_count, 1)
        subject.assert_absent(self.f)


if __name__ == '__main__': unittest.main()
