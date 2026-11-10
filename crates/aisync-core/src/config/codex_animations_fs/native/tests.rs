static TEST_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
use super::*;
struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn setup() -> (Temp, std::path::PathBuf) {
    let dir = Temp(std::env::temp_dir().join(format!("codex-fs-{}", uuid::Uuid::new_v4())));
    std::fs::create_dir(&dir.0).unwrap();
    let p = dir.0.join(CONFIG);
    std::fs::write(&p, b"# keep\r\n[tui]\nanimations = true\r\n").unwrap();
    (dir, p)
}
#[test]
fn actual_roundtrip_preserves_exact_bytes() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let (_dir, p) = setup();
    let old = std::fs::read(&p).unwrap();
    let token = preview(&p, false).unwrap().token;
    assert!(apply(&p, false, &token, &|_| Ok(())).unwrap());
    assert!(preview(&p, true).is_err());
    assert!(restore(&p, &|_| Ok(())).unwrap());
    assert_eq!(std::fs::read(&p).unwrap(), old);
}
#[test]
fn faults_at_each_apply_boundary_recover_or_preserve_partial_receipt() {
    let _serial = TEST_SERIAL.lock().unwrap();
    for at in 0..9 {
        let (_dir, p) = setup();
        let old = std::fs::read(&p).unwrap();
        let token = preview(&p, false).unwrap().token;
        let count = std::cell::Cell::new(0);
        let result = apply(&p, false, &token, &|_| {
            let n = count.get();
            count.set(n + 1);
            if n == at { Err(Error::Io) } else { Ok(()) }
        });
        assert!(result.is_err(), "{at}");
        if at == 0 {
            assert!(restore(&p, &|_| Ok(())).is_err());
            assert!(p.parent().unwrap().join(RECEIPT).exists());
        } else {
            restore(&p, &|_| Ok(())).unwrap();
            assert!(!p.parent().unwrap().join(RECEIPT).exists());
        }
        assert_eq!(std::fs::read(&p).unwrap(), old, "{at}");
    }
}
#[test]
fn faults_at_each_restore_boundary_are_retryable() {
    let _serial = TEST_SERIAL.lock().unwrap();
    for at in 0..8 {
        let (_dir, p) = setup();
        let old = std::fs::read(&p).unwrap();
        let token = preview(&p, false).unwrap().token;
        apply(&p, false, &token, &|_| Ok(())).unwrap();
        let count = std::cell::Cell::new(0);
        let result = restore(&p, &|_| {
            let n = count.get();
            count.set(n + 1);
            if n == at { Err(Error::Io) } else { Ok(()) }
        });
        assert!(result.is_err(), "{at}");
        if p.parent().unwrap().join(RECEIPT).exists() {
            restore(&p, &|_| Ok(())).unwrap();
        }
        assert_eq!(std::fs::read(&p).unwrap(), old, "{at}");
    }
}
#[test]
fn changed_and_same_bytes_new_inode_reject_stale_preview() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let (_dir, p) = setup();
    let t = preview(&p, false).unwrap().token;
    let bytes = std::fs::read(&p).unwrap();
    let other = p.with_extension("new");
    std::fs::write(&other, bytes).unwrap();
    std::fs::rename(other, &p).unwrap();
    assert!(apply(&p, false, &t, &|_| Ok(())).is_err());
}
#[test]
fn lock_is_exclusive_and_not_removed() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let (_dir, p) = setup();
    let d = Dir::open(&p).unwrap();
    let _lock = d.lock().unwrap();
    assert!(matches!(d.lock(), Err(Error::Busy)));
    assert!(p.parent().unwrap().join(LOCK).exists());
}

#[test]
fn subprocess_fault_driver() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let Some(path) = std::env::var_os("AGENTDECK_TEST_CODEX_FS_PATH") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let at: usize = std::env::var("AGENTDECK_TEST_CODEX_FS_POINT")
        .unwrap()
        .parse()
        .unwrap();
    let count = std::cell::Cell::new(0);
    let hook = |_: Point| {
        let n = count.get();
        count.set(n + 1);
        if n == at {
            std::process::exit(77)
        }
        Ok(())
    };
    if std::env::var("AGENTDECK_TEST_CODEX_FS_OPERATION").unwrap() == "apply" {
        let token = preview(&path, false).unwrap().token;
        apply(&path, false, &token, &hook).unwrap();
    } else {
        restore(&path, &hook).unwrap();
    }
    panic!("fault point not reached");
}

