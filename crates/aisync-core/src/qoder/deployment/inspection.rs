//! Revalidate existing private deployment without the cache or any mutation.
use super::*;
use DeploymentInspectionState as State;
use std::{collections::BTreeSet, ffi::CStr, os::fd::IntoRawFd};

struct Dir(*mut libc::DIR);
impl Drop for Dir {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}
fn entries(dir: &File) -> Result<BTreeSet<String>> {
    let fd = fsio::open_at(dir, OsStr::new("."), libc::O_RDONLY | libc::O_DIRECTORY, 0)
        .map_err(invalid)?
        .into_raw_fd();
    let raw = unsafe { libc::fdopendir(fd) };
    if raw.is_null() {
        unsafe {
            libc::close(fd);
        }
        return Err(DeploymentError::Invalid);
    }
    let stream = Dir(raw);
    let mut result = BTreeSet::new();
    loop {
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            return if unsafe { *libc::__errno_location() } == 0 {
                Ok(result)
            } else {
                Err(DeploymentError::Invalid)
            };
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
            .to_str()
            .map_err(invalid)?;
        if name == "." || name == ".." {
            continue;
        }
        if result.len() >= 4 || !result.insert(name.into()) {
            return Err(DeploymentError::Invalid);
        }
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    format: u64,
    state: String,
    version: String,
    target: String,
    execution_verified: bool,
    activated: bool,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Complete {
    format: u64,
    state: String,
    version: String,
    target: String,
    payload_bytes: u64,
    payload_sha256: String,
    executable_mode: String,
    execution_verified: bool,
    activated: bool,
    limitations: String,
}
fn read_record<T: serde::de::DeserializeOwned>(
    dir: &File,
    name: &str,
) -> Result<(T, File, Metadata)> {
    let mut file = fsio::open_at(dir, OsStr::new(name), libc::O_RDONLY | libc::O_NONBLOCK, 0)
        .map_err(invalid)?;
    let before = unpacking::linux::private_file(&file, 65536).map_err(invalid)?;
    no_xattrs(&file)?;
    let mut bytes = Vec::new();
    (&mut file)
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    if bytes.len() as u64 != before.len() {
        return Err(DeploymentError::Invalid);
    }
    let parsed = serde_json::from_slice(&bytes).map_err(invalid)?;
    Ok((parsed, file, before))
}
fn verify_record(dir: &File, name: &str, file: &File, before: &Metadata) -> Result<()> {
    if snapshot(before) != snapshot(&regular(file, before.len(), 0o600)?) {
        return Err(DeploymentError::Invalid);
    }
    verify_named(dir, name, file).map_err(invalid)
}
fn valid_intent(value: &Intent, contract: &Contract) -> bool {
    value.format == 1
        && value.state == "private_deployment_started"
        && value.version == contract.version
        && value.target == staging::TARGET
        && !value.execution_verified
        && !value.activated
}
fn valid_complete(value: &Complete, contract: &Contract) -> bool {
    value.format == 1
        && value.state == "private_executable_deployed_not_activated"
        && value.version == contract.version
        && value.target == staging::TARGET
        && value.payload_bytes == contract.bytes
        && value.payload_sha256 == contract.sha256
        && value.executable_mode == "0700"
        && !value.execution_verified
        && !value.activated
        && value.limitations == LIMITATIONS
}
fn incomplete_members(dir: &File, inventory: &BTreeSet<String>, contract: &Contract) -> Result<()> {
    for name in inventory {
        if name == "qodercli" {
            let payload = open_payload(dir)?;
            let m = payload.metadata().map_err(invalid)?;
            let mode = m.mode() & 0o7777;
            if m.len() > contract.bytes || (mode != 0o600 && mode != 0o700) {
                return Err(DeploymentError::Invalid);
            }
            regular(&payload, m.len(), mode)?;
            verify_named(dir, name, &payload).map_err(invalid)?;
        } else if name == "deployment.intent.json" {
            let (value, file, meta) = read_record::<Intent>(dir, name)?;
            if !valid_intent(&value, contract) {
                return Err(DeploymentError::Invalid);
            }
            verify_record(dir, name, &file, &meta)?;
        } else {
            let (value, file, meta) = read_record::<Complete>(dir, name)?;
            if !valid_complete(&value, contract) {
                return Err(DeploymentError::Invalid);
            }
            verify_record(dir, name, &file, &meta)?;
        }
    }
    Ok(())
}
fn inspect_inner(path: &Path, contract: &Contract) -> Result<State> {
    let parent_path = path.parent().ok_or(DeploymentError::Invalid)?;
    let parent = fsio::directory(parent_path).map_err(invalid)?;
    let parent_before = directory_identity(&parent)?;
    no_xattrs(&parent)?;
    let leaf = path.file_name().ok_or(DeploymentError::Invalid)?;
    if absent(&parent, leaf).is_ok() {
        let named_parent = fsio::directory(parent_path).map_err(invalid)?;
        if parent_before != directory_identity(&named_parent)? {
            return Err(DeploymentError::Invalid);
        }
        no_xattrs(&named_parent)?;
        absent(&named_parent, leaf)?;
        return Ok(State::Missing);
    }
    let dir =
        fsio::open_at(&parent, leaf, libc::O_RDONLY | libc::O_DIRECTORY, 0).map_err(invalid)?;
    directory_identity(&dir)?;
    no_xattrs(&dir)?;
    let before = dir.metadata().map_err(invalid)?;
    let inventory = entries(&dir)?;
    let required: BTreeSet<_> = [
        "qodercli",
        "deployment.intent.json",
        "deployment.complete.json",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if inventory != required {
        let allowed: BTreeSet<_> = required
            .iter()
            .cloned()
            .chain(["deployment.pending.json".into()])
            .collect();
        if !inventory.is_subset(&allowed) {
            return Ok(State::UnsupportedOrChanged);
        }
        incomplete_members(&dir, &inventory, contract)?;
        if inventory != entries(&dir)?
            || snapshot(&before) != snapshot(&dir.metadata().map_err(invalid)?)
            || snapshot(&before)
                != snapshot(
                    &fsio::directory(path)
                        .map_err(invalid)?
                        .metadata()
                        .map_err(invalid)?,
                )
            || parent_before != directory_identity(&fsio::directory(parent_path).map_err(invalid)?)?
        {
            return Err(DeploymentError::Invalid);
        }
        no_xattrs(&dir)?;
        no_xattrs(&parent)?;
        return Ok(State::Incomplete);
    }
    let (intent, intent_file, intent_meta) = read_record::<Intent>(&dir, "deployment.intent.json")?;
    let (complete, complete_file, complete_meta) =
        read_record::<Complete>(&dir, "deployment.complete.json")?;
    if !valid_intent(&intent, contract) || !valid_complete(&complete, contract) {
        return Err(DeploymentError::Invalid);
    }
    let mut payload = open_payload(&dir)?;
    let payload_before = regular(&payload, contract.bytes, 0o700)?;
    let mut hash = Sha256::new();
    let mut read = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let n = payload.read(&mut buffer).map_err(invalid)?;
        if n == 0 {
            break;
        }
        read += n as u64;
        if read > contract.bytes {
            return Err(DeploymentError::Invalid);
        }
        hash.update(&buffer[..n]);
    }
    if read != contract.bytes
        || format!("{:x}", hash.finalize()) != contract.sha256
        || snapshot(&payload_before) != snapshot(&regular(&payload, contract.bytes, 0o700)?)
    {
        return Err(DeploymentError::Invalid);
    }
    verify_named(&dir, "qodercli", &payload).map_err(invalid)?;
    verify_record(&dir, "deployment.intent.json", &intent_file, &intent_meta)?;
    verify_record(
        &dir,
        "deployment.complete.json",
        &complete_file,
        &complete_meta,
    )?;
    if inventory != entries(&dir)?
        || snapshot(&before) != snapshot(&dir.metadata().map_err(invalid)?)
        || snapshot(&before)
            != snapshot(
                &fsio::directory(path)
                    .map_err(invalid)?
                    .metadata()
                    .map_err(invalid)?,
            )
        || parent_before != directory_identity(&fsio::directory(parent_path).map_err(invalid)?)?
    {
        return Err(DeploymentError::Invalid);
    }
    no_xattrs(&dir)?;
    no_xattrs(&parent)?;
    Ok(State::PayloadVerified)
}
pub(super) fn inspect(
    reviewed: distribution::ReviewedRelease,
    path: &Path,
) -> std::result::Result<State, DeploymentInspectionError> {
    let leaf = path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or(DeploymentInspectionError::Unavailable)?;
    validate_path(path, leaf).map_err(|_| DeploymentInspectionError::Unavailable)?;
    Ok(inspect_inner(path, &Contract::pinned(reviewed)).unwrap_or(State::UnsupportedOrChanged))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("qoder-inspection-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture() -> (Temp, std::path::PathBuf, Contract) {
        let root = Temp::new();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.path().join("deployment");
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let contract = Contract {
            version: "1.1.85",
            bytes: 8,
            sha256: format!("{:x}", Sha256::digest(b"authored")),
            authored_fixture: true,
        };
        let dir = fsio::directory(&path).unwrap();
        let mut payload = fsio::new_file(&dir, "qodercli").unwrap();
        payload.write_all(b"authored").unwrap();
        payload
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .unwrap();
        fsio::record(&dir,"deployment.intent.json",&serde_json::json!({"format":1,"state":"private_deployment_started","version":contract.version,"target":staging::TARGET,"execution_verified":false,"activated":false})).unwrap();
        fsio::record(&dir,"deployment.complete.json",&serde_json::json!({"format":1,"state":"private_executable_deployed_not_activated","version":contract.version,"target":staging::TARGET,"payload_bytes":contract.bytes,"payload_sha256":contract.sha256,"executable_mode":"0700","execution_verified":false,"activated":false,"limitations":LIMITATIONS})).unwrap();
        (root, path, contract)
    }
    type MetadataSnapshot = (u64, u64, u32, u32, u32, u64, u64, i64, i64, i64, i64);
    type FileImage = (String, Vec<u8>, MetadataSnapshot);
    fn image(path: &Path) -> Vec<FileImage> {
        let mut result = Vec::new();
        for name in [
            "qodercli",
            "deployment.intent.json",
            "deployment.complete.json",
        ] {
            let p = path.join(name);
            result.push((
                name.into(),
                std::fs::read(&p).unwrap(),
                snapshot(&std::fs::metadata(p).unwrap()),
            ));
        }
        result
    }
    #[test]
    fn repeat_without_cache_retains_bytes_and_metadata() {
        let (_root, path, c) = fixture();
        let before = image(&path);
        for _ in 0..2 {
            assert_eq!(inspect_inner(&path, &c).unwrap(), State::PayloadVerified);
        }
        assert_eq!(before, image(&path));
        // The production API never accepts this authored payload or its self-authored receipt.
        assert_eq!(
            super::inspect(distribution::reviewed_release("1.1.85").unwrap(), &path).unwrap(),
            State::UnsupportedOrChanged
        );
        assert_eq!(before, image(&path));
    }
    #[test]
    fn missing_pending_extra_and_wrong_release_are_not_verified() {
        let (root, path, c) = fixture();
        assert_eq!(
            inspect_inner(&root.path().join("missing"), &c).unwrap(),
            State::Missing
        );
        std::fs::copy(
            path.join("deployment.complete.json"),
            path.join("deployment.pending.json"),
        )
        .unwrap();
        assert_eq!(inspect_inner(&path, &c).unwrap(), State::Incomplete);
        std::fs::remove_file(path.join("deployment.pending.json")).unwrap();
        std::fs::write(path.join("extra"), b"retained").unwrap();
        assert_eq!(
            inspect_inner(&path, &c).unwrap(),
            State::UnsupportedOrChanged
        );
        std::fs::remove_file(path.join("extra")).unwrap();
        let changed = Contract {
            version: "1.1.86",
            ..c
        };
        assert!(inspect_inner(&path, &changed).is_err());
    }
    #[test]
    fn wrong_hash_permissions_links_and_records_are_retained() {
        for case in [
            "hash",
            "mode",
            "hardlink",
            "symlink",
            "record",
            "duplicate",
            "unknown",
            "xattr",
        ] {
            let (root, path, c) = fixture();
            let payload = path.join("qodercli");
            match case {
                "hash" => std::fs::write(&payload, b"changed!").unwrap(),
                "mode" => {
                    std::fs::set_permissions(&payload, std::fs::Permissions::from_mode(0o755))
                        .unwrap()
                }
                "hardlink" => std::fs::hard_link(&payload, root.path().join("alias")).unwrap(),
                "symlink" => {
                    std::fs::rename(&payload, root.path().join("original")).unwrap();
                    symlink(root.path().join("original"), &payload).unwrap();
                }
                "record" => {
                    let p = path.join("deployment.complete.json");
                    let mut v: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
                    v["activated"] = true.into();
                    std::fs::write(p, serde_json::to_vec(&v).unwrap()).unwrap();
                }
                "duplicate" => {
                    let p = path.join("deployment.intent.json");
                    let bytes =
                        std::fs::read_to_string(&p)
                            .unwrap()
                            .replacen('{', "{\"format\":1,", 1);
                    std::fs::write(p, bytes).unwrap();
                }
                "unknown" => {
                    let p = path.join("deployment.intent.json");
                    let bytes = std::fs::read_to_string(&p).unwrap().replacen(
                        '{',
                        "{\"authority\":true,",
                        1,
                    );
                    std::fs::write(p, bytes).unwrap();
                }
                "xattr" => {
                    let file = open_payload(&fsio::directory(&path).unwrap()).unwrap();
                    let name = c"user.agentdeck-inspection-test";
                    assert_eq!(
                        unsafe {
                            libc::fsetxattr(
                                file.as_raw_fd(),
                                name.as_ptr(),
                                b"x".as_ptr().cast(),
                                1,
                                0,
                            )
                        },
                        0
                    );
                }
                _ => unreachable!(),
            }
            let before = image(&path);
            assert!(inspect_inner(&path, &c).is_err(), "{case}");
            assert_eq!(before, image(&path), "{case}");
        }
    }
    #[test]
    fn incomplete_structure_does_not_mask_unsafe_members() {
        for case in ["symlink", "mode", "record", "pending"] {
            let (root, path, c) = fixture();
            std::fs::remove_file(path.join("deployment.complete.json")).unwrap();
            match case {
                "symlink" => {
                    let payload = path.join("qodercli");
                    std::fs::rename(&payload, root.path().join("outside")).unwrap();
                    symlink(root.path().join("outside"), payload).unwrap();
                }
                "mode" => std::fs::set_permissions(
                    path.join("qodercli"),
                    std::fs::Permissions::from_mode(0o755),
                )
                .unwrap(),
                "record" => std::fs::write(path.join("deployment.intent.json"), b"{}").unwrap(),
                "pending" => std::fs::write(path.join("deployment.pending.json"), b"{}").unwrap(),
                _ => unreachable!(),
            }
            assert!(inspect_inner(&path, &c).is_err(), "{case}");
        }
    }

    #[test]
    fn empty_or_valid_partial_structure_is_incomplete_not_verified() {
        let (_root, path, c) = fixture();
        std::fs::remove_file(path.join("deployment.complete.json")).unwrap();
        assert_eq!(inspect_inner(&path, &c).unwrap(), State::Incomplete);
        std::fs::remove_file(path.join("deployment.intent.json")).unwrap();
        std::fs::remove_file(path.join("qodercli")).unwrap();
        assert_eq!(inspect_inner(&path, &c).unwrap(), State::Incomplete);
    }

    #[test]
    fn directory_links_and_modes_are_not_accepted() {
        let (root, path, c) = fixture();
        let link = root.path().join("linked");
        symlink(&path, &link).unwrap();
        assert!(inspect_inner(&link, &c).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o750)).unwrap();
        assert!(inspect_inner(&path, &c).is_err());
    }
}
