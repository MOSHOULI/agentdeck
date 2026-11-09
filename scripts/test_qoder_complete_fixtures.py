"""Offline setup regressions; not proof of real archive or GUI success."""
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch
import qoder_complete_fixtures as subject
import fetch_qoder_ui_archive as download

class Tests(unittest.TestCase):
    def test_linux_guard_is_explicit_hosted_x64_and_windows_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'new'
            env={'GITHUB_ACTIONS':'true','RUNNER_ENVIRONMENT':'github-hosted','RUNNER_TEMP':tmp}
            with patch.dict(os.environ,{**env,'RUNNER_OS':'Windows'},clear=True): download._guard_destination(path)
            with patch.dict(os.environ,{**env,'RUNNER_OS':'Linux'},clear=True), patch.object(download.sys,'platform','linux'), patch.object(download.platform,'machine',return_value='x86_64'):download._guard_destination(path)
            for osname,host,arch in [('Linux','linux','aarch64'),('Linux','darwin','x86_64'),('macOS','darwin','arm64')]:
                with patch.dict(os.environ,{**env,'RUNNER_OS':osname},clear=True),patch.object(download.sys,'platform',host),patch.object(download.platform,'machine',return_value=arch),self.assertRaises(download.DownloadFailure): download._guard_destination(path)
            with patch.dict(os.environ,{**env,'RUNNER_OS':'Linux','RUNNER_ENVIRONMENT':'self-hosted'},clear=True),self.assertRaises(download.DownloadFailure):download._guard_destination(path)

    def test_streaming_digest_size_hash_mode_and_links(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'file';p.write_bytes(b'a'*70000);p.chmod(0o600)
            digest=hashlib.sha256(b'a'*70000).hexdigest()
            self.assertEqual(subject.file_digest(p,70000,(70000,digest)),digest)
            for limit,exact in [(65536,None),(70000,(69999,digest)),(70000,(70000,'0'*64))]:
                with self.assertRaises(subject.PreparationFailure):subject.file_digest(p,limit,exact)
            p.chmod(0o700)
            with self.assertRaises(subject.PreparationFailure):subject.file_digest(p,70000)
            p.chmod(0o600); link=Path(tmp)/'link';link.symlink_to(p)
            with self.assertRaises(subject.PreparationFailure):subject.file_digest(link,70000)
            link.unlink();os.link(p,link)
            with self.assertRaises(subject.PreparationFailure):subject.file_digest(p,70000)

    def test_reports_are_exactly_typed_and_never_mean_installed(self):
        stage={'state':'verified_archive_staged_not_installed','version':'1.1.85','target':'linux-amd64-baseline','bytes':download.SIZE,'installed':False,'extracted':False,'limitations':'fixed'}
        unpack={'state':'verified_payload_unpacked_not_installed','installed':False,'executable':False,'payload_bytes':subject.PAYLOAD_BYTES,'payload_sha256':subject.PAYLOAD_SHA256,'limitations':'fixed'}
        for report,is_unpack in [(stage,False),(unpack,True)]:
            subject.validate_report(report,is_unpack)
            for bad in [{**report,'installed':True},{**report,'installed':0},{**report,'unexpected':'SECRET'},{**report,'state':'installed'}]:
                with self.assertRaises(subject.PreparationFailure):subject.validate_report(bad,is_unpack)
            with self.assertRaises(subject.PreparationFailure):subject.validate_report(report,not is_unpack)

    def test_cli_output_bound_and_errors(self):
        env=dict(os.environ)
        self.assertEqual(subject.run_cli([sys.executable,'-c','print("{}")'],env),{})
        for code in ['print("x"*70000)', 'print("not json")', 'print(\'{"a":1,"a":2}\')', 'raise SystemExit(1)']:
            with self.assertRaises(subject.PreparationFailure):subject.run_cli([sys.executable,'-c',code],env)

    def test_timeout_kills_owned_group_and_never_retries(self):
        process=Mock();process.stdout=io.BytesIO(b'');process.pid=123456
        process.wait.side_effect=[subprocess.TimeoutExpired('cli',120),-9];process.poll.return_value=-9
        with patch.object(subject.subprocess,'Popen',return_value=process) as launch,patch.object(subject.os,'killpg') as kill:
            with self.assertRaises(subject.PreparationFailure):subject.run_cli(['/built/aisync'],{})
            launch.assert_called_once();kill.assert_called_once_with(123456,subject.signal.SIGKILL)

    def test_invalid_stage_report_prevents_unpack(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); archive=root/download.NAME
            with patch.object(download,'_guard_destination'),patch.object(subject,'file_digest'),patch.object(subject,'run_cli',return_value={'state':'fake'}) as cli:
                with self.assertRaises(subject.PreparationFailure):subject.prepare(Path('/built/aisync'),archive,root,{})
                cli.assert_called_once()
                self.assertIn('stage-package',cli.call_args.args[0]);self.assertIn('--yes',cli.call_args.args[0])
                self.assertFalse((root/'unpacked').exists())

if __name__=='__main__':unittest.main()
