"""Synthetic npm management acceptance through real native GUI/IPC/processes.

This generates authored local fixtures, never downloads or executes an agent.
It does not certify registry, real-package or lifecycle-script compatibility.
The caller enforces hosted Linux and retains the WebKit sandbox.
"""
import json
import hashlib
import base64
import math
import os
from pathlib import Path
import shutil
import stat

from installer_node_fixture import measured_version

STAGES = ('preview_cancel', 'stale_revision', 'confirmed_install', 'observed_source', 'no_configuration')
PREVIEW = "//*[@data-testid='install-preview-codex']"
DIALOG = "//*[@data-testid='install-review-dialog']"
CONFIRM = "//*[@data-testid='install-review-confirm']"
CANCEL = "//*[@data-testid='install-review-cancel']"
CARD = PREVIEW + '/ancestor::article'
DETAILS = CARD + "//button[contains(@class, 'agent-detail-button')]"
SOURCE = CARD + "//section[contains(@class, 'agent-installation-evidence')]"


class InstallFailure(Exception):
    """Fixed classifications only; no external diagnostic content."""


def reject_existing_codex(path):
    for entry in path.split(os.pathsep):
        # Relative/empty PATH entries make absence dependent on process cwd.
        directory = Path(entry)
        if not entry or not directory.is_absolute():
            raise InstallFailure('unsafe_search_path')
        if os.path.lexists(directory / 'codex'):
            raise InstallFailure('preexisting_codex')


def private_write(path, content, executable=False):
    with path.open('x', encoding='utf-8') as stream:
        stream.write(content)
    path.chmod(0o700 if executable else 0o600)


