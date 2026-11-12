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
    set_dacl(path, "D:P(A;;FA;;;OW)");
    assert_eq!(
        acl(path).2,
        SE_DACL_PROTECTED,
        "legacy fixture must be protected without AUTO_INHERITED or AUTO_INHERIT_REQ"
    );
}

fn set_dacl(path: &Path, sddl: &str) {
    // OWNER RIGHTS grants the current synthetic file owner full access only.
    let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
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

fn acl(path: &Path) -> (Vec<u8>, bool, u16) {
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
        control & (SE_DACL_PROTECTED | 0x0400 | 0x0100),
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

#[test]
fn windows_configuration_inherited_dacl_survives_write_and_backup() {
    let (home, old_target) = fixture("inherited");
    // Do not assume the runner's temporary directory uses the modern model.
    // The configuration parent is deliberately different from the backup parent;
    // do not make their ACLs equal just to hide re-inheritance at backup creation.
    fs::remove_file(&old_target).unwrap();
    let parent = home.join("private-config");
    fs::create_dir(&parent).unwrap();
    set_protected_modern_dacl(&parent, &format!("D:P(A;OICI;FA;;;{})", current_user_sid()));
    assert_eq!(acl(&parent).2, SE_DACL_PROTECTED | 0x0400);
    assert_ne!(
        acl(&parent).0,
        acl(&home).0,
        "configuration and backup parent policies must differ"
    );
    let target = parent.join("instructions.md");
    fs::write(&target, b"original synthetic instructions").unwrap();
    // Explicitly convert the child using an empty unprotected DACL. Windows
    // populates it from the private parent's inheritable ACE, as .NET does.
    set_modern_dacl(&target, "D:", false);
    let before = acl(&target);
    assert!(!before.1, "fixture must retain ordinary inheritance");
    assert_eq!(
        before.2, 0x0400,
        "ordinary fixture must have AI without P or AR"
    );
    let ace_count = u16::from_le_bytes([before.0[4], before.0[5]]);
    assert_eq!(ace_count, 1, "private parent's sole ACE must be inherited");
    assert_eq!(
        before.0[9] & 0x10,
        0x10,
        "child ACE must carry INHERITED_ACE"
    );
    let captured_security = windows_security::Security::read_path(&target).unwrap();
    let backup = match backup_file_in(&target, &home) {
        Ok(backup) => backup.unwrap(),
        Err(error) => {
            assert!(
                fs::read(&target).is_ok_and(|bytes| bytes == b"original synthetic instructions"),
                "failed writer changed original bytes"
            );
            assert!(
                windows_security::Security::read_path(&target)
                    .is_ok_and(|observed| observed == captured_security),
                "failed writer changed original security"
            );
            // Separate empty staging under the actual default backup parent.
            let diagnostic_parent = home.join(".agentdeck/backups");
            fs::create_dir_all(&diagnostic_parent).unwrap();
            let stage = diagnostic_parent.join("empty-dacl-only-diagnostic");
            let output = windows_security::create(&stage).unwrap();
            let (unprotected, api_success, exact, empty) =
                captured_security.probe_dacl_only_empty_stage(&output);
            drop(output);
            fs::remove_file(&stage).unwrap();
            let rendered = error.to_string();
            let category = [
                "staging DACL verification failed (ACL bytes and control flags)",
                "staging DACL verification failed (control flags)",
                "staging DACL verification failed (ACL bytes)",
                "staging DACL verification failed (owner/group)",
                "staging DACL application failed",
                "staging owner/group differs from original",
            ]
            .into_iter()
            .find(|category| rendered.contains(category))
            .unwrap_or("other configuration failure");
            // The real writer still fails this test, even if the alternate probe works.
            panic!(
                "real_writer_error={category}; original_preserved=true; dacl_only_stage_initially_unprotected={unprotected}; dacl_only_api_success={api_success}; dacl_only_exact_security={exact}; diagnostic_stage_empty={empty}"
            );
        }
    };
    assert_eq!(acl(&backup), before);
    inject_in_home(Format::PlainText, &target, "updated", &home).unwrap();
    assert_eq!(acl(&target), before);
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn windows_configuration_runner_default_dacl_survives_write_and_backup() {
    let (home, target) = fixture("runner-default");
    let before = acl(&target);
    let backup = backup_file_in(&target, &home).unwrap().unwrap();
    assert_eq!(acl(&backup), before);
    inject_in_home(Format::PlainText, &target, "updated", &home).unwrap();
    assert_eq!(acl(&target), before);
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn windows_configuration_staging_is_secured_while_still_empty() {
    let (home, target) = fixture("empty-staging");
    restrict(&target);
    let security = windows_security::Security::read_path(&target).unwrap();
    let staging = home.join("staging");
    let mut created = false;
    let mut output = create_staging_file(&staging, &mut created).unwrap();
    assert!(created);
    assert_eq!(output.metadata().unwrap().len(), 0);
    security.apply_before_content(&output).unwrap();
    assert_eq!(output.metadata().unwrap().len(), 0);
    assert_eq!(acl(&staging), acl(&target));
    output.write_all(b"only now write synthetic bytes").unwrap();
    drop(output);
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn windows_configuration_rollback_uses_completed_journal_entry() {
    let (home, target) = fixture("journal");
    restrict(&target);
    let original = acl(&target);
    let mut journal = Vec::new();
    inject_journaled(
        Format::PlainText,
        &target,
        "committed replacement",
        Some(&home),
        &mut journal,
    )
    .unwrap();
    assert_eq!(journal.len(), 1);
    assert_eq!(fs::read(&target).unwrap(), b"committed replacement");
    let entry = journal.pop().unwrap();
    rollback_security_checked(
        &entry.path,
        entry.before,
        entry.security,
        entry.committed_security,
    )
    .unwrap();
    assert_eq!(
        fs::read(&target).unwrap(),
        b"original synthetic instructions"
    );
    assert_eq!(acl(&target), original);
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn windows_configuration_rollback_refuses_security_drift_for_restore_and_delete() {
    for existed in [true, false] {
        let (home, target) = fixture("drift");
        if !existed {
            fs::remove_file(&target).unwrap();
        }
        let mut journal = Vec::new();
        inject_journaled(
            Format::PlainText,
            &target,
            "committed replacement",
            Some(&home),
            &mut journal,
        )
        .unwrap();
        assert_eq!(journal.len(), 1);
        restrict(&target); // Changes ordinary inherited DACL to protected owner-only.
        let changed = acl(&target);
        let entry = journal.pop().unwrap();
        assert!(
            rollback_security_checked(
                &entry.path,
                entry.before,
                entry.security,
                entry.committed_security
            )
            .is_err()
        );
        assert_eq!(fs::read(&target).unwrap(), b"committed replacement");
        assert_eq!(acl(&target), changed);
        fs::remove_dir_all(home).unwrap();
    }
}

#[test]
fn windows_configuration_null_dacl_refuses_before_backup_or_write() {
    let (home, target) = fixture("null-dacl");
    set_dacl(&target, "D:NO_ACCESS_CONTROL");
    assert!(windows_security::Security::read_path(&target).is_err());
    assert!(inject_in_home(Format::PlainText, &target, "must not write", &home).is_err());
    assert_eq!(
        fs::read(&target).unwrap(),
        b"original synthetic instructions"
    );
    assert!(!home.join(".agentdeck/backups").exists());
    restrict(&target);
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn windows_configuration_security_query_error_is_not_empty_metadata() {
    let (home, target) = fixture("query-error");
    fs::remove_file(&target).unwrap();
    assert!(windows_security::Security::read_path(&target).is_err());
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn windows_configuration_unsupported_staging_dacl_refuses_while_empty() {
    let (home, target) = fixture("unsupported-staging");
    let stage = home.join("empty-stage");
    let output = windows_security::create(&stage).unwrap();
    set_dacl(&stage, "D:NO_ACCESS_CONTROL");
    let result = checked_staging_file(|| Ok(output));
    assert!(result.is_err());
    assert_eq!(fs::metadata(&stage).unwrap().len(), 0);
    assert_eq!(
        fs::read(&target).unwrap(),
        b"original synthetic instructions"
    );
    restrict(&stage);
    fs::remove_dir_all(home).unwrap();
}

#[test]
fn windows_configuration_conditional_dacl_refuses_before_backup_or_write() {
    let (home, target) = fixture("conditional-dacl");
    // Owner retains full access, so refusal must be policy, not an access denial.
    // Callback conditional ACE refers to a resource attribute outside our scope.
    set_dacl(
        &target,
        "D:P(A;;FA;;;OW)(XA;;FR;;;WD;(@Resource.Department == \"Engineering\"))",
    );
    let before = acl(&target);
    let error = inject_in_home(Format::PlainText, &target, "must not write", &home).unwrap_err();
    assert!(
        error.to_string().contains("ordinary allow/deny subset"),
        "{error}"
    );
    assert_eq!(
        fs::read(&target).unwrap(),
        b"original synthetic instructions"
    );
    assert_eq!(acl(&target), before);
    assert!(!home.join(".agentdeck/backups").exists());
    restrict(&target);
    fs::remove_dir_all(home).unwrap();
}

fn current_user_sid() -> String {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER,
            TokenUser,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    let mut token = std::ptr::null_mut();
    assert_ne!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) },
        0
    );
    let mut needed = 0;
    unsafe {
        GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
    }
    assert!(needed > 0 && needed < 65536);
    let mut storage = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
    let result = unsafe {
        GetTokenInformation(
            token,
            TokenUser,
            storage.as_mut_ptr().cast(),
            needed,
            &mut needed,
        )
    };
    unsafe {
        CloseHandle(token);
    }
    assert_ne!(result, 0);
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let mut text = std::ptr::null_mut();
    assert_ne!(
        unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) },
        0
    );
    let mut length = 0;
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    let result = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) }).unwrap();
    unsafe {
        LocalFree(text.cast());
    }
    result
}

