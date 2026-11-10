#!/usr/bin/env python3
"""Offline parser/layout tests; Windows runner fixtures execute only authored Python."""
import base64
from contextlib import contextmanager
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

@contextmanager
def synthetic_metadata_diagnostics():
    """Only wrapped synthetic-fixture tests: report metadata, never paths/contents.

    Keep the actual comparator's return value and failure unchanged. This records
    the exact inputs at rejection, not a later stat that may describe another state.
    """
    original_digest = h.file_digest
    original_compare = h.metadata_families_match
    suffix = None
    def digest(path, *args, **kwargs):
        nonlocal suffix
        previous = suffix; suffix = Path(path).suffix
        try: return original_digest(path, *args, **kwargs)
        finally: suffix = previous
    def compare(before, opened, windows):
        result = original_compare(before, opened, windows)
        if not result:
            keys = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_nlink',
                    'st_mtime_ns', 'st_ctime_ns', 'st_birthtime_ns', 'st_file_attributes')
            print('SYNTHETIC_METADATA_DIAGNOSTIC ' + json.dumps({
                'python': sys.version, 'suffix': suffix, 'windows': windows,
                'before': {k: getattr(before, k, None) for k in keys},
                'opened': {k: getattr(opened, k, None) for k in keys},
                'different_fields': [k for k in keys if getattr(before, k, None) != getattr(opened, k, None)],
            }, sort_keys=True), flush=True)
        return result
    with patch.object(h, 'file_digest', side_effect=digest), patch.object(h, 'metadata_families_match', side_effect=compare):
        yield


