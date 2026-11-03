//! Signed registry updates. All payloads are validated before touching the installed
//! directory; files and version metadata are committed as one directory transaction.
use crate::{Error, registry::DataPaths, release::ReleaseManifest};
use std::io::Read;

pub const REGISTRY_FILES: &[&str] = &["registry-v3.json", "mcp-registry.json"];
pub const MAX_DOWNLOAD_BYTES: u64 = 50 * 1024 * 1024;

pub fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.split('.').all(|s| {
            !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()) && s.parse::<u64>().is_ok()
        })
}

pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let pa: Vec<u64> = a.split('.').map(|s| s.parse().unwrap_or(0)).collect();
    let pb: Vec<u64> = b.split('.').map(|s| s.parse().unwrap_or(0)).collect();
    for i in 0..pa.len().max(pb.len()) {
        let order = pa
            .get(i)
            .copied()
            .unwrap_or(0)
            .cmp(&pb.get(i).copied().unwrap_or(0));
        if !order.is_eq() {
            return order;
        }
    }
    std::cmp::Ordering::Equal
}

pub fn is_update_available(current: &str, manifest: &ReleaseManifest) -> bool {
    is_update_available_for(current, env!("CARGO_PKG_VERSION"), manifest)
}

pub fn is_update_available_for(
    current: &str,
    application: &str,
    manifest: &ReleaseManifest,
) -> bool {
    valid_version(current)
        && valid_version(application)
        && valid_version(&manifest.version)
        && valid_version(&manifest.min_version)
        && compare_versions(&manifest.version, current).is_gt()
        // Never reintroduce metadata older than (or equal to) the bundled
        // registry, even if an unknown/custom legacy installation reports 0.0.0.
        && compare_versions(&manifest.version, crate::registry::BUNDLED_REGISTRY_VERSION).is_gt()
        && !compare_versions(application, &manifest.min_version).is_lt()
}

/// Constructed only after signature verification; callers cannot mutate its manifest.
#[derive(Debug)]
pub struct VerifiedManifest(ReleaseManifest);
impl std::ops::Deref for VerifiedManifest {
    type Target = ReleaseManifest;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn parse_and_verify(
    manifest_json: &str,
    sig: &[u8],
    pubkey: &[u8],
) -> crate::Result<VerifiedManifest> {
    if !crate::release::verify_manifest(manifest_json, sig, pubkey) {
        return Err(Error::Other("manifest 签名校验失败".into()));
    }
    let manifest: ReleaseManifest = serde_json::from_str(manifest_json)
        .map_err(|e| Error::Other(format!("manifest 解析失败: {e}")))?;
    validate_manifest(&manifest)?;
    if manifest.pubkey_fp != crate::release::pubkey_fingerprint(pubkey) {
        return Err(Error::Other(
            "manifest public-key fingerprint mismatch".into(),
        ));
    }
    Ok(VerifiedManifest(manifest))
}

fn validate_manifest(manifest: &ReleaseManifest) -> crate::Result<()> {
    if !valid_version(&manifest.version)
        || !valid_version(&manifest.min_version)
        || compare_versions(&manifest.min_version, &manifest.version).is_gt()
    {
        return Err(Error::Other("Invalid manifest version".into()));
    }
    if manifest.files.len() != REGISTRY_FILES.len() {
        return Err(Error::Other("Invalid registry file set".into()));
    }
    let mut names = std::collections::HashSet::new();
    for file in &manifest.files {
        if !REGISTRY_FILES.contains(&file.name.as_str()) || !names.insert(&file.name) {
            return Err(Error::Other(format!(
                "Invalid or duplicate registry filename: {}",
                file.name
            )));
        }
        if file.size > MAX_DOWNLOAD_BYTES
            || file.sha256.len() != 64
            || !file.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(Error::Other(format!(
                "Invalid registry checksum or size: {}",
                file.name
            )));
        }
    }
    Ok(())
}

