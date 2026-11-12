"""Offline fixture/harness assertions. These do NOT certify native GUI or IPC."""
import json
import base64
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import xml.etree.ElementTree as ET
from unittest.mock import Mock, patch

import installer_node_fixture
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
        if selector == subject.PREVIEW: return 'preview-element'
        if selector == subject.DETAILS: return 'details-element'
        if selector != "//*[@id='agent-search']": raise AssertionError('unexpected input')
        return 'search-element'
    def command(self, *args):
        self.commands.append(args)
        if args[1] == '/element/preview-element/value':
            self.dialog = True
    def type(self, *_): pass
    def enabled(self, *_): return True
    def value(self, *_): return 'Codex'
    def elements(self, selector):
        return ([{}] if self.dialog else []) if selector == subject.DIALOG else [{}]
    def wait(self, predicate, **_):
        if not predicate(): raise harness.Failure('ui_wait_expired')
    def text(self, selector):
        f = self.fixture
        fields = {'npm 包': '@openai/codex', 'Codex 根包固定版本': '1.2.3', '启动命令': 'codex', 'npm prefix': str(f['prefix']),
                  'npm 全局目录': str(f['global']), '包目录': str(f['package']),
                  '启动文件目录': str(f['bin']), 'npm 规范入口': str(f['manager']), '安装器 Node': str(f['node']),
                  '安装器 Node 实测版本': f['installer_node_version'],
                  'AgentDeck 安装器 Node 最低策略': '18.0.0'}
        if '/dd' in selector:
            return fields[selector.split("dt[text()='")[1].split("']")[0]]
        if selector == subject.DIALOG:
            return 'Codex 根包版本已固定 生命周期脚本：禁用（ignore-scripts） 仅安装软件，不应用 profile'
        if "@role='alert'" in selector:
            return 'wrong failure' if self.fault == 'wrong_stale' else '安装请求已发送，但完成状态未确认。'
        if "@role='status'" in selector: return '软件安装操作已完成，未应用配置'
        return 'npm 全局目录匹配，安装历史未知 实测版本 1.2.3 @openai/codex ' + str(f['package']) + ' ' + str(f['manager_bin'] / 'npm')


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
                '--no-audit', '--no-fund', '--no-progress', '--ignore-scripts', '@openai/codex@1.2.3']

    def test_native_node_expected_version_matches_real_program(self):
        actual = subprocess.run([str(self.f['node']), '--version'], capture_output=True,
                                check=True, timeout=5).stdout.decode().strip()
        self.assertEqual(self.f['installer_node_version'], actual)
        subject.assert_absent(self.f)

    def test_expected_node_version_rejects_malformed_failure_and_preloads(self):
        for output in (b'v18.0.0\n\r\n', b'v18.0.0-rc.1', b'v018.0.0',
                       b'v17.9.0', b'v4294967296.0.0', b'', b'v18.0.0\nextra', b'v18.0.0 '):
            with patch.object(installer_node_fixture.subprocess, 'run', return_value=Mock(returncode=0, stdout=output)):
                with self.assertRaises(ValueError): installer_node_fixture.measured_version(self.f['node'])
        with patch.object(installer_node_fixture.subprocess, 'run', return_value=Mock(returncode=1, stdout=b'v18.0.0')):
            with self.assertRaises(ValueError): installer_node_fixture.measured_version(self.f['node'])
        with patch.dict(os.environ, {'NODE_OPTIONS':'--require=private', 'Node_Path':'private', 'npm_config_node_options':'private'}), \
             patch.object(installer_node_fixture.subprocess, 'run', return_value=Mock(returncode=0, stdout=b'v18.0.0\r\n')) as run:
            self.assertEqual(installer_node_fixture.measured_version(self.f['node']), 'v18.0.0')
            self.assertEqual(run.call_args.args[0], [str(self.f['node']), '--version'])
            self.assertFalse({'node_options','node_path','npm_config_node_options'} & {k.lower() for k in run.call_args.kwargs['env']})

    def test_queries_are_readonly_install_is_exact_and_single_use(self):
        for query, key in [('prefix', 'prefix'), ('root', 'global')]:
            result = self.run_fixture([query, '--global'])
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, str(self.f[key]) + '\n')
            subject.assert_absent(self.f)
        self.assertEqual(subject.query_counts(self.f), {'prefix_queries': 1, 'root_queries': 1, 'metadata_queries': 0})
        result = self.run_fixture(self.install_args())
        self.assertEqual(result.returncode, 0, result.stderr)
        subject.assert_installed(self.f)
        result = subprocess.run([str(self.f['bin'] / 'codex'), '--version'],
                                env={'PATH': self.f['path']}, capture_output=True, text=True, timeout=5)
        self.assertEqual((result.returncode, result.stdout), (0, 'codex-cli 1.2.3\n'))
        self.assertEqual(self.run_fixture(self.install_args()).returncode, 64)
        subject.assert_installed(self.f)

    def test_latest_metadata_changes_without_manager_mutation_or_install(self):
        manager = self.f['manager'].read_bytes()
        query = ['view', '@openai/codex@latest', 'version', '--json', '--fetch-retries=0', '--fetch-timeout=25000']
        for version in ('1.2.3', '1.2.4'):
            self.f['latest'].write_text(version)
            result = self.run_fixture(query)
            self.assertEqual((result.returncode, json.loads(result.stdout)), (0, version))
            subject.assert_absent(self.f)
        self.assertEqual(self.f['manager'].read_bytes(), manager)
        self.assertEqual(subject.query_counts(self.f)['metadata_queries'], 2)
        self.assertEqual(self.run_fixture(self.install_args()[:-1] + ['@openai/codex']).returncode, 64)
        self.assertEqual(self.run_fixture(self.install_args()[:-1] + ['@openai/codex@latest']).returncode, 64)
        self.assertEqual(self.run_fixture(self.install_args()[:-1] + ['@openai/codex@1.2.4']).returncode, 64)
        subject.assert_absent(self.f)

    def test_rejects_malformed_arguments_without_side_effects(self):
        cases = [[], ['--version'], ['prefix'], ['root', '-g'], ['install', '-g', '@openai/codex'],
                 self.install_args() + ['--ignore-scripts'],
                 [arg for arg in self.install_args() if arg != '--ignore-scripts'], self.install_args()[:-1] + ['other'],
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
        self.assertEqual(driver.clicks.count(subject.PREVIEW), 0)
        self.assertEqual(driver.clicks.count(subject.CANCEL), 1)
        self.assertNotIn("//*[@id='workspace-panel-agents']//h1", driver.clicks)
        self.assertEqual(driver.commands, [('POST', '/element/search-element/value', {'text': '\ue004', 'value': ['\ue004']})] + [('POST', '/element/preview-element/value', {'text': '\ue007', 'value': ['\ue007']})] * 3 + [('POST', '/element/details-element/value', {'text': '\ue007', 'value': ['\ue007']})])
        self.assertLess(phases.index('install_search_tab'), phases.index('install_initial_preview_click'))
        self.assertNotIn(subject.DETAILS, driver.clicks)
        self.assertNotIn('Codex', subject.DETAILS)
        self.assertEqual([entry['stage'] for entry in trace], ['install_initial_preview', 'install_stale_preview', 'install_fresh_preview'])
        self.assertIn('install_initial_preview_wait_dialog', phases)
        self.assertIn('install_cancel_wait_closed', phases)

    def test_source_requires_observed_npm_command_not_canonical_preview_entry(self):
        records = []
        def install(): self.assertEqual(self.run_fixture(self.install_args()).returncode, 0)
        driver = OfflineDriver(self.f, install)
        original = driver.text
        driver.text = lambda selector: original(selector).replace(str(self.f['manager_bin'] / 'npm'), str(self.f['manager'])) if selector == subject.SOURCE else original(selector)
        with self.assertRaisesRegex(subject.InstallFailure, 'source_mismatch'):
            subject.exercise(driver, self.f, self.root / 'empty-home', records, lambda _: None)
        self.assertEqual([entry['stage'] for entry in records], ['preview_cancel', 'stale_revision', 'confirmed_install'])
        self.assertEqual(driver.clicks.count(subject.CONFIRM), 2)

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
        self.assertEqual(sum(args[1] == '/element/preview-element/value' for args in driver.commands), 1)
        self.assertEqual(driver.clicks.count(subject.CONFIRM), 0)
        self.assertEqual(trace, [{'stage': 'install_initial_preview',
                                 'before': {'prefix_queries': 0, 'root_queries': 0, 'metadata_queries': 0},
                                 'after': {'prefix_queries': 0, 'root_queries': 0, 'metadata_queries': 0}}])

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

    def test_failed_native_preview_activation_is_never_replayed(self):
        driver = OfflineDriver(self.f, lambda: self.fail('must not install'))
        original = driver.command
        def fail_activation(*args):
            original(*args)
            if args[1] == '/element/preview-element/value':
                raise harness.Failure('driver_http_error')
        driver.command = fail_activation
        with self.assertRaises(harness.Failure):
            subject.exercise(driver, self.f, self.root, [], lambda _: None)
        self.assertEqual(sum(args[1] == '/element/preview-element/value' for args in driver.commands), 1)
        self.assertEqual(driver.clicks.count(subject.PREVIEW), 0)
        self.assertEqual(driver.clicks.count(subject.CONFIRM), 0)
        subject.assert_absent(self.f)

    def test_configuration_diagnostics_only_relative_names_and_bounded(self):
        home = self.root / 'diagnostic-home'; home.mkdir()
        product = home / '.agentdeck'; product.mkdir()
        for index in range(40):
            (product / ('profile.' + str(index))).write_text('SECRET-CONTENT')
        facts = subject.configuration_facts(home)
        self.assertEqual(len(facts['product_entries']), 32)
        self.assertEqual(len(facts['matching_paths']), 32)
        self.assertNotIn('SECRET-CONTENT', json.dumps(facts))
        self.assertTrue(all(not Path(p).is_absolute() for p in facts['matching_paths']))

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





class DetailsActivationTests(unittest.TestCase):
    def test_native_enter_once_no_coordinate_click_or_retry(self):
        driver = Mock()
        driver.elements.return_value = [1]
        driver.enabled.return_value = True
        driver.one.return_value = 'details'
        driver.wait.side_effect = lambda predicate: self.assertTrue(predicate())
        subject.open_details(driver)
        driver.click.assert_not_called()
        driver.command.assert_called_once_with('POST', '/element/details/value',
            {'text': '\ue007', 'value': ['\ue007']})

    def test_transport_failure_never_replayed(self):
        driver = Mock()
        driver.elements.return_value = [1]
        driver.enabled.return_value = True
        driver.one.return_value = 'details'
        driver.wait.side_effect = lambda predicate: self.assertTrue(predicate())
        driver.command.side_effect = harness.Failure('webdriver_command_failed')
        with self.assertRaises(harness.Failure): subject.open_details(driver)
        self.assertEqual(driver.command.call_count, 1)
        driver.click.assert_not_called()

class WrongRootFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name); self.search=self.root/'search'; self.search.mkdir(mode=0o700)
        node=shutil.which('node'); self.assertIsNotNone(node)
        (self.search/'node').symlink_to(Path(node).resolve())
        self.f=subject.prepare(self.root/'wrong-root',str(self.search),mode='wrong-root')
    def install(self):
        args=['install','--global','--prefix',str(self.f['prefix']),'--force=false','--no-audit','--no-fund','--no-progress','--ignore-scripts','@openai/codex@1.2.3']
        result=subprocess.run([str(self.f['node']),str(self.f['manager']),*args],capture_output=True,timeout=5,env={'PATH':self.f['path'],'HOME':str(self.root)})
        self.assertEqual(result.returncode,0,result.stderr)
    def test_complete_wrong_root_is_retained_without_probe(self):
        self.install(); snapshot=subject.assert_wrong_root(self.f)
        for _ in range(3): self.assertEqual(subject.assert_wrong_root(self.f,snapshot),snapshot)
        self.assertEqual(self.f['calls'].read_bytes(),b'i')
        self.assertEqual(self.f['versions'].read_bytes(),b'')
    def test_authored_launcher_sentinel_detects_any_probe(self):
        self.install()
        result=subprocess.run([str(self.f['node']),str(self.f['package']/'bin/codex.js'),'--version'],capture_output=True,timeout=5)
        self.assertEqual(result.returncode,0)
        with self.assertRaisesRegex(subject.InstallFailure,'wrong_root_agent_executed'):subject.assert_wrong_root(self.f)
    def test_snapshot_detects_metadata_mutation_without_ignoring_timestamps(self):
        self.install(); snapshot=subject.assert_wrong_root(self.f)
        path=self.f['package']/'bin/codex.js'; meta=path.stat()
        os.utime(path,ns=(meta.st_atime_ns,meta.st_mtime_ns-1000000))
        with self.assertRaisesRegex(subject.InstallFailure,'wrong_root_residue_changed'):subject.assert_wrong_root(self.f,snapshot)
    def test_snapshot_detects_changed_complete_payload(self):
        self.install(); snapshot=subject.assert_wrong_root(self.f)
        with (self.f['package']/'bin/codex.js').open('a',encoding='utf-8') as stream:stream.write('// mutation')
        with self.assertRaisesRegex(subject.InstallFailure,'wrong_root_residue_changed'):subject.assert_wrong_root(self.f,snapshot)
    def test_offline_wrong_root_flow_scopes_duplicate_refresh_buttons(self):
        owner=self
        mounted_panels=ET.fromstring("<main><section id='workspace-panel-agents'><button>刷新列表</button></section><section id='workspace-panel-vault' hidden=''><button>刷新列表</button></section></main>")
        class Driver(OfflineDriver):
            def click(self,selector):
                if selector==subject.CONFIRM:
                    self.clicks.append(selector);self.dialog=False;owner.install()
                else:super().click(selector)
            def elements(self,selector):
                # Both mounted workspaces expose the same button text. The
                # Vault workspace remains hidden, not absent from the DOM.
                if selector=="//button[text()='刷新列表']":
                    return [node for node in mounted_panels.iter('button') if node.text=='刷新列表']
                if selector=="//*[@id='workspace-panel-agents' and not(@hidden)]//button[text()='刷新列表']":
                    return [node for panel in mounted_panels if panel.get('id')=='workspace-panel-agents' and 'hidden' not in panel.attrib for node in panel.iter('button') if node.text=='刷新列表']
                return super().elements(selector)
            def enabled(self,selector):
                if len(self.elements(selector))!=1:raise harness.Failure('missing_or_ambiguous_control')
                if selector==subject.PREVIEW and self.fixture['marker'].exists():return False
                return super().enabled(selector)
            def text(self,selector):
                if selector=="//*[@aria-label='Agent 列表']":return subject.WRONG_ROOT_GUARD
                if selector=='//body':return subject.WRONG_ROOT_GUARD
                return super().text(selector)
        driver=Driver(self.f,self.install);records=[]
        subject.exercise_wrong_root(driver,self.f,self.root/'empty-home',records,lambda _:None)
        subject.assert_wrong_root_coverage(records)
        self.assertEqual(driver.clicks.count(subject.CONFIRM),1)
        self.assertEqual(len(driver.elements("//button[text()='刷新列表']")),2)
        self.assertEqual(driver.clicks.count("//*[@id='workspace-panel-agents' and not(@hidden)]//button[text()='刷新列表']"),1)
        self.assertNotIn("//button[text()='刷新列表']",driver.clicks)
        self.assertEqual(self.f['calls'].read_bytes(),b'i')
        self.assertEqual(self.f['versions'].read_bytes(),b'')
    def test_wrong_root_waits_for_workspace_before_single_input(self):
        class Driver(RecoveryWorkspaceReadinessTests.Driver):
            def type(self, selector, value):
                super().type(selector, value)
                raise harness.Failure('test_stop_after_readiness')
        for selected in (True, False):
            with self.subTest(selected=selected):
                driver = Driver(selected=selected, delayed=True); phases=[]
                with self.assertRaisesRegex(harness.Failure, 'test_stop_after_readiness'):
                    subject.exercise_wrong_root(driver, self.f, self.root, [], phases.append)
                self.assertEqual(len(driver.clicks), 0 if selected else 1)
                self.assertEqual(driver.typed, 1)
                self.assertEqual(driver.commands, [])
                self.assertEqual(phases, ['wrong_root_wait_workspace', 'wrong_root_type_search'])
                self.assertTrue(any("@aria-busy='false'" in q for q in driver.polls))

    def test_wrong_root_missing_ambiguous_or_disabled_tab_never_acted_on(self):
        tab = "//*[@role='tab' and @aria-label='Agent 管理']"
        for count, enabled in ((0,True),(2,True),(1,False)):
            with self.subTest(count=count, enabled=enabled):
                driver = Mock()
                driver.elements.side_effect = lambda q: ([{}]*count if q==tab else ([{}] if enabled else []))
                def wait(predicate):
                    self.assertFalse(predicate()); raise harness.Failure('ui_wait_expired')
                driver.wait.side_effect=wait
                with self.assertRaisesRegex(subject.InstallFailure, 'wrong_root_workspace_tab_not_ready'):
                    subject.exercise_wrong_root(driver,self.f,self.root,[],lambda _:None)
                driver.click.assert_not_called();driver.type.assert_not_called();driver.enabled.assert_not_called()

    def test_wrong_root_uncertain_first_click_is_not_replayed(self):
        driver = RecoveryWorkspaceReadinessTests.Driver(selected=False, click_failure=True)
        with self.assertRaisesRegex(subject.InstallFailure, 'wrong_root_workspace_tab_activation_failed'):
            subject.exercise_wrong_root(driver,self.f,self.root,[],lambda _:None)
        self.assertEqual(len(driver.clicks),1);self.assertEqual(driver.typed,0)
        self.assertEqual(driver.commands,[])

    def test_native_harness_keeps_success_lane_and_distinct_wrong_root_process(self):
        source=Path(harness.__file__).read_text(encoding='utf-8')
        self.assertLess(source.index('memory_export.assert_coverage(memory_export_records)'),source.index("phase('wrong_root_close_success_session')"))
        close=source.index("phase('wrong_root_close_success_session')")
        self.assertLess(source.index("client.command('DELETE', '')",close),source.index("mode='wrong-root'",close))
        self.assertLess(source.index("mode='wrong-root'",close),source.index('install_gui.exercise_wrong_root(',close))
        self.assertIn('install_gui.assert_wrong_root_coverage(wrong_root_records)',source)
        self.assertIn("'wrong_root_install_gui_verified': passed and",source)

