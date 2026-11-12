"""Real WebView private deployment of fixed official Qoder bytes, never a launch.

The CLI lane separately traces all exec calls. This GUI lane proves explicit
UI/IPC file operations and source/config preservation, not process isolation,
runtime compatibility, login or service acceptance. No script/IPC injection.
"""
import os
from pathlib import Path

import qoder_private_deploy_acceptance as deployment
import qoder_complete_fixtures as complete

STAGES = ('preview_cancel', 'input_invalidates_preview', 'stale_destination_rejected',
          'confirmed_private_deployment', 'existing_destination_rejected',
          'clear_preserves_files', 'navigation_invalidates_preview',
          'deployment_inspection_verified', 'deployment_inspection_edit_invalidated',
          'deployment_inspection_conflict', 'deployment_inspection_clear_invalidated',
          'deployment_inspection_cancel_discards')
PANEL = "//section[@aria-label='Qoder 私有可执行部署']"
SOURCE = "//*[@id='qoder-deploy-source']"
DESTINATION = "//*[@id='qoder-deploy-destination']"
DIALOG = PANEL + "//*[@role='dialog' and @aria-label='确认 Qoder 私有可执行部署']"
ATTEMPT = PANEL + "//section[@aria-label='Qoder 私有部署提交记录']"
RESULT = ATTEMPT + "//*[@role='status']"
ALERT = PANEL + "/p[@role='alert']"
FAILED = ATTEMPT + "//*[@role='alert']"
BUTTONS = {
    'preview': '预览 Qoder 私有部署',
    'confirm': '确认部署 Qoder 可执行文件',
    'cancel': '取消 Qoder 部署确认',
    'clear': '清除 Qoder 部署选择与结果',
}
INSPECT_PATH = "//*[@id='qoder-inspect-deployment']"
INSPECT_RESULT = "//*[@data-testid='qoder-deployment-inspection-result']"
INSPECT_BUTTON = "//button[text()='只读检查 Qoder 私有部署']"
INSPECT_CLEAR = "//button[text()='清除 Qoder 部署检查']"
SUCCESS = 'Qoder 私有可执行文件部署已完成；未激活、未运行。'


class DeploymentGuiFailure(Exception):
    """Fixed categories only."""


def assert_coverage(records):
    if (records != [{'stage': name, 'verified': True} for name in STAGES]
            or any(type(item.get('verified')) is not bool for item in records)):
        raise DeploymentGuiFailure('incomplete_deployment_gui_coverage')


def activate(driver, selector):
    def ready():
        count = len(driver.elements(selector))
        if count > 1: raise DeploymentGuiFailure('ambiguous_deployment_control')
        return count == 1 and driver.enabled(selector)
    driver.wait(ready, seconds=90)
    element = driver.one(selector)
    # Single native keyboard activation; no fallback click or replay on failure.
    driver.command('POST', '/element/' + element + '/value', {'text': '\ue007', 'value': ['\ue007']})


