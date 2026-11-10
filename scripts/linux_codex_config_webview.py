"""Real App/WebView read-only Codex plan, authored files and process sentinel only.

Run after the unchanged installation acceptance. Harness-created conflict files
are not product writes and are never presented as real user configuration.
"""
import hashlib
import json
import os
import stat

CANARY = 'QODER-SYNTHETIC-PRIVATE-CONTENT-NOT-FOR-UI'
STAGES = ('absent_plan', 'absent_clear', 'existing_conflict', 'conflict_clear')
PANEL = "//section[@aria-label='Codex 只读配置计划']"
RESULT = PANEL + "//section[@aria-label='Codex 配置计划结果']"
PREVIEW = PANEL + "//button[@data-testid='codex-config-preview']"
CLEAR = PANEL + "//button[text()='清除 Codex 配置计划']"
START = '<!-- agentdeck:personalization:start -->'


class CodexConfigFailure(Exception):
    """Fixed error codes; never configuration text."""


def digest(data):
    return hashlib.sha256(data).hexdigest()


def snapshot(home):
    """Bounded config trees including absence, bytes and non-atime metadata."""
    result = {}
    for name in ('.agentdeck', '.codex'):
        root = home / name
        if not os.path.lexists(root):
            result[name] = None
            continue
        pending = [root]
        while pending:
            path = pending.pop()
            if len(result) >= 256:
                raise CodexConfigFailure('fixture_limit')
            m = path.lstat()
            if m.st_uid != os.geteuid() or not (stat.S_ISDIR(m.st_mode) or stat.S_ISREG(m.st_mode)):
                raise CodexConfigFailure('unsafe_fixture')
            hashed = None
            if stat.S_ISDIR(m.st_mode):
                pending.extend(sorted(path.iterdir()))
            else:
                if m.st_nlink != 1 or m.st_size > 2_097_152:
                    raise CodexConfigFailure('unsafe_fixture')
                with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
                    held = os.fstat(stream.fileno())
                    data = stream.read(2_097_153)
                if (held.st_dev, held.st_ino) != (m.st_dev, m.st_ino) or len(data) != m.st_size:
                    raise CodexConfigFailure('fixture_changed')
                hashed = digest(data)
            result[str(path.relative_to(home))] = (m.st_dev, m.st_ino, m.st_mode, m.st_uid,
                m.st_gid, m.st_nlink, m.st_size, m.st_mtime_ns, m.st_ctime_ns, hashed)
    return result


def identity(path):
    m = path.lstat()
    return (m.st_dev, m.st_ino, m.st_mode, m.st_uid, m.st_gid, m.st_nlink,
            m.st_size, m.st_mtime_ns, m.st_ctime_ns)


