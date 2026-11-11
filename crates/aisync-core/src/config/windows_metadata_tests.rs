//! Native Windows tests use synthetic files only; no Agent or system ACL changes.
use super::*;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::{
    Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorW, DACL_SECURITY_INFORMATION,
    GetFileSecurityW, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    PROTECTED_DACL_SECURITY_INFORMATION, SE_DACL_PROTECTED, SetFileSecurityW,
};

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

fn restrict(path: &Path) {
    // OWNER RIGHTS grants the current synthetic file owner full access only.
    let sddl: Vec<u16> = "D:P(A;;FA;;;OW)".encode_utf16().chain(Some(0)).collect();
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
    let result = unsafe {
        SetFileSecurityW(
            wide(path).as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor,
        )
    };
    unsafe {
        LocalFree(descriptor);
    }
    assert_ne!(
        result,
        0,
        "restrict synthetic fixture: {}",
        std::io::Error::last_os_error()
    );
}

fn acl(path: &Path) -> (Vec<u8>, bool) {
    let mut needed = 0;
    unsafe {
        GetFileSecurityW(
            wide(path).as_ptr(),
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            0,
            &mut needed,
        );
    }
    assert!(needed > 0);
    let mut descriptor = vec![0u32; (needed as usize).div_ceil(4)];
    assert_ne!(
        unsafe {
            GetFileSecurityW(
                wide(path).as_ptr(),
                DACL_SECURITY_INFORMATION,
                descriptor.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        },
        0
    );
    let mut present = 0;
    let mut defaulted = 0;
    let mut dacl = std::ptr::null_mut();
    assert_ne!(
        unsafe {
            GetSecurityDescriptorDacl(
                descriptor.as_mut_ptr().cast(),
                &mut present,
                &mut dacl,
                &mut defaulted,
            )
        },
        0
    );
    assert_ne!(present, 0);
    assert!(!dacl.is_null());
    let mut control = 0;
    let mut revision = 0;
    assert_ne!(
        unsafe {
            GetSecurityDescriptorControl(
                descriptor.as_mut_ptr().cast(),
                &mut control,
                &mut revision,
            )
        },
        0
    );
    (
        unsafe { std::slice::from_raw_parts(dacl.cast::<u8>(), (*dacl).AclSize as usize).to_vec() },
        control & SE_DACL_PROTECTED != 0,
    )
}

fn fixture(label: &str) -> (PathBuf, PathBuf) {
    let home =
        std::env::temp_dir().join(format!("agentdeck-dacl-{label}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&home).unwrap();
    let target = home.join("instructions.md");
    fs::write(&target, b"original synthetic instructions").unwrap();
    (home, target)
}

#[test]
fn windows_configuration_replacement_preserves_protected_dacl() {
    let (home, target) = fixture("replace");
    restrict(&target);
    let before = acl(&target);
    assert_ne!(
        before,
        acl(&home),
        "fixture must differ from broader parent"
    );
    inject_in_home(
        Format::PlainText,
        &target,
        "updated synthetic instructions",
        &home,
    )
    .unwrap();
    assert_eq!(
        fs::read(&target).unwrap(),
        b"updated synthetic instructions"
    );
    assert_eq!(
        acl(&target),
        before,
        "successful replacement lost DACL or protection"
    );
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn windows_configuration_backup_preserves_protected_dacl() {
    let (home, target) = fixture("backup");
    restrict(&target);
    let before = acl(&target);
    let backup = backup_file_in(&target, &home).unwrap().unwrap();
    assert_eq!(fs::read(&backup).unwrap(), fs::read(&target).unwrap());
    assert_eq!(
        acl(&backup),
        before,
        "per-file backup lost source DACL or protection"
    );
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn windows_configuration_transaction_rollback_preserves_protected_dacl() {
    let (home, target) = fixture("rollback");
    let source = profile_source_dir_in(&home).unwrap();
    let registry =
        crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
            .unwrap();
    let mut agent = registry.agents.get("codex").unwrap().clone();
    agent.config_inject.personalization.as_mut().unwrap().target = "~/instructions.md".into();
    let second = home.join(".codex/config.toml");
    fs::create_dir_all(&second).unwrap(); // Fail after first completed replacement.
    restrict(&target);
    let before = acl(&target);
    assert!(inject_for_agent_in_home(&agent, &source, &home).is_err());
    assert_eq!(
        fs::read(&target).unwrap(),
        b"original synthetic instructions"
    );
    assert_eq!(
        acl(&target),
        before,
        "rollback lost captured DACL or protection"
    );
    fs::remove_dir_all(home).unwrap();
}
