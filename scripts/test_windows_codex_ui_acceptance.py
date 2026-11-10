"""Offline wiring/contract regressions, not Windows GUI execution evidence."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = (ROOT / 'scripts/windows-codex-ui-acceptance.ps1').read_text()
LIFECYCLE = (ROOT / 'scripts/windows-installer-acceptance.ps1').read_text()


class Tests(unittest.TestCase):
    def test_real_installed_lifecycle_and_existing_gates_retained(self):
        self.assertIn("'windows-codex-ui-acceptance.ps1'", LIFECYCLE)
        self.assertLess(LIFECYCLE.index('    Check-QwenLifecycle'), LIFECYCLE.index('    Check-CodexReadonly'))
        self.assertLess(LIFECYCLE.index('    Check-QoderReadonly'), LIFECYCLE.index('    Check-CodexReadonly'))
        self.assertIn("$env:RUNNER_ENVIRONMENT -ne 'github-hosted'", LIFECYCLE)
        self.assertIn("Check-Gui 'codex-readiness-start' -KeepOpen", SCRIPT)
        for unsafe in ('--no-sandbox', 'executeScript', 'Invoke-WebRequest', 'invoke_handler', '--remote-debugging'):
            self.assertNotIn(unsafe, SCRIPT)

    def test_states_contract_and_zero_write_evidence(self):
        for stage in ('missing', 'true', 'external-false', 'unsupported', 'receipt'):
            self.assertIn("'codex-readiness-" + stage + "'", SCRIPT)
        for text in ('检查动画配置（只读）', '当前动画值：true。', '当前动画值：false。', '动画配置文件不存在；未创建文件。', '检测到受管收据；未读取收据，Windows 暂不支持恢复。'):
            self.assertIn(text, SCRIPT)
        for check in ('Get-CodexSnapshot', 'Assert-CodexQuiet $before', 'Assert-CodexNoWriter', 'Save-QwenEvidence $stage', 'CreationTimeUtc', 'LastWriteTimeUtc', 'Get-Acl'):
            self.assertIn(check, SCRIPT)
        self.assertEqual(SCRIPT.count('Observe-Codex '), 5)  # five observations
        self.assertEqual(SCRIPT.count('        Clear-CodexObservation'), 5)

    def test_process_sentinel_after_startup_and_environment_restored(self):
        self.assertLess(SCRIPT.index("Check-Gui 'codex-readiness-start'"), SCRIPT.index('[IO.File]::WriteAllText($launcher'))
        self.assertIn('$env:CODEX_HOME = $directory', SCRIPT)
        self.assertIn('$env:CODEX_HOME = $savedCodexHome', SCRIPT)
        self.assertIn('$env:PATH = $savedPath', SCRIPT)
        self.assertIn('Test-Path -LiteralPath $script:codexProcessMarker', SCRIPT)
        self.assertIn('actualCodexExecutionVerified = $false; windowsWritingSupported = $false', SCRIPT)


if __name__ == '__main__': unittest.main()
