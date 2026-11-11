"""Offline UI/filesystem model: never claims native IPC/GUI execution."""
from pathlib import Path
import re
import sqlite3
import tempfile
import unittest
import linux_memory_export_webview as subject
import linux_qoder_webview as harness
import memory_export_fixture as f


class Driver:
    def __init__(self, home, fault=None):
        self.home = home; self.fault = fault; self.target = 'codex'; self.saved = None
        self.draft = []; self.query = ''; self.results = []; self.preview = None
        self.error = False; self.status = False; self.calls = []
    def elements(self, selector):
        if selector == subject.control('load') and self.fault == 'ambiguous': return [1, 2]
        if selector == subject.control('saved'): return [1] if self.saved is not None else []
        if selector == subject.control('unread'): return [1] if self.saved is None else []
        if selector in (subject.control('preview-result'), subject.control('confirm')): return [1] if self.preview else []
        if selector == subject.control('error'): return [1] if self.error else []
        if selector == subject.control('status'): return [1] if self.status else []
        if selector.startswith(subject.control('draft') + '/li'):
            match = re.search("data-memory-id='([0-9]+)'", selector)
            return [1] * (int(match[1]) in self.draft) if match else list(self.draft)
        if "starts-with(@data-testid,'memory-select-')" in selector: return list(self.results)
        match = re.search("data-testid='memory-select-([0-9]+)'", selector)
        if match: return [1] if int(match[1]) in self.results else []
        return [1]
    def enabled(self, selector):
        if selector == subject.control('preview'): return self.saved is not None
        return True
    def one(self, selector):
        if len(self.elements(selector)) != 1: raise harness.Failure('missing_or_ambiguous_control')
        return selector
    def wait(self, predicate, **_):
        if not predicate(): raise harness.Failure('ui_wait_expired')
    def type(self, selector, text): self.query = text
    def value(self, selector): return self.target if 'export-target' in selector else self.query
    def click(self, selector):
        self.target = re.search("@value='([^']+)'", selector)[1]
        self.saved = None; self.draft = []; self.preview = None
    def text(self, selector):
        if self.fault == 'canary': return f.CANARY
        if selector == subject.control('error'): return '错误' if self.fault == 'wrong_error' else '已变化'
        if selector == subject.control('status'): return '不代表 Agent 已读取'
        if selector == subject.control('preview-result'):
            with f.connect_readonly(self.home) as c:
                auto = {r[0] for r in c.execute('SELECT id FROM memories WHERE agent_id=? AND auto_inject=1',(self.target,))}
            c.close()
            return f'手动 {len(self.draft)} 条 + 自动 {len(auto)} 条，去重后 {len(set(self.draft) | auto)} 条。'
        return 'Authored visible content'
    def command(self, method, path, data=None):
        if path == '/source': return self.text('//body')
        selector = path[len('/element/'):].rsplit('/', 1)[0]
        if path.endswith('/selected'):
            match = re.search("memory-select-([0-9]+)", selector)
            return int(match[1]) in self.draft if match else self.fault == 'collection'
        self.calls.append(selector)
        if self.fault == 'transport': raise harness.Failure('webdriver_command_failed')
        if selector == subject.control('load'):
            with f.connect_readonly(self.home) as c:
                self.saved = [r[0] for r in c.execute('SELECT memory_id FROM manual_injections WHERE agent_id=?',(self.target,))]
            c.close(); self.draft = list(self.saved); self.preview = None; self.error = False; self.status = False
            if self.fault == 'unrelated': (self.home / '.agentdeck/unrelated').write_bytes(b'wrong')
        elif selector == subject.control('reset'): self.draft = list(self.saved); self.preview = None
        elif selector == subject.control('cancel'): self.saved = None; self.draft = []; self.preview = None
        elif selector == subject.control('clear'): self.draft = []; self.preview = None
        elif selector == subject.control('preview-cancel'):
            if self.fault != 'cancel': self.preview = None
        elif selector == subject.control('preview'):
            self.preview = f.snapshot(self.home)
            if self.fault == 'preview_write': (self.home / '.agentdeck/memory/codex.md').write_bytes(b'changed')
        elif selector == subject.control('confirm'):
            stale = self.preview != f.snapshot(self.home)
            if stale and self.fault != 'stale_write': self.error = True
            else:
                with sqlite3.connect(self.home / '.agentdeck/memory.db') as c:
                    c.execute('DELETE FROM manual_injections WHERE agent_id=?',(self.target,))
                    c.executemany('INSERT INTO manual_injections VALUES (?,?)',[(self.target,i) for i in self.draft])
                    if self.fault == 'other_target': c.execute("DELETE FROM manual_injections WHERE agent_id='opencode'")
                    if self.fault == 'auto_flag': c.execute('UPDATE memories SET auto_inject=0 WHERE id=2')
                c.close()
                state = 'replaced' if self.draft else 'cleared'
                output = b'wrong' if self.fault == 'union' or (self.fault == 'clear' and not self.draft) else f.markdown(state, self.target)
                (self.home / '.agentdeck/memory' / (self.target + '.md')).write_bytes(output)
                self.status = True
            self.preview = None; self.saved = None
        elif '移除手动记忆 ' in selector:
            i = int(re.search(r'移除手动记忆 ([0-9]+)',selector)[1]); self.draft.remove(i)
        elif "data-testid='memory-select-" in selector:
            self.draft.append(int(re.search(r'memory-select-([0-9]+)',selector)[1]))
        elif selector == "//button[text()='搜索']":
            with f.connect_readonly(self.home) as c:
                self.results = [r[0] for r in c.execute('SELECT id FROM memories WHERE content LIKE ? ORDER BY collected_at DESC LIMIT 50', ('%' + self.query + '%',))]
            c.close()
            if self.fault == 'selection_lost': self.draft = list(self.saved)


