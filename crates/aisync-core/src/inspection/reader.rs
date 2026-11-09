//! Audited selected-source I/O boundary: bounded read/hash and explicit copying
//! into a caller-owned private destination handle. Source files are read-only;
//! this module never opens destination paths or runs processes. Inspection tests
//! inject an allowlist reader that rejects every other source path.
use std::fs::{self, File, Metadata, OpenOptions};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::io::Write;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use super::{InspectionIssue, LIMITS};

pub(crate) trait SelectedFileReader {
    fn read_selected(&self, path: &Path) -> Result<Vec<u8>, InspectionIssue>;
}

pub(crate) struct NativeSelectedFileReader;

pub(crate) fn validate_path(path: &Path, expected_name: &str) -> Result<(), InspectionIssue> {
    if !path.is_absolute() {
        return Err(InspectionIssue::PathMustBeAbsolute);
    }
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(InspectionIssue::UnsupportedPath);
    }
    if path.as_os_str().len() > LIMITS.max_path_bytes
        || path.components().count() > LIMITS.max_path_components
    {
        return Err(InspectionIssue::PathLimit);
    }
    for component in path.components() {
        match component {
            Component::ParentDir => return Err(InspectionIssue::ParentTraversal),
            Component::Prefix(prefix) => {
                // No UNC, device namespace or verbatim path support in this slice.
                if !matches!(prefix.kind(), std::path::Prefix::Disk(_)) {
                    return Err(InspectionIssue::UnsupportedPath);
                }
            }
            Component::Normal(name) => {
                #[cfg(windows)]
                {
                    let name = name.to_str().ok_or(InspectionIssue::UnsupportedPath)?;
                    let base = name.split('.').next().unwrap_or("").to_ascii_uppercase();
                    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                        || (base.len() == 4
                            && (base.starts_with("COM") || base.starts_with("LPT"))
                            && base.as_bytes()[3].is_ascii_digit());
                    if name.chars().any(|c| c < ' ' || r#"<>:"|?*"#.contains(c))
                        || name.ends_with([' ', '.'])
                        || reserved
                    {
                        return Err(InspectionIssue::UnsupportedPath);
                    }
                }
                #[cfg(not(windows))]
                let _ = name;
            }
            _ => {}
        }
    }
    if path.file_name() != Some(std::ffi::OsStr::new(expected_name)) {
        return Err(InspectionIssue::UnexpectedFileName);
    }
    Ok(())
}

impl SelectedFileReader for NativeSelectedFileReader {
    fn read_selected(&self, path: &Path) -> Result<Vec<u8>, InspectionIssue> {
        let expected = match path.file_name().and_then(|v| v.to_str()) {
            Some("package.json") => "package.json",
            Some("settings.json") => "settings.json",
            Some("settings.local.json") => "settings.local.json",
            Some(".mcp.json") => ".mcp.json",
            _ => return Err(InspectionIssue::UnexpectedFileName),
        };
        self.read_named(path, expected)
    }
}

impl NativeSelectedFileReader {
    pub(crate) fn read_named(
        &self,
        path: &Path,
        expected: &str,
    ) -> Result<Vec<u8>, InspectionIssue> {
        let mut bytes = Vec::new();
        self.visit_named(path, expected, LIMITS.max_manifest_bytes as u64, |chunk| {
            bytes.extend_from_slice(chunk);
            Ok(())
        })?;
        Ok(bytes)
    }

    /// Dedicated archive hashing; callers cannot enlarge settings inspection limits.
    pub(crate) fn hash_archive(
        &self,
        path: &Path,
        expected: &str,
    ) -> Result<(String, u64), InspectionIssue> {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        let size = self.visit_named(path, expected, 256 * 1024 * 1024, |chunk| {
            hash.update(chunk);
            Ok(())
        })?;
        Ok((format!("{:x}", hash.finalize()), size))
    }

