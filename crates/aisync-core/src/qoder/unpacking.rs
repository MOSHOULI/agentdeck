//! Strict extraction of a pinned archive cache. Never activation or execution.
use serde::Serialize;
use std::path::Path;
pub const PAYLOAD_BYTES: u64 = 182_543_840;
pub const PAYLOAD_SHA256: &str = "8af52502ae32eabd8eac922e6dc5769840d710bcf0374f50f23339779f19b06b";
pub const LIMITATIONS: &str = "Private extracted cache only, not installed or executable/runtime-ready. Ownership records are not authorization or signatures. Fixed same-source digests are not independent authenticity verification. No execution, PATH, login or config changes. Failure may leave incomplete output; never retry over an existing directory. Same-user hostile races, mount deadlines and full power-loss recovery are not solved.";
#[derive(Debug, Serialize)]
pub struct UnpackedCache {
    pub state: &'static str,
    pub installed: bool,
    pub executable: bool,
    pub payload_bytes: u64,
    pub payload_sha256: &'static str,
    pub limitations: &'static str,
}
#[derive(Debug, thiserror::Error)]
pub enum UnpackError {
    #[error("Only pinned Qoder baseline cache unpacking on Linux x86_64 is reviewed")]
    Unsupported,
    #[error(
        "Cache is unsafe, incomplete, changed, unsupported or does not match the pinned archive contract; private partial output may remain if creation had begun"
    )]
    InvalidCache,
    #[error(
        "Output destination must be a NEW private directory; no existing destination is reused"
    )]
    Destination,
    #[error(
        "Extraction not confirmed complete; private partial output may remain. Inspect before manual cleanup; do not retry in place"
    )]
    Incomplete,
}
pub fn unpack_cache(cache: &Path, destination: &Path) -> Result<UnpackedCache, UnpackError> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        linux::unpack(cache, destination)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (cache, destination);
        Err(UnpackError::Unsupported)
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use super::*;
    use crate::{
        inspection::reader::validate_path,
        qoder::{
            distribution,
            staging::{self, linux as fsio},
        },
    };
    use serde::Deserialize;
    use sha2::{Digest, Sha256};
    use std::{
        ffi::{CString, OsStr},
        fs::File,
        io::{self, BufReader, Read, Seek, SeekFrom, Write},
        os::{fd::AsRawFd, unix::fs::MetadataExt},
    };
    const ARCHIVE_BYTES: u64 = 78_587_775;
    const TAR_BYTES: u64 = 182_545_920;
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Complete {
        format: u8,
        owner: String,
        state: String,
        version: String,
        target: String,
        archive_sha256: String,
        bytes: u64,
        installed: bool,
        extracted: bool,
        limitations: String,
    }
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
    fn failed<T>(_: T) -> UnpackError {
        UnpackError::Incomplete
    }
    fn private_file(file: &File, max: u64) -> Result<std::fs::Metadata, UnpackError> {
        let m = file.metadata().map_err(|_| UnpackError::InvalidCache)?;
        if !m.is_file()
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & 0o7777 != 0o600
            || m.nlink() != 1
            || m.len() > max
        {
            return Err(UnpackError::InvalidCache);
        }
        Ok(m)
    }
    fn read_record<T: serde::de::DeserializeOwned>(
        dir: &File,
        child: &str,
    ) -> Result<(T, Vec<u8>), UnpackError> {
        let mut file = fsio::open_at(dir, OsStr::new(child), libc::O_RDONLY | libc::O_NONBLOCK, 0)
            .map_err(|_| UnpackError::InvalidCache)?;
        let before = private_file(&file, 65536)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| UnpackError::InvalidCache)?;
        let after = private_file(&file, 65536)?;
        let named = fsio::open_at(dir, OsStr::new(child), libc::O_RDONLY | libc::O_NONBLOCK, 0)
            .map_err(|_| UnpackError::InvalidCache)?;
        if bytes.len() as u64 != before.len()
            || after.len() != before.len()
            || after.modified().ok() != before.modified().ok()
            || fsio::identity(&file).map_err(failed)? != fsio::identity(&named).map_err(failed)?
        {
            return Err(UnpackError::InvalidCache);
        }
        let value = serde_json::from_slice(&bytes).map_err(|_| UnpackError::InvalidCache)?;
        Ok((value, bytes))
    }
    fn validate_records(dir: &File) -> Result<(Vec<u8>, Vec<u8>), UnpackError> {
        let (c, cb) = read_record::<Complete>(dir, "complete.json")?;
        let (i, ib) = read_record::<Intent>(dir, "incomplete.json")?;
        let p = distribution::plan(staging::TARGET).map_err(failed)?;
        if c.format != 1
            || c.owner != "agentdeck"
            || c.state != "verified_archive_staged_not_installed"
            || c.version != distribution::VERSION
            || c.target != staging::TARGET
            || c.archive_sha256 != p.sha256
            || c.bytes != ARCHIVE_BYTES
            || c.installed
            || c.extracted
            || c.limitations != staging::LIMITATIONS
            || i.format != 1
            || i.owner != "agentdeck"
            || i.state != "incomplete_archive_staging"
            || i.version != distribution::VERSION
            || i.target != staging::TARGET
            || i.installed
        {
            return Err(UnpackError::InvalidCache);
        }
        Ok((cb, ib))
    }
    struct HashRead<R> {
        inner: R,
        hash: Sha256,
        count: u64,
        max: u64,
    }
    impl<R: Read> Read for HashRead<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.inner.read(buf)?;
            self.count += n as u64;
            if self.count > self.max {
                return Err(io::Error::other("stream limit"));
            }
            self.hash.update(&buf[..n]);
            Ok(n)
        }
    }
    struct Cap<R> {
        inner: R,
        count: u64,
        max: u64,
        zero_after: u64,
    }
    impl<R: Read> Read for Cap<R> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.inner.read(buf)?;
            for (i, b) in buf[..n].iter().enumerate() {
                if self.count + i as u64 >= self.zero_after && *b != 0 {
                    return Err(invalid());
                }
            }
            self.count += n as u64;
            if self.count > self.max {
                return Err(io::Error::other("expanded limit"));
            }
            Ok(n)
        }
    }
    fn invalid() -> io::Error {
        io::Error::other("unsupported pinned archive shape")
    }
    // Internal parser only. Production always supplies fixed payload/tar sizes;
    // tests use small synthetic streams, never a CLI/env/digest bypass.
    fn extract<R: Read, W: Write>(
        input: R,
        output: &mut W,
        size: u64,
        tar_size: u64,
    ) -> io::Result<(String, String)> {
        let hashed = HashRead {
            inner: input,
            hash: Sha256::new(),
            count: 0,
            max: ARCHIVE_BYTES,
        };
        let gz = flate2::bufread::GzDecoder::new(BufReader::new(hashed));
        let capped = Cap {
            inner: gz,
            count: 0,
            max: tar_size,
            zero_after: 1024 + size,
        };
        let mut archive = tar::Archive::new(capped);
        let mut payload = Sha256::new();
        {
            let mut entries = archive.entries()?.raw(true);
            for index in 0..2 {
                let mut entry = entries.next().ok_or_else(invalid)??;
                let h = entry.header();
                let raw = h.as_bytes();
                let name = if index == 0 {
                    b"./".as_slice()
                } else {
                    b"./qodercli".as_slice()
                };
                if &raw[..name.len()] != name
                    || raw[name.len()..100].iter().any(|b| *b != 0)
                    || &raw[257..265] != b"ustar\x0000"
                    || raw[345..500].iter().any(|b| *b != 0)
                    || raw[157..257].iter().any(|b| *b != 0)
                    || h.entry_type().as_byte() != if index == 0 { b'5' } else { b'0' }
                    || h.size()? != if index == 0 { 0 } else { size }
                {
                    return Err(invalid());
                }
                if index == 1 {
                    let mut count = 0u64;
                    let mut buf = [0u8; 65536];
                    loop {
                        let n = entry.read(&mut buf)?;
                        if n == 0 {
                            break;
                        }
                        count += n as u64;
                        if count > size {
                            return Err(invalid());
                        }
                        payload.update(&buf[..n]);
                        output.write_all(&buf[..n])?;
                    }
                    if count != size {
                        return Err(invalid());
                    }
                }
            }
            if entries.next().is_some() {
                return Err(invalid());
            }
        }
        // tar stops after the first zero header. Require a second zero block and
        // only zero padding through the exact expanded length, reading gzip EOF.
        let mut capped = archive.into_inner();
        let mut tail = 0u64;
        let mut buf = [0u8; 4096];
        loop {
            let n = capped.read(&mut buf)?;
            if n == 0 {
                break;
            }
            if buf[..n].iter().any(|b| *b != 0) {
                return Err(invalid());
            }
            tail += n as u64;
        }
        if tail < 512 || capped.count != tar_size {
            return Err(invalid());
        }
        let mut compressed = capped.inner.into_inner();
        let mut extra = [0u8; 1];
        if compressed.read(&mut extra)? != 0 {
            return Err(invalid());
        }
        let hashed = compressed.into_inner();
        Ok((
            format!("{:x}", payload.finalize()),
            format!("{:x}", hashed.hash.finalize()),
        ))
    }
    pub(super) fn unpack(cache: &Path, destination: &Path) -> Result<UnpackedCache, UnpackError> {
        let leaf = cache
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or(UnpackError::InvalidCache)?;
        validate_path(cache, leaf).map_err(|_| UnpackError::InvalidCache)?;
        let cache_dir = fsio::directory(cache).map_err(|_| UnpackError::InvalidCache)?;
        let cm = cache_dir
            .metadata()
            .map_err(|_| UnpackError::InvalidCache)?;
        if cm.uid() != unsafe { libc::geteuid() } || cm.mode() & 0o7777 != 0o700 {
            return Err(UnpackError::InvalidCache);
        }
        let records = validate_records(&cache_dir)?;
        let mut source = fsio::open_at(
            &cache_dir,
            OsStr::new("archive.tar.gz"),
            libc::O_RDONLY | libc::O_NONBLOCK,
            0,
        )
        .map_err(|_| UnpackError::InvalidCache)?;
        let before = private_file(&source, ARCHIVE_BYTES)?;
        if before.len() != ARCHIVE_BYTES {
            return Err(UnpackError::InvalidCache);
        }
        let leaf = destination
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or(UnpackError::Destination)?;
        validate_path(destination, leaf).map_err(|_| UnpackError::Destination)?;
        let parent_path = destination.parent().ok_or(UnpackError::Destination)?;
        let parent = fsio::directory(parent_path).map_err(|_| UnpackError::Destination)?;
        let child = fsio::name(OsStr::new(leaf)).map_err(|_| UnpackError::Destination)?;
        if unsafe { libc::mkdirat(parent.as_raw_fd(), child.as_ptr(), 0o700) } != 0 {
            return Err(UnpackError::Destination);
        }
        let dir = fsio::open_at(
            &parent,
            OsStr::new(leaf),
            libc::O_RDONLY | libc::O_DIRECTORY,
            0,
        )
        .map_err(failed)?;
        let dm = dir.metadata().map_err(failed)?;
        if dm.uid() != unsafe { libc::geteuid() } || dm.mode() & 0o7777 != 0o700 {
            return Err(UnpackError::Incomplete);
        }
        fsio::record(&dir,"incomplete.json",&serde_json::json!({"format":1,"owner":"agentdeck","state":"incomplete_archive_unpack","version":distribution::VERSION,"target":staging::TARGET,"installed":false})).map_err(failed)?;
        let mut copy = fsio::new_file(&dir, "archive.tar.gz").map_err(failed)?;
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut buf = [0u8; 65536];
        loop {
            let n = source.read(&mut buf).map_err(failed)?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            if bytes > ARCHIVE_BYTES {
                return Err(UnpackError::Incomplete);
            }
            copy.write_all(&buf[..n]).map_err(failed)?;
            hash.update(&buf[..n]);
        }
        let plan = distribution::plan(staging::TARGET).map_err(failed)?;
        let after = private_file(&source, ARCHIVE_BYTES)?;
        if bytes != ARCHIVE_BYTES
            || format!("{:x}", hash.finalize()) != plan.sha256
            || before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
        {
            return Err(UnpackError::InvalidCache);
        }
        let named = fsio::open_at(
            &cache_dir,
            OsStr::new("archive.tar.gz"),
            libc::O_RDONLY | libc::O_NONBLOCK,
            0,
        )
        .map_err(failed)?;
        if fsio::identity(&named).map_err(failed)? != fsio::identity(&source).map_err(failed)?
            || fsio::identity(&fsio::directory(cache).map_err(failed)?).map_err(failed)?
                != fsio::identity(&cache_dir).map_err(failed)?
            || validate_records(&cache_dir)? != records
        {
            return Err(UnpackError::InvalidCache);
        }
        copy.sync_all().map_err(failed)?;
        copy.seek(SeekFrom::Start(0)).map_err(failed)?;
        let mut payload = fsio::new_file(&dir, "qodercli").map_err(failed)?;
        let (payload_hash, compressed_hash) =
            extract(&mut copy, &mut payload, PAYLOAD_BYTES, TAR_BYTES).map_err(failed)?;
        if payload_hash != PAYLOAD_SHA256 || compressed_hash != plan.sha256 {
            return Err(UnpackError::Incomplete);
        }
        payload.sync_all().map_err(failed)?;
        let pm = private_file(&payload, PAYLOAD_BYTES)?;
        if pm.len() != PAYLOAD_BYTES {
            return Err(UnpackError::Incomplete);
        }
        payload.seek(SeekFrom::Start(0)).map_err(failed)?;
        let mut written_hash = Sha256::new();
        let mut written_bytes = 0u64;
        loop {
            let n = payload.read(&mut buf).map_err(failed)?;
            if n == 0 {
                break;
            }
            written_bytes += n as u64;
            if written_bytes > PAYLOAD_BYTES {
                return Err(UnpackError::Incomplete);
            }
            written_hash.update(&buf[..n]);
        }
        if written_bytes != PAYLOAD_BYTES
            || format!("{:x}", written_hash.finalize()) != PAYLOAD_SHA256
        {
            return Err(UnpackError::Incomplete);
        }
        private_file(&copy, ARCHIVE_BYTES)?;
        for (name, file) in [("qodercli", &payload), ("archive.tar.gz", &copy)] {
            let named = fsio::open_at(&dir, OsStr::new(name), libc::O_RDONLY | libc::O_NONBLOCK, 0)
                .map_err(failed)?;
            if fsio::identity(&named).map_err(failed)? != fsio::identity(file).map_err(failed)? {
                return Err(UnpackError::Incomplete);
            }
        }
        if fsio::identity(&fsio::directory(destination).map_err(failed)?).map_err(failed)?
            != fsio::identity(&dir).map_err(failed)?
            || fsio::identity(&fsio::directory(parent_path).map_err(failed)?).map_err(failed)?
                != fsio::identity(&parent).map_err(failed)?
        {
            return Err(UnpackError::Incomplete);
        }
        fsio::record(&dir,"completion.pending",&serde_json::json!({"format":1,"owner":"agentdeck","state":"verified_payload_unpacked_not_installed","version":distribution::VERSION,"target":staging::TARGET,"archive_sha256":plan.sha256,"payload_sha256":PAYLOAD_SHA256,"payload_bytes":PAYLOAD_BYTES,"installed":false,"executable":false,"limitations":LIMITATIONS})).map_err(failed)?;
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
            return Err(UnpackError::Incomplete);
        }
        dir.sync_all().map_err(failed)?;
        parent.sync_all().map_err(failed)?;
        Ok(UnpackedCache {
            state: "verified_payload_unpacked_not_installed",
            installed: false,
            executable: false,
            payload_bytes: PAYLOAD_BYTES,
            payload_sha256: PAYLOAD_SHA256,
            limitations: LIMITATIONS,
        })
    }
    #[cfg(test)]
    mod tests;
}
