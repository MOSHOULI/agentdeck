use std::{fs, process::Command};
#[test]
fn selected_archive_is_read_only_bounded_and_has_no_overrides() {
    let dir = std::env::temp_dir().join(format!(
        "dsh-subprocess-cli-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    let archive = dir.join("dsh-subprocess-local-0.2.2-rc.1.tgz");
    fs::write(&archive, b"PRIVATE SECRET not an archive").unwrap();
    for extra in [
        vec![],
        vec!["--version", "latest"],
        vec!["--sha256", "abc"],
        vec!["--source", "https://example.com"],
        vec!["--extract"],
        vec!["--install"],
        vec!["--yes"],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_aisync"))
            .env("PATH", "")
            .current_dir(&dir)
            .args(["dsh", "verify-subprocess-package", "--archive"])
            .arg(&archive)
            .args(extra)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&out.stderr).contains("PRIVATE SECRET"));
        assert_eq!(
            fs::read(&archive).unwrap(),
            b"PRIVATE SECRET not an archive"
        );
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }
    let out = Command::new(env!("CARGO_BIN_EXE_aisync"))
        .args(["dsh", "verify-subprocess-package"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    fs::remove_dir_all(dir).unwrap();
}
