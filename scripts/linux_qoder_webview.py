#!/usr/bin/env python3
"""Real external WebDriver acceptance; no execute-script, mock or direct IPC."""
import argparse
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import signal
import stat
import subprocess
import tempfile
import threading
import time
import qoder_complete_fixtures as complete
import dsh_distribution_smoke as dsh_pinned
import linux_qwen_webview as qwen
import linux_install_webview as install_gui
import linux_codex_config_webview as codex_config

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
        self.wheel_diagnostic = None

    def close(self):
        self.connection.close()

    def request(self, method, path, body=None):
        # One fixed loopback HTTP/1.1 connection; no proxy lookup, redirects or
        # automatic replay. Tauri 2.1.0 forwards Connection headers unchanged to
        # its pooled native client, so do not send urllib's forced close header.
        if method not in {'GET', 'POST', 'DELETE'} or not path.startswith('/') or path.startswith('//') or any(ord(c) < 32 or ord(c) == 127 for c in path) or any(token in path for token in ('execute', 'script')):
            raise Failure('forbidden_driver_endpoint')
        if 'actions' in path.split('/') and (not self.session or path != '/session/' + self.session + '/actions' or method not in {'POST', 'DELETE'}):
            raise Failure('forbidden_driver_endpoint')
        payload = None if body is None else json.dumps(body).encode()
        operation = path.rsplit('/', 1)[-1]
        allowed_operations = {'status', 'session', 'elements', 'click', 'clear', 'value', 'enabled', 'displayed', 'rect', 'actions', 'text', 'source'}
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

    def wheel_scroll_workspace_top(self):
        element = self.one("//main[@id='workspace-content']")
        def position():
            value = self.command('GET', '/element/' + element + '/property/scrollTop')
            if type(value) not in (int, float) or not 0 <= value <= 1_000_000:
                raise Failure('workspace_scroll_invalid')
            return value
        self.wheel_diagnostic = {'initial': None, 'steps': []}
        current = position()
        self.wheel_diagnostic['initial'] = current
        if current == 0:
            return
        delta = -math.ceil(current)
        action = {'actions': [{'type': 'wheel', 'id': 'workspace-wheel', 'actions': [{
            'type': 'scroll', 'origin': {ELEMENT: element}, 'x': 0, 'y': 0,
            'deltaX': 0, 'deltaY': delta, 'duration': 200}]}]}
        step = {'index': 1, 'before': current, 'last': current, 'delta_y': delta,
                'polls': 0, 'posted': False, 'progressed': False, 'reached_zero': False}
        self.wheel_diagnostic['steps'].append(step)
        try:
            # A single real wheel gesture sized from observed bounded scrollTop.
            # No second gesture is issued, even if the first only partly scrolls.
            self.command('POST', '/actions', action)
            step['posted'] = True
            def at_top():
                nonlocal current
                observed = position()
                step['last'] = observed
                step['polls'] = min(step['polls'] + 1, 1000)
                if observed > current: raise Failure('workspace_scroll_invalid')
                current = observed
                step['progressed'] = current < step['before']
                step['reached_zero'] = current == 0
                return current == 0
            self.wait(at_top, seconds=3)
        except Exception:
            failed_response = self.last_response
            try:
                self.command('DELETE', '/actions')  # cleanup only; never resend scroll
            except Exception:
                pass
            self.last_response = failed_response
            raise
        self.command('DELETE', '/actions')

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
    # Classify structure and fixed W3C error enums only; never retain backend messages.
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
            if isinstance(value['value'], dict):
                info['has_error'] = 'error' in value['value']
                error = value['value'].get('error')
                allowed = {'element click intercepted', 'element not interactable', 'stale element reference',
                           'invalid argument', 'no such element', 'no such window', 'invalid session id',
                           'unknown error', 'timeout', 'unsupported operation', 'unknown command'}
                if 'error' in value['value']:
                    info['webdriver_error'] = error if isinstance(error, str) and error in allowed else 'unrecognized'
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


