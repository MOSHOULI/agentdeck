"""Offline wiring checks only; not evidence of native Windows UI behavior."""
from pathlib import Path
import unittest
ROOT = Path(__file__).resolve().parents[1]
UI = (ROOT / 'scripts/windows-install-ui-acceptance.ps1').read_text(encoding='utf-8')
LIFECYCLE = (ROOT / 'scripts/windows-installer-acceptance.ps1').read_text(encoding='utf-8')
class Tests(unittest.TestCase):
    def test_native_gate_and_order(self):
        self.assertLess(LIFECYCLE.index('    Check-QoderReadonly'), LIFECYCLE.index('    Check-ReviewedInstall'))
        self.assertLess(LIFECYCLE.index('    Check-ReviewedInstall'), LIFECYCLE.index('    Check-CodexReadonly'))
        self.assertIn('    Check-QwenLifecycle', LIFECYCLE)
        self.assertIn("Check-Gui 'reviewed-install-start' -KeepOpen", UI)
        for forbidden in ('executeScript', 'invoke_handler', '--no-sandbox', 'Invoke-WebRequest'):
            self.assertNotIn(forbidden, UI)
    def test_review_and_evidence_contract(self):
        self.assertEqual(UI.count('Open-ReviewedInstallPlan $fixture $manifest'), 3)
        self.assertEqual(UI.count("Click-Qwen '确认安装软件'"), 2)
        self.assertLess(UI.index("'--stale', '--manifest'"), UI.index("Click-Qwen '确认安装软件'"))
        for text in ('preview_cancel', 'stale_revision', 'confirmed_install', 'observed_source', 'no_configuration',
                     'authoredNpmAndAgentCodeExecuted = $true', 'officialAgentExecuted = $false',
                     '(Get-CodexSnapshot) -cne $baseline', 'Save-QwenEvidence', 'prefixQueries + 1', 'rootQueries + 1'):
            self.assertIn(text, UI)
        self.assertIn('$env:PATH = $savedPath; $env:PATHEXT = $savedExt', UI)
    def test_all_evidence_checks_names_and_values_before_persistence(self):
        shared = (ROOT / 'scripts/windows-qwen-ui-acceptance.ps1').read_text(encoding='utf-8')
        guard = shared[shared.index('function Assert-AcceptanceCanariesAbsent'):shared.index('function Save-QwenEvidence')]
        for token in ('SYNTHETIC-NOT-A-CREDENTIAL', 'AGENTDECK_SYNTHETIC_INSTALL_PRIVATE_CANARY',
                      '$node.Current.Name.Contains($canary)', 'ValuePattern', 'Current.Value.Contains($canary)'):
            self.assertIn(token, guard)
        save = shared[shared.index('function Save-QwenEvidence'):shared.index('function Open-QwenFixture')]
        self.assertLess(save.index('Assert-AcceptanceCanariesAbsent $nodes'), save.index('$names | ConvertTo-Json'))
        startup = LIFECYCLE[LIFECYCLE.index('function Check-Gui'):LIFECYCLE.index('\ntry {')]
        self.assertLess(startup.index('Assert-AcceptanceCanariesAbsent $nodes'), startup.index('$lastNames | ConvertTo-Json'))
        self.assertLess(startup.index('Assert-AcceptanceCanariesAbsent $currentNodes'), startup.index('$graphics.CopyFromScreen'))

if __name__ == '__main__': unittest.main()
