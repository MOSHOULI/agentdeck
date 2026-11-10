#!/usr/bin/env python3
"""Offline parser/layout tests; Windows runner fixtures execute only authored Python."""
import base64
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import time
import unittest
from unittest.mock import patch
import codex_official_install_windows as h
import windows_job_process as job

class Offline(unittest.TestCase):
    def test_pins(self):
        self.assertEqual(h.NATIVE.version,'0.167.0-win32-x64');self.assertEqual(h.NATIVE.files,47);self.assertEqual(h.NATIVE.expanded,501483647)
        self.assertEqual(len(base64.b64decode(h.NATIVE.integrity)),64)
        self.assertEqual(h.NATIVE.url,'https://registry.npmjs.org/@openai/codex/-/codex-0.167.0-win32-x64.tgz')
    def test_windows_names(self):
        for name in ('file:ads','NUL.txt','con','COM1','LPT9','name.','name ','a?b','AUX','COM¹'):
            self.assertFalse(h.windows_component(name),name)
        for name in ('codex.exe','README.md','codex-resources'):self.assertTrue(h.windows_component(name))
    def test_duplicate_json(self):
        with self.assertRaises(h.Failure):h.json_object(b'{"a":1,"a":2}')
    def test_manifest(self):
        root={'name':h.ROOT_NAME,'version':h.VERSION,'bin':{'codex':'bin/codex.js'},'optionalDependencies':h.OPTIONAL}
        h.validate_manifest(root,h.ROOT)
        for key in ('scripts','dependencies','peerDependencies'):
            with self.assertRaises(h.Failure):h.validate_manifest(dict(root,**{key:{'evil':'anything'}}),h.ROOT)
        native={'name':h.ROOT_NAME,'version':h.NATIVE.version,'os':['win32'],'cpu':['x64']}
        h.validate_manifest(native,h.NATIVE)
        with self.assertRaises(h.Failure):h.validate_manifest(dict(native,os=['linux']),h.NATIVE)
    def test_valid_synthetic_root_inventory(self):
        manifest=json.dumps({'name':h.ROOT_NAME,'version':h.VERSION,'bin':{'codex':'bin/codex.js'},'optionalDependencies':h.OPTIONAL}).encode()
        contents={'package.json':manifest,'bin/codex.js':b'#!/usr/bin/env node\n'}
        contents['README.md']=b'x'*(h.ROOT.expanded-sum(map(len,contents.values())))
        with tempfile.TemporaryDirectory() as t:
            p=Path(t)/'root.tgz'
            with tarfile.open(p,'w:gz') as archive:
                for name,data in contents.items():
                    member=tarfile.TarInfo('package/'+name);member.mode=0o644;member.size=len(data);archive.addfile(member,io.BytesIO(data))
            with patch.object(h,'file_digest',return_value=base64.b64decode(h.ROOT.integrity)):
                records=h.inventory(p,h.ROOT)
            self.assertEqual(set(records),set(contents));self.assertEqual(sum(r['size'] for r in records.values()),h.ROOT.expanded)
    def test_archive_rejects_windows_aliases_and_links(self):
        cases = [
            (('package/A','package/a'),'archive_duplicate_or_noncanonical_path'),
            (('package/A/one','package/a/two'),'archive_case_alias'),
            (('package/Foo','package/foo/child'),'archive_case_alias'),
            (('package/a:ads',),'archive_member_rejected'),
            (('package/CON',),'archive_member_rejected'),
            (('package/a.',),'archive_member_rejected'),
            (('package/../x',),'archive_member_rejected'),
            (('package/a\\x',),'archive_member_rejected'),
            (('package/link',),'archive_member_rejected'),
        ]
        for names, expected in cases:
            with tempfile.TemporaryDirectory() as t:
                p=Path(t)/'input.tgz'
                with tarfile.open(p,'w:gz') as archive:
                    for name in names:
                        member=tarfile.TarInfo(name);member.mode=0o644
                        if name.endswith('/link'):member.type=tarfile.SYMTYPE;member.linkname='target';archive.addfile(member)
                        else:member.size=1;archive.addfile(member,io.BytesIO(b'x'))
                with patch.object(h,'file_digest',return_value=base64.b64decode(h.ROOT.integrity)):
                    with self.assertRaisesRegex(h.Failure,'^'+expected+'$'):h.inventory(p,h.ROOT)
    def test_environment_and_direct_argv(self):
        with tempfile.TemporaryDirectory() as t:
            paths=h.prepare(Path(t)/'isolated');node=Path(t)/'node.exe';npm=Path(t)/'npm-cli.js'
            with patch.object(h,'windows_directory',return_value='C:\\Windows'),patch.dict(os.environ,{'NODE_OPTIONS':'evil','NPM_TOKEN':'secret','HTTPS_PROXY':'proxy'}):env=h.clean_environment(paths,node)
            self.assertNotIn('NODE_OPTIONS',env);self.assertNotIn('NPM_TOKEN',env);self.assertNotIn('HTTPS_PROXY',env);self.assertNotIn('COMSPEC',env)
            self.assertEqual(env['APPDATA'],str(paths['config']));self.assertEqual(env['PATH'],str(node.parent))
            argv=h.npm_command(node,npm,paths,Path(t)/'root.tgz')
            self.assertEqual(argv[:3],[str(node),str(npm),'install']);self.assertIn('--ignore-scripts',argv);self.assertIn('--include=optional',argv)
    def fixture(self,base):
        paths=h.prepare(base/'isolated');root={'package.json':b'{"name":"@openai/codex"}','bin/codex.js':b'#!/usr/bin/env node\n'};native={'package.json':b'{}','vendor/codex.exe':b'NOT_EXECUTABLE'}
        records=lambda data:{name:{'size':len(b),'sha256':hashlib.sha256(b).hexdigest(),'executable':False}for name,b in data.items()}
        rr,nr=records(root),records(native);files,dirs=h.expected_layout(rr,nr)
        for directory in sorted(dirs,key=len):(paths['prefix']/directory).mkdir(parents=True,exist_ok=True)
        for name,b in root.items():(paths['prefix']/'node_modules/@openai/codex'/name).write_bytes(b)
        for name,b in native.items():(paths['prefix']/'node_modules/@openai/codex/node_modules/@openai/codex-win32-x64'/name).write_bytes(b)
        for name in h.SHIMS:(paths['prefix']/name).write_bytes((Path(__file__).parent/'fixtures/codex-windows-shims'/(name+'.txt')).read_bytes())
        return paths,rr,nr
    def test_complete_layout_and_exact_shims(self):
        with tempfile.TemporaryDirectory() as t:
            paths,rr,nr=self.fixture(Path(t));result=h.verify_install(paths,rr,nr);self.assertTrue(result['launcher_shims_verified'])
            for name in h.SHIMS:
                p=paths['prefix']/name;original=p.read_bytes();p.write_bytes(original+b'evil')
                with self.assertRaises(h.Failure):h.verify_install(paths,rr,nr)
                p.write_bytes(original)
    def test_unexpected_file_and_agent_state_fail(self):
        with tempfile.TemporaryDirectory() as t:
            paths,rr,nr=self.fixture(Path(t));p=paths['prefix']/'unexpected';p.write_bytes(b'x')
            with self.assertRaises(h.Failure):h.verify_install(paths,rr,nr)
            p.unlink();(paths['home']/'.codex').mkdir()
            with self.assertRaises(h.Failure):h.verify_install(paths,rr,nr)
    def test_installed_hardlink_rejected(self):
        with tempfile.TemporaryDirectory() as t:
            paths,rr,nr=self.fixture(Path(t));target=paths['prefix']/'codex';os.link(target,Path(t)/'alias')
            with self.assertRaises(h.Failure):h.verify_install(paths,rr,nr)
    def test_download_pin_override_rejected(self):
        with self.assertRaises(h.Failure):h.download(h.Pin('x','x',1,1),Path('unused'))
    @unittest.skipIf(os.name=='nt','non-Windows refusal')
    def test_runner_wrong_platform(self):
        with self.assertRaises(job.ProcessFailure):job.run([sys.executable],Path('.'),{})

