#!/usr/bin/env python3
"""Real external WebDriver acceptance; no execute-script, mock or direct IPC."""
import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import tempfile
import threading
import time
import qoder_complete_fixtures as complete

ELEMENT = 'element-6066-11e4-a52e-4f735466cecf'
CANARY = 'QODER-SYNTHETIC-PRIVATE-CONTENT-NOT-FOR-UI'
LABELS = {
    'unknown': '无法确认缓存归属；请保留现场，不据此删除。',
    'incomplete': '缓存未完成；部分字节未获得完整性结论，请保留现场。',
    'conflict': '缓存存在冲突或不安全状态；请保留现场，不覆盖或强制修复。',
    'staged': '暂存归档与固定契约一致；未安装、未运行。',
    'unpacked': '解包缓存的归档与载荷符合固定契约；未安装、未运行。',
}

class Failure(Exception):
    pass

class Driver:
    def __init__(self):
        self.connection = http.client.HTTPConnection('127.0.0.1', 4444, timeout=30)
        self.session = None
        self.expected_input = None
        self.input_stage = None
        self.last_response = None

    def close(self):
        self.connection.close()

    def request(self, method, path, body=None):
        # One fixed loopback HTTP/1.1 connection; no proxy lookup, redirects or
        # automatic replay. Tauri 2.1.0 forwards Connection headers unchanged to
        # its pooled native client, so do not send urllib's forced close header.
        if method not in {'GET', 'POST', 'DELETE'} or not path.startswith('/') or path.startswith('//') or any(ord(c) < 32 or ord(c) == 127 for c in path) or any(token in path for token in ('execute', 'script')):
            raise Failure('forbidden_driver_endpoint')
        payload = None if body is None else json.dumps(body).encode()
        operation = path.rsplit('/', 1)[-1]
        allowed_operations = {'status', 'session', 'elements', 'click', 'clear', 'value', 'enabled', 'text', 'source'}
        self.last_response = {'operation': operation if operation in allowed_operations else 'other', 'method': method, 'received': False}
        try:
            self.connection.request(method, path, body=payload, headers={'Content-Type': 'application/json', 'Content-Length': str(len(payload) if payload is not None else 0)})
            response = self.connection.getresponse()
            try:
                raw = response.read(1_048_577)
                self.last_response.update({'received': True, 'status': response.status, **response_shape(raw)})
                if len(raw) > 1_048_576:
                    raise Failure('driver_response_limit')
                # read(amt), unlike read(), may return a short valid JSON body
                # at EOF without raising IncompleteRead. Never accept a frame
                # whose declared non-chunked body still has bytes outstanding.
                if not response.chunked and response.length not in (None, 0):
                    raise Failure('driver_incomplete_response')
                if 300 <= response.status <= 399:
                    raise Failure('driver_redirect_forbidden')
                if not 200 <= response.status <= 299:
                    raise Failure('driver_http_error')
            finally:
                response.close()
            decoded = json.loads(raw)
            if not isinstance(decoded, dict) or 'value' not in decoded:
                raise Failure('driver_response_schema')
            value = decoded['value']
            if isinstance(value, dict) and 'error' in value:
                raise Failure('webdriver_command_failed')
            return value
        except Exception:
            # This request may have taken effect. Close transport and propagate;
            # callers never replay a failed click, clear or key command.
            self.close()
            raise

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
        self.expected_input = text
        self.input_stage = 'locate_input'
        element = self.one(xpath)
        self.input_stage = 'clear_input'
        self.command('POST', '/element/' + element + '/clear', {})
        self.input_stage = 'send_keys'
        self.command('POST', '/element/' + element + '/value', {'text': text, 'value': list(text)})
        self.input_stage = 'send_keys_returned'

    def value(self, xpath):
        return self.command('GET', '/element/' + self.one(xpath) + '/property/value')

    def enabled(self, xpath):
        return self.command('GET', '/element/' + self.one(xpath) + '/enabled') is True

    def text(self, xpath):
        return self.command('GET', '/element/' + self.one(xpath) + '/text')

    def wait(self, predicate, seconds=20):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(0.1)
        raise Failure('ui_wait_expired')


