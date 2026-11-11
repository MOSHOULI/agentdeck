//! Production entry point, isolated temporary file; never executes an Agent.
#![cfg(target_os = "linux")]
use aisync_core::config::{self, Format};
use std::{
    ffi::CString,
    fs,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, PermissionsExt},
    },
};

#[test]
fn public_inject_refuses_xattrs_without_changing_file_or_creating_backup() {
    let root = std::env::temp_dir().join(format!("config-metadata-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let target = root.join("private.txt");
    fs::write(&target, b"original").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    let path = CString::new(target.as_os_str().as_bytes()).unwrap();
    let name = c"user.audit";
    let value = b"must survive";
    assert_eq!(
        unsafe {
            libc::setxattr(
                path.as_ptr(),
                name.as_ptr(),
                value.as_ptr().cast(),
                value.len(),
                0,
            )
        },
        0,
        "xattrs required: {}",
        std::io::Error::last_os_error()
    );
    let backups = aisync_core::home_dir().unwrap().join(".agentdeck/backups");
    let inventory = || {
        walkdir::WalkDir::new(&backups)
            .into_iter()
            .filter_map(Result::ok)
            .map(|e| e.into_path())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let backups_before = inventory();
    let before = fs::metadata(&target).unwrap();
    let result = config::inject(Format::PlainText, &target, "replacement");
    assert!(
        result.is_err(),
        "must refuse rather than lose extended metadata: {result:?}"
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("extended metadata")
    );
    let after = fs::metadata(&target).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"original");
    assert_eq!(before.ino(), after.ino());
    assert_eq!(before.mode(), after.mode());
    let mut found = [0u8; 32];
    let size = unsafe {
        libc::getxattr(
            path.as_ptr(),
            name.as_ptr(),
            found.as_mut_ptr().cast(),
            found.len(),
        )
    };
    assert_eq!(size, value.len() as isize);
    assert_eq!(&found[..size as usize], value);
    assert_eq!(inventory(), backups_before);
    fs::remove_dir_all(root).unwrap();
}
