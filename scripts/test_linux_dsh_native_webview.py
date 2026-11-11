"""Offline authored harness tests, NOT native UI/IPC acceptance."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import linux_dsh_native_webview as subject


class Driver:
    def __init__(self, items, paths, fault=None):
        self.items, self.paths, self.fault = items, paths, fault
        self.inputs = {subject.PACKAGE: '', subject.ARCHIVE: ''}
        self.result = None; self.alert = False
    def enabled(self, selector): return True
    def elements(self, selector):
        if selector == subject.RESULT: return [1] if self.result else []
        if selector == subject.ALERT: return [1] if self.alert else []
        return [1]
    def wait(self, predicate):
        if not predicate(): raise AssertionError('wait failed')
    def value(self, selector): return self.inputs[selector]
    def invalidate(self):
        if self.fault != 'stale': self.result = None
        self.alert = False
    def type(self, selector, value):
        self.inputs[selector] = value; self.invalidate()
    def click(self, selector):
        if '/option' in selector:
            self.inputs[subject.PACKAGE] = selector.split("@value='")[1].split("'")[0]
            self.inputs[subject.ARCHIVE] = ''; self.invalidate()
        elif selector == subject.CLEAR:
            if self.fault == 'clear': return
            self.inputs = {subject.PACKAGE: '', subject.ARCHIVE: ''}; self.invalidate()
        elif selector == subject.VERIFY:
            package = self.inputs[subject.PACKAGE]
            path = Path(self.inputs[subject.ARCHIVE])
            self.result = next(a for a in self.items if a['id'] == package) if path == self.paths[package][0] or self.fault == 'accept_bad' else None
            self.alert = self.result is None
            if self.fault == 'write': path.write_bytes(b'changed')
    def text(self, selector):
        a = self.result
        values = [a['id'], a['package'], a['version'], a['archive_name'], str(a['archive_bytes']), a['archive_sha256'], a['integrity'], '普通成员数：' + str(len(a['members'])), '所选原生依赖归档静态校验通过。', '依赖闭包未完成、依赖未解析、不可据此安装、未执行生命周期脚本、未加载原生代码、未验证运行或独立签名，平台兼容性未验证。']
        return ' '.join(values if self.fault != 'metadata' else values[:2])


class Tests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name); self.source = self.base / 'source'; self.source.mkdir()
        self.root = self.base / 'fixtures'; self.data = b'authored data only'
        self.items = [dict(id=p, package=p, version='fixture', archive_name=p+'.tgz', archive_bytes=len(self.data), archive_sha256=hashlib.sha256(self.data).hexdigest(), integrity='sha512-fixture', members=['one']) for p in subject.pinned.IDS]
        for a in self.items: subject.pinned.write_new(self.source / a['archive_name'], self.data)
        p = patch.object(subject.pinned, 'load_review', return_value=self.items); p.start(); self.addCleanup(p.stop)
        p = patch.object(subject.pinned, 'validate_archive', side_effect=lambda data, package: self.assertEqual(data, self.data)); p.start(); self.addCleanup(p.stop)
    def test_full_matrix(self):
        before = subject.pinned.snapshot(self.source)
        paths = subject.prepare(self.source, self.root)
        for good, bad in paths.values():
            self.assertEqual(good.stat().st_size, bad.stat().st_size)
            self.assertNotEqual(good.read_bytes(), bad.read_bytes())
        records = []; subject.exercise(Driver(self.items, paths), self.source, self.root, paths, records)
        subject.assert_coverage(records)
        self.assertEqual(len(records), 24)
        self.assertEqual(subject.pinned.snapshot(self.source), before)
    def test_faults_fail_closed(self):
        paths = subject.prepare(self.source, self.root)
        for fault in ('stale', 'clear', 'accept_bad', 'metadata', 'write'):
            with self.subTest(fault=fault), self.assertRaises((AssertionError, subject.NativeGuiFailure)):
                subject.exercise(Driver(self.items, paths, fault), self.source, self.root, paths, [])
    def test_unexpected_source_rejected(self):
        (self.source / 'extra').write_bytes(b'bad')
        with self.assertRaises(subject.NativeGuiFailure): subject.prepare(self.source, self.root)
    def test_symlink_source_rejected(self):
        path = self.source / self.items[0]['archive_name']; path.unlink(); path.symlink_to(self.source / self.items[1]['archive_name'])
        with self.assertRaises(subject.pinned.Failure): subject.prepare(self.source, self.root)
    def test_incomplete_coverage_rejected(self):
        with self.assertRaises(subject.NativeGuiFailure): subject.assert_coverage([])


if __name__ == '__main__': unittest.main()
