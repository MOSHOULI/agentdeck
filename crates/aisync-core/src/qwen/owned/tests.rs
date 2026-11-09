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
    let _guard = lock(&f.settings).unwrap();
    assert!(matches!(f.apply(true), Err(EditError::Busy)));
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
