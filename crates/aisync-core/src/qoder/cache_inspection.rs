//! Read-only observation. Cache records never authorize cleanup or execution.
use serde::Serialize;
use std::path::Path;

pub const LIMITATIONS: &str = "Observation only, not installation, execution or deletion authorization. Records and same-source hashes are not signatures. No locks, writes, repair, cleanup or PATH changes. Reads may update access times; same-user hostile races, atomic snapshots and mount I/O deadlines are not solved. Preserve unknown or incomplete data for manual review; never retry writes over an existing directory.";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheState {
    StagedArchiveVerified,
    UnpackedPayloadVerified,
    Incomplete,
    Conflict,
    UnknownOwnership,
}
#[derive(Debug, Serialize)]
pub struct CacheObservation {
    pub state: CacheState,
    pub recovery_plan: &'static str,
    pub deletion_authorized: bool,
    pub manual_review_required: bool,
    pub installed: bool,
    pub limitations: &'static str,
}
#[derive(Debug, thiserror::Error)]
pub enum InspectError {
    #[error("Only Qoder baseline cache inspection on Linux x86_64 is reviewed")]
    Unsupported,
    #[error("Explicit cache directory could not be safely inspected; no changes were made")]
    Unavailable,
}
pub fn inspect_cache(path: &Path) -> Result<CacheObservation, InspectError> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let state = linux::inspect(path)?;
        Ok(CacheObservation {
            state,
            recovery_plan: match state {
                CacheState::StagedArchiveVerified | CacheState::UnpackedPayloadVerified => {
                    "retain_cache_no_action"
                }
                _ => "preserve_and_review",
            },
            deletion_authorized: false,
            manual_review_required: true,
            installed: false,
            limitations: LIMITATIONS,
        })
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = path;
        Err(InspectError::Unsupported)
    }
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::*;
    use crate::qoder::{
        distribution,
        staging::{self, linux as fsio},
        unpacking::{self, linux as recordio},
    };
    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use std::{
        collections::BTreeSet,
        ffi::{CStr, OsStr},
        fs::{File, Metadata},
        io::Read,
        os::unix::fs::MetadataExt,
    };
    const ARCHIVE_BYTES: u64 = 78_587_775;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Intent {
        format: u8,
        owner: String,
        state: String,
        version: String,
        target: String,
        installed: bool,
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Unpacked {
        format: u8,
        owner: String,
        state: String,
        version: String,
        target: String,
        archive_sha256: String,
        payload_sha256: String,
        payload_bytes: u64,
        installed: bool,
        executable: bool,
        limitations: String,
    }
    fn snapshot(m: &Metadata) -> (u64, u64, u32, u32, u64, u64, i64, i64, i64, i64) {
        (
            m.dev(),
            m.ino(),
            m.uid(),
            m.mode(),
            m.nlink(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    }
    fn private_dir(dir: &File) -> Result<Metadata, ()> {
        let m = dir.metadata().map_err(|_| ())?;
        if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o7777 != 0o700 {
            return Err(());
        }
        Ok(m)
    }
    // Own a separate directory descriptor. fdopendir takes ownership on success;
    // closedir releases it even on early failure. No path traversal or writes.
    struct Dir(*mut libc::DIR);
    impl Drop for Dir {
        fn drop(&mut self) {
            unsafe {
                libc::closedir(self.0);
            }
        }
    }
    fn entries(dir: &File) -> Result<BTreeSet<String>, ()> {
        let copy = fsio::open_at(dir, OsStr::new("."), libc::O_RDONLY | libc::O_DIRECTORY, 0)
            .map_err(|_| ())?;
        use std::os::fd::IntoRawFd;
        let fd = copy.into_raw_fd();
        let raw = unsafe { libc::fdopendir(fd) };
        if raw.is_null() {
            unsafe {
                libc::close(fd);
            }
            return Err(());
        }
        let stream = Dir(raw);
        let mut names = BTreeSet::new();
        loop {
            unsafe {
                *libc::__errno_location() = 0;
            }
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                return if unsafe { *libc::__errno_location() } == 0 {
                    Ok(names)
                } else {
                    Err(())
                };
            }
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
                .to_str()
                .map_err(|_| ())?;
            if name == "." || name == ".." {
                continue;
            }
            if names.len() >= 5 || !names.insert(name.to_owned()) {
                return Err(());
            }
        }
    }
    fn file(dir: &File, name: &str, max: u64) -> Result<File, ()> {
        let file = fsio::open_at(dir, OsStr::new(name), libc::O_RDONLY | libc::O_NONBLOCK, 0)
            .map_err(|_| ())?;
        recordio::private_file(&file, max).map_err(|_| ())?;
        Ok(file)
    }
    fn digest(dir: &File, name: &str, size: u64, expected: &str) -> Result<(), ()> {
        let mut f = file(dir, name, size)?;
        let before = f.metadata().map_err(|_| ())?;
        if before.len() != size {
            return Err(());
        }
        let mut hash = Sha256::new();
        let mut bytes = 0;
        let mut buf = [0; 65536];
        loop {
            let n = f.read(&mut buf).map_err(|_| ())?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            if bytes > size {
                return Err(());
            }
            hash.update(&buf[..n]);
        }
        let named = file(dir, name, size)?;
        if bytes != size
            || format!("{:x}", hash.finalize()) != expected
            || snapshot(&before) != snapshot(&f.metadata().map_err(|_| ())?)
            || snapshot(&before) != snapshot(&named.metadata().map_err(|_| ())?)
        {
            return Err(());
        }
        Ok(())
    }
    fn observe(dir: &File, names: &BTreeSet<String>) -> Result<CacheState, ()> {
        if !names.contains("incomplete.json") {
            return Ok(CacheState::UnknownOwnership);
        }
        let (intent, original) = match recordio::read_record::<Intent>(dir, "incomplete.json") {
            Ok(v) => v,
            Err(_) => return Ok(CacheState::UnknownOwnership),
        };
        if intent.format != 1
            || intent.owner != "agentdeck"
            || intent.version != distribution::VERSION
            || intent.target != staging::TARGET
            || intent.installed
        {
            return Ok(CacheState::UnknownOwnership);
        }
        let unpack = match intent.state.as_str() {
            "incomplete_archive_staging" => false,
            "incomplete_archive_unpack" => true,
            _ => return Ok(CacheState::UnknownOwnership),
        };
        let allowed = if unpack {
            vec![
                "incomplete.json",
                "complete.json",
                "archive.tar.gz",
                "qodercli",
            ]
        } else {
            vec!["incomplete.json", "complete.json", "archive.tar.gz"]
        };
        if names.iter().any(|n| !allowed.contains(&n.as_str())) {
            return Err(());
        }
        for name in names {
            let max = match name.as_str() {
                "archive.tar.gz" => ARCHIVE_BYTES,
                "qodercli" => unpacking::PAYLOAD_BYTES,
                _ => 65536,
            };
            file(dir, name, max)?;
        }
        if !names.contains("complete.json") {
            return Ok(CacheState::Incomplete);
        }
        if names.len() != allowed.len() {
            return Err(());
        }
        let plan = distribution::plan(staging::TARGET).map_err(|_| ())?;
        let completed = if unpack {
            let (c, bytes) =
                recordio::read_record::<Unpacked>(dir, "complete.json").map_err(|_| ())?;
            if c.format != 1
                || c.owner != "agentdeck"
                || c.state != "verified_payload_unpacked_not_installed"
                || c.version != distribution::VERSION
                || c.target != staging::TARGET
                || c.archive_sha256 != plan.sha256
                || c.payload_sha256 != unpacking::PAYLOAD_SHA256
                || c.payload_bytes != unpacking::PAYLOAD_BYTES
                || c.installed
                || c.executable
                || c.limitations != unpacking::LIMITATIONS
            {
                return Err(());
            }
            digest(
                dir,
                "qodercli",
                unpacking::PAYLOAD_BYTES,
                unpacking::PAYLOAD_SHA256,
            )?;
            bytes
        } else {
            recordio::validate_records(dir).map_err(|_| ())?.0
        };
        digest(dir, "archive.tar.gz", ARCHIVE_BYTES, plan.sha256)?;
        let (_, now) = recordio::read_record::<Intent>(dir, "incomplete.json").map_err(|_| ())?;
        if now != original {
            return Err(());
        }
        let now = if unpack {
            recordio::read_record::<Unpacked>(dir, "complete.json")
                .map_err(|_| ())?
                .1
        } else {
            recordio::validate_records(dir).map_err(|_| ())?.0
        };
        if now != completed {
            return Err(());
        }
        Ok(if unpack {
            CacheState::UnpackedPayloadVerified
        } else {
            CacheState::StagedArchiveVerified
        })
    }
    pub(super) fn inspect(path: &Path) -> Result<CacheState, InspectError> {
        let leaf = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or(InspectError::Unavailable)?;
        crate::inspection::reader::validate_path(path, leaf)
            .map_err(|_| InspectError::Unavailable)?;
        let dir = fsio::directory(path).map_err(|_| InspectError::Unavailable)?;
        let before = match private_dir(&dir) {
            Ok(m) => m,
            Err(_) => return Ok(CacheState::Conflict),
        };
        let names = match entries(&dir) {
            Ok(v) => v,
            Err(_) => return Ok(CacheState::Conflict),
        };
        let mut observed_files = Vec::new();
        for name in &names {
            let max = match name.as_str() {
                "incomplete.json" | "complete.json" => 65536,
                "archive.tar.gz" => ARCHIVE_BYTES,
                "qodercli" => unpacking::PAYLOAD_BYTES,
                _ => continue, // Unknown entries are never opened or disclosed.
            };
            let f = match file(&dir, name, max) {
                Ok(v) => v,
                Err(_) => return Ok(CacheState::Conflict),
            };
            let metadata = f.metadata().map_err(|_| InspectError::Unavailable)?;
            observed_files.push((name, max, f, metadata));
        }
        let state = observe(&dir, &names).unwrap_or(CacheState::Conflict);
        let named = fsio::directory(path).map_err(|_| InspectError::Unavailable)?;
        if snapshot(&before) != snapshot(&private_dir(&dir).map_err(|_| InspectError::Unavailable)?)
            || snapshot(&before)
                != snapshot(&private_dir(&named).map_err(|_| InspectError::Unavailable)?)
            || entries(&dir).ok().as_ref() != Some(&names)
        {
            return Ok(CacheState::Conflict);
        }
        for (name, max, f, metadata) in observed_files {
            let named = match file(&dir, name, max) {
                Ok(v) => v,
                Err(_) => return Ok(CacheState::Conflict),
            };
            if snapshot(&metadata)
                != snapshot(&f.metadata().map_err(|_| InspectError::Unavailable)?)
                || snapshot(&metadata)
                    != snapshot(&named.metadata().map_err(|_| InspectError::Unavailable)?)
            {
                return Ok(CacheState::Conflict);
            }
        }
        Ok(state)
    }
    #[cfg(test)]
    mod tests;
}
