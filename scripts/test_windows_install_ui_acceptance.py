"""Offline wiring checks only; not evidence of native Windows UI behavior."""
import base64
import json
from pathlib import Path
import shutil
import subprocess
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
        self.assertIn("Open-BlockedRecovery", flow)
        self.assertIn("Persistent partial failure allowed installation", flow)
        for stage in ('preview_cancel','confirmed_failure','refresh_no_retry','restart_no_retry','persistent_recovery_blocked'):
            self.assertIn(stage, flow)
        for forbidden in ('Remove-Item', 'WriteAllText', '--stale', 'confirmInstall', 'Invoke-WebRequest'):
            self.assertNotIn(forbidden, flow)
        guard = UI[UI.index('function Assert-FailedInstallObservation'):UI.index('function Check-FailedReviewedInstall')]
        for check in ('installCalls -ne 1', 'versionProbes -ne 0', 'partialInstallRetained -ne $true',
                      '$observedResidual -cne $residual', '(Get-InstallConfigurationSnapshot) -cne $baseline',
                      'Assert-InstallNoConfirmation', "Contains('软件安装操作已完成')"):
            self.assertIn(check, guard)
        self.assertEqual(flow.count('Assert-FailedInstallObservation $fixture $manifest $baseline $residual'), 5)

    def test_residual_metadata_is_refreshed_without_dropping_time_checks(self):
        snapshot = UI[UI.index('function Get-FailedInstallState'):UI.index('function Get-FailedStateDifference')]
        self.assertLess(snapshot.index('$item.Refresh()'), snapshot.index('$item.Attributes'))
        self.assertIn('$item.Exists', snapshot)
        self.assertIn('created = $item.CreationTimeUtc.Ticks; modified = $item.LastWriteTimeUtc.Ticks', snapshot)
        self.assertIn('acl = (Get-Acl -LiteralPath $item.FullName).Sddl', snapshot)

    def test_failed_residual_diagnostics_are_fixed_and_do_not_relax_guard(self):
        diff = UI[UI.index('function Get-FailedStateDifference'):UI.index('function Assert-FailedInstallObservation')]
        self.assertIn("@('path','directory','hash','length','created','modified','attributes','acl')", diff)
        self.assertIn('entryIndex = $index; fields = $fields', diff)
        guard = UI[UI.index('function Assert-FailedInstallObservation'):UI.index('function Check-FailedReviewedInstall')]
        self.assertIn("throw ('Failed installation residual changed: ' + $stage)", guard)
        self.assertIn("checkpoint = $stage; changes = $difference", guard)
        for stage in ('initial', 'settled', 'refresh', 'restart', 'recovery-blocked'):
            self.assertIn("Assert-FailedInstallObservation $fixture $manifest $baseline $residual '" + stage + "'", UI)

    def test_session_panel_is_native_readonly_and_not_persistent_recovery(self):
        method = UI[UI.index('function Observe-FailedInstallSession'):UI.index('function Initialize-InstallWindowActivation')]
        self.assertIn('本会话安装记录：安装完成状态未确认', method)
        self.assertIn('TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern', method)
        self.assertIn('关闭或重启应用后记录丢失。', method)
        self.assertIn('不是当前磁盘检查、安装所有权或恢复凭据', method)
        self.assertIn("@('installCalls','versionProbes','prefixQueries','rootQueries')", method)
        for field in ('已审核 npm prefix','已审核全局目录','已审核包目录','已审核启动文件目录','已审核 npm 入口','已审核安装器 Node'):
            self.assertIn(field, method)
        self.assertNotIn("Click-Qwen '确认安装软件'", method)
        self.assertNotIn('catch', method)
        flow = UI[UI.index('function Check-FailedReviewedInstall'):]
        self.assertEqual(flow.count('Observe-FailedInstallSession $fixture $manifest $baseline $residual'), 3)
        self.assertEqual(flow.count('Assert-InstallSessionAbsent'), 2)
        self.assertIn("Activate-InstallWorkspace 'Qwen 配置检查'", flow)
        self.assertIn("Activate-InstallWorkspace 'Agent 管理'", flow)
        self.assertNotIn('Installation target changed since confirmation', UI)
        self.assertIn('安装请求已发送，但完成状态未确认。', UI)

    def test_explicit_residual_inspection_native_flow_and_no_execution(self):
        method = UI[UI.index('function Check-FailedInstallResidualInspection'):UI.index('function Check-FailedReviewedInstall')]
        self.assertEqual(method.count("Click-Qwen '只读检查固定残留槽位'"), 1)
        self.assertIn("Click-Qwen '关闭检查结果'", method)
        for state in ('package_directory: directory', 'posix_launcher: missing', 'cmd_launcher: missing', 'powershell_launcher: missing'):
            self.assertIn(state, method)
        self.assertIn("@('installCalls','versionProbes','prefixQueries','rootQueries')", method)
        self.assertIn("Assert-FailedInstallObservation $fixture $manifest $baseline $residual 'residual-inspection'", method)
        self.assertIn('officialAgentExecuted = $false', method)
        self.assertIn("ownership = 'unknown'", method)
        self.assertNotIn('catch', method)
        flow = UI[UI.index('function Check-FailedReviewedInstall'):]
        self.assertEqual(flow.count('Check-FailedInstallResidualInspection $fixture $manifest $baseline $residual'), 2)
        self.assertIn('Check-FailedInstallResidualInspection $fixture $manifest $baseline $residual -KeepOpen', flow)
        self.assertLess(flow.index('Check-FailedInstallResidualInspection $fixture $manifest $baseline $residual -KeepOpen'), flow.index("Activate-InstallWorkspace 'Qwen 配置检查'"))
        self.assertIn("Assert-ResidualResultAbsent\n        Save-QwenEvidence 'failed-reviewed-install-session-navigation'", flow)
        absent = UI[UI.index('function Assert-InstallSessionAbsent'):UI.index('function Observe-FailedInstallSession')]
        self.assertIn("Find-QwenControl '只读检查固定残留槽位'", absent)

    def test_residual_sharing_conflict_is_native_safe_error_then_recovery(self):
        method = UI[UI.index('function Check-FailedResidualUnavailable'):UI.index('function Check-FailedInstallResidualInspection')]
        self.assertIn('CreateFileW($fixture.package, 0x81, 0,', method)
        self.assertIn('if ($handle.IsInvalid)', method)
        self.assertIn('finally { $handle.Dispose() }', method)
        self.assertIn("Wait-InstallText $fixedError 'residual-unavailable'", method)
        self.assertIn("本会话只读检查不可用或路径发生变化。未清理或执行程序。请保留现有文件。", method)
        self.assertIn("Contains('Codex 固定槽位只读检查结果')", method)
        for forbidden in ('ReadFile', 'WriteFile', 'Remove-Item', 'WriteAllText', 'catch'):
            self.assertNotIn(forbidden, method)
        self.assertLess(method.index('$handle.Dispose()'), method.index('$after = Assert-FailedInstallObservation'))
        flow = UI[UI.index('function Check-FailedReviewedInstall'):]
        self.assertLess(flow.index('Check-FailedResidualUnavailable $fixture'), flow.index('Check-FailedInstallResidualInspection $fixture'))

    def test_navigation_waits_for_selected_visible_workspace(self):
        helper = UI[UI.index('function Wait-InstallWorkspaceSelected'):UI.index('function Assert-ResidualResultAbsent')]
        self.assertIn('.Current.IsSelected', helper)
        self.assertIn('-not $control.Current.IsOffscreen', helper)
        self.assertIn('selected = $selected; controlPresent = $controlPresent; visible = $visible', helper)
        self.assertIn('if ($selected -and $visible) { return }', helper)
        self.assertIn('$selected -and $controlPresent -and -not $visible -and -not $revealAttempted', helper)
        self.assertIn('ScrollItemPattern]$scroll).ScrollIntoView()', helper)
        self.assertIn('$control.SetFocus()', helper)
        activation = UI[UI.index('function Activate-InstallWorkspace'):UI.index('function Wait-InstallWorkspaceSelected')]
        self.assertEqual(activation.count('$invoke.Invoke()'), 1)
        self.assertEqual(activation.count('Send-InstallWorkspaceEnter $tab $handle'), 1)
        self.assertIn('if ($null -ne $invoke)', activation)
        self.assertIn('} else {', activation)
        self.assertIn('Wait-InstallWorkspaceSelected $name', activation)
        self.assertNotIn('.Select()', activation)
        self.assertIn("Save-QwenEvidence 'failed-reviewed-install-navigation'", activation)
        self.assertIn('throw $activationFailure', activation)
        keyboard = UI[UI.index('function Prepare-InstallWorkspaceKeyboard'):UI.index('function Activate-InstallWorkspace')]
        self.assertEqual(keyboard.count('SetForegroundWindow($handle)'), 1)
        self.assertEqual(keyboard.count('$tab.SetFocus()'), 1)
        self.assertEqual(keyboard.count("[Windows.Forms.SendKeys]::SendWait('{ENTER}')"), 1)
        self.assertIn('(Get-InstallForegroundWindow) -eq $handle -and $tab.Current.HasKeyboardFocus', keyboard)
        guard = keyboard[keyboard.index('function Send-InstallWorkspaceEnter('):]
        self.assertIn('(Get-InstallForegroundWindow) -ne $handle -or -not $tab.Current.HasKeyboardFocus', guard)
        self.assertLess(guard.index('throw '), guard.index('    Send-InstallWorkspaceEnterKey'))
        modeled = (ROOT / 'scripts/test_windows_workspace_activation.ps1').read_text(encoding='utf-8')
        self.assertIn('Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes', modeled)
        self.assertIn('case=$caseIndex route=$($script:case.route) fault=$($script:case.fault)', modeled)
        self.assertIn('$failure.Exception.ToString()', modeled)
        self.assertIn('Unexpected activation result ($caseLabel): $original', modeled)
        for fault in ('invoke', 'prepare', 'foreground', 'focus', 'zero-handle', 'send', 'settle'):
            self.assertIn("fault = '" + fault + "'", modeled)
        self.assertIn("Activate-InstallWorkspace 'Qwen 配置检查'", modeled)
        self.assertIn('./scripts/test_windows_workspace_activation.ps1', (ROOT / '.github/workflows/ci.yml').read_text())
        self.assertIn("Find-QwenControl 'settings.json 绝对路径'", helper)
        flow = UI[UI.index('function Check-FailedReviewedInstall'):]
        self.assertLess(flow.index("Activate-InstallWorkspace 'Qwen 配置检查'"), flow.index("Activate-InstallWorkspace 'Agent 管理'"))
        self.assertLess(flow.index("Activate-InstallWorkspace 'Agent 管理'"), flow.index("Save-QwenEvidence 'failed-reviewed-install-session-navigation'"))

    def test_qwen_current_gui_and_legacy_manifest_boundary_are_wired(self):
        qwen = (ROOT / 'scripts/windows-qwen-ui-acceptance.ps1').read_text(encoding='utf-8')
        current = qwen[qwen.index('function Check-QwenCurrentLifecycle'):qwen.index('function Check-QwenLifecycle')]
        legacy = qwen[qwen.index('function Check-QwenLifecycle'):]
        boundary = qwen[qwen.index('function Check-QwenManifestBoundary'):qwen.index('function Check-QwenCurrentLifecycle')]
        self.assertIn('"version":"0.27.0"', legacy)
        self.assertIn('"version":"0.27.2"', current)
        self.assertIn('Check-QwenManifestBoundary $fixture $manifest', legacy)
        for action in ('recovery', 'update', 'restore'):
            self.assertIn("'" + action + "'", boundary)
        self.assertIn('(Get-QwenFixtureSnapshot $fixture) -cne $before', boundary)
        self.assertIn('[IO.File]::WriteAllBytes($manifest, $legacy)', boundary)
        for action in ('确认应用 Vim 变更', '确认更新受管 Vim 值', '确认撤销 Vim 设置'):
            self.assertIn("Click-Qwen '" + action + "'", current)
        self.assertIn('$updated.before_digest -cne $first.before_digest', current)
        self.assertIn('qwenExecuted = $false', current)
        self.assertIn('Check-QwenCurrentLifecycle', LIFECYCLE)
        self.assertLess(LIFECYCLE.index('    Check-QwenLifecycle'), LIFECYCLE.index('    Check-QwenCurrentLifecycle'))

    def test_wrong_root_native_lane_rejects_and_retains_without_agent_probes(self):
        method=UI[UI.index('function Check-WrongRootReviewedInstall'):UI.index('function Check-FailedReviewedInstall')]
        guard=UI[UI.index('function Assert-WrongRootInstallObservation'):UI.index('function Check-WrongRootReviewedInstall')]
        self.assertIn("'--mode', 'wrong-root'",method)
        self.assertEqual(method.count("Click-Qwen '确认安装软件'"),1)
        self.assertEqual(method.count("Click-Qwen '刷新列表'"),2)
        self.assertEqual(method.count('Check-Gui '),2)
        for stage in ('verification_rejected','automatic_refresh_unprobed','manual_refresh_unprobed','navigation_unprobed'):
            self.assertIn(stage,method)
        for item in ('installCalls -ne 1','versionProbes -ne 0','completeWrongRootRetained -ne $true',
                     '$current -cne $residual','(Get-InstallConfigurationSnapshot) -cne $baseline','Assert-InstallNoConfirmation',
                     "'软件安装操作已完成'","'实测版本 1.2.3'"):
            self.assertIn(item,guard)
        self.assertIn("Activate-InstallWorkspace 'Qwen 配置检查'",method)
        self.assertIn("Activate-InstallWorkspace 'Agent 管理'",method)
        self.assertIn('wrongRootSessionRestarted = $true',method)
        self.assertIn("Check-Gui 'wrong-root-reviewed-install-restarted'",method)
        self.assertIn('Open-BlockedRecovery',method)
        self.assertIn('Check-ExplicitInstallRecovery $fixture $manifest',method)
        self.assertIn('authoredAgentExecuted = $false',method)
        self.assertIn("'probe-suppression'",guard)
        self.assertIn("@('installCalls','versionProbes','prefixQueries','rootQueries','metadataQueries')",guard)
        self.assertIn('$counts.$key -ne $expectedCounts.$key',guard)
        self.assertIn("@('安装 OpenAI Codex','应用默认配置 OpenAI Codex')",guard)
        self.assertIn('$action.Current.IsEnabled',guard)
        self.assertIn('    Check-WrongRootReviewedInstall',LIFECYCLE)
        self.assertLess(LIFECYCLE.index('    Check-FailedReviewedInstall'),LIFECYCLE.index('    Check-WrongRootReviewedInstall'))

    def test_persistent_recovery_and_synthetic_cases_keep_explicit_boundaries(self):
        recovery=UI[UI.index('function Check-ExplicitInstallRecovery'):UI.index('function Open-ReviewedInstallPlanAfterRecovery')]
        synthetic=UI[UI.index('function Check-SyntheticUncertainProtection'):UI.index('function Check-ExplicitInstallRecovery')]
        for token in ("'--relocate-owned'","'取消恢复检查'","'--conflict','create'","'--conflict','remove'",'Stale recovery cleared protection','Cancelled recovery changed protection',"'--protection','clear'",'Open-ReviewedInstallPlanAfterRecovery',"'--expected-attempts','2'",'testAuthoredResidueRelocation = $true','productCleanup = $false'):
            self.assertIn(token,recovery)
        self.assertEqual(recovery.count("Click-Qwen '确认恢复 Codex 探测'"),2) # stale once, fresh once
        self.assertEqual(recovery.count("Click-Qwen '确认安装软件'"),1)
        self.assertLess(recovery.index("'--protection','clear'"),recovery.index('Open-ReviewedInstallPlanAfterRecovery'))
        for token in ("@('armed','corrupt')",'stateUnchanged = $true','actualCrashVerified = $false','emptySlots = $true','Assert-RecoveryNoExecution','Set-Acl -LiteralPath $target -AclObject $acl','Synthetic protection security did not match its source'):
            self.assertIn(token,synthetic)
        homes=UI[UI.index('function Invoke-InstallPrivateHome'):UI.index('function Open-BlockedRecovery')]
        for token in ('finally {','Stop-App',"'HOME','USERPROFILE','AGENTDECK_TEST_HOME','APPDATA','LOCALAPPDATA'",'$script:homeDir = $savedHome'):
            self.assertIn(token,homes)

    @unittest.skipUnless(shutil.which('pwsh'),'actual PowerShell unavailable')
    def test_recovery_target_matches_core_dos_verbatim_spelling_only(self):
        function=UI[UI.index('function ConvertTo-ExpectedRecoveryPrefix'):UI.index('function Assert-RecoveryEligibleTarget')]
        inputs=[r'\\?\C:\测试 空格\prefix',r'C:\测试 空格\prefix',r'\\?\c:\MixedCASE',r'\\?\UNC\server\share',r'\\.\C:\prefix',r'\\?\Volume{test}\prefix',r'C:relative',r'\\?\C:/prefix']
        encoded=base64.b64encode(json.dumps(inputs,ensure_ascii=False).encode()).decode()
        command=function+"\n$values=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('"+encoded+"')) | ConvertFrom-Json; $out=@(foreach($value in $values){try{ConvertTo-ExpectedRecoveryPrefix $value}catch{'REJECTED'}}); ConvertTo-Json -InputObject $out -Compress"
        result=subprocess.run([shutil.which('pwsh'),'-NoProfile','-NonInteractive','-EncodedCommand',base64.b64encode(command.encode('utf-16le')).decode()],capture_output=True,text=True,timeout=15,check=True)
        self.assertEqual(json.loads(result.stdout),[r'C:\测试 空格\prefix',r'C:\测试 空格\prefix',r'c:\MixedCASE',*['REJECTED']*5])
        check=UI[UI.index('function Assert-RecoveryEligibleTarget'):UI.index('function Check-ExplicitInstallRecovery')]
        self.assertIn("$names -cnotcontains ('已记录安装位置：' + $expectedPrefix)",check)
        self.assertNotIn("$text.Contains('已记录安装位置：'",check)

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
                     '(Get-InstallConfigurationSnapshot) -cne $baseline', 'Save-QwenEvidence', 'prefixQueries + 1', 'rootQueries + 1'):
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
