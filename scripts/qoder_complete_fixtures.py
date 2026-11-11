"""CI fixture setup using the built CLI. Never run the staged Qoder payload."""
import hashlib
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import threading
import fetch_qoder_ui_archive as pinned

PAYLOAD_BYTES = 182_543_840
PAYLOAD_SHA256 = '8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b'

def select_release(release):
    global PAYLOAD_BYTES, PAYLOAD_SHA256
    pinned.select_release(release)
    _, _, PAYLOAD_BYTES, PAYLOAD_SHA256 = pinned.releases.contract(release)


def release_args():
    return ['--release', pinned.VERSION]

class PreparationFailure(Exception):
    pass


def file_digest(path, max_size, exact=None):
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_uid != os.geteuid() or stat.S_IMODE(before.st_mode) != 0o600 or before.st_size > max_size:
        raise PreparationFailure('unsafe_fixture_file')
    if exact is not None and before.st_size != exact[0]:
        raise PreparationFailure('fixture_size_mismatch')
    def identity(m): return (m.st_dev,m.st_ino,m.st_uid,m.st_mode,m.st_nlink,m.st_size,m.st_mtime_ns,m.st_ctime_ns)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        if identity(opened) != identity(before): raise PreparationFailure('fixture_changed_before_read')
        digest = hashlib.sha256(); count = 0
        while block := stream.read(65536):
            count += len(block)
            if count > max_size: raise PreparationFailure('fixture_size_exceeded')
            digest.update(block)
        after = os.fstat(stream.fileno())
    if any(identity(m) != identity(before) for m in (opened,after,path.lstat())) or count != before.st_size:
        raise PreparationFailure('fixture_changed')
    result = digest.hexdigest()
    if exact is not None and result != exact[1]: raise PreparationFailure('fixture_hash_mismatch')
    return result


def run_cli(command, env):
    # stdout is JSON and stderr is deliberately not retained: never emit paths
    # or raw backend errors. Drain bounded stdout; timeout kills only this group.
    process = subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, start_new_session=True)
    output = bytearray(); read_failed=[]
    def stop_group():
        try: os.killpg(process.pid,signal.SIGKILL)
        except ProcessLookupError: pass
    def drain():
        try:
            while block := process.stdout.read(65536):
                output.extend(block[:max(0,65537-len(output))])
        except Exception: read_failed.append(True)
    reader = threading.Thread(target=drain,daemon=True);reader.start()
    try:
        try: code=process.wait(timeout=120)
        except subprocess.TimeoutExpired:
            stop_group();process.wait(timeout=5)
            raise PreparationFailure('fixture_cli_timeout') from None
        reader.join(timeout=5)
        if reader.is_alive() or read_failed or code != 0 or len(output)>65536:
            raise PreparationFailure('fixture_cli_failed')
        def unique(pairs):
            result={}
            for key,value in pairs:
                if key in result: raise PreparationFailure('fixture_cli_duplicate_key')
                result[key]=value
            return result
        try: return json.loads(output,object_pairs_hook=unique)
        except (ValueError,UnicodeError): raise PreparationFailure('fixture_cli_invalid_json') from None
    finally:
        if process.poll() is None:
            stop_group();process.wait(timeout=5)
        if reader.is_alive():
            stop_group();reader.join(timeout=5)
        if not reader.is_alive():process.stdout.close()


def validate_report(report, unpack):
    expected = ({'state':'verified_payload_unpacked_not_installed','version':pinned.VERSION,'installed':False,'executable':False,'payload_bytes':PAYLOAD_BYTES,'payload_sha256':PAYLOAD_SHA256} if unpack else
                {'state':'verified_archive_staged_not_installed','version':pinned.VERSION,'target':'linux-amd64-baseline','bytes':pinned.SIZE,'installed':False,'extracted':False})
    if not isinstance(report,dict) or set(report)!=set(expected)|{'limitations'} or not isinstance(report['limitations'],str):
        raise PreparationFailure('fixture_cli_report_invalid')
    for key,value in expected.items():
        if type(report[key]) is not type(value) or report[key]!=value:raise PreparationFailure('fixture_cli_report_invalid')


def prepare(cli, archive, root, env, *, runner=None):
    runner = runner or run_cli
    if archive.name != pinned.NAME:
        raise PreparationFailure('wrong_archive_name')
    pinned._guard_destination(archive.parent)
    parent=archive.parent.lstat()
    if not stat.S_ISDIR(parent.st_mode) or parent.st_uid!=os.geteuid() or stat.S_IMODE(parent.st_mode)!=0o700:
        raise PreparationFailure('unsafe_archive_parent')
    file_digest(archive,pinned.SIZE,(pinned.SIZE,pinned.SHA256))
    staged=root/'staged';unpacked=root/'unpacked'
    if staged.exists() or staged.is_symlink() or unpacked.exists() or unpacked.is_symlink():
        raise PreparationFailure('fixture_destination_exists')
    first=runner([str(cli),'qoder','stage-package','--target','linux-amd64-baseline','--archive',str(archive),'--destination',str(staged),'--yes'] + release_args(),env)
    validate_report(first,False)
    second=runner([str(cli),'qoder','unpack-cache','--cache',str(staged),'--destination',str(unpacked),'--yes'] + release_args(),env)
    validate_report(second,True)
    exact={staged/'archive.tar.gz':(pinned.SIZE,pinned.SHA256),unpacked/'archive.tar.gz':(pinned.SIZE,pinned.SHA256),unpacked/'qodercli':(PAYLOAD_BYTES,PAYLOAD_SHA256)}
    for directory, members in [(staged,{'incomplete.json','complete.json','archive.tar.gz'}),(unpacked,{'incomplete.json','complete.json','archive.tar.gz','qodercli'})]:
        m=directory.lstat()
        if not stat.S_ISDIR(m.st_mode) or m.st_uid!=os.geteuid() or stat.S_IMODE(m.st_mode)!=0o700 or {p.name for p in directory.iterdir()}!=members:
            raise PreparationFailure('fixture_directory_invalid')
        for member in directory.iterdir():
            contract=exact.get(member)
            file_digest(member,contract[0] if contract else 65536,contract)
    return {'staged':staged,'unpacked':unpacked},exact
