#!/usr/bin/env python3
import email.message
import io
import json
import os
from pathlib import Path
import tempfile
import sys
import time
import unittest
from unittest.mock import Mock, patch
import dsh_distribution_smoke as subject
import fetch_dsh_ui_archive as ui_fetch


class Response(io.BytesIO):
    status = 200
    def __init__(self, data, url=subject.META_URL):
        super().__init__(data); self.url=url; self.headers=email.message.Message(); self.headers['Content-Length']=str(len(data))
    def geturl(self):return self.url


class Tests(unittest.TestCase):
    def metadata(self):return {'name':'@deepseek-ai/dsh','version':subject.VERSION,'engines':{'node':'>=24.9.0'},'bin':{'dsh':'lib/bin.js'},'dist':{'tarball':subject.URL,'integrity':subject.SRI}}
    def test_raw_metadata_contract_and_duplicates(self):
        metadata=self.metadata();subject.validate_metadata(json.dumps(metadata).encode())
        for key,value in [('name','other'),('version','latest'),('engines',{}),('bin',{}),('scripts',{'postinstall':'evil'}),('os',['linux']),('cpu',['x64']),('dist',{})]:
            bad={**metadata,key:value}
            with self.subTest(key=key),self.assertRaises(subject.Failure):subject.validate_metadata(json.dumps(bad).encode())
        for raw in [b'[]',b'null',b'{"a":1,"a":2}',b'\xff',b'not json']:
            with self.assertRaises(subject.Failure):subject.validate_metadata(raw)
    def test_fetch_fixed_source_length_status_encoding_and_deadline(self):
        opener=Mock();opener.open.return_value=Response(b'abc')
        self.assertEqual(subject.fetch(subject.META_URL,4,opener),b'abc')
        self.assertEqual(opener.open.call_args.args[0].full_url,subject.META_URL)
        self.assertEqual(opener.open.call_args.kwargs['timeout'],15)
        for kind in ['redirect','status','encoding','transfer','duplicate','oversize','short','missing']:
            response=Response(b'abc')
            if kind=='redirect':response.url='https://other.example/'
            elif kind=='status':response.status=302
            elif kind=='encoding':response.headers['Content-Encoding']='gzip'
            elif kind=='transfer':response.headers['Transfer-Encoding']='chunked'
            elif kind=='duplicate':response.headers['Content-Length']='3'
            elif kind=='oversize':response.headers.replace_header('Content-Length','99')
            elif kind=='short':response.headers.replace_header('Content-Length','4')
            elif kind=='missing':del response.headers['Content-Length']
            opener.open.return_value=response
            with self.subTest(kind=kind),self.assertRaises(subject.Failure):subject.fetch(subject.META_URL,4,opener)
        opener.open.return_value=Response(b'abc')
        with self.assertRaises(subject.Failure):subject.fetch(subject.META_URL,4,opener,clock=Mock(side_effect=[0,46]))
        with self.assertRaises(subject.Failure):subject.fetch('http://registry.npmjs.org/evil',4,opener)
        with self.assertRaises(subject.Failure):subject.guard.NoRedirect().redirect_request(None,None,302,'',{},subject.URL)
    def test_archive_fixed_hash_and_size_reject_tampering(self):
        for data in [b'',b'x'*subject.SIZE,b'x'*(subject.SIZE+1)]:
            with self.assertRaises(subject.Failure):subject.validate_archive(data)
    def test_new_private_files_never_overwrite_existing_or_links(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);target=root/subject.NAME;subject.write_new(target,b'first')
            self.assertEqual(target.stat().st_mode & 0o777,0o600)
            with self.assertRaises(FileExistsError):subject.write_new(target,b'second')
            linked=root/'link';linked.symlink_to(root/'absent')
            with self.assertRaises(FileExistsError):subject.write_new(linked,b'bad')
            self.assertEqual(target.read_bytes(),b'first');self.assertFalse((root/'absent').exists())
    def test_real_cli_protocol_not_npm_and_failure_required(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);cli=root/'aisync'
            result=Mock(returncode=0,stdout=b'{}',stderr=b'')
            with patch.object(subject,'bounded_process',return_value=result) as run:
                self.assertIs(subject.run_cli(cli,['verify-package','--archive','/fixture'],root),result)
                args=run.call_args
                self.assertEqual(args.args[0],[str(cli),'dsh','verify-package','--archive','/fixture'])
                self.assertEqual(args.args[1],root)
                self.assertEqual(args.args[2]['HOME'],str(root))
    def test_cli_fixture_mutation_is_not_reported_as_readonly(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);subject.write_new(root/'input',b'original')
            def mutate(*args,**kwargs):
                (root/'input').write_bytes(b'changed')
                return Mock(returncode=0,stdout=b'{}',stderr=b'')
            with patch.object(subject,'bounded_process',side_effect=mutate):
                with self.assertRaisesRegex(subject.Failure,'cli_changed_fixture'):subject.run_cli(Path('/explicit/aisync'),['distribution-plan'],root)

    def plan(self):return {'package':'@deepseek-ai/dsh','version':subject.VERSION,'release_channel':'prerelease','archive_name':subject.NAME,'url':subject.URL,'sha256':subject.SHA256,'integrity':subject.SRI,'bytes':subject.SIZE,'max_read_bytes':65536,'node_requirement':'>=24.9.0','direct_dependency_count':90,'direct_dependency_range_count':8,'platform_compatibility':'unverified','dependencies_resolved':False,'installed':False,'runtime_verified':False,'signature_verified':False,'limitations':'bounded root only'}
    def test_cli_plan_rejects_false_capability_and_type_coercion(self):
        plan=self.plan();subject.validate_plan(plan)
        for key,value in [('installed',True),('installed',0),('bytes',True),('version','latest'),('direct_dependency_range_count',0),('limitations','')]:
            with self.subTest(key=key,value=value),self.assertRaises(subject.Failure):subject.validate_plan({**plan,key:value})
        for raw in [b'null',b'[]',b'{"x":1,"x":2}']:
            with self.assertRaises(subject.Failure):subject.decode_report(raw)
    def test_smoke_requires_real_cli_outcomes_and_never_reuses_directory(self):
        # Tiny bytes only test orchestration; actual CI does not patch digest checks.
        plan=self.plan();report={'state':'root_archive_verified','bytes':subject.SIZE,'platform_compatibility':'unverified','dependencies_resolved':False,'installed':False,'runtime_verified':False,'signature_verified':False,'plan':plan}
        for fault in (None,'valid_rejected','corrupt_accepted','wrong_report'):
            with self.subTest(fault=fault),tempfile.TemporaryDirectory() as tmp:
                root=Path(tmp)/'new';badreport={**report,'installed':True} if fault=='wrong_report' else report
                responses=[Mock(returncode=0,stdout=json.dumps(plan).encode()),Mock(returncode=1 if fault=='valid_rejected' else 0,stdout=json.dumps(badreport).encode()),Mock(returncode=0 if fault=='corrupt_accepted' else 1,stdout=b'',stderr=b'does not match the pinned size and hashes')]
                with patch.object(subject,'fetch',side_effect=[json.dumps(self.metadata()).encode(),b'abc']),patch.object(subject,'validate_archive'),patch.object(subject,'run_cli',side_effect=responses) as cli:
                    if fault:
                        with self.assertRaises(subject.Failure):subject.smoke(root,Path('/explicit/aisync'))
                    else:
                        result=subject.smoke(root,Path('/explicit/aisync'))
                        self.assertTrue(result['real_cli_valid_and_corrupt_checks']);self.assertFalse(result['dsh_executed'])
                        self.assertEqual(cli.call_count,3)
                        self.assertEqual((root/subject.NAME).read_bytes(),b'abc')
                        self.assertEqual((root/'corrupt'/subject.NAME).read_bytes(),b'`bc')
                with self.assertRaises(FileExistsError):subject.smoke(root,Path('/explicit/aisync'))

    def test_real_subprocess_output_and_timeout_bounds_reap_children(self):
        with tempfile.TemporaryDirectory() as tmp:
            result=subject.bounded_process([sys.executable,'-c','import sys;print("ok");sys.stderr.write("err")'],Path(tmp),dict(os.environ))
            self.assertEqual(result.stdout,b'ok\n');self.assertEqual(result.stderr,b'err')
            real_popen=subject.subprocess.Popen
            for source,seconds,expected in [
                ('import os;os.write(1,b"x"*2000000)',3,'cli_output_limit'),
                ('import os;os.write(2,b"x"*2000000)',3,'cli_output_limit'),
                ('import time;time.sleep(30)',0.1,'cli_timeout')]:
                children=[]
                def start(*args,**kwargs):
                    child=real_popen(*args,**kwargs);children.append(child);return child
                began=time.monotonic()
                with patch.object(subject.subprocess,'Popen',side_effect=start):
                    with self.assertRaisesRegex(subject.Failure,expected):subject.bounded_process([sys.executable,'-c',source],Path(tmp),dict(os.environ),seconds)
                self.assertIsNotNone(children[0].poll());self.assertLess(time.monotonic()-began,6)
    def test_worker_deadline_kills_group_not_only_worker(self):
        process=Mock();process.pid=123;process.is_alive.return_value=True
        with patch.object(subject.os,'killpg') as kill:
            with self.assertRaisesRegex(subject.Failure,'worker_deadline'):subject.supervise(process,Mock())
            kill.assert_called_once_with(123,subject.signal.SIGKILL)
            self.assertEqual([c.args for c in process.join.call_args_list],[(240,),(5,)])

    def test_ui_download_only_preserves_hosted_guard(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            with patch.dict(os.environ,{'GITHUB_ACTIONS':'true','RUNNER_ENVIRONMENT':'github-hosted','RUNNER_OS':'Linux','RUNNER_TEMP':tmp}),patch.object(subject.guard.platform,'machine',return_value='x86_64'):
                ui_fetch.guard(root/'new')
            with patch.dict(os.environ,{'GITHUB_ACTIONS':'false'}):
                with self.assertRaises(subject.Failure):ui_fetch.guard(root/'new')
        sender=Mock()
        with patch.object(ui_fetch.os,'setsid'),patch.object(ui_fetch,'guard'),patch.object(subject,'fetch_package',return_value=(b'metadata',b'archive',Path('/unused'))),patch.object(subject,'run_cli') as cli:
            ui_fetch.worker('/new',sender)
            cli.assert_not_called();self.assertTrue(sender.send.call_args.args[0][0])

    def test_hosted_linux_guard_not_bypassed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);cli=root/'aisync';cli.write_text('not executed');cli.chmod(0o700)
            with patch.dict(os.environ,{'GITHUB_ACTIONS':'true','RUNNER_ENVIRONMENT':'github-hosted','RUNNER_OS':'Linux','RUNNER_TEMP':tmp}),patch.object(subject.guard.platform,'machine',return_value='x86_64'):
                subject.validate_environment(root/'new',cli)
                with self.assertRaises(subject.Failure):subject.validate_environment(root,cli)
                with self.assertRaises(subject.Failure):subject.validate_environment(root/'..'/'outside',cli)
            with patch.dict(os.environ,{'GITHUB_ACTIONS':'false'}):
                with self.assertRaises(subject.Failure):subject.validate_environment(root/'new',cli)


if __name__=='__main__':unittest.main()