def response_shape(raw):
    # Classify structure only: never keys, strings, IDs, URLs or backend messages.
    info = {'empty': len(raw) == 0, 'within_limit': len(raw) <= 1_048_576}
    if not info['within_limit']: return info
    try: value = json.loads(raw)
    except (ValueError, UnicodeError): return {**info, 'json': 'invalid'}
    def kind(item):
        if item is None: return 'null'
        if isinstance(item, bool): return 'boolean'
        if isinstance(item, dict): return 'object'
        if isinstance(item, list): return 'array'
        if isinstance(item, str): return 'string'
        if isinstance(item, (float, int)): return 'number'
        return 'other'
    info.update({'json': 'valid', 'root_type': kind(value)})
    if isinstance(value, dict):
        info['has_value'] = 'value' in value
        if 'value' in value:
            info['value_type'] = kind(value['value'])
            if isinstance(value['value'], dict): info['has_error'] = 'error' in value['value']
    return info


def exception_class(error):
    allowed = {'Failure', 'TypeError', 'AttributeError', 'KeyError', 'ValueError', 'JSONDecodeError',
               'UnicodeDecodeError', 'UnicodeEncodeError', 'HTTPError', 'URLError', 'TimeoutError',
               'ConnectionError', 'ConnectionResetError', 'BrokenPipeError', 'OSError',
               'RemoteDisconnected', 'BadStatusLine', 'IncompleteRead', 'LineTooLong', 'HTTPException'}
    name = type(error).__name__
    return name if name in allowed else 'OtherException'


def write_private(path, content):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, 'wb') as file:
        file.write(content)


def fixtures(root):
    paths = {}
    for kind in ('unknown', 'incomplete', 'conflict'):
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


def snapshot(root, exact=None):
    exact = exact or {}
    result = {}
    for path in [root, *sorted(root.rglob('*'))]:
        m = path.lstat()
        if path.is_symlink() or not (path.is_file() or path.is_dir()):
            raise Failure('unsafe_fixture')
        if path.is_dir() and (m.st_uid != os.geteuid() or stat.S_IMODE(m.st_mode) != 0o700):
            raise Failure('unsafe_fixture')
        contract = exact.get(path)
        try:
            digest = complete.file_digest(path,contract[0] if contract else 65536,contract) if path.is_file() else None
        except complete.PreparationFailure:
            raise Failure('unsafe_fixture') from None
        result[str(path.relative_to(root))] = (m.st_ino, m.st_mode, m.st_uid, m.st_nlink, m.st_size,
                                             m.st_mtime_ns, m.st_ctime_ns, digest)
    return result


def exercise(driver, paths, root, record, phase=lambda value: None, exact=None):
    tab = "//*[@role='tab' and @aria-label='Qoder 分发与校验']"
    phase('wait_qoder_tab')
    driver.wait(lambda: len(driver.elements(tab)) == 1)
    phase('click_qoder_tab')
    driver.click(tab)
    phase('assert_registry_startup_idle')
    if len(driver.elements("//*[@id='workspace-tab-updates' and @aria-label='注册表更新']")) != 1:
        raise Failure('startup_registry_not_idle')
    before = snapshot(root, exact)
    for kind, path in paths.items():
        phase(kind + '_type_path')
        driver.type("//*[@id='qoder-cache']", str(path))
        phase(kind + '_confirm_typed_path')
        driver.wait(lambda: driver.value("//*[@id='qoder-cache']") == str(path))
        phase(kind + '_wait_invalidation')
        driver.wait(lambda: not driver.elements("//section[@aria-label='Qoder 缓存诊断结果']"))
        phase(kind + '_wait_inspect_enabled')
        driver.wait(lambda: driver.enabled("//button[text()='只读诊断 Qoder 缓存']"))
        phase(kind + '_click_inspect')
        driver.click("//button[text()='只读诊断 Qoder 缓存']")
        phase(kind + '_wait_result')
        driver.wait(lambda: len(driver.elements("//section[@aria-label='Qoder 缓存诊断结果']")) == 1, seconds=90 if kind in ('staged', 'unpacked') else 20)
        phase(kind + '_assert_result_and_zero_write')
        text = driver.text("//section[@aria-label='Qoder 缓存诊断结果']")
        required_plan = '保留缓存，不执行操作。' if kind in ('staged','unpacked') else '保留现场，人工检查。'
        if LABELS[kind] not in text or '本报告不授权删除' not in text or required_plan not in text:
            raise Failure('unexpected_cache_state')
        if CANARY in driver.text('//body'):
            raise Failure('private_fixture_content_exposed')
        if before != snapshot(root, exact):
            raise Failure('cache_fixture_changed')
        record.append({'state': kind, 'real_webview_click_and_ipc': True, 'unchanged_bytes_metadata_members': True, 'complete_pinned_cache': kind in ('staged','unpacked')})
    phase('clear_and_assert')
    driver.click("//button[text()='清除 Qoder 选择与结果']")
    driver.wait(lambda: not driver.elements("//section[@aria-label='Qoder 缓存诊断结果']"))
    value = driver.command('GET', '/element/' + driver.one("//*[@id='qoder-cache']") + '/property/value')
    if value != '' or before != snapshot(root, exact):
        raise Failure('clear_or_final_zero_write_failed')