def exercise(driver, source, root, config_root, records, phase=lambda value: None):
    """Caller supplies a real fixed completed unpack cache and private empty root."""
    before_source = deployment.snapshot(source)
    before_config = deployment.snapshot(config_root)
    if list(root.iterdir()): raise DeploymentGuiFailure('deployment_root_not_empty')

    def safe():
        if deployment.snapshot(source) != before_source or deployment.snapshot(config_root) != before_config:
            raise DeploymentGuiFailure('deployment_changed_source_or_config')

    def button(name): return PANEL + "//button[text()='" + BUTTONS[name] + "']"
    def action(name): activate(driver, button(name))
    def record(name):
        safe()
        records.append({'stage': name, 'verified': True})
    def fill(target):
        driver.type(SOURCE, str(source))
        driver.type(DESTINATION, str(target))
        driver.wait(lambda: driver.value(SOURCE) == str(source) and driver.value(DESTINATION) == str(target))
        driver.wait(lambda: not driver.elements(DIALOG) and not driver.elements(ALERT))
    def preview(target):
        fill(target)
        before = deployment.snapshot(root)
        action('preview')
        driver.wait(lambda: len(driver.elements(DIALOG)) == 1, seconds=90)
        text = driver.text(DIALOG)
        for expected in (complete.pinned.VERSION, 'linux-amd64-baseline', '0700', str(source), str(target)):
            if expected not in text: raise DeploymentGuiFailure('deployment_preview_details_missing')
        if before != deployment.snapshot(root): raise DeploymentGuiFailure('deployment_preview_wrote_files')
        safe()

    phase('qoder_deployment_preview_cancel')
    cancelled = root / 'cancelled'
    preview(cancelled); action('cancel')
    driver.wait(lambda: not driver.elements(DIALOG))
    if os.path.lexists(cancelled) or list(root.iterdir()): raise DeploymentGuiFailure('deployment_cancel_wrote_files')
    record('preview_cancel')

    phase('qoder_deployment_input_invalidation')
    changed = root / 'changed'
    preview(cancelled)
    driver.type(DESTINATION, str(changed))
    driver.wait(lambda: driver.value(DESTINATION) == str(changed) and not driver.elements(DIALOG))
    if list(root.iterdir()): raise DeploymentGuiFailure('deployment_edit_wrote_files')
    record('input_invalidates_preview')

    phase('qoder_deployment_stale_destination')
    stale = root / 'stale'
    preview(stale)
    # Harness-authored external mutation AFTER review. It is not a product write.
    stale.mkdir(mode=0o700)
    deployment.private_file(stale / 'sentinel', b'preserve-external-directory')
    before = deployment.snapshot(root)
    stale_before = deployment.snapshot(stale)
    action('confirm')
    driver.wait(lambda: len(driver.elements(FAILED)) == 1, seconds=90)
    if driver.elements(DIALOG) or driver.elements(RESULT) or deployment.snapshot(root) != before:
        raise DeploymentGuiFailure('stale_deployment_changed_files')
    record('stale_destination_rejected')

    phase('qoder_deployment_confirmed')
    destination = root / 'confirmed'
    preview(destination); action('confirm')
    def completed():
        if driver.elements(FAILED): raise DeploymentGuiFailure('deployment_not_confirmed')
        return len(driver.elements(RESULT)) == 1 and SUCCESS in driver.text(RESULT)
    driver.wait(completed, seconds=120)
    if SUCCESS not in driver.text(RESULT) or driver.elements(DIALOG) or driver.elements(ALERT):
        raise DeploymentGuiFailure('deployment_not_confirmed')
    # Fixed bytes, private mode, exact records, no unexpected members. No exec.
    deployment.validate_destination(destination)
    if {p.name for p in root.iterdir()} != {'stale', 'confirmed'} or deployment.snapshot(stale) != stale_before:
        raise DeploymentGuiFailure('deployment_unexpected_output')
    record('confirmed_private_deployment')

    phase('qoder_deployment_existing_destination')
    before = deployment.snapshot(root)
    fill(destination); action('preview')
    driver.wait(lambda: len(driver.elements(ALERT)) == 1, seconds=90)
    if driver.elements(DIALOG) or deployment.snapshot(root) != before:
        raise DeploymentGuiFailure('existing_deployment_changed_files')
    record('existing_destination_rejected')

    phase('qoder_deployment_clear')
    action('clear')
    driver.wait(lambda: driver.value(SOURCE) == '' and driver.value(DESTINATION) == ''
                and not driver.elements(DIALOG) and not driver.elements(RESULT) and not driver.elements(ALERT))
    if deployment.snapshot(root) != before: raise DeploymentGuiFailure('deployment_clear_changed_files')
    record('clear_preserves_files')

    phase('qoder_deployment_navigation')
    future = root / 'not-submitted'
    preview(future)
    activate(driver, "//*[@role='tab' and @aria-label='Agent 管理']")
    activate(driver, "//*[@role='tab' and @aria-label='Qoder 分发与校验']")
    driver.wait(lambda: not driver.elements(DIALOG) and not driver.elements(RESULT))
    if deployment.snapshot(root) != before or os.path.lexists(future):
        raise DeploymentGuiFailure('deployment_navigation_changed_files')
    record('navigation_invalidates_preview')
    # Observe actual deployed bytes via native UI/IPC; never execute the payload.
    def inspect(target, expected):
        driver.type(INSPECT_PATH, str(target))
        driver.wait(lambda: driver.value(INSPECT_PATH) == str(target))
        activate(driver, INSPECT_BUTTON)
        driver.wait(lambda: len(driver.elements(INSPECT_RESULT + "[@data-state='" + expected + "']")) == 1, seconds=120)
        if deployment.snapshot(root) != before:
            raise DeploymentGuiFailure('deployment_inspection_changed_files')
    phase('qoder_deployment_inspection_verified')
    inspect(destination, 'payload_verified')
    record('deployment_inspection_verified')
    driver.type(INSPECT_PATH, str(stale))
    driver.wait(lambda: not driver.elements(INSPECT_RESULT))
    record('deployment_inspection_edit_invalidated')
    phase('qoder_deployment_inspection_conflict')
    inspect(stale, 'unsupported_or_changed')
    record('deployment_inspection_conflict')
    activate(driver, INSPECT_CLEAR)
    driver.wait(lambda: driver.value(INSPECT_PATH) == '' and not driver.elements(INSPECT_RESULT))
    if deployment.snapshot(root) != before:
        raise DeploymentGuiFailure('deployment_inspection_clear_changed_files')
    record('deployment_inspection_clear_invalidated')
    driver.type(INSPECT_PATH, str(destination))
    driver.wait(lambda: driver.value(INSPECT_PATH) == str(destination))
    activate(driver, INSPECT_BUTTON)
    # Always-present clear discards either pending or already-settled response.
    # Do not assume the native read can be interrupted or that it was still pending.
    activate(driver, INSPECT_CLEAR)
    driver.wait(lambda: driver.value(INSPECT_PATH) == '' and not driver.elements(INSPECT_RESULT))
    driver.type(INSPECT_PATH, str(destination))
    driver.wait(lambda: driver.enabled(INSPECT_BUTTON), seconds=120)
    if driver.elements(INSPECT_RESULT) or deployment.snapshot(root) != before:
        raise DeploymentGuiFailure('deployment_inspection_cancel_revived_or_changed_files')
    record('deployment_inspection_cancel_discards')
    assert_coverage(records)
