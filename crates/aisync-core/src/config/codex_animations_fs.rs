//! Linux-only persistence for the narrow animations protocol. No discovery or agent execution.
//! Directory handles pin operations, but unrelated writers do not participate in our lock.
//! Sync ordering is intentional; this is not a power-loss or hostile same-user CAS guarantee.
use super::codex_animations as protocol;
use std::path::Path;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unsupported file, platform, path or metadata")]
    Unsupported,
    #[error("Configuration changed or an ownership receipt already exists")]
    Conflict,
    #[error("Another configuration operation is active")]
    Busy,
    #[error("Persistence failed; preserve the receipt for recovery")]
    Io,
    #[error(transparent)]
    Protocol(#[from] protocol::ProtocolError),
}
#[derive(Debug, PartialEq, Eq)]
pub enum RecoveryState {
    NoReceipt,
    RestoreOriginal,
    CleanupReceipt,
}
type Result<T> = std::result::Result<T, Error>;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod native {
    use super::*;
    use std::{
        ffi::CString,
        fs::File,
        io::{Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::fs::MetadataExt,
        },
        path::Component,
    };
    const CONFIG: &str = "config.toml";
    const RECEIPT: &str = ".agentdeck-codex-animations.json";
    const LOCK: &str = ".agentdeck-codex-animations.lock";
    const LIMIT: u64 = 256 * 1024;
    struct Dir {
        file: File,
        binding: Vec<u8>,
        path: std::path::PathBuf,
        held_lock: std::cell::RefCell<Option<std::fs::Metadata>>,
    }
    struct Image {
        bytes: Vec<u8>,
        metadata: std::fs::Metadata,
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) enum Point {
        ReceiptCreated,
        ReceiptSynced,
        ReceiptDirectorySynced,
        TemporarySynced,
        Replaced,
        DirectorySynced,
        Removed,
        CleanupSynced,
    }
    type Hook<'a> = &'a dyn Fn(Point) -> Result<()>;
    fn c(name: &str) -> Result<CString> {
        CString::new(name).map_err(|_| Error::Unsupported)
    }
    fn fd_file(fd: i32) -> Result<File> {
        if fd < 0 {
            Err(Error::Io)
        } else {
            Ok(unsafe { File::from_raw_fd(fd) })
        }
    }
    fn identity(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
        a.dev() == b.dev() && a.ino() == b.ino()
    }
    fn metadata(file: &File) -> Result<std::fs::Metadata> {
        let m = file.metadata().map_err(|_| Error::Io)?;
        if !m.is_file()
            || m.nlink() != 1
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o7022 != 0
            || m.mode() & 0o200 == 0
        {
            return Err(Error::Unsupported);
        }
        let n = unsafe { libc::flistxattr(file.as_raw_fd(), std::ptr::null_mut(), 0) };
        if n != 0
            && !(n < 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::EOPNOTSUPP))
        {
            return Err(Error::Unsupported);
        }
        Ok(m)
    }
    impl Dir {
        fn open(path: &Path) -> Result<Self> {
            let text = path.to_str().ok_or(Error::Unsupported)?;
            if !path.is_absolute()
                || text.len() > 4096
                || text.chars().any(char::is_control)
                || path.file_name().and_then(|n| n.to_str()) != Some(CONFIG)
            {
                return Err(Error::Unsupported);
            }
            let mut dir = fd_file(unsafe {
                libc::open(
                    c"/".as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                )
            })?;
            let parent = path.parent().ok_or(Error::Unsupported)?;
            if parent.components().count() > 64 {
                return Err(Error::Unsupported);
            }
            for part in parent.components() {
                match part {
                    Component::RootDir => {}
                    Component::Normal(n) => {
                        let name = c(n.to_str().ok_or(Error::Unsupported)?)?;
                        dir = fd_file(unsafe {
                            libc::openat(
                                dir.as_raw_fd(),
                                name.as_ptr(),
                                libc::O_RDONLY
                                    | libc::O_DIRECTORY
                                    | libc::O_NOFOLLOW
                                    | libc::O_CLOEXEC,
                            )
                        })?;
                    }
                    _ => return Err(Error::Unsupported),
                }
            }
            let m = dir.metadata().map_err(|_| Error::Io)?;
            if m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o7022 != 0 {
                return Err(Error::Unsupported);
            }
            let attrs = unsafe { libc::flistxattr(dir.as_raw_fd(), std::ptr::null_mut(), 0) };
            if attrs != 0
                && !(attrs < 0
                    && std::io::Error::last_os_error().raw_os_error() == Some(libc::EOPNOTSUPP))
            {
                return Err(Error::Unsupported);
            }
            let binding =
                format!("codex-animations-fs/1:{}:{}:{}", text, m.dev(), m.ino()).into_bytes();
            Ok(Self {
                file: dir,
                binding,
                path: path.to_path_buf(),
                held_lock: std::cell::RefCell::new(None),
            })
        }
        fn check_parent(&self) -> Result<()> {
            let current = Self::open(&self.path)?;
            if current.binding != self.binding {
                return Err(Error::Conflict);
            }
            if let Some(held) = self.held_lock.borrow().as_ref() {
                let named = self.open_file(LOCK, libc::O_RDONLY)?;
                let m = metadata(&named)?;
                if !identity(held, &m) || m.len() != 0 {
                    return Err(Error::Conflict);
                }
            }
            Ok(())
        }
        fn open_file(&self, name: &str, flags: i32) -> Result<File> {
            fd_file(unsafe {
                libc::openat(
                    self.file.as_raw_fd(),
                    c(name)?.as_ptr(),
                    flags | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                    0o600,
                )
            })
        }
        fn exists(&self, name: &str) -> Result<bool> {
            let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
            let rc = unsafe {
                libc::fstatat(
                    self.file.as_raw_fd(),
                    c(name)?.as_ptr(),
                    st.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if rc == 0 {
                Ok(true)
            } else if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT) {
                Ok(false)
            } else {
                Err(Error::Io)
            }
        }
        fn read(&self, name: &str) -> Result<Image> {
            let f = self.open_file(name, libc::O_RDONLY)?;
            let m = metadata(&f)?;
            if m.len() > LIMIT {
                return Err(Error::Unsupported);
            }
            let mut bytes = Vec::new();
            (&f).take(LIMIT + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| Error::Io)?;
            let after = metadata(&f)?;
            let named = self.open_file(name, libc::O_RDONLY)?;
            if bytes.len() as u64 > LIMIT
                || !identity(&m, &metadata(&named)?)
                || m.len() != after.len()
                || m.mtime() != after.mtime()
                || m.mtime_nsec() != after.mtime_nsec()
                || m.ctime() != after.ctime()
                || m.ctime_nsec() != after.ctime_nsec()
            {
                return Err(Error::Conflict);
            }
            Ok(Image { bytes, metadata: m })
        }
        fn same(&self, name: &str, image: &Image) -> Result<()> {
            self.check_parent()?;
            let now = self.read(name)?;
            if !identity(&image.metadata, &now.metadata)
                || image.metadata.mode() != now.metadata.mode()
                || image.metadata.gid() != now.metadata.gid()
                || now.bytes != image.bytes
            {
                Err(Error::Conflict)
            } else {
                Ok(())
            }
        }
        fn sync(&self) -> Result<()> {
            self.check_parent()?;
            self.file.sync_all().map_err(|_| Error::Io)
        }
        fn lock(&self) -> Result<File> {
            if !self.exists(LOCK)? {
                match self.open_file(LOCK, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL) {
                    Ok(f) => {
                        metadata(&f)?;
                        f.sync_all().map_err(|_| Error::Io)?;
                        self.sync()?;
                    }
                    Err(_) => {
                        if !self.exists(LOCK)? {
                            return Err(Error::Io);
                        }
                    }
                }
            }
            let f = self.open_file(LOCK, libc::O_RDWR)?;
            let m = metadata(&f)?;
            if m.len() != 0 {
                return Err(Error::Unsupported);
            }
            f.try_lock().map_err(|_| Error::Busy)?;
            let named = self.open_file(LOCK, libc::O_RDONLY)?;
            if !identity(&m, &metadata(&named)?) {
                return Err(Error::Conflict);
            }
            *self.held_lock.borrow_mut() = Some(m);
            Ok(f)
        }
        fn create_receipt(&self, bytes: &[u8], hook: Hook<'_>) -> Result<Image> {
            self.check_parent()?;
            let mut f = self.open_file(RECEIPT, libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL)?;
            metadata(&f)?;
            self.check_parent()?;
            hook(Point::ReceiptCreated)?;
            f.write_all(bytes)
                .and_then(|_| f.sync_all())
                .map_err(|_| Error::Io)?;
            hook(Point::ReceiptSynced)?;
            self.sync()?;
            hook(Point::ReceiptDirectorySynced)?;
            let image = Image {
                bytes: bytes.to_vec(),
                metadata: metadata(&f)?,
            };
            self.same(RECEIPT, &image)?;
            Ok(image)
        }
        fn replace(&self, name: &str, old: &Image, bytes: &[u8], hook: Hook<'_>) -> Result<Image> {
            self.same(name, old)?;
            let temp = format!(".agentdeck-codex-{}.tmp", uuid::Uuid::new_v4());
            let mut f = self.open_file(&temp, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL)?;
            let result = (|| {
                let m = metadata(&f)?;
                if m.gid() != old.metadata.gid() {
                    return Err(Error::Unsupported);
                }
                if unsafe { libc::fchmod(f.as_raw_fd(), old.metadata.mode() & 0o777) } != 0 {
                    return Err(Error::Io);
                }
                f.write_all(bytes)
                    .and_then(|_| f.sync_all())
                    .map_err(|_| Error::Io)?;
                hook(Point::TemporarySynced)?;
                self.same(name, old)?;
                self.same(
                    &temp,
                    &Image {
                        bytes: bytes.to_vec(),
                        metadata: metadata(&f)?,
                    },
                )?;
                let named = self.open_file(&temp, libc::O_RDONLY)?;
                if !identity(&m, &metadata(&named)?) {
                    return Err(Error::Conflict);
                }
                if unsafe {
                    libc::renameat(
                        self.file.as_raw_fd(),
                        c(&temp)?.as_ptr(),
                        self.file.as_raw_fd(),
                        c(name)?.as_ptr(),
                    )
                } != 0
                {
                    return Err(Error::Io);
                }
                hook(Point::Replaced)?;
                self.sync()?;
                hook(Point::DirectorySynced)
            })();
            // Temporary contains config bytes: only remove our inode, never a replacement.
            if let Ok(named) = self.open_file(&temp, libc::O_RDONLY)
                && identity(
                    &f.metadata().map_err(|_| Error::Io)?,
                    &named.metadata().map_err(|_| Error::Io)?,
                )
            {
                let rc = unsafe { libc::unlinkat(self.file.as_raw_fd(), c(&temp)?.as_ptr(), 0) };
                if rc == 0 {
                    self.sync()?;
                }
            }
            result?;
            let image = Image {
                bytes: bytes.to_vec(),
                metadata: metadata(&f)?,
            };
            self.same(name, &image)?;
            Ok(image)
        }
        fn remove(&self, old: &Image, hook: Hook<'_>) -> Result<()> {
            self.same(RECEIPT, old)?;
            if unsafe { libc::unlinkat(self.file.as_raw_fd(), c(RECEIPT)?.as_ptr(), 0) } != 0 {
                return Err(Error::Io);
            }
            hook(Point::Removed)?;
            self.sync()?;
            hook(Point::CleanupSynced)?;
            self.check_parent()?;
            if self.exists(RECEIPT)? {
                return Err(Error::Conflict);
            }
            Ok(())
        }
    }
    fn fs_token(image: &Image, protocol_token: &str) -> String {
        use sha2::{Digest, Sha256};
        let m = &image.metadata;
        format!(
            "{:x}",
            Sha256::digest(format!(
                "codex-animations-fs-apply/1:{protocol_token}:{}:{}:{}:{}:{}:{}",
                m.dev(),
                m.ino(),
                m.mode(),
                m.gid(),
                m.ctime(),
                m.ctime_nsec()
            ))
        )
    }
    pub(super) fn preview(path: &Path, after: bool) -> Result<protocol::Preview> {
        let d = Dir::open(path)?;
        if d.exists(RECEIPT)? {
            return Err(Error::Conflict);
        }
        let image = d.read(CONFIG)?;
        let mut p = protocol::preview(&image.bytes, &d.binding, after)?;
        p.token = fs_token(&image, &p.token);
        d.check_parent()?;
        Ok(p)
    }
    pub(super) fn apply(path: &Path, after: bool, token: &str, hook: Hook<'_>) -> Result<bool> {
        let d = Dir::open(path)?;
        let _lock = d.lock()?;
        if d.exists(RECEIPT)? {
            return Err(Error::Conflict);
        }
        let old = d.read(CONFIG)?;
        let p = protocol::preview(&old.bytes, &d.binding, after)?;
        if token != fs_token(&old, &p.token) {
            return Err(Error::Conflict);
        }
        let Some(change) = protocol::prepare(&old.bytes, &d.binding, after, &p.token)? else {
            return Ok(false);
        };
        let receipt = d.create_receipt(
            &serde_json::to_vec(&change.receipt).map_err(|_| Error::Io)?,
            hook,
        )?;
        d.same(CONFIG, &old)?;
        d.same(RECEIPT, &receipt)?;
        let written = d.replace(CONFIG, &old, &change.output, hook)?;
        let applied = protocol::mark_applied(&written.bytes, &d.binding, &change.receipt)?;
        d.replace(
            RECEIPT,
            &receipt,
            &serde_json::to_vec(&applied).map_err(|_| Error::Io)?,
            hook,
        )?;
        d.same(CONFIG, &written)?;
        Ok(true)
    }
    pub(super) fn recovery_preview(path: &Path) -> Result<RecoveryState> {
        let d = Dir::open(path)?;
        let old = d.read(CONFIG)?;
        if !d.exists(RECEIPT)? {
            d.check_parent()?;
            return Ok(RecoveryState::NoReceipt);
        }
        let receipt = d.read(RECEIPT)?;
        let r = protocol::decode_receipt(&receipt.bytes)?;
        let state = match protocol::recovery(&old.bytes, &d.binding, &r)? {
            protocol::RecoveryAction::RestoreOriginal => RecoveryState::RestoreOriginal,
            protocol::RecoveryAction::CleanupReceipt => RecoveryState::CleanupReceipt,
        };
        d.same(CONFIG, &old)?;
        d.same(RECEIPT, &receipt)?;
        Ok(state)
    }
    pub(super) fn restore(path: &Path, hook: Hook<'_>) -> Result<bool> {
        let d = Dir::open(path)?;
        let _lock = d.lock()?;
        let old = d.read(CONFIG)?;
        let receipt = d.read(RECEIPT)?;
        let r = protocol::decode_receipt(&receipt.bytes)?;
        let Some(change) = protocol::prepare_restore(&old.bytes, &d.binding, &r)? else {
            d.same(CONFIG, &old)?;
            d.remove(&receipt, hook)?;
            d.same(CONFIG, &old)?;
            return Ok(false);
        };
        let restoring = d.replace(
            RECEIPT,
            &receipt,
            &serde_json::to_vec(&change.receipt).map_err(|_| Error::Io)?,
            hook,
        )?;
        d.same(RECEIPT, &restoring)?;
        let restored = d.replace(CONFIG, &old, &change.output, hook)?;
        if protocol::recovery(&restored.bytes, &d.binding, &change.receipt)?
            != protocol::RecoveryAction::CleanupReceipt
        {
            return Err(Error::Conflict);
        }
        d.same(CONFIG, &restored)?;
        d.remove(&restoring, hook)?;
        d.same(CONFIG, &restored)?;
        Ok(true)
    }
    #[cfg(test)]
    #[path = "tests.rs"]
    mod tests;
}

pub fn preview(path: &Path, after: bool) -> Result<protocol::Preview> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        native::preview(path, after)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (path, after);
        Err(Error::Unsupported)
    }
}
pub fn apply(path: &Path, after: bool, token: &str) -> Result<bool> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        native::apply(path, after, token, &|_| Ok(()))
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (path, after, token);
        Err(Error::Unsupported)
    }
}
pub fn restore(path: &Path) -> Result<bool> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        native::restore(path, &|_| Ok(()))
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = path;
        Err(Error::Unsupported)
    }
}

/// Read-only observation; creates no lock and authorizes no future write.
pub fn recovery_preview(path: &Path) -> Result<RecoveryState> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        native::recovery_preview(path)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = path;
        Err(Error::Unsupported)
    }
}
