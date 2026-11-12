"""Offline contract tests; these are not real payload/GUI acceptance."""
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch
import qoder_complete_fixtures as complete
import qoder_private_deploy_acceptance as deployment
import qoder_releases as releases

class Tests(unittest.TestCase):
    def tearDown(self): complete.select_release(releases.DEFAULT)

    def test_closed_selection_and_default_preserved(self):
        self.assertEqual(releases.DEFAULT, '1.1.85')
        self.assertEqual(set(releases.RELEASES), {'1.1.85', '1.1.86', '1.1.87'})
        for invalid in ('latest', '1.1.88', 'https://other', ''):
            with self.assertRaises(ValueError): complete.select_release(invalid)
        with self.assertRaises(TypeError): releases.RELEASES['latest'] = ()

    def test_new_contract_exact_and_reset(self):
        complete.select_release('1.1.86')
        self.assertEqual(complete.pinned.SIZE, 78_537_803)
        self.assertEqual(complete.PAYLOAD_BYTES, 182_412_768)
        self.assertEqual(complete.release_args(), ['--release', '1.1.86'])
        self.assertEqual(complete.pinned.URL, 'https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.86/qodercli-linux-x64-baseline.tar.gz')
        complete.select_release('1.1.85')
        self.assertEqual(complete.pinned.SIZE, 78_587_775)
        self.assertEqual(complete.PAYLOAD_BYTES, 182_543_840)

    def test_187_exact_fixed_contract_and_no_default_change(self):
        complete.select_release('1.1.87')
        self.assertEqual(complete.pinned.SIZE, 78_554_027)
        self.assertEqual(complete.PAYLOAD_BYTES, 182_466_016)
        self.assertEqual(complete.PAYLOAD_SHA256, '61e318f6c4d2ebae319aecfce2f18472c51e7b413a546accddaeb82e2592af2e')
        self.assertEqual(complete.release_args(), ['--release', '1.1.87'])
        self.assertEqual(complete.pinned.URL, 'https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/releases/1.1.87/qodercli-linux-x64-baseline.tar.gz')
        self.assertEqual(releases.DEFAULT, '1.1.85')

    def test_ci_retains_three_versions_and_all_pair_inputs(self):
        workflow = (Path(__file__).resolve().parents[1] / '.github/workflows/ci.yml').read_text()
        self.assertIn("qoder_release: ['1.1.85', '1.1.86', '1.1.87']", workflow)
        self.assertIn('--additional-archive "$RUNNER_TEMP/qoder-linux-official/qodercli-linux-x64-baseline.tar.gz"', workflow)
        self.assertIn('--current-archive "$RUNNER_TEMP/qoder-linux-intermediate/qodercli-linux-x64-baseline.tar.gz"', workflow)

    def test_manifest_snapshot_matches_archive_contract(self):
        path = Path(__file__).resolve().parents[1] / 'docs/upstream/qoder/manifest-1.1.86.json'
        data = path.read_bytes()
        self.assertEqual(len(data), 3358)
        self.assertEqual(hashlib.sha256(data).hexdigest(), 'f323905da3f75088b1e9ae4cf9b0fef6f117879f4f6942b609e92228a533fa78')
        self.assertIn(releases.contract('1.1.86')[1], data.decode())

    def test_report_selected_version_not_default(self):
        complete.select_release('1.1.86')
        current = {**deployment.OUTCOME, 'version': '1.1.86', 'limitations': 'bounded'}
        deployment.validate_report(current)
        with self.assertRaises(deployment.Failure):
            deployment.validate_report({**current, 'version': '1.1.85'})
        unpack = {'state':'verified_payload_unpacked_not_installed','version':'1.1.86','installed':False,'executable':False,'payload_bytes':complete.PAYLOAD_BYTES,'payload_sha256':complete.PAYLOAD_SHA256,'limitations':'bounded'}
        complete.validate_report(unpack, True)
        with self.assertRaises(complete.PreparationFailure):
            complete.validate_report({**unpack,'version':'1.1.85'}, True)

    def test_cli_preview_passes_selected_release_and_requires_exact_report(self):
        complete.select_release('1.1.86')
        report = {'version':'1.1.86','target':'linux-amd64-baseline','payload_bytes':complete.PAYLOAD_BYTES,'payload_sha256':complete.PAYLOAD_SHA256,'executable_mode':'0700','limitations':'bounded','token':'a'*64}
        with patch.object(complete,'run_cli',return_value=report) as call, patch.object(deployment,'check_trace'):
            deployment.run_preview(Path('/cli'),Path('/cache'),Path('/target'),{},Path('/trace'))
        self.assertEqual(call.call_args.args[0][-2:], ['--release','1.1.86'])

if __name__ == '__main__': unittest.main()
