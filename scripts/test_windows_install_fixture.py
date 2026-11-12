#!/usr/bin/env python3
"""Offline authored fixture checks; never invoke npm or official Codex."""
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
import windows_install_fixture as h
from install_protection_fixture import Failure as ProtectionFailure

def remove_owned_node_copy(path, temporary_root):
    """After subprocess.run has waited/reaped, tolerate only transient Windows
    image sharing violations on this test's exact copied Node, not arbitrary
    cleanup failures. No process termination, install retry or ignored errors.
    """
    path=Path(path);root=Path(temporary_root).resolve(strict=True)
    if path.name!='node.exe' or path.parent.name!='manager' or not path.is_absolute():
        raise ValueError('unexpected_owned_node_copy')
    if path.is_symlink() or not path.parent.resolve(strict=True).is_relative_to(root):
        raise ValueError('unexpected_owned_node_copy')
    for parent in path.parents:
        if parent==root:break
        if parent.is_symlink():raise ValueError('unexpected_owned_node_copy')
    for attempt in range(40):
        try:
            path.unlink()
            return
        except OSError as error:
            if getattr(error,'winerror',None)!=32 or attempt==39:raise
            time.sleep(0.05)


class OwnedNodeCleanupTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.node=self.root/'prepared'/'manager'/'node.exe'
        self.node.parent.mkdir(parents=True);self.node.write_bytes(b'owned authored copy')
    def sharing_error(self):
        error=PermissionError('authored sharing violation');error.winerror=32;return error
    def test_sharing_violation_retries_only_exact_owned_copy(self):
        unlink=Path.unlink;calls=[]
        def blocked_once(path,*args,**kwargs):
            calls.append(path)
            if len(calls)==1:raise self.sharing_error()
            return unlink(path,*args,**kwargs)
        with patch.object(Path,'unlink',blocked_once),patch.object(time,'sleep') as sleep:
            remove_owned_node_copy(self.node,self.root)
        self.assertEqual(calls,[self.node,self.node]);sleep.assert_called_once_with(0.05)
        self.assertFalse(self.node.exists())
    def test_persistent_sharing_violation_still_fails(self):
        with patch.object(Path,'unlink',side_effect=self.sharing_error()) as unlink,patch.object(time,'sleep') as sleep:
            with self.assertRaises(PermissionError):remove_owned_node_copy(self.node,self.root)
        self.assertEqual(unlink.call_count,40);self.assertEqual(sleep.call_count,39)
        self.assertEqual(self.node.read_bytes(),b'owned authored copy')
    def test_other_errors_and_outside_paths_are_not_retried(self):
        denied=PermissionError('authored access denied');denied.winerror=5
        with patch.object(Path,'unlink',side_effect=denied) as unlink,patch.object(time,'sleep') as sleep:
            with self.assertRaises(PermissionError):remove_owned_node_copy(self.node,self.root)
        self.assertEqual(unlink.call_count,1);sleep.assert_not_called()
        outside=self.root/'other-root';outside.mkdir()
        with self.assertRaises(ValueError):remove_owned_node_copy(self.node,outside)
        other=self.node.with_name('other.exe');other.write_bytes(b'other')
        with self.assertRaises(ValueError):remove_owned_node_copy(other,self.root)
        self.assertEqual(other.read_bytes(),b'other')


