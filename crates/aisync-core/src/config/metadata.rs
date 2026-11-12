//! Bounded Linux extended-metadata refusal and Windows DACL eligibility.
//! Neither is an atomic CAS; Windows staging preservation lives in windows_security.
use std::{fs::File, path::Path};

#[cfg(target_os = "linux")]
fn checked_size(size: isize) -> crate::Result<()> {
    if size == 0 {
        return Ok(());
    }
    let reason = if size < 0 {
        format!("inspection failed: {}", std::io::Error::last_os_error())
    } else {
        "attributes are present".into()
    };
    Err(crate::Error::InjectFailed(format!(
        "Configuration extended metadata cannot be preserved ({reason}); AgentDeck refused this file. Manage it with its owning tool; no attributes were automatically removed."
    )))
}

pub(super) fn check_path(path: &Path) -> crate::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| crate::Error::InjectFailed("Invalid configuration path".into()))?;
        // SAFETY: live NUL-terminated path; null buffer with zero size queries
        // the name-list length without reading content or following a leaf link.
        checked_size(unsafe { libc::llistxattr(path.as_ptr(), std::ptr::null_mut(), 0) })
    }
    #[cfg(windows)]
    {
        super::windows_security::Security::read_path(path).map(|_| ())
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = path;
        Ok(())
    }
}

pub(super) fn check_file(file: &File) -> crate::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: borrowed live file descriptor; zero-sized query, no buffer.
        checked_size(unsafe { libc::flistxattr(file.as_raw_fd(), std::ptr::null_mut(), 0) })
    }
    #[cfg(windows)]
    {
        super::windows_security::Security::read(file).map(|_| ())
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = file;
        Ok(())
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn nonzero_and_failed_queries_are_not_treated_as_empty() {
        assert!(super::checked_size(0).is_ok());
        assert!(super::checked_size(1).is_err());
        // All syscall errors take this branch, including EOPNOTSUPP; there is
        // deliberately no unsupported-filesystem exemption.
        assert!(super::checked_size(-1).is_err());
    }
}
