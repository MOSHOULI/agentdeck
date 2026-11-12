//! Fixed local files only; no permission repair on existing objects.
use super::{Result, err};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    };
    pub(crate) fn create_dir(path: &Path) -> Result<()> {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(path)
            .map_err(|_| err())
    }
    pub(crate) fn open_dir(path: &Path) -> Result<File> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| err())?;
        if !file.metadata().map_err(|_| err())?.is_dir() {
            return Err(err());
        }
        Ok(file)
    }
    pub(crate) fn open_file(path: &Path, write: bool) -> Result<File> {
        OpenOptions::new()
            .read(true)
            .write(write)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| err())
    }
    pub(crate) fn create_file(path: &Path) -> Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| err())
    }
    pub(crate) fn identity(file: &File) -> Result<String> {
        let m = file.metadata().map_err(|_| err())?;
        Ok(format!(
            "{}:{}:{}:{}:{}",
            m.dev(),
            m.ino(),
            m.uid(),
            m.gid(),
            m.mode()
        ))
    }
    pub(crate) fn private(file: &File, directory: bool) -> Result<()> {
        let m = file.metadata().map_err(|_| err())?;
        if m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o7777 != if directory { 0o700 } else { 0o600 }
            || if directory {
                !m.is_dir()
            } else {
                !m.is_file() || m.nlink() != 1
            }
        {
            return Err(err());
        }
        Ok(())
    }
    pub(crate) fn sync_dir(file: &File) -> Result<()> {
        file.sync_all().map_err(|_| err())
    }
}
#[cfg(windows)]
mod platform {
    use super::*;
    use std::{
        ffi::OsStr,
        mem,
        os::windows::{
            ffi::OsStrExt,
            fs::OpenOptionsExt,
            io::{AsRawHandle, FromRawHandle},
        },
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::*,
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    pub(crate) fn local_path(path: &Path) -> Result<()> {
        let text = path.to_str().ok_or_else(err)?;
        if text.as_bytes().get(1..3) != Some(b":\\") || !text.as_bytes()[0].is_ascii_alphabetic() {
            return Err(err());
        }
        let root = format!("{}\\", &text[..2]);
        if unsafe { GetDriveTypeW(wide(OsStr::new(&root)).as_ptr()) }
            != windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED
        {
            return Err(err());
        }
        Ok(())
    }
    fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect()
    }
    struct Descriptor(PSECURITY_DESCRIPTOR);
    impl Drop for Descriptor {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    fn descriptor() -> Result<Descriptor> {
        unsafe {
            let mut token = ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(err());
            }
            let mut needed = 0;
            GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut needed);
            if needed == 0 || needed > 65536 {
                CloseHandle(token);
                return Err(err());
            }
            let mut buffer = vec![0u64; (needed as usize).div_ceil(8)];
            let ok = GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            );
            CloseHandle(token);
            if ok == 0 {
                return Err(err());
            }
            let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
            let mut sid = ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
                return Err(err());
            }
            let mut len = 0;
            while *sid.add(len) != 0 {
                len += 1;
                if len > 256 {
                    LocalFree(sid.cast());
                    return Err(err());
                }
            }
            let sid_text =
                String::from_utf16(std::slice::from_raw_parts(sid, len)).map_err(|_| err());
            LocalFree(sid.cast());
            let sid_text = sid_text?;
            // Protected current-user and SYSTEM-only DACL. No inherited grants.
            let sddl = format!("O:{sid_text}D:P(A;;FA;;;SY)(A;;FA;;;{sid_text})");
            let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
            let mut sd = ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut sd,
                ptr::null_mut(),
            ) == 0
            {
                return Err(err());
            }
            Ok(Descriptor(sd))
        }
    }
    fn attributes(sd: &Descriptor) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: sd.0,
            bInheritHandle: 0,
        }
    }
    pub(crate) fn create_dir(path: &Path) -> Result<()> {
        let sd = descriptor()?;
        let sa = attributes(&sd);
        if unsafe { CreateDirectoryW(wide(path.as_os_str()).as_ptr(), &sa) } == 0 {
            Err(err())
        } else {
            Ok(())
        }
    }
    pub(crate) fn open_dir(path: &Path) -> Result<File> {
        let file = OpenOptions::new()
            .access_mode(FILE_READ_DATA | FILE_READ_ATTRIBUTES | READ_CONTROL)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .map_err(|_| err())?;
        let info = info(&file)?;
        if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0 {
            return Err(err());
        }
        Ok(file)
    }
    pub(crate) fn open_file(path: &Path, write: bool) -> Result<File> {
        let file = OpenOptions::new()
            .access_mode(GENERIC_READ | READ_CONTROL | if write { GENERIC_WRITE } else { 0 })
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| err())?;
        let info = info(&file)?;
        if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 || info.nNumberOfLinks != 1 {
            return Err(err());
        }
        Ok(file)
    }
    pub(crate) fn create_file(path: &Path) -> Result<File> {
        let sd = descriptor()?;
        let sa = attributes(&sd);
        let handle = unsafe {
            CreateFileW(
                wide(path.as_os_str()).as_ptr(),
                GENERIC_READ | GENERIC_WRITE | READ_CONTROL,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                &sa,
                CREATE_NEW,
                FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
                ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(err());
        }
        let file = unsafe { File::from_raw_handle(handle) };
        private(&file, false)?;
        Ok(file)
    }
    fn info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { mem::zeroed() };
        if unsafe { GetFileType(file.as_raw_handle()) } != FILE_TYPE_DISK
            || unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
            || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(err());
        }
        Ok(info)
    }
    pub(crate) fn identity(file: &File) -> Result<String> {
        let i = info(file)?;
        Ok(format!(
            "{}:{}:{}:{}",
            i.dwVolumeSerialNumber, i.nFileIndexHigh, i.nFileIndexLow, i.dwFileAttributes
        ))
    }
    unsafe fn security_parts(sd: PSECURITY_DESCRIPTOR) -> Result<(Vec<u8>, Vec<u8>, u16)> {
        unsafe {
            let mut owner = ptr::null_mut();
            let mut defaulted = 0;
            let mut acl = ptr::null_mut();
            let mut present = 0;
            let mut control = 0;
            let mut revision = 0;
            if GetSecurityDescriptorOwner(sd, &mut owner, &mut defaulted) == 0
                || owner.is_null()
                || GetSecurityDescriptorDacl(sd, &mut present, &mut acl, &mut defaulted) == 0
                || present == 0
                || acl.is_null()
                || IsValidAcl(acl) == 0
                || GetSecurityDescriptorControl(sd, &mut control, &mut revision) == 0
            {
                return Err(err());
            }
            Ok((
                std::slice::from_raw_parts(owner.cast::<u8>(), GetLengthSid(owner) as usize)
                    .to_vec(),
                std::slice::from_raw_parts(acl.cast::<u8>(), (*acl).AclSize as usize).to_vec(),
                control,
            ))
        }
    }
    pub(crate) fn private(file: &File, directory: bool) -> Result<()> {
        let i = info(file)?;
        if (i.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
            || (!directory && i.nNumberOfLinks != 1)
        {
            return Err(err());
        }
        let desired = descriptor()?;
        unsafe {
            let flags = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
            let mut needed = 0;
            GetKernelObjectSecurity(file.as_raw_handle(), flags, ptr::null_mut(), 0, &mut needed);
            if needed == 0 || needed > 65536 {
                return Err(err());
            }
            let mut bytes = vec![0u32; (needed as usize).div_ceil(4)];
            if GetKernelObjectSecurity(
                file.as_raw_handle(),
                flags,
                bytes.as_mut_ptr().cast(),
                needed,
                &mut needed,
            ) == 0
            {
                return Err(err());
            }
            let (owner, acl, control) = security_parts(bytes.as_mut_ptr().cast())?;
            let (want_owner, want_acl, _) = security_parts(desired.0)?;
            if owner != want_owner || acl != want_acl || control & SE_DACL_PROTECTED == 0 {
                return Err(err());
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn broaden_test_dacl(path: &Path, protected: bool) -> Result<()> {
        let file = OpenOptions::new()
            .access_mode(READ_CONTROL | WRITE_DAC)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| err())?;
        unsafe {
            let sddl: Vec<u16> = "D:P(A;;FA;;;WD)".encode_utf16().chain(Some(0)).collect();
            let mut pointer = ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut pointer,
                ptr::null_mut(),
            ) == 0
            {
                return Err(err());
            }
            let sd = Descriptor(pointer);
            let mut present = 0;
            let mut defaulted = 0;
            let mut acl = ptr::null_mut();
            if GetSecurityDescriptorDacl(sd.0, &mut present, &mut acl, &mut defaulted) == 0
                || acl.is_null()
            {
                return Err(err());
            }
            if SetSecurityInfo(
                file.as_raw_handle(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION
                    | if protected {
                        PROTECTED_DACL_SECURITY_INFORMATION
                    } else {
                        UNPROTECTED_DACL_SECURITY_INFORMATION
                    },
                ptr::null_mut(),
                ptr::null_mut(),
                acl,
                ptr::null_mut(),
            ) != 0
            {
                return Err(err());
            }
        }
        Ok(())
    }
    // Windows does not provide a portable directory FlushFileBuffers contract.
    // File bytes are flushed before rename; no power-loss durability claim.
    pub(crate) fn sync_dir(_file: &File) -> Result<()> {
        Ok(())
    }
}
#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    use super::*;
    pub(crate) fn create_dir(_: &Path) -> Result<()> {
        Err(err())
    }
    pub(crate) fn open_dir(_: &Path) -> Result<File> {
        Err(err())
    }
    pub(crate) fn open_file(_: &Path, _: bool) -> Result<File> {
        Err(err())
    }
    pub(crate) fn create_file(_: &Path) -> Result<File> {
        Err(err())
    }
    pub(crate) fn identity(_: &File) -> Result<String> {
        Err(err())
    }
    pub(crate) fn private(_: &File, _: bool) -> Result<()> {
        Err(err())
    }
    pub(crate) fn sync_dir(_: &File) -> Result<()> {
        Err(err())
    }
}
pub(super) use platform::*;
