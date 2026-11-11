"""Offline authoring/verifier guards only; not native application acceptance."""
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest
import memory_export_fixture as f


class Tests(unittest.TestCase):
    def test_fixture_population_union_order_and_search_bound(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp); f.prepare(home); before = f.snapshot(home)
            f.verify(home, 'initial'); self.assertEqual(before, f.snapshot(home))
            with f.connect_readonly(home) as c:
                recent = c.execute('SELECT id FROM memories ORDER BY collected_at DESC LIMIT 50').fetchall()
                self.assertEqual({r[0] for r in recent}, set(range(17, 67)))
                self.assertNotIn((1,), recent); self.assertNotIn((2,), recent)
                self.assertEqual(c.execute("SELECT rowid FROM memories_fts WHERE memories_fts MATCH 'Harbor'").fetchall(), [(4,)])
            c.close()
            self.assertEqual(len(f.rows()), 66)
            expected = f.markdown('initial', 'codex').decode()
            self.assertEqual(expected.count('Birch'), 1)
            self.assertLess(expected.index('Cedar'), expected.index('Birch'))  # tied timestamps, ID DESC
            self.assertNotIn(f.CANARY, expected)
            with self.assertRaises(f.FixtureFailure): f.prepare(home)

    def test_external_mutations_never_export_or_change_other_targets(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp); f.prepare(home)
            before = {t: (home / '.agentdeck/memory' / (t + '.md')).read_bytes() for t in f.TARGETS}
            for kind in ('manual', 'auto', 'row'):
                f.mutate(home, kind); f.verify(home, kind + '-drift')
                self.assertEqual(before, {t: (home / '.agentdeck/memory' / (t + '.md')).read_bytes() for t in f.TARGETS})
            with self.assertRaises(f.FixtureFailure): f.mutate(home, 'manual')

    def test_readonly_wal_coordination_does_not_hide_data_or_change_markers(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp); f.prepare(home); before = f.snapshot(home)
            db = home / '.agentdeck/memory.db'
            with sqlite3.connect(db.as_uri() + '?mode=ro', uri=True) as c:
                self.assertEqual(c.execute('SELECT COUNT(*) FROM memories').fetchone(), (66,))
            c.close()
            self.assertEqual(db.with_name('memory.db-wal').stat().st_size, 0)
            self.assertEqual(db.with_name('memory.db-shm').stat().st_size, 32768)
            self.assertEqual(before, f.snapshot(home)); f.verify(home, 'initial')
            # Immutable reads must never ignore pending/committed WAL content.
            db.with_name('memory.db-wal').write_bytes(b'nonempty')
            with self.assertRaises(f.FixtureFailure): f.connect_readonly(home)

    def test_unsafe_paths_and_coordination_fail_closed(self):
        cases = ['wal_content', 'shm_size', 'journal', 'symlink', 'hardlink', 'oversize']
        if hasattr(os, 'mkfifo'): cases.append('fifo')
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory() as temp:
                home = Path(temp); f.prepare(home); root = home / '.agentdeck'
                if case == 'wal_content': (root / 'memory.db-wal').write_bytes(b'x')
                elif case == 'shm_size': (root / 'memory.db-shm').write_bytes(b'x')
                elif case == 'journal': (root / 'memory.db-journal').write_bytes(b'')
                else:
                    path = root / 'memory/codex.md'; path.unlink()
                    if case == 'symlink': path.symlink_to(root / 'memory-fixture-private.txt')
                    elif case == 'hardlink': os.link(root / 'memory-fixture-private.txt', path)
                    elif case == 'oversize': path.write_bytes(b'x' * 2_097_153)
                    else: os.mkfifo(path)
                with self.assertRaises(f.FixtureFailure): f.snapshot(home)

    def test_verifier_catches_export_rows_flags_targets_collection_and_canary_drift(self):
        for kind in ('export', 'row', 'auto', 'other_target', 'collection', 'canary'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temp:
                home = Path(temp); f.prepare(home); root = home / '.agentdeck'
                if kind in ('row', 'auto', 'other_target'):
                    with sqlite3.connect(root / 'memory.db') as c:
                        c.execute({'row': 'DELETE FROM memories WHERE id=60', 'auto': 'UPDATE memories SET auto_inject=0 WHERE id=2', 'other_target': "DELETE FROM manual_injections WHERE agent_id='opencode'"}[kind])
                    c.close()
                else:
                    path = {'export': 'memory/codex.md', 'collection': 'memory-collection.json', 'canary': 'memory-fixture-private.txt'}[kind]
                    (root / path).write_bytes(b'wrong')
                with self.assertRaises(f.FixtureFailure): f.verify(home, 'initial')

    def test_cli_refuses_ordinary_home_and_redacts_error(self):
        with tempfile.TemporaryDirectory() as temp:
            home = Path(temp); env = dict(os.environ, GITHUB_ACTIONS='false')
            result = subprocess.run([sys.executable, '-B', f.__file__, '--home', str(home), '--prepare'], env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, '')
            self.assertEqual(result.stderr, 'Authored memory fixture guard failed.\n')
            self.assertEqual(list(home.iterdir()), [])


if __name__ == '__main__': unittest.main()