def created_cache_contract(directory, unpack):
    """Audit only the known files created by the actual native write command."""
    members = {'incomplete.json', 'complete.json', 'archive.tar.gz'}
    exact = {directory / 'archive.tar.gz': (complete.pinned.SIZE, complete.pinned.SHA256)}
    if unpack:
        members.add('qodercli')
        exact[directory / 'qodercli'] = (complete.PAYLOAD_BYTES, complete.PAYLOAD_SHA256)
    m = directory.lstat()
    if not stat.S_ISDIR(m.st_mode) or m.st_uid != os.geteuid() or stat.S_IMODE(m.st_mode) != 0o700 or {p.name for p in directory.iterdir()} != members:
        raise Failure('desktop_write_output_invalid')
    # snapshot enforces regular owner-only, single-link files; exact entries are
    # the only size exceptions. The payload remains data, never executable.
    snapshot(directory, exact)
    return exact


def exercise_cache_writes(driver, archive, root, records, phase=lambda value: None):
    source_input = "//*[@id='qoder-write-source']"
    destination_input = "//*[@id='qoder-write-destination']"
    preview = "//button[text()='预览 Qoder 缓存写入']"
    confirm = "//button[text()='确认创建 Qoder 缓存']"
    dialog = "//*[@role='dialog' and @aria-label='确认 Qoder 缓存写入']"
    result = "//section[@aria-label='Qoder 缓存写入结果']"
    alert = "//*[@role='alert' and starts-with(text(),'Qoder 缓存写入未确认完成。')]"
    read_result = "//section[@aria-label='Qoder 缓存诊断结果']"
    staged, unpacked = root / 'desktop-staged', root / 'desktop-unpacked'
    unused = root / 'confirmation-invalidated'
    for path in (staged, unpacked, unused):
        if path.exists() or path.is_symlink(): raise Failure('desktop_destination_not_new')
    source_contract = {archive: (complete.pinned.SIZE, complete.pinned.SHA256)}
    original_source = snapshot(archive.parent, source_contract)
    exact = {}

    def form(operation, source, destination):
        driver.wait(lambda: driver.enabled(source_input))
        driver.click("//button[text()='" + ('选择暂存归档' if operation == 'stage' else '选择解包缓存') + "']")
        for selector, value in ((source_input, source), (destination_input, destination)):
            driver.type(selector, str(value))
            driver.wait(lambda s=selector, v=str(value): driver.value(s) == v)
        if driver.elements(dialog) or driver.elements(confirm):
            raise Failure('desktop_confirmation_not_invalidated')
        driver.wait(lambda: not driver.elements(result) and not driver.elements(alert))

    def open_preview(operation, source, destination):
        driver.wait(lambda: driver.enabled(preview))
        driver.click(preview)
        driver.wait(lambda: len(driver.elements(dialog)) == 1)
        text = driver.text(dialog)
        required = ['操作：' + ('暂存归档' if operation == 'stage' else '解包缓存'),
                    '固定目标：linux-amd64-baseline', '版本：1.1.85',
                    '来源：' + str(source), '全新目录：' + str(destination)]
        if any(value not in text for value in required):
            raise Failure('desktop_confirmation_details_mismatch')

    phase('desktop_confirmation_negative_paths')
    form('stage', archive, staged)
    initial = snapshot(root)
    open_preview('stage', archive, staged)
    driver.click("//button[text()='取消 Qoder 写入确认']")
    driver.wait(lambda: not driver.elements(dialog))
    if snapshot(root) != initial or staged.exists():
        raise Failure('desktop_unconfirmed_write')
    open_preview('stage', archive, staged)
    driver.type(destination_input, str(unused))
    driver.wait(lambda: driver.value(destination_input) == str(unused))
    driver.wait(lambda: not driver.elements(dialog) and not driver.elements(confirm))
    if snapshot(root) != initial or unused.exists():
        raise Failure('desktop_unconfirmed_write')
    records.append({'operation': 'confirmation', 'cancel_and_input_change_no_write': True})

    for operation, source, destination, kind in [('stage', archive, staged, 'staged'), ('unpack', staged, unpacked, 'unpacked')]:
        phase('desktop_' + operation + '_confirm')
        form(operation, source, destination)
        open_preview(operation, source, destination)
        preserved_source = snapshot(source, exact) if operation == 'unpack' else original_source
        driver.wait(lambda: driver.enabled(confirm))
        driver.click(confirm)  # Never replay a possibly completed write.
        phase('desktop_' + operation + '_wait_completion')
        driver.wait(lambda: bool(driver.elements(result) or driver.elements(alert)), seconds=120)
        if driver.elements(alert) or LABELS[kind] not in driver.text(result):
            raise Failure('desktop_write_not_confirmed')
        driver.wait(lambda: driver.enabled(source_input))
        exact.update(created_cache_contract(destination, operation == 'unpack'))
        if (snapshot(source, exact) if operation == 'unpack' else snapshot(archive.parent, source_contract)) != preserved_source:
            raise Failure('desktop_write_modified_source')
        phase('desktop_' + operation + '_diagnose_zero_write')
        before = snapshot(root, exact)
        driver.type("//*[@id='qoder-cache']", str(destination))
        driver.wait(lambda: driver.value("//*[@id='qoder-cache']") == str(destination))
        driver.wait(lambda: not driver.elements(read_result))
        driver.wait(lambda: driver.enabled("//button[text()='只读诊断 Qoder 缓存']"))
        driver.click("//button[text()='只读诊断 Qoder 缓存']")
        driver.wait(lambda: len(driver.elements(read_result)) == 1, seconds=90)
        text = driver.text(read_result)
        if any(value not in text for value in (LABELS[kind], '保留缓存，不执行操作。', '本报告不授权删除')) or snapshot(root, exact) != before:
            raise Failure('desktop_diagnosis_or_zero_write_failed')
        records.append({'operation': operation, 'real_webview_write_and_diagnosis': True, 'fixed_files_hashes_modes_verified': True, 'diagnosis_zero_write': True})

        phase('desktop_' + operation + '_reject_existing_destination')
        form(operation, source, destination)
        open_preview(operation, source, destination)
        before = snapshot(root, exact)
        driver.wait(lambda: driver.enabled(confirm))
        driver.click(confirm)
        driver.wait(lambda: bool(driver.elements(alert) or driver.elements(result)), seconds=120)
        if not driver.elements(alert) or driver.elements(result):
            raise Failure('desktop_existing_destination_not_rejected')
        driver.wait(lambda: driver.enabled(source_input))
        if snapshot(root, exact) != before or snapshot(archive.parent, source_contract) != original_source:
            raise Failure('desktop_existing_destination_changed')
        records.append({'operation': operation + '_existing', 'rejected_without_overwrite': True})
    phase('desktop_clear_and_final_zero_write')
    before = snapshot(root, exact)
    driver.click("//button[text()='清除 Qoder 选择与结果']")
    driver.wait(lambda: not driver.elements(result) and not driver.elements(alert) and not driver.elements(read_result) and not driver.elements(dialog))
    if snapshot(root, exact) != before or snapshot(archive.parent, source_contract) != original_source or unused.exists():
        raise Failure('desktop_final_zero_write_failed')
    if CANARY in driver.text('//body'):
        raise Failure('private_fixture_content_exposed')