#[test]
fn process_exit_at_each_persistence_boundary_leaves_recoverable_evidence() {
    let _serial = TEST_SERIAL.lock().unwrap();
    for (operation, points) in [("apply", 9), ("restore", 8)] {
        for at in 0..points {
            let (_dir, p) = setup();
            let original = std::fs::read(&p).unwrap();
            if operation == "restore" {
                let token = preview(&p, false).unwrap().token;
                apply(&p, false, &token, &|_| Ok(())).unwrap();
            }
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "config::codex_animations_fs::native::tests::subprocess_fault_driver",
                ])
                .env("AGENTDECK_TEST_CODEX_FS_PATH", &p)
                .env("AGENTDECK_TEST_CODEX_FS_POINT", at.to_string())
                .env("AGENTDECK_TEST_CODEX_FS_OPERATION", operation)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(77), "{operation}/{at}");
            if operation == "apply" && at == 0 {
                assert!(restore(&p, &|_| Ok(())).is_err());
                assert!(p.parent().unwrap().join(RECEIPT).exists());
            } else if p.parent().unwrap().join(RECEIPT).exists() {
                restore(&p, &|_| Ok(())).unwrap();
            }
            assert_eq!(std::fs::read(&p).unwrap(), original, "{operation}/{at}");
        }
    }
}

#[test]
fn recovery_observation_is_zero_write() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let (_dir, p) = setup();
    assert_eq!(recovery_preview(&p).unwrap(), RecoveryState::NoReceipt);
    assert!(!p.parent().unwrap().join(LOCK).exists());
    let token = preview(&p, false).unwrap().token;
    apply(&p, false, &token, &|_| Ok(())).unwrap();
    let before = std::fs::read(p.parent().unwrap().join(RECEIPT)).unwrap();
    assert_eq!(
        recovery_preview(&p).unwrap(),
        RecoveryState::RestoreOriginal
    );
    assert_eq!(
        std::fs::read(p.parent().unwrap().join(RECEIPT)).unwrap(),
        before
    );
}

#[test]
fn receipt_and_lock_replacements_during_apply_are_not_adopted() {
    let _serial = TEST_SERIAL.lock().unwrap();
    for name in [RECEIPT, LOCK] {
        let (_dir, p) = setup();
        let original = std::fs::read(&p).unwrap();
        let token = preview(&p, false).unwrap().token;
        let changed = std::cell::Cell::new(false);
        let result = apply(&p, false, &token, &|point| {
            if point == Point::ReceiptDirectorySynced && !changed.replace(true) {
                let dest = p.parent().unwrap().join(name);
                std::fs::remove_file(&dest).unwrap();
                std::fs::write(dest, b"foreign").unwrap();
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&p).unwrap(), original);
        assert_eq!(
            std::fs::read(p.parent().unwrap().join(name)).unwrap(),
            b"foreign"
        );
    }
}
#[test]
fn parent_move_and_target_mutation_at_prewrite_are_rejected() {
    let _serial = TEST_SERIAL.lock().unwrap();
    for move_parent in [false, true] {
        let (_dir, p) = setup();
        let token = preview(&p, false).unwrap().token;
        let moved = p.parent().unwrap().with_extension("moved");
        let fired = std::cell::Cell::new(false);
        let result = apply(&p, false, &token, &|point| {
            if point == Point::TemporarySynced && !fired.replace(true) {
                if move_parent {
                    std::fs::rename(p.parent().unwrap(), &moved).unwrap();
                } else {
                    std::fs::write(&p, b"[tui]\nanimations = true\n# external\n").unwrap();
                }
            }
            Ok(())
        });
        assert!(result.is_err());
        if move_parent {
            assert_eq!(
                std::fs::read(moved.join(CONFIG)).unwrap(),
                b"# keep\r\n[tui]\nanimations = true\r\n"
            );
            std::fs::rename(moved, p.parent().unwrap()).unwrap();
        } else {
            assert_eq!(
                std::fs::read(&p).unwrap(),
                b"[tui]\nanimations = true\n# external\n"
            );
        }
    }
}
#[test]
fn unsafe_file_kinds_and_readonly_files_fail_closed() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let _serial = TEST_SERIAL.lock().unwrap();
    for name in [CONFIG, RECEIPT, LOCK] {
        for kind in ["symlink", "hardlink", "fifo", "readonly"] {
            let (_dir, p) = setup();
            let token = preview(&p, false).unwrap().token;
            let target = p.parent().unwrap().join(name);
            if target.exists() {
                std::fs::remove_file(&target).unwrap();
            }
            let other = p.parent().unwrap().join("other");
            std::fs::write(&other, b"[tui]\nanimations=true\n").unwrap();
            match kind {
                "symlink" => symlink(&other, &target).unwrap(),
                "hardlink" => std::fs::hard_link(&other, &target).unwrap(),
                "fifo" => assert_eq!(
                    unsafe { libc::mkfifo(c(target.to_str().unwrap()).unwrap().as_ptr(), 0o600) },
                    0
                ),
                _ => {
                    std::fs::write(&target, b"").unwrap();
                    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o400))
                        .unwrap();
                }
            }
            assert!(
                apply(&p, false, &token, &|_| Ok(())).is_err(),
                "{name}/{kind}"
            );
            assert_eq!(std::fs::read(other).unwrap(), b"[tui]\nanimations=true\n");
        }
    }
}
#[test]
fn noop_never_takes_receipt_and_malformed_ownership_is_retained() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let (_dir, p) = setup();
    let t = preview(&p, true).unwrap().token;
    assert!(!apply(&p, true, &t, &|_| Ok(())).unwrap());
    let receipt = p.parent().unwrap().join(RECEIPT);
    assert!(!receipt.exists());
    std::fs::write(&receipt, b"broken").unwrap();
    assert!(restore(&p, &|_| Ok(())).is_err());
    assert!(preview(&p, false).is_err());
    assert_eq!(std::fs::read(receipt).unwrap(), b"broken");
}
#[test]
fn writable_parent_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let _serial = TEST_SERIAL.lock().unwrap();
    let (_dir, p) = setup();
    std::fs::set_permissions(p.parent().unwrap(), std::fs::Permissions::from_mode(0o777)).unwrap();
    assert!(preview(&p, false).is_err());
}

