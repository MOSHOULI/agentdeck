//! Explicit archive-only staging. Not installation, extraction, or execution.
use serde::Serialize;
use std::path::Path;

pub const TARGET: &str = "linux-amd64-baseline";
pub const LIMITATIONS: &str = "Archive cache only, not installed or runtime-ready. Archive members have not been validated or extracted. No execution, PATH, login or configuration changes. Fixed upstream hash is not independent signature verification. Failed staging may leave a private incomplete directory; inspect it before manual removal, never retry over an existing directory. Same-user hostile races and mount I/O deadlines are not solved.";
#[derive(Debug, Serialize)]
pub struct StagedPackage {
    pub state: &'static str,
    pub version: &'static str,
    pub target: &'static str,
    pub bytes: u64,
    pub installed: bool,
    pub extracted: bool,
    pub limitations: &'static str,
}
#[derive(Debug, thiserror::Error)]
pub enum StagingError {
    #[error("Only explicit linux-amd64-baseline staging on a Linux x86_64 host is reviewed")]
    Unsupported,
    #[error("Destination must be an absolute new directory under existing, non-linked parents")]
    InvalidDestination,
    #[error("Destination already exists or cannot be safely created; nothing is overwritten")]
    DestinationConflict,
    #[error(
        "Staging not confirmed complete; an incomplete private destination may remain. Inspect it before manual cleanup; do not overwrite or retry in place"
    )]
    Incomplete,
}