class RecoveryWorkspaceReadinessTests(unittest.TestCase):
    class Driver(OfflineDriver):
        def __init__(self, selected=True, delayed=False, click_failure=False, blocked=None):
            super().__init__(None, None)
            self.selected = selected; self.delayed = delayed
            self.click_failure = click_failure; self.blocked = blocked
            self.polls = {}; self.typed = 0
        def elements(self, selector):
            self.polls[selector] = self.polls.get(selector, 0) + 1
            if self.blocked and self.blocked in selector: return []
            if self.delayed and self.polls[selector] == 1: return []
            if "@aria-selected='true'" in selector: return [{}] if self.selected else []
            if "@aria-selected='false'" in selector: return [] if self.selected else [{}]
            return [{}]
        def wait(self, predicate, **_):
            for _ in range(10):
                if predicate(): return
            raise harness.Failure('ui_wait_expired')
        def enabled(self, selector):
            raise AssertionError('do not re-locate controls through enabled() during readiness')
        def click(self, selector):
            self.clicks.append(selector)
            if self.click_failure: raise harness.Failure('uncertain_click')
            self.selected = True
        def type(self, selector, text):
            self.typed += 1
            if not self.selected: raise AssertionError('input before selected workspace')

    def test_selected_workspace_waits_for_delayed_dom_without_click(self):
        driver = self.Driver(delayed=True)
        subject.select_recovery_agent(driver)
        self.assertEqual(driver.clicks, [])
        self.assertEqual(driver.typed, 1)
        self.assertTrue(any("@aria-busy='false'" in q for q in driver.polls))
        self.assertEqual(len(driver.commands), 1)  # one Tab, no replay

    def test_unselected_workspace_receives_one_navigation_click(self):
        driver = self.Driver(selected=False)
        subject.select_recovery_agent(driver)
        self.assertEqual(len(driver.clicks), 1)
        self.assertIn("@aria-selected='false'", driver.clicks[0])
        self.assertEqual(driver.typed, 1)

    def test_uncertain_navigation_action_is_not_replayed(self):
        driver = self.Driver(selected=False, click_failure=True)
        with self.assertRaisesRegex(subject.InstallFailure, 'recovery_workspace_tab_activation_failed'):
            subject.select_recovery_agent(driver)
        self.assertEqual(len(driver.clicks), 1)
        self.assertEqual(driver.typed, 0)
        self.assertEqual(driver.commands, [])

    def test_missing_search_or_busy_list_fails_before_input_with_diagnostic(self):
        for blocked, diagnostic in [('agent-search', 'search_not_ready'),
                                    ("@aria-busy='false'", 'list_not_idle')]:
            with self.subTest(blocked=blocked):
                driver = self.Driver(blocked=blocked)
                with self.assertRaisesRegex(subject.InstallFailure, diagnostic):
                    subject.select_recovery_agent(driver)
                self.assertEqual(driver.clicks, [])
                self.assertEqual(driver.typed, 0)
                self.assertEqual(driver.commands, [])