def failure_code(error):
    if isinstance(error, complete.PreparationFailure): return 'complete_cache_preparation_failed'
    if isinstance(error, Failure):
        allowed = {'driver_redirect_forbidden', 'forbidden_driver_endpoint', 'driver_response_limit',
                   'webdriver_command_failed', 'driver_response_schema', 'driver_http_error', 'driver_incomplete_response', 'no_session', 'missing_or_ambiguous_control', 'ui_wait_expired',
                   'unsafe_fixture', 'fixture_size_changed', 'unexpected_cache_state',
                   'private_fixture_content_exposed', 'cache_fixture_changed',
                   'clear_or_final_zero_write_failed', 'driver_exited', 'driver_not_ready', 'startup_registry_not_idle', 'incomplete_gui_coverage', 'desktop_write_output_invalid', 'desktop_destination_not_new',
                   'desktop_confirmation_not_invalidated', 'desktop_confirmation_details_mismatch', 'desktop_unconfirmed_write',
                   'desktop_write_not_confirmed', 'desktop_write_modified_source', 'desktop_diagnosis_or_zero_write_failed',
                   'desktop_existing_destination_not_rejected', 'desktop_existing_destination_changed', 'desktop_final_zero_write_failed'}
        return str(error) if str(error) in allowed else 'harness_failure'
    if isinstance(error, (ConnectionError, http.client.HTTPException)): return 'webdriver_transport_error'
    if isinstance(error, TimeoutError): return 'webdriver_timeout'
    return 'unexpected_harness_error'


