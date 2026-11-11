//! Linux fork regressions must run alone so gated children never retain another
//! parallel test's descriptors. No real agents, user files, network or credentials.
use std::{
    fs::File,
    io::Write,
    os::fd::{AsRawFd, FromRawFd},
};

pub(crate) fn isolated(test: &str) -> bool {
    const DRIVER: &str = "AGENTDECK_TEST_LOCK_FORK_DRIVER";
    if std::env::var(DRIVER).as_deref() == Ok(test) {
        return true;
    }
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env(DRIVER, test)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fork regression failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    false
}

// Only async-signal-safe libc calls are used after fork in the child. The pipe
// keeps inherited descriptors open until the parent releases/reaps this child.
pub(crate) struct BeforeExecChild {
    pid: libc::pid_t,
    release: File,
}
impl BeforeExecChild {
    pub(crate) fn fork() -> Self {
        let mut pipe = [-1; 2];
        assert_eq!(
            unsafe { libc::pipe2(pipe.as_mut_ptr(), libc::O_CLOEXEC) },
            0
        );
        let read = unsafe { File::from_raw_fd(pipe[0]) };
        let write = unsafe { File::from_raw_fd(pipe[1]) };
        let pid = unsafe { libc::fork() };
        if pid == 0 {
            unsafe {
                libc::close(write.as_raw_fd());
                let mut byte = 0u8;
                loop {
                    let rc = libc::read(read.as_raw_fd(), (&mut byte as *mut u8).cast(), 1);
                    if rc >= 0 {
                        libc::_exit(0);
                    }
                    if *libc::__errno_location() != libc::EINTR {
                        libc::_exit(1);
                    }
                }
            }
        }
        assert!(pid > 0, "fork failed: {}", std::io::Error::last_os_error());
        drop(read);
        Self {
            pid,
            release: write,
        }
    }
}
impl Drop for BeforeExecChild {
    fn drop(&mut self) {
        let _ = self.release.write_all(b"x");
        loop {
            let rc = unsafe { libc::waitpid(self.pid, std::ptr::null_mut(), 0) };
            if rc >= 0 || std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                break;
            }
        }
    }
}

pub(crate) fn assert_busy(path: &std::path::Path) {
    let contender = File::options().read(true).write(true).open(path).unwrap();
    assert!(matches!(
        contender.try_lock(),
        Err(std::fs::TryLockError::WouldBlock)
    ));
}
pub(crate) fn assert_released(path: &std::path::Path) {
    let contender = File::options().read(true).write(true).open(path).unwrap();
    contender
        .try_lock()
        .expect("completed operation must release lock while child remains alive");
    contender.unlock().unwrap();
}
