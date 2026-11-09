"""Bounded offline harness regressions. These do NOT certify GUI/native IPC."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import linux_qoder_webview as harness
import linux_qwen_webview as subject


class OfflineDriver:
    """Test-only UI model for harness assertions, never used by acceptance."""
    def __init__(self, root, fault=None):
        self.root = root
        self.fault = fault
        self.values = {}
        self.dialog = False
        self.recovery = ''
        self.status = ''
        self.target = False
        self.preview_hash = None
        self.clicks = []

    def one(self, selector): return 'checkbox'
    def enabled(self, selector): return True
    def value(self, selector): return self.values.get(selector, '')
    def type(self, selector, value):
        self.values[selector] = value
        self.dialog = False
        self.status = ''; self.recovery = ''
    def command(self, method, path):
        if path.endswith('/checked'): return self.target
        return subject.CANARY if self.fault == 'canary' else '<html>redacted</html>'
    def elements(self, selector):
        if selector.startswith('//button'): return [{}]
        if "@role='status'" in selector: return [{}] if self.status else []
        if "@role='tab'" in selector: return [{}]
        if 'Vim 恢复状态观察' in selector: return [{}] if self.recovery else []
        if "@role='dialog'" in selector: return [{}] if self.dialog else []
        return []
    def text(self, selector):
        if 'Vim 恢复状态观察' in selector: return self.recovery
        if "@role='dialog'" in selector: return '原值：关闭；目标：启用。'
        return self.status
    def wait(self, predicate, **kwargs):
        if not predicate(): raise harness.Failure('ui_wait_expired')
    def click(self, selector):
        self.clicks.append(selector)
        settings = self.root / 'settings.json'
        receipt = self.root / subject.RECEIPT
        if "@role='tab'" in selector: return
        if "@type='checkbox'" in selector:
            self.target = not self.target
            return
        label = selector.split("text()='")[1].split("']")[0]
        self.status = ''; self.recovery = ''
        if label == '清除路径与结果':
            self.values.clear(); self.dialog = False; self.status = ''; self.recovery = ''
            if self.fault == 'clear_status': self.status = 'stale status'
            if self.fault == 'clear_recovery': self.recovery = 'stale recovery'
        elif label in ('取消 Vim 变更', '取消撤销'):
            self.dialog = False
            if self.fault == 'cancel_alert': self.status = subject.ERROR
            if self.fault == 'cancel_recovery': self.recovery = 'stale recovery'
        elif label == '预览 Vim 变更':
            self.dialog = True
            self.preview_hash = subject.digest(settings.read_bytes())
        elif label == '撤销受管 Vim 设置': self.dialog = True
        elif label == '确认应用 Vim 变更':
            self.dialog = False
            if self.preview_hash != subject.digest(settings.read_bytes()):
                self.status = subject.ERROR
                if self.fault == 'stale_write': settings.write_bytes(subject.original())
                return
            applied = json.dumps(subject.document(True)).encode()
            settings.write_bytes(applied)
            record = {'format': 1, 'phase': 'applied',
                      'selection_binding': subject.digest(os.fsencode(settings) + b'\0' + os.fsencode(self.root / 'package.json')),
                      'manifest_digest': subject.digest(subject.MANIFEST), 'restore_digest': None,
                      'before': False, 'after': True, 'had_general': True,
                      'before_digest': subject.digest(subject.original()), 'after_digest': subject.digest(applied)}
            harness.write_private(receipt, json.dumps(record).encode())
            harness.write_private(self.root / subject.LOCK, b'')
            if self.fault == 'apply_extra': harness.write_private(self.root / 'extra', b'')
            self.status = '所选文件的 Vim 模式字段已应用。尚未验证 Qwen 实际运行效果。'
        elif label == '确认撤销 Vim 设置':
            self.dialog = False
            if not json.loads(settings.read_bytes())['general']['vimMode']:
                self.status = subject.ERROR
                if self.fault == 'conflict_write': receipt.unlink()
                return
            settings.write_bytes(json.dumps(subject.document(False)).encode())
            if self.fault != 'restore_receipt': receipt.unlink()
            self.status = '受管字段已撤销。尚未验证 Qwen 实际运行效果。'
        elif label == '只读检查 Vim 恢复状态':
            if not receipt.exists(): self.recovery = '未发现所有权记录；不代表配置从未被修改。'
            elif settings.read_bytes() == subject.original(): self.recovery = '字段处于原状态，仅待清理所有权记录；尚未清理。'
            elif json.loads(settings.read_bytes())['general']['vimMode']: self.recovery = '观察到可撤销的受管字段；尚未执行撤销。'
            else: self.recovery = '受管字段存在冲突，当前观察不支持撤销。'
            if self.fault == 'recovery_write': settings.write_bytes(settings.read_bytes() + b' ')
        else: raise AssertionError('Unexpected model command')


class Tests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        harness.write_private(self.root / 'settings.json', subject.original())
        harness.write_private(self.root / 'package.json', subject.MANIFEST)

    def test_private_synthetic_contract_and_metadata(self):
        before = subject.audit(self.root, subject.BASE)
        self.assertEqual(subject.assert_document(self.root, False, subject.BASE), subject.original())
        self.assertEqual(before, subject.audit(self.root, subject.BASE))
        self.assertEqual(subject.CANARY, harness.CANARY)
        subject.rewrite_settings(self.root, subject.original() + b'\n', subject.BASE)
        self.assertNotEqual(before, subject.audit(self.root, subject.BASE))
        self.assertEqual(before['package.json'], subject.audit(self.root, subject.BASE)['package.json'])

    def test_exact_member_set(self):
        harness.write_private(self.root / 'unexpected', b'')
        with self.assertRaises(subject.QwenFailure): subject.audit(self.root, subject.BASE)
        (self.root / 'unexpected').unlink()
        (self.root / 'settings.json').unlink()
        with self.assertRaises(subject.QwenFailure): subject.audit(self.root, subject.BASE)

    def test_symlink_hardlink_fifo_and_directory_rejected(self):
        path = self.root / 'settings.json'
        for kind in ('symlink', 'hardlink', 'fifo', 'directory'):
            with self.subTest(kind=kind):
                path.unlink()
                if kind == 'symlink': path.symlink_to(self.root / 'package.json')
                elif kind == 'hardlink': os.link(self.root / 'package.json', path)
                elif kind == 'fifo': os.mkfifo(path, 0o600)
                else: path.mkdir(mode=0o700)
                with self.assertRaises(subject.QwenFailure): subject.audit(self.root, subject.BASE)
                if kind == 'directory': path.rmdir()
                else: path.unlink()
                harness.write_private(path, subject.original())

    def test_wrong_mode_owner_and_oversize_rejected(self):
        path = self.root / 'settings.json'
        path.chmod(0o644)
        with self.assertRaises(subject.QwenFailure): subject.audit(self.root, subject.BASE)
        path.chmod(0o600)
        with patch.object(subject.os, 'geteuid', return_value=os.geteuid() + 1):
            with self.assertRaises(subject.QwenFailure): subject.audit(self.root, subject.BASE)
        path.write_bytes(b'x' * 65537)
        with self.assertRaises(subject.QwenFailure): subject.audit(self.root, subject.BASE)
        path.write_bytes(subject.original())
        self.root.chmod(0o755)
        with self.assertRaises(subject.QwenFailure): subject.audit(self.root, subject.BASE)
        self.root.chmod(0o700)

    def test_manifest_and_lock_hashes_enforced(self):
        (self.root / 'package.json').write_bytes(subject.MANIFEST + b' ')
        with self.assertRaises(subject.QwenFailure): subject.audit(self.root, subject.BASE)
        (self.root / 'package.json').write_bytes(subject.MANIFEST)
        harness.write_private(self.root / subject.LOCK, b'not empty')
        with self.assertRaises(subject.QwenFailure): subject.audit(self.root, subject.BASE | {subject.LOCK})

    def test_unrelated_fields_and_boolean_type_enforced(self):
        for change in ('extra', 'token', 'numeric'):
            value = subject.document(True)
            if change == 'extra': value['extra'] = True
            elif change == 'token': value['security']['token'] = 'changed'
            else: value['general']['vimMode'] = 1
            subject.rewrite_settings(self.root, json.dumps(value).encode(), subject.BASE)
            with self.assertRaises(subject.QwenFailure): subject.assert_document(self.root, True, subject.BASE)

    def test_receipt_exact_fields_bindings_hashes_and_types(self):
        data = {'format': 1, 'phase': 'applied',
                'selection_binding': subject.digest(os.fsencode(self.root / 'settings.json') + b'\0' + os.fsencode(self.root / 'package.json')),
                'manifest_digest': subject.digest(subject.MANIFEST), 'restore_digest': None,
                'before': False, 'after': True, 'had_general': True,
                'before_digest': subject.digest(subject.original()), 'after_digest': subject.digest(b'applied')}
        subject.assert_receipt(self.root, json.dumps(data), b'applied')
        for key, value in [('extra', True), ('phase', 'prepared'), ('after_digest', '0' * 64),
                           ('before_digest', '0' * 64), ('selection_binding', '0' * 64),
                           ('manifest_digest', '0' * 64), ('before', 0), ('after', 1), ('format', True)]:
            with self.subTest(key=key):
                with self.assertRaises(subject.QwenFailure):
                    subject.assert_receipt(self.root, json.dumps({**data, key: value}), b'applied')

    def test_coverage_must_be_exact_order_all_verified(self):
        records = [{'stage': name, 'verified': True} for name in subject.STAGES]
        subject.assert_coverage(records)
        for invalid in ([], records[:-1], list(reversed(records)), records + records[:1],
                        [{**entry, 'verified': False} for entry in records]):
            with self.assertRaises(subject.QwenFailure): subject.assert_coverage(invalid)

    def test_driver_error_not_replayed_and_no_stage_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            driver = Mock()
            driver.elements.return_value = [{}]
            driver.wait.side_effect = lambda predicate, **_: predicate()
            driver.click.side_effect = harness.Failure('webdriver_command_failed')
            records = []
            with self.assertRaises(harness.Failure):
                subject.exercise(driver, Path(tmp), records, harness.write_private)
            driver.click.assert_called_once()
            self.assertEqual(records, [])

    def test_offline_lifecycle_model_and_injected_failures(self):
        for fault in (None, 'canary', 'stale_write', 'apply_extra', 'conflict_write',
                      'restore_receipt', 'recovery_write'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                driver = OfflineDriver(root, fault)
                records = []
                if fault is None:
                    subject.exercise(driver, root, records, harness.write_private)
                    subject.assert_coverage(records)
                    subject.assert_document(root, False, subject.BASE | {subject.LOCK})
                    self.assertEqual(driver.values, {})
                else:
                    with self.assertRaises(subject.QwenFailure):
                        subject.exercise(driver, root, records, harness.write_private)
                    with self.assertRaises(subject.QwenFailure): subject.assert_coverage(records)
                    self.assertLess(len(records), len(subject.STAGES))

    def test_clear_and_cancel_reject_stale_status_alert_or_recovery(self):
        for fault in ('clear_status', 'clear_recovery', 'cancel_alert', 'cancel_recovery'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as tmp:
                records = []
                driver = OfflineDriver(Path(tmp), fault)
                with self.assertRaises(harness.Failure):
                    subject.exercise(driver, Path(tmp), records, harness.write_private)
                self.assertEqual([entry['stage'] for entry in records], ['recovery_no_receipt'])
                with self.assertRaises(subject.QwenFailure): subject.assert_coverage(records)

    def test_delayed_buttons_poll_before_enabled_and_click_once(self):
        class DelayedDriver(OfflineDriver):
            def __init__(self, root):
                super().__init__(root)
                self.observed = {}
            def elements(self, selector):
                if selector.startswith('//button'):
                    self.observed[selector] = self.observed.get(selector, 0) + 1
                    if self.observed[selector] == 1: return []
                return super().elements(selector)
            def enabled(self, selector):
                if selector.startswith('//button') and self.observed.get(selector, 0) < 2:
                    raise AssertionError('Enabled queried before unique control appeared')
                return True
            def wait(self, predicate, **kwargs):
                for _ in range(3):
                    if predicate(): return
                raise harness.Failure('ui_wait_expired')
        with tempfile.TemporaryDirectory() as tmp:
            driver = DelayedDriver(Path(tmp))
            records = []
            subject.exercise(driver, Path(tmp), records, harness.write_private)
            subject.assert_coverage(records)
            self.assertEqual(driver.clicks.count("//button[text()='确认应用 Vim 变更']"), 2)
            self.assertEqual(driver.clicks.count("//button[text()='确认撤销 Vim 设置']"), 2)

    def test_ambiguous_button_fails_without_click(self):
        class AmbiguousDriver(OfflineDriver):
            def elements(self, selector):
                return [{}, {}] if selector.startswith('//button') else super().elements(selector)
        with tempfile.TemporaryDirectory() as tmp:
            driver = AmbiguousDriver(Path(tmp))
            records = []
            with self.assertRaises(subject.QwenFailure):
                subject.exercise(driver, Path(tmp), records, harness.write_private)
            self.assertEqual(records, [])
            self.assertFalse(any(click.startswith('//button') for click in driver.clicks))

    def test_unknown_failure_detail_not_retained(self):
        self.assertEqual(harness.failure_code(subject.QwenFailure(subject.CANARY)), 'qwen_gui_acceptance_failed')

    def test_failure_dom_suppresses_shared_canary_before_truncation(self):
        driver = Mock(session='session', expected_input='')
        driver.enabled.return_value = False
        driver.value.return_value = ''
        driver.text.return_value = 'x' * 65536 + subject.CANARY
        driver.command.return_value = 'x' * 65536 + subject.CANARY
        with tempfile.TemporaryDirectory() as tmp:
            harness.capture_failure_dom(driver, Path(tmp))
            report = json.loads((Path(tmp) / 'failure-dom.json').read_text())
            self.assertEqual(report['body']['state'], 'private_content_suppressed')
            self.assertEqual(report['source']['state'], 'private_content_suppressed')
            self.assertNotIn(subject.CANARY, json.dumps(report))


if __name__ == '__main__':
    unittest.main()
