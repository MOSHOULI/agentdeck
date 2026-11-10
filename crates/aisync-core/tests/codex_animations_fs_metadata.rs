//! Real temporary-filesystem metadata regressions, never Codex execution.
//! Capability-dependent cases are explicitly required in hosted Linux CI.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod linux {
    use aisync_core::config::codex_animations_fs as subject;
    use std::{
        collections::BTreeMap,
        ffi::CString,
        fs,
        os::unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, PermissionsExt},
        },
        path::{Path, PathBuf},
    };

    unsafe extern "C" {
        fn setxattr(
            path: *const std::ffi::c_char,
            name: *const std::ffi::c_char,
            value: *const std::ffi::c_void,
            size: usize,
            flags: i32,
        ) -> i32;
        fn getxattr(
            path: *const std::ffi::c_char,
            name: *const std::ffi::c_char,
            value: *mut std::ffi::c_void,
            size: usize,
        ) -> isize;
    }
    const ORIGINAL: &[u8] =
        b"# private authored fixture\n[tui]\nanimations = true # preserve comment\n";
    struct Fixture {
        root: PathBuf,
        config: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("codex-fs-metadata-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            let config = root.join("config.toml");
            fs::write(&config, ORIGINAL).unwrap();
            fs::set_permissions(&config, fs::Permissions::from_mode(0o600)).unwrap();
            Self { root, config }
        }
        fn apply(&self) {
            let preview = subject::preview(&self.config, false).unwrap();
            assert!(subject::apply(&self.config, false, &preview.token).unwrap());
        }
        fn files(&self) -> BTreeMap<PathBuf, Vec<u8>> {
            fs::read_dir(&self.root)
                .unwrap()
                .map(|entry| {
                    let path = entry.unwrap().path();
                    let bytes = fs::read(&path).unwrap();
                    (path, bytes)
                })
                .collect()
        }
        fn receipt(&self) -> PathBuf {
            let paths: Vec<_> = fs::read_dir(&self.root)
                .unwrap()
                .map(|p| p.unwrap().path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect();
            assert_eq!(paths.len(), 1, "exactly one ownership receipt required");
            paths[0].clone()
        }
        fn no_sidecar_content(&self) {
            for (path, bytes) in self.files() {
                if path != self.config {
                    assert!(bytes.is_empty(), "failed preflight left sidecar content");
                }
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn set_attribute(path: &Path, name: &str, value: &[u8]) {
        let path = CString::new(path.as_os_str().as_bytes()).unwrap();
        let name = CString::new(name).unwrap();
        assert_eq!(
            unsafe {
                setxattr(
                    path.as_ptr(),
                    name.as_ptr(),
                    value.as_ptr().cast(),
                    value.len(),
                    0,
                )
            },
            0,
            "required test filesystem attribute capability unavailable: {}",
            std::io::Error::last_os_error()
        );
    }
    fn attribute(path: &Path, name: &str) -> Vec<u8> {
        let path = CString::new(path.as_os_str().as_bytes()).unwrap();
        let name = CString::new(name).unwrap();
        let size = unsafe { getxattr(path.as_ptr(), name.as_ptr(), std::ptr::null_mut(), 0) };
        assert!((0..=4096).contains(&size), "fixture attribute missing");
        let mut bytes = vec![0; size as usize];
        assert_eq!(
            unsafe {
                getxattr(
                    path.as_ptr(),
                    name.as_ptr(),
                    bytes.as_mut_ptr().cast(),
                    bytes.len(),
                )
            },
            size
        );
        bytes
    }
    fn acl(path: &Path) -> Vec<u8> {
        let mut bytes = 2u32.to_le_bytes().to_vec();
        for (tag, perm, id) in [
            (1u16, 6u16, u32::MAX),
            (2, 4, fs::metadata(path).unwrap().uid()),
            (4, 0, u32::MAX),
            (16, 4, u32::MAX),
            (32, 0, u32::MAX),
        ] {
            bytes.extend(tag.to_le_bytes());
            bytes.extend(perm.to_le_bytes());
            bytes.extend(id.to_le_bytes());
        }
        bytes
    }
    fn identity(path: &Path) -> (u64, u64, u32, u32, u32, u64) {
        let m = fs::symlink_metadata(path).unwrap();
        (m.dev(), m.ino(), m.mode(), m.uid(), m.gid(), m.nlink())
    }
    #[test]
    fn plain_mode_and_original_bytes_survive_apply_restore() {
        let f = Fixture::new();
        fs::set_permissions(&f.config, fs::Permissions::from_mode(0o640)).unwrap();
        f.apply();
        assert_eq!(fs::metadata(&f.config).unwrap().mode() & 0o777, 0o640);
        assert_eq!(fs::metadata(f.receipt()).unwrap().mode() & 0o777, 0o600);
        assert!(subject::restore(&f.config).unwrap());
        assert_eq!(fs::read(&f.config).unwrap(), ORIGINAL);
        assert_eq!(fs::metadata(&f.config).unwrap().mode() & 0o777, 0o640);
    }
    #[test]
    #[ignore = "requires user xattrs; mandatory hosted Linux CI"]
    fn codex_fs_metadata_target_xattr_rejects_apply_without_loss() {
        let f = Fixture::new();
        let p = subject::preview(&f.config, false).unwrap();
        set_attribute(&f.config, "user.agentdeck-test", b"metadata");
        let id = identity(&f.config);
        assert!(subject::apply(&f.config, false, &p.token).is_err());
        assert_eq!(fs::read(&f.config).unwrap(), ORIGINAL);
        assert_eq!(identity(&f.config), id);
        assert_eq!(attribute(&f.config, "user.agentdeck-test"), b"metadata");
        f.no_sidecar_content();
    }
    #[test]
    #[ignore = "requires POSIX ACL; mandatory hosted Linux CI"]
    fn codex_fs_metadata_target_acl_rejects_apply_without_loss() {
        let f = Fixture::new();
        let p = subject::preview(&f.config, false).unwrap();
        set_attribute(&f.config, "system.posix_acl_access", &acl(&f.config));
        let stored = attribute(&f.config, "system.posix_acl_access");
        let id = identity(&f.config);
        assert!(subject::preview(&f.config, false).is_err());
        assert!(subject::apply(&f.config, false, &p.token).is_err());
        assert_eq!(fs::read(&f.config).unwrap(), ORIGINAL);
        assert_eq!(identity(&f.config), id);
        assert_eq!(attribute(&f.config, "system.posix_acl_access"), stored);
        f.no_sidecar_content();
    }
    #[test]
    #[ignore = "requires user xattrs; mandatory hosted Linux CI"]
    fn codex_fs_metadata_target_xattr_refuses_restore_before_receipt_transition() {
        let f = Fixture::new();
        f.apply();
        set_attribute(&f.config, "user.agentdeck-test", b"restore");
        let before = f.files();
        let id = identity(&f.config);
        assert!(subject::restore(&f.config).is_err());
        assert_eq!(f.files(), before);
        assert_eq!(identity(&f.config), id);
        assert_eq!(attribute(&f.config, "user.agentdeck-test"), b"restore");
    }
    #[test]
    #[ignore = "requires user xattrs; mandatory hosted Linux CI"]
    fn codex_fs_metadata_receipt_xattr_refuses_restore_without_loss() {
        let f = Fixture::new();
        f.apply();
        let receipt = f.receipt();
        set_attribute(&receipt, "user.agentdeck-test", b"receipt");
        let before = f.files();
        let id = identity(&f.config);
        let receipt_id = identity(&receipt);
        assert!(subject::restore(&f.config).is_err());
        assert_eq!(f.files(), before);
        assert_eq!(identity(&f.config), id);
        assert_eq!(identity(&receipt), receipt_id);
        assert_eq!(attribute(&receipt, "user.agentdeck-test"), b"receipt");
    }
    #[test]
    #[ignore = "requires default POSIX ACL inheritance; mandatory hosted Linux CI"]
    fn codex_fs_metadata_inherited_acl_refuses_replacement_temp_without_writes() {
        let f = Fixture::new();
        f.apply();
        set_attribute(&f.root, "system.posix_acl_default", &acl(&f.root));
        let probe = f.root.join("inheritance-probe");
        fs::write(&probe, b"").unwrap();
        assert!(!attribute(&probe, "system.posix_acl_access").is_empty());
        fs::remove_file(probe).unwrap();
        let before = f.files();
        let config_id = identity(&f.config);
        let receipt_id = identity(&f.receipt());
        assert!(subject::restore(&f.config).is_err());
        assert_eq!(
            f.files(),
            before,
            "temporary must be removed, receipt unchanged"
        );
        assert_eq!(identity(&f.config), config_id);
        assert_eq!(identity(&f.receipt()), receipt_id);
    }

    #[test]
    #[ignore = "requires default POSIX ACL inheritance; mandatory hosted Linux CI"]
    fn codex_fs_metadata_inherited_acl_refuses_new_sidecar_content() {
        let f = Fixture::new();
        let p = subject::preview(&f.config, false).unwrap();
        set_attribute(&f.root, "system.posix_acl_default", &acl(&f.root));
        // Prove this filesystem actually propagates nontrivial ACL metadata.
        let probe = f.root.join("inheritance-probe");
        fs::write(&probe, b"").unwrap();
        assert!(!attribute(&probe, "system.posix_acl_access").is_empty());
        fs::remove_file(probe).unwrap();
        let id = identity(&f.config);
        assert!(subject::preview(&f.config, false).is_err());
        assert!(subject::apply(&f.config, false, &p.token).is_err());
        assert_eq!(fs::read(&f.config).unwrap(), ORIGINAL);
        assert_eq!(identity(&f.config), id);
        f.no_sidecar_content();
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
#[test]
fn unsupported_platform_never_creates_configuration_or_sidecars() {
    use aisync_core::config::codex_animations_fs as subject;
    let root = std::env::temp_dir().join(format!("codex-fs-unsupported-{}", uuid::Uuid::new_v4()));
    let config = root.join("config.toml");
    assert!(matches!(
        subject::preview(&config, false),
        Err(subject::Error::Unsupported)
    ));
    assert!(matches!(
        subject::apply(&config, false, "invalid"),
        Err(subject::Error::Unsupported)
    ));
    assert!(matches!(
        subject::restore(&config),
        Err(subject::Error::Unsupported)
    ));
    assert!(!root.exists());
}
