"""Offline installed-Windows harness contract; not a native GUI result."""
from pathlib import Path
import unittest
import linux_memory_export_webview as linux

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = (ROOT / 'scripts/windows-memory-ui-acceptance.ps1').read_text()
LIFECYCLE = (ROOT / 'scripts/windows-installer-acceptance.ps1').read_text()
WORKSPACE = (ROOT / 'scripts/windows-install-ui-acceptance.ps1').read_text()
WORKFLOW = (ROOT / '.github/workflows/ci.yml').read_text()


class Tests(unittest.TestCase):
    def test_existing_installed_gates_and_hosted_guard_retained(self):
        self.assertIn("$env:RUNNER_ENVIRONMENT -ne 'github-hosted'", LIFECYCLE)
        self.assertIn("'windows-memory-ui-acceptance.ps1'", LIFECYCLE)
        for old in ('Check-QwenLifecycle', 'Check-QoderReadonly', 'Check-ReviewedInstall', 'Check-FailedReviewedInstall', 'Check-CodexReadonly'):
            self.assertLess(LIFECYCLE.index('    ' + old), LIFECYCLE.index('    Check-MemoryExport'))
        self.assertIn("Check-Gui 'memory-editor-start' -KeepOpen", SCRIPT)
        self.assertIn("Activate-InstallWorkspace '记忆管理'", SCRIPT)
        self.assertIn("[ValidateSet('Qwen 配置检查','Agent 管理','记忆管理')]", WORKSPACE)
        self.assertIn('$selected -and $visible', WORKSPACE)
        self.assertIn('Get-InstallWorkspaceInvoke $tab', WORKSPACE)
        for forbidden in ('--no-sandbox', 'executeScript', 'Invoke-WebRequest', 'invoke_handler', '--remote-debugging', 'collect_memory'):
            self.assertNotIn(forbidden, SCRIPT)

    def test_exact_lifecycle_and_assertions(self):
        for stage in linux.STAGES:
            self.assertIn("'" + stage + "'", SCRIPT)
        for assertion in ('Assert-MemoryDraft', 'Assert-MemoryQuiet $before', 'Assert-MemoryUnrelated',
                          "@('--verify', 'replaced')", "@('--verify', 'cleared')", 'Search-Memory \'\' @(17..66)',
                          'Preview-Memory 3 3 5', 'Preview-Memory 0 3 3', 'Wait-MemoryText \'已变化\'',
                          "Save-QwenEvidence ('memory-' + $stage)", '$toggle.Current.ToggleState',
                          'Assert-AcceptanceCanariesAbsent $nodes', 'realAgentConsumptionVerified = $false'):
            self.assertIn(assertion, SCRIPT)
        self.assertIn("@('--mutate', $case.kind)", SCRIPT)
        self.assertIn("$case.kind + '-drift'", SCRIPT)
        self.assertIn("($script:memoryStages -join ',') -cne ($expected -join ',')", SCRIPT)
        # Native actions fail closed: no fallback/retry after sending keys.
        self.assertIn('Prepare-InstallWorkspaceKeyboard $control', SCRIPT)
        self.assertIn('(Get-InstallForegroundWindow) -ne $handle -or -not $control.Current.HasKeyboardFocus', SCRIPT)
        self.assertEqual(SCRIPT.count('[Windows.Forms.SendKeys]::SendWait($keys)'), 1)
        self.assertIn('Get-MemoryTargetValue $control', SCRIPT)
        self.assertIn("Select-MemoryTarget 'Claude'", SCRIPT)
        self.assertIn("Select-MemoryTarget 'OpenCode'", SCRIPT)
        self.assertIn("Select-MemoryTarget 'Codex'", SCRIPT)

    def test_ci_mandatory_and_no_publication_changes(self):
        for name in ('test_memory_export_fixture.py', 'test_linux_memory_export_webview.py', 'test_windows_memory_ui_acceptance.py'):
            self.assertIn(name, WORKFLOW)
        self.assertIn("'windows-memory-ui-acceptance.ps1'", WORKFLOW)
        self.assertIn('needs: [frontend, portable-core, dependency-audit, windows, windows-acceptance, linux-webview]', WORKFLOW)
        self.assertNotIn('continue-on-error', SCRIPT)


if __name__ == '__main__': unittest.main()
