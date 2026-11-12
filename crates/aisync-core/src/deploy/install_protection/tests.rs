use super::*;
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("agentdeck-protection-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn home(&self) -> PathBuf {
        let p = self.0.join("home");
        if !p.exists() {
            fs::create_dir(&p).unwrap();
        }
        p
    }
    fn preview(&self) -> InstallPreview {
        preview_at(&self.0.join("prefix"))
    }
    fn state(&self) -> PathBuf {
        self.home()
            .join(".agentdeck/install-protection/codex/state.json")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn preview_at(prefix: &Path) -> InstallPreview {
    fs::create_dir_all(prefix).unwrap();
    let prefix = normalize(prefix).unwrap();
    let root = prefix.join(if cfg!(windows) {
        "node_modules"
    } else {
        "lib/node_modules"
    });
    let bin = if cfg!(windows) {
        prefix.clone()
    } else {
        prefix.join("bin")
    };
    InstallPreview {
        schema_version: 1,
        agent_id: "codex".into(),
        package: "@openai/codex".into(),
        package_version: Some("1.2.3".into()),
        executable: "codex".into(),
        manager_path: "authored-manager".into(),
        node_path: "authored-node".into(),
        installer_node_version: "v24.0.0".into(),
        installer_node_minimum: "18.0.0".into(),
        prefix: prefix.to_str().unwrap().into(),
        global_root: root.to_str().unwrap().into(),
        package_root: root.join("@openai/codex").to_str().unwrap().into(),
        launcher_directory: bin.to_str().unwrap().into(),
        script_policy: "disabled".into(),
        version_policy: "exact_root".into(),
        revision: "authored-reviewed-plan".into(),
    }
}
fn child(f: &Fixture, mode: &str) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args([
        "--exact",
        "deploy::install_protection::tests::authored_child",
        "--ignored",
        "--nocapture",
    ])
    .env("AGENTDECK_PROTECTION_CHILD", mode)
    .env("AGENTDECK_PROTECTION_FIXTURE", &f.0)
    .env("PATH", f.0.join("absent-path"))
    .env("PATHEXT", ".COM;.EXE;.BAT;.CMD")
    .stdin(Stdio::null())
    .stdout(Stdio::null());
    c
}
#[test]
#[ignore = "invoked only as an isolated authored child"]
fn authored_child() {
    let mode = std::env::var("AGENTDECK_PROTECTION_CHILD").unwrap();
    let root = PathBuf::from(std::env::var_os("AGENTDECK_PROTECTION_FIXTURE").unwrap());
    let home = root.join("home");
    let preview = preview_at(&root.join("prefix"));
    if mode == "expect_busy" {
        assert!(Store::open(&home).is_err());
        return;
    }
    if mode == "initialize_arm" {
        if let Ok(mut store) = Store::open(&home)
            && store.is_clear()
        {
            let attempt = store.begin(&preview).unwrap();
            store.mark_failed(&attempt).unwrap();
            fs::write(
                root.join(format!("initialized-{}", std::process::id())),
                b"armed",
            )
            .unwrap();
        }
        return;
    }
    let mut store = Store::open(&home).unwrap();
    match mode.as_str() {
        "arm_wait" => {
            store.begin(&preview).unwrap();
            fs::write(root.join("ready"), b"armed").unwrap();
            loop {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        "recover" => {
            let p = store.recovery_preview().unwrap();
            assert_eq!(p.state, "eligible");
            store
                .confirm_recovery(p.revision.as_deref().unwrap())
                .unwrap();
            assert!(store.is_clear());
        }
        "path_conflict" => {
            fs::create_dir_all(root.join("absent-path")).unwrap();
            fs::write(root.join("absent-path/codex"), b"never execute").unwrap();
            assert_eq!(store.recovery_preview().unwrap().state, "blocked");
        }
        "stale_recovery" => {
            let p = store.recovery_preview().unwrap();
            assert_eq!(p.state, "eligible");
            fs::create_dir_all(root.join("absent-path")).unwrap();
            assert!(
                store
                    .confirm_recovery(p.revision.as_deref().unwrap())
                    .is_err()
            );
            assert!(!store.is_clear());
        }
        #[cfg(target_os = "linux")]
        "path_alias" => {
            use std::os::unix::fs::symlink;
            fs::create_dir(root.join("real-path")).unwrap();
            symlink(root.join("real-path"), root.join("absent-path")).unwrap();
            let p = store.recovery_preview().unwrap();
            assert_eq!(p.state, "eligible");
            fs::write(root.join("real-path/codex"), b"never executed").unwrap();
            assert_eq!(store.recovery_preview().unwrap().state, "blocked");
            assert!(
                store
                    .confirm_recovery(p.revision.as_deref().unwrap())
                    .is_err()
            );
        }
        _ => panic!("unsupported authored child mode"),
    }
}
#[test]
fn clear_arm_failure_restart_and_explicit_recovery() {
    let f = Fixture::new();
    let home = f.home();
    let preview = f.preview();
    let mut store = Store::open(&home).unwrap();
    assert!(store.is_clear());
    let token = store.begin(&preview).unwrap();
    assert!(!store.is_clear());
    assert_eq!(store.recovery_preview().unwrap().state, "blocked");
    store.mark_failed(&token).unwrap();
    drop(store);
    assert!(child(&f, "recover").status().unwrap().success());
    assert!(Store::open(&home).unwrap().is_clear());
}
#[test]
fn process_lock_and_abrupt_stop_leave_unknown_attempt_blocked() {
    let f = Fixture::new();
    let home = f.home();
    let lease = Store::open(&home).unwrap();
    assert!(child(&f, "expect_busy").status().unwrap().success());
    drop(lease);
    let mut process = child(&f, "arm_wait").spawn().unwrap();
    let start = Instant::now();
    while !f.0.join("ready").exists() {
        assert!(start.elapsed() < Duration::from_secs(15));
        assert!(process.try_wait().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(Store::open(&home).is_err());
    process.kill().unwrap();
    process.wait().unwrap();
    let store = Store::open(&home).unwrap();
    assert_eq!(store.record.phase, Phase::Armed);
    assert!(!store.is_clear());
    assert_eq!(store.recovery_preview().unwrap().state, "blocked");
    assert!(store.recovery_preview().unwrap().revision.is_none());
}
#[test]
fn successful_exact_attempt_clears_and_wrong_attempt_never_does() {
    let f = Fixture::new();
    let mut s = Store::open(&f.home()).unwrap();
    let a = s.begin(&f.preview()).unwrap();
    assert!(
        s.finish_success(&Attempt {
            id: uuid::Uuid::new_v4().to_string()
        })
        .is_err()
    );
    s.finish_success(&a).unwrap();
    assert!(s.is_clear());
    drop(s);
    assert!(Store::open(&f.home()).unwrap().is_clear());
}
#[test]
fn residues_and_path_conflicts_block_recovery_without_execution() {
    for mode in ["path_conflict", "stale_recovery"] {
        let f = Fixture::new();
        let mut s = Store::open(&f.home()).unwrap();
        let a = s.begin(&f.preview()).unwrap();
        s.mark_failed(&a).unwrap();
        drop(s);
        assert!(child(&f, mode).status().unwrap().success());
    }
    let f = Fixture::new();
    let p = f.preview();
    let mut s = Store::open(&f.home()).unwrap();
    let a = s.begin(&p).unwrap();
    s.mark_failed(&a).unwrap();
    let package = PathBuf::from(p.package_root);
    fs::create_dir_all(&package).unwrap();
    fs::write(package.join("package.json"), b"authored residual").unwrap();
    let before = fs::read(package.join("package.json")).unwrap();
    let r = s.recovery_preview().unwrap();
    assert_eq!(r.state, "blocked");
    assert_eq!(r.slots[0].state, "present");
    assert_eq!(fs::read(package.join("package.json")).unwrap(), before);
}
#[test]
fn partial_initialization_and_damaged_records_never_self_repair() {
    for missing in ["state.json", "gate.lock"] {
        let f = Fixture::new();
        drop(Store::open(&f.home()).unwrap());
        fs::remove_file(f.state().with_file_name(missing)).unwrap();
        assert!(Store::open(&f.home()).is_err());
        assert!(!f.state().with_file_name(missing).exists());
    }
    for data in [b"{".to_vec(),vec![b'x';LIMIT as usize+1],br#"{"schema":2,"platform":"linux","phase":"clear","attempt":null,"prefix":null,"prefixIdentity":null}"#.to_vec()]{let f=Fixture::new();drop(Store::open(&f.home()).unwrap());fs::write(f.state(),&data).unwrap();assert!(Store::open(&f.home()).is_err());assert_eq!(fs::read(f.state()).unwrap(),data);}
    let f = Fixture::new();
    let parent = f.home().join(".agentdeck/install-protection");
    fs::create_dir_all(&parent).unwrap();
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    }
    native::create_dir(&parent.join("codex")).unwrap();
    assert!(Store::open(&f.home()).is_err());
    assert!(!parent.join("codex/state.json").exists());
}
#[test]
fn symlinks_hardlinks_permissions_and_target_redirection_refuse() {
    let f = Fixture::new();
    drop(Store::open(&f.home()).unwrap());
    fs::hard_link(f.state(), f.0.join("state-alias")).unwrap();
    assert!(Store::open(&f.home()).is_err());
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let f = Fixture::new();
        drop(Store::open(&f.home()).unwrap());
        let state = f.state();
        fs::rename(&state, f.0.join("real-state")).unwrap();
        symlink(f.0.join("real-state"), &state).unwrap();
        assert!(Store::open(&f.home()).is_err());
        let f = Fixture::new();
        drop(Store::open(&f.home()).unwrap());
        fs::set_permissions(f.state(), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Store::open(&f.home()).is_err());
        let f = Fixture::new();
        let p = f.preview();
        let mut s = Store::open(&f.home()).unwrap();
        let a = s.begin(&p).unwrap();
        s.mark_failed(&a).unwrap();
        fs::create_dir_all(f.0.join("other")).unwrap();
        symlink(f.0.join("other"), f.0.join("prefix/lib")).unwrap();
        assert_eq!(s.recovery_preview().unwrap().state, "blocked");
    }
}
#[test]
fn private_storage_and_exact_native_security_survive_state_replacement() {
    let f = Fixture::new();
    let mut s = Store::open(&f.home()).unwrap();
    for name in ["gate.lock", "state.json"] {
        native::private(
            &native::open_file(&s.directory.join(name), false).unwrap(),
            false,
        )
        .unwrap();
    }
    native::private(&native::open_dir(&s.directory).unwrap(), true).unwrap();
    let a = s.begin(&f.preview()).unwrap();
    s.mark_failed(&a).unwrap();
    native::private(
        &native::open_file(&s.directory.join("state.json"), false).unwrap(),
        false,
    )
    .unwrap();
    drop(s);
    assert!(!Store::open(&f.home()).unwrap().is_clear());
}
#[test]
fn uncertain_replace_errors_are_not_claimed_to_leave_old_disk_state() {
    for point in [
        WritePoint::TemporarySynced,
        WritePoint::Replaced,
        WritePoint::DirectorySynced,
    ] {
        let f = Fixture::new();
        let mut s = Store::open(&f.home()).unwrap();
        let _a = s.begin(&f.preview()).unwrap();
        let result = s.replace_with(Record::clear(), |at| {
            if at == point {
                Err("injected persistence interruption".into())
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        assert!(!s.is_clear());
        drop(s);
        let reopened = Store::open(&f.home()).unwrap();
        assert_eq!(reopened.is_clear(), point != WritePoint::TemporarySynced);
    }
}
#[test]
fn invalid_reviewed_target_cannot_arm() {
    let f = Fixture::new();
    let mut s = Store::open(&f.home()).unwrap();
    let mut p = f.preview();
    p.package_root = f.0.join("unrelated").to_str().unwrap().into();
    assert!(s.begin(&p).is_err());
    assert!(s.is_clear());
}

#[test]
fn read_only_open_never_initializes_or_repairs() {
    let f = Fixture::new();
    let home = f.home();
    assert!(Store::open_existing(&home).is_err());
    assert!(!home.join(".agentdeck").exists());
    drop(Store::open(&home).unwrap());
    fs::remove_file(f.state()).unwrap();
    assert!(Store::open_existing(&home).is_err());
    assert!(!f.state().exists());
}
#[test]
#[cfg(target_os = "linux")]
fn readonly_path_aliases_are_supported_but_candidate_changes_are_not() {
    let f = Fixture::new();
    let mut s = Store::open(&f.home()).unwrap();
    let a = s.begin(&f.preview()).unwrap();
    s.mark_failed(&a).unwrap();
    drop(s);
    assert!(child(&f, "path_alias").status().unwrap().success());
}
#[test]
fn replacement_prefix_identity_does_not_recover() {
    let f = Fixture::new();
    let p = f.preview();
    let mut s = Store::open(&f.home()).unwrap();
    let a = s.begin(&p).unwrap();
    s.mark_failed(&a).unwrap();
    let prefix = PathBuf::from(p.prefix);
    fs::rename(&prefix, f.0.join("old-prefix")).unwrap();
    fs::create_dir(&prefix).unwrap();
    assert!(s.recovery_preview().is_err());
    assert!(!s.is_clear());
}
#[test]
fn raw_traversal_components_and_unknown_record_fields_are_rejected() {
    let f = Fixture::new();
    assert!(normalize(&f.0.join("./prefix")).is_err());
    assert!(normalize(&f.0.join("../prefix")).is_err());
    drop(Store::open(&f.home()).unwrap());
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(f.state()).unwrap()).unwrap();
    value["extra"] = true.into();
    fs::write(f.state(), serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(Store::open(&f.home()).is_err());
}

#[test]
#[cfg(windows)]
fn windows_untrusted_dacl_and_junction_state_are_rejected() {
    let f = Fixture::new();
    drop(Store::open(&f.home()).unwrap());
    native::broaden_test_dacl(&f.state(), true).unwrap();
    assert!(Store::open_existing(&f.home()).is_err());
    let f = Fixture::new();
    drop(Store::open(&f.home()).unwrap());
    let directory = f.state().parent().unwrap().to_owned();
    let moved = f.0.join("moved-state");
    fs::rename(&directory, &moved).unwrap();
    let status = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&directory)
        .arg(&moved)
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success());
    assert!(Store::open_existing(&f.home()).is_err());
    // Remove only this test-created junction, never recurse through its target.
    fs::remove_dir(&directory).unwrap();
}

#[test]
fn simultaneous_initializers_cannot_clear_another_process_attempt() {
    let f = Fixture::new();
    f.home();
    f.preview();
    let mut first = child(&f, "initialize_arm").spawn().unwrap();
    let mut second = child(&f, "initialize_arm").spawn().unwrap();
    assert!(first.wait().unwrap().success());
    assert!(second.wait().unwrap().success());
    assert!(fs::read_dir(&f.0).unwrap().any(|p| {
        p.unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("initialized-")
    }));
    let store = Store::open_existing(&f.home()).unwrap();
    assert_eq!(store.record.phase, Phase::FailedReturned);
    assert!(!store.is_clear());
}
#[test]
#[cfg(windows)]
fn windows_unprotected_dacl_never_counts_as_private_state() {
    let f = Fixture::new();
    drop(Store::open(&f.home()).unwrap());
    native::broaden_test_dacl(&f.state(), false).unwrap();
    assert!(Store::open_existing(&f.home()).is_err());
}
