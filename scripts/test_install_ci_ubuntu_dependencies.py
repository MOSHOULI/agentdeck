#!/usr/bin/env python3
"""Offline contract tests. Never invokes sudo, apt, package downloads or installs."""
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import install_ci_ubuntu_dependencies as helper

ENV = {'GITHUB_ACTIONS': 'true', 'RUNNER_ENVIRONMENT': 'github-hosted', 'RUNNER_OS': 'Linux'}
RELEASE = {'ID': 'ubuntu', 'VERSION_ID': '24.04', 'VERSION_CODENAME': 'noble'}


class UbuntuDependenciesTests(unittest.TestCase):
    def test_reviewed_host_context(self):
        helper.check_context(ENV, RELEASE, 'x86_64', 'amd64')

    def test_unreviewed_environments_fail(self):
        for change in ({'GITHUB_ACTIONS': 'false'}, {'RUNNER_ENVIRONMENT': 'self-hosted'},
                       {'RUNNER_OS': 'Windows'}, {'APT_CONFIG': ''}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                helper.check_context({**ENV, **change}, RELEASE, 'x86_64', 'amd64')
        with self.assertRaises(ValueError):
            helper.check_context({}, RELEASE, 'x86_64', 'amd64')

    def test_unreviewed_distribution_release_and_arch_fail(self):
        for change in ({'ID': 'debian'}, {'VERSION_ID': '26.04'},
                       {'VERSION_CODENAME': 'jammy'}, {'ID': 'linuxmint'}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                helper.check_context(ENV, {**RELEASE, **change}, 'x86_64', 'amd64')
        for machine, arch in [('aarch64', 'arm64'), ('x86_64', 'arm64'), ('i686', 'amd64')]:
            with self.subTest(machine=machine, arch=arch), self.assertRaises(ValueError):
                helper.check_context(ENV, RELEASE, machine, arch)

    def test_commands_preserve_runner_sources_and_trust(self):
        for group in helper.PACKAGES:
            for command in helper.commands(group):
                options = [command[i + 1] for i, arg in enumerate(command) if arg == '-o']
                self.assertEqual(options, [
                    'APT::Update::Error-Mode=any', 'Acquire::Retries=2',
                    'Acquire::http::Timeout=20', 'Acquire::https::Timeout=20',
                ])
                self.assertFalse(any('Dir::' in option or 'Trusted' in option
                                     or 'Proxy' in option for option in options))

    def test_commands_are_fixed_and_preserve_authentication(self):
        for group, packages in helper.PACKAGES.items():
            commands = helper.commands(group)
            self.assertEqual(len(commands), 2)
            for command, seconds in zip(commands, ['180s', '600s']):
                self.assertEqual(command[:7], ['/usr/bin/sudo', '--non-interactive',
                    '/usr/bin/timeout', '--signal=TERM', '--kill-after=15s', seconds,
                    '/usr/bin/apt-get'])
                self.assertIn('Acquire::Retries=2', command)
                self.assertIn('Acquire::https::Timeout=20', command)
                self.assertIn('Acquire::http::Timeout=20', command)
                self.assertIn('APT::Update::Error-Mode=any', command)
                self.assertFalse(any(word in ' '.join(command).lower() for word in
                    ['allow-unauthenticated', 'allowinsecure', 'verify-peer=false',
                     'verify-host=false', 'fix-missing', 'force-yes']))
            self.assertEqual(commands[0][-1], 'update')
            self.assertEqual(commands[1][-len(packages)-3:],
                             ['install', '--no-install-recommends', '-y', *packages])

    def test_unknown_group_and_option_injection_rejected_before_commands(self):
        for group in ('', '--allow-unauthenticated', 'glib; touch /tmp/x', 'curl'):
            with self.subTest(group=group), self.assertRaises(KeyError):
                helper.commands(group)
            with patch.object(helper.subprocess, 'check_output') as execute:
                with self.assertRaises(ValueError):
                    helper.main([group])
                execute.assert_not_called()
        with self.assertRaises(ValueError):
            helper.main(['glib', '--allow-unauthenticated'])

    def invoke_install(self, returncodes):
        calls = []
        def run(command, **kwargs):
            calls.append(command)
            self.assertEqual(kwargs, {'check': False})
            return SimpleNamespace(returncode=returncodes[len(calls)-1])
        result = helper.install('glib', run)
        return result, calls

    def test_success_requires_update_then_install(self):
        result, calls = self.invoke_install([0, 0])
        self.assertEqual(result, 0)
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[0][-1], 'update')
        self.assertEqual(calls[1][-2:], ['libglib2.0-dev', 'pkg-config'])

    def test_update_failure_or_timeout_never_installs_or_retries(self):
        for code in [1, 100, 124, 137]:
            with self.subTest(code=code):
                result, calls = self.invoke_install([code])
                self.assertEqual(result, code)
                self.assertEqual(len(calls), 1)

    def test_install_failure_or_timeout_never_repeats_transaction(self):
        for code in [1, 100, 124, 137]:
            with self.subTest(code=code):
                result, calls = self.invoke_install([0, code])
                self.assertEqual(result, code)
                self.assertEqual(len(calls), 2)

    def test_missing_keyring_fails_before_install(self):
        with patch.dict(helper.os.environ, ENV, clear=True), \
             patch.object(helper.platform, 'freedesktop_os_release', return_value=RELEASE), \
             patch.object(helper.platform, 'machine', return_value='x86_64'), \
             patch.object(helper.subprocess, 'check_output', return_value='amd64\n'), \
             patch.object(Path, 'is_file', return_value=False), \
             patch.object(helper, 'install') as install:
            with self.assertRaises(ValueError):
                helper.main(['glib'])
            install.assert_not_called()

    def test_workflow_keeps_fixed_groups_and_all_verification_gates(self):
        workflow = (Path(__file__).resolve().parents[1] / '.github/workflows/ci.yml').read_text()
        self.assertEqual(workflow.count('python -B scripts/install_ci_ubuntu_dependencies.py '), 3)
        for group in helper.PACKAGES:
            self.assertIn('python -B scripts/install_ci_ubuntu_dependencies.py ' + group, workflow)
        self.assertIn('python -B scripts/test_install_ci_ubuntu_dependencies.py', workflow)
        self.assertNotIn('sudo apt-get', workflow)
        self.assertIn('name: Linux core, CLI and GLib regression\n    runs-on: ubuntu-24.04', workflow)
        self.assertIn('test -x /usr/bin/strace', workflow)
        self.assertIn('test -x /usr/bin/WebKitWebDriver', workflow)
        self.assertIn('GLib string iterator regressions (debug, opt-level 2, release)', workflow)


if __name__ == '__main__':
    unittest.main()
