use super::*;
use std::fs;
use windows_sys::Win32::System::WindowsProgramming::{
    DRIVE_CDROM, DRIVE_NO_ROOT_DIR, DRIVE_RAMDISK, DRIVE_REMOTE, DRIVE_REMOVABLE, DRIVE_UNKNOWN,
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("codex-residual-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn states(r: ResidualObservation) -> Vec<SlotState> {
    r.slots.into_iter().map(|s| s.state).collect()
}

#[test]
fn codex_residual_empty_prefix_creates_nothing() {
    let f = Fixture::new();
    let before = fs::metadata(&f.0).unwrap().modified().unwrap();
    assert_eq!(states(inspect(&f.0).unwrap()), vec![SlotState::Missing; 4]);
    assert_eq!(fs::read_dir(&f.0).unwrap().count(), 0);
    assert_eq!(fs::metadata(&f.0).unwrap().modified().unwrap(), before);
}
#[test]
fn codex_residual_partial_slots_do_not_read_or_execute_contents() {
    let f = Fixture::new();
    let pkg = f.0.join("node_modules/@openai/codex");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(pkg.join("secret"), [0xff, 0x00, 0xfe]).unwrap();
    let launcher = f.0.join("codex.cmd");
    let bytes = b"@echo SECRET-DO-NOT-EXECUTE > codex-executed.txt\n";
    fs::write(&launcher, bytes).unwrap();
    fs::create_dir(f.0.join("codex.ps1")).unwrap();
    let before = fs::metadata(&launcher).unwrap().modified().unwrap();
    let report = inspect(&f.0).unwrap();
    let json = serde_json::to_string(&report).unwrap();
    assert!(
        !json.contains("SECRET") && !json.contains("secret") && !json.contains("codex-executed")
    );
    assert_eq!(
        states(report),
        vec![
            SlotState::Directory,
            SlotState::Missing,
            SlotState::File,
            SlotState::Directory
        ]
    );
    assert_eq!(fs::read(launcher).unwrap(), bytes);
    assert_eq!(
        fs::metadata(f.0.join("codex.cmd"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
    assert!(!f.0.join("codex-executed.txt").exists());
}
#[test]
fn codex_residual_middle_file_is_blocked_not_missing() {
    let f = Fixture::new();
    fs::write(f.0.join("node_modules"), b"secret").unwrap();
    assert_eq!(inspect(&f.0).unwrap().slots[0].state, SlotState::Blocked);
}
#[test]
fn codex_residual_hardlink_is_blocked() {
    let f = Fixture::new();
    fs::write(f.0.join("secret"), b"private").unwrap();
    fs::hard_link(f.0.join("secret"), f.0.join("codex.cmd")).unwrap();
    assert_eq!(inspect(&f.0).unwrap().slots[2].state, SlotState::Blocked);
}
#[test]
fn codex_residual_missing_appearing_during_observation_rejects() {
    let f = Fixture::new();
    let r = inspect_with(&f.0, |p| fs::write(p.join("codex.cmd"), b"new").unwrap());
    assert!(matches!(r, Err(ResidualInspectionError::Changed)));
}
#[test]
fn codex_residual_open_writer_is_unavailable_not_missing() {
    let f = Fixture::new();
    let p = f.0.join("codex.cmd");
    fs::write(&p, b"x").unwrap();
    let _write = fs::OpenOptions::new().write(true).open(&p).unwrap();
    assert!(matches!(
        inspect(&f.0),
        Err(ResidualInspectionError::Unavailable)
    ));
}
#[test]
fn codex_residual_rejects_ambiguous_paths() {
    for p in [
        r"relative",
        r"C:relative",
        r"\\server\share",
        r"\\?\C:\prefix",
        r"\\.\C:\prefix",
        r"C:\a\..\b",
        r"C:\a\.\b",
        r"C:\prefix:stream",
        r"C:\NUL",
        r"C:\COM1.txt",
        r"C:\COM¹.txt",
        r"C:\LPT³",
        "",
        "中",
        r"C:\prefix.",
        r"C:\prefix ",
        "C:\\a\u{202e}b",
    ] {
        assert!(validate(Path::new(p)).is_err(), "{p}");
    }
}
#[test]
fn codex_residual_supports_unicode_and_spaces() {
    let f = Fixture::new();
    let p = f.0.join("中文 空格");
    fs::create_dir(&p).unwrap();
    assert_eq!(states(inspect(&p).unwrap()), vec![SlotState::Missing; 4]);
}
#[test]
fn codex_residual_held_directories_and_files_deny_replacement_or_write() {
    let f = Fixture::new();
    let pkg = f.0.join("node_modules/@openai/codex");
    fs::create_dir_all(&pkg).unwrap();
    fs::write(f.0.join("codex.cmd"), b"fixed").unwrap();
    let r = inspect_with(&f.0, |p| {
        assert!(fs::rename(p.join("node_modules"), p.join("moved")).is_err());
        assert!(fs::write(p.join("codex.cmd"), b"changed").is_err());
    });
    assert!(r.is_ok());
}

#[test]
fn codex_residual_reparse_ancestors_reject_and_leaf_is_blocked() {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let f = Fixture::new();
    let outside = Fixture::new();
    fs::write(outside.0.join("secret"), b"not-read").unwrap();
    symlink_dir(&outside.0, f.0.join("node_modules")).unwrap();
    symlink_file(outside.0.join("secret"), f.0.join("codex.cmd")).unwrap();
    let report = inspect(&f.0).unwrap();
    assert_eq!(report.slots[0].state, SlotState::Blocked);
    assert_eq!(report.slots[2].state, SlotState::Blocked);
    assert!(matches!(
        inspect(&f.0.join("node_modules")),
        Err(ResidualInspectionError::Unavailable)
    ));
    assert_eq!(fs::read(outside.0.join("secret")).unwrap(), b"not-read");
}

#[test]
fn codex_residual_drive_policy_is_fixed_only() {
    assert!(fixed_drive(DRIVE_FIXED));
    for kind in [
        DRIVE_UNKNOWN,
        DRIVE_NO_ROOT_DIR,
        DRIVE_REMOVABLE,
        DRIVE_REMOTE,
        DRIVE_CDROM,
        DRIVE_RAMDISK,
        u32::MAX,
    ] {
        assert!(!fixed_drive(kind));
    }
    let f = Fixture::new();
    assert!(validate(&f.0).is_ok());
}
