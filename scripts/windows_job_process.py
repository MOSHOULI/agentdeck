"""Bounded Windows process-tree runner; direct executable, never a command shell.

The process is created suspended, assigned to a kill-on-close Job, then resumed.
This is lifetime/output containment, not a network or malicious-code sandbox.
"""
import ctypes
from ctypes import wintypes as w
import os
import subprocess
import threading
import time

class ProcessFailure(Exception):
    pass


def run(argv, cwd, env, seconds=300, output_limit=65536):
    if os.name != 'nt':
        raise ProcessFailure('requires_windows')
    if not argv or not os.path.isabs(argv[0]) or not 0 < seconds <= 300 or not 0 < output_limit <= 65536:
        raise ProcessFailure('process_arguments_rejected')
    import msvcrt
    k = ctypes.WinDLL('kernel32', use_last_error=True)
    size = ctypes.c_size_t
    class Basic(ctypes.Structure):
        _fields_ = [('process_time', ctypes.c_longlong), ('job_time', ctypes.c_longlong), ('flags', w.DWORD),
                    ('min_working', size), ('max_working', size), ('active', w.DWORD), ('affinity', size),
                    ('priority', w.DWORD), ('scheduling', w.DWORD)]
    class IO(ctypes.Structure):
        _fields_ = [(x, ctypes.c_ulonglong) for x in ('reads', 'writes', 'other', 'read_bytes', 'write_bytes', 'other_bytes')]
    class Extended(ctypes.Structure):
        _fields_ = [('basic', Basic), ('io', IO), ('process_memory', size), ('job_memory', size), ('peak_process', size), ('peak_job', size)]
    class Startup(ctypes.Structure):
        _fields_ = [('cb', w.DWORD), ('reserved', w.LPWSTR), ('desktop', w.LPWSTR), ('title', w.LPWSTR),
                    ('x', w.DWORD), ('y', w.DWORD), ('xsize', w.DWORD), ('ysize', w.DWORD),
                    ('xchars', w.DWORD), ('ychars', w.DWORD), ('fill', w.DWORD), ('flags', w.DWORD),
                    ('show', w.WORD), ('reserved_size', w.WORD), ('reserved_bytes', ctypes.POINTER(w.BYTE)),
                    ('stdin', w.HANDLE), ('stdout', w.HANDLE), ('stderr', w.HANDLE)]
    class StartupEx(ctypes.Structure):
        _fields_ = [('startup', Startup), ('attributes', ctypes.c_void_p)]
    class Info(ctypes.Structure):
        _fields_ = [('process', w.HANDLE), ('thread', w.HANDLE), ('pid', w.DWORD), ('tid', w.DWORD)]
    signatures = {
        'InitializeProcThreadAttributeList': ([ctypes.c_void_p, w.DWORD, w.DWORD, ctypes.POINTER(size)], w.BOOL),
        'UpdateProcThreadAttribute': ([ctypes.c_void_p, w.DWORD, size, ctypes.c_void_p, size, ctypes.c_void_p, ctypes.c_void_p], w.BOOL),
        'DeleteProcThreadAttributeList': ([ctypes.c_void_p], None),
        'CreateJobObjectW': ([ctypes.c_void_p, w.LPCWSTR], w.HANDLE),
        'SetInformationJobObject': ([w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD], w.BOOL),
        'CreateProcessW': ([w.LPCWSTR, w.LPWSTR, ctypes.c_void_p, ctypes.c_void_p, w.BOOL, w.DWORD,
                            ctypes.c_void_p, w.LPCWSTR, ctypes.POINTER(Startup), ctypes.POINTER(Info)], w.BOOL),
        'AssignProcessToJobObject': ([w.HANDLE, w.HANDLE], w.BOOL),
        'ResumeThread': ([w.HANDLE], w.DWORD),
        'WaitForSingleObject': ([w.HANDLE, w.DWORD], w.DWORD),
        'GetExitCodeProcess': ([w.HANDLE, ctypes.POINTER(w.DWORD)], w.BOOL),
        'TerminateJobObject': ([w.HANDLE, w.UINT], w.BOOL),
        'TerminateProcess': ([w.HANDLE, w.UINT], w.BOOL),
        'CloseHandle': ([w.HANDLE], w.BOOL),
    }
    for name, (args, result) in signatures.items():
        fn = getattr(k, name); fn.argtypes = args; fn.restype = result
    job = k.CreateJobObjectW(None, None)
    if not job: raise ProcessFailure('job_creation_failed')
    descriptors = []; threads = []; info = Info(); assigned = False; attributes_ready = False; attributes = None
    output = [bytearray(), bytearray()]; overflow = threading.Event(); read_error = threading.Event()
    try:
        limits = Extended(); limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not k.SetInformationJobObject(job, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            raise ProcessFailure('job_configuration_failed')
        pipes = []
        for _ in range(2):
            pair = os.pipe(); pipes.append(pair); descriptors.extend(pair)
        null = os.open(os.devnull, os.O_RDONLY); descriptors.append(null)
        extended = StartupEx(); startup = extended.startup; startup.cb = ctypes.sizeof(extended); startup.flags = 0x100  # STARTF_USESTDHANDLES
        for fd in (null, pipes[0][1], pipes[1][1]): os.set_inheritable(fd, True)
        startup.stdin = msvcrt.get_osfhandle(null)
        startup.stdout = msvcrt.get_osfhandle(pipes[0][1]); startup.stderr = msvcrt.get_osfhandle(pipes[1][1])
        required = size()
        k.InitializeProcThreadAttributeList(None, 1, 0, ctypes.byref(required))
        if not 0 < required.value <= 65536: raise ProcessFailure('handle_list_initialization_failed')
        attributes = ctypes.create_string_buffer(required.value)
        if not k.InitializeProcThreadAttributeList(attributes, 1, 0, ctypes.byref(required)):
            raise ProcessFailure('handle_list_initialization_failed')
        attributes_ready = True
        inherited = (w.HANDLE * 3)(startup.stdin, startup.stdout, startup.stderr)
        if not k.UpdateProcThreadAttribute(attributes, 0, 0x00020002, inherited, ctypes.sizeof(inherited), None, None):
            raise ProcessFailure('handle_list_configuration_failed')
        extended.attributes = ctypes.cast(attributes, ctypes.c_void_p)
        environment = ctypes.create_unicode_buffer('\0'.join(f'{key}={value}' for key, value in sorted(env.items(), key=lambda item:item[0].upper())) + '\0\0')
        command = ctypes.create_unicode_buffer(subprocess.list2cmdline(argv))
        # CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT
        if not k.CreateProcessW(argv[0], command, None, None, True, 0x08080404,
                                environment, str(cwd), ctypes.cast(ctypes.byref(extended), ctypes.POINTER(Startup)), ctypes.byref(info)):
            raise ProcessFailure('process_creation_failed')
        for fd in (null, pipes[0][1], pipes[1][1]): os.close(fd); descriptors.remove(fd)
        if not k.AssignProcessToJobObject(job, info.process): raise ProcessFailure('job_assignment_failed')
        assigned = True
        def read(index, fd):
            try:
                while True:
                    block = os.read(fd, 8192)
                    if not block: break
                    if len(output[index]) + len(block) > output_limit:
                        overflow.set(); break
                    output[index].extend(block)
            except OSError:
                read_error.set()
        for index, (fd, _) in enumerate(pipes):
            thread = threading.Thread(target=read, args=(index, fd), daemon=True); thread.start(); threads.append(thread)
        if k.ResumeThread(info.thread) == 0xffffffff: raise ProcessFailure('process_resume_failed')
        deadline = time.monotonic() + seconds
        while True:
            if overflow.is_set(): raise ProcessFailure('npm_output_limit')
            if read_error.is_set(): raise ProcessFailure('npm_output_read_failed')
            if time.monotonic() >= deadline: raise ProcessFailure('npm_process_timeout')
            status = k.WaitForSingleObject(info.process, 50)
            if status == 0: break
            if status != 258: raise ProcessFailure('process_wait_failed')
        # A successful leader must not leave background children holding output open.
        if not k.TerminateJobObject(job, 1): raise ProcessFailure('job_cleanup_failed')
        for thread in threads: thread.join(timeout=5)
        if any(thread.is_alive() for thread in threads): raise ProcessFailure('output_cleanup_failed')
        if overflow.is_set(): raise ProcessFailure('npm_output_limit')
        if read_error.is_set(): raise ProcessFailure('npm_output_read_failed')
        result = w.DWORD()
        if not k.GetExitCodeProcess(info.process, ctypes.byref(result)) or result.value != 0:
            raise ProcessFailure('npm_process_failed')
        return bytes(output[0]), bytes(output[1])
    finally:
        if info.process:
            # Even assignment failure cannot leave a suspended orphan.
            if assigned: k.TerminateJobObject(job, 1)
            else: k.TerminateProcess(info.process, 1)
            k.WaitForSingleObject(info.process, 5000)
        k.CloseHandle(job)
        if attributes_ready: k.DeleteProcThreadAttributeList(attributes)
        for thread in threads: thread.join(timeout=5)
        for fd in descriptors:
            try: os.close(fd)
            except OSError: pass
        for handle in (info.thread, info.process):
            if handle: k.CloseHandle(handle)
