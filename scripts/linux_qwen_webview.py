"""Synthetic Qwen files through AgentDeck's real GUI/IPC, never Qwen execution.

Only called by the hosted, sandbox-retaining Linux WebView harness. Filesystem
edits here simulate an external editor in the private fixture, not product IPC.
"""
import hashlib
import json
import os
import stat

import qoder_complete_fixtures as complete

# Share the runner's canary so its bounded log and failure-DOM suppression applies.
CANARY = 'QODER-SYNTHETIC-PRIVATE-CONTENT-NOT-FOR-UI'
RECEIPT = '.agentdeck-qwen-vim.json'
LOCK = '.agentdeck-qwen-vim.lock'
BASE = {'settings.json', 'package.json'}
STAGES = ('recovery_no_receipt', 'preview_cancel_clear', 'stale_preview', 'apply',
          'update_cancel', 'update_stale', 'update_false', 'update_true',
          'recovery_restore_field', 'restore_cancel', 'restore_conflict',
          'recovery_conflict', 'recovery_cleanup_receipt', 'restore', 'clear')
MANIFEST = b'{"name":"@qwen-code/qwen-code","version":"0.27.0"}'
PANEL = "//section[@aria-label='Qwen Vim 模式管理']"
SETTINGS = "//*[@id='qwen-settings']"
RUNTIME = "//*[@id='qwen-manifest']"
DIALOG = PANEL + "//*[@role='dialog']"
ERROR = '未能确认操作完成。请检查文件与所有权记录；不要假定配置未改变或重复提交。重新预览或处理冲突后再试。'


class QwenFailure(Exception):
    """No configuration content is placed in exception messages."""


def document(enabled):
    return {'$version': 4, 'general': {'vimMode': enabled, 'other': 'preserve-me'},
            'security': {'token': CANARY}}


def original():
    return json.dumps(document(False), separators=(',', ':')).encode()


def digest(data):
    return hashlib.sha256(data).hexdigest()


def audit(root, members):
    """Exact members, owner, modes, links, bounded hashes and metadata."""
    m = root.lstat()
    if not stat.S_ISDIR(m.st_mode) or m.st_uid != os.geteuid() or stat.S_IMODE(m.st_mode) != 0o700:
        raise QwenFailure('unsafe_directory')
    if {p.name for p in root.iterdir()} != members:
        raise QwenFailure('unexpected_members')
    result = {}
    for path in [root, *sorted(root.iterdir())]:
        m = path.lstat()
        try:
            hashed = None if path == root else complete.file_digest(path, 65536)
        except complete.PreparationFailure:
            raise QwenFailure('unsafe_file') from None
        result[path.name] = (m.st_dev, m.st_ino, m.st_mode, m.st_uid, m.st_nlink,
                             m.st_size, m.st_mtime_ns, m.st_ctime_ns, hashed)
    if result['package.json'][-1] != digest(MANIFEST):
        raise QwenFailure('manifest_changed')
    if LOCK in members and result[LOCK][-1] != digest(b''):
        raise QwenFailure('lock_changed')
    return result