class Tests(unittest.TestCase):
    def test_full_model_and_faults(self):
        for fault in (None, 'canary', 'collection', 'unrelated', 'cancel', 'preview_write', 'wrong_error', 'stale_write', 'other_target', 'auto_flag', 'union', 'clear', 'selection_lost'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as temp:
                home = Path(temp); records = []; driver = Driver(home, fault)
                if fault is None:
                    subject.exercise(driver, home, records)
                    subject.assert_coverage(records); f.verify(home, 'cleared')
                    self.assertEqual(driver.calls.count(subject.control('confirm')), 5)
                else:
                    with self.assertRaises((subject.MemoryGuiFailure, f.FixtureFailure, harness.Failure)):
                        subject.exercise(driver, home, records)
                    self.assertLess(len(records), len(subject.STAGES))

    def test_prior_install_sentinel_and_queries_remain_quiet(self):
        for fault in (None, 'process', 'query'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as temp:
                root = Path(temp); home = root / 'home'; home.mkdir()
                package = root / 'package'; (package / 'bin').mkdir(parents=True)
                (package / 'bin/codex.js').write_bytes(b'authored sentinel')
                queries = root / 'queries'; queries.write_bytes(b'pr')
                fixture = {'root': root, 'package': package, 'queries': queries}
                class ProcessDriver(Driver):
                    def command(self, method, path, data=None):
                        result = super().command(method, path, data)
                        if method == 'POST' and path.endswith('/value') and fault:
                            target = root / 'codex-config-process-marker' if fault == 'process' else queries
                            target.write_bytes(b'unexpected')
                        return result
                driver = ProcessDriver(home)
                if fault is None:
                    subject.exercise(driver, home, [], process_fixture=fixture)
                else:
                    with self.assertRaises(subject.MemoryGuiFailure):
                        subject.exercise(driver, home, [], process_fixture=fixture)

    def test_ambiguous_and_uncertain_actions_never_replayed(self):
        for fault in ('ambiguous', 'transport'):
            with tempfile.TemporaryDirectory() as temp:
                home = Path(temp); driver = Driver(home, fault)
                with self.assertRaises((subject.MemoryGuiFailure, harness.Failure)):
                    subject.exercise(driver, home, [])
                self.assertEqual(len(driver.calls), 1)  # tab only; transport stops at tab

    def test_coverage_order_redaction_and_real_runner_wiring(self):
        records = [{'stage': name, 'verified': True} for name in subject.STAGES]
        subject.assert_coverage(records)
        for bad in ([], records[:-1], records[::-1], records + records[:1]):
            with self.assertRaises(subject.MemoryGuiFailure): subject.assert_coverage(bad)
        for error in (subject.MemoryGuiFailure(f.CANARY), f.FixtureFailure(f.CANARY)):
            self.assertEqual(harness.failure_code(error), 'memory_export_gui_acceptance_failed')
        source = Path(harness.__file__).read_text()
        self.assertLess(source.index('codex_config.assert_animation_coverage(codex_animation_records)'), source.index('memory_export.exercise('))
        self.assertIn("'memory_export_checks': memory_export_records", source)
        self.assertIn('any(canary in value for canary in memory_export.CANARIES)', source)
        self.assertIn('any(canary.encode() in retained for canary in memory_export.CANARIES)', source)
        own = Path(subject.__file__).read_text()
        for forbidden in ('/execute', 'invoke(', 'collect_memory', 'set_memory_collection_enabled'):
            self.assertNotIn(forbidden, own)


if __name__ == '__main__': unittest.main()
