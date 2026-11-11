#!/usr/bin/env python3
"""Authored disposable SQLite fixtures, never session collection or agent execution.

CLI is restricted to the installed-app hosted CI home. Offline unit tests call
functions with their own TemporaryDirectory. No content or private paths leave
this module's CLI: output is a fixed success object or a fixed error code.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import stat

CANARY = 'SYNTHETIC-NOT-A-CREDENTIAL'
SCHEMA = '''
CREATE TABLE memories (id INTEGER PRIMARY KEY AUTOINCREMENT, agent_id TEXT NOT NULL,
content TEXT NOT NULL, collected_at INTEGER NOT NULL, source TEXT,
auto_inject INTEGER NOT NULL DEFAULT 0, tags TEXT NOT NULL DEFAULT '');
CREATE TABLE manual_injections (agent_id TEXT NOT NULL, memory_id INTEGER NOT NULL,
PRIMARY KEY (agent_id, memory_id));
CREATE UNIQUE INDEX idx_memories_dedup ON memories(source, content);
CREATE VIRTUAL TABLE memories_fts USING fts5(content, tags, content='memories', content_rowid='id');
CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
INSERT INTO memories_fts(rowid, content, tags) VALUES (new.id, new.content, new.tags); END;
CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
INSERT INTO memories_fts(memories_fts, rowid, content, tags) VALUES ('delete', old.id, old.content, old.tags); END;
CREATE TRIGGER memories_au AFTER UPDATE ON memories BEGIN
INSERT INTO memories_fts(memories_fts, rowid, content, tags) VALUES ('delete', old.id, old.content, old.tags);
INSERT INTO memories_fts(rowid, content, tags) VALUES (new.id, new.content, new.tags); END;
'''
TARGETS = ('codex', 'claude-code', 'opencode')
STATES = ('initial', 'manual-drift', 'auto-drift', 'row-drift', 'replaced', 'cleared')


class FixtureFailure(Exception):
    pass


def safe_path(path, directory=False):
    for part in (path, *path.parents):
        m = part.lstat()
        if stat.S_ISLNK(m.st_mode) or getattr(m, 'st_file_attributes', 0) & 1024:
            raise FixtureFailure('unsafe_fixture')
    m = path.lstat()
    if not (stat.S_ISDIR(m.st_mode) if directory else stat.S_ISREG(m.st_mode)):
        raise FixtureFailure('unsafe_fixture')
    if hasattr(os, 'geteuid') and m.st_uid != os.geteuid():
        raise FixtureFailure('unsafe_fixture_owner')
    if not directory and (m.st_nlink != 1 or m.st_size > 2_097_152):
        raise FixtureFailure('unsafe_fixture')
    return m


def private_write(path, data):
    safe_path(path.parent, directory=True)
    with os.fdopen(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), 'wb') as f:
        f.write(data)


def rows(state='initial'):
    result = []
    labels = {1: ('claude-code', 'Orchard saved outside recent results'),
              2: ('codex', 'Birch saved and automatic overlap'),
              3: ('codex', 'Cedar automatic retained'),
              4: ('opencode', 'Harbor cross source addition'),
              5: ('claude-code', 'Meadow cross source addition'),
              6: ('opencode', 'Lagoon other target selection'),
              7: ('codex', 'Juniper authored automatic drift')}
    for i in range(1, 67):
        source, text = labels.get(i, ('opencode', f'Authored filler record {i:03d}'))
        if i == 4 and state in ('row-drift', 'replaced', 'cleared'): text += ' revised'
        automatic = i in (2, 3) or (i == 7 and state in ('auto-drift', 'row-drift', 'replaced', 'cleared'))
        result.append((i, source, text, 1700000000 + (2 if i == 3 else i),
                       f'authored-fixture-{i}-{CANARY}', int(automatic), 'authored'))
    return result


def manual(state='initial'):
    ids = [1, 2] if state == 'initial' else [2, 4, 5] if state == 'replaced' else [] if state == 'cleared' else [1, 2, 6]
    return sorted([('codex', i) for i in ids] + [('claude-code', 6), ('opencode', 5)])


def markdown(state, target):
    selected = {i for agent, i in manual(state) if agent == target}
    selected.update(r[0] for r in rows(state) if r[1] == target and r[5])
    entries = sorted((r for r in rows(state) if r[0] in selected), key=lambda r: (r[3], r[0]), reverse=True)
    text = '# AgentDeck 记忆注入\n\n> 由 AgentDeck 自动维护，手动改动会被覆盖。\n\n'
    for r in entries:
        timestamp = datetime.fromtimestamp(r[3], timezone.utc).strftime('%Y-%m-%d %H:%M')
        text += f'- **{timestamp}** [{r[6]}] {r[2]}\n'
    return text.encode('utf-8')


def paths(home):
    return home / '.agentdeck', home / '.agentdeck/memory.db'


def prepare(home):
    safe_path(home, directory=True)
    root, db = paths(home)
    root.mkdir(mode=0o700, exist_ok=True)
    safe_path(root, directory=True)
    for name in ('memory.db', 'memory.db-wal', 'memory.db-shm', 'memory', 'memory-collection.json', 'memory-fixture-private.txt'):
        if os.path.lexists(root / name): raise FixtureFailure('fixture_requires_absent_memory')
    private_write(db, b'')
    with sqlite3.connect(db) as connection:
        connection.execute('PRAGMA journal_mode=WAL')
        connection.executescript(SCHEMA)
        connection.executemany('INSERT INTO memories VALUES (?,?,?,?,?,?,?)', rows())
        connection.executemany('INSERT INTO manual_injections VALUES (?,?)', manual())
    connection.close()
    (root / 'memory').mkdir(mode=0o700)
    for target in TARGETS: private_write(root / 'memory' / (target + '.md'), markdown('initial', target))
    private_write(root / 'memory-collection.json', b'false')
    private_write(root / 'memory-fixture-private.txt', CANARY.encode())
    verify(home, 'initial')


def connect_readonly(home):
    root, db = paths(home)
    safe_path(db)
    # Product operations must have settled; do not ignore a live WAL transaction.
    for suffix in ('-wal', '-shm', '-journal'):
        sidecar = Path(str(db) + suffix)
        if os.path.lexists(sidecar):
            m = safe_path(sidecar)
            if suffix == '-journal' or (suffix == '-wal' and m.st_size != 0) or (suffix == '-shm' and m.st_size != 32768):
                raise FixtureFailure('database_not_settled')
    # Read-only WAL readers may retain an empty WAL and 32KiB shared-memory
    # coordination file. Only their existence/locks may change; no WAL bytes
    # are ignored. Immutable reads are safe only after this zero-WAL guard.
    return sqlite3.connect(db.as_uri() + '?mode=ro&immutable=1', uri=True)


def verify(home, state):
    if state not in STATES: raise FixtureFailure('unknown_fixture_state')
    root, db = paths(home)
    with connect_readonly(home) as connection:
        actual_rows = connection.execute('SELECT * FROM memories ORDER BY id').fetchall()
        actual_manual = connection.execute('SELECT * FROM manual_injections ORDER BY agent_id,memory_id').fetchall()
        if actual_rows != rows(state) or actual_manual != manual(state): raise FixtureFailure('database_mismatch')
        if connection.execute('PRAGMA integrity_check').fetchall() != [('ok',)]: raise FixtureFailure('database_integrity')
    connection.close()
    safe_path(root / 'memory', directory=True)
    if {p.name for p in (root / 'memory').iterdir()} != {t + '.md' for t in TARGETS}: raise FixtureFailure('export_members_mismatch')
    for target in TARGETS:
        path = root / 'memory' / (target + '.md'); safe_path(path)
        # Authored external mutations never call product export. Until a confirmed
        # write, old files remain byte-for-byte initial fixture output.
        expected = markdown(state if state in ('replaced', 'cleared') else 'initial', target)
        if path.read_bytes() != expected: raise FixtureFailure('export_mismatch')
    for filename, data in [('memory-collection.json', b'false'), ('memory-fixture-private.txt', CANARY.encode())]:
        path = root / filename; safe_path(path)
        if path.read_bytes() != data: raise FixtureFailure('unrelated_memory_file_changed')


def mutate(home, kind):
    previous = {'manual': 'initial', 'auto': 'manual-drift', 'row': 'auto-drift'}
    if kind not in previous: raise FixtureFailure('unknown_fixture_edit')
    verify(home, previous[kind])
    root, db = paths(home)
    with sqlite3.connect(db) as connection:
        if kind == 'manual': connection.execute("INSERT INTO manual_injections VALUES ('codex',6)")
        elif kind == 'auto': connection.execute('UPDATE memories SET auto_inject=1 WHERE id=7')
        else: connection.execute("UPDATE memories SET content=content || ' revised' WHERE id=4")
    connection.close()
    verify(home, kind + '-drift')


def snapshot(home):
    """No-write assertion includes bytes, non-atime metadata, ACL (Windows caller).

    Only the authored memory namespace is here. Native wrappers also snapshot
    unrelated configuration trees so writes cannot disappear outside this scope.
    """
    root, _ = paths(home)
    result = {}
    for name in ('memory.db', 'memory', 'memory-collection.json', 'memory-fixture-private.txt'):
        start = root / name
        for path in [start, *sorted(start.rglob('*'))] if start.is_dir() else [start]:
            m = safe_path(path, directory=path.is_dir())
            data = None if path.is_dir() else hashlib.sha256(path.read_bytes()).hexdigest()
            result[str(path.relative_to(root))] = (m.st_dev, m.st_ino, m.st_mode, m.st_nlink, m.st_size,
                m.st_mtime_ns, m.st_ctime_ns, getattr(m, 'st_file_attributes', 0), data)
    # Read only immutable SQLite after recording physical state, never opening
    # the writer or checkpointing as part of verification.
    connection = connect_readonly(home); connection.close()
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--home', type=Path, required=True)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--prepare', action='store_true')
    group.add_argument('--mutate', choices=('manual', 'auto', 'row'))
    group.add_argument('--verify', choices=STATES)
    group.add_argument('--snapshot', action='store_true')
    args = parser.parse_args()
    # CLI authoring must never be pointed at an ordinary user home.
    if (os.environ.get('GITHUB_ACTIONS'), os.environ.get('RUNNER_ENVIRONMENT'), os.environ.get('RUNNER_OS')) != ('true', 'github-hosted', 'Windows'):
        raise FixtureFailure('requires_hosted_windows_ci')
    home = args.home
    runner = Path(os.environ['RUNNER_TEMP']).resolve()
    if not home.is_absolute() or home.resolve() != home or runner not in home.parents or str(home) != os.environ.get('AGENTDECK_TEST_HOME'):
        raise FixtureFailure('requires_disposable_home')
    if args.prepare: prepare(home)
    elif args.mutate: mutate(home, args.mutate)
    elif args.verify: verify(home, args.verify)
    print(json.dumps(snapshot(home) if args.snapshot else {'verified': True}, sort_keys=True))


if __name__ == '__main__':
    try: main()
    except Exception:
        print('Authored memory fixture guard failed.', file=__import__('sys').stderr)
        raise SystemExit(1) from None