fn restrict_with_modern_file_api(path: &Path) {
    set_protected_modern_dacl(path, &format!("D:P(A;;FA;;;{})", current_user_sid()));
}

fn set_protected_modern_dacl(path: &Path, sddl: &str) {
    set_modern_dacl(path, sddl, true);
}

fn set_modern_dacl(path: &Path, sddl: &str, protected: bool) {
    use windows_sys::Win32::Security::{
        Authorization::{SE_FILE_OBJECT, SetNamedSecurityInfoW},
        UNPROTECTED_DACL_SECURITY_INFORMATION,
    };
    let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
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
    let mut present = 0;
    let mut defaulted = 0;
    let mut dacl = std::ptr::null_mut();
    assert_ne!(
        unsafe { GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted) },
        0
    );
    assert_ne!(present, 0);
    assert!(!dacl.is_null());
    // Same named-file API and protection flags used by .NET FileSecurity.Persist.
    let result = unsafe {
        SetNamedSecurityInfoW(
            wide(path).as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION
                | if protected {
                    PROTECTED_DACL_SECURITY_INFORMATION
                } else {
                    UNPROTECTED_DACL_SECURITY_INFORMATION
                },
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            dacl,
            std::ptr::null(),
        )
    };
    unsafe {
        LocalFree(descriptor);
    }
    assert_eq!(result, 0);
    assert_eq!(
        acl(path).2,
        (if protected { SE_DACL_PROTECTED } else { 0 }) | 0x0400,
        "must reproduce requested protection plus AUTO_INHERITED, without AR"
    );
}