#[test]
fn foreign_temporary_content_or_inode_is_never_installed() {
    let _serial = TEST_SERIAL.lock().unwrap();
    for replace_inode in [false, true] {
        let (_dir, p) = setup();
        let original = std::fs::read(&p).unwrap();
        let token = preview(&p, false).unwrap().token;
        let foreign = std::cell::RefCell::new(None);
        let result = apply(&p, false, &token, &|point| {
            if point == Point::TemporarySynced && foreign.borrow().is_none() {
                let temp = std::fs::read_dir(p.parent().unwrap())
                    .unwrap()
                    .map(|e| e.unwrap().path())
                    .find(|p| p.extension().and_then(|s| s.to_str()) == Some("tmp"))
                    .unwrap();
                if replace_inode {
                    std::fs::remove_file(&temp).unwrap();
                }
                std::fs::write(&temp, b"foreign").unwrap();
                *foreign.borrow_mut() = Some(temp);
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&p).unwrap(), original);
        if replace_inode {
            assert_eq!(
                std::fs::read(foreign.borrow().as_ref().unwrap()).unwrap(),
                b"foreign"
            );
        }
    }
}
#[test]
fn final_receipt_sync_cannot_hide_a_changed_config() {
    let _serial = TEST_SERIAL.lock().unwrap();
    let (_dir, p) = setup();
    let token = preview(&p, false).unwrap().token;
    let count = std::cell::Cell::new(0);
    let result = apply(&p, false, &token, &|point| {
        if point == Point::DirectorySynced {
            let n = count.get();
            count.set(n + 1);
            if n == 1 {
                std::fs::write(&p, b"external").unwrap();
            }
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&p).unwrap(), b"external");
}

#[test]
fn restore_cleanup_final_state_cannot_hide_foreign_changes() {
    let _serial = TEST_SERIAL.lock().unwrap();
    for prepared in [false, true] {
        for change_receipt in [false, true] {
            let (_dir, p) = setup();
            let token = preview(&p, false).unwrap().token;
            let result = apply(&p, false, &token, &|point| {
                if prepared && point == Point::ReceiptDirectorySynced {
                    Err(Error::Io)
                } else {
                    Ok(())
                }
            });
            assert_eq!(result.is_err(), prepared);
            let dest = if change_receipt {
                p.parent().unwrap().join(RECEIPT)
            } else {
                p.clone()
            };
            let result = restore(&p, &|point| {
                if point == Point::CleanupSynced {
                    std::fs::write(&dest, b"foreign").unwrap();
                }
                Ok(())
            });
            assert!(
                result.is_err(),
                "prepared={prepared},receipt={change_receipt}"
            );
            assert_eq!(std::fs::read(dest).unwrap(), b"foreign");
        }
    }
}