def prepare(root, path, mode="success"):
    if mode not in ("success", "wrong-root"): raise InstallFailure("invalid_fixture_mode")
    reject_existing_codex(path)
    node = shutil.which('node', path=path)
    if not node:
        raise InstallFailure('node_missing')
    node = Path(node).resolve(strict=True)
    if not node.is_file() or not os.access(node, os.X_OK):
        raise InstallFailure('node_not_native')
    with node.open('rb') as stream:
        if stream.read(4) != b'\x7fELF': raise InstallFailure('node_not_native')
    try:
        node_version = measured_version(node)
    except (ValueError, OSError) as error:
        raise InstallFailure('node_version_unavailable') from error
    root.mkdir(mode=0o700)
    root = root.resolve(strict=True)
    paths = {'root': root, 'mode': mode, 'node': node, 'installer_node_version': node_version, 'manager_bin': root / 'manager/bin',
             'manager': root / 'manager/lib/node_modules/npm/bin/npm-cli.js',
             'prefix': root / 'prefix', 'global': root / 'prefix/lib/node_modules',
             'package': root / 'prefix/lib/node_modules/@openai/codex',
             'bin': root / 'prefix/bin', 'marker': root / 'install-marker',
             'versions': root / 'version-events', 'calls': root / 'install-events', 'queries': root / 'query-events', 'latest': root / 'latest-version'}
    for directory in (paths['manager'].parent, paths['manager_bin'], paths['bin'], paths['global']):
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    # Intermediate directories also remain private independently of umask.
    for directory in root.rglob('*'):
        if directory.is_dir(): directory.chmod(0o700)
    private_write(paths['manager'].parent.parent / 'package.json', json.dumps({
        'name': 'npm', 'version': '0.0.0', 'bin': {'npm': 'bin/npm-cli.js'}}))
    private_write(paths['queries'], '')
    private_write(paths['versions'], '')
    private_write(paths['calls'], '')
    private_write(paths['latest'], '1.2.3')
    expected = ['install', '--global', '--prefix', str(paths['prefix']), '--force=false',
                '--no-audit', '--no-fund', '--no-progress', '--ignore-scripts', '@openai/codex@1.2.3']
    constants = {key: str(value) for key, value in paths.items()}
    codex = "#!/usr/bin/env node\n'use strict';\nif (JSON.stringify(process.argv.slice(2)) !== '[\"--version\"]') process.exit(64);\nconsole.log('codex-cli 1.2.3');\n"
    if mode == 'wrong-root':
        codex = "#!/usr/bin/env node\n'use strict';\nconst fs = require('node:fs');\nconst log = " + json.dumps(str(paths['versions'])) + ";\nconst s = fs.lstatSync(log); if (!s.isFile() || s.nlink !== 1 || s.size >= 4096) process.exit(64); fs.appendFileSync(log, 'v');\nif (JSON.stringify(process.argv.slice(2)) !== '[\"--version\"]') process.exit(64);\nconsole.log('codex-cli 1.2.3');\n"
    script = "#!/usr/bin/env node\n'use strict';\n" + 'const p = ' + json.dumps(constants) + ';\n'
    script += 'const expected = ' + json.dumps(expected) + ';\nconst codex = ' + json.dumps(codex) + ';\n'
    script += r'''
// Only built-in filesystem operations; no network, shell, npm or child process.
const fs = require('node:fs');
const path = require('node:path');
function safeDirectory(target) {
  if (target !== p.root && !target.startsWith(p.root + '/')) throw Error('escape');
  let current = p.root;
  for (const component of [''].concat(path.relative(p.root, target).split('/').filter(Boolean))) {
    current = path.join(current, component);
    const s = fs.lstatSync(current);
    if (!s.isDirectory() || s.isSymbolicLink() || (s.mode & 0o777) !== 0o700 || s.uid !== process.getuid()) throw Error('unsafe');
  }
}
function query(kind) {
  const s = fs.lstatSync(p.queries);
  if (!s.isFile() || s.nlink !== 1 || s.uid !== process.getuid() || (s.mode & 0o777) !== 0o600 || s.size >= 4096) throw Error('query log');
  fs.appendFileSync(p.queries, kind);
}
function absent(target) { try { fs.lstatSync(target); } catch (e) { if (e.code === 'ENOENT') return; throw e; } throw Error('exists'); }
try {
  for (const directory of [p.prefix, p.global, p.bin, path.dirname(p.manager)]) safeDirectory(directory);
  if (fs.realpathSync(process.argv[1]) !== p.manager) throw Error('entry');
  const args = JSON.stringify(process.argv.slice(2));
  if (args === '["prefix","--global"]') { query('p'); console.log(p.prefix); }
  else if (args === '["root","--global"]') { query('r'); console.log(p.global); }
  else if (args === '["view","@openai/codex@latest","version","--json","--fetch-retries=0","--fetch-timeout=25000"]') { query('m'); const s = fs.lstatSync(p.latest); if (!s.isFile() || s.nlink !== 1 || s.size !== 5) throw Error('metadata'); const version = fs.readFileSync(p.latest, 'utf8'); if (!['1.2.3','1.2.4'].includes(version)) throw Error('metadata'); console.log(JSON.stringify(version)); }
  else if (args === JSON.stringify(expected)) {
    if (p.mode === 'wrong-root') { const s = fs.lstatSync(p.calls); if (!s.isFile() || s.nlink !== 1 || s.size >= 4096) throw Error('calls'); fs.appendFileSync(p.calls, 'i'); }
    absent(p.marker); absent(p.package); absent(path.join(p.bin, 'codex'));
    const scope = path.dirname(p.package);
    if (fs.existsSync(scope)) safeDirectory(scope);
    // Exclusive marker prevents replay. A failure after this point is not retried.
    fs.writeFileSync(p.marker, 'synthetic-install-once\n', {flag:'wx', mode:0o600});
    fs.mkdirSync(scope, {recursive:true, mode:0o700});
    fs.mkdirSync(p.package, {mode:0o700});
    fs.mkdirSync(path.join(p.package, 'bin'), {mode:0o700});
    fs.writeFileSync(path.join(p.package, 'package.json'), JSON.stringify({name:'@openai/codex',version:p.mode === 'wrong-root' ? '1.2.4' : '1.2.3',bin:{codex:'bin/codex.js'}}), {flag:'wx',mode:0o600});
    fs.writeFileSync(path.join(p.package, 'bin/codex.js'), codex, {flag:'wx',mode:0o700});
    fs.symlinkSync(path.join(p.package, 'bin/codex.js'), path.join(p.bin, 'codex'));
  } else { throw Error('arguments'); }
} catch (_) { process.stderr.write('synthetic fixture rejected operation\n'); process.exitCode = 64; }
'''
    private_write(paths['manager'], script, executable=True)
    (paths['manager_bin'] / 'npm').symlink_to(paths['manager'])
    (paths['manager_bin'] / 'node').symlink_to(node)
    paths['path'] = os.pathsep.join((str(paths['manager_bin']), str(paths['bin']), path))
    return paths


def query_counts(fixture):
    """Bounded fixed-enum evidence, never shell output or external paths."""
    path = fixture['queries']
    meta = path.lstat()
    if not stat.S_ISREG(meta.st_mode) or meta.st_nlink != 1 or meta.st_uid != os.geteuid() or stat.S_IMODE(meta.st_mode) != 0o600 or meta.st_size > 4096:
        raise InstallFailure('unsafe_query_evidence')
    with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
        data = stream.read(4097)
    if len(data) > 4096 or any(byte not in b'prm' for byte in data):
        raise InstallFailure('unsafe_query_evidence')
    return {'prefix_queries': data.count(b'p'), 'root_queries': data.count(b'r'), 'metadata_queries': data.count(b'm')}


def assert_absent(fixture):
    if any(os.path.lexists(p) for p in (fixture['marker'], fixture['package'], fixture['bin'] / 'codex')):
        raise InstallFailure('unconfirmed_write')


def assert_installed(fixture):
    if fixture['marker'].read_bytes() != b'synthetic-install-once\n':
        raise InstallFailure('install_count')
    if (fixture['bin'] / 'codex').resolve(strict=True) != fixture['package'] / 'bin/codex.js':
        raise InstallFailure('launcher_changed')
    manifest = json.loads((fixture['package'] / 'package.json').read_text())
    if manifest != {'name': '@openai/codex', 'version': '1.2.3', 'bin': {'codex': 'bin/codex.js'}}:
        raise InstallFailure('package_changed')