class Offline(unittest.TestCase):
    def test_production_static_parser_command_and_closed_output_contract(self):
        paths = {'prefix': Path('/synthetic/prefix'), 'home': Path('/synthetic/home')}
        good = json.dumps(h.STATIC_OBSERVATION).encode()
        with patch.object(h, 'bounded_process', return_value=(good, b'')) as process:
            self.assertEqual(h.static_observation(Path('/trusted/aisync.exe'), paths, Path('/trusted/node.exe'), {}), h.STATIC_OBSERVATION)
            process.assert_called_once_with([str(Path('/trusted/aisync.exe')), 'codex-static', '--prefix', str(paths['prefix']), '--node', str(Path('/trusted/node.exe'))], paths['home'], {}, seconds=30)
        malformed = [b'not-json', b'{}', good[:-1] + b',"extra":true}',
                     good.replace(b'"schemaVersion": 1', b'"schemaVersion": true'),
                     good.replace(b'"applySupported": false', b'"applySupported": 0'),
                     good.replace(b'runtime_not_run', b'installed')]
        for output in malformed:
            with self.subTest(output=output), patch.object(h, 'bounded_process', return_value=(output, b'')):
                with self.assertRaises(h.Failure): h.static_observation(Path('parser'), paths, Path('node'), {})
        with patch.object(h, 'bounded_process', return_value=(good, b'private-output')):
            with self.assertRaisesRegex(h.Failure, '^production_static_output_rejected$'):
                h.static_observation(Path('parser'), paths, Path('node'), {})
        with patch.object(h, 'bounded_process', side_effect=h.Failure('npm_process_failed')):
            with self.assertRaisesRegex(h.Failure, '^production_static_process_failed$'):
                h.static_observation(Path('parser'), paths, Path('node'), {})

    def test_static_parser_requires_native_absolute_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            parser = Path(tmp) / 'aisync.exe'; parser.write_bytes(b'MZauthored-never-executed')
            self.assertEqual(h.parser_tool(parser), parser.resolve())
            parser.write_bytes(b'not-native')
            with self.assertRaises(h.Failure): h.parser_tool(parser)
            with self.assertRaises(h.Failure): h.parser_tool(Path('relative.exe'))

    def test_static_bridge_is_between_complete_layout_gates(self):
        import inspect
        source = inspect.getsource(h.run)
        first = source.index('verified = verify_install(')
        call = source.index('observation = static_observation(')
        last = source.index('if verify_install(')
        self.assertLess(first, call); self.assertLess(call, last)
        self.assertIn("prefix='AgentDeck 官方 安装 '", source)

    def test_path_handle_mode_normalization_is_narrow(self):
        from types import SimpleNamespace
        values=dict(st_dev=1,st_ino=2,st_size=10,st_nlink=1,st_mtime_ns=30,st_ctime_ns=40,st_birthtime_ns=40,st_mode=0o100666)
        before=SimpleNamespace(**values)
        executable=SimpleNamespace(**dict(values,st_mode=0o100777))
        self.assertTrue(h.metadata_families_match(before,executable,True))
        self.assertFalse(h.metadata_families_match(before,executable,False))
        for key in values.keys() - {'st_ctime_ns'}:
            changed=SimpleNamespace(**dict(values,**{key:values[key]+(0o200 if key=='st_mode' else 1)}))
            self.assertFalse(h.metadata_families_match(before,changed,True),key)
    def test_windows_birthtime_handles_observed_ctime_semantic_difference(self):
        from types import SimpleNamespace
        common=dict(st_dev=1,st_ino=2,st_mode=0o100666,st_size=11,st_nlink=1,
                    st_birthtime_ns=1794317359870845700,st_mtime_ns=1794317359886755700)
        before=SimpleNamespace(**common,st_ctime_ns=common['st_birthtime_ns'])
        opened=SimpleNamespace(**common,st_ctime_ns=common['st_mtime_ns'])
        self.assertTrue(h.metadata_families_match(before,opened,True))
        self.assertFalse(h.metadata_families_match(before,opened,False))
        changed=SimpleNamespace(**dict(common,st_birthtime_ns=common['st_birthtime_ns']+1),st_ctime_ns=opened.st_ctime_ns)
        self.assertFalse(h.metadata_families_match(before,changed,True))
    def test_native_snapshot_change_during_read_fails(self):
        with tempfile.TemporaryDirectory() as t:
            path=Path(t)/'fixture';path.write_bytes(b'constant bytes')
            # First pinned handle and its named reopen agree; the same handle's
            # post-read ChangeTime differs. Reject despite identical content.
            with patch.object(h,'handle_fingerprint',side_effect=[(1,100),(1,100),(1,101)]):
                with self.assertRaisesRegex(h.Failure,'^file_changed$'):h.file_digest(path,100)
    def test_each_metadata_family_still_rejects_ctime_changes(self):
        from types import SimpleNamespace
        for family in ('handle','path'):
            with tempfile.TemporaryDirectory() as t:
                path=Path(t)/'fixture';path.write_bytes(b'constant bytes')
                original=h.os.fstat if family=='handle' else Path.lstat
                calls=0
                def changing(value):
                    nonlocal calls
                    result=original(value);calls+=1
                    if calls==2:
                        fields={key:getattr(result,key) for key in dir(result) if key.startswith('st_')}
                        fields['st_ctime_ns']+=1
                        return SimpleNamespace(**fields)
                    return result
                target=patch.object(h.os,'fstat',side_effect=changing) if family=='handle' else patch.object(Path,'lstat',autospec=True,side_effect=changing)
                with patch.object(h,'handle_fingerprint',return_value=(1,100)),target:
                    with self.assertRaisesRegex(h.Failure,'^file_changed$'):h.file_digest(path,100)
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
    @synthetic_metadata_diagnostics()
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
    @synthetic_metadata_diagnostics()
    def test_native_plain_cmd_exe_metadata_consistency(self):
        with tempfile.TemporaryDirectory() as t:
            for name in ('plain','codex.cmd','codex.exe'):
                path=Path(t)/name;path.write_bytes(b'NOT_EXECUTED')
                self.assertEqual(h.file_digest(path,100),hashlib.sha256(b'NOT_EXECUTED').digest())
    def test_native_change_time_detects_attribute_update(self):
        import ctypes
        from ctypes import wintypes as w
        with tempfile.TemporaryDirectory() as t:
            path=Path(t)/'fixture';path.write_bytes(b'constant bytes')
            fd=h.open_read_no_follow(path)
            k=ctypes.WinDLL('kernel32',use_last_error=True)
            k.GetFileAttributesW.argtypes=[w.LPCWSTR];k.GetFileAttributesW.restype=w.DWORD
            k.SetFileAttributesW.argtypes=[w.LPCWSTR,w.DWORD];k.SetFileAttributesW.restype=w.BOOL
            attributes=k.GetFileAttributesW(str(path));self.assertNotEqual(attributes,0xffffffff)
            try:
                before=h.handle_fingerprint(fd);time.sleep(.02)
                self.assertTrue(k.SetFileAttributesW(str(path),attributes ^ 2))
                after=h.handle_fingerprint(fd)
                self.assertNotEqual(before[-1],after[-1],'native ChangeTime must detect metadata-only change')
            finally:
                k.SetFileAttributesW(str(path),attributes);os.close(fd)
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
