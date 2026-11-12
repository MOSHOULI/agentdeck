#!/usr/bin/env python3
"""Offline authored fixture checks; never invoke npm or official Codex."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import windows_install_fixture as h

class FixtureTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.root=Path(self.tmp.name)/'fixture';self.root.mkdir()
        home=Path(self.tmp.name)/'home';home.mkdir()
        self.p=h.layout(h.namespaced(self.root),h.namespaced(home))
        for d in (Path(self.p['manager']).parent,Path(self.p['globalRoot'])):d.mkdir(parents=True)
        for k in ('queries','versions','installCalls'):Path(self.p[k]).write_bytes(b'')
        Path(self.p['node']).write_bytes(b'AUTHORED_NOT_EXECUTABLE')
        Path(self.p['canary']).write_text(h.CANARY)
        Path(self.p['latest']).write_text('1.2.3')
        self.p['nodeSha256']=hashlib.sha256(Path(self.p['node']).read_bytes()).hexdigest()
        self.p['mode']='success'
        self.node=shutil.which('node')
        self.write('success')
    def tearDown(self):self.tmp.cleanup()
    def write(self,mode):
        self.p['mode']=mode;Path(self.p['manager']).write_text(h.manager_program(self.p,mode),encoding='utf-8',newline='')
    def invoke(self,args,program=None):
        if not self.node:self.fail('Trusted Node is required for authored fixture tests')
        env={'PATH':str(Path(self.node).parent),'HOME':self.p['home'],'USERPROFILE':self.p['home'],'TEMP':self.tmp.name,'TMP':self.tmp.name}
        if os.name=='nt':
            import codex_official_install_windows as official
            env['SystemRoot']=official.windows_directory()
        return subprocess.run([self.node,str(program or self.p['manager']),*args],cwd=self.root,env=env,capture_output=True,text=True,timeout=10)
    def args(self):return ['install','--global','--prefix',self.p['prefix'],'--force=false','--no-audit','--no-fund','--no-progress','--ignore-scripts','@openai/codex@1.2.3']
    def test_queries_do_not_install_or_probe(self):
        for kind,expected in [('prefix',self.p['prefix']),('root',self.p['globalRoot'])]:
            r=self.invoke([kind,'--global']);self.assertEqual(r.returncode,0,r.stderr);self.assertEqual(r.stdout.strip(),expected)
        self.assertEqual(h.assert_phase(self.p,'absent')['versionProbes'],0)
        self.assertEqual(Path(self.p['queries']).read_bytes(),b'pr')
    def test_latest_changes_independently_and_exact_install_only(self):
        manager=Path(self.p['manager']).read_bytes()
        for version in ('1.2.3','1.2.4'):
            Path(self.p['latest']).write_text(version)
            result=self.invoke(['view','@openai/codex@latest','version','--json','--fetch-retries=0','--fetch-timeout=25000'])
            self.assertEqual((result.returncode,json.loads(result.stdout)),(0,version))
            h.assert_phase(self.p,'absent')
        self.assertEqual(Path(self.p['manager']).read_bytes(),manager)
        self.assertEqual(h.facts(self.p)['metadataQueries'],2)
        for spec in ('@openai/codex','@openai/codex@latest','@openai/codex@1.2.4'):
            self.assertEqual(self.invoke(self.args()[:-1]+[spec]).returncode,64)
            h.assert_phase(self.p,'absent')

    def test_install_then_real_authored_version_probe_and_no_configuration(self):
        r=self.invoke(self.args());self.assertEqual(r.returncode,0,r.stderr)
        with self.assertRaisesRegex(h.Failure,'version_not_probed'):h.assert_phase(self.p,'installed')
        entry=Path(self.p['package'])/'bin/codex.js';r=self.invoke(['--version'],entry)
        self.assertEqual((r.returncode,r.stdout.strip()),(0,'codex-cli 1.2.3'))
        self.assertEqual(h.assert_phase(self.p,'installed')['versionProbes'],1)
        self.assertNotEqual(self.invoke(self.args()).returncode,0)
        self.assertEqual(Path(self.p['marker']).read_bytes(),h.MARKER)
        with self.assertRaisesRegex(h.Failure,'unexpected_install_calls'):h.assert_phase(self.p,'installed')
    def test_failure_partial_state_retained_and_retries_are_observable(self):
        self.write('fail');self.assertEqual(self.invoke(self.args()).returncode,64)
        partial=Path(self.p['package'])/'partial-install.txt'
        def snapshot():
            result=[]
            for p in (partial,Path(self.p['marker'])):
                data=p.read_bytes();m=p.stat()
                result.append((data,m.st_dev,m.st_ino,m.st_mode,m.st_nlink,m.st_size,m.st_mtime_ns,m.st_ctime_ns))
            return result
        self.assertIn(h.CANARY.encode(), partial.read_bytes())
        before=snapshot()
        for _ in range(3):
            result=h.assert_phase(self.p,'failed')
            self.assertEqual((result['installCalls'],result['versionProbes'],result['partialInstallRetained']),(1,0,True))
            self.assertEqual(self.invoke(['prefix','--global']).returncode,0)
        self.assertEqual(snapshot(),before)
        self.assertEqual(self.invoke(self.args()).returncode,64)
        self.assertEqual(Path(self.p['installCalls']).read_bytes(),b'ii')
        self.assertEqual(partial.read_bytes(),h.PARTIAL)
        with self.assertRaisesRegex(h.Failure,'unexpected_install_calls'):h.assert_phase(self.p,'failed')
    def test_failure_assertion_rejects_missing_changed_or_extra_residue_and_probe(self):
        self.write('fail');self.assertEqual(self.invoke(self.args()).returncode,64)
        partial=Path(self.p['package'])/'partial-install.txt'
        partial.write_bytes(b'changed')
        with self.assertRaisesRegex(h.Failure,'failure_state_mismatch'):h.assert_phase(self.p,'failed')
        partial.write_bytes(h.PARTIAL);extra=partial.parent/'extra';extra.write_bytes(b'x')
        with self.assertRaisesRegex(h.Failure,'failure_state_mismatch'):h.assert_phase(self.p,'failed')
        extra.unlink();Path(self.p['versions']).write_bytes(b'v')
        with self.assertRaisesRegex(h.Failure,'unexpected_version_probe'):h.assert_phase(self.p,'failed')
        Path(self.p['versions']).write_bytes(b'');partial.unlink()
        with self.assertRaisesRegex(h.Failure,'failure_state_mismatch'):h.assert_phase(self.p,'failed')
    def test_unknown_arguments_and_missing_ignore_scripts_do_nothing(self):
        cases=[['--version'],['exec','anything'],['prefix'],self.args()[:-1]+['@evil/package'],[x for x in self.args() if x!='--ignore-scripts']]
        for args in cases:self.assertNotEqual(self.invoke(args).returncode,0);h.assert_phase(self.p,'absent')
    def test_occupied_launcher_is_not_overwritten(self):
        target=Path(self.p['prefix'])/'codex.ps1';target.write_bytes(b'canary')
        self.assertNotEqual(self.invoke(self.args()).returncode,0);self.assertEqual(target.read_bytes(),b'canary');self.assertFalse(Path(self.p['marker']).exists())
    def test_wrong_version_arguments_are_not_counted(self):
        self.assertEqual(self.invoke(self.args()).returncode,0)
        r=self.invoke(['--help'],Path(self.p['package'])/'bin/codex.js');self.assertNotEqual(r.returncode,0);self.assertEqual(Path(self.p['versions']).read_bytes(),b'')
    def test_canary_and_configuration_mutation_are_rejected(self):
        Path(self.p['canary']).write_bytes(b'changed')
        with self.assertRaisesRegex(h.Failure,'canary_changed'):h.assert_phase(self.p,'absent')
        Path(self.p['canary']).write_text(h.CANARY)
        Path(self.p['latest']).write_text('1.2.3');(Path(self.p['home'])/'.codex').mkdir()
        with self.assertRaisesRegex(h.Failure,'configuration_created'):h.assert_phase(self.p,'absent')
    def test_exact_cmd_shim_and_authored_source_have_no_dynamic_executor(self):
        actual=h.shim('node_modules/@openai/codex/bin/codex.js')
        fixture=Path(__file__).parent/'fixtures/codex-windows-shims/codex.cmd.txt'
        self.assertEqual(actual,fixture.read_bytes())
        text=h.manager_program(self.p,'success')+h.codex_program(self.p)
        for forbidden in ('node:child_process','node:http','node:https','fetch(','eval(','exec('):self.assertNotIn(forbidden,text)
    def test_only_exact_existing_lifecycle_sentinel_is_allowed(self):
        product=Path(self.p['home'])/'.agentdeck';product.mkdir();sentinel=product/'acceptance-user-data.txt'
        sentinel.write_text('Synthetic data, must survive installation, upgrade and uninstall.\n')
        h.assert_phase(self.p,'absent')
        sentinel.write_text('changed')
        with self.assertRaisesRegex(h.Failure,'sentinel_changed'):h.assert_phase(self.p,'absent')
    @unittest.skipUnless(os.name=='nt','real Windows trusted Node PE preparer')
    def test_windows_preparer_and_manifest_contract(self):
        node=Path(self.node).resolve();root=Path(self.tmp.name)/'actual-prepared'
        manifest=h.prepare(root,node,Path(self.p['home']),'success');m=h.load(manifest)
        self.assertEqual(m['display']['managerPath'],m['manager']);self.assertTrue(m['prefix'].startswith('\\\\?\\'))
        h.assert_phase(m,'absent')
        result=subprocess.run([m['node'],m['manager'],'prefix','--global'],capture_output=True,text=True,timeout=10)
        self.assertEqual(result.returncode,0,result.stderr);self.assertEqual(result.stdout.strip(),m['prefix'])
    def test_failure_diagnostic_never_exposes_messages_or_dynamic_types(self):
        for error in (FileNotFoundError('SECRET_PATH'),h.Failure('SECRET_PATH'),ValueError('SECRET_PATH')):
            report=h.failure_diagnostic(error);self.assertNotIn('SECRET_PATH',json.dumps(report))
        self.assertEqual(h.failure_diagnostic(h.Failure('canary_changed'))['failureCode'],'canary_changed')
        SecretType=type('SECRET_PATH',(Exception,),{})
        self.assertEqual(h.failure_diagnostic(SecretType())['exceptionType'],'Exception')
    @unittest.skipUnless(os.name=='nt','Windows Unicode CLI pipe roundtrip')
    def test_windows_nonascii_space_cli_prepare_load_and_query(self):
        root=Path(self.tmp.name)/'AgentDeck 验收 空格 fixture';home=Path(self.tmp.name)/'用户 配置 home';home.mkdir()
        env=dict(os.environ,PYTHONIOENCODING='cp1252',PYTHONUTF8='0')
        helper=Path(h.__file__).resolve()
        result=subprocess.run([sys.executable,str(helper),'--prepare',str(root),'--node',str(Path(self.node).resolve()),'--home',str(home)],env=env,capture_output=True,timeout=30)
        self.assertEqual(result.returncode,0,result.stderr.decode('utf-8'))
        manifest=Path(result.stdout.decode('utf-8').strip());self.assertEqual(manifest,root/'fixture.json')
        m=h.load(manifest)
        check=subprocess.run([sys.executable,str(helper),'--assert','absent','--manifest',str(manifest)],env=env,capture_output=True,timeout=30)
        self.assertEqual(check.returncode,0,check.stderr.decode('utf-8'));self.assertEqual(json.loads(check.stdout)['versionProbes'],0)
        # Authored manager query via trusted Node, not npm or an official agent.
        import codex_official_install_windows as official
        clean={'SystemRoot':official.windows_directory(),'HOME':str(home),'USERPROFILE':str(home),'TEMP':str(home),'TMP':str(home)}
        query=subprocess.run([m['node'],m['manager'],'prefix','--global'],env=clean,capture_output=True,timeout=10)
        self.assertEqual(query.returncode,0);self.assertEqual(query.stdout.decode('utf-8').strip(),m['prefix'])
    @unittest.skipIf(os.name=='nt','Unix authored symlink negative fixture')
    def test_symlink_query_log_refused(self):
        log=Path(self.p['queries']);log.unlink();target=self.root/'outside';target.write_bytes(b'canary');log.symlink_to(target)
        self.assertNotEqual(self.invoke(['prefix','--global']).returncode,0);self.assertEqual(target.read_bytes(),b'canary')

if __name__=='__main__':unittest.main()
