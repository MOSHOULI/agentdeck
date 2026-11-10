"""Offline fail-closed harness tests, NOT official deployment acceptance."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch

import qoder_private_deploy_acceptance as h


class Tests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_guard_cannot_run_outside_hosted_ci(self):
        with patch.dict(os.environ, {}, clear=True), self.assertRaises(h.Failure):
            h.guard(Path('/cli'), self.root / 'archive', self.root / 'evidence')

    def test_file_hash_requires_exact_private_mode_bytes_and_identity(self):
        path = self.root / 'payload'; h.private_file(path, b'not executable test data')
        expected = hashlib.sha256(path.read_bytes()).hexdigest()
        self.assertEqual(h.file_hash(path, 0o600, 24, expected), expected)
        for kwargs in ({'size': 23}, {'expected': '0'*64}, {'mode': 0o700}):
            with self.assertRaises(h.Failure): h.file_hash(path, **({'mode': 0o600} | kwargs))
        link = self.root / 'link'; link.symlink_to(path)
        with self.assertRaises(h.Failure): h.file_hash(link, 0o600)
        link.unlink(); os.link(path, link)
        with self.assertRaises(h.Failure): h.file_hash(path, 0o600)

    def test_snapshot_detects_config_write_metadata_members_and_links(self):
        path = self.root / 'config'; h.private_file(path, b'private fixture')
        before = h.snapshot(self.root)
        path.write_bytes(b'new configuration')
        self.assertNotEqual(h.snapshot(self.root), before)
        path.chmod(0o644)
        with self.assertRaises(h.Failure): h.snapshot(self.root)
        path.chmod(0o600); (self.root / 'linked').symlink_to(path)
        with self.assertRaises(h.Failure): h.snapshot(self.root)

    def test_trace_requires_exactly_initial_cli_success_no_child_exec(self):
        trace = self.root / 'trace'; cli = Path('/built/aisync')
        valid = '123 execve("/built/aisync", ["/built/aisync", "qoder"], 0xabc /* 4 vars */) = 0\n'
        trace.write_text(valid); self.assertEqual(h.check_trace(trace, cli), 1)
        for data in ('', valid+valid, valid+'456 execve("/private/qodercli", [], 0xabc) = 0\n',
                     valid.replace(' = 0', ' = -1 EACCES'), valid.replace('aisync', 'qodercli'),
                     '123 execveat(4, "qodercli", [], 0x0, 0) = 0\n', 'x'*65537):
            trace.write_text(data)
            with self.assertRaises(h.Failure): h.check_trace(trace, cli)

    def test_report_exact_types_and_no_runtime_or_activation_claims(self):
        valid = {**h.OUTCOME, 'limitations': 'reviewed boundaries'}
        h.validate_report(valid)
        for invalid in ({**valid,'execution_verified':True}, {**valid,'activated':0},
                        {**valid,'installed':True}, {**valid,'version':'1.1.86'},
                        {**valid,'limitations':''}, {**valid,'executable_mode':'0755'}):
            with self.assertRaises(h.Failure): h.validate_report(invalid)

    def test_fixed_destination_contract_and_mutation_rejection(self):
        # Unit-only tiny fixture. No production hash/size override CLI exists.
        data = b'sentinel, never a program'; digest = hashlib.sha256(data).hexdigest()
        payload = self.root / 'qodercli'; h.private_file(payload, data); payload.chmod(0o700)
        intent = {'format':1,'state':'private_deployment_started','version':'1.1.85',
                  'target':'linux-amd64-baseline','execution_verified':False,'activated':False}
        complete = {**h.OUTCOME,'format':1,'payload_bytes':len(data),'payload_sha256':digest,'limitations':'bounded'}
        h.private_file(self.root / 'deployment.intent.json', json.dumps(intent).encode())
        h.private_file(self.root / 'deployment.complete.json', json.dumps(complete).encode())
        # PAYLOAD_BYTES is also an upper bound for records; keep that bound for
        # snapshot then patch only the final exact payload validator.
        original_hash = h.file_hash
        def tiny_hash(path, mode, size=None, expected=None):
            if path.name == 'qodercli' and size is not None:
                size, expected = len(data), digest
            return original_hash(path, mode, size, expected)
        complete['payload_bytes'] = h.complete.PAYLOAD_BYTES
        complete['payload_sha256'] = h.complete.PAYLOAD_SHA256
        (self.root / 'deployment.complete.json').write_text(json.dumps(complete))
        with patch.object(h, 'file_hash', side_effect=tiny_hash):
            h.validate_destination(self.root)
            payload.chmod(0o600)
            with self.assertRaises(h.Failure): h.validate_destination(self.root)
            payload.chmod(0o700); (self.root / 'unexpected').write_text('x')
            with self.assertRaises(h.Failure): h.validate_destination(self.root)

    def test_duplicate_record_key_rejected(self):
        path = self.root / 'record'; h.private_file(path, b'{"format":1,"format":1}')
        with self.assertRaises(h.Failure): h.load_record(path)

    def test_preview_trace_required_and_token_not_omitted(self):
        report = {'version':'1.1.85','target':'linux-amd64-baseline',
                  'payload_bytes':h.complete.PAYLOAD_BYTES,'payload_sha256':h.complete.PAYLOAD_SHA256,
                  'executable_mode':'0700','token':'a'*64,'limitations':'reviewed'}
        with patch.object(h.complete,'run_cli',return_value=report) as run, patch.object(h,'check_trace') as trace:
            token = h.run_preview(Path('/cli'), self.root, self.root/'new', {}, self.root/'trace')
            self.assertEqual(token,'a'*64); trace.assert_called_once()
            self.assertIn('deploy-preview',run.call_args.args[0])
            self.assertEqual(run.call_args.args[0][0],'/usr/bin/strace')
        with patch.object(h.complete,'run_cli',return_value={**report,'token':'bad'}), patch.object(h,'check_trace'), self.assertRaises(h.Failure):
            h.run_preview(Path('/cli'), self.root, self.root/'new', {}, self.root/'trace')

    def test_deploy_trace_required_no_execution_fallback(self):
        with patch.object(h.complete,'run_cli',return_value={}) as run, patch.object(h,'check_trace',side_effect=h.Failure('trace')):
            with self.assertRaises(h.Failure): h.run_deploy(Path('/cli'),self.root,self.root/'new','a'*64,{},self.root/'trace')
            run.assert_called_once()
            self.assertIn('--preview-token',run.call_args.args[0]); self.assertIn('--yes',run.call_args.args[0])

    def test_repeat_requires_nonzero_and_trace(self):
        process=Mock(); process.wait.return_value=1; process.poll.return_value=1
        with patch.object(h.subprocess,'Popen',return_value=process) as launch, patch.object(h,'check_trace') as trace:
            h.run_deploy(Path('/cli'),self.root,self.root/'new','a'*64,{},self.root/'trace',reject=True)
            launch.assert_called_once(); trace.assert_called_once()
        process.wait.return_value=0; process.poll.return_value=0
        with patch.object(h.subprocess,'Popen',return_value=process), self.assertRaises(h.Failure):
            h.run_deploy(Path('/cli'),self.root,self.root/'new','a'*64,{},self.root/'trace',reject=True)

    def test_orchestration_fails_on_source_config_or_repeat_mutation(self):
        for fault in (None, 'source', 'config', 'repeat'):
            with self.subTest(fault=fault), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); archive=root/'archive'; h.private_file(archive,b'archive sentinel')
                def prepare(cli, archive, caches, env):
                    unpacked=caches/'unpacked'; unpacked.mkdir(mode=0o700)
                    h.private_file(unpacked/'qodercli',b'cache sentinel')
                    return {'unpacked':unpacked}, {}
                def deploy(cli, cache, destination, token, env, trace, reject=False):
                    if not reject:
                        destination.mkdir(mode=0o700)
                        h.private_file(destination/'qodercli',b'deployed sentinel')
                        if fault == 'source': (cache/'qodercli').write_bytes(b'changed')
                        if fault == 'config': (Path(env['HOME'])/'.qoder'/'sentinel.json').write_bytes(b'changed')
                    elif fault == 'repeat': (destination/'qodercli').write_bytes(b'changed')
                    return {**h.OUTCOME,'limitations':'reviewed'}
                with patch.dict(os.environ,{'RUNNER_TEMP':tmp}), patch.object(h,'guard'), \
                     patch.object(h.complete,'file_digest',return_value='fixed'), patch.object(h.complete,'prepare',side_effect=prepare), \
                     patch.object(h,'run_preview',return_value='a'*64), patch.object(h,'run_deploy',side_effect=deploy), patch.object(h,'validate_destination'):
                    self.assertEqual(h.run(Path('/built/aisync'),archive,root/'evidence'), fault is None)
                result=json.loads((root/'evidence'/'result.json').read_text())
                self.assertEqual(result['passed'],fault is None)
                self.assertNotIn(tmp,json.dumps(result))
                self.assertNotIn('.trace',json.dumps(result)); self.assertNotIn('execve(',json.dumps(result))

    def test_ci_has_mandatory_gate_and_only_fixed_evidence_uploaded(self):
        workflow=(Path(__file__).resolve().parents[1]/'.github/workflows/ci.yml').read_text()
        self.assertIn('python -B scripts/qoder_private_deploy_acceptance.py',workflow)
        self.assertIn('qoder-private-deploy-evidence/result.json',workflow)
        self.assertIn('dbus-x11 strace',workflow)
        source=Path(h.__file__).read_text()
        self.assertNotIn('--hash',source)
        self.assertNotIn('sysctl',source)
        self.assertNotIn('shell=True',source)


if __name__ == '__main__': unittest.main()
