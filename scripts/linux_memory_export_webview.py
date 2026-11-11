"""Native WebDriver memory editor acceptance over authored SQLite only.

No execute-script, direct IPC, test plugin, collectors or agent process. Called
only from the already guarded Linux hosted GUI lane, after its existing gates.
"""
import memory_export_fixture as fixture
import linux_codex_config_webview as config

STAGES = ('saved_outside_recent', 'multisearch_reset', 'cancel_edit', 'target_changes',
          'preview_cancel', 'stale_manual_rejected', 'stale_auto_rejected',
          'stale_row_rejected', 'replace_union', 'clear_manual_retains_auto')
CANARIES = (fixture.CANARY, config.CANARY)


class MemoryGuiFailure(Exception):
    pass


def control(name):
    return "//*[@data-testid='memory-export-" + name + "']"


def assert_coverage(records):
    if records != [{'stage': stage, 'verified': True} for stage in STAGES]:
        raise MemoryGuiFailure('incomplete_memory_coverage')


def exercise(driver, home, records, phase=lambda _: None, evidence=lambda _: None, process_fixture=None):
    phase('memory_author_fixture')
    fixture.prepare(home)
    watched = {}
    if process_fixture is not None:
        for path in (process_fixture['package'] / 'bin/codex.js', process_fixture['queries']):
            watched[path] = (path.read_bytes(), config.identity(path))

    def configuration(unrelated_only=False):
        state = config.snapshot(home)
        # SQLite coordination can change directory mtime/ctime/size, but must
        # not change ownership, permissions, identity or other file metadata.
        root = state['.agentdeck']
        state['.agentdeck'] = (*root[:6], 0, 0, 0, root[9])
        return {k: v for k, v in state.items()
                if k not in ('.agentdeck/memory.db-wal', '.agentdeck/memory.db-shm')
                and (not unrelated_only or not k.startswith('.agentdeck/memory'))}

    unrelated = configuration(unrelated_only=True)

    def clean():
        if process_fixture is not None:
            if (process_fixture['root'] / 'codex-config-process-marker').exists():
                raise MemoryGuiFailure('memory_unexpected_process')
            if any((path.read_bytes(), config.identity(path)) != before for path, before in watched.items()):
                raise MemoryGuiFailure('memory_process_fixture_changed')
        now = configuration(unrelated_only=True)
        if now != unrelated: raise MemoryGuiFailure('unrelated_configuration_changed')
        for value in (driver.text('//body'), driver.command('GET', '/source')):
            if not isinstance(value, str) or any(canary in value for canary in CANARIES):
                raise MemoryGuiFailure('private_content_exposed')
        collection = "//input[@aria-describedby='memory-collection-help']"
        if driver.command('GET', '/element/' + driver.one(collection) + '/selected') is not False:
            raise MemoryGuiFailure('collection_enabled')

    def snapshot():
        return (fixture.snapshot(home), configuration())

    def unchanged(before):
        clean()
        if snapshot() != before: raise MemoryGuiFailure('unconfirmed_memory_write')

    def activate(selector, key='\ue007'):
        def ready():
            count = len(driver.elements(selector))
            if count > 1: raise MemoryGuiFailure('ambiguous_memory_control')
            return count == 1 and driver.enabled(selector)
        driver.wait(ready)
        driver.command('POST', '/element/' + driver.one(selector) + '/value', {'text': key, 'value': [key]})

    def draft(ids):
        selector = control('draft') + '/li'
        driver.wait(lambda: len(driver.elements(selector)) == len(ids)
                    and all(len(driver.elements(selector + "[@data-memory-id='" + str(i) + "']")) == 1 for i in ids))

    def no_preview():
        driver.wait(lambda: not driver.elements(control('preview-result')) and not driver.elements(control('confirm')))

    def load(ids):
        activate(control('load'))
        driver.wait(lambda: len(driver.elements(control('saved'))) == 1 and driver.enabled(control('preview')))
        draft(ids); no_preview(); clean()

    def search(query, ids):
        driver.type("//*[@id='memory-search-input']", query)
        driver.wait(lambda: driver.value("//*[@id='memory-search-input']") == query)
        activate("//button[text()='搜索']")
        result = "//ul[@aria-label='记忆查询结果']//input[starts-with(@data-testid,'memory-select-')]"
        driver.wait(lambda: len(driver.elements(result)) == len(ids)
                    and all(len(driver.elements("//*[@data-testid='memory-select-" + str(i) + "']")) == 1 for i in ids))
        clean()

    def check(i):
        selector = "//*[@data-testid='memory-select-" + str(i) + "']"
        if driver.command('GET', '/element/' + driver.one(selector) + '/selected') is not False:
            raise MemoryGuiFailure('selection_precondition')
        activate(selector, '\ue00d')  # native Space, never JS click/IPC
        driver.wait(lambda: driver.command('GET', '/element/' + driver.one(selector) + '/selected') is True)

    def proposal(saved):
        load(saved)
        for i in saved:
            if i != 2: activate("//button[@aria-label='移除手动记忆 " + str(i) + "']")
        draft([2])
        search('Harbor', [4]); check(4); draft([2, 4])
        search('Meadow', [5]); check(5); draft([2, 4, 5])
        search('Harbor', [4])
        if driver.command('GET', '/element/' + driver.one("//*[@data-testid='memory-select-4']") + '/selected') is not True:
            raise MemoryGuiFailure('selection_lost_across_search')
        draft([2, 4, 5])

    def preview(manual_count, auto_count, union_count):
        before = snapshot()
        activate(control('preview'))
        driver.wait(lambda: len(driver.elements(control('preview-result'))) == 1 and driver.enabled(control('confirm')))
        expected = f'手动 {manual_count} 条 + 自动 {auto_count} 条，去重后 {union_count} 条。'
        if expected not in driver.text(control('preview-result')): raise MemoryGuiFailure('preview_union_mismatch')
        unchanged(before)

    def recorded(stage):
        clean(); evidence('memory-' + stage)
        records.append({'stage': stage, 'verified': True})

    phase('memory_open_workspace')
    activate("//*[@id='workspace-tab-memory']")
    driver.wait(lambda: len(driver.elements(control('unread'))) == 1)
    before = snapshot(); load([1, 2]); search('', list(range(17, 67)))
    draft([1, 2]); unchanged(before); recorded('saved_outside_recent')

    phase('memory_multisearch_reset')
    before = snapshot(); proposal([1, 2]); activate(control('reset')); draft([1, 2])
    unchanged(before); recorded('multisearch_reset')

    phase('memory_cancel_edit')
    before = snapshot(); proposal([1, 2]); activate(control('cancel'))
    driver.wait(lambda: len(driver.elements(control('unread'))) == 1 and not driver.enabled(control('preview')))
    draft([]); no_preview(); unchanged(before); recorded('cancel_edit')

    phase('memory_target_changes')
    before = snapshot(); load([1, 2]); preview(2, 2, 3)
    for target, ids in [('claude-code', [6]), ('opencode', [5]), ('codex', [1, 2])]:
        selector = "//*[@id='memory-export-target']"
        driver.click(selector + "/option[@value='" + target + "']")
        driver.wait(lambda: driver.value(selector) == target and len(driver.elements(control('unread'))) == 1)
        draft([]); no_preview(); load(ids)
    unchanged(before); recorded('target_changes')

    phase('memory_preview_cancel')
    before = snapshot(); proposal([1, 2]); preview(3, 2, 4)
    activate(control('preview-cancel')); no_preview(); draft([2, 4, 5]); unchanged(before)
    recorded('preview_cancel')

    for kind, saved, auto, count in [('manual', [1, 2], 2, 4), ('auto', [1, 2, 6], 2, 4), ('row', [1, 2, 6], 3, 5)]:
        phase('memory_stale_' + kind)
        proposal(saved); preview(3, auto, count)
        fixture.mutate(home, kind)
        before = snapshot()
        activate(control('confirm'))
        driver.wait(lambda: len(driver.elements(control('error'))) == 1 and len(driver.elements(control('unread'))) == 1)
        if '已变化' not in driver.text(control('error')): raise MemoryGuiFailure('stale_error_not_identified')
        no_preview()
        if driver.enabled(control('preview')): raise MemoryGuiFailure('stale_editor_remained_authorized')
        unchanged(before); fixture.verify(home, kind + '-drift')
        recorded('stale_' + kind + '_rejected')

    phase('memory_replace_union')
    proposal([1, 2, 6]); preview(3, 3, 5); activate(control('confirm'))
    driver.wait(lambda: len(driver.elements(control('status'))) == 1 and len(driver.elements(control('unread'))) == 1)
    if '不代表 Agent 已读取' not in driver.text(control('status')): raise MemoryGuiFailure('agent_consumption_boundary_missing')
    no_preview(); fixture.verify(home, 'replaced'); load([2, 4, 5]); recorded('replace_union')

    phase('memory_clear_manual')
    before = snapshot(); activate(control('clear')); draft([]); preview(0, 3, 3); unchanged(before)
    activate(control('confirm'))
    driver.wait(lambda: len(driver.elements(control('status'))) == 1 and len(driver.elements(control('unread'))) == 1)
    no_preview(); fixture.verify(home, 'cleared'); load([]); recorded('clear_manual_retains_auto')
    assert_coverage(records)
