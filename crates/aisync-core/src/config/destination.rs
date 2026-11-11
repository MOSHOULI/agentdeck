//! Refuse existing linked destinations before reading/backing up/replacing them.
//! Metadata checks are not a cross-process transaction or race-free confinement.
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

const MESSAGE: &str = "Configuration target or parent is linked or is not a regular file/directory; AgentDeck refused this target. Manage it with the tool that created the link, or prepare a separate regular file in an unlinked directory yourself before trying again. No links were automatically detached.";

fn refused() -> crate::Error {
    crate::Error::InjectFailed(MESSAGE.into())
}

fn reject_link(metadata: &fs::Metadata) -> crate::Result<()> {
    if metadata.file_type().is_symlink() {
        return Err(refused());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return Err(refused());
        }
    }
    Ok(())
}

/// Walk existing components without intentionally following a link. Missing leaf
/// or parent is allowed. Recheck at mutation boundaries, but concurrent pathname
/// substitution can still invalidate an observation.
pub(super) fn check(path: &Path) -> crate::Result<()> {
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(refused());
    }
    let mut current = PathBuf::new();
    let count = path.components().count();
    for (index, component) in path.components().enumerate() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        reject_link(&metadata)?;
        if index + 1 < count {
            if !metadata.is_dir() {
                return Err(refused());
            }
        } else {
            if !metadata.is_file() {
                return Err(refused());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.nlink() != 1 {
                    return Err(refused());
                }
            }
            #[cfg(windows)]
            check_windows_handle(&current)?;
            super::metadata::check_path(&current)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn check_windows_handle(path: &Path) -> crate::Result<()> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_FLAG_OPEN_REPARSE_POINT, GetFileInformationByHandle,
    };
    // Desired access zero obtains metadata without reading configuration bytes.
    let file = fs::OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    // SAFETY: integer-only output structure and live handle owned by `file`.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if info.nNumberOfLinks != 1
        || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0
    {
        return Err(refused());
    }
    Ok(())
}