    /// Same audited source handle supplies both the private copy and its digest.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub(crate) fn copy_archive(
        &self,
        path: &Path,
        expected: &str,
        destination: &mut File,
    ) -> Result<(String, u64), InspectionIssue> {
        use sha2::{Digest, Sha256};
        let mut hash = Sha256::new();
        let size = self.visit_named(path, expected, 256 * 1024 * 1024, |chunk| {
            destination.write_all(chunk)?;
            hash.update(chunk);
            Ok(())
        })?;
        Ok((format!("{:x}", hash.finalize()), size))
    }

    fn visit_named(
        &self,
        path: &Path,
        expected: &str,
        limit: u64,
        mut visit: impl FnMut(&[u8]) -> io::Result<()>,
    ) -> Result<u64, InspectionIssue> {
        validate_path(path, expected)?;
        // Refuse all platforms without the no-follow/non-blocking flags audited
        // below. Future ports need explicit review, not a weaker fallback open.
        if !cfg!(any(
            all(
                target_os = "linux",
                any(target_arch = "x86_64", target_arch = "aarch64")
            ),
            windows
        )) {
            return Err(InspectionIssue::PlatformUnsupported);
        }
        let before = check_components(path)?;
        check_file(&before, limit)?;
        let file = open_read_only(path).map_err(io_issue)?;
        #[cfg(windows)]
        let handle_identity = checked_handle_identity(&file)?;
        let opened = file.metadata().map_err(io_issue)?;
        check_file(&opened, limit)?;
        if !same_file(&before, &opened) {
            return Err(InspectionIssue::FileChanged);
        }
        // Check ancestors again before reading. This narrows, but does not close,
        // same-privilege ancestor-replacement races (no sandbox claim).
        let rechecked = check_components(path)?;
        if !same_file(&opened, &rechecked) {
            return Err(InspectionIssue::FileChanged);
        }
        // Reopen only to compare pathname identity, never to consume different bytes.
        #[cfg(windows)]
        if checked_handle_identity(&open_read_only(path).map_err(io_issue)?)? != handle_identity {
            return Err(InspectionIssue::FileChanged);
        }
        let mut bounded = (&file).take(limit + 1);
        let mut buffer = [0u8; 64 * 1024];
        let mut size = 0u64;
        loop {
            let count = bounded.read(&mut buffer).map_err(io_issue)?;
            if count == 0 {
                break;
            }
            size += count as u64;
            if size > limit {
                return Err(InspectionIssue::FileTooLarge);
            }
            visit(&buffer[..count]).map_err(io_issue)?;
        }
        #[cfg(windows)]
        if checked_handle_identity(&file)? != handle_identity {
            return Err(InspectionIssue::FileChanged);
        }
        let after = file.metadata().map_err(io_issue)?;
        check_file(&after, limit)?;
        if !same_file(&opened, &after)
            || opened.len() != after.len()
            || opened.modified().ok() != after.modified().ok()
            || size != after.len()
        {
            return Err(InspectionIssue::FileChanged);
        }
        let final_path = check_components(path)?;
        if !same_file(&after, &final_path) {
            return Err(InspectionIssue::FileChanged);
        }
        #[cfg(windows)]
        if checked_handle_identity(&open_read_only(path).map_err(io_issue)?)? != handle_identity {
            return Err(InspectionIssue::FileChanged);
        }
        Ok(size)
    }
}

fn check_components(path: &Path) -> Result<Metadata, InspectionIssue> {
    let mut current = PathBuf::new();
    let count = path.components().count();
    let mut leaf = None;
    for (index, component) in path.components().enumerate() {
        current.push(component.as_os_str());
        // A Windows drive prefix alone is not yet an absolute directory.
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata = fs::symlink_metadata(&current).map_err(io_issue)?;
        reject_link(&metadata)?;
        if index + 1 < count && !metadata.is_dir() {
            return Err(InspectionIssue::NotDirectory);
        }
        leaf = Some(metadata);
    }
    leaf.ok_or(InspectionIssue::UnsupportedPath)
}

fn reject_link(metadata: &Metadata) -> Result<(), InspectionIssue> {
    if metadata.file_type().is_symlink() {
        return Err(InspectionIssue::LinkOrReparsePoint);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(InspectionIssue::LinkOrReparsePoint);
        }
    }
    Ok(())
}

fn check_file(metadata: &Metadata, limit: u64) -> Result<(), InspectionIssue> {
    reject_link(metadata)?;
    if !metadata.is_file() {
        return Err(InspectionIssue::NotRegularFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(InspectionIssue::MultipleHardLinks);
        }
    }
    if metadata.len() > limit {
        return Err(InspectionIssue::FileTooLarge);
    }
    Ok(())
}

fn same_file(left: &Metadata, right: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        // Windows std metadata does not expose stable file IDs. Reparse checks
        // and this comparison cannot establish race-free handle identity.
        left.len() == right.len() && left.modified().ok() == right.modified().ok()
    }
}

fn open_read_only(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Linux ABI constants, deliberately not used on other Unix platforms.
        // O_NOFOLLOW rejects a replaced leaf symlink; O_NONBLOCK avoids a FIFO
        // open blocking before handle metadata can reject a replaced special file.
        const O_NOFOLLOW: i32 = 0o400000;
        const O_NONBLOCK: i32 = 0o4000;
        options.custom_flags(O_NOFOLLOW | O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

fn io_issue(error: io::Error) -> InspectionIssue {
    match error.kind() {
        io::ErrorKind::NotFound => InspectionIssue::Missing,
        io::ErrorKind::PermissionDenied => InspectionIssue::PermissionDenied,
        _ => InspectionIssue::IoFailure,
    }
}

#[cfg(windows)]
fn checked_handle_identity(file: &File) -> Result<(u32, u64), InspectionIssue> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, GetFileInformationByHandle,
    };
    // SAFETY: this POD struct has only integer fields, and `file` owns a live,
    // readable handle for the duration of the call. The API writes into `info`.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let success = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if success == 0 {
        return Err(io_issue(io::Error::last_os_error()));
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(InspectionIssue::LinkOrReparsePoint);
    }
    if info.nNumberOfLinks != 1 {
        return Err(InspectionIssue::MultipleHardLinks);
    }
    Ok((
        info.dwVolumeSerialNumber,
        ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
    ))
}
