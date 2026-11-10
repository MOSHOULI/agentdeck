//! Read-only Windows observation, not a write eligibility or authorization API.
//! No agent execution, profile initialization, locks, receipt reads or write tokens.
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationState {
    Ready,
    Missing,
    Unsupported,
    ReceiptPresent,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub schema_version: u8,
    pub read_only: bool,
    pub state: ObservationState,
    pub current: Option<bool>,
    pub apply_supported: bool,
    pub restore_supported: bool,
}
impl Observation {
    fn state(state: ObservationState) -> Self {
        Self {
            schema_version: 1,
            read_only: true,
            state,
            current: None,
            apply_supported: false,
            restore_supported: false,
        }
    }
}
#[cfg(any(windows, test))]
fn document(bytes: &[u8]) -> Observation {
    match super::codex_animations::preview(bytes, b"read-only-observation", false) {
        Ok(p) => Observation {
            current: Some(p.before),
            ..Observation::state(ObservationState::Ready)
        },
        Err(_) => Observation::state(ObservationState::Unsupported),
    }
}

/// Caller selects the fixed home/registry target and checks CODEX_HOME separately.
/// A receipt of any kind blocks the observation; its contents are never read.
pub fn observe(path: &Path) -> Observation {
    #[cfg(windows)]
    {
        native::observe(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Observation::state(ObservationState::Unsupported)
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use crate::inspection::reader::{NativeSelectedFileReader, validate_path};
    use std::{
        fs,
        os::windows::fs::MetadataExt,
        path::{Component, PathBuf},
    };
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    const RECEIPT: &str = ".agentdeck-codex-animations.json";
    fn ordinary(metadata: &fs::Metadata) -> bool {
        !metadata.file_type().is_symlink()
            && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0
    }
    fn parent_status(path: &Path) -> std::result::Result<(), ObservationState> {
        let mut current = PathBuf::new();
        for part in path
            .parent()
            .ok_or(ObservationState::Unsupported)?
            .components()
        {
            current.push(part);
            if matches!(part, Component::Prefix(_)) {
                continue;
            }
            let metadata = match fs::symlink_metadata(&current) {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(ObservationState::Missing);
                }
                Err(_) => return Err(ObservationState::Unsupported),
            };
            if !ordinary(&metadata) || !metadata.is_dir() {
                return Err(ObservationState::Unsupported);
            }
        }
        Ok(())
    }
    fn receipt_handle_safe(path: &Path) -> std::result::Result<(), ObservationState> {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_READ_ATTRIBUTES, GetFileInformationByHandle,
        };
        // Attribute-only access: never read or parse receipt content.
        let file = fs::OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| ObservationState::Unsupported)?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
            || info.nNumberOfLinks != 1
            || info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)
                != 0
        {
            return Err(ObservationState::Unsupported);
        }
        Ok(())
    }
    fn receipt_exists(path: &Path) -> std::result::Result<bool, ObservationState> {
        match fs::symlink_metadata(path.with_file_name(RECEIPT)) {
            Ok(m) if ordinary(&m) && m.is_file() => {
                receipt_handle_safe(&path.with_file_name(RECEIPT)).map(|_| true)
            }
            Ok(_) => Err(ObservationState::Unsupported),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err(ObservationState::Unsupported),
        }
    }
    pub(super) fn observe(path: &Path) -> Observation {
        let text = match path.to_str() {
            Some(t) => t,
            None => return Observation::state(ObservationState::Unsupported),
        };
        if text.chars().any(char::is_control) || validate_path(path, "config.toml").is_err() {
            return Observation::state(ObservationState::Unsupported);
        }
        if let Err(state) = parent_status(path) {
            return Observation::state(state);
        }
        match receipt_exists(path) {
            Ok(true) => return Observation::state(ObservationState::ReceiptPresent),
            Err(state) => return Observation::state(state),
            _ => {}
        }
        match fs::symlink_metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Observation::state(ObservationState::Missing);
            }
            Err(_) => return Observation::state(ObservationState::Unsupported),
            Ok(m) if !ordinary(&m) || !m.is_file() => {
                return Observation::state(ObservationState::Unsupported);
            }
            _ => {}
        }
        let bytes = match NativeSelectedFileReader.read_named(path, "config.toml") {
            Ok(b) => b,
            Err(_) => return Observation::state(ObservationState::Unsupported),
        };
        // Recheck before returning the observation; no atomic snapshot claim.
        if parent_status(path).is_err() {
            return Observation::state(ObservationState::Unsupported);
        }
        match receipt_exists(path) {
            Ok(false) => document(&bytes),
            Ok(true) => Observation::state(ObservationState::ReceiptPresent),
            Err(state) => Observation::state(state),
        }
    }
}
#[cfg(test)]
mod tests;
