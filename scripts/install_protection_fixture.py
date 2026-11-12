"""Strict inspection/preparation of private authored installation protection.

This is acceptance support, not product recovery. Synthetic armed/corrupt states
are prepared only while the affected test application is stopped; no crash proof.
Never print record paths, attempt IDs or prefix identity values as public evidence.
"""
import json
import os
from pathlib import Path
import stat
import uuid

class Failure(Exception): pass

def _ordinary(path, directory=False):
    m=path.lstat()
    if stat.S_ISLNK(m.st_mode) or getattr(m,'st_file_attributes',0)&0x400:
        raise Failure('unsafe_protection_entry')
    if directory:
        if not stat.S_ISDIR(m.st_mode):raise Failure('unsafe_protection_entry')
    elif not stat.S_ISREG(m.st_mode) or m.st_nlink!=1 or m.st_size>16384:
        raise Failure('unsafe_protection_entry')
    if os.name!='nt' and (m.st_uid!=os.geteuid() or stat.S_IMODE(m.st_mode)!=(0o700 if directory else 0o600)):
        raise Failure('unsafe_protection_permissions')
    return m

def _directory(home):return Path(home)/'.agentdeck'/'install-protection'/'codex'

def protection_snapshot(home):
    """Private bounded snapshot, including raw bytes even for corrupt state."""
    home=Path(home)
    for ancestor in (home,home/'.agentdeck'):
        m=ancestor.lstat()
        if not stat.S_ISDIR(m.st_mode) or stat.S_ISLNK(m.st_mode) or getattr(m,'st_file_attributes',0)&0x400:
            raise Failure('unsafe_protection_entry')
    directory=_directory(home);parent=directory.parent
    _ordinary(parent,True);_ordinary(directory,True)
    if {p.name for p in parent.iterdir()}!={'codex'} or {p.name for p in directory.iterdir()}!={'gate.lock','state.json'}:
        raise Failure('unexpected_protection_inventory')
    result=[]
    for path in (parent,directory,directory/'gate.lock',directory/'state.json'):
        m=_ordinary(path,path in (parent,directory))
        identity=tuple(getattr(m,k) for k in ('st_dev','st_ino','st_mode','st_nlink','st_size','st_mtime_ns','st_ctime_ns'))
        result.append((path.name,identity,None if path in (parent,directory) else path.read_bytes()))
    if result[2][2]!=b'':raise Failure('protection_lock_not_empty')
    return result

def validate_install_protection(home,expected_phase=None,required=False):
    parent=_directory(home).parent
    if not os.path.lexists(parent):
        if required:raise Failure('protection_missing')
        return None
    snapshot=protection_snapshot(home)
    def unique(pairs):
        result={}
        for key,value in pairs:
            if key in result:raise Failure('invalid_protection_record')
            result[key]=value
        return result
    try:record=json.loads(snapshot[3][2],object_pairs_hook=unique)
    except (ValueError,UnicodeError,RecursionError):raise Failure('invalid_protection_record') from None
    if not isinstance(record,dict) or set(record)!={'schema','platform','phase','attempt','prefix','prefixIdentity'}:
        raise Failure('invalid_protection_record')
    if type(record['schema']) is not int or record['schema']!=1 or record['platform']!=('windows' if os.name=='nt' else 'linux'):
        raise Failure('invalid_protection_record')
    phase=record['phase']
    if phase not in ('clear','armed','failed_returned') or (expected_phase is not None and phase!=expected_phase):
        raise Failure('unexpected_protection_phase')
    if phase=='clear':
        if any(record[key] is not None for key in ('attempt','prefix','prefixIdentity')):raise Failure('invalid_protection_record')
    else:
        try:
            if not isinstance(record['attempt'],str) or str(uuid.UUID(record['attempt']))!=record['attempt']:raise ValueError()
        except (ValueError,TypeError,AttributeError):raise Failure('invalid_protection_record') from None
        prefix=record['prefix'];identity=record['prefixIdentity']
        if not isinstance(prefix,str) or not prefix or len(prefix)>32768 or not Path(prefix).is_absolute() or any(ord(c)<32 for c in prefix):
            raise Failure('invalid_protection_record')
        if not isinstance(identity,str) or not 0<len(identity)<=1024:raise Failure('invalid_protection_record')
    return record

def prepare_synthetic_protection(source_home,destination_home,case):
    """Copy only the exact valid guard into a fresh authored HOME; no caches.

    The recorded target remains the original owned fixture. Caller keeps it and
    its PATH unchanged and stops all affected apps before preparing the case.
    """
    record=validate_install_protection(source_home,'failed_returned',True)
    if case not in ('armed','corrupt'):raise Failure('invalid_synthetic_case')
    home=Path(destination_home)
    if not home.is_absolute() or not home.is_dir() or any(home.iterdir()):raise Failure('synthetic_home_not_empty')
    directory=_directory(home)
    for path in (home/'.agentdeck',directory.parent,directory):path.mkdir(mode=0o700)
    record=dict(record);record['phase']='armed'
    payload=json.dumps(record,separators=(',',':')).encode() if case=='armed' else b'{"schema":'
    for name,data in (('gate.lock',b''),('state.json',payload)):
        fd=os.open(directory/name,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
        with os.fdopen(fd,'wb') as out:out.write(data)
    return protection_snapshot(home)
