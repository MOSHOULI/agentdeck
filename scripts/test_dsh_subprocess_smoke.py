#!/usr/bin/env python3
"""Offline smoke safety regressions; not real archive or lifecycle acceptance."""
import email.message
import io
import json
import os
from pathlib import Path
import tempfile
import multiprocessing
import time
import unittest
from unittest.mock import patch, Mock
import dsh_subprocess_smoke as subject

def exited_worker_with_live_child(sender, pid_file):
    os.setsid()
    child = os.fork()
    if child == 0:
        sender.close()
        time.sleep(60)
        os._exit(0)
    Path(pid_file).write_text(str(child))
    sender.send((False, 'synthetic_inner_timeout'))
    sender.close()
    os._exit(1)


def child_has_exited(child):
    # The process can disappear at any time after cleanup. Read once instead of
    # checking existence before a second, racy filesystem operation.
    try:
        status = Path(f'/proc/{child}/stat').read_text()
    except FileNotFoundError:
        return True
    return status.split()[2] == 'Z'


class Response(io.BytesIO):
    status=200
    def __init__(self,data,url):
        super().__init__(data);self.url=url;self.headers=email.message.Message();self.headers['Content-Length']=str(len(data))
    def geturl(self):return self.url

class Tests(unittest.TestCase):
    def metadata(self):return {'name':'@deepseek-ai/dsh-subprocess-local','version':subject.VERSION,'scripts':{'postinstall':'node scripts/ensure-spawn-helper.mjs'},'dist':{'tarball':subject.URL,'integrity':subject.SRI}}
    def test_metadata_exact_identity_lifecycle_source(self):
        good=self.metadata();subject.validate_metadata(json.dumps(good).encode())
        for key,value in [('name','other'),('version','latest'),('scripts',{}),('dist',{})]:
            with self.assertRaises(subject.Failure):subject.validate_metadata(json.dumps({**good,key:value}).encode())
        for raw in [b'[]',b'{"a":1,"a":2}',b'\xff']:
            with self.assertRaises(subject.Failure):subject.validate_metadata(raw)
    def test_closed_sources_and_transport(self):
        with self.assertRaises(subject.Failure):subject.fetch('https://example.org/evil',100)
        from unittest.mock import Mock
        response=Response(b'abc',subject.META_URL);opener=Mock();opener.open.return_value=response
        self.assertEqual(subject.fetch(subject.META_URL,3,opener),b'abc')
        for change in ('redirect','length','encoding'):
            response=Response(b'abc',subject.META_URL)
            if change=='redirect':response.url='https://example.org/'
            if change=='length':response.headers.replace_header('Content-Length','4')
            if change=='encoding':response.headers['Content-Encoding']='gzip'
            opener.open.return_value=response
            with self.assertRaises(subject.Failure):subject.fetch(subject.META_URL,3,opener)
    def test_archive_wrong_size_or_digest(self):
        for data in (b'',b'x'*subject.SIZE,b'x'*(subject.SIZE+1)):
            with self.assertRaises(subject.Failure):subject.validate_archive(data)
    def test_exclusive_write_and_snapshot(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);p=root/'fixture';subject.write_new(p,b'bytes')
            before=subject.snapshot(root)
            with self.assertRaises(FileExistsError):subject.write_new(p,b'overwrite')
            self.assertEqual(before,subject.snapshot(root));p.write_bytes(b'other')
            self.assertNotEqual(before,subject.snapshot(root))
    def test_no_environment_bypass(self):
        with patch.dict(os.environ,{},clear=True),self.assertRaises(subject.Failure):
            subject.validate_environment(Path('/tmp/test'),Path('/cli'))
    def report(self):
        return {'state':'subprocess_archive_static_verified','package':'@deepseek-ai/dsh-subprocess-local',
                'version':subject.VERSION,'archive_name':subject.NAME,'bytes':subject.SIZE,'sha256':subject.SHA256,'integrity':subject.SRI,
                'archive_verified_during_this_command':True,'manifest_snapshot_matched':True,'postinstall_snapshot_matched':True,
                'metadata_archive_agreement':'reviewed_declaration_fields_match','regular_member_count':35,
                'limitations':'bounded static verification', 'closure_complete':False,'install_ready':False,'dependencies_resolved':False,'runtime_verified':False,'signature_verified':False}
    def test_report_fails_closed(self):
        good=self.report();subject.validate_report(good)
        for key,value in [('bytes',True),('regular_member_count',34),('install_ready',True),('manifest_snapshot_matched',1),('postinstall_snapshot_matched',False),('sha256','0'*64),('version','latest')]:
            with self.assertRaises(subject.Failure):subject.validate_report({**good,key:value})
        with self.assertRaises(subject.Failure):subject.validate_report({'state':'root_archive_verified'})
    def test_real_cli_mutation_is_rejected_by_snapshot(self):
        from subprocess import CompletedProcess
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);p=root/'sentinel';subject.write_new(p,b'original')
            def changed(*args):
                p.write_bytes(b'changed');return CompletedProcess([],0,b'{}',b'')
            with patch.object(subject,'bounded_process',side_effect=changed), patch.object(subject,'check_trace'),self.assertRaises(subject.Failure):
                subject.run_cli(Path('/built/aisync'),['verify-subprocess-package'],root)
    def test_exec_trace_exact_initial_cli_only(self):
        with tempfile.TemporaryDirectory() as d:
            path=Path(d)/'trace';cli=Path('/built/aisync')
            good='123 execve("/built/aisync", ["aisync"], 0x0) = 0\n'
            path.write_text(good);subject.check_trace(path,cli)
            for bad in ('',good+good,good.replace('aisync','node'),good.replace(' = 0',' = -1 EACCES'), '1 execveat(4, "node", [], 0x0, 0) = 0\n','x'*65537):
                path.write_text(bad)
                with self.assertRaises(subject.Failure):subject.check_trace(path,cli)

    def test_supervisor_cleans_group_after_failed_worker_already_exited(self):
        process=Mock(pid=12345,exitcode=1);process.is_alive.return_value=False
        receiver=Mock();receiver.poll.return_value=True;receiver.recv.return_value=(False,'cli_timeout')
        events=[]
        process.join.side_effect=lambda seconds:events.append('join')
        with patch.object(subject.os,'killpg',side_effect=lambda *args:events.append('killpg')) as kill, self.assertRaises(subject.Failure):
            subject.supervise(process,receiver)
        kill.assert_called_once_with(12345,subject.signal.SIGKILL)
        self.assertEqual(events,['killpg','join'])
        receiver.poll.assert_called_once_with(240)

    def test_supervisor_cleans_on_crash_eof_timeout_and_success(self):
        for mode in ('eof','timeout','success'):
            process=Mock(pid=12345,exitcode=-9);process.is_alive.return_value=False
            receiver=Mock();receiver.poll.return_value=mode!='timeout'
            if mode=='eof':receiver.recv.side_effect=EOFError
            else:receiver.recv.return_value=(True,{'passed':True})
            with patch.object(subject.os,'killpg') as kill:
                if mode=='success':self.assertEqual(subject.supervise(process,receiver),{'passed':True})
                else:
                    with self.assertRaises(subject.Failure):subject.supervise(process,receiver)
            kill.assert_called_once_with(12345,subject.signal.SIGKILL)
            process.join.assert_called_once_with(5)

    def test_real_exited_worker_does_not_leave_live_group_child(self):
        with tempfile.TemporaryDirectory() as d:
            context=multiprocessing.get_context('fork')
            receiver,sender=context.Pipe()
            pid_file=Path(d)/'child'
            process=context.Process(target=exited_worker_with_live_child,args=(sender,str(pid_file)))
            process.start();sender.close()
            # Observe exit without reaping/reserving neither PID nor group anew.
            os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOWAIT)
            child=int(pid_file.read_text())
            confirmed_dead=False
            try:
                with self.assertRaises(subject.Failure):subject.supervise(process,receiver)
                deadline=time.monotonic()+3
                while time.monotonic()<deadline:
                    if child_has_exited(child):
                        confirmed_dead=True;break
                    time.sleep(0.01)
                else:self.fail('group child remained live after failed-worker cleanup')
            finally:
                receiver.close()
                if not confirmed_dead:
                    try:os.kill(child,subject.signal.SIGKILL)
                    except ProcessLookupError:pass
                process.join(5);process.close()

    def test_child_exit_observation_handles_disappearance_but_not_read_errors(self):
        with patch.object(Path, 'read_text', side_effect=FileNotFoundError):
            self.assertTrue(child_has_exited(12345))
        for state in ('Z', 'S', 'R'):
            with patch.object(Path, 'read_text', return_value=f'12345 (python) {state} 1'):
                self.assertEqual(child_has_exited(12345), state == 'Z')
        with patch.object(Path, 'read_text', side_effect=PermissionError), self.assertRaises(PermissionError):
            child_has_exited(12345)

    def test_ci_gate_preserves_root_lane(self):
        workflow=(Path(__file__).resolve().parents[1]/'.github/workflows/ci.yml').read_text()
        self.assertIn('python -B scripts/dsh_distribution_smoke.py',workflow)
        self.assertIn('python -B scripts/dsh_subprocess_smoke.py',workflow)
        self.assertIn('DSH-subprocess-verification-',workflow)
        self.assertNotIn('npm install',Path(subject.__file__).read_text())

if __name__=='__main__':unittest.main()