def scope_fixtures(root):
    user = root / 'user'; user.mkdir(mode=0o700)
    project = root / 'project'; project.mkdir(mode=0o700)
    shared = CANARY + '-shared-server'
    entry = {'command': CANARY + '-never-execute', 'env': {'TOKEN': CANARY}}
    settings = {'mcpServers': {shared: entry, CANARY + '-user-only': entry},
                'permissions': {'allow': [CANARY]}, 'model': CANARY, CANARY + '-field': CANARY}
    mcp = {'mcpServers': {shared: entry, CANARY + '-project-only': entry}}
    paths = {'user': user / 'settings.json', 'project_mcp': project / '.mcp.json'}
    write_private(paths['user'], ('// synthetic read-only fixture\n' + json.dumps(settings)).encode())
    write_private(paths['project_mcp'], json.dumps(mcp).encode())
    return paths


def dsh_fixtures(root, archive):
    complete.file_digest(archive, dsh_pinned.SIZE, (dsh_pinned.SIZE, dsh_pinned.SHA256))
    with os.fdopen(os.open(archive, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as source:
        metadata = os.fstat(source.fileno())
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or metadata.st_size != dsh_pinned.SIZE:
            raise Failure('unsafe_fixture')
        data = source.read(dsh_pinned.SIZE + 1)
    dsh_pinned.validate_archive(data)
    paths = {}
    for kind in ('valid', 'corrupt'):
        directory = root / kind; directory.mkdir(mode=0o700)
        paths[kind] = directory / dsh_pinned.NAME
        content = data if kind == 'valid' else CANARY.encode() + data[len(CANARY):]
        write_private(paths[kind], content)
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
        phase('desktop_' + operation + '_recover_attempt_diagnosis')
        history = "//section[@aria-label='Qoder 上次提交记录']"
        fill = "//button[text()='将上次提交目标填入只读诊断']"
        # Editing a draft must not retarget the immutable last actual request.
        driver.type(destination_input, str(unused))
        driver.wait(lambda: driver.value(destination_input) == str(unused))
        if '已提交目标：' + str(destination) not in driver.text(history):
            raise Failure('desktop_attempt_target_changed')
        driver.wait(lambda: driver.enabled(fill))
        driver.click(fill)
        driver.wait(lambda: driver.value("//*[@id='qoder-cache']") == str(destination))
        if driver.elements(read_result) or snapshot(root, exact) != before:
            raise Failure('desktop_attempt_fill_not_readonly')
        driver.wait(lambda: driver.enabled("//button[text()='只读诊断 Qoder 缓存']"))
        driver.click("//button[text()='只读诊断 Qoder 缓存']")
        driver.wait(lambda: len(driver.elements(read_result)) == 1, seconds=90)
        recovery = '可将这个暂存目录作为后续解包来源' if operation == 'stage' else '仅保留此解包缓存，不激活或执行'
        expected = (LABELS[kind], '本次只读诊断目录：' + str(destination), recovery, '本报告不授权删除')
        if any(value not in driver.text(read_result) for value in expected) or snapshot(root, exact) != before or snapshot(archive.parent, source_contract) != original_source:
            raise Failure('desktop_attempt_recovery_invalid')
        records.append({'operation': operation + '_existing', 'rejected_without_overwrite': True,
                        'immutable_attempt_target_diagnosed': True, 'recovery_guidance_verified': True,
                        'recovery_diagnosis_zero_write': True})
    phase('desktop_clear_and_final_zero_write')
    before = snapshot(root, exact)
    retained_inputs = [driver.value(path) for path in (source_input, destination_input, "//*[@id='qoder-cache']")]
    driver.wheel_scroll_workspace_top()
    if retained_inputs != [driver.value(path) for path in (source_input, destination_input, "//*[@id='qoder-cache']")]:
        raise Failure('workspace_scroll_changed_inputs')
    driver.wait(lambda: driver.enabled("//button[text()='清除 Qoder 选择与结果']"))
    driver.click("//button[text()='清除 Qoder 选择与结果']")
    driver.wait(lambda: not driver.elements(result) and not driver.elements(alert) and not driver.elements(read_result) and not driver.elements(dialog) and not driver.elements("//section[@aria-label='Qoder 上次提交记录']"))
    if snapshot(root, exact) != before or snapshot(archive.parent, source_contract) != original_source or unused.exists():
        raise Failure('desktop_final_zero_write_failed')
    if CANARY in driver.text('//body'):
        raise Failure('private_fixture_content_exposed')


def exercise_scopes(driver, paths, root, records, phase=lambda _: None):
    result = "//section[@aria-label='Qoder 作用域检查结果']"
    selectors = {name: "//*[@id='qoder-scope-" + name + "']" for name in ('user', 'project', 'local', 'mcp')}
    before = snapshot(root)
    phase('scope_explicit_synthetic_inspection')
    for name, path in (('user', paths['user']), ('mcp', paths['project_mcp'])):
        driver.wait(lambda: driver.enabled(selectors[name]))
        driver.type(selectors[name], str(path))
        driver.wait(lambda: driver.value(selectors[name]) == str(path))
    inspect = "//button[text()='只读检查 Qoder 作用域']"
    driver.wait(lambda: driver.enabled(inspect))
    driver.click(inspect)
    driver.wait(lambda: len(driver.elements(result)) == 1)
    text = driver.text(result)
    if any(value not in text for value in ('重复 MCP 标识数：1', '重复只表示潜在重叠，不是冲突或生效优先级', '未选作用域不视为空', '不执行、不写入、不备份、不恢复')):
        raise Failure('scope_summary_invalid')
    for scope in ('user_settings', 'project_mcp'):
        text = driver.text("//section[@aria-label='Qoder 作用域 " + scope + "']")
        if '已读取固定摘要' not in text or 'MCP 条目数：2' not in text:
            raise Failure('scope_summary_invalid')
        expected_flags = ('MCP 分区：存在', '钩子分区：未见', '安全分区：未见') + (('权限分区：存在', '模型分区：存在', '其他顶层字段：存在') if scope == 'user_settings' else ('权限分区：未见', '模型分区：未见', '其他顶层字段：未见'))
        if any(flag not in text for flag in expected_flags): raise Failure('scope_summary_invalid')
    for scope in ('project_settings', 'local_settings'):
        text = driver.text("//section[@aria-label='Qoder 作用域 " + scope + "']")
        if '未选择' not in text or 'MCP 条目数' in text:
            raise Failure('scope_summary_invalid')
    for value in (driver.text('//body'), driver.command('GET', '/source')):
        if not isinstance(value, str): raise Failure('scope_summary_invalid')
        if CANARY in value: raise Failure('private_fixture_content_exposed')
    if snapshot(root) != before: raise Failure('scope_fixture_changed')
    records.append({'scope': 'synthetic_settings_and_project_mcp', 'counts_verified': True,
                    'potential_overlap_only': True, 'no_secret_rendered': True, 'zero_write': True})
    phase('scope_input_invalidation_and_clear')
    driver.type(selectors['user'], str(paths['user']))
    driver.wait(lambda: not driver.elements(result))
    driver.click("//button[text()='清除 Qoder 作用域选择与结果']")
    driver.wait(lambda: not driver.elements(result) and all(driver.value(selector) == '' for selector in selectors.values()))
    if snapshot(root) != before: raise Failure('scope_fixture_changed')
    records.append({'scope': 'input_invalidation_and_clear', 'old_result_invalidated': True, 'zero_write': True})


def exercise_dsh_root(driver, paths, root, records, phase=lambda _: None):
    before = snapshot(root)
    tab = "//*[@role='tab' and @aria-label='DSH 声明盘点']"
    field = "//*[@id='dsh-root-archive']"
    plan = "//section[@aria-label='DSH 根包计划结果']"
    verified = "//section[@aria-label='DSH 根归档校验结果']"
    alert = "//*[@role='alert' and starts-with(text(),'DSH 根包未确认。')]"
    verify_button = "//button[text()='只读校验 DSH 根归档']"
    phase('dsh_root_plan_and_valid_archive')
    driver.wait(lambda: len(driver.elements(tab)) == 1); driver.click(tab)
    driver.wait(lambda: driver.enabled(field)); driver.type(field, str(paths['valid']))
    driver.wait(lambda: driver.value(field) == str(paths['valid']))
    driver.click("//button[text()='查看 DSH 根包计划']")
    driver.wait(lambda: len(driver.elements(plan)) == 1)
    text = driver.text(plan)
    if any(value not in text for value in (dsh_pinned.VERSION, dsh_pinned.NAME, dsh_pinned.URL, dsh_pinned.SHA256, dsh_pinned.SRI, '预发布', '90 个直接依赖，其中 8 项范围', '未验证独立签名')):
        raise Failure('dsh_gui_plan_invalid')
    driver.wait(lambda: driver.enabled(verify_button)); driver.click(verify_button)
    driver.wait(lambda: len(driver.elements(verified)) == 1)
    text = driver.text(verified)
    if driver.elements(alert) or any(value not in text for value in ('根归档与固定长度、SHA256 和 SHA512 一致。', '已读取字节：25197', '依赖未解析、未安装、未运行、平台兼容性未验证')):
        raise Failure('dsh_gui_success_invalid')
    if snapshot(root) != before: raise Failure('dsh_gui_fixture_changed')
    records.append({'state': 'official_root_verified', 'real_gui_plan_and_verify': True, 'zero_write': True})
    phase('dsh_root_corrupt_archive_rejected')
    driver.type(field, str(paths['corrupt']))
    driver.wait(lambda: driver.value(field) == str(paths['corrupt']) and not driver.elements(verified) and not driver.elements(plan))
    driver.wait(lambda: driver.enabled(verify_button)); driver.click(verify_button)
    driver.wait(lambda: len(driver.elements(alert)) == 1)
    if driver.elements(verified) or driver.elements(plan): raise Failure('dsh_gui_corrupt_accepted')
    for value in (driver.text('//body'), driver.command('GET', '/source')):
        if not isinstance(value, str): raise Failure('dsh_gui_success_invalid')
        if CANARY in value: raise Failure('private_fixture_content_exposed')
    if snapshot(root) != before: raise Failure('dsh_gui_fixture_changed')
    records.append({'state': 'corrupt_root_rejected', 'old_success_invalidated': True, 'no_secret_rendered': True, 'zero_write': True})
    phase('dsh_root_clear')
    driver.wait(lambda: driver.enabled("//button[text()='清除 DSH 根包选择与结果']"))
    driver.click("//button[text()='清除 DSH 根包选择与结果']")
    driver.wait(lambda: driver.value(field) == '' and not driver.elements(alert) and not driver.elements(plan) and not driver.elements(verified))
    if snapshot(root) != before: raise Failure('dsh_gui_fixture_changed')
    records.append({'state': 'cleared', 'zero_write': True})


def failure_code(error):
    if isinstance(error, codex_config.CodexConfigFailure): return 'codex_config_gui_acceptance_failed'
    if isinstance(error, install_gui.InstallFailure): return 'reviewed_install_gui_acceptance_failed'
    if isinstance(error, qwen.QwenFailure): return 'qwen_gui_acceptance_failed'
    if isinstance(error, complete.PreparationFailure): return 'complete_cache_preparation_failed'
    if isinstance(error, Failure):
        allowed = {'driver_redirect_forbidden', 'forbidden_driver_endpoint', 'driver_response_limit',
                   'webdriver_command_failed', 'driver_response_schema', 'driver_http_error', 'driver_incomplete_response', 'no_session', 'missing_or_ambiguous_control', 'ui_wait_expired',
                   'unsafe_fixture', 'fixture_size_changed', 'unexpected_cache_state',
                   'private_fixture_content_exposed', 'cache_fixture_changed',
                   'clear_or_final_zero_write_failed', 'driver_exited', 'driver_not_ready', 'startup_registry_not_idle', 'incomplete_gui_coverage', 'desktop_write_output_invalid', 'desktop_destination_not_new',
                   'desktop_confirmation_not_invalidated', 'desktop_confirmation_details_mismatch', 'desktop_unconfirmed_write',
                   'desktop_write_not_confirmed', 'desktop_write_modified_source', 'desktop_diagnosis_or_zero_write_failed',
                   'desktop_existing_destination_not_rejected', 'desktop_existing_destination_changed', 'desktop_final_zero_write_failed', 'dsh_gui_plan_invalid', 'dsh_gui_success_invalid', 'dsh_gui_corrupt_accepted', 'dsh_gui_fixture_changed', 'scope_summary_invalid', 'scope_fixture_changed', 'desktop_attempt_target_changed', 'desktop_attempt_fill_not_readonly', 'desktop_attempt_recovery_invalid', 'workspace_scroll_limit', 'workspace_scroll_invalid', 'workspace_scroll_changed_inputs'}
        return str(error) if str(error) in allowed else 'harness_failure'
    if isinstance(error, (ConnectionError, http.client.HTTPException)): return 'webdriver_transport_error'
    if isinstance(error, TimeoutError): return 'webdriver_timeout'
    return 'unexpected_harness_error'


def bounded_rect(value):
    # Geometry is diagnostic only, not a click target or permission to replay.
    if not isinstance(value, dict): raise Failure('harness_failure')
    result = {}
    for key in ('x', 'y', 'width', 'height'):
        number = value.get(key)
        if type(number) not in (int, float) or not -1_000_000 <= number <= 1_000_000 or not math.isfinite(number):
            raise Failure('harness_failure')
        if key in ('width', 'height') and number < 0: raise Failure('harness_failure')
        result[key] = number
    return result


def capture_clear_geometry(driver):
    report = {}
    try:
        element = driver.one("//button[text()='清除 Qoder 选择与结果']")
        if not isinstance(element, str): raise Failure('harness_failure')
    except Exception as error:
        return {'state': 'unavailable', 'code': failure_code(error)}
    for name, path in [('enabled', '/element/' + element + '/enabled'),
                       ('displayed', '/element/' + element + '/displayed'),
                       ('element_rect', '/element/' + element + '/rect'), ('window_rect', '/window/rect')]:
        try:
            value = driver.command('GET', path)
            if name in ('enabled', 'displayed'):
                if type(value) is not bool: raise Failure('harness_failure')
                report[name] = value
            else:
                report[name] = bounded_rect(value)
        except Exception as error:
            report[name] = {'state': 'unavailable', 'code': failure_code(error)}
    return report


def capture_scroll_facts(driver):
    # Fixed known elements/properties only, no IDs, source text or DOM dumps.
    report = {}
    for name, xpath in [('workspace', "//main[@id='workspace-content']"), ('body', '//body'),
                        ('write_source', "//*[@id='qoder-write-source']"),
                        ('write_destination', "//*[@id='qoder-write-destination']"),
                        ('cache_input', "//*[@id='qoder-cache']")]:
        try:
            element = driver.one(xpath)
            if not isinstance(element, str): raise Failure('harness_failure')
            facts = {}
            for prop in ('scrollTop', 'scrollHeight', 'clientHeight'):
                value = driver.command('GET', '/element/' + element + '/property/' + prop)
                if type(value) not in (int, float) or not 0 <= value <= 1_000_000:
                    raise Failure('workspace_scroll_invalid')
                facts[prop] = value
            report[name] = facts
        except Exception as error:
            report[name] = {'state': 'unavailable', 'code': failure_code(error)}
    return report


def capture_failure_dom(driver, evidence):
    # Read-only standard WebDriver endpoints, never executeScript. Inspect the
    # full bounded response for the canary BEFORE truncating retained evidence.
    report = {}
    if not driver.session:
        report['session'] = 'unavailable'
    else:
        report['scroll_facts'] = capture_scroll_facts(driver)
        report['clear_geometry'] = capture_clear_geometry(driver)
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


def guarded_run(application, driver_binary, native_driver, evidence, cli=None, archive=None, dsh_archive=None):
    if (os.environ.get('GITHUB_ACTIONS'), os.environ.get('RUNNER_ENVIRONMENT'), os.environ.get('RUNNER_OS')) != ('true', 'github-hosted', 'Linux'):
        raise Failure('requires_hosted_linux_ci')
    for name in ('WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS', 'WEBKIT_FORCE_SANDBOX'):
        if name in os.environ:
            raise Failure('sandbox_override_forbidden')
    if cli is None or archive is None or dsh_archive is None: raise Failure('complete_cache_inputs_required')
    for binary in (application, driver_binary, native_driver, cli):
        if not binary.is_absolute() or not binary.is_file() or not os.access(binary, os.X_OK):
            raise Failure('missing_explicit_executable')
    evidence.mkdir(exist_ok=False)
    temporary = Path(tempfile.mkdtemp(prefix='agentdeck-webview-', dir=os.environ['RUNNER_TEMP']))
    home = temporary / 'home'; home.mkdir(mode=0o700)
    fixture_root = temporary / 'fixtures'; fixture_root.mkdir(mode=0o700)
    paths = fixtures(fixture_root)
    env = dict(os.environ, HOME=str(home), CODEX_HOME=str(home / '.codex'))
    for key, name in [('XDG_CONFIG_HOME', 'config'), ('XDG_CACHE_HOME', 'cache'), ('XDG_DATA_HOME', 'data'), ('XDG_STATE_HOME', 'state'), ('XDG_RUNTIME_DIR', 'runtime')]:
        directory = home / name; directory.mkdir(mode=0o700); env[key] = str(directory)
    client = Driver(); process = None; records = []; write_records = []; scope_records = []; dsh_records = []; qwen_records = []; qwen_current_records = []; install_records = []; install_trace = []; codex_config_records = []; codex_animation_records = []; passed = False
    log = bytearray(); logger = None
    diagnostic = {'phase': 'start_driver', 'failure_code': None}
    def phase(value): diagnostic['phase'] = value
    try:
        phase('prepare_official_complete_caches')
        completed_paths,exact=complete.prepare(cli,archive,fixture_root,env)
        paths.update(completed_paths)
        phase('prepare_synthetic_install')
        install_fixture = install_gui.prepare(temporary / 'install-fixture', env['PATH'])
        env['PATH'] = install_fixture['path']
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
        scope_root = temporary / 'scope-fixtures'; scope_root.mkdir(mode=0o700)
        scope_paths = scope_fixtures(scope_root)
        exercise_scopes(client, scope_paths, scope_root, scope_records, phase)
        if len(scope_records) != 2 or snapshot(fixture_root, exact) != readonly_before:
            raise Failure('incomplete_gui_coverage')
        dsh_source_before = snapshot(dsh_archive.parent)
        dsh_root = temporary / 'dsh-fixtures'; dsh_root.mkdir(mode=0o700)
        dsh_paths = dsh_fixtures(dsh_root, dsh_archive)
        exercise_dsh_root(client, dsh_paths, dsh_root, dsh_records, phase)
        if len(dsh_records) != 3 or snapshot(dsh_archive.parent) != dsh_source_before:
            raise Failure('incomplete_gui_coverage')
        qwen_root = temporary / 'qwen-fixtures'; qwen_root.mkdir(mode=0o700)
        qwen.exercise(client, qwen_root, qwen_records, write_private, phase)
        qwen.assert_coverage(qwen_records)
        qwen_current_root = temporary / 'qwen-current-fixtures'; qwen_current_root.mkdir(mode=0o700)
        qwen.exercise_current(client, qwen_current_root, qwen_current_records, write_private, phase)
        qwen.assert_current_coverage(qwen_current_records)
        if snapshot(fixture_root, exact) != readonly_before or snapshot(dsh_archive.parent) != dsh_source_before:
            raise Failure('incomplete_gui_coverage')
        install_gui.exercise(client, install_fixture, home, install_records, phase, install_trace,
                             lambda stage: install_gui.capture_geometry(client, evidence, stage))
        install_gui.assert_coverage(install_records)
        codex_config.exercise(client, home, install_fixture, codex_config_records, write_private, phase)
        codex_config.assert_coverage(codex_config_records)
        codex_config.exercise_animations(client, home, install_fixture, codex_animation_records, write_private, phase)
        codex_config.assert_animation_coverage(codex_animation_records)
        phase('acceptance_complete')
        passed = True
    except Exception as error:
        diagnostic['failure_code'] = failure_code(error)
        diagnostic['exception_class'] = exception_class(error)
        diagnostic['input_stage'] = client.input_stage
        diagnostic['last_response'] = client.last_response
        diagnostic['wheel_scroll'] = client.wheel_diagnostic
        try:
            diagnostic['synthetic_home_configuration_facts'] = install_gui.configuration_facts(home)
        except Exception:
            diagnostic['synthetic_home_configuration_facts'] = {'state': 'unavailable'}
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
        (evidence / 'result.json').write_text(json.dumps({'passed': passed, **diagnostic, 'checks': records, 'codex_animations_checks': codex_animation_records, 'codex_animations_gui_verified': passed and codex_animation_records == [{'stage': name, 'verified': True} for name in codex_config.ANIMATION_STAGES], 'codex_animations_agent_executed': False, 'codex_config_plan_checks': codex_config_records, 'codex_config_plan_gui_verified': passed and codex_config_records == [{'stage': name, 'verified': True} for name in codex_config.STAGES], 'codex_config_plan_agent_executed': False, 'reviewed_install_checks': install_records, 'reviewed_install_query_trace': install_trace, 'synthetic_install_gui_verified': passed and install_records == [{'stage': name, 'verified': True} for name in install_gui.STAGES], 'real_agent_package_installed_or_executed': False, 'registry_or_lifecycle_compatibility_verified': False, 'qwen_owned_checks': qwen_records, 'qwen_owned_gui_verified': passed and qwen_records == [{'stage': name, 'verified': True} for name in qwen.STAGES], 'qwen_current_checks': qwen_current_records, 'qwen_current_gui_verified': passed and qwen_current_records == [{'stage': name, 'verified': True} for name in qwen.CURRENT_STAGES], 'qwen_current_version': '0.27.2', 'qwen_legacy_version': '0.27.0', 'qwen_executed': False, 'desktop_write_checks': write_records, 'scope_checks': scope_records, 'dsh_root_checks': dsh_records, 'dsh_root_gui_verified': passed and len(dsh_records)==3, 'dsh_executed': False, 'synthetic_scope_gui_verified': passed and len(scope_records)==2, 'desktop_cache_writes_verified': passed and len(write_records)==5, 'synthetic_partial_states_only': False, 'complete_cache_gui_verified': passed and len(records)==5, 'qoder_executed': False, 'sandbox_overrides': False, 'archive_sha256': complete.pinned.SHA256, 'archive_bytes': complete.pinned.SIZE, 'payload_sha256': complete.PAYLOAD_SHA256, 'payload_bytes': complete.PAYLOAD_BYTES, 'required_file_mode': '0600', 'required_directory_mode': '0700'}, indent=2))
    if not passed:
        raise Failure('acceptance_not_confirmed')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    for name in ('application', 'driver', 'native-driver', 'evidence', 'cli', 'archive', 'dsh-archive'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    try:
        guarded_run(args.application, args.driver, args.native_driver, args.evidence, args.cli, args.archive, args.dsh_archive)
    except Exception:
        # Do not dump backend bodies, fixture contents or arbitrary paths.
        print('Linux real WebView acceptance failed; inspect CI driver/platform evidence.', file=__import__('sys').stderr)
        raise SystemExit(1) from None
