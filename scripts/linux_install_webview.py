"""Synthetic npm management acceptance through real native GUI/IPC/processes.

This generates authored local fixtures, never downloads or executes an agent.
It does not certify registry, real-package or lifecycle-script compatibility.
The caller enforces hosted Linux and retains the WebKit sandbox.
"""
import json
import base64
import math
import os
from pathlib import Path
import shutil
import stat

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


def prepare(root, path):
    reject_existing_codex(path)
    node = shutil.which('node', path=path)
    if not node:
        raise InstallFailure('node_missing')
    node = Path(node).resolve(strict=True)
    if not node.is_file() or not os.access(node, os.X_OK):
        raise InstallFailure('node_not_native')
    with node.open('rb') as stream:
        if stream.read(4) != b'\x7fELF': raise InstallFailure('node_not_native')
    root.mkdir(mode=0o700)
    root = root.resolve(strict=True)
    paths = {'root': root, 'node': node, 'manager_bin': root / 'manager/bin',
             'manager': root / 'manager/lib/node_modules/npm/bin/npm-cli.js',
             'prefix': root / 'prefix', 'global': root / 'prefix/lib/node_modules',
             'package': root / 'prefix/lib/node_modules/@openai/codex',
             'bin': root / 'prefix/bin', 'marker': root / 'install-marker',
             'queries': root / 'query-events'}
    for directory in (paths['manager'].parent, paths['manager_bin'], paths['bin'], paths['global']):
        directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    # Intermediate directories also remain private independently of umask.
    for directory in root.rglob('*'):
        if directory.is_dir(): directory.chmod(0o700)
    private_write(paths['manager'].parent.parent / 'package.json', json.dumps({
        'name': 'npm', 'version': '0.0.0', 'bin': {'npm': 'bin/npm-cli.js'}}))
    private_write(paths['queries'], '')
    expected = ['install', '--global', '--prefix', str(paths['prefix']), '--force=false',
                '--no-audit', '--no-fund', '--no-progress', '--ignore-scripts', '@openai/codex']
    constants = {key: str(value) for key, value in paths.items()}
    codex = "#!/usr/bin/env node\n'use strict';\nif (JSON.stringify(process.argv.slice(2)) !== '[\"--version\"]') process.exit(64);\nconsole.log('codex-cli 1.2.3');\n"
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
  else if (args === JSON.stringify(expected)) {
    absent(p.marker); absent(p.package); absent(path.join(p.bin, 'codex'));
    const scope = path.dirname(p.package);
    if (fs.existsSync(scope)) safeDirectory(scope);
    // Exclusive marker prevents replay. A failure after this point is not retried.
    fs.writeFileSync(p.marker, 'synthetic-install-once\n', {flag:'wx', mode:0o600});
    fs.mkdirSync(scope, {recursive:true, mode:0o700});
    fs.mkdirSync(p.package, {mode:0o700});
    fs.mkdirSync(path.join(p.package, 'bin'), {mode:0o700});
    fs.writeFileSync(path.join(p.package, 'package.json'), JSON.stringify({name:'@openai/codex',version:'1.2.3',bin:{codex:'bin/codex.js'}}), {flag:'wx',mode:0o600});
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
    if len(data) > 4096 or any(byte not in b'pr' for byte in data):
        raise InstallFailure('unsafe_query_evidence')
    return {'prefix_queries': data.count(b'p'), 'root_queries': data.count(b'r')}


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
    if product.exists() and any(p.name not in {'registry', 'registry-update.lock'} for p in product.iterdir()):
        raise InstallFailure('configuration_created')
    if any(p.name.lower() in forbidden or p.name.lower().startswith(('vault.', 'profile.')) for p in home.rglob('*')):
        raise InstallFailure('configuration_created')


def assert_plan(driver, fixture):
    fields = {'npm 包': '@openai/codex', '启动命令': 'codex', 'npm prefix': str(fixture['prefix']),
              'npm 全局目录': str(fixture['global']), '包目录': str(fixture['package']),
              '启动文件目录': str(fixture['bin']), 'npm 规范入口': str(fixture['manager']),
              '安装器 Node': str(fixture['node'])}
    for label, value in fields.items():
        if driver.text(DIALOG + "//div[dt[text()='" + label + "']]/dd") != value:
            raise InstallFailure('plan_mismatch')
    text = driver.text(DIALOG)
    for value in ('版本由当前 npm 配置选择', '生命周期脚本：禁用（ignore-scripts）', '仅安装软件，不应用 profile'):
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
    with fixture['manager'].open('a') as stream: stream.write('\n// harmless synthetic revision change\n')
    driver.click(CONFIRM)
    driver.wait(lambda: driver.elements("//*[@role='alert' and contains(@class, 'agent-notice')]") and '安装失败' in driver.text("//*[@role='alert' and contains(@class, 'agent-notice')]"))
    if 'Installation target changed since confirmation' not in driver.text("//*[@role='alert' and contains(@class, 'agent-notice')]"):
        raise InstallFailure('stale_rejection_not_verified')
    assert_absent(fixture); done('stale_revision')
    phase('install_confirmed')
    driver.wait(lambda: driver.enabled(PREVIEW))
    preview('install_fresh_preview'); driver.click(CONFIRM)
    driver.wait(lambda: fixture['marker'].exists())
    driver.wait(lambda: driver.elements("//*[@role='status' and contains(@class, 'agent-notice')]") and '软件安装操作已完成，未应用配置' in driver.text("//*[@role='status' and contains(@class, 'agent-notice')]"))
    assert_installed(fixture); done('confirmed_install')
    phase('install_observed_source')
    driver.click(DETAILS)
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