def configuration_facts(home):
    """Bounded names from the freshly created synthetic HOME, never contents."""
    product = home / '.agentdeck'
    return {
        'product_entries': sorted(p.name for p in product.iterdir())[:32] if product.is_dir() else [],
        'matching_paths': sorted(str(p.relative_to(home)) for p in home.rglob('*')
                                 if p.name.lower() in {'.aisync', '.codex', 'profiles', 'profile', 'vault', 'vault.json', 'backups', 'backup'}
                                 or p.name.lower().startswith(('vault.', 'profile.')))[:32],
    }


def assert_no_configuration(home):
    # Native WebKit caches are permitted, product/configuration mutations are not.
    forbidden = {'.aisync', '.codex', 'profiles', 'profile', 'vault', 'vault.json', 'backups', 'backup'}
    product = home / '.agentdeck'
    if product.exists() and any(p.name not in {'registry', 'registry-update.lock', 'install-protection'} for p in product.iterdir()):
        raise InstallFailure('configuration_created')
    if os.path.lexists(product / 'install-protection'):
        from install_protection_fixture import validate_install_protection
        validate_install_protection(home, required=True)
    if any(p.name.lower() in forbidden or p.name.lower().startswith(('vault.', 'profile.')) for p in home.rglob('*')):
        raise InstallFailure('configuration_created')


def assert_plan(driver, fixture):
    fields = {'npm 包': '@openai/codex', 'Codex 根包固定版本': '1.2.3', '启动命令': 'codex', 'npm prefix': str(fixture['prefix']),
              'npm 全局目录': str(fixture['global']), '包目录': str(fixture['package']),
              '启动文件目录': str(fixture['bin']), 'npm 规范入口': str(fixture['manager']),
              '安装器 Node': str(fixture['node']),
              '安装器 Node 实测版本': fixture['installer_node_version'],
              'AgentDeck 安装器 Node 最低策略': '18.0.0'}
    for label, value in fields.items():
        if driver.text(DIALOG + "//div[dt[text()='" + label + "']]/dd") != value:
            raise InstallFailure('plan_mismatch')
    text = driver.text(DIALOG)
    for value in ('Codex 根包版本已固定', '生命周期脚本：禁用（ignore-scripts）', '仅安装软件，不应用 profile'):
        if value not in text: raise InstallFailure('policy_mismatch')


def capture_geometry(driver, evidence, stage):
    """Read-only W3C evidence. Never scripts, hit-test mutation, or click replay."""
    if stage not in {prefix + suffix for prefix in ('install_initial_preview', 'install_stale_preview', 'install_fresh_preview') for suffix in ('_before', '_after_click', '_settled')}:
        raise InstallFailure('invalid_diagnostic_stage')
    result = {}
    last_response = getattr(driver, 'last_response', None)
    def read(name, endpoint, kind):
        try:
            value = driver.command('GET', endpoint)
            if kind == 'rect':
                if not isinstance(value, dict): raise ValueError()
                value = {key: value[key] for key in ('x', 'y', 'width', 'height')}
                if any(type(v) not in (int, float) or not math.isfinite(v) or abs(v) > 1_000_000 for v in value.values()): raise ValueError()
                if value['width'] < 0 or value['height'] < 0: raise ValueError()
            elif kind == 'boolean':
                if type(value) is not bool: raise ValueError()
            elif kind == 'number':
                if type(value) not in (int, float) or not math.isfinite(value) or abs(value) > 1_000_000: raise ValueError()
            elif kind == 'css':
                if value not in {'auto', 'none', 'visible', 'hidden', 'clip', 'scroll', 'block', 'inline', 'flex', 'grid', 'inline-flex', '0', '1'}: value = 'other'
            result[name] = value
        except Exception:
            result[name] = {'state': 'unavailable'}
    elements = {}
    for label, selector in (('button', PREVIEW), ('card', CARD), ('search', "//*[@id='agent-search']"),
                            ('workspace', "//*[@id='workspace-content']"),
                            ('actions', CARD + "//*[contains(@class, 'agent-row-actions')]")):
        try:
            element = driver.one(selector)
            if not isinstance(element, str): raise ValueError()
            elements[label] = element
            read(label + '_rect', '/element/' + element + '/rect', 'rect')
            read(label + '_displayed', '/element/' + element + '/displayed', 'boolean')
            read(label + '_enabled', '/element/' + element + '/enabled', 'boolean')
            for prop in ('pointer-events', 'visibility', 'overflow-x', 'overflow-y', 'opacity'):
                read(label + '_' + prop, '/element/' + element + '/css/' + prop, 'css')
            for prop in ('scrollLeft', 'scrollTop', 'clientWidth', 'clientHeight', 'scrollWidth', 'scrollHeight'):
                read(label + '_' + prop, '/element/' + element + '/property/' + prop, 'number')
        except Exception:
            result[label] = {'state': 'unavailable'}
    read('window_rect', '/window/rect', 'rect')
    try:
        active = driver.command('GET', '/element/active')
        identifier = active['element-6066-11e4-a52e-4f735466cecf']
        result['active_element'] = next((name for name, element in elements.items() if element == identifier), 'other')
    except Exception:
        result['active_element'] = 'unavailable'
    try:
        # Only synthetic isolated HOME content is eligible. Inspect bounded DOM
        # for the private fixture canary before retaining the screenshot.
        source = driver.command('GET', '/source')
        if not isinstance(source, str) or len(source.encode()) > 1_048_576 or 'QODER-SYNTHETIC-PRIVATE-CONTENT-NOT-FOR-UI' in source:
            raise ValueError()
        encoded = driver.command('GET', '/screenshot')
        if not isinstance(encoded, str) or len(encoded) > 1_000_000: raise ValueError()
        png = base64.b64decode(encoded, validate=True)
        if not png.startswith(b'\x89PNG\r\n\x1a\n'): raise ValueError()
        name = stage + '.png'
        (evidence / name).write_bytes(png)
        result['screenshot'] = name
    except Exception:
        result['screenshot'] = 'unavailable_or_suppressed'
    if hasattr(driver, 'last_response'): driver.last_response = last_response
    return result