@unittest.skipUnless(os.name=='nt','actual Windows Job Object tests')
class WindowsRunner(unittest.TestCase):
    def execute(self,code,path,seconds=5,limit=65536):
        env={'SystemRoot':h.windows_directory(),'WINDIR':h.windows_directory(),'TEMP':str(path),'TMP':str(path)}
        return job.run([str(Path(sys.executable).resolve()),'-I','-c',code],path,env,seconds,limit)
    def test_alternate_stream_rejected(self):
        with tempfile.TemporaryDirectory() as t:
            path=Path(t)/'plain';path.write_bytes(b'plain');Path(str(path)+':extra').write_bytes(b'canary')
            with self.assertRaisesRegex(h.Failure,'alternate_stream_rejected'):h.file_digest(path,100)
    def test_success_output(self):
        with tempfile.TemporaryDirectory() as t:self.assertEqual(self.execute("print('fixture')",Path(t))[0].strip(),b'fixture')
    def test_output_limit(self):
        with tempfile.TemporaryDirectory() as t:
            with self.assertRaisesRegex(job.ProcessFailure,'npm_output_limit'):self.execute("print('x'*100000)",Path(t),limit=1024)
    def test_timeout(self):
        with tempfile.TemporaryDirectory() as t:
            with self.assertRaisesRegex(job.ProcessFailure,'npm_process_timeout'):self.execute('import time;time.sleep(30)',Path(t),seconds=.2)
    def test_unrelated_inheritable_file_handle_is_not_leaked(self):
        import msvcrt
        with tempfile.TemporaryDirectory() as t:
            path=Path(t)/'unrelated-sensitive-handle.txt';path.write_text('canary')
            fd=os.open(path,os.O_RDONLY);os.set_inheritable(fd,True)
            try:
                handle=msvcrt.get_osfhandle(fd)
                code=("import ctypes;from ctypes import wintypes as w;"
                      "k=ctypes.WinDLL('kernel32');k.GetFinalPathNameByHandleW.argtypes=[w.HANDLE,w.LPWSTR,w.DWORD,w.DWORD];"
                      "b=ctypes.create_unicode_buffer(32768);"
                      f"k.GetFinalPathNameByHandleW(w.HANDLE({handle}),b,len(b),0);"
                      "print(b.value.endswith('unrelated-sensitive-handle.txt'))")
                self.assertEqual(self.execute(code,Path(t))[0].strip(),b'False')
            finally:os.close(fd)
    def test_early_leader_exit_kills_child(self):
        with tempfile.TemporaryDirectory() as t:
            p=Path(t);pidfile=p/'pid.txt'
            code="import subprocess,sys;from pathlib import Path;p=subprocess.Popen([sys.executable,'-I','-c','import time;time.sleep(60)']);Path('pid.txt').write_text(str(p.pid))"
            self.execute(code,p)
            import ctypes
            from ctypes import wintypes as w
            k=ctypes.WinDLL('kernel32',use_last_error=True);k.OpenProcess.argtypes=[w.DWORD,w.BOOL,w.DWORD];k.OpenProcess.restype=w.HANDLE
            k.WaitForSingleObject.argtypes=[w.HANDLE,w.DWORD];k.WaitForSingleObject.restype=w.DWORD;k.CloseHandle.argtypes=[w.HANDLE]
            handle=k.OpenProcess(0x100000,False,int(pidfile.read_text()))
            if handle:
                try:self.assertEqual(k.WaitForSingleObject(handle,5000),0)
                finally:k.CloseHandle(handle)

if __name__=='__main__':unittest.main()
