#!/usr/bin/env python3
"""Real external WebDriver acceptance; no execute-script, mock or direct IPC."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request

ELEMENT = 'element-6066-11e4-a52e-4f735466cecf'
CANARY = 'QODER-SYNTHETIC-PRIVATE-CONTENT-NOT-FOR-UI'
LABELS = {
    'unknown': '无法确认缓存归属；请保留现场，不据此删除。',
    'incomplete': '缓存未完成；部分字节未获得完整性结论，请保留现场。',
    'conflict': '缓存存在冲突或不安全状态；请保留现场，不覆盖或强制修复。',
}

class Failure(Exception):
    pass

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise Failure('driver_redirect_forbidden')


class Driver:
    def __init__(self):
        self.client = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
        self.session = None

    def request(self, method, path, body=None):
        # Only the local external driver. No external URL or script endpoint API.
        if not path.startswith('/') or any(token in path for token in ('execute', 'script')):
            raise Failure('forbidden_driver_endpoint')
        payload = None if body is None else json.dumps(body).encode()
        req = urllib.request.Request('http://127.0.0.1:4444' + path, data=payload, method=method,
                                     headers={'Content-Type': 'application/json'})
        with self.client.open(req, timeout=30) as response:
            raw = response.read(1_048_577)
            if len(raw) > 1_048_576:
                raise Failure('driver_response_limit')
        value = json.loads(raw).get('value')
        if isinstance(value, dict) and 'error' in value:
            raise Failure('webdriver_command_failed')
        return value

    def command(self, method, path, body=None):
        if not self.session:
            raise Failure('no_session')
        return self.request(method, '/session/' + self.session + path, body)

    def elements(self, xpath):
        return self.command('POST', '/elements', {'using': 'xpath', 'value': xpath})

    def one(self, xpath):
        found = self.elements(xpath)
        if len(found) != 1:
            raise Failure('missing_or_ambiguous_control')
        return found[0][ELEMENT]

    def click(self, xpath):
        self.command('POST', '/element/' + self.one(xpath) + '/click', {})

    def type(self, xpath, text):
        element = self.one(xpath)
        self.command('POST', '/element/' + element + '/clear', {})
        self.command('POST', '/element/' + element + '/value', {'text': text, 'value': list(text)})

    def text(self, xpath):
        return self.command('GET', '/element/' + self.one(xpath) + '/text')

    def wait(self, predicate, seconds=20):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(0.1)
        raise Failure('ui_wait_expired')


def write_private(path, content):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as file:
        file.write(content)


def fixtures(root):
    paths = {}
    for kind in LABELS:
        directory = root / kind
        directory.mkdir(mode=0o700)
        paths[kind] = directory
        if kind != 'unknown':
            record = {'format': 1, 'owner': 'agentdeck', 'state': 'incomplete_archive_staging',
                      'version': '1.1.85', 'target': 'linux-amd64-baseline', 'installed': False}
            write_private(directory / 'incomplete.json', json.dumps(record).encode())
            write_private(directory / 'archive.tar.gz', CANARY.encode())
        if kind == 'conflict':
            write_private(directory / 'completion.pending', CANARY.encode())
    return paths


def snapshot(root):
    result = {}
    for path in [root, *sorted(root.rglob('*'))]:
        m = path.lstat()
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise Failure('unsafe_fixture')
        if path.is_file() and m.st_size > 65536:
            raise Failure('fixture_size_changed')
        digest = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
        result[str(path.relative_to(root))] = (m.st_ino, m.st_mode, m.st_uid, m.st_nlink, m.st_size,
                                             m.st_mtime_ns, m.st_ctime_ns, digest)
    return result


def exercise(driver, paths, root, record):
    tab = "//*[@role='tab' and @aria-label='Qoder 分发与校验']"
    driver.wait(lambda: len(driver.elements(tab)) == 1)
    driver.click(tab)
    before = snapshot(root)
    for kind, path in paths.items():
        driver.type("//*[@id='qoder-cache']", str(path))
        driver.wait(lambda: not driver.elements("//section[@aria-label='Qoder 缓存诊断结果']"))
        driver.click("//button[text()='只读诊断 Qoder 缓存']")
        driver.wait(lambda: len(driver.elements("//section[@aria-label='Qoder 缓存诊断结果']")) == 1)
        text = driver.text("//section[@aria-label='Qoder 缓存诊断结果']")
        if LABELS[kind] not in text or '本报告不授权删除' not in text:
            raise Failure('unexpected_cache_state')
        if CANARY in driver.text('//body'):
            raise Failure('private_fixture_content_exposed')
        if before != snapshot(root):
            raise Failure('cache_fixture_changed')
        record.append({'state': kind, 'real_webview_click_and_ipc': True, 'unchanged_bytes_metadata_members': True})
    driver.click("//button[text()='清除 Qoder 选择与结果']")
    driver.wait(lambda: not driver.elements("//section[@aria-label='Qoder 缓存诊断结果']"))
    value = driver.command('GET', '/element/' + driver.one("//*[@id='qoder-cache']") + '/property/value')
    if value != '' or before != snapshot(root):
        raise Failure('clear_or_final_zero_write_failed')


def capture_log(stream, output):
    # Drain the process pipe even past the retained cap to avoid deadlock.
    kept = 0
    while chunk := stream.read(16384):
        chunk = chunk[:max(0, 1_048_576 - kept)]
        output.extend(chunk); kept += len(chunk)


def guarded_run(application, driver_binary, native_driver, evidence):
    if (os.environ.get('GITHUB_ACTIONS'), os.environ.get('RUNNER_ENVIRONMENT'), os.environ.get('RUNNER_OS')) != ('true', 'github-hosted', 'Linux'):
        raise Failure('requires_hosted_linux_ci')
    for name in ('WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS', 'WEBKIT_FORCE_SANDBOX'):
        if name in os.environ:
            raise Failure('sandbox_override_forbidden')
    for binary in (application, driver_binary, native_driver):
        if not binary.is_absolute() or not binary.is_file() or not os.access(binary, os.X_OK):
            raise Failure('missing_explicit_executable')
    evidence.mkdir(exist_ok=False)
    temporary = Path(tempfile.mkdtemp(prefix='agentdeck-webview-', dir=os.environ['RUNNER_TEMP']))
    home = temporary / 'home'; home.mkdir(mode=0o700)
    fixture_root = temporary / 'fixtures'; fixture_root.mkdir(mode=0o700)
    paths = fixtures(fixture_root)
    env = dict(os.environ, HOME=str(home))
    for key, name in [('XDG_CONFIG_HOME', 'config'), ('XDG_CACHE_HOME', 'cache'), ('XDG_DATA_HOME', 'data'), ('XDG_STATE_HOME', 'state'), ('XDG_RUNTIME_DIR', 'runtime')]:
        directory = home / name; directory.mkdir(mode=0o700); env[key] = str(directory)
    client = Driver(); process = None; records = []; passed = False
    log = bytearray(); logger = None
    try:
        process = subprocess.Popen([str(driver_binary), '--native-driver', str(native_driver), '--native-host', '127.0.0.1'], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        logger = threading.Thread(target=capture_log, args=(process.stdout, log), daemon=True)
        logger.start()
        # A bounded startup probe only; session/command failures never skip.
        deadline = time.monotonic() + 15
        while True:
            if process.poll() is not None:
                raise Failure('driver_exited')
            try:
                client.request('GET', '/status'); break
            except (urllib.error.URLError, ConnectionError):
                if time.monotonic() > deadline: raise Failure('driver_not_ready') from None
                time.sleep(0.2)
        session = client.request('POST', '/session', {'capabilities': {'alwaysMatch': {'browserName': 'wry', 'tauri:options': {'application': str(application)}}}})
        client.session = session['sessionId']
        exercise(client, paths, fixture_root, records)
        passed = True
    finally:
        if client.session:
            try: client.command('DELETE', '')
            except Exception: passed = False
        if process is not None:
            try: os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError: pass
            try: process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL); process.wait(timeout=5)
        if logger is not None:
            logger.join(timeout=5)
            if logger.is_alive(): passed = False
        retained = bytes(log) if logger is None or not logger.is_alive() else b'Log capture did not finish.\n'
        if CANARY.encode() in retained:
            retained = b'Synthetic private content suppressed.\n'; passed = False
        (evidence / 'driver.log').write_bytes(retained)
        (evidence / 'result.json').write_text(json.dumps({'passed': passed, 'checks': records, 'synthetic_partial_states_only': True, 'complete_cache_gui_not_tested': True, 'qoder_executed': False, 'sandbox_overrides': False}, indent=2))
    if not passed:
        raise Failure('acceptance_not_confirmed')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    for name in ('application', 'driver', 'native-driver', 'evidence'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    try:
        guarded_run(args.application, args.driver, args.native_driver, args.evidence)
    except Exception:
        # Do not dump backend bodies, fixture contents or arbitrary paths.
        print('Linux real WebView acceptance failed; inspect CI driver/platform evidence.', file=__import__('sys').stderr)
        raise SystemExit(1) from None