def assert_coverage(records):
    if records != [{'stage': stage, 'verified': True} for stage in STAGES]:
        raise InstallFailure('incomplete_coverage')


def open_details(driver):
    # The completion notice precedes asynchronous list refresh. Wait for its
    # settled installed card, then activate the exact control via native Enter;
    # coordinate clicks can hit the adjacent operation during layout reflow.
    driver.wait(lambda: len(driver.elements(CARD + "//*[@class='agent-version' and text()='v1.2.3']")) == 1
                and len(driver.elements("//*[@aria-label='Agent 列表' and @aria-busy='false']")) == 1)
    driver.wait(lambda: len(driver.elements(DETAILS + "[@aria-expanded='false']")) == 1
                and driver.enabled(DETAILS))
    element = driver.one(DETAILS)
    driver.command('POST', '/element/' + element + '/value', {'text': '\ue007', 'value': ['\ue007']})
    driver.wait(lambda: len(driver.elements(DETAILS + "[@aria-expanded='true']")) == 1)


def exercise(driver, fixture, home, records, phase, trace=None, observe=None):
    driver.click("//*[@role='tab' and @aria-label='Agent 管理']")
    phase('install_search')
    driver.type("//*[@id='agent-search']", 'Codex')
    # Commit the native edit with one WebDriver Tab key before review. Static
    # headings are not interactable click targets in native WebKit. Tab changes
    # focus only; it does not activate the next control or replay any action.
    phase('install_search_tab')
    search = driver.one("//*[@id='agent-search']")
    driver.command('POST', '/element/' + search + '/value', {'text': '\ue004', 'value': ['\ue004']})
    phase('install_search_ready')
    driver.wait(lambda: driver.value("//*[@id='agent-search']") == 'Codex')
    driver.wait(lambda: len(driver.elements("//*[@aria-label='Agent 列表']/article")) == 1)
    driver.wait(lambda: driver.elements(PREVIEW) and driver.enabled(PREVIEW))
    def preview(stage):
        # WebKit can retain a stale scroll offset after filtering, even when
        # content no longer scrolls. Native Enter focuses and activates the
        # button without a moving pointer hit area. One action, never replayed.
        before = query_counts(fixture)
        observation = {'before_ui': observe(stage + '_before')} if observe else {}
        phase(stage + '_click')
        try:
            element = driver.one(PREVIEW)
            driver.command('POST', '/element/' + element + '/value', {'text': '\ue007', 'value': ['\ue007']})
            if observe: observation['after_click_ui'] = observe(stage + '_after_click')
            phase(stage + '_wait_dialog')
            driver.wait(lambda: driver.elements(DIALOG))
        finally:
            if observe: observation['settled_ui'] = observe(stage + '_settled')
            if trace is not None:
                trace.append({'stage': stage, 'before': before, 'after': query_counts(fixture), **observation})
        phase(stage + '_assert_plan')
        assert_plan(driver, fixture)
    def done(stage): records.append({'stage': stage, 'verified': True})
    phase('install_preview_cancel')
    preview('install_initial_preview'); assert_absent(fixture)
    phase('install_cancel_click')
    driver.click(CANCEL)
    phase('install_cancel_wait_closed')
    driver.wait(lambda: not driver.elements(DIALOG))
    assert_absent(fixture); done('preview_cancel')
    phase('install_stale_revision')
    preview('install_stale_preview')
    manager_before = fixture['manager'].read_bytes()
    fixture['latest'].write_text('1.2.4')
    driver.click(CONFIRM)
    driver.wait(lambda: driver.elements("//*[@role='alert' and contains(@class, 'agent-notice')]"))
    if '安装请求已发送，但完成状态未确认。' not in driver.text("//*[@role='alert' and contains(@class, 'agent-notice')]"):
        raise InstallFailure('stale_rejection_not_verified')
    assert_absent(fixture)
    if fixture['manager'].read_bytes() != manager_before: raise InstallFailure('manager_changed')
    fixture['latest'].write_text('1.2.3')
    done('stale_revision')
    phase('install_confirmed')
    driver.wait(lambda: driver.enabled(PREVIEW))
    preview('install_fresh_preview'); driver.click(CONFIRM)
    driver.wait(lambda: fixture['marker'].exists())
    driver.wait(lambda: driver.elements("//*[@role='status' and contains(@class, 'agent-notice')]") and '软件安装操作已完成，未应用配置' in driver.text("//*[@role='status' and contains(@class, 'agent-notice')]"))
    assert_installed(fixture); done('confirmed_install')
    phase('install_observed_source')
    open_details(driver)
    source = SOURCE
    driver.wait(lambda: driver.elements(source))
    driver.wait(lambda: '实测版本 1.2.3' in driver.text(source))
    text = driver.text(source)
    if any(value not in text for value in ('npm 全局目录匹配，安装历史未知', '实测版本 1.2.3', '@openai/codex', str(fixture['package']), str(fixture['manager_bin'] / 'npm'))):
        raise InstallFailure('source_mismatch')
    done('observed_source')
    phase('install_no_configuration')
    assert_no_configuration(home); done('no_configuration')
    assert_coverage(records)


