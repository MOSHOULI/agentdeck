use super::*;
use crate::{
    inspection::reader::validate_path,
    qoder::{
        cache_inspection, distribution,
        staging::{self, linux as fsio},
        unpacking,
    },
};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
};
type Result<T> = std::result::Result<T, DeploymentError>;
fn invalid<T>(_: T) -> DeploymentError {
    DeploymentError::Invalid
}
fn incomplete<T>(_: T) -> DeploymentError {
    DeploymentError::Incomplete
}
fn snapshot(m: &Metadata) -> (u64, u64, u32, u32, u32, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.uid(),
        m.gid(),
        m.mode(),
        m.nlink(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn directory_identity(f: &File) -> Result<(u64, u64, u32, u32, u32)> {
    let m = f.metadata().map_err(invalid)?;
    if !m.is_dir() || m.uid() != unsafe { libc::geteuid() } || m.mode() & 0o7777 != 0o700 {
        return Err(DeploymentError::Invalid);
    }
    Ok((m.dev(), m.ino(), m.uid(), m.gid(), m.mode()))
}
fn no_xattrs(f: &File) -> Result<()> {
    // Attribute names/values are never read. Refuse labels/ACLs rather than imply preservation.
    if unsafe { libc::flistxattr(f.as_raw_fd(), std::ptr::null_mut(), 0) } != 0 {
        return Err(DeploymentError::Invalid);
    }
    Ok(())
}
fn regular(f: &File, length: u64, mode: u32) -> Result<Metadata> {
    let m = f.metadata().map_err(invalid)?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o7777 != mode
        || m.nlink() != 1
        || m.len() != length
    {
        return Err(DeploymentError::Invalid);
    }
    no_xattrs(f)?;
    Ok(m)
}
fn open_payload(dir: &File) -> Result<File> {
    fsio::open_at(
        dir,
        OsStr::new("qodercli"),
        libc::O_RDONLY | libc::O_NONBLOCK,
        0,
    )
    .map_err(invalid)
}
fn absent(parent: &File, leaf: &OsStr) -> Result<()> {
    let name = fsio::name(leaf).map_err(invalid)?;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
        || std::io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT)
    {
        return Err(DeploymentError::Invalid);
    }
    Ok(())
}
fn records(dir: &File) -> Result<Vec<Vec<u8>>> {
    ["incomplete.json", "complete.json"]
        .into_iter()
        .map(|name| {
            let mut file =
                fsio::open_at(dir, OsStr::new(name), libc::O_RDONLY | libc::O_NONBLOCK, 0)
                    .map_err(invalid)?;
            let before = unpacking::linux::private_file(&file, 65536).map_err(invalid)?;
            let mut bytes = Vec::new();
            (&mut file)
                .take(65537)
                .read_to_end(&mut bytes)
                .map_err(invalid)?;
            if bytes.len() as u64 != before.len() || bytes.len() > 65536 {
                return Err(DeploymentError::Invalid);
            }
            verify_named(dir, name, &file).map_err(invalid)?;
            if snapshot(&before) != snapshot(&file.metadata().map_err(invalid)?) {
                return Err(DeploymentError::Invalid);
            }
            bytes.extend_from_slice(format!("{:?}", snapshot(&before)).as_bytes());
            Ok(bytes)
        })
        .collect()
}
struct Selection {
    cache_dir: File,
    parent: File,
    payload: File,
    metadata: Metadata,
    records: Vec<Vec<u8>>,
    token: String,
}
fn select(
    reviewed: distribution::ReviewedRelease,
    cache: &Path,
    destination: &Path,
) -> Result<Selection> {
    for p in [cache, destination] {
        let leaf = p
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or(DeploymentError::Invalid)?;
        validate_path(p, leaf).map_err(invalid)?;
    }
    let parent =
        fsio::directory(destination.parent().ok_or(DeploymentError::Invalid)?).map_err(invalid)?;
    let parent_id = directory_identity(&parent)?;
    no_xattrs(&parent)?;
    absent(
        &parent,
        destination.file_name().ok_or(DeploymentError::Invalid)?,
    )?;
    let cache_dir = fsio::directory(cache).map_err(invalid)?;
    let cache_id = directory_identity(&cache_dir)?;
    let payload = open_payload(&cache_dir)?;
    let metadata = regular(&payload, reviewed.payload_bytes, 0o600)?;
    let bytes = records(&cache_dir)?;
    // An unsigned record is never sufficient: production inspector independently
    // validates exact record fields, members, full archive AND payload hashes.
    if cache_inspection::inspect_cache_for_release(reviewed.version, cache)
        .map_err(invalid)?
        .state
        != cache_inspection::CacheState::UnpackedPayloadVerified
    {
        return Err(DeploymentError::Invalid);
    }
    if cache_id != directory_identity(&fsio::directory(cache).map_err(invalid)?)?
        || snapshot(&metadata) != snapshot(&regular(&payload, reviewed.payload_bytes, 0o600)?)
        || snapshot(&metadata)
            != snapshot(&regular(
                &open_payload(&cache_dir)?,
                reviewed.payload_bytes,
                0o600,
            )?)
        || bytes != records(&cache_dir)?
    {
        return Err(DeploymentError::Invalid);
    }
    if parent_id
        != directory_identity(&fsio::directory(destination.parent().unwrap()).map_err(invalid)?)?
    {
        return Err(DeploymentError::Invalid);
    }
    let mut hash = Sha256::new();
    hash.update(b"agentdeck/qoder/private-deployment/1\0");
    for value in [
        cache.as_os_str().as_bytes(),
        destination.as_os_str().as_bytes(),
        reviewed.version.as_bytes(),
        staging::TARGET.as_bytes(),
        reviewed.payload_sha256.as_bytes(),
    ] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value);
    }
    hash.update(format!("{:?}{:?}{:?}", cache_id, parent_id, snapshot(&metadata)).as_bytes());
    for record in &bytes {
        hash.update((record.len() as u64).to_le_bytes());
        hash.update(record);
    }
    Ok(Selection {
        cache_dir,
        parent,
        payload,
        metadata,
        records: bytes,
        token: format!("{:x}", hash.finalize()),
    })
}
pub(super) fn preview(
    reviewed: distribution::ReviewedRelease,
    cache: &Path,
    destination: &Path,
) -> Result<DeploymentPreview> {
    let selected = select(reviewed, cache, destination)?;
    Ok(DeploymentPreview {
        version: reviewed.version,
        target: staging::TARGET,
        payload_bytes: reviewed.payload_bytes,
        payload_sha256: reviewed.payload_sha256,
        executable_mode: "0700",
        token: selected.token,
        limitations: LIMITATIONS,
    })
}
fn copy_verified(
    source: &mut File,
    output: &mut File,
    length: u64,
    expected: &str,
) -> Result<Metadata> {
    source.seek(SeekFrom::Start(0)).map_err(incomplete)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = source.read(&mut buffer).map_err(incomplete)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > length {
            return Err(DeploymentError::Incomplete);
        }
        output.write_all(&buffer[..n]).map_err(incomplete)?;
        hash.update(&buffer[..n]);
    }
    if total != length || format!("{:x}", hash.finalize()) != expected {
        return Err(DeploymentError::Incomplete);
    }
    output.sync_all().map_err(incomplete)?;
    let before_read = regular(output, length, 0o600).map_err(incomplete)?;
    output.seek(SeekFrom::Start(0)).map_err(incomplete)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    loop {
        let n = output.read(&mut buffer).map_err(incomplete)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > length {
            return Err(DeploymentError::Incomplete);
        }
        hash.update(&buffer[..n]);
    }
    if total != length || format!("{:x}", hash.finalize()) != expected {
        return Err(DeploymentError::Incomplete);
    }
    let after_read = regular(output, length, 0o600).map_err(incomplete)?;
    if snapshot(&before_read) != snapshot(&after_read) {
        return Err(DeploymentError::Incomplete);
    }
    Ok(after_read)
}
fn verify_named(dir: &File, name: &str, file: &File) -> Result<()> {
    let named = fsio::open_at(dir, OsStr::new(name), libc::O_RDONLY | libc::O_NONBLOCK, 0)
        .map_err(incomplete)?;
    if snapshot(&named.metadata().map_err(incomplete)?)
        != snapshot(&file.metadata().map_err(incomplete)?)
    {
        return Err(DeploymentError::Incomplete);
    }
    Ok(())
}
pub(super) fn deploy(
    reviewed: distribution::ReviewedRelease,
    cache: &Path,
    destination: &Path,
    token: &str,
) -> Result<DeploymentOutcome> {
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(DeploymentError::Changed);
    }
    let selected = select(reviewed, cache, destination)?;
    if selected.token != token {
        return Err(DeploymentError::Changed);
    }
    execute(
        selected,
        cache,
        destination,
        &Contract::pinned(reviewed),
        |_| Ok(()),
    )
}
// Private execution stage shared with fault-injection tests. Production always
// constructs the fixed contract; no caller-selected digest/size API exists.
struct Contract {
    version: &'static str,
    bytes: u64,
    sha256: String,
    #[cfg(test)]
    authored_fixture: bool,
}
impl Contract {
    fn pinned(reviewed: distribution::ReviewedRelease) -> Self {
        Self {
            version: reviewed.version,
            bytes: reviewed.payload_bytes,
            sha256: reviewed.payload_sha256.into(),
            #[cfg(test)]
            authored_fixture: false,
        }
    }
    fn verify_cache(&self, cache: &Path) -> Result<()> {
        #[cfg(test)]
        if self.authored_fixture {
            return Ok(());
        }
        if cache_inspection::inspect_cache_for_release(self.version, cache)
            .map_err(incomplete)?
            .state
            != cache_inspection::CacheState::UnpackedPayloadVerified
        {
            return Err(DeploymentError::Incomplete);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    AfterCopy,
    AfterModeChange,
    BeforePublication,
    AfterPublication,
}
fn execute(
    mut selected: Selection,
    cache: &Path,
    destination: &Path,
    contract: &Contract,
    mut hook: impl FnMut(Stage) -> Result<()>,
) -> Result<DeploymentOutcome> {
    let leaf = destination.file_name().ok_or(DeploymentError::Invalid)?;
    let child = fsio::name(leaf).map_err(invalid)?;
    if unsafe { libc::mkdirat(selected.parent.as_raw_fd(), child.as_ptr(), 0o700) } != 0 {
        return Err(DeploymentError::Invalid);
    }
    let dir = fsio::open_at(
        &selected.parent,
        leaf,
        libc::O_RDONLY | libc::O_DIRECTORY,
        0,
    )
    .map_err(incomplete)?;
    directory_identity(&dir).map_err(incomplete)?;
    no_xattrs(&dir).map_err(incomplete)?;
    fsio::record(&dir,"deployment.intent.json",&serde_json::json!({"format":1,"state":"private_deployment_started","version":contract.version,"target":staging::TARGET,"execution_verified":false,"activated":false})).map_err(incomplete)?;
    let mut output = fsio::new_file(&dir, "qodercli").map_err(incomplete)?;
    let output_before = copy_verified(
        &mut selected.payload,
        &mut output,
        contract.bytes,
        &contract.sha256,
    )?;
    hook(Stage::AfterCopy)?;
    if snapshot(&selected.metadata)
        != snapshot(&regular(&selected.payload, contract.bytes, 0o600).map_err(incomplete)?)
        || selected.records != records(&selected.cache_dir).map_err(incomplete)?
        || directory_identity(&selected.cache_dir).map_err(incomplete)?
            != directory_identity(&fsio::directory(cache).map_err(incomplete)?)
                .map_err(incomplete)?
    {
        return Err(DeploymentError::Incomplete);
    }
    contract.verify_cache(cache)?;
    if selected.records != records(&selected.cache_dir).map_err(incomplete)?
        || snapshot(&selected.metadata)
            != snapshot(&regular(&selected.payload, contract.bytes, 0o600).map_err(incomplete)?)
        || snapshot(&output_before)
            != snapshot(&regular(&output, contract.bytes, 0o600).map_err(incomplete)?)
    {
        return Err(DeploymentError::Incomplete);
    }
    verify_named(&selected.cache_dir, "qodercli", &selected.payload)?;
    verify_named(&dir, "qodercli", &output)?;
    if directory_identity(&dir).map_err(incomplete)?
        != directory_identity(&fsio::directory(destination).map_err(incomplete)?)
            .map_err(incomplete)?
        || directory_identity(&selected.parent).map_err(incomplete)?
            != directory_identity(
                &fsio::directory(destination.parent().unwrap()).map_err(incomplete)?,
            )
            .map_err(incomplete)?
    {
        return Err(DeploymentError::Incomplete);
    }
    no_xattrs(&dir).map_err(incomplete)?;
    no_xattrs(&selected.parent).map_err(incomplete)?;
    // Only our new, hash-checked, held output handle receives execute bits.
    // From this point EVERY failure may leave executable partial output.
    if unsafe { libc::fchmod(output.as_raw_fd(), 0o700) } != 0 {
        return Err(DeploymentError::Incomplete);
    }
    let executable_baseline = regular(&output, contract.bytes, 0o700).map_err(incomplete)?;
    // chmod may change ctime/mode, not payload identity, size or mtime.
    if output_before.dev() != executable_baseline.dev()
        || output_before.ino() != executable_baseline.ino()
        || output_before.uid() != executable_baseline.uid()
        || output_before.gid() != executable_baseline.gid()
        || output_before.mtime() != executable_baseline.mtime()
        || output_before.mtime_nsec() != executable_baseline.mtime_nsec()
    {
        return Err(DeploymentError::Incomplete);
    }
    hook(Stage::AfterModeChange)?;
    unchanged_output(&output, &executable_baseline, contract.bytes)?;
    verify_named(&dir, "qodercli", &output)?;
    output.sync_all().map_err(incomplete)?;
    fsio::record(&dir,"deployment.pending.json",&serde_json::json!({"format":1,"state":"private_executable_deployed_not_activated","version":contract.version,"target":staging::TARGET,"payload_bytes":contract.bytes,"payload_sha256":&contract.sha256,"executable_mode":"0700","execution_verified":false,"activated":false,"limitations":LIMITATIONS})).map_err(incomplete)?;
    hook(Stage::BeforePublication)?;
    unchanged_output(&output, &executable_baseline, contract.bytes)?;
    verify_named(&dir, "qodercli", &output)?;
    if directory_identity(&dir).map_err(incomplete)?
        != directory_identity(&fsio::directory(destination).map_err(incomplete)?)
            .map_err(incomplete)?
    {
        return Err(DeploymentError::Incomplete);
    }
    let from = fsio::name(OsStr::new("deployment.pending.json")).map_err(incomplete)?;
    let to = fsio::name(OsStr::new("deployment.complete.json")).map_err(incomplete)?;
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
        return Err(DeploymentError::Incomplete);
    }
    dir.sync_all().map_err(incomplete)?;
    selected.parent.sync_all().map_err(incomplete)?;
    hook(Stage::AfterPublication)?;
    unchanged_output(&output, &executable_baseline, contract.bytes)?;
    verify_named(&dir, "qodercli", &output)?;
    if directory_identity(&dir).map_err(incomplete)?
        != directory_identity(&fsio::directory(destination).map_err(incomplete)?)
            .map_err(incomplete)?
    {
        return Err(DeploymentError::Incomplete);
    }
    Ok(DeploymentOutcome {
        state: "private_executable_deployed_not_activated",
        version: contract.version,
        target: staging::TARGET,
        executable_mode: "0700",
        execution_verified: false,
        activated: false,
        limitations: LIMITATIONS,
    })
}
fn unchanged_output(file: &File, baseline: &Metadata, bytes: u64) -> Result<()> {
    if snapshot(baseline) != snapshot(&regular(file, bytes, 0o700).map_err(incomplete)?) {
        return Err(DeploymentError::Incomplete);
    }
    Ok(())
}
#[cfg(test)]
mod tests;
