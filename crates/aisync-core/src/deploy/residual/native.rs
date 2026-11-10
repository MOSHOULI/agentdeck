//! Read-capable Windows handles used solely for metadata queries (no ReadFile or
//! directory enumeration). All existing ancestor handles remain open without
//! FILE_SHARE_WRITE or FILE_SHARE_DELETE. Reparse points are opened themselves,
//! never followed. Attribute-only access does not establish the needed sharing
//! exclusion; FILE_READ_DATA (FILE_LIST_DIRECTORY for directories) does.
use super::*;
use std::{
    ffi::OsStr,
    fs::File,
    io, mem,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::{Component, PathBuf},
};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, INVALID_HANDLE_VALUE},
    Storage::FileSystem::*,
    System::WindowsProgramming::DRIVE_FIXED,
};

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    volume: u32,
    index: u64,
    creation: i64,
    write: i64,
    change: i64,
    attributes: u32,
    size: i64,
    links: u32,
    directory: bool,
    delete_pending: bool,
}
fn snapshot(file: &File) -> Result<Snapshot> {
    unsafe {
        let handle = file.as_raw_handle() as _;
        if GetFileType(handle) != FILE_TYPE_DISK {
            return Err(ResidualInspectionError::Unavailable);
        }
        // Directory modification timestamps describe unrelated children; pin identity,
        // type and attributes instead. File changes are conservatively rejected.
        let mut identity: BY_HANDLE_FILE_INFORMATION = mem::zeroed();
        let mut basic: FILE_BASIC_INFO = mem::zeroed();
        let mut standard: FILE_STANDARD_INFO = mem::zeroed();
        if GetFileInformationByHandle(handle, &mut identity) == 0
            || GetFileInformationByHandleEx(
                handle,
                FileBasicInfo,
                &mut basic as *mut _ as _,
                mem::size_of_val(&basic) as u32,
            ) == 0
            || GetFileInformationByHandleEx(
                handle,
                FileStandardInfo,
                &mut standard as *mut _ as _,
                mem::size_of_val(&standard) as u32,
            ) == 0
        {
            return Err(ResidualInspectionError::Unavailable);
        }
        Ok(Snapshot {
            volume: identity.dwVolumeSerialNumber,
            index: ((identity.nFileIndexHigh as u64) << 32) | identity.nFileIndexLow as u64,
            creation: basic.CreationTime,
            write: if standard.Directory != 0 {
                0
            } else {
                basic.LastWriteTime
            },
            change: if standard.Directory != 0 {
                0
            } else {
                basic.ChangeTime
            },
            attributes: basic.FileAttributes,
            size: if standard.Directory != 0 {
                0
            } else {
                standard.EndOfFile
            },
            links: standard.NumberOfLinks,
            directory: standard.Directory != 0,
            delete_pending: standard.DeletePending != 0,
        })
    }
}
fn open(path: &Path) -> Result<Option<File>> {
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            // Attribute-only access is outside the relevant sharing checks.
            // Request read capability to establish them, but never consume data.
            // https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew
            FILE_READ_DATA | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        let error = io::Error::last_os_error().raw_os_error().map(|v| v as u32);
        return if matches!(error, Some(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND)) {
            Ok(None)
        } else {
            Err(ResidualInspectionError::Unavailable)
        };
    }
    Ok(Some(unsafe { File::from_raw_handle(handle as _) }))
}
fn blocked(s: &Snapshot) -> bool {
    s.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || s.delete_pending
        || (!s.directory && s.links != 1)
}
fn fixed_drive(kind: u32) -> bool {
    kind == DRIVE_FIXED
}
fn validate(prefix: &Path) -> Result<()> {
    let text = prefix
        .to_str()
        .ok_or(ResidualInspectionError::Unavailable)?;
    if text.len() > 4096 || text.trim() != text || text.chars().any(|c| c.is_control() || matches!(c, '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')) {
        return Err(ResidualInspectionError::Unavailable);
    }
    let parts: Vec<_> = prefix.components().collect();
    if parts.len() > 32
        || parts.len() < 2
        || !matches!(parts[0], Component::Prefix(p) if matches!(p.kind(), std::path::Prefix::Disk(_)))
        || parts[1] != Component::RootDir
    {
        return Err(ResidualInspectionError::Unavailable);
    }
    // A mapped remote drive also has a Disk prefix and FILE_TYPE_DISK. Refuse it
    // before opening ancestors; only fixed local volumes are reviewed here.
    let root = format!("{}\\", &text[..2]);
    let wide: Vec<u16> = root.encode_utf16().chain(Some(0)).collect();
    if !fixed_drive(unsafe { GetDriveTypeW(wide.as_ptr()) }) {
        return Err(ResidualInspectionError::Unavailable);
    }
    // Reject components Win32 might normalize or interpret, even if Path removes them.
    for part in text[3..].split(['\\', '/']).filter(|s| !s.is_empty()) {
        let base = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let reserved = matches!(
            base.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "COM¹" | "COM²" | "COM³" | "LPT¹" | "LPT²" | "LPT³"
        ) || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && base.as_bytes()[3].is_ascii_digit());
        if part == "."
            || part == ".."
            || part.ends_with([' ', '.'])
            || part.chars().any(|c| r#"<>:\"|?*"#.contains(c))
            || reserved
        {
            return Err(ResidualInspectionError::Unavailable);
        }
    }
    Ok(())
}
struct Held {
    path: PathBuf,
    file: File,
    before: Snapshot,
}
#[derive(Default)]
struct Observation {
    held: Vec<Held>,
    missing: Vec<PathBuf>,
}
impl Observation {
    fn take(&mut self, path: &Path) -> Result<Option<Snapshot>> {
        let Some(file) = open(path)? else {
            self.missing.push(path.to_owned());
            return Ok(None);
        };
        let before = snapshot(&file)?;
        // Query twice rather than clone identity data so races fail conservatively.
        let result = snapshot(&file)?;
        if before != result {
            return Err(ResidualInspectionError::Changed);
        }
        self.held.push(Held {
            path: path.to_owned(),
            file,
            before,
        });
        Ok(Some(result))
    }
    fn prefix(&mut self, prefix: &Path) -> Result<()> {
        let mut path = PathBuf::new();
        for component in prefix.components() {
            path.push(component.as_os_str());
            if matches!(component, Component::Prefix(_)) {
                continue;
            }
            let info = self
                .take(&path)?
                .ok_or(ResidualInspectionError::Unavailable)?;
            if blocked(&info) || !info.directory {
                return Err(ResidualInspectionError::Unavailable);
            }
        }
        Ok(())
    }
    fn slot(&mut self, prefix: &Path, components: &[&str]) -> Result<SlotState> {
        let mut path = prefix.to_owned();
        for (index, name) in components.iter().enumerate() {
            path.push(OsStr::new(name));
            let Some(info) = self.take(&path)? else {
                return Ok(SlotState::Missing);
            };
            if blocked(&info) {
                return Ok(SlotState::Blocked);
            }
            if index + 1 != components.len() {
                if !info.directory {
                    return Ok(SlotState::Blocked);
                }
            } else {
                return Ok(if info.directory {
                    SlotState::Directory
                } else {
                    SlotState::File
                });
            }
        }
        Err(ResidualInspectionError::Unavailable)
    }
    fn verify(&self) -> Result<()> {
        for held in &self.held {
            if snapshot(&held.file)? != held.before {
                return Err(ResidualInspectionError::Changed);
            }
            let current = open(&held.path)?.ok_or(ResidualInspectionError::Changed)?;
            if snapshot(&current)? != held.before {
                return Err(ResidualInspectionError::Changed);
            }
        }
        for path in &self.missing {
            if open(path)?.is_some() {
                return Err(ResidualInspectionError::Changed);
            }
        }
        Ok(())
    }
}
pub(super) fn inspect(prefix: &Path) -> Result<ResidualObservation> {
    inspect_with(prefix, |_| {})
}
fn inspect_with(prefix: &Path, before_verify: impl FnOnce(&Path)) -> Result<ResidualObservation> {
    validate(prefix)?;
    let mut observation = Observation::default();
    observation.prefix(prefix)?;
    let states = [
        observation.slot(prefix, &["node_modules", "@openai", "codex"])?,
        observation.slot(prefix, &["codex"])?,
        observation.slot(prefix, &["codex.cmd"])?,
        observation.slot(prefix, &["codex.ps1"])?,
    ];
    before_verify(prefix);
    observation.verify()?;
    validate(prefix)?;
    Ok(report(states))
}
#[cfg(test)]
mod tests;
