"""Offline authored harness regressions, NOT real WebView or Qoder acceptance."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import linux_qoder_deploy_webview as subject
import qoder_private_deploy_acceptance as deployment
import linux_qoder_webview as main

PAYLOAD = b'authored-harness-only-not-qoder' * 200


class Driver:
    def __init__(self, source, fault=None):
        self.source = source
        self.fault = fault
        self.dialog = False
        self.preview_error = False
        self.outcome = None
        self.inputs = {subject.SOURCE: '', subject.DESTINATION: ''}
        self.calls = []
        self.selected = None
        self.result_reads = 0
        self.inspection = None
    def elements(self, selector):
        if selector.startswith(subject.INSPECT_RESULT):
            return [1] if self.inspection and (selector == subject.INSPECT_RESULT or self.inspection in selector) else []
        if selector == subject.DIALOG: return [1] if self.dialog else []
        if selector == subject.RESULT: return [1] if self.outcome == 'complete' else []
        if selector == subject.ALERT: return [1] if self.preview_error else []
        if selector == subject.FAILED: return [1] if self.outcome == 'failed' else []
        return [1]
    def enabled(self, selector): return True
    def one(self, selector):
        if selector == subject.INSPECT_BUTTON: return 'inspect_deployed'
        if selector == subject.INSPECT_CLEAR: return 'clear_inspection'
        for name, label in subject.BUTTONS.items():
            if label in selector: return name
        return 'leave' if 'Agent 管理' in selector else 'return'
    def wait(self, predicate, seconds=20):
        if not any(predicate() for _ in range(3)): raise main.Failure('ui_wait_expired')
    def value(self, selector): return self.inputs[selector]
    def type(self, selector, value):
        self.inputs[selector] = value
        self.inspection = None
        if self.fault != 'invalidation': self.dialog = False
        self.preview_error = False
    def text(self, selector):
        if selector == subject.RESULT:
            self.result_reads += 1
            if self.fault == 'pending_first' and self.result_reads == 1: return 'Qoder 私有部署正在进行'
            return subject.SUCCESS
        if selector == subject.DIALOG:
            target = self.inputs[subject.DESTINATION]
            if self.fault == 'wrong_details': target = '/incorrect'
            return '1.1.85 linux-amd64-baseline 0700 ' + str(self.source) + ' ' + target
        return 'fixed local text'
    def command(self, method, path, data=None):
        self.calls.append(path)
        name = path.split('/')[2]
        if self.fault == 'transport': raise ConnectionError('no replay')
        destination = Path(self.inputs[subject.DESTINATION])
        if name == 'inspect_deployed':
            self.inspection = 'payload_verified' if Path(self.inputs[subject.INSPECT_PATH]).name == 'confirmed' else 'unsupported_or_changed'
        elif name == 'clear_inspection':
            self.inspection = None; self.inputs[subject.INSPECT_PATH] = ''
        elif name == 'preview':
            if os.path.lexists(destination): self.preview_error = True
            else:
                self.dialog = True
                self.selected = destination
                if self.fault == 'preview_write': destination.mkdir(mode=0o700)
        elif name == 'cancel':
            if self.fault != 'cancel': self.dialog = False
        elif name == 'confirm':
            self.dialog = False
            if os.path.lexists(self.selected):
                self.outcome = 'failed'
                if self.fault == 'stale_overwrite': (self.selected / 'sentinel').write_bytes(b'changed')
            else:
                self.selected.mkdir(mode=0o700)
                deployment.private_file(self.selected / 'qodercli', PAYLOAD)
                (self.selected / 'qodercli').chmod(0o700)
                deployment.private_file(self.selected / 'deployment.intent.json', json.dumps({
                    'format': 1, 'state': 'private_deployment_started', 'version': '1.1.85',
                    'target': 'linux-amd64-baseline', 'execution_verified': False, 'activated': False}).encode())
                deployment.private_file(self.selected / 'deployment.complete.json', json.dumps({
                    **deployment.OUTCOME, 'format': 1, 'payload_bytes': len(PAYLOAD),
                    'payload_sha256': hashlib.sha256(PAYLOAD).hexdigest(), 'limitations': 'test only'}).encode())
                self.outcome = 'complete'
                if self.fault == 'bad_payload': (self.selected / 'qodercli').write_bytes(b'bad')
                if self.fault == 'bad_mode': (self.selected / 'qodercli').chmod(0o600)
                if self.fault == 'extra_file': deployment.private_file(self.selected / 'extra', b'unexpected')
                if self.fault == 'source_changed': (self.source / 'qodercli').write_bytes(b'changed')
        elif name == 'clear':
            self.inputs = {subject.SOURCE: '', subject.DESTINATION: ''}
            self.dialog = False; self.preview_error = False
            if self.fault != 'clear': self.outcome = None
        elif name == 'leave':
            if self.fault != 'navigation': self.dialog = False


class Tests(unittest.TestCase):
    def run_fixture(self, fault=None):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            source = base / 'source'; source.mkdir(mode=0o700)
            deployment.private_file(source / 'qodercli', PAYLOAD)
            config = base / 'config'; config.mkdir(mode=0o700)
            deployment.private_file(config / 'sentinel', b'unchanged configuration')
            root = base / 'deployment'; root.mkdir(mode=0o700)
            driver = Driver(source, fault); records = []
            with patch.object(subject.complete, 'PAYLOAD_BYTES', len(PAYLOAD)), patch.object(subject.complete, 'PAYLOAD_SHA256', hashlib.sha256(PAYLOAD).hexdigest()):
                subject.exercise(driver, source, root, config, records)
            subject.assert_coverage(records)
            self.assertFalse((root / 'cancelled').exists())
            self.assertFalse((root / 'changed').exists())
            self.assertFalse((root / 'not-submitted').exists())
            self.assertEqual(len([p for p in driver.calls if '/confirm/' in p]), 2)
            return driver, records

    def test_full_authored_offline_flow(self):
        self.run_fixture()

    def test_pending_status_does_not_complete_deployment(self):
        self.run_fixture('pending_first')

    def test_all_faults_fail_without_complete_coverage(self):
        for fault in ('wrong_details', 'preview_write', 'cancel', 'invalidation',
                      'stale_overwrite', 'bad_payload', 'bad_mode', 'extra_file',
                      'source_changed', 'clear', 'navigation', 'transport'):
            with self.subTest(fault=fault):
                with self.assertRaises((subject.DeploymentGuiFailure, deployment.Failure, main.Failure, ConnectionError)):
                    self.run_fixture(fault)

    def test_strict_order_and_boolean_coverage(self):
        records = [{'stage': name, 'verified': True} for name in subject.STAGES]
        subject.assert_coverage(records)
        for bad in (records[:-1], records[::-1], records + records[:1], [], [{'stage': name, 'verified': 1} for name in subject.STAGES]):
            with self.assertRaises(subject.DeploymentGuiFailure): subject.assert_coverage(bad)

    def test_native_activation_is_one_shot_and_not_a_click(self):
        driver = Mock(); driver.elements.return_value = [1]; driver.enabled.return_value = True
        driver.one.return_value = 'native-button'
        driver.wait.side_effect = lambda fn, **kwargs: fn()
        subject.activate(driver, 'selector')
        driver.command.assert_called_once_with('POST', '/element/native-button/value', {'text': '\ue007', 'value': ['\ue007']})
        driver.click.assert_not_called()
        driver.command.reset_mock(); driver.command.side_effect = ConnectionError()
        with self.assertRaises(ConnectionError): subject.activate(driver, 'selector')
        driver.command.assert_called_once()
        driver.elements.return_value = [1, 2]; driver.command.reset_mock()
        with self.assertRaises(subject.DeploymentGuiFailure): subject.activate(driver, 'selector')
        driver.command.assert_not_called()

    def test_main_lane_requires_full_evidence_and_separate_trace_scope(self):
        source = Path(main.__file__).read_text()
        self.assertIn('qoder_deploy.assert_coverage(deployment_records)', source)
        self.assertLess(source.index('qoder_deploy.assert_coverage(deployment_records)'), source.index("phase('acceptance_complete')"))
        self.assertIn("'qoder_deployment_gui_verified': passed and", source)
        self.assertIn('GUI process execution is not traced', source)
        workflow = (Path(main.__file__).parent.parent / '.github/workflows/ci.yml').read_text()
        self.assertIn('scripts/test_linux_qoder_deploy_webview.py', workflow)
        self.assertIn('scripts/qoder_private_deploy_acceptance.py', workflow)
        self.assertIn('strace', workflow)

    def test_failure_category_is_fixed(self):
        self.assertEqual(main.failure_code(subject.DeploymentGuiFailure('/private/SECRET')), 'qoder_private_deployment_gui_failed')


if __name__ == '__main__': unittest.main()