#[test]
fn windows_configuration_modern_protected_ai_preserves_write_backup_and_rollback() {
    for action in ["write", "backup", "rollback"] {
        let (home, target) = fixture(action);
        restrict_with_modern_file_api(&target);
        let before = acl(&target);
        let original_security = windows_security::Security::read_path(&target).unwrap();
        match action {
            "backup" => {
                let backup = backup_file_in(&target, &home).unwrap().unwrap();
                assert_eq!(
                    fs::read(&backup).unwrap(),
                    b"original synthetic instructions"
                );
                assert_eq!(acl(&backup), before);
                assert_eq!(
                    windows_security::Security::read_path(&backup).unwrap(),
                    original_security
                );
            }
            "write" => {
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
            }
            "rollback" => {
                let mut journal = Vec::new();
                inject_journaled(
                    Format::PlainText,
                    &target,
                    "committed replacement",
                    Some(&home),
                    &mut journal,
                )
                .unwrap();
                assert_eq!(journal.len(), 1);
                assert_eq!(fs::read(&target).unwrap(), b"committed replacement");
                let entry = journal.pop().unwrap();
                rollback_security_checked(
                    &entry.path,
                    entry.before,
                    entry.security,
                    entry.committed_security,
                )
                .unwrap();
                assert_eq!(
                    fs::read(&target).unwrap(),
                    b"original synthetic instructions"
                );
            }
            _ => unreachable!(),
        }
        assert_eq!(acl(&target), before);
        assert_eq!(
            windows_security::Security::read_path(&target).unwrap(),
            original_security
        );
        fs::remove_dir_all(home).unwrap();
    }
}