class DurableRecoveryFixtureTests(unittest.TestCase):
    setUp=WrongRootFixtureTests.setUp
    install=WrongRootFixtureTests.install
    def create_state(self,home,phase='failed_returned'):
        import uuid
        home.mkdir(mode=0o700,exist_ok=True)
        product=home/'.agentdeck';product.mkdir(mode=0o700)
        parent=product/'install-protection';parent.mkdir(mode=0o700)
        directory=parent/'codex';directory.mkdir(mode=0o700)
        subject.private_write(directory/'gate.lock','')
        record={'schema':1,'platform':'linux','phase':phase,'attempt':str(uuid.uuid4()),'prefix':str(self.f['prefix']),'prefixIdentity':'test-only-identity'}
        subject.private_write(directory/'state.json',json.dumps(record))
        return record
    def test_manual_relocation_preserves_prefix_ancestors_and_original_bytes(self):
        self.install()
        parents=[self.f['prefix'],self.f['bin'],self.f['global'],self.f['global'].parent]
        identities=[(p.stat().st_dev,p.stat().st_ino) for p in parents]
        payload=(self.f['package']/'bin/codex.js').read_bytes()
        archive=subject.relocate_authored_residue(self.f)
        self.assertEqual(identities,[(p.stat().st_dev,p.stat().st_ino) for p in parents])
        self.assertEqual((archive/'codex-package/bin/codex.js').read_bytes(),payload)
        self.assertFalse(self.f['package'].exists());self.assertFalse(os.path.lexists(self.f['bin']/'codex'))
        self.install();subject.assert_wrong_root(self.f,attempts=2)
        self.assertEqual((archive/'codex-package/bin/codex.js').read_bytes(),payload)
    def test_configuration_allowlist_requires_exact_private_guard(self):
        import install_protection_fixture as protection
        home=self.root/'guard-home';self.create_state(home)
        subject.assert_no_configuration(home)
        (home/'.agentdeck/install-protection/codex/extra').write_bytes(b'not allowed')
        with self.assertRaises(protection.Failure):subject.assert_no_configuration(home)
        linked=self.root/'linked-home';linked.mkdir(mode=0o700);(linked/'.agentdeck').mkdir(mode=0o700)
        (linked/'.agentdeck/install-protection').symlink_to(linked/'missing')
        with self.assertRaises(protection.Failure):subject.assert_no_configuration(linked)
    def test_offline_recovery_protocol_model_requires_each_confirmation(self):
        # Real authored npm/file transitions; modeled UI/backend, not IPC proof.
        self.install();home=self.root/'recovery-home';record=self.create_state(home)
        state=home/'.agentdeck/install-protection/codex/state.json'
        residue=subject.assert_wrong_root(self.f);before=subject.protection_snapshot(home,'failed_returned');queries=subject.query_counts(self.f)
        owner=self
        class Driver(OfflineDriver):
            plan=None;cleared=False;error=False
            def click(self,selector):
                self.clicks.append(selector)
                if selector==subject.RECOVERY_PREVIEW:
                    self.plan='blocked' if self.fixture['package'].exists() else 'eligible'
                elif selector==subject.RECOVERY_CANCEL:self.plan=None
                elif selector==subject.RECOVERY_CONFIRM:
                    self.plan=None
                    if self.fixture['bin'].joinpath('codex').exists():self.error=True
                    else:
                        self.cleared=True;self.error=False
                        state.write_text(json.dumps({'schema':1,'platform':'linux','phase':'clear','attempt':None,'prefix':None,'prefixIdentity':None}),encoding='utf-8')
                elif selector==subject.PREVIEW:self.dialog=True
                elif selector==subject.CONFIRM:
                    self.dialog=False;owner.install();state.write_text(json.dumps(record),encoding='utf-8');self.cleared=False
            def elements(self,selector):
                if selector.startswith(subject.RECOVERY_RESULT):
                    return [{}] if self.plan and (selector==subject.RECOVERY_RESULT or "@data-state='"+self.plan+"'" in selector) else []
                if selector==subject.RECOVERY_CONFIRM:return [{}] if self.plan=='eligible' else []
                return super().elements(selector)
            def text(self,selector):
                if selector.startswith(subject.RECOVERY_RESULT) and selector.endswith('/dd'):return 'missing'
                if selector==subject.RECOVERY_RESULT:return '已记录安装位置：'+str(self.fixture['prefix'])+' 确认只解除探测保护；之后刷新可恢复 npm/Codex 查询。不安装、不删除，也不代表包真实性或运行验证。'
                if selector=='//body' and self.error:return '恢复检查或确认未完成，保护状态未确认。请保留现有文件；不会自动重试。'
                if selector=="//*[@aria-label='Agent 列表']":return subject.WRONG_ROOT_GUARD
                return super().text(selector)
        driver=Driver(self.f,self.install);records=[]
        subject.exercise_recovery_after_restart(driver,self.f,home,records,lambda _:None,residue,before,queries)
        self.assertEqual(records,[{'stage':stage,'verified':True} for stage in subject.RECOVERY_STAGES])
        self.assertEqual(driver.clicks.count(subject.RECOVERY_CONFIRM),2)
        self.assertEqual(driver.clicks.count(subject.RECOVERY_CANCEL),2)
        self.assertEqual(driver.clicks.count(subject.CONFIRM),1)
        self.assertEqual(self.f['calls'].read_bytes(),b'ii');self.assertEqual(self.f['versions'].read_bytes(),b'')
    def test_synthetic_empty_slot_recovery_models_preserve_raw_records(self):
        import install_protection_fixture as protection
        self.install();source=self.root/'source-home';self.create_state(source)
        subject.relocate_authored_residue(self.f)
        original=protection.protection_snapshot(source)
        class Driver(OfflineDriver):
            plan=False
            def click(self,selector):
                self.clicks.append(selector)
                if selector==subject.RECOVERY_PREVIEW:self.plan=True
                elif selector==subject.RECOVERY_CANCEL:self.plan=False
            def elements(self,selector):
                if selector.startswith(subject.RECOVERY_RESULT):return [{}] if self.plan and self.case=='armed' else []
                if selector=="//*[@data-testid='codex-recovery-error']":return [{}] if self.plan and self.case=='corrupt' else []
                if selector==subject.RECOVERY_CONFIRM:return []
                return super().elements(selector)
            def text(self,selector):
                if selector=="//*[@data-testid='codex-recovery-error']":return '恢复检查或确认未完成，保护状态未确认。请保留现有文件；不会自动重试。'
                if selector=="//*[@aria-label='Agent 列表']":return subject.WRONG_ROOT_GUARD
                return super().text(selector)
        records=[]
        for case in ('armed','corrupt'):
            home=self.root/case;home.mkdir(mode=0o700)
            before=protection.prepare_synthetic_protection(source,home,case)
            driver=Driver(self.f,self.install);driver.case=case
            subject.exercise_synthetic_recovery_blocked(driver,self.f,home,case,records,lambda _:None,before,subject.query_counts(self.f))
            self.assertNotIn(subject.RECOVERY_CONFIRM,driver.clicks)
            self.assertEqual(protection.protection_snapshot(source),original)
        self.assertEqual(len(records),6)
        self.assertTrue(all(row['testAuthoredState'] and not row['crashRecoveryVerified'] for row in records))

    def test_native_restart_uses_same_home_path_and_stopped_synthetic_preparation(self):
        source=Path(harness.__file__).read_text(encoding='utf-8')
        self.assertIn('restart_for_home(original_wrong_home)',source)
        self.assertIn("if env['PATH']!=wrong_fixture['path']",source)
        method=source[source.index('        def restart_for_home'):source.index("        phase('durable_wrong_root_restart_same_home')")]
        self.assertLess(method.index('process.wait(timeout=5)'),method.index('if prepare is not None:prepare()'))
        self.assertLess(method.index('if prepare is not None:prepare()'),method.index('subprocess.Popen'))
        self.assertIn("for case in ('armed','corrupt')",source)

if __name__ == '__main__': unittest.main()