/// Bounded reads reject oversized responses instead of silently truncating them.
/// Local paths are supported for offline packages and isolated tests.
pub fn fetch_bytes(url: &str) -> crate::Result<Vec<u8>> {
    let reader: Box<dyn Read> = if url.starts_with("https://") || url.starts_with("http://") {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(60))
            .build();
        Box::new(
            agent
                .get(url)
                .call()
                .map_err(|e| Error::Other(format!("下载失败 {url}: {e}")))?
                .into_reader(),
        )
    } else {
        if url.contains("://") && !url.starts_with("file://") {
            return Err(Error::Other("Unsupported download URL scheme".into()));
        }
        Box::new(std::fs::File::open(
            url.strip_prefix("file://").unwrap_or(url),
        )?)
    };
    let mut buf = Vec::new();
    reader.take(MAX_DOWNLOAD_BYTES + 1).read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_DOWNLOAD_BYTES {
        return Err(Error::Other("Download exceeds size limit".into()));
    }
    Ok(buf)
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn apply_update(manifest: &VerifiedManifest) -> crate::Result<Vec<String>> {
    apply_update_at(manifest, &DataPaths::discover()?)
}

pub fn apply_update_at(
    manifest: &VerifiedManifest,
    paths: &DataPaths,
) -> crate::Result<Vec<String>> {
    apply_update_with(manifest, paths, |stage, target| {
        std::fs::rename(stage, target)
    })
}

#[cfg(test)]
fn apply_trusted_test_update_at(
    manifest: &ReleaseManifest,
    paths: &DataPaths,
) -> crate::Result<Vec<String>> {
    apply_update_with(manifest, paths, |stage, target| {
        std::fs::rename(stage, target)
    })
}

