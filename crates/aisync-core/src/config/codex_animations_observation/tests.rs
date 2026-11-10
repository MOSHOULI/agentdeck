use super::*;
#[test]
fn fixed_dto_never_exports_tokens_or_config_secrets() {
    for value in [true, false] {
        let input = format!("secret='SENSITIVE_CANARY'\n[tui]\nanimations={value}\n");
        let result = document(input.as_bytes());
        assert_eq!(result.state, ObservationState::Ready);
        assert_eq!(result.current, Some(value));
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 6);
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["readOnly"], true);
        assert_eq!(json["applySupported"], false);
        assert_eq!(json["restoreSupported"], false);
        assert!(!format!("{result:?} {json}").contains("SENSITIVE_CANARY"));
        assert!(json.get("token").is_none());
    }
}
#[test]
fn malformed_missing_or_ambiguous_fields_are_unsupported() {
    for input in [
        "",
        "[tui]\n",
        "tui.animations=true",
        "tui={animations=true}",
        "[[tui]]\nanimations=true",
        "[tui]\nanimations='true'",
        "[tui]\nanimations=true\nanimations=false",
        "\u{feff}[tui]\nanimations=true",
    ] {
        let o = document(input.as_bytes());
        assert_eq!(o.state, ObservationState::Unsupported);
        assert_eq!(o.current, None);
    }
    assert_eq!(document(&[255]).state, ObservationState::Unsupported);
    assert_eq!(
        document(&vec![b'x'; 256 * 1024 + 1]).state,
        ObservationState::Unsupported
    );
}
#[cfg(not(windows))]
#[test]
fn other_platforms_do_not_offer_windows_observation() {
    assert_eq!(
        observe(Path::new("/nonexistent/config.toml")).state,
        ObservationState::Unsupported
    );
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{fs, path::PathBuf};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("codex-observe-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn config(&self) -> PathBuf {
            self.0.join("config.toml")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Ok(m) = fs::metadata(self.config()) {
                let mut p = m.permissions();
                p.set_readonly(false);
                let _ = fs::set_permissions(self.config(), p);
            }
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn native_existing_readonly_and_missing_are_zero_write() {
        let f = Fixture::new();
        assert_eq!(observe(&f.config()).state, ObservationState::Missing);
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
        let bytes = b"secret='PRIVATE_CANARY'\r\n[tui]\r\nanimations=true\r\n";
        fs::write(f.config(), bytes).unwrap();
        let mut permissions = fs::metadata(f.config()).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(f.config(), permissions).unwrap();
        let before = fs::metadata(f.config()).unwrap();
        let o = observe(&f.config());
        assert_eq!(o.state, ObservationState::Ready);
        assert_eq!(o.current, Some(true));
        let after = fs::metadata(f.config()).unwrap();
        assert_eq!(before.modified().unwrap(), after.modified().unwrap());
        assert!(after.permissions().readonly());
        assert_eq!(fs::read(f.config()).unwrap(), bytes);
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 1);
    }
    #[test]
    fn receipt_is_not_parsed_and_cannot_authorize_restore() {
        let f = Fixture::new();
        fs::write(f.config(), b"not even toml").unwrap();
        let receipt = f.0.join(".agentdeck-codex-animations.json");
        fs::write(&receipt, b"SECRET_INVALID_RECEIPT").unwrap();
        assert_eq!(observe(&f.config()).state, ObservationState::ReceiptPresent);
        assert_eq!(fs::read(receipt).unwrap(), b"SECRET_INVALID_RECEIPT");
        assert_eq!(fs::read_dir(&f.0).unwrap().count(), 2);
    }
    #[test]
    fn hardlinks_and_directory_targets_are_rejected() {
        let f = Fixture::new();
        let source = f.0.join("source");
        fs::write(&source, b"[tui]\nanimations=true\n").unwrap();
        fs::hard_link(&source, f.config()).unwrap();
        assert_eq!(observe(&f.config()).state, ObservationState::Unsupported);
        fs::remove_file(f.config()).unwrap();
        fs::create_dir(f.config()).unwrap();
        assert_eq!(observe(&f.config()).state, ObservationState::Unsupported);
    }
    #[test]
    fn receipt_hardlink_is_unsupported_without_reading_its_content() {
        let f = Fixture::new();
        fs::write(f.config(), b"[tui]\nanimations=true\n").unwrap();
        let other = f.0.join("receipt-source");
        fs::write(&other, b"DO_NOT_PARSE").unwrap();
        fs::hard_link(&other, f.0.join(".agentdeck-codex-animations.json")).unwrap();
        assert_eq!(observe(&f.config()).state, ObservationState::Unsupported);
    }
    #[test]
    fn junction_ancestor_is_rejected_and_not_traversed() {
        let f = Fixture::new();
        let target = f.0.join("actual");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("config.toml"), b"[tui]\nanimations=true\n").unwrap();
        let junction = f.0.join("junction");
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .unwrap();
        assert!(output.status.success(), "junction fixture setup failed");
        assert_eq!(
            observe(&junction.join("config.toml")).state,
            ObservationState::Unsupported
        );
        fs::remove_dir(junction).unwrap();
        assert_eq!(
            fs::read(target.join("config.toml")).unwrap(),
            b"[tui]\nanimations=true\n"
        );
    }
    #[test]
    fn relative_reserved_and_alternate_stream_paths_are_rejected() {
        for p in [
            "config.toml",
            "C:\\tmp\\config.toml:stream",
            "C:\\tmp\\..\\config.toml",
            "C:\\CON\\config.toml",
        ] {
            assert_eq!(observe(Path::new(p)).state, ObservationState::Unsupported);
        }
    }
}
