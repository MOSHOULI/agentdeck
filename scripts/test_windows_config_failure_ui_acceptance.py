"""Offline authored fixture and lifecycle contracts, not Windows GUI evidence."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import windows_config_failure_fixture as fixture

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = (ROOT / 'scripts/windows-config-failure-ui-acceptance.ps1').read_text()
LIFECYCLE = (ROOT / 'scripts/windows-installer-acceptance.ps1').read_text()


class Tests(unittest.TestCase):
    def test_existing_gates_and_real_installed_wiring(self):
        self.assertIn("'windows-config-failure-ui-acceptance.ps1'", LIFECYCLE)
        self.assertLess(LIFECYCLE.index('    Check-MemoryExport\n'), LIFECYCLE.index('    Check-ConfigurationFailurePreservation\n'))
        for gate in ('Check-QwenLifecycle', 'Check-QwenCurrentLifecycle', 'Check-QoderReadonly', 'Check-ReviewedInstall', 'Check-FailedReviewedInstall', 'Check-CodexReadonly', 'Check-MemoryExport'):
            self.assertIn('    ' + gate + '\n', LIFECYCLE)
        for unsafe in ('executeScript', 'invoke_handler', '--remote-debugging', '--no-sandbox', 'Invoke-WebRequest'):
            self.assertNotIn(unsafe, SCRIPT)
        self.assertIn("$env:RUNNER_ENVIRONMENT -ne 'github-hosted'", LIFECYCLE)
        self.assertIn('Get-Command git.exe', LIFECYCLE)
        self.assertIn('Split-Path $script:acceptanceGit', SCRIPT)

    def test_single_confirmation_strict_error_and_preservation(self):
        self.assertEqual(SCRIPT.count('            Confirm-ConfigurationOnce $agent.name'), 1)
        self.assertEqual(SCRIPT.count('            Click-Qwen ("应用默认配置 " + $agent.name)'), 1)
        self.assertIn('if ($buttons.Count -gt 1)', SCRIPT)
        self.assertIn('$names -cnotcontains $expected', SCRIPT)
        self.assertIn('Instruction file has ambiguous or incomplete AgentDeck markers; left unchanged', SCRIPT)
        self.assertIn('Get-Acl -LiteralPath', SCRIPT)
        self.assertEqual(SCRIPT.count('(Get-ConfigurationSnapshot $fixture $manifest) -cne $baseline'), 3)
        self.assertIn("'.agentdeck/git-backup/.git'", SCRIPT)
        self.assertIn('$result.agents.Count -ne 2', SCRIPT)
        self.assertIn('$result.passed = $true', SCRIPT)
        self.assertIn('"$stage.json"', SCRIPT)
        self.assertIn('$env:PATH = $savedPath; $env:PATHEXT = $savedExt', SCRIPT)

    def test_snapshot_detects_identical_bytes_replacement_and_metadata_change(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'instructions'
            path.write_bytes(b'same bytes')
            before = fixture.snapshot([path])
            # Keep original inode allocated so replacement cannot recycle it.
            path.rename(path.with_suffix('.old'))
            path.write_bytes(b'same bytes')
            after = fixture.snapshot([path])
            self.assertEqual(before[0]['hash'], after[0]['hash'])
            self.assertNotEqual(before[0]['fileId'], after[0]['fileId'])
            self.assertNotEqual(before, after)
            import os
            os.utime(path, ns=(946684800000000000, 946684800000000000))
            self.assertNotEqual(after, fixture.snapshot([path]))

    def test_version_only_program_rejects_any_other_invocation(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / 'events'
            log.write_bytes(b'')
            js = Path(tmp) / 'fixture.js'
            js.write_bytes(fixture.program(log, '2.0.30 (Claude Code)'))
            good = subprocess.run(['node', str(js), '--version'], capture_output=True, text=True)
            self.assertEqual(good.returncode, 0)
            self.assertEqual(good.stdout.strip(), '2.0.30 (Claude Code)')
            for args in ([], ['login'], ['--version', 'extra']):
                bad = subprocess.run(['node', str(js), *args], capture_output=True)
                self.assertEqual(bad.returncode, 64)
                self.assertEqual(bad.stdout, b'')
            self.assertEqual([json.loads(x) for x in log.read_text().splitlines()], [['--version'], [], ['login'], ['--version', 'extra']])

    def test_real_setup_is_windows_only(self):
        if fixture.os.name == 'nt':
            self.skipTest('Linux guard contract')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'fixture'
            with self.assertRaises(Exception):
                fixture.setup(root, Path(tmp) / 'node.exe', Path(tmp))
            self.assertFalse(root.exists())

    def test_setup_authors_only_two_version_packages_and_preserved_targets(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'fixture'
            home = Path(tmp) / 'home'
            home.mkdir()
            def base_prepare(root, node, home, mode):
                root.mkdir()
                prefix = root / 'prefix'
                global_root = prefix / 'node_modules'
                global_root.mkdir(parents=True)
                paths = {'root': str(root), 'prefix': str(prefix), 'globalRoot': str(global_root),
                         'node': str(root / 'node.exe'), 'manager': str(root / 'npm.js'),
                         'installCalls': str(root / 'install-events')}
                for key in ('node', 'manager', 'installCalls'):
                    Path(paths[key]).write_bytes(b'')
                paths.update(nodeSha256=fixture.hashlib.sha256(b'').hexdigest(),
                             managerSha256=fixture.hashlib.sha256(b'').hexdigest())
                manifest = root / 'fixture.json'
                manifest.write_text(json.dumps(paths))
                return manifest
            with patch.object(fixture, 'prepare', side_effect=base_prepare):
                manifest = fixture.setup(root, Path(tmp) / 'unused', home)
            m = json.loads(manifest.read_text())
            self.assertEqual(len(m['configurationTargets']), 4)
            self.assertEqual(len(m['authored']), 8)
            for binary, package, version, instruction in fixture.AGENTS:
                code = Path(m['globalRoot']) / package / 'cli.js'
                result = subprocess.run(['node', str(code), '--version'], capture_output=True, text=True)
                self.assertEqual(result.returncode, 0)
                self.assertEqual(result.stdout.strip(), version)
                text = (home / instruction).read_bytes()
                self.assertIn(b'<!-- agentdeck:personalization:start -->', text)
                self.assertNotIn(b'<!-- agentdeck:personalization:end -->', text)
            self.assertEqual(fixture.verify(m), {'claude': 1, 'gemini': 1})
            Path(m['installCalls']).write_bytes(b'i')
            with self.assertRaisesRegex(ValueError, 'unexpected_install'):
                fixture.verify(m)
            Path(m['installCalls']).write_bytes(b'')
            (root / 'gemini-versions.jsonl').write_text('["login"]\n')
            with self.assertRaisesRegex(ValueError, 'missing_or_nonversion_invocation'):
                fixture.verify(m)

    def test_package_contract_matches_registry(self):
        registry = json.loads((ROOT / 'agents/registry-v3.json').read_text())['agents']
        for binary, _, _, target in fixture.AGENTS:
            agent = registry['claude-code' if binary == 'claude' else 'gemini-cli']
            self.assertEqual(agent['config_inject']['personalization']['target'], '~/' + target)
            self.assertEqual(agent['config_inject']['model_config']['file'], '~/' + str(Path(target).parent / 'settings.json').replace('\\', '/'))


if __name__ == '__main__':
    unittest.main()