fn apply_update_with(
    manifest: &ReleaseManifest,
    paths: &DataPaths,
    commit: impl FnOnce(&std::path::Path, &std::path::Path) -> std::io::Result<()>,
) -> crate::Result<Vec<String>> {
    validate_manifest(manifest)?;
    // No disk mutations until every downloaded payload has passed integrity/schema checks.
    let mut payloads = Vec::new();
    for file in &manifest.files {
        let data = fetch_bytes(&file.url)?;
        if data.len() as u64 != file.size || sha256_hex(&data) != file.sha256.to_ascii_lowercase() {
            return Err(Error::Other(format!("文件校验失败: {}", file.name)));
        }
        let content = std::str::from_utf8(&data).map_err(|e| Error::Other(e.to_string()))?;
        if file.name == "registry-v3.json" {
            crate::registry::parse_registry(content)?;
        } else {
            let value: serde_json::Value =
                serde_json::from_str(content).map_err(|e| Error::Other(e.to_string()))?;
            let servers = value
                .get("servers")
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    Error::Other("Invalid MCP registry: servers must be an array".into())
                })?;
            let mut names = std::collections::HashSet::new();
            for server in servers {
                let name = server
                    .get("name")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| Error::Other("MCP server requires a name".into()))?;
                if !names.insert(name) {
                    return Err(Error::Other(format!("Duplicate MCP server: {name}")));
                }
                for field in ["description", "url", "command"] {
                    if server
                        .get(field)
                        .is_some_and(|v| !v.is_null() && !v.is_string())
                    {
                        return Err(Error::Other(format!("Invalid MCP {field}: {name}")));
                    }
                }
                if server.get("disabled").is_some_and(|v| !v.is_boolean())
                    || server.get("args").is_some_and(|v| {
                        !v.as_array()
                            .is_some_and(|a| a.iter().all(|v| v.is_string()))
                    })
                {
                    return Err(Error::Other(format!(
                        "Invalid MCP server configuration: {name}"
                    )));
                }
            }
        }
        payloads.push((file.name.clone(), data));
    }
    let target = &paths.registry_dir;
    let parent = target
        .parent()
        .ok_or_else(|| Error::Other("Invalid registry directory".into()))?;
    std::fs::create_dir_all(parent)?;
    let _lock = crate::registry::lock_and_recover(paths)?;
    struct Cleanup {
        path: std::path::PathBuf,
        directory: bool,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            if self.directory {
                let _ = std::fs::remove_dir_all(&self.path);
            } else {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
    let current = crate::registry::installed_version_locked(paths, env!("CARGO_PKG_VERSION"))?;
    if !is_update_available(&current, manifest) {
        return Err(Error::Other(
            "没有可应用的更新（版本或最低版本不兼容）".into(),
        ));
    }
    if target
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(Error::Other(
            "Registry directory must not be a symbolic link".into(),
        ));
    }
    let id = uuid::Uuid::new_v4();
    let stage = parent.join(format!(".registry-stage-{id}"));
    let backup = parent.join(format!("registry-backup-{id}"));
    std::fs::create_dir(&stage)?;
    let _stage_cleanup = Cleanup {
        path: stage.clone(),
        directory: true,
    };
    for (name, data) in &payloads {
        crate::vault::keystore::atomic_write_bytes(&stage.join(name), data)?;
    }
    crate::vault::keystore::atomic_write_bytes(
        &stage.join("installed-version"),
        manifest.version.as_bytes(),
    )?;
    crate::registry::sync_directory(&stage)?;
    let had_previous = target.exists();
    let journal_path = parent.join("registry-update.json");
    let journal = crate::registry::UpdateJournal {
        stage: stage.file_name().unwrap().to_string_lossy().into_owned(),
        backup: backup.file_name().unwrap().to_string_lossy().into_owned(),
        had_previous,
    };
    let journal_data = serde_json::to_vec(&journal).map_err(|e| Error::Other(e.to_string()))?;
    crate::vault::keystore::atomic_write_bytes(&journal_path, &journal_data)?;
    crate::registry::sync_directory(parent)?;
    if had_previous {
        std::fs::rename(target, &backup)?;
    }
    if let Err(error) = commit(&stage, target) {
        if had_previous {
            std::fs::rename(&backup, target).map_err(|rollback| Error::Other(format!("Update failed: {error}; restore failed: {rollback}; previous files retained at {}", backup.display())))?;
        }
        std::fs::remove_file(&journal_path)?;
        crate::registry::sync_directory(parent)?;
        return Err(error.into());
    }
    crate::registry::sync_directory(parent)?;
    std::fs::remove_file(&journal_path)?;
    crate::registry::sync_directory(parent)?;
    Ok(payloads.into_iter().map(|(name, _)| name).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::ManifestFile;
    struct Fixture {
        root: std::path::PathBuf,
        paths: DataPaths,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("agentdeck-update-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&root).unwrap();
            Self {
                paths: DataPaths::from_data_dir(root.join("data")),
                root,
            }
        }
        fn manifest(&self) -> ReleaseManifest {
            let mut files = Vec::new();
            for (name, content) in [
                (
                    "registry-v3.json",
                    include_str!("../../../../agents/registry-v3.json"),
                ),
                (
                    "mcp-registry.json",
                    include_str!("../../../../agents/mcp-registry.json"),
                ),
            ] {
                let path = self.root.join(name);
                std::fs::write(&path, content).unwrap();
                files.push(ManifestFile {
                    name: name.into(),
                    url: path.to_str().unwrap().into(),
                    sha256: sha256_hex(content.as_bytes()),
                    size: content.len() as u64,
                });
            }
            ReleaseManifest {
                version: "9.9.9".into(),
                min_version: "1.0.0".into(),
                released: "2026-11-02".into(),
                files,
                pubkey_fp: String::new(),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn versions_reject_invalid_and_downgrades() {
        let f = Fixture::new();
        let m = f.manifest();
        assert!(is_update_available("1.0.0", &m));
        assert!(!is_update_available("9.9.9", &m));
        assert!(!is_update_available_for("0.9.0", "0.9.0", &m));
        assert!(!is_update_available("invalid", &m));
        assert!(compare_versions("1.10.0", "1.9.0").is_gt());
    }

    #[test]
    fn stock_feed_cannot_replace_bundled_metadata_even_for_unknown_legacy_data() {
        let f = Fixture::new();
        let mut manifest = f.manifest();
        for version in ["1.0.0", "1.0.1", "1.1.0", "1.1.0.0"] {
            manifest.version = version.into();
            assert!(!is_update_available("0.0.0", &manifest));
            assert!(!is_update_available("1.0.0", &manifest));
            assert!(!is_update_available("1.1.0", &manifest));
        }
        manifest.version = "1.1.1".into();
        assert!(is_update_available("0.0.0", &manifest));
        assert!(is_update_available("1.1.0", &manifest));
        assert!(!is_update_available("1.2.0", &manifest));
    }

    #[test]
    fn signed_updates_enforce_bundled_floor_and_preserve_legacy_originals() {
        let f = Fixture::new();
        std::fs::create_dir_all(&f.paths.registry_dir).unwrap();
        let stock = [
            (
                "registry-v3.json",
                include_bytes!("../../tests/fixtures/registry-v1.0.0/registry-v3.json").as_slice(),
            ),
            (
                "mcp-registry.json",
                include_bytes!("../../tests/fixtures/registry-v1.0.0/mcp-registry.json").as_slice(),
            ),
        ];
        for (name, bytes) in stock {
            std::fs::write(f.paths.registry_dir.join(name), bytes).unwrap();
        }
        let (secret, public) = crate::release::generate_keypair().unwrap();
        let mut manifest = f.manifest();
        manifest.pubkey_fp = crate::release::pubkey_fingerprint(&public);
        for version in ["1.0.0", "1.1.0", "1.1.1"] {
            manifest.version = version.into();
            let json = crate::release::canonical_manifest_json(&manifest).unwrap();
            let sig = crate::release::sign_manifest(&manifest, &secret).unwrap();
            let verified = parse_and_verify(&json, &sig, &public).unwrap();
            if version != "1.1.1" {
                assert!(apply_update_at(&verified, &f.paths).is_err());
                for (name, bytes) in stock {
                    assert_eq!(
                        std::fs::read(f.paths.registry_dir.join(name)).unwrap(),
                        bytes
                    );
                }
                assert!(!f.paths.registry_dir.join("installed-version").exists());
            } else {
                apply_update_at(&verified, &f.paths).unwrap();
            }
        }
        assert_eq!(
            crate::registry::installed_version_at(&f.paths, "1.1.0").unwrap(),
            "1.1.1"
        );
        let backup = std::fs::read_dir(f.paths.registry_dir.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("registry-backup-")
            })
            .unwrap();
        for (name, bytes) in stock {
            assert_eq!(std::fs::read(backup.join(name)).unwrap(), bytes);
        }
    }

    #[test]
    fn signed_old_feed_cannot_replace_a_custom_unversioned_registry() {
        let f = Fixture::new();
        std::fs::create_dir_all(&f.paths.registry_dir).unwrap();
        let mut custom =
            crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
                .unwrap();
        custom.description = "preserve custom metadata".into();
        let original = serde_json::to_vec(&custom).unwrap();
        let path = f.paths.registry_dir.join("registry-v3.json");
        std::fs::write(&path, &original).unwrap();
        assert_eq!(
            crate::registry::installed_version_at(&f.paths, "1.1.0").unwrap(),
            "0.0.0"
        );
        let (secret, public) = crate::release::generate_keypair().unwrap();
        let mut manifest = f.manifest();
        manifest.pubkey_fp = crate::release::pubkey_fingerprint(&public);
        for version in ["1.0.0", "1.1.0"] {
            manifest.version = version.into();
            let json = crate::release::canonical_manifest_json(&manifest).unwrap();
            let sig = crate::release::sign_manifest(&manifest, &secret).unwrap();
            let verified = parse_and_verify(&json, &sig, &public).unwrap();
            assert!(apply_update_at(&verified, &f.paths).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), original);
            assert!(!f.paths.registry_dir.join("installed-version").exists());
            assert!(!f.paths.registry_dir.join("mcp-registry.json").exists());
        }
    }

    #[test]
    fn apply_update_roundtrip_with_backup() {
        let f = Fixture::new();
        let mut m = f.manifest();
        assert_eq!(
            crate::registry::installed_version_at(&f.paths, "1.0.0").unwrap(),
            "1.0.0"
        );
        apply_trusted_test_update_at(&m, &f.paths).unwrap();
        assert_eq!(
            crate::registry::installed_version_at(&f.paths, "1.0.0").unwrap(),
            "9.9.9"
        );
        assert!(
            !crate::registry::load_registry_at(&f.paths)
                .unwrap()
                .agents
                .is_empty()
        );
        assert!(apply_trusted_test_update_at(&m, &f.paths).is_err());
        m.version = "10.0.0".into();
        apply_trusted_test_update_at(&m, &f.paths).unwrap();
        let parent = f.paths.registry_dir.parent().unwrap();
        assert!(std::fs::read_dir(parent).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("registry-backup-")
        }));
    }

    #[test]
    fn corrupt_second_file_does_not_modify_first_or_version() {
        let f = Fixture::new();
        let mut m = f.manifest();
        apply_trusted_test_update_at(&m, &f.paths).unwrap();
        let before = std::fs::read(f.paths.registry_dir.join("registry-v3.json")).unwrap();
        m.version = "10.0.0".into();
        m.files[1].sha256 = "0".repeat(64);
        assert!(apply_trusted_test_update_at(&m, &f.paths).is_err());
        assert_eq!(
            before,
            std::fs::read(f.paths.registry_dir.join("registry-v3.json")).unwrap()
        );
        assert_eq!(
            crate::registry::installed_version_at(&f.paths, "1.0.0").unwrap(),
            "9.9.9"
        );
    }

    #[test]
    fn commit_failure_restores_complete_previous_directory() {
        let f = Fixture::new();
        let mut m = f.manifest();
        apply_trusted_test_update_at(&m, &f.paths).unwrap();
        std::fs::write(f.paths.registry_dir.join("extra"), "preserved in rollback").unwrap();
        m.version = "10.0.0".into();
        assert!(
            apply_update_with(&m, &f.paths, |_, _| Err(std::io::Error::other(
                "injected commit failure"
            )))
            .is_err()
        );
        assert_eq!(
            crate::registry::installed_version_at(&f.paths, "1.0.0").unwrap(),
            "9.9.9"
        );
        assert_eq!(
            std::fs::read_to_string(f.paths.registry_dir.join("extra")).unwrap(),
            "preserved in rollback"
        );
        assert!(f.paths.registry_dir.join("mcp-registry.json").exists());
    }

    #[test]
    fn rejects_traversal_duplicates_and_oversized_payloads() {
        let f = Fixture::new();
        let original = f.manifest();
        for name in ["../registry-v3.json", "/registry-v3.json", "evil.txt"] {
            let mut m = original.clone();
            m.files[0].name = name.into();
            assert!(apply_trusted_test_update_at(&m, &f.paths).is_err());
        }
        let mut m = original.clone();
        m.files[1] = m.files[0].clone();
        assert!(apply_trusted_test_update_at(&m, &f.paths).is_err());
        m = original;
        m.files[0].size = MAX_DOWNLOAD_BYTES + 1;
        assert!(apply_trusted_test_update_at(&m, &f.paths).is_err());
        assert!(!f.paths.registry_dir.exists());
    }

    #[test]
    fn signature_chain_uses_ephemeral_key_not_user_key_store() {
        let f = Fixture::new();
        let mut m = f.manifest();
        let (secret, public) = crate::release::generate_keypair().unwrap();
        m.pubkey_fp = crate::release::pubkey_fingerprint(&public);
        let sig = crate::release::sign_manifest(&m, &secret).unwrap();
        let json = crate::release::canonical_manifest_json(&m).unwrap();
        let verified = parse_and_verify(&json, &sig, &public).unwrap();
        apply_update_at(&verified, &f.paths).unwrap();
        assert!(parse_and_verify(&json.replace("9.9.9", "9.9.8"), &sig, &public).is_err());
    }
    #[test]
    fn schema_failure_is_rejected_even_with_valid_checksum() {
        let f = Fixture::new();
        let mut m = f.manifest();
        let data = b"{}";
        std::fs::write(&m.files[1].url, data).unwrap();
        m.files[1].size = data.len() as u64;
        m.files[1].sha256 = sha256_hex(data);
        assert!(apply_trusted_test_update_at(&m, &f.paths).is_err());
        assert!(!f.paths.registry_dir.exists());
    }

    #[test]
    fn bounded_local_read_rejects_one_byte_over_limit() {
        let f = Fixture::new();
        let path = f.root.join("oversized");
        std::fs::File::create(&path)
            .unwrap()
            .set_len(MAX_DOWNLOAD_BYTES + 1)
            .unwrap();
        assert!(fetch_bytes(path.to_str().unwrap()).is_err());
    }
    #[test]
    fn binary_compatibility_is_independent_of_registry_version() {
        let f = Fixture::new();
        let mut m = f.manifest();
        m.version = "10.0.0".into();
        m.min_version = "9.0.0".into();
        assert!(!is_update_available_for("9.9.9", "1.1.0", &m));
        assert!(is_update_available_for("9.9.9", "9.0.0", &m));
        m.files.pop();
        assert!(validate_manifest(&m).is_err());
    }

    #[test]
    fn binary_upgrade_does_not_falsify_installed_data_version() {
        let f = Fixture::new();
        apply_trusted_test_update_at(&f.manifest(), &f.paths).unwrap();
        assert_eq!(
            crate::registry::installed_version_at(&f.paths, "10.0.0").unwrap(),
            "9.9.9"
        );
        let mut m = f.manifest();
        m.version = "10.0.0".into();
        assert!(is_update_available_for("9.9.9", "10.0.0", &m));
    }

    #[test]
    fn recovery_restores_interrupted_directory_swap_and_lock_is_reusable() {
        let f = Fixture::new();
        apply_trusted_test_update_at(&f.manifest(), &f.paths).unwrap();
        let parent = f.paths.registry_dir.parent().unwrap();
        let id = uuid::Uuid::new_v4();
        let journal = crate::registry::UpdateJournal {
            stage: format!(".registry-stage-{id}"),
            backup: format!("registry-backup-{id}"),
            had_previous: true,
        };
        let lock = crate::registry::lock_and_recover(&f.paths).unwrap();
        let started = std::time::Instant::now();
        let error = crate::registry::load_registry_at(&f.paths).unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() >= std::time::Duration::from_secs(3));
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        std::fs::create_dir(parent.join(&journal.stage)).unwrap();
        std::fs::write(
            parent.join("registry-update.json"),
            serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        std::fs::rename(&f.paths.registry_dir, parent.join(&journal.backup)).unwrap();
        drop(lock); // Models OS lock release after process death.
        assert_eq!(
            crate::registry::installed_version_at(&f.paths, "1.1.0").unwrap(),
            "9.9.9"
        );
        assert!(
            !crate::registry::load_registry_at(&f.paths)
                .unwrap()
                .agents
                .is_empty()
        );
        assert!(!parent.join("registry-update.json").exists());
        assert!(!parent.join(&journal.stage).exists());
        assert!(
            parent.join("registry-update.lock").exists(),
            "persistent lock inode is not a stale lock"
        );
        let mut m = f.manifest();
        m.version = "10.0.0".into();
        apply_trusted_test_update_at(&m, &f.paths).unwrap();
    }
    #[test]
    fn recovery_after_completed_commit_keeps_new_registry() {
        let f = Fixture::new();
        apply_trusted_test_update_at(&f.manifest(), &f.paths).unwrap();
        let parent = f.paths.registry_dir.parent().unwrap();
        let id = uuid::Uuid::new_v4();
        let journal = crate::registry::UpdateJournal {
            stage: format!(".registry-stage-{id}"),
            backup: format!("registry-backup-{id}"),
            had_previous: true,
        };
        std::fs::create_dir(parent.join(&journal.backup)).unwrap();
        std::fs::write(
            parent.join(&journal.backup).join("installed-version"),
            "1.0.0",
        )
        .unwrap();
        std::fs::write(
            parent.join("registry-update.json"),
            serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        assert_eq!(
            crate::registry::installed_version_at(&f.paths, "1.1.0").unwrap(),
            "9.9.9"
        );
        assert!(parent.join(&journal.backup).exists());
        assert!(!parent.join("registry-update.json").exists());
    }
    #[cfg(unix)]
    #[test]
    fn recovery_rejects_symlink_backup_without_following_it() {
        let f = Fixture::new();
        let parent = f.paths.registry_dir.parent().unwrap();
        std::fs::create_dir_all(parent).unwrap();
        let outside = f.root.join("outside");
        std::fs::create_dir(&outside).unwrap();
        let id = uuid::Uuid::new_v4();
        let journal = crate::registry::UpdateJournal {
            stage: format!(".registry-stage-{id}"),
            backup: format!("registry-backup-{id}"),
            had_previous: true,
        };
        std::os::unix::fs::symlink(&outside, parent.join(&journal.backup)).unwrap();
        std::fs::write(
            parent.join("registry-update.json"),
            serde_json::to_vec(&journal).unwrap(),
        )
        .unwrap();
        assert!(
            crate::registry::load_registry_at(&f.paths)
                .unwrap_err()
                .to_string()
                .contains("symbolic links")
        );
        assert!(outside.is_dir());
        assert!(!f.paths.registry_dir.exists());
    }
}