pub fn stage_package(
    target: &str,
    archive: &Path,
    destination: &Path,
) -> Result<StagedPackage, StagingError> {
    stage_package_for_release(
        crate::qoder::distribution::VERSION,
        target,
        archive,
        destination,
    )
}
pub fn stage_package_for_release(
    release: &str,
    target: &str,
    archive: &Path,
    destination: &Path,
) -> Result<StagedPackage, StagingError> {
    let reviewed = crate::qoder::distribution::reviewed_release(release)
        .map_err(|_| StagingError::Unsupported)?;

    if target != TARGET {
        return Err(StagingError::Unsupported);
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        linux::stage(reviewed, archive, destination)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (reviewed, archive, destination);
        Err(StagingError::Unsupported)
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) mod linux {
    use super::*;
    use crate::inspection::reader::{NativeSelectedFileReader, validate_path};
    use crate::qoder::distribution;
    use sha2::{Digest, Sha256};
    use std::{
        ffi::{CString, OsStr},
        fs::File,
        io::{Read, Seek, SeekFrom, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::MetadataExt},
        },
        path::Component,
    };

    pub(crate) fn name(value: &OsStr) -> Result<CString, StagingError> {
        CString::new(value.as_bytes()).map_err(|_| StagingError::InvalidDestination)
    }
    pub(crate) fn open_at(
        parent: &File,
        child: &OsStr,
        flags: i32,
        mode: libc::mode_t,
    ) -> Result<File, StagingError> {
        let child = name(child)?;
        // SAFETY: parent is a live descriptor; child is NUL-terminated; returned fd is owned once.
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                child.as_ptr(),
                flags | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                mode,
            )
        };
        if fd < 0 {
            return Err(StagingError::Incomplete);
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }
    pub(crate) fn directory(path: &Path) -> Result<File, StagingError> {
        let mut current = File::open("/").map_err(|_| StagingError::InvalidDestination)?;
        for part in path.components() {
            match part {
                Component::RootDir => {}
                Component::Normal(child) => {
                    current = open_at(&current, child, libc::O_RDONLY | libc::O_DIRECTORY, 0)
                        .map_err(|_| StagingError::InvalidDestination)?
                }
                _ => return Err(StagingError::InvalidDestination),
            }
        }
        Ok(current)
    }
    pub(crate) fn new_file(dir: &File, child: &str) -> Result<File, StagingError> {
        open_at(
            dir,
            OsStr::new(child),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL,
            0o600,
        )
    }
    pub(crate) fn identity(file: &File) -> Result<(u64, u64), StagingError> {
        let m = file.metadata().map_err(|_| StagingError::Incomplete)?;
        Ok((m.dev(), m.ino()))
    }
    pub(crate) fn record(
        dir: &File,
        child: &str,
        value: &serde_json::Value,
    ) -> Result<(), StagingError> {
        let mut file = new_file(dir, child)?;
        file.write_all(
            serde_json::to_string_pretty(value)
                .map_err(|_| StagingError::Incomplete)?
                .as_bytes(),
        )
        .map_err(|_| StagingError::Incomplete)?;
        file.sync_all().map_err(|_| StagingError::Incomplete)
    }
    pub(super) fn stage(
        reviewed: distribution::ReviewedRelease,
        archive: &Path,
        destination: &Path,
    ) -> Result<StagedPackage, StagingError> {
        let leaf = destination
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(StagingError::InvalidDestination)?;
        validate_path(destination, leaf).map_err(|_| StagingError::InvalidDestination)?;
        let plan = distribution::plan_for_release(reviewed.version, TARGET)
            .map_err(|_| StagingError::Unsupported)?;
        validate_path(archive, plan.archive_name).map_err(|_| StagingError::Incomplete)?;
        let parent_path = destination
            .parent()
            .ok_or(StagingError::InvalidDestination)?;
        let parent = directory(parent_path)?;
        let child = name(OsStr::new(leaf))?;
        // mkdirat is atomic no-clobber: existing directories, files and dangling links all fail.
        if unsafe { libc::mkdirat(parent.as_raw_fd(), child.as_ptr(), 0o700) } != 0 {
            return Err(StagingError::DestinationConflict);
        }
        let dir = open_at(
            &parent,
            OsStr::new(leaf),
            libc::O_RDONLY | libc::O_DIRECTORY,
            0,
        )?;
        let metadata = dir.metadata().map_err(|_| StagingError::Incomplete)?;
        // Reject wrong-owner or non-private directories. This does not defeat a
        // malicious same-UID replacement between mkdirat and openat.
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o7777 != 0o700 {
            return Err(StagingError::Incomplete);
        }
        record(
            &dir,
            "incomplete.json",
            &serde_json::json!({"format":1,"owner":"agentdeck","state":"incomplete_archive_staging","version":reviewed.version,"target":TARGET,"installed":false}),
        )?;
        let mut copy = new_file(&dir, "archive.tar.gz")?;
        let (digest, bytes) = NativeSelectedFileReader
            .copy_archive(archive, plan.archive_name, &mut copy)
            .map_err(|_| StagingError::Incomplete)?;
        if digest != plan.sha256 {
            return Err(StagingError::Incomplete);
        }
        copy.sync_all().map_err(|_| StagingError::Incomplete)?;
        // Recheck the private copy via its original handle; never reopen the source path.
        copy.seek(SeekFrom::Start(0))
            .map_err(|_| StagingError::Incomplete)?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut copied = 0u64;
        loop {
            let n = copy
                .read(&mut buffer)
                .map_err(|_| StagingError::Incomplete)?;
            if n == 0 {
                break;
            }
            copied += n as u64;
            if copied > plan.max_archive_bytes {
                return Err(StagingError::Incomplete);
            }
            hash.update(&buffer[..n]);
        }
        let m = copy.metadata().map_err(|_| StagingError::Incomplete)?;
        if copied != bytes
            || m.len() != bytes
            || m.nlink() != 1
            || !m.is_file()
            || m.mode() & 0o7777 != 0o600
            || format!("{:x}", hash.finalize()) != plan.sha256
        {
            return Err(StagingError::Incomplete);
        }
        if identity(&directory(parent_path)?)? != identity(&parent)?
            || identity(&directory(destination)?)? != identity(&dir)?
        {
            return Err(StagingError::Incomplete);
        }
        let named_copy = open_at(
            &dir,
            OsStr::new("archive.tar.gz"),
            libc::O_RDONLY | libc::O_NONBLOCK,
            0,
        )?;
        if identity(&named_copy)? != identity(&copy)? {
            return Err(StagingError::Incomplete);
        }
        record(
            &dir,
            "completion.pending",
            &serde_json::json!({"format":1,"owner":"agentdeck","state":"verified_archive_staged_not_installed","version":reviewed.version,"target":TARGET,"archive_sha256":plan.sha256,"bytes":bytes,"installed":false,"extracted":false,"limitations":LIMITATIONS}),
        )?;
        // Atomic completion-marker publication without replacing any preexisting name.
        let from = CString::new("completion.pending").unwrap();
        let to = CString::new("complete.json").unwrap();
        if unsafe {
            libc::renameat2(
                dir.as_raw_fd(),
                from.as_ptr(),
                dir.as_raw_fd(),
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            return Err(StagingError::Incomplete);
        }
        dir.sync_all().map_err(|_| StagingError::Incomplete)?;
        parent.sync_all().map_err(|_| StagingError::Incomplete)?;
        Ok(StagedPackage {
            state: "verified_archive_staged_not_installed",
            version: reviewed.version,
            target: TARGET,
            bytes,
            installed: false,
            extracted: false,
            limitations: LIMITATIONS,
        })
    }
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };
    struct Fixture {
        root: std::path::PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("qoder-stage-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            Self { root }
        }
        fn source(&self) -> std::path::PathBuf {
            self.root.join("qodercli-linux-x64-baseline.tar.gz")
        }
        fn dest(&self) -> std::path::PathBuf {
            self.root.join("new-stage")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn existing_destination_is_never_reused_or_touched() {
        let f = Fixture::new();
        fs::write(f.source(), b"bad").unwrap();
        fs::create_dir(f.dest()).unwrap();
        fs::write(f.dest().join("sentinel"), b"keep").unwrap();
        assert!(matches!(
            stage_package(TARGET, &f.source(), &f.dest()),
            Err(StagingError::DestinationConflict)
        ));
        assert_eq!(fs::read_dir(f.dest()).unwrap().count(), 1);
        assert_eq!(fs::read(f.dest().join("sentinel")).unwrap(), b"keep");
    }
    #[test]
    fn dangling_destination_and_linked_parent_are_rejected() {
        let f = Fixture::new();
        symlink(f.root.join("missing"), f.dest()).unwrap();
        assert!(stage_package(TARGET, &f.source(), &f.dest()).is_err());
        assert!(!f.root.join("missing").exists());
        fs::remove_file(f.dest()).unwrap();
        let parent = f.root.join("linked");
        symlink(&f.root, &parent).unwrap();
        assert!(stage_package(TARGET, &f.source(), &parent.join("new-stage")).is_err());
        assert!(!f.dest().exists());
    }
    #[test]
    fn mismatch_keeps_private_incomplete_data_without_completion_or_execution() {
        let f = Fixture::new();
        fs::write(f.source(), b"SECRET invalid archive").unwrap();
        let e = stage_package(TARGET, &f.source(), &f.dest()).unwrap_err();
        assert!(!e.to_string().contains("SECRET"));
        assert!(f.dest().join("incomplete.json").is_file());
        assert!(!f.dest().join("complete.json").exists());
        assert_eq!(
            fs::metadata(f.dest()).unwrap().permissions().mode() & 0o7777,
            0o700
        );
        for item in fs::read_dir(f.dest()).unwrap() {
            assert_eq!(
                item.unwrap().metadata().unwrap().permissions().mode() & 0o7777,
                0o600
            );
        }
        assert_eq!(fs::read(f.source()).unwrap(), b"SECRET invalid archive");
        assert!(stage_package(TARGET, &f.source(), &f.dest()).is_err());
    }
    #[test]
    fn unsupported_target_and_bad_destination_create_nothing() {
        let f = Fixture::new();
        assert!(matches!(
            stage_package("windows-amd64-standard", &f.source(), &f.dest()),
            Err(StagingError::Unsupported)
        ));
        assert!(stage_package(TARGET, &f.source(), Path::new("relative")).is_err());
        assert!(stage_package(TARGET, &f.source(), &f.root.join("absent/child")).is_err());
        assert_eq!(fs::read_dir(&f.root).unwrap().count(), 0);
    }
    #[test]
    fn unsafe_source_never_completes() {
        let f = Fixture::new();
        let source = f.root.join("source");
        fs::write(&source, b"keep").unwrap();
        symlink(&source, f.source()).unwrap();
        assert!(stage_package(TARGET, &f.source(), &f.dest()).is_err());
        assert!(!f.dest().join("complete.json").exists());
        assert_eq!(fs::read(source).unwrap(), b"keep");
    }
    #[test]
    fn oversized_source_never_publishes_completion() {
        let f = Fixture::new();
        let source = fs::File::create(f.source()).unwrap();
        source.set_len(256 * 1024 * 1024 + 1).unwrap();
        assert!(stage_package(TARGET, &f.source(), &f.dest()).is_err());
        assert!(f.dest().join("incomplete.json").is_file());
        assert!(!f.dest().join("complete.json").exists());
        assert!(!f.dest().join("completion.pending").exists());
        assert_eq!(
            fs::metadata(f.dest().join("archive.tar.gz")).unwrap().len(),
            0
        );
        assert_eq!(
            fs::metadata(f.source()).unwrap().len(),
            256 * 1024 * 1024 + 1
        );
    }
    #[test]
    fn fifo_source_never_blocks_or_publishes_completion() {
        use std::{
            ffi::CString,
            os::unix::{ffi::OsStrExt, fs::FileTypeExt},
        };
        let f = Fixture::new();
        let source = CString::new(f.source().as_os_str().as_bytes()).unwrap();
        // SAFETY: valid NUL-terminated disposable fixture path and POSIX mode.
        assert_eq!(unsafe { libc::mkfifo(source.as_ptr(), 0o600) }, 0);
        assert!(stage_package(TARGET, &f.source(), &f.dest()).is_err());
        assert!(f.dest().join("incomplete.json").is_file());
        assert!(!f.dest().join("complete.json").exists());
        assert!(!f.dest().join("completion.pending").exists());
        assert_eq!(
            fs::metadata(f.dest().join("archive.tar.gz")).unwrap().len(),
            0
        );
        assert!(
            fs::symlink_metadata(f.source())
                .unwrap()
                .file_type()
                .is_fifo()
        );
    }
    #[test]
    fn copy_write_failure_propagates_and_preserves_source_and_destination() {
        use crate::inspection::reader::NativeSelectedFileReader;
        let f = Fixture::new();
        fs::write(f.source(), b"source unchanged").unwrap();
        let target = f.root.join("readonly-destination");
        fs::write(&target, b"destination unchanged").unwrap();
        let mut readonly = fs::File::open(&target).unwrap();
        assert!(
            NativeSelectedFileReader
                .copy_archive(
                    &f.source(),
                    "qodercli-linux-x64-baseline.tar.gz",
                    &mut readonly
                )
                .is_err()
        );
        assert_eq!(fs::read(f.source()).unwrap(), b"source unchanged");
        assert_eq!(fs::read(&target).unwrap(), b"destination unchanged");
        assert_eq!(fs::read_dir(&f.root).unwrap().count(), 2);
        assert!(!f.dest().exists());
    }
}