def read_checked(root, name, members):
    before = audit(root, members)
    with os.fdopen(os.open(root / name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
        data = stream.read(65537)
    if len(data) > 65536 or audit(root, members) != before or digest(data) != before[name][-1]:
        raise QwenFailure('file_changed')
    return data


def rewrite_settings(root, data, members):
    audit(root, members)
    fd = os.open(root / 'settings.json', os.O_WRONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'wb') as stream:
        m = os.fstat(stream.fileno())
        if not stat.S_ISREG(m.st_mode) or m.st_nlink != 1 or m.st_uid != os.geteuid() or stat.S_IMODE(m.st_mode) != 0o600:
            raise QwenFailure('unsafe_file')
        stream.truncate(0)
        stream.write(data)
    audit(root, members)


def assert_document(root, enabled, members):
    data = read_checked(root, 'settings.json', members)
    expected = document(enabled)
    actual = json.loads(data)
    # bool is an int subclass in Python: require an actual boolean as well.
    if actual != expected or type(actual['general']['vimMode']) is not bool:
        raise QwenFailure('settings_semantics')
    return data


def assert_receipt(root, data, applied, *, updated=False, enabled=True):
    expected = {'format': 2 if updated else 1, 'phase': 'applied',
                'selection_binding': digest(os.fsencode(root / 'settings.json') + b'\0' + os.fsencode(root / 'package.json')),
                'manifest_digest': digest(MANIFEST), 'restore_digest': None,
                'before': False, 'after': enabled, 'had_general': True,
                'before_digest': digest(original()), 'after_digest': digest(applied)}
    actual = json.loads(data)
    if actual != expected or any(type(actual[key]) is not type(value) for key, value in expected.items()):
        raise QwenFailure('receipt_mismatch')


def assert_coverage(records):
    if records != [{'stage': name, 'verified': True} for name in STAGES]:
        raise QwenFailure('incomplete_coverage')


def exercise(driver, root, records, write_private, phase=lambda _: None):
    write_private(root / 'settings.json', original())
    write_private(root / 'package.json', MANIFEST)
    members = set(BASE)
    initial = audit(root, members)
    manifest_image = initial['package.json']

    def button(label):
        return "//button[text()='" + label + "']"

    def wait_button(label):
        selector = button(label)
        def ready():
            count = len(driver.elements(selector))
            if count > 1:
                raise QwenFailure('ambiguous_control')
            return count == 1 and driver.enabled(selector)
        driver.wait(ready)
        return selector

    def click(label):
        selector = wait_button(label)
        driver.click(selector)  # Never replay a failed mutation command.

    def no_confirmation():
        driver.wait(lambda: not driver.elements(DIALOG))

    def cleared_results():
        # React may commit path/cancel invalidation after the input event. Poll
        # read-only DOM state, never replay the clearing/cancelling command.
        driver.wait(lambda: not driver.elements(DIALOG)
                    and not driver.elements(PANEL + "//*[@role='status' or @role='alert']")
                    and not driver.elements(PANEL + "//section[@aria-label='Vim 恢复状态观察']"))

    def clean_dom():
        for value in (driver.text('//body'), driver.command('GET', '/source')):
            if not isinstance(value, str) or CANARY in value:
                raise QwenFailure('private_content_exposed')

    def finish(stage):
        clean_dom()
        if audit(root, members)['package.json'] != manifest_image:
            raise QwenFailure('manifest_changed')
        records.append({'stage': stage, 'verified': True})

    def unchanged(before):
        if audit(root, members) != before:
            raise QwenFailure('unexpected_write')

    def fill():
        for selector, path in ((SETTINGS, root / 'settings.json'), (RUNTIME, root / 'package.json')):
            driver.wait(lambda s=selector: driver.enabled(s))
            driver.type(selector, str(path))
            driver.wait(lambda s=selector, p=path: driver.value(s) == str(p))
        cleared_results()

    def preview():
        click('预览 Vim 变更')
        driver.wait(lambda: len(driver.elements(PANEL + "//*[@role='dialog' and @aria-label='确认 Vim 变更']")) == 1)
        if '当前值：关闭；目标：启用。' not in driver.text(DIALOG):
            raise QwenFailure('preview_mismatch')
        clean_dom()

    def message(text):
        driver.wait(lambda: text in driver.text(PANEL))
        no_confirmation()

    def recovery(state, label):
        phase('qwen_recovery_' + state)
        before = audit(root, members)
        click('只读检查 Vim 恢复状态')
        region = PANEL + "//section[@aria-label='Vim 恢复状态观察']"
        driver.wait(lambda: len(driver.elements(region)) == 1)
        if label not in driver.text(region):
            raise QwenFailure('recovery_mismatch')
        no_confirmation(); unchanged(before); finish('recovery_' + state)

    phase('qwen_open')
    tab = "//*[@role='tab' and @aria-label='Qwen 配置检查']"
    driver.wait(lambda: len(driver.elements(tab)) == 1); driver.click(tab)
    fill()
    recovery('no_receipt', '未发现所有权记录；不代表配置从未被修改。')
    phase('qwen_preview_cancel_clear')
    checkbox = PANEL + "//input[@type='checkbox']"
    if driver.command('GET', '/element/' + driver.one(checkbox) + '/property/checked') is not False:
        raise QwenFailure('initial_target_mismatch')
    driver.click(checkbox)
    preview(); unchanged(initial)
    click('取消 Vim 变更'); cleared_results(); unchanged(initial)
    preview(); click('清除路径与结果')
    driver.wait(lambda: driver.value(SETTINGS) == '' and driver.value(RUNTIME) == '')
    cleared_results(); unchanged(initial); finish('preview_cancel_clear')

    phase('qwen_stale_preview')
    fill(); preview()
    rewrite_settings(root, original() + b'\n', members)
    before = audit(root, members)
    click('确认应用 Vim 变更'); message(ERROR)
    unchanged(before); finish('stale_preview')
    rewrite_settings(root, original(), members)

    phase('qwen_apply')
    preview(); click('确认应用 Vim 变更')
    message('所选文件的 Vim 模式字段已应用。尚未验证 Qwen 实际运行效果。')
    members |= {RECEIPT, LOCK}
    applied = assert_document(root, True, members)
    receipt = read_checked(root, RECEIPT, members)
    assert_receipt(root, receipt, applied)
    if CANARY.encode() in receipt:
        raise QwenFailure('private_content_exposed')
    finish('apply')
    def update_preview(current, target):
        click('预览更新受管 Vim 值')
        driver.wait(lambda: len(driver.elements(PANEL + "//*[@role='dialog' and @aria-label='确认更新受管 Vim 值']")) == 1)
        expected = '当前值：' + ('启用' if current else '关闭') + '；目标：' + ('启用' if target else '关闭') + '。'
        if expected not in driver.text(DIALOG) or '首次管理前的原值' not in driver.text(DIALOG):
            raise QwenFailure('update_preview_mismatch')
        clean_dom()

    phase('qwen_update_cancel')
    driver.click(checkbox)  # true -> false target; this does not write the fixture.
    before = audit(root, members)
    update_preview(True, False); unchanged(before)
    click('取消 Vim 变更'); cleared_results(); unchanged(before); finish('update_cancel')

    phase('qwen_update_stale')
    update_preview(True, False)
    rewrite_settings(root, applied + b'\n', members)
    before = audit(root, members)
    click('确认更新受管 Vim 值'); message(ERROR)
    unchanged(before); finish('update_stale')
    rewrite_settings(root, applied, members)

    # Both directions use the same original receipt, not newly acquired ownership.
    for target in (False, True):
        stage = 'update_true' if target else 'update_false'
        phase('qwen_' + stage)
        if target:
            driver.click(checkbox)
        before = audit(root, members)
        update_preview(not target, target); unchanged(before)
        click('确认更新受管 Vim 值')
        message('受管 Vim 值已更新；首次原值仍保留供撤销。尚未验证 Qwen 实际运行效果。')
        applied = assert_document(root, target, members)
        receipt = read_checked(root, RECEIPT, members)
        assert_receipt(root, receipt, applied, updated=True, enabled=target)
        if CANARY.encode() in receipt:
            raise QwenFailure('private_content_exposed')
        finish(stage)
    recovery('restore_field', '观察到可撤销的受管字段；尚未执行撤销。')

    phase('qwen_restore_cancel')
    before = audit(root, members)
    click('撤销受管 Vim 设置'); click('取消撤销')
    cleared_results(); unchanged(before); finish('restore_cancel')

    phase('qwen_restore_conflict')
    click('撤销受管 Vim 设置')
    wait_button('确认撤销 Vim 设置')
    rewrite_settings(root, original() + b'\n', members)
    before = audit(root, members)
    click('确认撤销 Vim 设置'); message(ERROR)
    unchanged(before); finish('restore_conflict')
    recovery('conflict', '受管字段存在冲突，当前观察不支持撤销。')
    rewrite_settings(root, original(), members)
    recovery('cleanup_receipt', '字段处于原状态，仅待清理所有权记录；尚未清理。')

    phase('qwen_restore')
    rewrite_settings(root, applied, members)
    if read_checked(root, RECEIPT, members) != receipt:
        raise QwenFailure('receipt_changed')
    click('撤销受管 Vim 设置'); click('确认撤销 Vim 设置')
    message('受管字段已撤销。尚未验证 Qwen 实际运行效果。')
    members -= {RECEIPT}
    assert_document(root, False, members); finish('restore')
    phase('qwen_clear')
    before = audit(root, members)
    click('清除路径与结果')
    driver.wait(lambda: driver.value(SETTINGS) == '' and driver.value(RUNTIME) == '')
    cleared_results(); unchanged(before); finish('clear')
    assert_coverage(records)
