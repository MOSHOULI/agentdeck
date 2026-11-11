"""Real external WebDriver native archive verification; no direct IPC or execution."""
import os
import stat
import dsh_native_smoke as pinned

PACKAGE = "//*[@id='dsh-native-package']"
ARCHIVE = "//*[@id='dsh-native-archive']"
VERIFY = "//*[@id='dsh-native-verify']"
CLEAR = "//*[@id='dsh-native-clear']"
RESULT = "//section[@aria-label='DSH 原生依赖归档校验结果']"
ALERT = "//*[@role='alert' and contains(.,'DSH 原生依赖归档未确认')]"
STAGES = tuple(package + ':' + stage for package in pinned.IDS
               for stage in ('valid', 'selection_invalidated', 'mismatch', 'path_invalidated', 'corrupt', 'clear'))


class NativeGuiFailure(Exception):
    pass


def prepare(source, root):
    before = pinned.snapshot(source)
    items = pinned.load_review()
    if set(before) != {'.', *(a['archive_name'] for a in items)}:
        raise NativeGuiFailure('source_members')
    root.mkdir(mode=0o700, exist_ok=False)
    corrupt = root / 'corrupt'; corrupt.mkdir(mode=0o700)
    paths = {}
    for a in items:
        path = source / a['archive_name']
        with os.fdopen(os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK), 'rb') as stream:
            m = os.fstat(stream.fileno())
            if not stat.S_ISREG(m.st_mode) or m.st_nlink != 1 or m.st_size != a['archive_bytes']:
                raise NativeGuiFailure('unsafe_source')
            data = stream.read(a['archive_bytes'] + 1)
        pinned.validate_archive(data, a['id'])
        good, bad = root / a['archive_name'], corrupt / a['archive_name']
        pinned.write_new(good, data)
        pinned.write_new(bad, bytes([data[0] ^ 1]) + data[1:])
        paths[a['id']] = (good, bad)
    if pinned.snapshot(source) != before:
        raise NativeGuiFailure('source_changed')
    return paths


def assert_coverage(records):
    if records != [{'stage': stage, 'verified': True, 'zero_write': True} for stage in STAGES]:
        raise NativeGuiFailure('incomplete_coverage')


def exercise(driver, source, root, paths, records, phase=lambda _: None):
    before = pinned.snapshot(root)
    source_before = pinned.snapshot(source)

    def click(selector):
        driver.wait(lambda: len(driver.elements(selector)) == 1 and driver.enabled(selector))
        driver.click(selector)

    def select(package):
        click(PACKAGE + "/option[@value='" + package + "']")
        driver.wait(lambda: driver.value(PACKAGE) == package)

    def fill(path):
        driver.type(ARCHIVE, str(path))
        driver.wait(lambda: driver.value(ARCHIVE) == str(path))

    def cleared():
        driver.wait(lambda: not driver.elements(RESULT) and not driver.elements(ALERT))

    def finish(package, stage):
        if pinned.snapshot(root) != before or pinned.snapshot(source) != source_before:
            raise NativeGuiFailure('unexpected_write')
        records.append({'stage': package + ':' + stage, 'verified': True, 'zero_write': True})

    def verify(a):
        click(VERIFY)
        driver.wait(lambda: len(driver.elements(RESULT)) == 1)
        text = driver.text(RESULT)
        expected = (a['id'], a['package'], a['version'], a['archive_name'], str(a['archive_bytes']),
                    a['archive_sha256'], a['integrity'], '普通成员数：' + str(len(a['members'])),
                    '依赖闭包未完成、依赖未解析、不可据此安装、未执行生命周期脚本、未加载原生代码、未验证运行或独立签名，平台兼容性未验证。', '所选原生依赖归档静态校验通过。')
        if driver.elements(ALERT) or any(value not in text for value in expected):
            raise NativeGuiFailure('result_mismatch')

    click("//*[@role='tab' and @aria-label='DSH 声明盘点']")
    for i, a in enumerate(pinned.load_review()):
        package = a['id']; good, bad = paths[package]
        phase('dsh_native_' + package)
        select(package); fill(good); cleared(); verify(a); finish(package, 'valid')
        select(pinned.IDS[(i + 1) % len(pinned.IDS)])
        cleared(); finish(package, 'selection_invalidated'); fill(good)
        click(VERIFY); driver.wait(lambda: len(driver.elements(ALERT)) == 1)
        if driver.elements(RESULT): raise NativeGuiFailure('mismatch_accepted')
        finish(package, 'mismatch')
        select(package); cleared(); fill(good); verify(a)
        fill(bad); cleared(); finish(package, 'path_invalidated')
        click(VERIFY); driver.wait(lambda: len(driver.elements(ALERT)) == 1)
        if driver.elements(RESULT): raise NativeGuiFailure('corruption_accepted')
        finish(package, 'corrupt')
        fill(good); cleared(); verify(a)
        click(CLEAR)
        driver.wait(lambda: driver.value(PACKAGE) == '' and driver.value(ARCHIVE) == '')
        cleared(); finish(package, 'clear')
    assert_coverage(records)
