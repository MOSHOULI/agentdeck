"""Offline authored fixture and lifecycle contracts, not Windows GUI evidence."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import windows_config_failure_fixture as fixture

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = (ROOT / 'scripts/windows-config-failure-ui-acceptance.ps1').read_text(encoding='utf-8')
LIFECYCLE = (ROOT / 'scripts/windows-installer-acceptance.ps1').read_text(encoding='utf-8')


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
        self.assertEqual(SCRIPT.split("function Check-ConfigurationFailurePreservation", 1)[1].count('(Get-ConfigurationSnapshot $fixture $manifest) -cne $baseline'), 3)
        self.assertIn("'.agentdeck/git-backup/.git'", SCRIPT)
        self.assertIn('$result.agents.Count -ne 2', SCRIPT)
        self.assertIn('$result.passed = $true', SCRIPT)
        self.assertIn('"$stage.json"', SCRIPT)
        self.assertIn('$env:PATH = $savedPath; $env:PATHEXT = $savedExt', SCRIPT)

    def test_hardlink_stages_are_additive_real_ui_and_preserve_both_aliases(self):
        self.assertIn("$result.hardlinks = @(Check-ConfigurationHardlinkPreservation $manifest)", SCRIPT)
        self.assertLess(SCRIPT.index("if ($result.agents.Count -ne 2)"), SCRIPT.index("$result.hardlinks ="))
        body = SCRIPT.split('function Check-ConfigurationHardlinkPreservation', 1)[1].split('function Check-ConfigurationFailurePreservation', 1)[0]
        self.assertEqual(body.count('Confirm-ConfigurationOnce $agent.name'), 1)
        self.assertEqual(body.count('Click-Qwen ("应用默认配置 " + $agent.name)'), 1)
        self.assertIn('Wait-ConfigurationLinkedTargetError', body)
        self.assertIn('bothAliasesBytesIdentityMetadataAclPreserved = $true', body)
        self.assertIn('$fixture.configurationTargets.Count -ne 6', body)
        self.assertEqual(body.count('(Get-ConfigurationSnapshot $fixture $manifest) -cne $baseline'), 3)
        self.assertIn('No links were automatically detached.', SCRIPT)

    def test_valid_hardlink_fixture_detects_detachment_and_alias_changes(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / 'home'
            root = Path(tmp) / 'root'
            root.mkdir()
            targets = []
            for _, _, _, instruction in fixture.AGENTS:
                target = home / instruction
                target.parent.mkdir(parents=True)
                target.write_bytes(b'malformed')
                settings = target.parent / 'settings.json'
                settings.write_bytes(b'{}')
                targets.extend((str(target), str(settings)))
            m = fixture.prepare_hardlinks(dict(home=str(home), root=str(root), configurationTargets=targets))
            before = fixture.snapshot(m['configurationTargets'])
            self.assertEqual(len(before), 6)
            for index, alias_index in ((0, 4), (2, 5)):
                self.assertEqual(before[index]['fileId'], before[alias_index]['fileId'])
                self.assertEqual(before[index]['links'], 2)
                self.assertNotIn(b'agentdeck:personalization', Path(m['configurationTargets'][index]).read_bytes())
            target = Path(m['configurationTargets'][0])
            data = target.read_bytes()
            target.unlink()
            target.write_bytes(data)
            after = fixture.snapshot(m['configurationTargets'])
            self.assertNotEqual(before, after)
            self.assertEqual(before[0]['hash'], after[0]['hash'])
            self.assertEqual(after[4]['links'], 1)
            Path(m['hardlinkAliases'][1]).write_bytes(b'changed alias')
            self.assertNotEqual(after, fixture.snapshot(m['configurationTargets']))

    def test_success_dacl_stage_is_additive_and_checks_real_content_backup_and_acls(self):
        script = (ROOT / 'scripts/windows-config-dacl-ui-acceptance.ps1').read_text(encoding='utf-8')
        self.assertIn("'windows-config-dacl-ui-acceptance.ps1'", LIFECYCLE)
        self.assertLess(SCRIPT.index('$result.hardlinks ='), SCRIPT.index('$result.successfulDacl ='))
        self.assertEqual(script.count('Confirm-ConfigurationOnce $agent.name'), 1)
        self.assertEqual(script.count('Click-Qwen ("应用默认配置 " + $agent.name)'), 1)
        for required in ('AccessControlSections]::Access', 'AreAccessRulesProtected', '$otherParentGrants.Count -eq 0',
                         '$rules.Count -ne 1', '$rules[0].IdentityReference -ne $owner',
                         '$beforeOwner', 'Get-ConfigurationDacl $verified.backup',
                         "'verify-success'", 'Get-InactiveConfigurationSnapshot', 'Wait-ConfigurationSuccess',
                         'rollbackExercised = $false', 'officialAgentExecuted = $false'):
            self.assertIn(required, script)
        self.assertIn('$beforeGroup = $beforeAcl.GetGroup([Security.Principal.SecurityIdentifier]).Value', script)
        for state in ('afterAcl', 'closedAcl'):
            for field, expected in (('AreAccessRulesProtected', '$beforeAcl.AreAccessRulesProtected'),
                                    ('GetOwner([Security.Principal.SecurityIdentifier]).Value', '$beforeOwner'),
                                    ('GetGroup([Security.Principal.SecurityIdentifier]).Value', '$beforeGroup')):
                self.assertIn('$' + state + '.' + field + (' -ne ' if field == 'AreAccessRulesProtected' else ' -cne ') + expected, script)
        close_checks = script.split('$closedAcl = Get-Acl', 1)[0]
        self.assertGreater(close_checks.rfind('Stop-App'), close_checks.rfind('Save-QwenEvidence'))
        self.assertIn('primaryGroupPreserved = $true', script)
        self.assertIn("file = '.claude/CLAUDE.md'; protected = $true", script)
        self.assertIn("file = '.gemini/GEMINI.md'; protected = $false", script)
        for unsafe in ('executeScript', 'invoke_handler', '--remote-debugging', '--no-sandbox', 'Invoke-WebRequest'):
            self.assertNotIn(unsafe, script)

    def test_success_fixture_requires_managed_content_and_original_backup(self):
        with tempfile.TemporaryDirectory() as tmp:
            home, root = Path(tmp) / 'home', Path(tmp) / 'root'
            root.mkdir()
            targets = []
            for _, _, _, instruction in fixture.AGENTS:
                target = home / instruction
                target.parent.mkdir(parents=True)
                target.write_bytes(b'initial')
                settings = target.parent / 'settings.json'
                settings.write_bytes(b'{}')
                targets.extend((str(target), str(settings)))
            m = fixture.prepare_hardlinks(dict(home=str(home), root=str(root), configurationTargets=targets))
            alias_before = fixture.snapshot(m['hardlinkAliases'])
            m = fixture.prepare_dacl(m)
            alias_after = fixture.snapshot(m['hardlinkAliases'])
            self.assertEqual([a['hash'] for a in alias_before], [a['hash'] for a in alias_after])
            self.assertTrue(all(a['links'] == 1 for a in alias_after))
            self.assertEqual(len(m['inactiveTargets']), 4)
            for binary, _, _, instruction in fixture.AGENTS:
                target = home / instruction
                self.assertEqual(target.read_bytes(), fixture.DACL_ORIGINAL)
                template = home / '.agentdeck/profiles/default/personalization' / binary / 'AGENTS.md'
                template.parent.mkdir(parents=True)
                template.write_bytes(b'Authored template.\n')
                expected = fixture.DACL_ORIGINAL + b'\n\n<!-- agentdeck:personalization:start -->\nAuthored template.\n<!-- agentdeck:personalization:end -->\n'
                target.write_bytes(expected)
                backup = home / '.agentdeck/backups/inject-1' / target.name
                backup.parent.mkdir(parents=True, exist_ok=True)
                backup.write_bytes(fixture.DACL_ORIGINAL)
                with patch.object(fixture, 'verify', return_value={'claude': 1, 'gemini': 1}):
                    self.assertEqual(fixture.verify_success(m, binary)['backup'], str(backup))
                    target.write_bytes(b'false success')
                    with self.assertRaisesRegex(ValueError, 'incorrect_successful_configuration'):
                        fixture.verify_success(m, binary)
                    target.write_bytes(expected)
                    backup.write_bytes(b'wrong old bytes')
                    with self.assertRaisesRegex(ValueError, 'incorrect_prewrite_backup'):
                        fixture.verify_success(m, binary)
                    backup.write_bytes(fixture.DACL_ORIGINAL)
                    duplicate = home / '.agentdeck/backups/inject-2' / target.name
                    duplicate.parent.mkdir(parents=True, exist_ok=True)
                    duplicate.write_bytes(fixture.DACL_ORIGINAL)
                    with self.assertRaisesRegex(ValueError, 'incorrect_prewrite_backup'):
                        fixture.verify_success(m, binary)

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

    def test_authored_entrypoints_meet_production_windows_interpreter_contract(self):
        # Node itself accepts a script without a shebang; production Windows
        # inspection deliberately does not. Keep that distinction under test.
        production = (ROOT / 'crates/aisync-core/src/deploy/inspection.rs').read_text(encoding='utf-8')
        self.assertIn('if head != b"#!/usr/bin/env node" && head != b"#!/usr/bin/env node\\r"', production)
        for binary, _, version, _ in fixture.AGENTS:
            with self.subTest(agent=binary):
                data = fixture.program(Path('authored-version-events'), version)
                bounded_first_line = data[:256].split(b'\n', 1)[0]
                self.assertIn(bounded_first_line, (b'#!/usr/bin/env node', b'#!/usr/bin/env node\r'))
                self.assertNotIn(data.split(b'\n', 1)[1][:256].split(b'\n', 1)[0],
                                 (b'#!/usr/bin/env node', b'#!/usr/bin/env node\r'))

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
            self.assertEqual([json.loads(x) for x in log.read_text(encoding='utf-8').splitlines()], [['--version'], [], ['login'], ['--version', 'extra']])

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
                manifest.write_text(json.dumps(paths), encoding='utf-8')
                return manifest
            with patch.object(fixture, 'prepare', side_effect=base_prepare):
                manifest = fixture.setup(root, Path(tmp) / 'unused', home)
            m = json.loads(manifest.read_text(encoding='utf-8'))
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
            (root / 'gemini-versions.jsonl').write_text('["login"]\n', encoding='utf-8')
            with self.assertRaisesRegex(ValueError, 'missing_or_nonversion_invocation'):
                fixture.verify(m)

    def test_cp1252_default_does_not_change_authored_utf8_io(self):
        # Simulate Windows Python's legacy default without relying on the host
        # locale. Explicit encodings pass through; omitted encodings use cp1252.
        import sys
        code = r'''import io, runpy, unittest, sys
sys.path.insert(0, "scripts")
# pathlib normalizes None via io.text_encoding before calling Path.open.
# Override that normalization, rather than missing its resolved "locale" value.
original = io.text_encoding
def legacy_text_encoding(encoding, stacklevel=2):
    return "cp1252" if encoding is None else original(encoding, stacklevel)
io.text_encoding = legacy_text_encoding
module = runpy.run_path("scripts/test_windows_config_failure_ui_acceptance.py", run_name="locale_probe")
case = module["Tests"]
suite = unittest.TestSuite(case(name) for name in (
    "test_package_contract_matches_registry",
    "test_setup_authors_only_two_version_packages_and_preserved_targets",
    "test_version_only_program_rejects_any_other_invocation",
))
raise SystemExit(not unittest.TextTestRunner().run(suite).wasSuccessful())
'''
        completed = subprocess.run([sys.executable, '-B', '-c', code], cwd=ROOT,
                                   capture_output=True, text=True, encoding='utf-8')
        self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)

    def test_package_contract_matches_registry(self):
        registry = json.loads((ROOT / 'agents/registry-v3.json').read_text(encoding='utf-8'))['agents']
        for binary, _, _, target in fixture.AGENTS:
            agent = registry['claude-code' if binary == 'claude' else 'gemini-cli']
            self.assertEqual(agent['config_inject']['personalization']['target'], '~/' + target)
            self.assertEqual(agent['config_inject']['model_config']['file'], '~/' + str(Path(target).parent / 'settings.json').replace('\\', '/'))


if __name__ == '__main__':
    unittest.main()