def install_sentinel(fixture):
    """Replace only the already-verified synthetic Codex program, after its gate."""
    program = fixture['package'] / 'bin/codex.js'
    expected = b"#!/usr/bin/env node\n'use strict';\nif (JSON.stringify(process.argv.slice(2)) !== '[\"--version\"]') process.exit(64);\nconsole.log('codex-cli 1.2.3');\n"
    marker = fixture['root'] / 'codex-config-process-marker'
    m = program.lstat()
    if (not stat.S_ISREG(m.st_mode) or m.st_nlink != 1 or m.st_uid != os.geteuid()
            or stat.S_IMODE(m.st_mode) != 0o700 or program.read_bytes() != expected
            or os.path.lexists(marker)):
        raise CodexConfigFailure('sentinel_precondition')
    content = ('#!/usr/bin/env node\nrequire("node:fs").writeFileSync(' + json.dumps(str(marker))
               + ', "unexpected synthetic Codex invocation", {flag:"wx",mode:0o600});process.exit(64);\n').encode()
    with os.fdopen(os.open(program, os.O_WRONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'wb') as stream:
        held = os.fstat(stream.fileno())
        if (held.st_dev, held.st_ino) != (m.st_dev, m.st_ino):
            raise CodexConfigFailure('sentinel_changed')
        stream.truncate(0); stream.write(content)
    return program, marker, content


def assert_coverage(records):
    if records != [{'stage': stage, 'verified': True} for stage in STAGES]:
        raise CodexConfigFailure('incomplete_coverage')


def exercise(driver, home, fixture, records, write_private, phase=lambda _: None):
    # No refresh/list action after installing this sentinel. Earlier installation
    # proof has completed before the program changes; no real Codex is installed.
    phase('codex_config_sentinel')
    program, marker, content = install_sentinel(fixture)
    program_identity = identity(program)
    queries = fixture['queries'].read_bytes()
    queries_identity = identity(fixture['queries'])
    if os.path.lexists(home / '.codex') or os.path.lexists(home / '.agentdeck/profiles'):
        raise CodexConfigFailure('unexpected_initial_configuration')

    def unchanged(before):
        if snapshot(home) != before or os.path.lexists(marker) or program.read_bytes() != content or identity(program) != program_identity or fixture['queries'].read_bytes() != queries or identity(fixture['queries']) != queries_identity:
            raise CodexConfigFailure('unexpected_write_or_process')

    def clean():
        for value in (driver.text('//body'), driver.command('GET', '/source')):
            if not isinstance(value, str) or CANARY in value:
                raise CodexConfigFailure('private_content_exposed')

    def activate(selector):
        def ready():
            count = len(driver.elements(selector))
            if count > 1: raise CodexConfigFailure('ambiguous_control')
            return count == 1 and driver.enabled(selector)
        driver.wait(ready)
        element = driver.one(selector)
        # Native Enter avoids moving pointer hit areas; one activation, no retry.
        driver.command('POST', '/element/' + element + '/value', {'text': '\ue007', 'value': ['\ue007']})

    def plan(before):
        activate(PREVIEW)
        driver.wait(lambda: len(driver.elements(RESULT)) == 1)
        clean(); unchanged(before)
        value = driver.text(RESULT)
        if '应用功能尚未开放。' not in value:
            raise CodexConfigFailure('write_boundary_missing')
        if driver.elements(PANEL + "//button[contains(text(),'应用')]"):
            raise CodexConfigFailure('unexpected_apply_control')
        for kind, filename in (('instructions', 'AGENTS.md'), ('settings', 'config.toml')):
            selector = RESULT + "//section[@aria-label='Codex " + kind + " 配置目标']"
            if len(driver.elements(selector)) != 1 or str(home / '.codex' / filename) not in driver.text(selector):
                raise CodexConfigFailure('target_path_mismatch')
        return value

    def clear(before):
        activate(CLEAR)
        driver.wait(lambda: not driver.elements(RESULT) and not driver.elements(CLEAR)
                    and not driver.elements(PANEL + "//*[@role='status' or @role='alert']"))
        clean(); unchanged(before)

    phase('codex_config_absent_plan')
    before = snapshot(home)
    value = plan(before)
    if ('默认 profile 缺失，使用内置模板计算；未创建 profile。' not in value
            or '冲突总数：0' not in value or value.count('文件状态：不存在；计划：拟创建。') != 2):
        raise CodexConfigFailure('absent_plan_mismatch')
    records.append({'stage': 'absent_plan', 'verified': True})
    phase('codex_config_absent_clear'); clear(before)
    records.append({'stage': 'absent_clear', 'verified': True})

    phase('codex_config_author_conflict_fixture')
    profile = home / '.agentdeck/profiles/default/personalization'
    (profile / 'codex').mkdir(parents=True, mode=0o700)
    target = home / '.codex'; target.mkdir(mode=0o700)
    write_private(profile / 'codex/AGENTS.md', ('Authored template ' + CANARY).encode())
    write_private(profile / 'codex.json', json.dumps({'sections': {'features': {'synthetic_fixture': True}}}).encode())
    write_private(target / 'AGENTS.md', (START + '\n' + CANARY).encode())
    write_private(target / 'config.toml', ('# ' + CANARY + '\n[features]\nsynthetic_fixture = false\n').encode())
    before = snapshot(home)
    phase('codex_config_existing_conflict')
    value = plan(before)
    if ('使用现有默认 profile；未修改源文件。' not in value or '冲突总数：2' not in value
            or value.count('文件状态：已存在；计划：存在冲突，不可应用。') != 2
            or '受管块标记不完整或重复' not in value or '现有配置值与模板不同' not in value
            or 'synthetic_fixture' in value):
        raise CodexConfigFailure('conflict_plan_mismatch')
    records.append({'stage': 'existing_conflict', 'verified': True})
    phase('codex_config_conflict_clear'); clear(before)
    records.append({'stage': 'conflict_clear', 'verified': True})
    assert_coverage(records)


ANIMATION_STAGES = ('preview_cancel', 'stale_apply_rejected', 'apply',
                    'inspect_cancel', 'drift_restore_rejected', 'restore')


def assert_animation_coverage(records):
    if records != [{'stage': stage, 'verified': True} for stage in ANIMATION_STAGES]:
        raise CodexConfigFailure('incomplete_animations_coverage')


def exercise_animations(driver, home, fixture, records, write_private, phase=lambda _: None):
    """Native controls over authored files; no real Codex process or credentials."""
    target = home / '.codex/config.toml'
    receipt = target.parent / '.agentdeck-codex-animations.json'
    original = ('# ' + CANARY + '\n[tui]\nanimations = true # keep authored comment\n').encode()
    owned = original.replace(b'animations = true', b'animations = false')
    drift = owned + b'# external edit\n'
    program = fixture['package'] / 'bin/codex.js'
    marker = fixture['root'] / 'codex-config-process-marker'
    program_bytes, program_id = program.read_bytes(), identity(program)
    query_bytes, query_id = fixture['queries'].read_bytes(), identity(fixture['queries'])
    def author(data):
        # Harness-authored external edits, never product writes or symlink traversal.
        with os.fdopen(os.open(target, os.O_WRONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'wb') as stream:
            m = os.fstat(stream.fileno())
            if not stat.S_ISREG(m.st_mode) or m.st_uid != os.geteuid() or m.st_nlink != 1:
                raise CodexConfigFailure('unsafe_animations_fixture')
            stream.truncate(0); stream.write(data)

    # Existing read-only plan fixture is intentionally replaced only after its gate.
    author(original)
    unaffected = {p: (p.read_bytes(), identity(p)) for p in (
        home / '.codex/AGENTS.md',
        home / '.agentdeck/profiles/default/personalization/codex/AGENTS.md',
        home / '.agentdeck/profiles/default/personalization/codex.json')}

    expected_paths = set(snapshot(home)) | {'.codex/.agentdeck-codex-animations.json', '.codex/.agentdeck-codex-animations.lock'}

    def selector(name):
        return "//*[@data-testid='codex-animations-" + name + "']"

    def activate(name):
        control = selector(name)
        def ready():
            count = len(driver.elements(control))
            if count > 1: raise CodexConfigFailure('ambiguous_animations_control')
            return count == 1 and driver.enabled(control)
        driver.wait(ready)
        element = driver.one(control)
        driver.command('POST', '/element/' + element + '/value', {'text': '\ue007', 'value': ['\ue007']})

    def completed(status):
        driver.wait(lambda: len(driver.elements(selector('result') + "[@data-status='" + status + "']")) == 1)
        clean()

    def clean():
        if set(snapshot(home)) - expected_paths:
            raise CodexConfigFailure('animations_unexpected_artifact')
        if (os.path.lexists(marker) or program.read_bytes() != program_bytes
                or identity(program) != program_id or fixture['queries'].read_bytes() != query_bytes
                or identity(fixture['queries']) != query_id):
            raise CodexConfigFailure('animations_unexpected_process')
        for path, before in unaffected.items():
            if (path.read_bytes(), identity(path)) != before:
                raise CodexConfigFailure('animations_unrelated_write')
        for text in (driver.text('//body'), driver.command('GET', '/source')):
            if not isinstance(text, str) or CANARY in text:
                raise CodexConfigFailure('animations_private_content_exposed')

    def same(before):
        clean()
        if snapshot(home) != before: raise CodexConfigFailure('animations_unexpected_write')

    def cancel(before):
        activate('cancel')
        driver.wait(lambda: not driver.elements(selector('result')))
        same(before)

    def preview():
        before = snapshot(home)
        activate('preview'); completed('preview'); same(before)

    def recorded(stage):
        clean(); records.append({'stage': stage, 'verified': True})

    phase('codex_animations_preview_cancel')
    # Target defaults false in UI; assert the checkbox rather than silently toggling.
    control = selector('enabled')
    driver.wait(lambda: len(driver.elements(control)) == 1)
    checked = driver.command('GET', '/element/' + driver.one(control) + '/selected')
    if checked is not False: raise CodexConfigFailure('animations_target_not_false')
    before = snapshot(home); preview(); cancel(before); recorded('preview_cancel')

    phase('codex_animations_stale_apply')
    preview(); author(original + b'# external edit\n')
    before = snapshot(home)
    activate('confirm-apply'); completed('error')
    # First rejected apply may create only its documented empty private lock.
    after = snapshot(home)
    lock_key = '.codex/.agentdeck-codex-animations.lock'
    if lock_key not in before and lock_key in after:
        lock = home / lock_key
        if lock.read_bytes() != b'' or stat.S_IMODE(lock.stat().st_mode) != 0o600:
            raise CodexConfigFailure('animations_invalid_lock')
        after.pop(lock_key)
        after['.codex'] = before['.codex']
    if after != before: raise CodexConfigFailure('animations_stale_write')
    before = snapshot(home)
    if receipt.exists(): raise CodexConfigFailure('animations_stale_receipt')
    cancel(before); recorded('stale_apply_rejected')

    phase('codex_animations_apply')
    author(original); preview()
    activate('confirm-apply'); completed('applied')
    if target.read_bytes() != owned or not receipt.is_file():
        raise CodexConfigFailure('animations_apply_mismatch')
    state = json.loads(receipt.read_bytes())
    if (state.get('phase') != 'applied' or state.get('field') != 'tui.animations'
            or state.get('before') is not True or state.get('after') is not False
            or state.get('before_digest') != digest(original)
            or state.get('after_digest') != digest(owned)
            or stat.S_IMODE(receipt.stat().st_mode) != 0o600):
        raise CodexConfigFailure('animations_receipt_mismatch')
    recorded('apply')

    phase('codex_animations_inspect_cancel')
    before = snapshot(home)
    activate('inspect'); completed('recovery'); same(before)
    cancel(before); recorded('inspect_cancel')

    phase('codex_animations_restore_drift')
    activate('inspect'); completed('recovery')
    author(drift); before = snapshot(home)
    activate('confirm-restore'); completed('error'); same(before)
    if not receipt.is_file(): raise CodexConfigFailure('animations_recovery_evidence_lost')
    cancel(before); recorded('drift_restore_rejected')

    phase('codex_animations_restore')
    author(owned)
    before = snapshot(home); activate('inspect'); completed('recovery'); same(before)
    activate('confirm-restore'); completed('restored')
    if target.read_bytes() != original or receipt.exists():
        raise CodexConfigFailure('animations_restore_mismatch')
    recorded('restore')
    assert_animation_coverage(records)