class FixtureTests(unittest.TestCase):
    def setUp(self):
        self.owned_node_copies=[]
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
    def tearDown(self):
        # Both preparer cases invoke only synchronous subprocess.run calls.
        # Those calls wait/reap even on timeout before teardown can begin.
        for node in self.owned_node_copies:remove_owned_node_copy(node,Path(self.tmp.name))
        self.tmp.cleanup()
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
    def failure_script_argument_line(self):
        source=(Path(__file__).parent/'test_windows_failed_install_state.ps1').read_text(encoding='utf-8')
        lines=[line.strip() for line in source.splitlines() if '$info.ArgumentList.Add($argument)' in line]
        self.assertEqual(len(lines),1)
        return lines[0]

    def assert_failure_script_args(self,args):
        self.assertEqual(args[0],self.p['manager'])
        self.assertEqual(args[1:],self.args())
        self.write('fail')
        result=self.invoke(args[1:])
        self.assertEqual(result.returncode,64,result.stderr)
        counts=h.assert_phase(self.p,'failed')
        self.assertEqual((counts['installCalls'],counts['versionProbes'],counts['partialInstallRetained']),(1,0,True))

    def test_standalone_failure_script_uses_fixture_exact_install_contract(self):
        # Execute the authored fixture with argv read from the actual PS script,
        # not a second copied recipe that could independently become stale.
        line=self.failure_script_argument_line()
        prefix='foreach ($argument in @(';suffix=')) { $info.ArgumentList.Add($argument) }'
        self.assertTrue(line.startswith(prefix) and line.endswith(suffix))
        tokens=line[len(prefix):-len(suffix)].split(',')
        args=[]
        for token in tokens:
            if token=='$fixture.manager':args.append(self.p['manager'])
            elif token=='$fixture.prefix':args.append(self.p['prefix'])
            else:
                self.assertRegex(token,r"^'[a-zA-Z0-9@./=:_-]+'$")
                args.append(token[1:-1])
        self.assert_failure_script_args(args)

    @unittest.skipUnless(shutil.which('pwsh'),'PowerShell not available')
    def test_actual_powershell_failure_script_argument_list(self):
        # Run only the real script's ArgumentList construction on this host.
        # Full Windows ACL/timestamp snapshot acceptance remains Windows CI.
        encoded=base64.b64encode(json.dumps(self.p).encode()).decode()
        code=("$ErrorActionPreference='Stop'; $fixture=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('"+encoded+"')) | ConvertFrom-Json; "
              "$info=[Diagnostics.ProcessStartInfo]::new(); "+self.failure_script_argument_line()+
              "; ConvertTo-Json -InputObject @($info.ArgumentList) -Compress")
        result=subprocess.run([shutil.which('pwsh'),'-NoProfile','-NonInteractive','-EncodedCommand',base64.b64encode(code.encode('utf-16le')).decode()],
                              capture_output=True,text=True,timeout=15,check=True)
        self.assert_failure_script_args(json.loads(result.stdout))

    def test_successful_npm_wrong_root_is_complete_unprobed_and_unchanged(self):
        self.write('wrong-root')
        result=self.invoke(self.args())
        self.assertEqual(result.returncode,0,result.stderr)
        def snapshot():
            root=Path(self.p['prefix']);values=[]
            for p in [root,*sorted(root.rglob('*')),Path(self.p['marker'])]:
                m=p.stat();values.append((str(p),p.read_bytes() if p.is_file() else None,m.st_dev,m.st_ino,m.st_mode,m.st_nlink,m.st_size,m.st_mtime_ns,m.st_ctime_ns))
            return values
        before=snapshot()
        for _ in range(3):
            counts=h.assert_phase(self.p,'wrong-root')
            self.assertEqual((counts['installCalls'],counts['versionProbes'],counts['completeWrongRootRetained']),(1,0,True))
        self.assertEqual(snapshot(),before)
        # Demonstrate the authored launcher is runnable and the guard detects it;
        # this deliberately executed offline negative control is not GUI evidence.
        result=self.invoke(['--version'],Path(self.p['package'])/'bin/codex.js')
        self.assertEqual(result.returncode,0,result.stderr)
        with self.assertRaisesRegex(h.Failure,'unexpected_version_probe'):h.assert_phase(self.p,'wrong-root')

    def test_wrong_root_sentinel_also_detects_unexpected_invocation(self):
        self.write('wrong-root');self.assertEqual(self.invoke(self.args()).returncode,0)
        result=self.invoke(['--help'],Path(self.p['package'])/'bin/codex.js')
        self.assertEqual(result.returncode,64)
        self.assertEqual(Path(self.p['versions']).read_bytes(),b'v')
        with self.assertRaisesRegex(h.Failure,'unexpected_version_probe'):h.assert_phase(self.p,'wrong-root')

    def test_authored_relocation_preserves_prefix_identity_and_requires_new_install(self):
        self.write('wrong-root');self.assertEqual(self.invoke(self.args()).returncode,0)
        parents=[Path(self.p['prefix']),Path(self.p['globalRoot']),Path(self.p['package']).parent]
        identities=[(p.stat().st_dev,p.stat().st_ino) for p in parents]
        old=(Path(self.p['package'])/'package.json').read_bytes()
        h.relocate_owned_residue(self.p)
        self.assertEqual([(p.stat().st_dev,p.stat().st_ino) for p in parents],identities)
        self.assertEqual((Path(self.p['root'])/'retained-package/package.json').read_bytes(),old)
        h.assert_phase(self.p,'empty-owned')
        h.authored_conflict(self.p)
        with self.assertRaises(h.Failure):h.assert_phase(self.p,'empty-owned')
        h.authored_conflict(self.p,True);h.assert_phase(self.p,'empty-owned')
        self.assertEqual(self.invoke(self.args()).returncode,0)
        self.assertEqual(h.assert_phase(self.p,'wrong-root',2)['installCalls'],2)
        with self.assertRaises(h.Failure):h.relocate_owned_residue(self.p)

    def test_protection_allowlist_accepts_only_exact_valid_record(self):
        directory=Path(self.p['home'])/'.agentdeck/install-protection/codex'
        directory.mkdir(parents=True)
        for p in (directory.parent,directory):p.chmod(0o700)
        record={'schema':1,'platform':'windows' if os.name=='nt' else 'linux','phase':'clear','attempt':None,'prefix':None,'prefixIdentity':None}
        (directory/'gate.lock').write_bytes(b'');(directory/'state.json').write_text(json.dumps(record),encoding='utf-8')
        for p in directory.iterdir():p.chmod(0o600)
        h.no_configuration(self.p)
        (directory/'unexpected').write_bytes(b'')
        with self.assertRaises(ProtectionFailure):h.no_configuration(self.p)
        (directory/'unexpected').unlink();(directory/'state.json').write_bytes(b'corrupt')
        with self.assertRaises(ProtectionFailure):h.no_configuration(self.p)

    def test_wrong_root_guard_rejects_missing_or_changed_complete_files(self):
        self.write('wrong-root');self.assertEqual(self.invoke(self.args()).returncode,0)
        manifest=Path(self.p['package'])/'package.json';old=manifest.read_bytes()
        manifest.write_bytes(old.replace(b'1.2.4',b'1.2.3'))
        with self.assertRaisesRegex(h.Failure,'installed_bytes_changed'):h.assert_phase(self.p,'wrong-root')
        manifest.write_bytes(old)
        launcher=Path(self.p['prefix'])/'codex.cmd';launcher.write_bytes(b'changed')
        with self.assertRaisesRegex(h.Failure,'shim_changed'):h.assert_phase(self.p,'wrong-root')

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
        manifest=h.prepare(root,node,Path(self.p['home']),'success')
        self.owned_node_copies.append(root/'manager'/'node.exe')
        m=h.load(manifest)
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
        self.owned_node_copies.append(root/'manager'/'node.exe')
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