def capture_failure_dom(driver, evidence):
    # Read-only standard WebDriver endpoints, never executeScript. Inspect the
    # full bounded response for the canary BEFORE truncating retained evidence.
    report = {}
    if not driver.session:
        report['session'] = 'unavailable'
    else:
        # Retain only agreement/length/boolean facts, never an untrusted input value.
        try:
            value = driver.value("//*[@id='qoder-cache']")
            report['cache_input'] = {
                'state': 'observed',
                'is_string': isinstance(value, str),
                'matches_expected': isinstance(value, str) and value == driver.expected_input,
                'length_at_most_4096': isinstance(value, str) and len(value.encode('utf-8')) <= 4096,
            }
        except Exception as error:
            report['cache_input'] = {'state': 'unavailable', 'code': failure_code(error)}
        try:
            report['inspect_button'] = {'enabled': driver.enabled("//button[text()='只读诊断 Qoder 缓存']")}
        except Exception as error:
            report['inspect_button'] = {'state': 'unavailable', 'code': failure_code(error)}
        for name, read in [('body', lambda: driver.text('//body')), ('source', lambda: driver.command('GET', '/source'))]:
            try:
                value = read()
                if not isinstance(value, str): raise Failure('harness_failure')
                encoded = value.encode('utf-8')
                if CANARY in value:
                    report[name] = {'state': 'private_content_suppressed'}
                else:
                    report[name] = {'state': 'captured', 'truncated': len(encoded) > 65536,
                                    'text': encoded[:65536].decode('utf-8', errors='ignore')}
            except Exception as error:
                report[name] = {'state': 'unavailable', 'code': failure_code(error)}
    (evidence / 'failure-dom.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))


def capture_log(stream, output):
    # Drain the process pipe even past the retained cap to avoid deadlock.
    kept = 0
    while chunk := stream.read(16384):
        chunk = chunk[:max(0, 1_048_576 - kept)]
        output.extend(chunk); kept += len(chunk)


def guarded_run(application, driver_binary, native_driver, evidence, cli=None, archive=None):
    if (os.environ.get('GITHUB_ACTIONS'), os.environ.get('RUNNER_ENVIRONMENT'), os.environ.get('RUNNER_OS')) != ('true', 'github-hosted', 'Linux'):
        raise Failure('requires_hosted_linux_ci')
    for name in ('WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS', 'WEBKIT_FORCE_SANDBOX'):
        if name in os.environ:
            raise Failure('sandbox_override_forbidden')
    if cli is None or archive is None: raise Failure('complete_cache_inputs_required')
    for binary in (application, driver_binary, native_driver, cli):
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
    client = Driver(); process = None; records = []; write_records = []; passed = False
    log = bytearray(); logger = None
    diagnostic = {'phase': 'start_driver', 'failure_code': None}
    def phase(value): diagnostic['phase'] = value
    try:
        phase('prepare_official_complete_caches')
        completed_paths,exact=complete.prepare(cli,archive,fixture_root,env)
        paths.update(completed_paths)
        phase('start_driver')
        process = subprocess.Popen([str(driver_binary), '--native-driver', str(native_driver), '--native-host', '127.0.0.1'], env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        logger = threading.Thread(target=capture_log, args=(process.stdout, log), daemon=True)
        logger.start()
        phase('driver_readiness')
        # A bounded startup probe only; session/command failures never skip.
        deadline = time.monotonic() + 15
        while True:
            if process.poll() is not None:
                raise Failure('driver_exited')
            try:
                client.request('GET', '/status'); break
            except (OSError, http.client.HTTPException):
                if time.monotonic() > deadline: raise Failure('driver_not_ready') from None
                time.sleep(0.2)
        phase('create_session')
        session = client.request('POST', '/session', {'capabilities': {'alwaysMatch': {'browserName': 'wry', 'tauri:options': {'application': str(application)}}}})
        client.session = session['sessionId']
        exercise(client, paths, fixture_root, records, phase, exact)
        if [entry.get('state') for entry in records] != ['unknown','incomplete','conflict','staged','unpacked']:
            raise Failure('incomplete_gui_coverage')
        readonly_before = snapshot(fixture_root, exact)
        write_root = temporary / 'desktop-writes'; write_root.mkdir(mode=0o700)
        exercise_cache_writes(client, archive, write_root, write_records, phase)
        if [entry.get('operation') for entry in write_records] != ['confirmation', 'stage', 'stage_existing', 'unpack', 'unpack_existing'] or snapshot(fixture_root, exact) != readonly_before:
            raise Failure('incomplete_gui_coverage')
        phase('acceptance_complete')
        passed = True
    except Exception as error:
        diagnostic['failure_code'] = failure_code(error)
        diagnostic['exception_class'] = exception_class(error)
        diagnostic['input_stage'] = client.input_stage
        diagnostic['last_response'] = client.last_response
        capture_failure_dom(client, evidence)
        raise
    finally:
        if client.session:
            try: client.command('DELETE', '')
            except Exception: passed = False
        client.close()
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
        (evidence / 'result.json').write_text(json.dumps({'passed': passed, **diagnostic, 'checks': records, 'desktop_write_checks': write_records, 'desktop_cache_writes_verified': passed and len(write_records)==5, 'synthetic_partial_states_only': False, 'complete_cache_gui_verified': passed and len(records)==5, 'qoder_executed': False, 'sandbox_overrides': False, 'archive_sha256': complete.pinned.SHA256, 'archive_bytes': complete.pinned.SIZE, 'payload_sha256': complete.PAYLOAD_SHA256, 'payload_bytes': complete.PAYLOAD_BYTES, 'required_file_mode': '0600', 'required_directory_mode': '0700'}, indent=2))
    if not passed:
        raise Failure('acceptance_not_confirmed')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    for name in ('application', 'driver', 'native-driver', 'evidence', 'cli', 'archive'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    try:
        guarded_run(args.application, args.driver, args.native_driver, args.evidence, args.cli, args.archive)
    except Exception:
        # Do not dump backend bodies, fixture contents or arbitrary paths.
        print('Linux real WebView acceptance failed; inspect CI driver/platform evidence.', file=__import__('sys').stderr)
        raise SystemExit(1) from None
