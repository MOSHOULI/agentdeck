#!/usr/bin/env python3
"""Authored npm/Codex fixture for real installed Windows App UI/IPC acceptance.

Not official npm, Codex, registry, network or lifecycle acceptance. The only real
executable is an explicitly supplied trusted Node PE, copied into the fixture.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import struct
import sys

from installer_node_fixture import measured_version
from install_protection_fixture import validate_install_protection, protection_snapshot, prepare_synthetic_protection

CANARY = 'AGENTDECK_SYNTHETIC_INSTALL_PRIVATE_CANARY'
MARKER = b'synthetic-install-once\n'
PARTIAL = ('authored incomplete installation; retain for inspection\n' + CANARY + '\n').encode()
class Failure(Exception): pass

def safe_file(path, limit):
    m=path.lstat()
    if not stat.S_ISREG(m.st_mode) or m.st_nlink!=1 or m.st_size>limit or getattr(m,'st_file_attributes',0)&0x400: raise Failure('unsafe_fixture_file')
    return path.read_bytes()

def namespaced(path):
    value=str(path.resolve(strict=True))
    return '\\\\?\\'+value if os.name=='nt' and not value.startswith('\\\\?\\') else value

def write_new(path,data):
    with path.open('xb') as out:out.write(data)

def shim(target):
    # Exact cmd-shim form recognized by the production Windows adapter.
    target=target.replace('/','\\')
    return ('@ECHO off\nGOTO start\n:find_dp0\nSET dp0=%~dp0\nEXIT /b\n:start\nSETLOCAL\nCALL :find_dp0\n\n'
            'IF EXIST "%dp0%\\node.exe" (\n  SET "_prog=%dp0%\\node.exe"\n) ELSE (\n  SET "_prog=node"\n'
            '  SET PATHEXT=%PATHEXT:;.JS;=;%\n)\n\n'
            'endLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%"  "%dp0%\\'+target+'" %*\n').replace('\n','\r\n').encode()

def layout(root,home):
    root=Path(root)
    paths={'root':root,'managerBin':root/'manager','manager':root/'manager/node_modules/npm/bin/npm-cli.js',
           'node':root/'manager/node.exe','prefix':root/'prefix','globalRoot':root/'prefix/node_modules',
           'package':root/'prefix/node_modules/@openai/codex','marker':root/'install-marker',
           'installCalls':root/'install-events','latest':root/'latest-version','queries':root/'query-events','versions':root/'version-events','canary':root/'canary.txt','home':Path(home)}
    return {k:str(v) for k,v in paths.items()}

def codex_program(p, count_all=False):
    program = ("#!/usr/bin/env node\n'use strict';\nconst fs=require('node:fs');const path=require('node:path');\n"
            'const log='+json.dumps(p['versions'])+';\n'
            "try { if(JSON.stringify(process.argv.slice(2))!=='[\"--version\"]')throw Error();"
            "if(path.toNamespacedPath(fs.realpathSync(log))!==log)throw Error();const s=fs.lstatSync(log);if(!s.isFile()||s.isSymbolicLink()||s.nlink!==1||s.size>=4096)throw Error();"
            "fs.appendFileSync(log,'v');console.log('codex-cli 1.2.3');"
            "}catch(_){process.stderr.write('synthetic version rejected\\n');process.exitCode=64;}\n")
    if count_all:
        # The wrong-root sentinel records any authored launcher invocation, even
        # unsupported argv. The ordinary success fixture contract is unchanged.
        check="if(JSON.stringify(process.argv.slice(2))!=='[\"--version\"]')throw Error();"
        program=program.replace(check,'').replace("fs.appendFileSync(log,'v');", "fs.appendFileSync(log,'v');"+check)
    return program

def manager_program(p,mode):
    expected=['install','--global','--prefix',p['prefix'],'--force=false','--no-audit','--no-fund','--no-progress','--ignore-scripts','@openai/codex@1.2.3']
    header=("#!/usr/bin/env node\n'use strict';\nconst fs=require('node:fs');const path=require('node:path');\n"
            'const p='+json.dumps(p)+';\nconst expected='+json.dumps(expected)+';\nconst mode='+json.dumps(mode)+';\n'
            'const partial='+json.dumps(PARTIAL.decode())+';\n'
            'const codex='+json.dumps(codex_program(p,mode=='wrong-root'))+';\nconst launcher='+json.dumps(shim('node_modules/@openai/codex/bin/codex.js').decode())+';\n')
    return header+r'''
function canonical(target){return path.toNamespacedPath(fs.realpathSync(target));}
function directory(target){
 const relative=path.relative(p.root,target);
 if(relative==='..'||relative.startsWith('..'+path.sep)||path.isAbsolute(relative))throw Error('escape');
 let current=p.root;
 for(const part of [''].concat(relative.split(path.sep).filter(Boolean))){
  current=path.join(current,part);const s=fs.lstatSync(current);
  if(!s.isDirectory()||s.isSymbolicLink()||canonical(current)!==path.toNamespacedPath(current))throw Error('directory');
 }
}
function record(kind,target=p.queries){const s=fs.lstatSync(target);if(!s.isFile()||s.isSymbolicLink()||s.nlink!==1||s.size>=4096||canonical(target)!==target)throw Error('log');fs.appendFileSync(target,kind);}
function absent(target){try{fs.lstatSync(target);}catch(e){if(e.code==='ENOENT')return;throw e;}throw Error('exists');}
try{
 for(const d of [p.prefix,p.globalRoot,path.dirname(p.manager)])directory(d);
 if(canonical(process.argv[1])!==p.manager)throw Error('entry');
 const args=JSON.stringify(process.argv.slice(2));
 if(args==='["prefix","--global"]'){record('p');console.log(p.prefix);}
 else if(args==='["root","--global"]'){record('r');console.log(p.globalRoot);}
 else if(args==='["view","@openai/codex@latest","version","--json","--fetch-retries=0","--fetch-timeout=25000"]'){record('m');const s=fs.lstatSync(p.latest);if(!s.isFile()||s.isSymbolicLink()||s.nlink!==1||s.size!==5)throw Error('metadata');const version=fs.readFileSync(p.latest,'utf8');if(!['1.2.3','1.2.4'].includes(version))throw Error('metadata');console.log(JSON.stringify(version));}
 else if(args===JSON.stringify(expected)){
  record('i',p.installCalls);
  absent(p.marker);absent(p.package);
  for(const suffix of ['','.cmd','.ps1','.exe','.com','.bat'])absent(path.join(p.prefix,'codex'+suffix));
  const scope=path.dirname(p.package);if(fs.existsSync(scope))directory(scope);
  fs.writeFileSync(p.marker,'synthetic-install-once\n',{flag:'wx'});
  fs.mkdirSync(scope,{recursive:true});fs.mkdirSync(p.package);
  if(mode==='fail'){
   fs.writeFileSync(path.join(p.package,'partial-install.txt'),partial,{flag:'wx'});
   throw Error('authored failure after partial installation');
  }
  fs.mkdirSync(path.join(p.package,'bin'));
  fs.writeFileSync(path.join(p.package,'package.json'),JSON.stringify({name:'@openai/codex',version:mode==='wrong-root'?'1.2.4':'1.2.3',bin:{codex:'bin/codex.js'}}),{flag:'wx'});
  fs.writeFileSync(path.join(p.package,'bin/codex.js'),codex,{flag:'wx'});
  fs.writeFileSync(path.join(p.prefix,'codex.cmd'),launcher,{flag:'wx'});
 }else throw Error('arguments');
}catch(_){process.stderr.write('synthetic fixture rejected operation\n');process.exitCode=64;}
'''

def prepare(root,node,home,mode):
    if os.name!='nt':raise Failure('requires_windows')
    if mode not in ('success','fail','wrong-root') or not root.is_absolute() or not node.is_absolute() or not home.is_absolute():raise Failure('invalid_fixture_arguments')
    node=node.resolve(strict=True)
    data=safe_file(node,256*1024*1024)
    if len(data)<64 or data[:2]!=b'MZ':raise Failure('trusted_node_not_pe')
    offset=struct.unpack_from('<I',data,0x3c)[0]
    if offset>len(data)-6 or data[offset:offset+4]!=b'PE\0\0' or struct.unpack_from('<H',data,offset+4)[0]!=0x8664:raise Failure('trusted_node_not_x64')
    node_version=measured_version(node)
    root.mkdir(exist_ok=False)
    p=layout(namespaced(root),namespaced(home))
    for directory in (Path(p['manager']).parent,Path(p['globalRoot'])):directory.mkdir(parents=True)
    write_new(Path(p['node']),data)
    write_new(Path(p['manager']).parent.parent/'package.json',json.dumps({'name':'npm','version':'0.0.0','bin':{'npm':'bin/npm-cli.js'}}).encode())
    for key in ('queries','versions','installCalls'):write_new(Path(p[key]),b'')
    write_new(Path(p['canary']),CANARY.encode())
    write_new(Path(p['latest']),b'1.2.3')
    write_new(Path(p['manager']),manager_program(p,mode).encode())
    write_new(Path(p['managerBin'])/'npm.cmd',shim('node_modules/npm/bin/npm-cli.js'))
    manifest={'schemaVersion':1,**p,'mode':mode,'path':p['managerBin']+';'+p['prefix'],
              'packageVersion':'1.2.3','installerNodeVersion':node_version,'installerNodeMinimum':'18.0.0',
              'display':{'managerPath':p['manager'],'nodePath':p['node'],'prefix':p['prefix'],'globalRoot':p['globalRoot'],'packageRoot':p['package'],'launcherDirectory':p['prefix']},
              'nodeSha256':hashlib.sha256(data).hexdigest(),'managerSha256':hashlib.sha256(Path(p['manager']).read_bytes()).hexdigest()}
    destination=root/'fixture.json';write_new(destination,json.dumps(manifest,indent=2).encode());return destination

def load(manifest):
    m=json.loads(safe_file(manifest,65536))
    if m.get('schemaVersion')!=1 or m.get('mode') not in ('success','fail','wrong-root'):raise Failure('invalid_manifest')
    expected=layout(namespaced(manifest.parent),m['home'])
    if any(m.get(k)!=v for k,v in expected.items()) or m.get('path')!=expected['managerBin']+';'+expected['prefix']:raise Failure('invalid_manifest_paths')
    for value in expected.values():
        path=Path(value)
        for parent in (path,*path.parents):
            if parent.exists():
                meta=parent.lstat()
                if stat.S_ISLNK(meta.st_mode) or getattr(meta,'st_file_attributes',0)&0x400:raise Failure('unsafe_fixture_path')
    return m

def no_configuration(m):
    home=Path(m['home'])
    for name in ('.codex','.aisync'):
        if os.path.lexists(home/name):raise Failure('configuration_created')
    product=home/'.agentdeck'
    if product.exists():
        for entry in product.iterdir():
            if entry.name=='acceptance-user-data.txt':
                # Existing installer-lifecycle sentinel, not a new agent profile.
                if safe_file(entry,256).decode('utf-8-sig').rstrip('\r\n')!='Synthetic data, must survive installation, upgrade and uninstall.':raise Failure('sentinel_changed')
            elif entry.name not in ('registry','registry-update.lock','install-protection'):raise Failure('configuration_created')
        validate_install_protection(home)

def facts(m):
    q=safe_file(Path(m['queries']),4096);v=safe_file(Path(m['versions']),4096);calls=safe_file(Path(m['installCalls']),4096)
    if any(c not in b'prm' for c in q) or any(c!=ord('v') for c in v) or any(c!=ord('i') for c in calls):raise Failure('invalid_event_log')
    if safe_file(Path(m['canary']),256)!=CANARY.encode():raise Failure('canary_changed')
    if hashlib.sha256(safe_file(Path(m['node']),256*1024*1024)).hexdigest()!=m['nodeSha256']:raise Failure('node_changed')
    return {'installCalls':len(calls),'partialInstallRetained':False,'prefixQueries':q.count(b'p'),'rootQueries':q.count(b'r'),'metadataQueries':q.count(b'm'),'versionProbes':len(v),'syntheticFixture':True,'officialAgentExecuted':False,'noConfigurationCreated':True}

def assert_phase(m,phase,expected_attempts=1):
    result=facts(m);prefix=Path(m['prefix']);package=Path(m['package']);marker=Path(m['marker'])
    if phase=='absent':
        if any(os.path.lexists(p) for p in (marker,package,*[prefix/('codex'+s) for s in ('','.cmd','.ps1','.exe','.com','.bat')])):raise Failure('unconfirmed_install_write')
        if result['versionProbes']:raise Failure('unexpected_version_probe')
        if result['installCalls']:raise Failure('unexpected_install_calls')
    elif phase in ('installed','failed','wrong-root'):
        if result['installCalls']!=expected_attempts:raise Failure('unexpected_install_calls')
        if safe_file(marker,128)!=MARKER:raise Failure('install_marker_mismatch')
        if phase=='failed':
            if m['mode']!='fail' or not package.is_dir() or any(os.path.lexists(prefix/('codex'+suffix)) for suffix in ('','.cmd','.ps1','.exe','.com','.bat')):raise Failure('failure_state_mismatch')
            if {p.name for p in package.iterdir()}!={'partial-install.txt'} or safe_file(package/'partial-install.txt',256)!=PARTIAL:raise Failure('failure_state_mismatch')
            if result['versionProbes']:raise Failure('unexpected_version_probe')
            result['partialInstallRetained']=True
        else:
            if phase=='wrong-root' and m['mode']!='wrong-root':raise Failure('failure_state_mismatch')
            expected={'package.json':json.dumps({'name':'@openai/codex','version':'1.2.4' if phase=='wrong-root' else '1.2.3','bin':{'codex':'bin/codex.js'}},separators=(',',':')).encode(),
                      'bin/codex.js':codex_program(m,phase=='wrong-root').encode()}
            if {p.relative_to(package).as_posix() for p in package.rglob('*') if p.is_file()}!=set(expected):raise Failure('installed_inventory_mismatch')
            for name,data in expected.items():
                if safe_file(package/name,65536)!=data:raise Failure('installed_bytes_changed')
            if safe_file(prefix/'codex.cmd',4096)!=shim('node_modules/@openai/codex/bin/codex.js'):raise Failure('shim_changed')
            if phase=='wrong-root':
                if result['versionProbes']!=0:raise Failure('unexpected_version_probe')
                result['completeWrongRootRetained']=True
            elif result['versionProbes']<1:raise Failure('version_not_probed')
    elif phase=='empty-owned':
        if result['installCalls']!=expected_attempts or result['versionProbes']!=0:raise Failure('unexpected_install_calls')
        if any(os.path.lexists(p) for p in (marker,package,*[prefix/('codex'+s) for s in ('','.cmd','.ps1','.exe','.com','.bat')])):raise Failure('unconfirmed_install_write')
    elif phase!='no-config':raise Failure('unknown_assertion')
    no_configuration(m);return result

def relocate_owned_residue(m):
    # Test-authored preparation, never product cleanup. Preserve every prefix
    # ancestor identity used by recovery; archive exact fixture leaves only.
    assert_phase(m,'wrong-root')
    root=Path(m['root']);package=Path(m['package']);launcher=Path(m['prefix'])/'codex.cmd';marker=Path(m['marker'])
    for target in (root/'retained-package',root/'retained-launcher.cmd',root/'retained-marker'):
        if os.path.lexists(target):raise Failure('retained_residue_exists')
    package.rename(root/'retained-package');launcher.rename(root/'retained-launcher.cmd');marker.rename(root/'retained-marker')
    return {'testAuthoredRelocation':True,'productCleanup':False,'prefixParentsRetained':True}

def authored_conflict(m,remove=False):
    path=Path(m['prefix'])/'codex.cmd';data=b'authored recovery conflict, not executable'
    if remove:
        if safe_file(path,len(data))!=data:raise Failure('conflict_changed')
        path.unlink()
    else:write_new(path,data)
    return {'testAuthoredConflict':True,'removed':remove}

FAILURE_CODES = frozenset({
    'unsafe_fixture_file', 'requires_windows', 'invalid_fixture_arguments',
    'trusted_node_not_pe', 'trusted_node_not_x64', 'invalid_manifest',
    'invalid_manifest_paths', 'unsafe_fixture_path', 'configuration_created',
    'sentinel_changed', 'invalid_event_log', 'canary_changed', 'node_changed',
    'unconfirmed_install_write', 'unexpected_version_probe', 'install_marker_mismatch',
    'failure_state_mismatch', 'installed_inventory_mismatch', 'installed_bytes_changed',
    'shim_changed', 'version_not_probed', 'unknown_assertion', 'missing_prepare_arguments',
    'missing_manifest', 'manager_already_changed', 'unexpected_install_calls', 'retained_residue_exists','conflict_changed',
})

def failure_diagnostic(error):
    if isinstance(error, Failure):
        return {'failureCode': str(error) if str(error) in FAILURE_CODES else 'fixture_failure', 'exceptionType': 'Failure'}
    # Never include exception messages, filesystem paths, or dynamically named types.
    types = {FileNotFoundError: 'FileNotFoundError', FileExistsError: 'FileExistsError',
             PermissionError: 'PermissionError', OSError: 'OSError', ValueError: 'ValueError',
             TypeError: 'TypeError', KeyError: 'KeyError', UnicodeEncodeError: 'UnicodeEncodeError',
             UnicodeDecodeError: 'UnicodeDecodeError', json.JSONDecodeError: 'JSONDecodeError'}
    return {'failureCode': 'unexpected_fixture_error', 'exceptionType': types.get(type(error), 'Exception')}


def main():
    # Pipe encoding is a protocol, not the Windows console code page. PowerShell
    # callers must decode UTF-8 too. This also overrides PYTHONIOENCODING=cp1252.
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, 'reconfigure'): stream.reconfigure(encoding='utf-8', errors='strict')
    parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--prepare',type=Path);group.add_argument('--assert',dest='phase',choices=('absent','installed','failed','wrong-root','empty-owned','no-config'));group.add_argument('--stale',action='store_true');group.add_argument('--restore-latest',action='store_true');group.add_argument('--relocate-owned',action='store_true');group.add_argument('--conflict',choices=('create','remove'));group.add_argument('--protection',choices=('clear','armed','failed_returned','snapshot'));group.add_argument('--synthetic-protection',choices=('armed','corrupt'))
    parser.add_argument('--destination-home',type=Path);parser.add_argument('--expected-attempts',type=int,choices=(1,2),default=1);parser.add_argument('--node',type=Path);parser.add_argument('--home',type=Path);parser.add_argument('--mode',choices=('success','fail','wrong-root'),default='success');parser.add_argument('--manifest',type=Path)
    a=parser.parse_args()
    try:
        if a.prepare:
            if a.node is None or a.home is None:raise Failure('missing_prepare_arguments')
            print(prepare(a.prepare,a.node,a.home,a.mode));return 0
        if a.manifest is None:raise Failure('missing_manifest')
        m=load(a.manifest)
        if a.relocate_owned:print(json.dumps(relocate_owned_residue(m)));return 0
        if a.conflict:print(json.dumps(authored_conflict(m,a.conflict=='remove')));return 0
        if a.protection:
            if a.protection=='snapshot':
                # Only fixed digest returned, never prefix, attempt or rawstate.
                value=protection_snapshot(a.home or m['home']);print(json.dumps({'digest':hashlib.sha256(repr(value).encode()).hexdigest()}));return 0
            validate_install_protection(a.home or m['home'],a.protection,True);print(json.dumps({'phase':a.protection,'verified':True}));return 0
        if a.synthetic_protection:
            if a.destination_home is None:raise Failure('missing_prepare_arguments')
            prepare_synthetic_protection(m['home'],a.destination_home,a.synthetic_protection);print(json.dumps({'syntheticState':a.synthetic_protection,'actualCrashVerified':False}));return 0
        if a.stale or a.restore_latest:
            path=Path(m['manager']);data=safe_file(path,65536)
            if hashlib.sha256(data).hexdigest()!=m['managerSha256']:raise Failure('manager_already_changed')
            latest=Path(m['latest']);current=safe_file(latest,5)
            if current!=(b'1.2.3' if a.stale else b'1.2.4'):raise Failure('invalid_manifest')
            latest.write_bytes(b'1.2.4' if a.stale else b'1.2.3')
            print(json.dumps({'latestChangedForStalePreview':a.stale,'managerUnchanged':True}));return 0
        print(json.dumps(assert_phase(m,a.phase,a.expected_attempts)));return 0
    except Exception as error:
        print(json.dumps(failure_diagnostic(error)), file=sys.stderr);return 1
if __name__=='__main__':raise SystemExit(main())
