"""Offline wiring checks only; not evidence of native Windows UI behavior."""
from pathlib import Path
import unittest
ROOT = Path(__file__).resolve().parents[1]
UI = (ROOT / 'scripts/windows-install-ui-acceptance.ps1').read_text(encoding='utf-8')
LIFECYCLE = (ROOT / 'scripts/windows-installer-acceptance.ps1').read_text(encoding='utf-8')
class Tests(unittest.TestCase):
    def test_explicit_utf8_native_transport_and_restoration(self):
        wrapper = UI[UI.index('function Invoke-InstallFixture'):UI.index('function Get-InstallPlanNames')]
        self.assertIn('$script:acceptancePython -X utf8', wrapper)
        self.assertIn('[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)', wrapper)
        self.assertIn('$OutputEncoding = [Text.UTF8Encoding]::new($false)', wrapper)
        self.assertIn('finally {', wrapper)
        self.assertIn('[Console]::OutputEncoding = $savedConsoleEncoding', wrapper)
        self.assertIn('$OutputEncoding = $savedOutputEncoding', wrapper)

    def test_details_native_supported_action_without_replay(self):
        method = UI[UI.index('function Open-InstalledCodexDetails'):UI.index('function Check-ReviewedInstall')]
        self.assertIn('TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern', method)
        self.assertIn('ExpandCollapseState]::Collapsed', method)
        self.assertEqual(method.count('$expand.Expand()'), 1)
        self.assertEqual(method.count("[Windows.Forms.SendKeys]::SendWait('{ENTER}')"), 1)
        self.assertIn('} else {', method)
        self.assertLess(method.index('$control.SetFocus()'), method.index('$control.Current.HasKeyboardFocus'))
        self.assertLess(method.index('$control.Current.HasKeyboardFocus'), method.index('SendWait'))
        self.assertNotIn('catch', method)
        self.assertNotIn('GetCurrentPattern(', method.replace('TryGetCurrentPattern(', ''))
        self.assertIn("Wait-QwenControl '收起 OpenAI Codex 详情'", method)
        self.assertIn("Wait-QwenControl '关闭 OpenAI Codex 详情'", method)
        self.assertNotIn("Click-Qwen '查看 OpenAI Codex 详情'", UI)

    def test_details_failure_evidence_uses_fixed_keys_and_preserves_checks(self):
        wait = UI[UI.index('function Wait-InstallText'):UI.index('function Assert-InstallNoConfirmation')]
        self.assertIn('ValidateSet(', wait)
        self.assertIn("Save-QwenEvidence ('reviewed-install-timeout-' + $check)", wait)
        self.assertIn("@{ check = $check }", wait)
        self.assertNotIn('+ $expected', wait)
        self.assertLess(UI.index("Save-QwenEvidence 'reviewed-install-details-expanded'"),
                        UI.index("Wait-InstallText '实测版本 1.2.3' 'version'"))
        for line in ("Wait-InstallText 'npm 全局目录匹配，安装历史未知' 'source'",
                     "Wait-InstallText $fixture.package 'package'", "Wait-InstallText $fixture.globalRoot 'global'",
                     "Wait-InstallText $fixture.prefix 'prefix'", "Wait-InstallText $fixture.manager 'manager'"):
            self.assertIn(line, UI)

    def test_failure_lane_preserves_partial_state_without_reinstall(self):
        flow = UI[UI.index('function Check-FailedReviewedInstall'):]
        self.assertIn("'--mode', 'fail'", flow)
        self.assertEqual(flow.count("Click-Qwen '确认安装软件'"), 1)
        self.assertEqual(flow.count('Open-ReviewedInstallPlan $fixture $manifest'), 2)
        self.assertLess(flow.index("Click-Qwen '取消'"), flow.index("Click-Qwen '确认安装软件'"))
        self.assertIn("Check-Gui 'failed-reviewed-install-restart' -KeepOpen", flow)
        self.assertIn("Click-Qwen '刷新列表'", flow)
        self.assertIn("Wait-InstallText '无法取得有效安装预览，请刷新后重新检查。' 'occupied-preview-error'", flow)
        for stage in ('preview_cancel','confirmed_failure','refresh_no_retry','restart_no_retry','occupied_preview_rejected'):
            self.assertIn(stage, flow)
        for forbidden in ('Remove-Item', 'WriteAllText', '--stale', 'confirmInstall', 'Invoke-WebRequest'):
            self.assertNotIn(forbidden, flow)
        guard = UI[UI.index('function Assert-FailedInstallObservation'):UI.index('function Check-FailedReviewedInstall')]
        for check in ('installCalls -ne 1', 'versionProbes -ne 0', 'partialInstallRetained -ne $true',
                      '$observedResidual -cne $residual', '(Get-CodexSnapshot) -cne $baseline',
                      'Assert-InstallNoConfirmation', "Contains('软件安装操作已完成')"):
            self.assertIn(check, guard)
        self.assertEqual(flow.count('Assert-FailedInstallObservation $fixture $manifest $baseline $residual'), 5)

    def test_failed_residual_diagnostics_are_fixed_and_do_not_relax_guard(self):
        diff = UI[UI.index('function Get-FailedStateDifference'):UI.index('function Assert-FailedInstallObservation')]
        self.assertIn("@('path','directory','hash','length','created','modified','attributes','acl')", diff)
        self.assertIn('entryIndex = $index; fields = $fields', diff)
        guard = UI[UI.index('function Assert-FailedInstallObservation'):UI.index('function Check-FailedReviewedInstall')]
        self.assertIn("throw ('Failed installation residual changed: ' + $stage)", guard)
        self.assertIn("checkpoint = $stage; changes = $difference", guard)
        for stage in ('initial', 'settled', 'refresh', 'restart', 'occupied-preview'):
            self.assertIn("Assert-FailedInstallObservation $fixture $manifest $baseline $residual '" + stage + "'", UI)

    def test_native_gate_and_order(self):
        self.assertLess(LIFECYCLE.index('    Check-QoderReadonly'), LIFECYCLE.index('    Check-ReviewedInstall'))
        self.assertLess(LIFECYCLE.index('    Check-ReviewedInstall'), LIFECYCLE.index('    Check-CodexReadonly'))
        self.assertIn('    Check-QwenLifecycle', LIFECYCLE)
        self.assertIn("Check-Gui 'reviewed-install-start' -KeepOpen", UI)
        for forbidden in ('executeScript', 'invoke_handler', '--no-sandbox', 'Invoke-WebRequest'):
            self.assertNotIn(forbidden, UI)
    def test_review_and_evidence_contract(self):
        self.assertEqual(UI[:UI.index('function Get-FailedInstallState')].count('Open-ReviewedInstallPlan $fixture $manifest'), 3)
        self.assertEqual(UI[:UI.index('function Get-FailedInstallState')].count("Click-Qwen '确认安装软件'"), 2)
        self.assertLess(UI.index("'--stale', '--manifest'"), UI.index("Click-Qwen '确认安装软件'"))
        for text in ('preview_cancel', 'stale_revision', 'confirmed_install', 'observed_source', 'no_configuration',
                     'authoredNpmAndAgentCodeExecuted = $true', 'officialAgentExecuted = $false',
                     '(Get-CodexSnapshot) -cne $baseline', 'Save-QwenEvidence', 'prefixQueries + 1', 'rootQueries + 1'):
            self.assertIn(text, UI)
        self.assertIn('$env:PATH = $savedPath; $env:PATHEXT = $savedExt', UI)
    def test_all_evidence_checks_names_and_values_before_persistence(self):
        shared = (ROOT / 'scripts/windows-qwen-ui-acceptance.ps1').read_text(encoding='utf-8')
        guard = shared[shared.index('function Assert-AcceptanceCanaryValues'):shared.index('function Save-QwenEvidence')]
        for token in ('SYNTHETIC-NOT-A-CREDENTIAL', 'AGENTDECK_SYNTHETIC_INSTALL_PRIVATE_CANARY',
                      '([string]$name).Contains($canary)', 'ValuePattern', '([string]$value).Contains($canary)'):
            self.assertIn(token, guard)
        save = shared[shared.index('function Save-QwenEvidence'):shared.index('function Open-QwenFixture')]
        self.assertLess(save.index('Assert-AcceptanceCanariesAbsent $nodes'), save.index('$names | ConvertTo-Json'))
        startup = LIFECYCLE[LIFECYCLE.index('function Check-Gui'):LIFECYCLE.index('\ntry {')]
        self.assertLess(startup.index('Assert-AcceptanceCanariesAbsent $nodes'), startup.index('$lastNames | ConvertTo-Json'))
        self.assertLess(startup.index('Assert-AcceptanceCanariesAbsent $currentNodes'), startup.index('$graphics.CopyFromScreen'))

if __name__ == '__main__': unittest.main()
