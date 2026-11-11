use super::*;
struct Fixture {
    dir: PathBuf,
    settings: PathBuf,
    manifest: PathBuf,
}
impl Fixture {
    fn new(document: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("qwen-owned-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let settings = dir.join("settings.json");
        let manifest = dir.join("package.json");
        fs::write(&settings, document).unwrap();
        fs::write(
            &manifest,
            r#"{"name":"@qwen-code/qwen-code","version":"0.27.0"}"#,
        )
        .unwrap();
        Self {
            dir,
            settings,
            manifest,
        }
    }
    fn apply(&self, v: bool) -> Result<bool> {
        let plan = preview(&self.settings, &self.manifest, v)?;
        apply(&self.settings, &self.manifest, v, &plan.token)
    }
    fn value(&self) -> Value {
        serde_json::from_slice(&fs::read(&self.settings).unwrap()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn preview_zero_writes_and_redacted() {
    let f = Fixture::new(r#"{"$version":4,"security":{"token":"SECRET"}}"#);
    let original = fs::read(&f.settings).unwrap();
    let p = preview(&f.settings, &f.manifest, true).unwrap();
    assert!(!serde_json::to_string(&p).unwrap().contains("SECRET"));
    assert_eq!(fs::read_dir(&f.dir).unwrap().count(), 2);
    assert_eq!(original, fs::read(&f.settings).unwrap());
}
#[test]
fn apply_restore_absent_general_preserves_other_fields() {
    let f = Fixture::new(r#"{"$version":4,"security":{"token":"SECRET"},"other":[1,2]}"#);
    let original = f.value();
    assert!(f.apply(true).unwrap());
    assert_eq!(f.value()["general"]["vimMode"], true);
    let receipt = fs::read_to_string(f.dir.join(RECEIPT)).unwrap();
    assert!(!receipt.contains("SECRET"));
    assert!(!receipt.contains("security"));
    assert!(restore(&f.settings, &f.manifest).unwrap());
    assert_eq!(f.value(), original);
    assert!(!f.dir.join(RECEIPT).exists());
}
#[test]
fn original_true_false_and_absent_fields_restore() {
    for general in [
        r#"{"vimMode":true,"other":1}"#,
        r#"{"vimMode":false,"other":1}"#,
        r#"{}"#,
    ] {
        let f = Fixture::new(&format!(r#"{{"$version":4,"general":{general}}}"#));
        let original = f.value();
        let target = field(&original).unwrap() != Some(true);
        assert!(f.apply(target).unwrap());
        assert!(restore(&f.settings, &f.manifest).unwrap());
        assert_eq!(f.value(), original);
    }
}
#[test]
fn unchanged_value_does_not_create_lock_or_receipt() {
    let f = Fixture::new(r#"{"$version":4,"general":{"vimMode":true}}"#);
    let original = fs::read(&f.settings).unwrap();
    assert!(!f.apply(true).unwrap());
    assert_eq!(fs::read_dir(&f.dir).unwrap().count(), 2);
    assert_eq!(fs::read(&f.settings).unwrap(), original);
}
#[test]
fn preview_changes_and_manifest_changes_are_rejected() {
    let f = Fixture::new(r#"{"$version":4}"#);
    let p = preview(&f.settings, &f.manifest, true).unwrap();
    fs::write(&f.settings, r#"{"$version":4,"general":{}}"#).unwrap();
    assert!(matches!(
        apply(&f.settings, &f.manifest, true, &p.token),
        Err(EditError::Changed)
    ));
    assert!(!f.dir.join(RECEIPT).exists());
    let p = preview(&f.settings, &f.manifest, true).unwrap();
    fs::write(
        &f.manifest,
        r#"{"name":"@qwen-code/qwen-code","version":"0.27.0","extra":1}"#,
    )
    .unwrap();
    assert!(matches!(
        apply(&f.settings, &f.manifest, true, &p.token),
        Err(EditError::Changed)
    ));
}
#[test]
fn restore_preserves_unrelated_external_edits() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    let mut value = f.value();
    value["general"]["other"] = Value::String("external".into());
    value["new"] = Value::Bool(true);
    fs::write(&f.settings, serde_json::to_vec(&value).unwrap()).unwrap();
    restore(&f.settings, &f.manifest).unwrap();
    let value = f.value();
    assert_eq!(value["general"]["other"], "external");
    assert_eq!(value["new"], true);
    assert!(value["general"].get("vimMode").is_none());
}
#[test]
fn managed_field_drift_refuses_and_retains_receipt() {
    let f = Fixture::new(r#"{"$version":4,"general":{"vimMode":false}}"#);
    f.apply(true).unwrap();
    fs::write(
        &f.settings,
        r#"{"$version":4,"general":{"vimMode":false},"new":1}"#,
    )
    .unwrap();
    let bytes = fs::read(&f.settings).unwrap();
    assert!(matches!(
        restore(&f.settings, &f.manifest),
        Err(EditError::Drift)
    ));
    assert_eq!(fs::read(&f.settings).unwrap(), bytes);
    assert!(f.dir.join(RECEIPT).exists());
}
#[test]
fn prepared_only_receipt_is_cancelled_without_settings_write() {
    let f = Fixture::new(r#"{"$version":4}"#);
    let bytes = fs::read(&f.settings).unwrap();
    f.apply(true).unwrap();
    fs::write(&f.settings, &bytes).unwrap();
    assert!(!restore(&f.settings, &f.manifest).unwrap());
    assert_eq!(fs::read(&f.settings).unwrap(), bytes);
    assert!(!f.dir.join(RECEIPT).exists());
}
#[test]
fn cannot_retake_ownership() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    assert!(matches!(f.apply(false), Err(EditError::AlreadyOwned)));
    assert_eq!(f.value()["general"]["vimMode"], true);
}
#[test]
fn invalid_json_schema_shape_and_duplicates_never_write() {
    for document in [
        r#"{"$version":3}"#,
        r#"{"$version":4,"general":null}"#,
        r#"{"$version":4,"general":{"vimMode":"yes"}}"#,
        r#"{"$version":4,"$version":4}"#,
        "\u{feff}{\"$version\":4}",
        "// comment\n{\"$version\":4}",
    ] {
        let f = Fixture::new(document);
        assert!(f.apply(true).is_err());
        assert_eq!(fs::read_to_string(&f.settings).unwrap(), document);
        assert_eq!(fs::read_dir(&f.dir).unwrap().count(), 2);
    }
}
#[test]
fn wrong_package_or_version_never_writes() {
    let f = Fixture::new(r#"{"$version":4}"#);
    for manifest in [
        r#"{"name":"evil","version":"0.27.0"}"#,
        r#"{"name":"@qwen-code/qwen-code","version":"0.28.0"}"#,
    ] {
        fs::write(&f.manifest, manifest).unwrap();
        assert!(f.apply(true).is_err());
        assert!(!f.dir.join(RECEIPT).exists());
    }
}
#[test]
fn occupied_or_malformed_receipt_never_overwritten() {
    let f = Fixture::new(r#"{"$version":4}"#);
    fs::write(f.dir.join(RECEIPT), b"bad").unwrap();
    assert!(matches!(f.apply(true), Err(EditError::AlreadyOwned)));
    assert!(restore(&f.settings, &f.manifest).is_err());
    assert_eq!(fs::read(f.dir.join(RECEIPT)).unwrap(), b"bad");
}
#[test]
fn cooperative_lock_excludes_another_writer() {
    let f = Fixture::new(r#"{"$version":4}"#);
    let original = fs::read(&f.settings).unwrap();
    let guard = lock(&f.settings).unwrap();
    // Windows mandatory byte locks can reject the safety read before try_lock;
    // Unix reaches the advisory-lock Busy path. Both must refuse without writes.
    let error = f.apply(true).unwrap_err();
    #[cfg(windows)]
    assert!(matches!(error, EditError::Busy | EditError::Unsupported));
    #[cfg(not(windows))]
    assert!(matches!(error, EditError::Busy));
    assert_eq!(fs::read(&f.settings).unwrap(), original);
    assert!(!f.dir.join(RECEIPT).exists());
    drop(guard);
    assert!(f.apply(true).unwrap());
    assert!(restore(&f.settings, &f.manifest).unwrap());
}
#[cfg(unix)]
#[test]
fn symlink_or_hardlink_targets_are_rejected() {
    let f = Fixture::new(r#"{"$version":4}"#);
    let external = f.dir.join("external.json");
    fs::rename(&f.settings, &external).unwrap();
    std::os::unix::fs::symlink(&external, &f.settings).unwrap();
    assert!(f.apply(true).is_err());
    fs::remove_file(&f.settings).unwrap();
    fs::hard_link(&external, &f.settings).unwrap();
    assert!(f.apply(true).is_err());
}
#[cfg(unix)]
#[test]
fn linked_receipt_and_lock_cannot_be_used() {
    for name in [RECEIPT, LOCK] {
        let f = Fixture::new(r#"{"$version":4}"#);
        let external = f.dir.join("outside");
        fs::write(&external, b"do not touch").unwrap();
        std::os::unix::fs::symlink(&external, f.dir.join(name)).unwrap();
        assert!(f.apply(true).is_err());
        assert_eq!(fs::read(&external).unwrap(), b"do not touch");
    }
}
#[cfg(unix)]
#[test]
fn private_receipt_and_original_mode_preserved() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(r#"{"$version":4}"#);
    fs::set_permissions(&f.settings, fs::Permissions::from_mode(0o640)).unwrap();
    f.apply(true).unwrap();
    assert_eq!(
        fs::metadata(f.dir.join(RECEIPT))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(&f.settings).unwrap().permissions().mode() & 0o777,
        0o640
    );
}
#[test]
fn preview_cannot_be_reused_for_identical_other_paths() {
    let a = Fixture::new(r#"{"$version":4}"#);
    let b = Fixture::new(r#"{"$version":4}"#);
    let p = preview(&a.settings, &a.manifest, true).unwrap();
    assert!(matches!(
        apply(&b.settings, &b.manifest, true, &p.token),
        Err(EditError::Changed)
    ));
    assert!(!b.dir.join(RECEIPT).exists());
}
#[test]
fn copied_receipt_cannot_target_a_different_instance() {
    let a = Fixture::new(r#"{"$version":4}"#);
    let b = Fixture::new(r#"{"$version":4,"general":{"vimMode":true}}"#);
    a.apply(true).unwrap();
    fs::copy(a.dir.join(RECEIPT), b.dir.join(RECEIPT)).unwrap();
    assert!(matches!(
        restore(&b.settings, &b.manifest),
        Err(EditError::Unsupported)
    ));
}
#[test]
fn prepared_receipt_does_not_claim_external_change() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    let mut receipt: Receipt =
        serde_json::from_slice(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
    receipt.phase = Phase::Prepared;
    fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&receipt).unwrap()).unwrap();
    fs::write(
        &f.settings,
        r#"{"$version":4,"general":{"vimMode":true},"userChange":1}"#,
    )
    .unwrap();
    assert!(matches!(
        restore(&f.settings, &f.manifest),
        Err(EditError::Drift)
    ));
}
#[test]
fn prepared_exact_after_image_can_restore() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    let mut receipt: Receipt =
        serde_json::from_slice(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
    receipt.phase = Phase::Prepared;
    fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&receipt).unwrap()).unwrap();
    assert!(restore(&f.settings, &f.manifest).unwrap());
    assert!(f.value().get("general").is_none());
}
#[test]
fn restoring_receipt_can_finish_after_replace_or_retry_before_replace() {
    for completed in [false, true] {
        let f = Fixture::new(r#"{"$version":4}"#);
        f.apply(true).unwrap();
        let mut value = f.value();
        value["userChange"] = Value::Bool(true);
        fs::write(&f.settings, serde_json::to_vec(&value).unwrap()).unwrap();
        let before = fs::read(&f.settings).unwrap();
        set_field(&mut value, None, true);
        let after = encode(&value).unwrap();
        let mut receipt: Receipt =
            serde_json::from_slice(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
        receipt.phase = Phase::Restoring;
        receipt.after_digest = digest(&before);
        receipt.restore_digest = Some(digest(&after));
        fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&receipt).unwrap()).unwrap();
        if completed {
            fs::write(&f.settings, &after).unwrap();
        }
        assert_eq!(restore(&f.settings, &f.manifest).unwrap(), !completed);
        assert_eq!(fs::read(&f.settings).unwrap(), after);
        assert!(!f.dir.join(RECEIPT).exists());
    }
}
#[test]
fn restoring_receipt_refuses_new_external_changes() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    let mut receipt: Receipt =
        serde_json::from_slice(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
    receipt.phase = Phase::Restoring;
    receipt.restore_digest = Some(receipt.before_digest.clone());
    fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&receipt).unwrap()).unwrap();
    fs::write(
        &f.settings,
        r#"{"$version":4,"general":{"vimMode":true},"another":1}"#,
    )
    .unwrap();
    assert!(matches!(
        restore(&f.settings, &f.manifest),
        Err(EditError::Drift)
    ));
}
#[cfg(windows)]
#[test]
fn restricted_windows_acl_is_copied_before_temp_content() {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::{
        Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW,
        DACL_SECURITY_INFORMATION, GetFileSecurityW, GetSecurityDescriptorDacl,
        PROTECTED_DACL_SECURITY_INFORMATION, SetFileSecurityW,
    };
    let f = Fixture::new(r#"{"$version":4,"security":{"example":"SYNTHETIC"}}"#);
    let wide = |p: &Path| {
        p.as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let sddl = "D:P(A;;FA;;;OW)"
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut descriptor = std::ptr::null_mut();
    assert_ne!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        },
        0
    );
    assert_ne!(
        unsafe {
            SetFileSecurityW(
                wide(&f.settings).as_ptr(),
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                descriptor,
            )
        },
        0
    );
    unsafe {
        LocalFree(descriptor);
    }
    let get_acl = |p: &Path| {
        let mut needed = 0;
        unsafe {
            GetFileSecurityW(
                wide(p).as_ptr(),
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                0,
                &mut needed,
            );
        }
        assert!(needed > 0);
        let mut buffer = vec![0u32; (needed as usize).div_ceil(4)];
        assert_ne!(
            unsafe {
                GetFileSecurityW(
                    wide(p).as_ptr(),
                    DACL_SECURITY_INFORMATION,
                    buffer.as_mut_ptr().cast(),
                    needed,
                    &mut needed,
                )
            },
            0
        );
        let mut present = 0;
        let mut defaulted = 0;
        let mut acl = std::ptr::null_mut();
        assert_ne!(
            unsafe {
                GetSecurityDescriptorDacl(
                    buffer.as_mut_ptr().cast(),
                    &mut present,
                    &mut acl,
                    &mut defaulted,
                )
            },
            0
        );
        assert_ne!(present, 0);
        assert!(!acl.is_null());
        unsafe { std::slice::from_raw_parts(acl.cast::<u8>(), (*acl).AclSize as usize).to_vec() }
    };
    let original_acl = get_acl(&f.settings);
    let temporary = f.dir.join("acl-probe.tmp");
    let temporary_file = private_file(&temporary).unwrap();
    copy_dacl(&f.settings, &temporary_file).unwrap();
    drop(temporary_file);
    assert_eq!(fs::metadata(&temporary).unwrap().len(), 0);
    assert_eq!(get_acl(&temporary), original_acl);
    f.apply(true).unwrap();
    assert_eq!(get_acl(&f.settings), original_acl);
    restore(&f.settings, &f.manifest).unwrap();
    assert_eq!(get_acl(&f.settings), original_acl);
}
#[cfg(target_os = "linux")]
#[test]
fn special_permission_bits_fail_closed() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new(r#"{"$version":4}"#);
    let before = fs::read(&f.settings).unwrap();
    fs::set_permissions(&f.settings, fs::Permissions::from_mode(0o2600)).unwrap();
    assert!(f.apply(true).is_err());
    assert_eq!(fs::read(&f.settings).unwrap(), before);
}
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires POSIX ACL-capable filesystem; explicitly exercised in Linux CI"]
fn extended_posix_acl_is_not_silently_lost() {
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn setxattr(
            path: *const std::ffi::c_char,
            name: *const std::ffi::c_char,
            value: *const std::ffi::c_void,
            size: usize,
            flags: i32,
        ) -> i32;
    }
    let f = Fixture::new(r#"{"$version":4}"#);
    let before = fs::read(&f.settings).unwrap();
    let mut acl = 2u32.to_le_bytes().to_vec();
    for (tag, perm, id) in [
        (1u16, 6u16, u32::MAX),
        (2, 4, {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&f.settings).unwrap().uid()
        }),
        (4, 0, u32::MAX),
        (16, 4, u32::MAX),
        (32, 0, u32::MAX),
    ] {
        acl.extend(tag.to_le_bytes());
        acl.extend(perm.to_le_bytes());
        acl.extend(id.to_le_bytes());
    }
    let path = std::ffi::CString::new(f.settings.as_os_str().as_bytes()).unwrap();
    assert_eq!(
        unsafe {
            setxattr(
                path.as_ptr(),
                c"system.posix_acl_access".as_ptr(),
                acl.as_ptr().cast(),
                acl.len(),
                0,
            )
        },
        0,
        "fixture filesystem must support POSIX ACLs: {}",
        std::io::Error::last_os_error()
    );
    assert!(!linux_has_no_acl(&f.settings));
    assert!(f.apply(true).is_err());
    assert_eq!(fs::read(&f.settings).unwrap(), before);
    assert!(!linux_has_no_acl(&f.settings));
}

#[cfg(target_os = "linux")]
fn set_test_xattr(path: &Path, value: &[u8]) {
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn setxattr(
            path: *const std::ffi::c_char,
            name: *const std::ffi::c_char,
            value: *const std::ffi::c_void,
            size: usize,
            flags: i32,
        ) -> i32;
    }
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(
        unsafe {
            setxattr(
                path.as_ptr(),
                c"user.agentdeck-test".as_ptr(),
                value.as_ptr().cast(),
                value.len(),
                0,
            )
        },
        0,
        "fixture filesystem must support user xattrs: {}",
        std::io::Error::last_os_error()
    );
}

#[cfg(target_os = "linux")]
fn test_xattr(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn getxattr(
            path: *const std::ffi::c_char,
            name: *const std::ffi::c_char,
            value: *mut std::ffi::c_void,
            size: usize,
        ) -> isize;
    }
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    let mut value = vec![0u8; 64];
    let length = unsafe {
        getxattr(
            path.as_ptr(),
            c"user.agentdeck-test".as_ptr(),
            value.as_mut_ptr().cast(),
            value.len(),
        )
    };
    assert!(length >= 0, "fixture xattr must remain unchanged");
    value.truncate(length as usize);
    value
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires user-xattr-capable filesystem; explicitly exercised in Linux CI"]
fn extended_attributes_refuse_apply_before_receipt_without_metadata_loss() {
    use std::os::unix::fs::MetadataExt;
    let f = Fixture::new(r#"{"$version":4,"secret":"SECRET"}"#);
    let original = fs::read(&f.settings).unwrap();
    set_test_xattr(&f.settings, b"apply-sentinel");
    let inode = fs::metadata(&f.settings).unwrap().ino();
    // A read-only preview remains valid; it never promises write readiness.
    assert!(preview(&f.settings, &f.manifest, true).is_ok());
    assert!(matches!(f.apply(true), Err(EditError::Unsupported)));
    assert_eq!(fs::read(&f.settings).unwrap(), original);
    assert_eq!(fs::metadata(&f.settings).unwrap().ino(), inode);
    assert_eq!(test_xattr(&f.settings), b"apply-sentinel");
    assert!(!f.dir.join(RECEIPT).exists());
    assert!(!fs::read_dir(&f.dir).unwrap().any(|entry| {
        entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|e| e == "tmp")
    }));
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires user-xattr-capable filesystem; explicitly exercised in Linux CI"]
fn extended_attributes_refuse_restore_before_receipt_transition() {
    use std::os::unix::fs::MetadataExt;
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    set_test_xattr(&f.settings, b"restore-sentinel");
    let original = recovery_snapshot(&f);
    let inode = fs::metadata(&f.settings).unwrap().ino();
    assert!(matches!(
        update_fixture(&f, false),
        Err(EditError::Unsupported)
    ));
    assert_eq!(recovery_snapshot(&f), original);
    assert!(matches!(
        restore(&f.settings, &f.manifest),
        Err(EditError::Unsupported)
    ));
    assert_eq!(recovery_snapshot(&f), original);
    assert_eq!(fs::metadata(&f.settings).unwrap().ino(), inode);
    assert_eq!(test_xattr(&f.settings), b"restore-sentinel");
    assert_eq!(
        parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap())
            .unwrap()
            .phase,
        Phase::Applied
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires user-xattr-capable filesystem; explicitly exercised in Linux CI"]
fn extended_attributes_on_receipt_refuse_replacement_without_changing_settings() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    let receipt = f.dir.join(RECEIPT);
    set_test_xattr(&receipt, b"receipt-sentinel");
    let original = recovery_snapshot(&f);
    assert!(matches!(
        restore(&f.settings, &f.manifest),
        Err(EditError::Unsupported)
    ));
    assert_eq!(recovery_snapshot(&f), original);
    assert_eq!(test_xattr(&receipt), b"receipt-sentinel");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires user-xattr-capable filesystem; explicitly exercised in Linux CI"]
fn extended_attributes_are_checked_on_open_temporary_inode() {
    let f = Fixture::new(r#"{"$version":4}"#);
    let path = f.dir.join("metadata-test.tmp");
    let file = private_file(&path).unwrap();
    assert!(linux_has_no_extended_attributes(&file));
    set_test_xattr(&path, b"temporary-sentinel");
    assert!(!linux_has_no_extended_attributes(&file));
    // Renaming the same inode does not change the fd-based observation.
    let moved = f.dir.join("renamed.tmp");
    fs::rename(&path, &moved).unwrap();
    assert!(!linux_has_no_extended_attributes(&file));
    assert_eq!(fs::metadata(&moved).unwrap().len(), 0);
    assert_eq!(test_xattr(&moved), b"temporary-sentinel");
}

fn recovery_snapshot(f: &Fixture) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(&f.dir)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

fn assert_recovery(f: &Fixture, expected: RecoveryState) {
    let before = recovery_snapshot(f);
    let report = recovery_preview(&f.settings, &f.manifest).unwrap();
    assert_eq!(
        report.changes_settings,
        expected == RecoveryState::RestoreField
    );
    assert_eq!(report.state, expected);
    let serialized = serde_json::to_string(&report).unwrap();
    for forbidden in [
        "SECRET",
        "digest",
        "selection_binding",
        f.dir.to_str().unwrap(),
    ] {
        assert!(!serialized.contains(forbidden));
    }
    assert_eq!(before, recovery_snapshot(f));
}

#[test]
fn recovery_without_receipt_never_creates_lock_or_files() {
    let f = Fixture::new(r#"{"$version":4,"secret":"SECRET"}"#);
    assert_recovery(&f, RecoveryState::NoReceipt);
    assert!(!f.dir.join(LOCK).exists());
}

#[test]
fn recovery_applied_and_unrelated_changes_agree_with_restore() {
    let f = Fixture::new(r#"{"$version":4,"general":{"vimMode":false},"secret":"SECRET"}"#);
    f.apply(true).unwrap();
    assert_recovery(&f, RecoveryState::RestoreField);
    let mut document = f.value();
    document["unrelated"] = Value::String("SECRET".into());
    fs::write(&f.settings, serde_json::to_vec(&document).unwrap()).unwrap();
    assert_recovery(&f, RecoveryState::RestoreField);
    assert!(restore(&f.settings, &f.manifest).unwrap());
    assert_eq!(f.value()["unrelated"], "SECRET");
}

#[test]
fn recovery_drift_reports_conflict_without_touching_receipt() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    fs::write(&f.settings, r#"{"$version":4,"general":{"vimMode":false}}"#).unwrap();
    assert_recovery(&f, RecoveryState::Conflict);
    assert!(matches!(
        restore(&f.settings, &f.manifest),
        Err(EditError::Drift)
    ));
}

#[test]
fn recovery_prepared_phase_matches_restore_for_before_after_and_drift() {
    for case in [0, 1, 2] {
        let f = Fixture::new(r#"{"$version":4}"#);
        let original = fs::read(&f.settings).unwrap();
        f.apply(true).unwrap();
        let mut receipt = parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
        receipt.phase = Phase::Prepared;
        fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&receipt).unwrap()).unwrap();
        let state = match case {
            0 => {
                fs::write(&f.settings, original).unwrap();
                RecoveryState::CleanupReceipt
            }
            1 => RecoveryState::RestoreField,
            _ => {
                let mut value = f.value();
                value["unrelated"] = Value::Bool(true);
                fs::write(&f.settings, serde_json::to_vec(&value).unwrap()).unwrap();
                RecoveryState::Conflict
            }
        };
        assert_recovery(&f, state);
        match case {
            0 => assert!(!restore(&f.settings, &f.manifest).unwrap()),
            1 => assert!(restore(&f.settings, &f.manifest).unwrap()),
            _ => assert!(matches!(
                restore(&f.settings, &f.manifest),
                Err(EditError::Drift)
            )),
        }
    }
}

#[test]
fn recovery_restoring_phase_matches_restore_for_prewrite_postwrite_and_drift() {
    for case in [0, 1, 2] {
        let f = Fixture::new(r#"{"$version":4}"#);
        f.apply(true).unwrap();
        let mut receipt = parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
        let mut restored = f.value();
        set_field(&mut restored, receipt.before, !receipt.had_general);
        let restored = encode(&restored).unwrap();
        receipt.phase = Phase::Restoring;
        receipt.restore_digest = Some(digest(&restored));
        fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&receipt).unwrap()).unwrap();
        let state = match case {
            0 => RecoveryState::RestoreField,
            1 => {
                fs::write(&f.settings, restored).unwrap();
                RecoveryState::CleanupReceipt
            }
            _ => {
                let mut value = f.value();
                value["unrelated"] = Value::Bool(true);
                fs::write(&f.settings, serde_json::to_vec(&value).unwrap()).unwrap();
                RecoveryState::Conflict
            }
        };
        assert_recovery(&f, state);
        match case {
            0 => assert!(restore(&f.settings, &f.manifest).unwrap()),
            1 => assert!(!restore(&f.settings, &f.manifest).unwrap()),
            _ => assert!(matches!(
                restore(&f.settings, &f.manifest),
                Err(EditError::Drift)
            )),
        }
    }
}

#[test]
fn recovery_invalid_receipt_or_manifest_remain_errors_with_zero_writes() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    let valid = fs::read(f.dir.join(RECEIPT)).unwrap();
    for malformed in [b"{\"SECRET\":1}".as_slice(), b"not json"] {
        fs::write(f.dir.join(RECEIPT), malformed).unwrap();
        let snapshot = recovery_snapshot(&f);
        assert!(matches!(
            recovery_preview(&f.settings, &f.manifest),
            Err(EditError::Unsupported)
        ));
        assert_eq!(snapshot, recovery_snapshot(&f));
    }
    fs::write(f.dir.join(RECEIPT), valid).unwrap();
    fs::write(
        &f.manifest,
        r#"{"name":"@qwen-code/qwen-code","version":"0.27.0","other":true}"#,
    )
    .unwrap();
    let snapshot = recovery_snapshot(&f);
    assert!(matches!(
        recovery_preview(&f.settings, &f.manifest),
        Err(EditError::Unsupported)
    ));
    assert_eq!(snapshot, recovery_snapshot(&f));
}

#[test]
#[cfg(unix)]
fn recovery_linked_receipt_is_rejected_without_reading_target() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new(r#"{"$version":4}"#);
    symlink(&f.settings, f.dir.join(RECEIPT)).unwrap();
    assert!(matches!(
        recovery_preview(&f.settings, &f.manifest),
        Err(EditError::Unsupported)
    ));
    assert!(!f.dir.join(LOCK).exists());
}

#[test]
fn recovery_copied_receipt_is_rejected_without_mutation() {
    let original = Fixture::new(r#"{"$version":4}"#);
    original.apply(true).unwrap();
    let other = Fixture::new(r#"{"$version":4}"#);
    fs::copy(&original.settings, &other.settings).unwrap();
    fs::copy(original.dir.join(RECEIPT), other.dir.join(RECEIPT)).unwrap();
    let snapshot = recovery_snapshot(&other);
    assert!(matches!(
        recovery_preview(&other.settings, &other.manifest),
        Err(EditError::Unsupported)
    ));
    assert_eq!(snapshot, recovery_snapshot(&other));
    assert!(!other.dir.join(LOCK).exists());
}

#[test]
fn recovery_hardlinked_receipt_is_rejected_without_mutation() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    fs::hard_link(f.dir.join(RECEIPT), f.dir.join("receipt-copy")).unwrap();
    let snapshot = recovery_snapshot(&f);
    assert!(matches!(
        recovery_preview(&f.settings, &f.manifest),
        Err(EditError::Unsupported)
    ));
    assert_eq!(snapshot, recovery_snapshot(&f));
}

#[test]
fn recovery_does_not_acquire_or_mutate_existing_operation_lock() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    let snapshot = recovery_snapshot(&f);
    let held = lock(&f.settings).unwrap();
    let report = recovery_preview(&f.settings, &f.manifest).unwrap();
    assert_eq!(report.state, RecoveryState::RestoreField);
    assert!(
        report
            .limitations
            .contains("not a restore authorization or write-readiness")
    );
    drop(held);
    assert_eq!(snapshot, recovery_snapshot(&f));
}

#[test]
#[cfg(target_os = "linux")]
fn recovery_fifo_receipt_is_rejected_without_open_blocking() {
    use std::os::unix::ffi::OsStrExt;
    unsafe extern "C" {
        fn mkfifo(path: *const std::ffi::c_char, mode: u32) -> i32;
    }
    let f = Fixture::new(r#"{"$version":4}"#);
    let path = f.dir.join(RECEIPT);
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { mkfifo(c_path.as_ptr(), 0o600) }, 0);
    assert!(matches!(
        recovery_preview(&f.settings, &f.manifest),
        Err(EditError::Unsupported)
    ));
    assert!(!f.dir.join(LOCK).exists());
    assert_eq!(fs::read_dir(&f.dir).unwrap().count(), 3);
}

#[test]
fn readonly_comment_support_does_not_enable_owned_vim_rewrite() {
    for document in [
        "\u{feff}{\"$version\":4}",
        "{/*private-comment*/\"$version\":4}",
        "{\"$version\":4}//private-comment",
    ] {
        let f = Fixture::new(document);
        let bytes = fs::read(&f.settings).unwrap();
        assert!(matches!(
            preview(&f.settings, &f.manifest, true),
            Err(EditError::Unsupported)
        ));
        assert_eq!(fs::read(&f.settings).unwrap(), bytes);
        assert_eq!(fs::read_dir(&f.dir).unwrap().count(), 2);
    }
}

fn update_fixture(f: &Fixture, after: bool) -> Result<bool> {
    let p = update_preview(&f.settings, &f.manifest, after)?;
    update(&f.settings, &f.manifest, after, &p.token)
}

#[test]
fn update_repeated_retains_first_original_and_unrelated_fields() {
    for original in [None, Some(false), Some(true)] {
        let f = Fixture::new(r#"{"$version":4,"unrelated":"keep"}"#);
        let mut d = f.value();
        if let Some(v) = original {
            set_field(&mut d, Some(v), false);
        }
        fs::write(&f.settings, encode(&d).unwrap()).unwrap();
        f.apply(original != Some(true)).unwrap();
        let first: Receipt = parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
        for v in [false, true, false, true] {
            update_fixture(&f, v).unwrap();
        }
        let last: Receipt = parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
        assert_eq!(last.format, 2);
        assert_eq!(last.before, original);
        assert_eq!(last.before_digest, first.before_digest);
        assert_eq!(last.had_general, first.had_general);
        restore(&f.settings, &f.manifest).unwrap();
        assert_eq!(f.value(), d);
    }
}

#[test]
fn update_preview_and_noop_do_not_write_or_expose_values() {
    let f = Fixture::new(r#"{"$version":4,"secret":"SENSITIVE"}"#);
    f.apply(true).unwrap();
    let bytes = fs::read(&f.settings).unwrap();
    let receipt = fs::read(f.dir.join(RECEIPT)).unwrap();
    fs::remove_file(f.dir.join(LOCK)).unwrap();
    let p = update_preview(&f.settings, &f.manifest, true).unwrap();
    assert!(!p.changes);
    assert!(!serde_json::to_string(&p).unwrap().contains("SENSITIVE"));
    assert!(!update(&f.settings, &f.manifest, true, &p.token).unwrap());
    assert!(!f.dir.join(LOCK).exists());
    assert_eq!(fs::read(&f.settings).unwrap(), bytes);
    assert_eq!(fs::read(f.dir.join(RECEIPT)).unwrap(), receipt);
}

#[test]
fn update_tokens_bind_receipt_document_target_and_operation() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    let p = update_preview(&f.settings, &f.manifest, false).unwrap();
    assert!(matches!(
        update(&f.settings, &f.manifest, true, &p.token),
        Err(EditError::Changed)
    ));
    let apply_p = preview(&f.settings, &f.manifest, false).unwrap();
    assert!(matches!(
        update(&f.settings, &f.manifest, false, &apply_p.token),
        Err(EditError::Changed)
    ));
    let receipt = fs::read(f.dir.join(RECEIPT)).unwrap();
    let mut changed_receipt = receipt.clone();
    changed_receipt.push(b' ');
    fs::write(f.dir.join(RECEIPT), changed_receipt).unwrap();
    assert!(matches!(
        update(&f.settings, &f.manifest, false, &p.token),
        Err(EditError::Changed)
    ));
    fs::write(f.dir.join(RECEIPT), receipt).unwrap();
    let mut d = f.value();
    d["other"] = Value::Bool(true);
    fs::write(&f.settings, encode(&d).unwrap()).unwrap();
    assert!(matches!(
        update(&f.settings, &f.manifest, false, &p.token),
        Err(EditError::Changed)
    ));
    update_fixture(&f, false).unwrap();
    restore(&f.settings, &f.manifest).unwrap();
    assert_eq!(f.value()["other"], true);
}

#[test]
fn update_refuses_missing_drifted_or_incomplete_receipts() {
    let f = Fixture::new(r#"{"$version":4}"#);
    assert!(update_preview(&f.settings, &f.manifest, true).is_err());
    f.apply(true).unwrap();
    let mut r = parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
    for phase in [Phase::Prepared, Phase::Restoring] {
        r.phase = phase;
        fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&r).unwrap()).unwrap();
        assert!(update_preview(&f.settings, &f.manifest, false).is_err());
    }
    r.phase = Phase::Applied;
    fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&r).unwrap()).unwrap();
    let mut d = f.value();
    set_field(&mut d, Some(false), false);
    fs::write(&f.settings, encode(&d).unwrap()).unwrap();
    assert!(matches!(
        update_preview(&f.settings, &f.manifest, true),
        Err(EditError::Drift)
    ));
}

fn pending_update(f: &Fixture, after: bool) -> (Vec<u8>, Vec<u8>) {
    let before = fs::read(&f.settings).unwrap();
    let mut d = f.value();
    set_field(&mut d, Some(after), false);
    let output = encode(&d).unwrap();
    let mut r = parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
    r.format = 2;
    r.phase = Phase::Updating;
    r.updating = Some(UpdateIntent {
        previous: r.after,
        previous_digest: digest(&before),
    });
    r.after = after;
    r.after_digest = digest(&output);
    fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&r).unwrap()).unwrap();
    (before, output)
}

#[test]
fn interrupted_update_recovers_exact_pre_and_post_images_to_initial() {
    for completed in [false, true] {
        let f = Fixture::new(r#"{"$version":4,"other":42}"#);
        let original = f.value();
        f.apply(true).unwrap();
        let (_, output) = pending_update(&f, false);
        if completed {
            fs::write(&f.settings, output).unwrap();
        }
        assert!(update_preview(&f.settings, &f.manifest, true).is_err());
        assert_eq!(
            recovery_preview(&f.settings, &f.manifest).unwrap().state,
            RecoveryState::RestoreField
        );
        assert!(restore(&f.settings, &f.manifest).unwrap());
        assert_eq!(f.value(), original);
        assert!(!f.dir.join(RECEIPT).exists());
    }
}

#[test]
fn interrupted_update_refuses_unrelated_drift_too() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    pending_update(&f, false);
    let mut d = f.value();
    d["external"] = Value::Bool(true);
    fs::write(&f.settings, encode(&d).unwrap()).unwrap();
    let bytes = fs::read(&f.settings).unwrap();
    let r = fs::read(f.dir.join(RECEIPT)).unwrap();
    assert_eq!(
        recovery_preview(&f.settings, &f.manifest).unwrap().state,
        RecoveryState::Conflict
    );
    assert!(matches!(
        restore(&f.settings, &f.manifest),
        Err(EditError::Drift)
    ));
    assert_eq!(fs::read(&f.settings).unwrap(), bytes);
    assert_eq!(fs::read(f.dir.join(RECEIPT)).unwrap(), r);
}

#[test]
fn interrupted_recovery_of_preupdate_image_can_retry() {
    let f = Fixture::new(r#"{"$version":4,"general":{"vimMode":false}}"#);
    let original = f.value();
    f.apply(true).unwrap();
    pending_update(&f, false);
    let applied_bytes = fs::read(&f.settings).unwrap();
    // Exercise the production transition, then simulate failure immediately
    // after its actual Restoring receipt is persisted but before settings write.
    assert!(matches!(
        restore_impl(
            &f.settings,
            &f.manifest,
            None,
            &|| Ok(()),
            &|| Err(EditError::Io),
            &|| Ok(())
        ),
        Err(EditError::Io)
    ));
    assert_eq!(fs::read(&f.settings).unwrap(), applied_bytes);
    let persisted = parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
    assert_eq!(persisted.phase, Phase::Restoring);
    assert!(persisted.updating.is_none());
    assert!(persisted.after); // Actual pre-restore value, NOT update target=false.
    assert_eq!(persisted.before, Some(false));
    assert_eq!(persisted.after_digest, digest(&applied_bytes));
    assert_eq!(
        recovery_preview(&f.settings, &f.manifest).unwrap().state,
        RecoveryState::RestoreField
    );
    assert!(restore(&f.settings, &f.manifest).unwrap());
    assert_eq!(f.value(), original);
    assert!(!f.dir.join(RECEIPT).exists());
}

#[test]
fn malformed_update_intents_fail_closed() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    pending_update(&f, false);
    let r = parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
    for variant in 0..4 {
        let mut bad = r.clone();
        match variant {
            0 => bad.format = 1,
            1 => bad.updating = None,
            2 => bad.updating.as_mut().unwrap().previous = false,
            _ => bad.updating.as_mut().unwrap().previous_digest = "bad".into(),
        }
        fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(matches!(
            restore(&f.settings, &f.manifest),
            Err(EditError::Unsupported)
        ));
    }
}

#[test]
fn reviewed_releases_complete_lifecycle_without_cross_version_receipt_adoption() {
    for version in ["0.27.0", "0.27.2"] {
        for original in [
            r#"{"$version":4,"unrelated":{"keep":"value"}}"#,
            r#"{"$version":4,"general":{"vimMode":false,"language":"en"}}"#,
        ] {
            let f = Fixture::new(original);
            let manifest = format!(r#"{{"name":"@qwen-code/qwen-code","version":"{version}"}}"#);
            fs::write(&f.manifest, &manifest).unwrap();
            let before = f.value();
            f.apply(true).unwrap();
            for enabled in [false, true] {
                let plan = update_preview(&f.settings, &f.manifest, enabled).unwrap();
                update(&f.settings, &f.manifest, enabled, &plan.token).unwrap();
            }
            let old_update = update_preview(&f.settings, &f.manifest, false).unwrap();
            let settings_bytes = fs::read(&f.settings).unwrap();
            let receipt_path = f.dir.join(RECEIPT);
            let receipt_bytes = fs::read(&receipt_path).unwrap();
            let other = if version == "0.27.0" {
                "0.27.2"
            } else {
                "0.27.0"
            };
            fs::write(&f.manifest, manifest.replace(version, other)).unwrap();
            assert!(recovery_preview(&f.settings, &f.manifest).is_err());
            assert!(update_preview(&f.settings, &f.manifest, false).is_err());
            assert!(update(&f.settings, &f.manifest, false, &old_update.token).is_err());
            assert!(restore(&f.settings, &f.manifest).is_err());
            assert_eq!(fs::read(&f.settings).unwrap(), settings_bytes);
            assert_eq!(fs::read(&receipt_path).unwrap(), receipt_bytes);
            fs::write(&f.manifest, &manifest).unwrap();
            restore(&f.settings, &f.manifest).unwrap();
            assert_eq!(f.value(), before);
            assert!(!receipt_path.exists());
        }
    }
}

#[test]
fn unknown_releases_and_preview_manifest_changes_are_never_implicitly_compatible() {
    for version in ["0.27.1", "0.27.3", "0.27.2-beta", "0.27.2+build", " 0.27.2"] {
        let f = Fixture::new(r#"{"$version":4}"#);
        fs::write(
            &f.manifest,
            format!(r#"{{"name":"@qwen-code/qwen-code","version":"{version}"}}"#),
        )
        .unwrap();
        assert!(f.apply(true).is_err());
        assert!(!f.dir.join(RECEIPT).exists());
        assert!(!f.dir.join(LOCK).exists());
    }
    let f = Fixture::new(r#"{"$version":4}"#);
    let plan = preview(&f.settings, &f.manifest, true).unwrap();
    fs::write(
        &f.manifest,
        r#"{"name":"@qwen-code/qwen-code","version":"0.27.2"}"#,
    )
    .unwrap();
    assert!(apply(&f.settings, &f.manifest, true, &plan.token).is_err());
    assert!(!f.dir.join(RECEIPT).exists());
}

#[test]
fn manifest_upgrade_refuses_every_receipt_phase_without_writes() {
    for phase in [
        Phase::Prepared,
        Phase::Applied,
        Phase::Updating,
        Phase::Restoring,
    ] {
        let f = Fixture::new(r#"{"$version":4,"general":{"vimMode":false}}"#);
        f.apply(true).unwrap();
        let path = f.dir.join(RECEIPT);
        let mut receipt = parse_receipt(&fs::read(&path).unwrap()).unwrap();
        receipt.format = 2;
        if phase == Phase::Updating {
            receipt.updating = Some(UpdateIntent {
                previous: false,
                previous_digest: receipt.before_digest.clone(),
            });
        }
        if phase == Phase::Restoring {
            receipt.restore_digest = Some(receipt.before_digest.clone());
        }
        receipt.phase = phase;
        fs::write(&path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        let bytes = fs::read(&f.settings).unwrap();
        let owned = fs::read(&path).unwrap();
        fs::write(
            &f.manifest,
            r#"{"name":"@qwen-code/qwen-code","version":"0.27.2"}"#,
        )
        .unwrap();
        assert!(recovery_preview(&f.settings, &f.manifest).is_err());
        assert!(update_preview(&f.settings, &f.manifest, false).is_err());
        assert!(update(&f.settings, &f.manifest, false, &"0".repeat(64)).is_err());
        assert!(restore(&f.settings, &f.manifest).is_err());
        assert_eq!(fs::read(&f.settings).unwrap(), bytes);
        assert_eq!(fs::read(&path).unwrap(), owned);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn restore_lock_released_before_forked_child_exits() {
    use crate::lock_tests::{BeforeExecChild, assert_busy, assert_released, isolated};
    if !isolated("qwen::owned::tests::restore_lock_released_before_forked_child_exits") {
        return;
    }
    for exit in ["success", "error", "panic"] {
        let f = Fixture::new(r#"{"$version":4,"general":{"vimMode":false},"other":1}"#);
        let original = f.value();
        assert!(f.apply(true).unwrap());
        let child = std::cell::RefCell::new(None);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            restore_impl(
                &f.settings,
                &f.manifest,
                None,
                &|| Ok(()),
                &|| {
                    *child.borrow_mut() = Some(BeforeExecChild::fork());
                    assert_busy(&f.dir.join(LOCK));
                    assert_ne!(exit, "panic", "injected restore unwind");
                    if exit == "error" {
                        Err(EditError::Io)
                    } else {
                        Ok(())
                    }
                },
                &|| Ok(()),
            )
        }));
        assert!(child.borrow().is_some());
        match exit {
            "panic" => assert!(outcome.is_err()),
            "error" => assert!(matches!(outcome.unwrap(), Err(EditError::Io))),
            _ => assert!(outcome.unwrap().unwrap()),
        }
        assert_released(&f.dir.join(LOCK));
        if exit != "success" {
            assert!(f.dir.join(RECEIPT).exists());
            assert!(restore(&f.settings, &f.manifest).unwrap());
        }
        assert_eq!(f.value(), original);
        assert!(!f.dir.join(RECEIPT).exists());
        assert!(f.dir.join(LOCK).exists());
    }
}

// Checked desktop restoration has a separate confirmation contract. Legacy
// recovery observations and CLI restore remain independently tested above.
fn checked_fixture(cleanup: bool) -> Fixture {
    let f = Fixture::new(r#"{"$version":4,"general":{"vimMode":false},"secret":"SECRET"}"#);
    let original = fs::read(&f.settings).unwrap();
    f.apply(true).unwrap();
    if cleanup {
        update_fixture(&f, false).unwrap();
        fs::write(&f.settings, original).unwrap();
    }
    fs::remove_file(f.dir.join(LOCK)).unwrap();
    f
}

fn assert_checked_refused_without_writes(f: &Fixture, token: &str) {
    let settings = fs::read(&f.settings).unwrap();
    let manifest = fs::read(&f.manifest).unwrap();
    let receipt = fs::read(f.dir.join(RECEIPT)).unwrap();
    assert!(restore_checked(&f.settings, &f.manifest, token).is_err());
    assert_eq!(fs::read(&f.settings).unwrap(), settings);
    assert_eq!(fs::read(&f.manifest).unwrap(), manifest);
    assert_eq!(fs::read(f.dir.join(RECEIPT)).unwrap(), receipt);
    assert!(!f.dir.join(LOCK).exists());
}

#[test]
fn restore_confirmation_is_read_only_concrete_and_redacted() {
    for cleanup in [false, true] {
        let f = checked_fixture(cleanup);
        let settings = fs::read(&f.settings).unwrap();
        let receipt = fs::read(f.dir.join(RECEIPT)).unwrap();
        let p = restore_preview(&f.settings, &f.manifest).unwrap();
        assert_eq!(p.current, Some(!cleanup));
        assert_eq!(p.initial, Some(false));
        assert_eq!(p.changes_settings, !cleanup);
        assert_eq!(
            p.state,
            if cleanup {
                RecoveryState::CleanupReceipt
            } else {
                RecoveryState::RestoreField
            }
        );
        let dto = serde_json::to_string(&p).unwrap();
        assert!(!dto.contains("SECRET"));
        assert!(!dto.contains("before_digest"));
        assert_eq!(fs::read(&f.settings).unwrap(), settings);
        assert_eq!(fs::read(f.dir.join(RECEIPT)).unwrap(), receipt);
        assert!(!f.dir.join(LOCK).exists());
        assert_eq!(
            restore_checked(&f.settings, &f.manifest, &p.token).unwrap(),
            !cleanup
        );
        assert_eq!(f.value()["general"]["vimMode"], false);
        assert!(!f.dir.join(RECEIPT).exists());
    }
}

#[test]
fn restore_confirmation_rejects_other_domains_and_malformed_tokens() {
    for cleanup in [false, true] {
        let f = checked_fixture(cleanup);
        let apply = preview(&f.settings, &f.manifest, false).unwrap();
        let update = update_preview(&f.settings, &f.manifest, false).unwrap();
        let observation = recovery_preview(&f.settings, &f.manifest).unwrap();
        assert!(
            serde_json::to_value(&observation)
                .unwrap()
                .get("token")
                .is_none()
        );
        for token in [
            apply.token,
            update.token,
            serde_json::to_string(&observation).unwrap(),
            "".into(),
            "x".repeat(64),
            "A".repeat(64),
            "0".repeat(65),
            "0".repeat(64),
        ] {
            assert_checked_refused_without_writes(&f, &token);
        }
    }
}

#[test]
fn restore_confirmation_binds_exact_settings_manifest_receipt_bytes() {
    for cleanup in [false, true] {
        for target in ["settings", "manifest", "receipt", "initial", "phase"] {
            let f = checked_fixture(cleanup);
            let p = restore_preview(&f.settings, &f.manifest).unwrap();
            let path = match target {
                "settings" => &f.settings,
                "manifest" => &f.manifest,
                _ => &f.dir.join(RECEIPT),
            };
            let mut bytes = fs::read(path).unwrap();
            if target == "initial" || target == "phase" {
                let mut r = parse_receipt(&bytes).unwrap();
                if target == "initial" {
                    r.before = None;
                } else {
                    r.phase = Phase::Prepared;
                }
                bytes = serde_json::to_vec(&r).unwrap();
            } else {
                bytes.push(b' '); // Even semantically identical JSON is a new confirmation.
            }
            fs::write(path, bytes).unwrap();
            assert_checked_refused_without_writes(&f, &p.token);
        }
    }
}

#[test]
fn restore_confirmation_rechecks_under_lock_before_restore_or_cleanup() {
    for cleanup in [false, true] {
        let f = checked_fixture(cleanup);
        let p = restore_preview(&f.settings, &f.manifest).unwrap();
        let original = fs::read(&f.settings).unwrap();
        let receipt = fs::read(f.dir.join(RECEIPT)).unwrap();
        let mut changed = receipt.clone();
        changed.push(b' ');
        let result = restore_impl(
            &f.settings,
            &f.manifest,
            Some(&p.token),
            &|| {
                fs::write(f.dir.join(RECEIPT), &changed).unwrap();
                Ok(())
            },
            &|| panic!("stale confirmation must not write"),
            &|| panic!("stale confirmation must not clean up"),
        );
        assert!(matches!(result, Err(EditError::Changed)));
        assert_eq!(fs::read(&f.settings).unwrap(), original);
        assert_eq!(fs::read(f.dir.join(RECEIPT)).unwrap(), changed);
    }
}

#[test]
fn restore_confirmation_rejects_updates_changed_cycles_and_different_paths() {
    let f = checked_fixture(false);
    let p = restore_preview(&f.settings, &f.manifest).unwrap();
    update_fixture(&f, false).unwrap();
    fs::remove_file(f.dir.join(LOCK)).unwrap();
    assert_checked_refused_without_writes(&f, &p.token);
    restore(&f.settings, &f.manifest).unwrap();
    let mut changed = f.value();
    changed["unrelated"] = Value::Bool(true);
    fs::write(&f.settings, encode(&changed).unwrap()).unwrap();
    f.apply(true).unwrap();
    fs::remove_file(f.dir.join(LOCK)).unwrap();
    assert_checked_refused_without_writes(&f, &p.token);
    let other = checked_fixture(false);
    assert_checked_refused_without_writes(&other, &p.token);
    // An alternate manifest with identical bytes still changes path selection.
    let dir = f.dir.join("runtime");
    fs::create_dir(&dir).unwrap();
    let alternative = dir.join("package.json");
    fs::copy(&f.manifest, &alternative).unwrap();
    let fresh = restore_preview(&f.settings, &f.manifest).unwrap();
    assert!(restore_checked(&f.settings, &alternative, &fresh.token).is_err());
    assert!(!f.dir.join(LOCK).exists());
}

#[test]
fn restore_confirmation_has_no_token_for_missing_conflicting_or_malformed_receipts() {
    let f = Fixture::new(r#"{"$version":4}"#);
    assert!(restore_preview(&f.settings, &f.manifest).is_err());
    assert_eq!(fs::read_dir(&f.dir).unwrap().count(), 2);
    f.apply(true).unwrap();
    fs::remove_file(f.dir.join(LOCK)).unwrap();
    fs::write(
        &f.settings,
        br#"{"$version":4,"general":{"vimMode":false}}"#,
    )
    .unwrap();
    assert!(matches!(
        restore_preview(&f.settings, &f.manifest),
        Err(EditError::Drift)
    ));
    fs::write(f.dir.join(RECEIPT), b"malformed SECRET").unwrap();
    assert!(restore_preview(&f.settings, &f.manifest).is_err());
    assert!(!f.dir.join(LOCK).exists());
}

#[test]
fn restore_confirmation_preserves_unrelated_edits_before_preview_and_initial_absence() {
    let f = Fixture::new(r#"{"$version":4}"#);
    f.apply(true).unwrap();
    update_fixture(&f, false).unwrap();
    let mut document = f.value();
    document["unrelated"] = Value::String("keep".into());
    fs::write(&f.settings, encode(&document).unwrap()).unwrap();
    let p = restore_preview(&f.settings, &f.manifest).unwrap();
    assert_eq!(p.initial, None);
    assert_eq!(p.current, Some(false));
    assert!(restore_checked(&f.settings, &f.manifest, &p.token).unwrap());
    assert_eq!(
        f.value(),
        serde_json::json!({"$version":4,"unrelated":"keep"})
    );
}

#[test]
fn restore_confirmation_byte_binding_does_not_claim_unique_cycle_identity() {
    let f = checked_fixture(false);
    let p = restore_preview(&f.settings, &f.manifest).unwrap();
    let original = fs::read(&f.settings).unwrap();
    let receipt = fs::read(f.dir.join(RECEIPT)).unwrap();
    restore_checked(&f.settings, &f.manifest, &p.token).unwrap();
    // Exact byte recreation is deliberately not a supported identity boundary.
    fs::write(&f.settings, original).unwrap();
    fs::write(f.dir.join(RECEIPT), receipt).unwrap();
    assert_eq!(
        restore_preview(&f.settings, &f.manifest).unwrap().token,
        p.token
    );
    assert!(restore_checked(&f.settings, &f.manifest, &p.token).unwrap());
}

#[test]
fn restore_confirmation_equal_current_and_initial_still_can_write_settings() {
    let f = Fixture::new(r#"{"$version":4,"general":{"vimMode":false}}"#);
    f.apply(true).unwrap();
    update_fixture(&f, false).unwrap();
    let mut document = f.value();
    document["unrelated"] = Value::Bool(true);
    fs::write(&f.settings, encode(&document).unwrap()).unwrap();
    let p = restore_preview(&f.settings, &f.manifest).unwrap();
    assert_eq!(p.current, p.initial);
    assert_eq!(p.current, Some(false));
    assert_eq!(p.state, RecoveryState::RestoreField);
    assert!(p.changes_settings);
    assert!(restore_checked(&f.settings, &f.manifest, &p.token).unwrap());
    assert_eq!(f.value(), document);
}

#[test]
fn restore_confirmation_preserves_interrupted_apply_update_restore_recovery() {
    for phase in [Phase::Prepared, Phase::Updating, Phase::Restoring] {
        for completed in [false, true] {
            let f = Fixture::new(r#"{"$version":4,"general":{"vimMode":false}}"#);
            let original = fs::read(&f.settings).unwrap();
            f.apply(true).unwrap();
            let expected_write = match phase {
                Phase::Prepared => {
                    let mut r = parse_receipt(&fs::read(f.dir.join(RECEIPT)).unwrap()).unwrap();
                    r.phase = Phase::Prepared;
                    fs::write(f.dir.join(RECEIPT), serde_json::to_vec(&r).unwrap()).unwrap();
                    if !completed {
                        fs::write(&f.settings, &original).unwrap();
                    }
                    completed
                }
                Phase::Updating => {
                    let (_, output) = pending_update(&f, false);
                    if completed {
                        fs::write(&f.settings, output).unwrap();
                    }
                    true
                }
                Phase::Restoring => {
                    let p = restore_preview(&f.settings, &f.manifest).unwrap();
                    assert!(matches!(
                        restore_impl(
                            &f.settings,
                            &f.manifest,
                            Some(&p.token),
                            &|| Ok(()),
                            &|| Err(EditError::Io),
                            &|| Ok(())
                        ),
                        Err(EditError::Io)
                    ));
                    if completed {
                        let mut d = f.value();
                        set_field(&mut d, Some(false), false);
                        fs::write(&f.settings, encode(&d).unwrap()).unwrap();
                    }
                    !completed
                }
                _ => unreachable!(),
            };
            let p = restore_preview(&f.settings, &f.manifest).unwrap();
            assert_eq!(p.changes_settings, expected_write);
            assert_eq!(
                restore_checked(&f.settings, &f.manifest, &p.token).unwrap(),
                expected_write
            );
            assert_eq!(
                f.value(),
                serde_json::from_slice::<Value>(&original).unwrap()
            );
            assert!(!f.dir.join(RECEIPT).exists());
        }
    }
}

#[test]
fn restore_transition_drift_never_overwrites_or_consumes_changed_state() {
    // Check both real transition gaps, not only stale input before locking.
    // No concurrent writer is launched: each hook changes exactly one file.
    for after_replacement in [false, true] {
        for target in ["settings", "manifest", "receipt"] {
            let f = checked_fixture(false);
            let p = restore_preview(&f.settings, &f.manifest).unwrap();
            let receipt_path = f.dir.join(RECEIPT);
            let observed = std::cell::RefCell::new(None);
            let mutate = || -> Result<()> {
                let path = match target {
                    "settings" => &f.settings,
                    "manifest" => &f.manifest,
                    _ => &receipt_path,
                };
                let mut changed = fs::read(path).unwrap();
                changed.push(b' ');
                fs::write(path, changed).unwrap();
                *observed.borrow_mut() = Some((
                    fs::read(&f.settings).unwrap(),
                    fs::read(&f.manifest).unwrap(),
                    fs::read(&receipt_path).unwrap(),
                ));
                Ok(())
            };
            let before = || if after_replacement { Ok(()) } else { mutate() };
            let after = || {
                assert!(after_replacement, "must stop before settings replacement");
                mutate()
            };
            let result = restore_impl(
                &f.settings,
                &f.manifest,
                Some(&p.token),
                &|| Ok(()),
                &before,
                &after,
            );
            assert!(matches!(result, Err(EditError::Changed)));
            let (settings, manifest, receipt) = observed.into_inner().unwrap();
            assert_eq!(fs::read(&f.settings).unwrap(), settings);
            assert_eq!(fs::read(&f.manifest).unwrap(), manifest);
            assert_eq!(fs::read(&receipt_path).unwrap(), receipt);
            assert_eq!(parse_receipt(&receipt).unwrap().phase, Phase::Restoring);
            assert_eq!(f.value()["general"]["vimMode"], !after_replacement);
        }
    }
}
