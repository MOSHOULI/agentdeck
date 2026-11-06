//! Native cleanup for bounded read-only probes. No cleanup executable is looked
//! up on PATH, and Windows children cannot execute before joining their job.

#[cfg(unix)]
pub(super) fn kill_group(pid: u32) {
    // SAFETY: POSIX kill accepts a negative process ID to address its process
    // group. Probe children create their own group; no pointers are involved.
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    if let Ok(pid) = i32::try_from(pid) {
        unsafe {
            kill(-pid, 9);
        }
    }
}

#[cfg(windows)]
mod windows {
    use std::io;
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_NO_MORE_FILES, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessIdOfThread, OpenThread, ResumeThread, THREAD_QUERY_LIMITED_INFORMATION,
        THREAD_SUSPEND_RESUME,
    };

    struct OwnedHandle(HANDLE);
    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // SAFETY: this wrapper uniquely owns a valid non-null Windows handle.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    pub(crate) struct ProbeJob(OwnedHandle);

    impl ProbeJob {
        pub(crate) fn new() -> io::Result<Self> {
            // SAFETY: null security/name parameters request a private unnamed job.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Self(OwnedHandle(handle));
            // SAFETY: this Windows C struct is valid when zero-initialized, with
            // LimitFlags filled before the API consumes its sized immutable data.
            let mut information: JOBOBJECT_EXTENDED_LIMIT_INFORMATION =
                unsafe { std::mem::zeroed() };
            information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let success = unsafe {
                SetInformationJobObject(
                    job.0.0,
                    JobObjectExtendedLimitInformation,
                    (&information as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of_val(&information) as u32,
                )
            };
            if success == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }

        /// Caller MUST have used CREATE_SUSPENDED. Failure leaves the process
        /// unresumed; the caller terminates/reaps it before returning an error.
        pub(crate) fn assign_and_resume(&self, child: &Child) -> io::Result<()> {
            // SAFETY: borrowed child handle is live; the private job stays owned
            // throughout the probe. No breakaway permission is granted.
            if unsafe { AssignProcessToJobObject(self.0.0, child.as_raw_handle().cast()) } == 0 {
                return Err(io::Error::last_os_error());
            }
            // std::process::Child exposes the process handle but not its initial
            // thread handle. Enumerate its sole, still-suspended initial thread.
            let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
            if snapshot == INVALID_HANDLE_VALUE {
                return Err(io::Error::last_os_error());
            }
            let snapshot = OwnedHandle(snapshot);
            let mut entry: THREADENTRY32 = unsafe { std::mem::zeroed() };
            entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
            let mut thread_id = None;
            let mut present = unsafe { Thread32First(snapshot.0, &mut entry) };
            while present != 0 {
                if entry.th32OwnerProcessID == child.id()
                    && thread_id.replace(entry.th32ThreadID).is_some()
                {
                    return Err(io::Error::other(
                        "Probe had multiple threads before being resumed",
                    ));
                }
                entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
                present = unsafe { Thread32Next(snapshot.0, &mut entry) };
            }
            if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                return Err(io::Error::last_os_error());
            }
            let id = thread_id
                .ok_or_else(|| io::Error::other("Suspended probe thread was not found"))?;
            let thread = unsafe {
                OpenThread(
                    THREAD_SUSPEND_RESUME | THREAD_QUERY_LIMITED_INFORMATION,
                    0,
                    id,
                )
            };
            if thread.is_null() {
                return Err(io::Error::last_os_error());
            }
            let thread = OwnedHandle(thread);
            if unsafe { GetProcessIdOfThread(thread.0) } != child.id() {
                return Err(io::Error::other("Probe thread owner changed during setup"));
            }
            if unsafe { ResumeThread(thread.0) } != 1 {
                return Err(io::Error::other(
                    "Probe thread could not be resumed exactly once",
                ));
            }
            Ok(())
        }
    }
}

#[cfg(windows)]
pub(super) use windows::ProbeJob;
