use std::{fs, process::Command};
#[test]
fn native_archive_cli_requires_exact_selection_and_never_runs_package_code() {
    let dir = std::env::temp_dir().join(format!(
        "dsh-native-cli-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&dir).unwrap();
    for (id, name) in [
        ("node-pty", "node-pty-1.2.0-beta.15.tgz"),
        ("koffi", "koffi-3.1.1.tgz"),
        ("koffi-linux-x64", "koffi-linux-x64-3.1.1.tgz"),
        ("koffi-win32-x64", "koffi-win32-x64-3.1.1.tgz"),
    ] {
        let path = dir.join(name);
        fs::write(&path, b"PRIVATE SYNTHETIC not an archive").unwrap();
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        for extra in [
            vec![],
            vec!["--version", "latest"],
            vec!["--sha256", "abc"],
            vec!["--source", "https://example.com"],
            vec!["--extract"],
            vec!["--install"],
            vec!["--yes"],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_aisync"))
                .env("PATH", "")
                .current_dir(&dir)
                .args(["dsh", "verify-native-package", "--package", id, "--archive"])
                .arg(&path)
                .args(extra)
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE SYNTHETIC"));
            assert_eq!(
                fs::read(&path).unwrap(),
                b"PRIVATE SYNTHETIC not an archive"
            );
            assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
            assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        }
        fs::remove_file(path).unwrap();
    }
    for args in [
        vec!["dsh", "verify-native-package"],
        vec!["dsh", "verify-native-package", "--package", "latest"],
        vec!["dsh", "verify-native-package", "--archive", "relative.tgz"],
    ] {
        assert!(
            !Command::new(env!("CARGO_BIN_EXE_aisync"))
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    fs::remove_dir_all(dir).unwrap();
}