WRONG_ROOT_GUARD = 'Codex 安装保护未通过检查，已暂停此条目的主动探测。刷新或切换页面不会为此条目执行 Codex 或 npm；重启不代表安全恢复。未清理残留。'
WRONG_ROOT_STAGES = ('wrong_root_rejected', 'automatic_refresh_guarded', 'manual_refresh_guarded', 'navigation_guarded', 'residue_unchanged')


def wrong_root_snapshot(fixture):
    """Private bounded byte/identity evidence; returned values are never published."""
    prefix = fixture['prefix']
    paths = [prefix, *sorted(prefix.rglob('*')), fixture['marker']]
    if len(paths) > 32: raise InstallFailure('wrong_root_inventory')
    result = []
    for path in paths:
        meta = path.lstat()
        identity = tuple(getattr(meta, key) for key in ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_uid', 'st_gid', 'st_size', 'st_mtime_ns', 'st_ctime_ns'))
        if stat.S_ISLNK(meta.st_mode): value = os.readlink(path)
        elif stat.S_ISREG(meta.st_mode):
            if meta.st_nlink != 1 or meta.st_size > 65536: raise InstallFailure('wrong_root_inventory')
            value = hashlib.sha256(path.read_bytes()).hexdigest()
        elif stat.S_ISDIR(meta.st_mode): value = None
        else: raise InstallFailure('wrong_root_inventory')
        result.append((str(path), identity, value))
    return result


def assert_wrong_root(fixture, expected=None, attempts=1):
    if fixture['mode'] != 'wrong-root': raise InstallFailure('invalid_fixture_mode')
    if fixture['versions'].read_bytes() != b'': raise InstallFailure('wrong_root_agent_executed')
    if fixture['calls'].read_bytes() != b'i' * attempts: raise InstallFailure('wrong_root_install_count')
    if fixture['marker'].read_bytes() != b'synthetic-install-once\n': raise InstallFailure('install_count')
    package = fixture['package']
    if (fixture['bin'] / 'codex').is_symlink() is not True or (fixture['bin'] / 'codex').resolve(strict=True) != package / 'bin/codex.js':
        raise InstallFailure('launcher_changed')
    executable = (package / 'bin/codex.js').lstat()
    if not stat.S_ISREG(executable.st_mode) or executable.st_nlink != 1 or stat.S_IMODE(executable.st_mode) != 0o700:
        raise InstallFailure('wrong_root_inventory')
    if {str(p.relative_to(package)) for p in package.rglob('*')} != {'package.json', 'bin', 'bin/codex.js'}:
        raise InstallFailure('wrong_root_inventory')
    if json.loads((package / 'package.json').read_text(encoding='utf-8')) != {'name': '@openai/codex', 'version': '1.2.4', 'bin': {'codex': 'bin/codex.js'}}:
        raise InstallFailure('package_changed')
    snapshot = wrong_root_snapshot(fixture)
    if expected is not None and expected != snapshot: raise InstallFailure('wrong_root_residue_changed')
    return snapshot


def assert_wrong_root_coverage(records):
    if records != [{'stage': stage, 'verified': True} for stage in WRONG_ROOT_STAGES]:
        raise InstallFailure('incomplete_wrong_root_coverage')


def exercise_wrong_root(driver, fixture, home, records, phase):
    """Initial wrong-root attempt; the durable lane then restarts this HOME."""
    # A new WebDriver session can return before React mounts its controls.
    # Poll only read-only readiness; each click/type action is still sent once.
    agents_tab = "//*[@role='tab' and @aria-label='Agent 管理']"
    search_input = "//*[@id='agent-search']"
    phase('wrong_root_wait_agents_tab')
    driver.wait(lambda: len(driver.elements(agents_tab)) == 1 and driver.enabled(agents_tab))
    phase('wrong_root_open_agents')
    driver.click(agents_tab)
    phase('wrong_root_wait_search')
    driver.wait(lambda: len(driver.elements(search_input)) == 1 and driver.enabled(search_input))
    phase('wrong_root_type_search')
    driver.type(search_input, 'Codex')
    search = driver.one("//*[@id='agent-search']")
    driver.command('POST', '/element/' + search + '/value', {'text': '\ue004', 'value': ['\ue004']})
    driver.wait(lambda: driver.value("//*[@id='agent-search']") == 'Codex')
    driver.wait(lambda: len(driver.elements("//*[@aria-label='Agent 列表']/article")) == 1 and driver.elements(PREVIEW) and driver.enabled(PREVIEW))
    preview = driver.one(PREVIEW)
    driver.command('POST', '/element/' + preview + '/value', {'text': '\ue007', 'value': ['\ue007']})
    driver.wait(lambda: driver.elements(DIALOG))
    assert_plan(driver, fixture)
    assert_absent(fixture)
    phase('wrong_root_confirm')
    driver.click(CONFIRM)
    alert = "//*[@role='alert' and contains(@class, 'agent-notice')]"
    driver.wait(lambda: driver.elements(alert) and '安装请求已发送，但完成状态未确认。' in driver.text(alert))
    snapshot = assert_wrong_root(fixture)
    queries = query_counts(fixture)
    records.append({'stage': 'wrong_root_rejected', 'verified': True})

    def check(stage):
        phase(stage)
        driver.wait(lambda: len(driver.elements("//*[@aria-label='Agent 列表' and @aria-busy='false']")) == 1 and WRONG_ROOT_GUARD in driver.text("//*[@aria-label='Agent 列表']"))
        if query_counts(fixture) != queries: raise InstallFailure('wrong_root_query_repeated')
        assert_wrong_root(fixture, snapshot)
        assert_no_configuration(home)
        text = driver.text("//*[@aria-label='Agent 列表']")
        if '软件安装操作已完成' in driver.text('//body') or '实测版本 1.2.3' in text:
            raise InstallFailure('wrong_root_success_displayed')
        # Exact suppression explanation is coordinated with the desktop guard.
        if WRONG_ROOT_GUARD not in text: raise InstallFailure('wrong_root_guard_not_displayed')
        if driver.elements(PREVIEW) and driver.enabled(PREVIEW): raise InstallFailure('wrong_root_install_not_disabled')
        records.append({'stage': stage, 'verified': True})

    check('automatic_refresh_guarded')
    phase('wrong_root_manual_refresh')
    refresh = "//*[@id='workspace-panel-agents' and not(@hidden)]//button[text()='刷新列表']"
    driver.wait(lambda: len(driver.elements(refresh)) == 1 and driver.enabled(refresh))
    driver.click(refresh)
    check('manual_refresh_guarded')
    phase('wrong_root_navigation')
    driver.click("//*[@role='tab' and @aria-label='Qwen 配置检查']")
    driver.wait(lambda: driver.elements("//*[@role='tab' and @aria-label='Qwen 配置检查' and @aria-selected='true']"))
    assert_wrong_root(fixture, snapshot)
    driver.click("//*[@role='tab' and @aria-label='Agent 管理']")
    driver.wait(lambda: driver.elements("//*[@role='tab' and @aria-label='Agent 管理' and @aria-selected='true']"))
    check('navigation_guarded')
    assert_wrong_root(fixture, snapshot)
    assert_no_configuration(home)
    records.append({'stage': 'residue_unchanged', 'verified': True})
    assert_wrong_root_coverage(records)


RECOVERY_RESULT = "//*[@data-testid='codex-recovery-result']"
RECOVERY_PREVIEW = "//*[@data-testid='codex-recovery-preview']"
RECOVERY_CONFIRM = "//*[@data-testid='codex-recovery-confirm']"
RECOVERY_CANCEL = "//*[@data-testid='codex-recovery-cancel']"
RECOVERY_STAGES = ('restart_guarded', 'restart_refresh_guarded', 'restart_navigation_guarded', 'residue_recovery_blocked', 'authored_residue_relocated', 'recovery_cancel_unchanged', 'stale_recovery_rejected', 'explicit_recovery_confirmed', 'new_install_explicitly_confirmed', 'repeated_wrong_root_guarded')


def protection_snapshot(home, phase=None):
    from install_protection_fixture import validate_install_protection, protection_snapshot as snapshot
    validate_install_protection(home, expected_phase=phase, required=True)
    return snapshot(home)


def unique_action(driver, selector):
    driver.wait(lambda: len(driver.elements(selector))==1 and driver.enabled(selector))
    driver.click(selector)


def select_recovery_agent(driver):
    tab = "//*[@role='tab' and @aria-label='Agent 管理']"
    selected = tab + "[@aria-selected='true']"
    unselected = tab + "[@aria-selected='false' and not(@disabled)]"
    search = "//*[@id='agent-search']"
    idle = "//*[@aria-label='Agent 列表' and @aria-busy='false']"

    def wait_read_only(predicate, classification):
        # Startup can render between queries. Poll only observations; never
        # retry a click, input, install or recovery request on failure.
        try:
            driver.wait(predicate)
        except Exception as error:
            raise InstallFailure(classification) from error

    wait_read_only(lambda: len(driver.elements(tab)) == 1 and
                   (len(driver.elements(selected)) == 1 or
                    len(driver.elements(unselected)) == 1),
                   'recovery_workspace_tab_not_ready')
    if len(driver.elements(selected)) != 1:
        # At most one navigation action. An uncertain click is not replayed.
        try:
            driver.click(unselected)
        except Exception as error:
            raise InstallFailure('recovery_workspace_tab_activation_failed') from error
    wait_read_only(lambda: len(driver.elements(selected)) == 1,
                   'recovery_workspace_tab_not_selected')
    wait_read_only(lambda: len(driver.elements(search)) == 1 and
                   len(driver.elements(search + "[not(@disabled)]")) == 1,
                   'recovery_workspace_search_not_ready')
    wait_read_only(lambda: len(driver.elements(idle)) == 1,
                   'recovery_workspace_list_not_idle')
    driver.type(search,'Codex')
    element=driver.one(search)
    driver.command('POST','/element/'+element+'/value',{'text':'\ue004','value':['\ue004']})
    driver.wait(lambda: driver.value(search)=='Codex' and len(driver.elements("//*[@aria-label='Agent 列表']/article"))==1)


def recovery_preview(driver, state, prefix=None):
    unique_action(driver,RECOVERY_PREVIEW)
    driver.wait(lambda: len(driver.elements(RECOVERY_RESULT+"[@data-state='"+state+"']"))==1)
    if state=='eligible':
        if prefix is None or '已记录安装位置：'+str(prefix) not in driver.text(RECOVERY_RESULT):raise InstallFailure('recovery_target_mismatch')
        for slot in ('package_directory','posix_launcher'):
            if driver.text(RECOVERY_RESULT+"//div[dt[text()='"+slot+"']]/dd")!='missing':raise InstallFailure('recovery_slot_mismatch')
        if '确认只解除探测保护；之后刷新可恢复 npm/Codex 查询。不安装、不删除，也不代表包真实性或运行验证。' not in driver.text(RECOVERY_RESULT):raise InstallFailure('recovery_boundary_missing')
    if state=='blocked' and driver.elements(RECOVERY_CONFIRM):
        raise InstallFailure('blocked_recovery_has_confirm')


def cancel_recovery(driver):
    unique_action(driver,RECOVERY_CANCEL)
    driver.wait(lambda:not driver.elements(RECOVERY_RESULT))


def relocate_authored_residue(fixture):
    # Harness-only manual repair. Product recovery never performs these moves.
    # Preserve prefix/bin/lib/node_modules identities bound by the durable record.
    archive=fixture['root']/'authored-relocated-residue'; archive.mkdir(mode=0o700)
    files={}
    for path in fixture['package'].rglob('*'):
        if path.is_file():files[str(path.relative_to(fixture['package']))]=path.read_bytes()
    fixture['package'].rename(archive/'codex-package')
    (fixture['bin']/'codex').rename(archive/'codex-launcher')
    fixture['marker'].rename(archive/'install-marker')
    if {str(p.relative_to(archive/'codex-package')):p.read_bytes() for p in (archive/'codex-package').rglob('*') if p.is_file()}!=files:
        raise InstallFailure('authored_relocation_changed_bytes')
    if fixture['package'].exists() or os.path.lexists(fixture['bin']/'codex') or fixture['marker'].exists():
        raise InstallFailure('authored_relocation_incomplete')
    return archive


def exercise_recovery_after_restart(driver,fixture,home,records,phase,residue,state_before,queries_before,synthetic_cases=None):
    select_recovery_agent(driver)
    def unchanged(stage, require_residue=True):
        phase(stage)
        driver.wait(lambda: len(driver.elements("//*[@aria-label='Agent 列表' and @aria-busy='false']"))==1)
        if fixture['versions'].read_bytes()!=b'' or fixture['calls'].read_bytes()!=b'i' or query_counts(fixture)!=queries_before:
            raise InstallFailure('durable_guard_executed_fixture')
        if protection_snapshot(home,'failed_returned')!=state_before:raise InstallFailure('durable_state_changed')
        if require_residue:assert_wrong_root(fixture,residue)
        assert_no_configuration(home)
        records.append({'stage':stage,'verified':True})
    driver.wait(lambda:WRONG_ROOT_GUARD in driver.text("//*[@aria-label='Agent 列表']"))
    unchanged('restart_guarded')
    unique_action(driver,"//*[@id='workspace-panel-agents' and not(@hidden)]//button[text()='刷新列表']")
    unchanged('restart_refresh_guarded')
    unique_action(driver,"//*[@role='tab' and @aria-label='Qwen 配置检查']")
    unique_action(driver,"//*[@role='tab' and @aria-label='Agent 管理']")
    unchanged('restart_navigation_guarded')
    recovery_preview(driver,'blocked');cancel_recovery(driver)
    unchanged('residue_recovery_blocked')
    phase('authored_residue_relocation')
    archive=relocate_authored_residue(fixture)
    preserved={str(p.relative_to(archive/'codex-package')):p.read_bytes() for p in (archive/'codex-package').rglob('*') if p.is_file()}
    unchanged('authored_residue_relocated',False)
    if synthetic_cases is not None:
        driver=synthetic_cases()
        select_recovery_agent(driver)
        driver.wait(lambda:WRONG_ROOT_GUARD in driver.text("//*[@aria-label='Agent 列表']"))
        if protection_snapshot(home,'failed_returned')!=state_before or query_counts(fixture)!=queries_before:raise InstallFailure('original_guard_changed_after_synthetic_cases')
    recovery_preview(driver,'eligible',fixture['prefix']);cancel_recovery(driver)
    unchanged('recovery_cancel_unchanged',False)
    recovery_preview(driver,'eligible',fixture['prefix'])
    # A test-authored PATH conflict invalidates the reviewed absence; do not
    # modify/delete the preserved original package or the persistent record.
    conflict=fixture['bin']/'codex';private_write(conflict,'authored conflict\n')
    unique_action(driver,RECOVERY_CONFIRM)
    driver.wait(lambda:not driver.elements(RECOVERY_RESULT+"[@data-state='eligible']"))
    driver.wait(lambda:'恢复检查或确认未完成，保护状态未确认。请保留现有文件；不会自动重试。' in driver.text('//body'))
    if conflict.read_text(encoding='utf-8')!='authored conflict\n':raise InstallFailure('recovery_changed_conflict')
    conflict.rename(archive/'stale-recovery-conflict')
    unchanged('stale_recovery_rejected',False)
    if driver.elements(RECOVERY_RESULT):cancel_recovery(driver)
    recovery_preview(driver,'eligible',fixture['prefix']);unique_action(driver,RECOVERY_CONFIRM)
    driver.wait(lambda:not driver.elements(RECOVERY_RESULT))
    driver.wait(lambda:len(driver.elements(PREVIEW))==1 and driver.enabled(PREVIEW))
    protection_snapshot(home,'clear')
    if fixture['calls'].read_bytes()!=b'i' or fixture['versions'].read_bytes()!=b'':raise InstallFailure('recovery_auto_installed_or_probed')
    records.append({'stage':'explicit_recovery_confirmed','verified':True})
    unique_action(driver,PREVIEW)
    driver.wait(lambda:driver.elements(DIALOG));assert_plan(driver,fixture)
    if fixture['calls'].read_bytes()!=b'i':raise InstallFailure('new_preview_installed')
    unique_action(driver,CONFIRM)
    alert="//*[@role='alert' and contains(@class,'agent-notice')]"
    driver.wait(lambda:driver.elements(alert) and '安装请求已发送，但完成状态未确认。' in driver.text(alert))
    assert_wrong_root(fixture,attempts=2)
    records.append({'stage':'new_install_explicitly_confirmed','verified':True})
    driver.wait(lambda:WRONG_ROOT_GUARD in driver.text("//*[@aria-label='Agent 列表']"))
    protection_snapshot(home,'failed_returned');assert_wrong_root(fixture,attempts=2)
    if {str(p.relative_to(archive/'codex-package')):p.read_bytes() for p in (archive/'codex-package').rglob('*') if p.is_file()}!=preserved:raise InstallFailure('authored_archive_changed')
    records.append({'stage':'repeated_wrong_root_guarded','verified':True})
    if records!=[{'stage':stage,'verified':True} for stage in RECOVERY_STAGES]:raise InstallFailure('incomplete_recovery_coverage')


def raw_protection_snapshot(home):
    from install_protection_fixture import protection_snapshot as snapshot
    return snapshot(home)


def exercise_synthetic_recovery_blocked(driver,fixture,home,case,records,phase,before,queries):
    if case not in ('armed','corrupt'):raise InstallFailure('unknown_synthetic_case')
    select_recovery_agent(driver)
    phase('synthetic_'+case+'_guarded')
    driver.wait(lambda:WRONG_ROOT_GUARD in driver.text("//*[@aria-label='Agent 列表']"))
    for stage in ('startup','readonly_recovery','manual_refresh'):
        if stage=='readonly_recovery':
            if case=='armed':recovery_preview(driver,'blocked')
            else:
                unique_action(driver,RECOVERY_PREVIEW)
                error="//*[@data-testid='codex-recovery-error']"
                driver.wait(lambda:len(driver.elements(error))==1 and driver.text(error)=='恢复检查或确认未完成，保护状态未确认。请保留现有文件；不会自动重试。')
                if driver.elements(RECOVERY_RESULT) or driver.elements(RECOVERY_CONFIRM):raise InstallFailure('corrupt_recovery_offered_result')
            cancel_recovery(driver)
        if stage=='manual_refresh':
            unique_action(driver,"//*[@id='workspace-panel-agents' and not(@hidden)]//button[text()='刷新列表']")
        driver.wait(lambda:len(driver.elements("//*[@aria-label='Agent 列表' and @aria-busy='false']"))==1)
        if raw_protection_snapshot(home)!=before:raise InstallFailure('synthetic_guard_changed')
        if fixture['versions'].read_bytes()!=b'' or fixture['calls'].read_bytes()!=b'i' or query_counts(fixture)!=queries:raise InstallFailure('synthetic_guard_executed_fixture')
        if fixture['package'].exists() or os.path.lexists(fixture['bin']/'codex') or fixture['marker'].exists():raise InstallFailure('synthetic_guard_created_residue')
        records.append({'case':case,'stage':stage,'verified':True,'testAuthoredState':True,'crashRecoveryVerified':False})
