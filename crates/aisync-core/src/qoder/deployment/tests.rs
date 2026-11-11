use super::*;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("qoder-deploy-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn file(dir: &File, name: &str, data: &[u8]) -> File {
    let mut f = fsio::new_file(dir, name).unwrap();
    f.write_all(data).unwrap();
    f
}
#[test]
fn tiny_internal_copy_verifies_contents_but_never_sets_execute_bits() {
    let temp = Temp::new();
    let dir = fsio::directory(&temp.0).unwrap();
    let mut source = file(&dir, "source", b"authored sentinel; never run");
    let before = snapshot(&source.metadata().unwrap());
    let mut out = fsio::new_file(&dir, "output").unwrap();
    let hash = format!("{:x}", Sha256::digest(b"authored sentinel; never run"));
    copy_verified(
        &mut source,
        &mut out,
        b"authored sentinel; never run".len() as u64,
        &hash,
    )
    .unwrap();
    assert_eq!(
        fs::read(temp.0.join("output")).unwrap(),
        b"authored sentinel; never run"
    );
    assert_eq!(out.metadata().unwrap().mode() & 0o7777, 0o600);
    assert_eq!(snapshot(&source.metadata().unwrap()), before);
}
#[test]
fn bad_size_or_digest_retains_non_executable_partial_bytes() {
    for (size, hash) in [(2, "bad"), (3, "bad"), (4, "bad")] {
        let temp = Temp::new();
        let dir = fsio::directory(&temp.0).unwrap();
        let mut source = file(&dir, "source", b"abc");
        let mut out = fsio::new_file(&dir, "output").unwrap();
        assert!(copy_verified(&mut source, &mut out, size, hash).is_err());
        assert_eq!(out.metadata().unwrap().mode() & 0o7777, 0o600);
        assert!(temp.0.join("output").exists());
    }
}
#[test]
fn existing_destination_of_every_kind_is_not_absence() {
    let temp = Temp::new();
    let parent = fsio::directory(&temp.0).unwrap();
    file(&parent, "file", b"keep");
    fs::create_dir(temp.0.join("directory")).unwrap();
    symlink("missing", temp.0.join("link")).unwrap();
    for name in ["file", "directory", "link"] {
        assert!(absent(&parent, OsStr::new(name)).is_err());
        assert!(
            preview(
                distribution::reviewed_release(distribution::VERSION).unwrap(),
                &temp.0.join("invalid-cache"),
                &temp.0.join(name)
            )
            .is_err()
        );
    }
    assert_eq!(fs::read(temp.0.join("file")).unwrap(), b"keep");
}
#[test]
fn private_parent_and_source_metadata_fail_closed() {
    let temp = Temp::new();
    let dir = fsio::directory(&temp.0).unwrap();
    let source = file(&dir, "source", b"abc");
    regular(&source, 3, 0o600).unwrap();
    fs::hard_link(temp.0.join("source"), temp.0.join("other")).unwrap();
    assert!(regular(&source, 3, 0o600).is_err());
    fs::remove_file(temp.0.join("other")).unwrap();
    source
        .set_permissions(fs::Permissions::from_mode(0o700))
        .unwrap();
    assert!(regular(&source, 3, 0o600).is_err());
    fs::set_permissions(&temp.0, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(directory_identity(&dir).is_err());
    assert!(
        preview(
            distribution::reviewed_release(distribution::VERSION).unwrap(),
            &temp.0.join("cache"),
            &temp.0.join("new")
        )
        .is_err()
    );
    assert!(!temp.0.join("new").exists());
}
#[test]
fn named_replacement_and_no_follow_are_detected() {
    let temp = Temp::new();
    let dir = fsio::directory(&temp.0).unwrap();
    let old = file(&dir, "qodercli", b"abc");
    fs::rename(temp.0.join("qodercli"), temp.0.join("old")).unwrap();
    file(&dir, "qodercli", b"abc");
    assert!(verify_named(&dir, "qodercli", &old).is_err());
    fs::remove_file(temp.0.join("qodercli")).unwrap();
    symlink("old", temp.0.join("qodercli")).unwrap();
    assert!(open_payload(&dir).is_err());
}
#[test]
fn unsupported_cache_and_invalid_tokens_create_nothing() {
    let temp = Temp::new();
    let cache = temp.0.join("cache");
    fs::create_dir(&cache).unwrap();
    fs::set_permissions(&cache, fs::Permissions::from_mode(0o700)).unwrap();
    let dest = temp.0.join("new");
    assert!(
        preview(
            distribution::reviewed_release(distribution::VERSION).unwrap(),
            &cache,
            &dest
        )
        .is_err()
    );
    for token in ["", "not-preview", &"0".repeat(64)] {
        assert!(
            deploy(
                distribution::reviewed_release(distribution::VERSION).unwrap(),
                &cache,
                &dest,
                token
            )
            .is_err()
        );
        assert!(!dest.exists());
    }
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
}
#[test]
fn no_execute_claim_in_result_and_error_keeps_post_chmod_boundary() {
    assert!(
        DeploymentError::Incomplete
            .to_string()
            .contains("MAY ALREADY BE EXECUTABLE")
    );
    let report = DeploymentOutcome {
        state: "private_executable_deployed_not_activated",
        version: distribution::VERSION,
        target: staging::TARGET,
        executable_mode: "0700",
        execution_verified: false,
        activated: false,
        limitations: LIMITATIONS,
    };
    let value = serde_json::to_value(report).unwrap();
    assert_eq!(value["execution_verified"], false);
    assert_eq!(value["activated"], false);
    assert!(value.get("installed").is_none());
}
#[test]
fn completion_write_failure_never_reuses_or_deletes_existing_data() {
    let temp = Temp::new();
    let dir = fsio::directory(&temp.0).unwrap();
    let payload = file(&dir, "qodercli", b"never execute");
    payload
        .set_permissions(fs::Permissions::from_mode(0o700))
        .unwrap();
    file(&dir, "deployment.pending.json", b"existing");
    assert!(
        fsio::record(
            &dir,
            "deployment.pending.json",
            &serde_json::json!({"state":"new"})
        )
        .is_err()
    );
    assert_eq!(
        fs::read(temp.0.join("deployment.pending.json")).unwrap(),
        b"existing"
    );
    assert_eq!(payload.metadata().unwrap().mode() & 0o7777, 0o700);
}

#[test]
fn symlinked_ancestor_and_fifo_are_not_followed_or_blocked_on() {
    let temp = Temp::new();
    let real = temp.0.join("real");
    fs::create_dir(&real).unwrap();
    symlink(&real, temp.0.join("alias")).unwrap();
    assert!(fsio::directory(&temp.0.join("alias")).is_err());
    let dir = fsio::directory(&temp.0).unwrap();
    let name = fsio::name(OsStr::new("qodercli")).unwrap();
    assert_eq!(
        unsafe { libc::mkfifoat(dir.as_raw_fd(), name.as_ptr(), 0o600) },
        0
    );
    let fifo = open_payload(&dir).unwrap();
    assert!(regular(&fifo, 0, 0o600).is_err());
}

#[test]
fn chmod_output_cannot_change_source_permissions_or_inode() {
    let temp = Temp::new();
    let dir = fsio::directory(&temp.0).unwrap();
    let mut source = file(&dir, "source", b"abc");
    let before = snapshot(&source.metadata().unwrap());
    let mut out = fsio::new_file(&dir, "qodercli").unwrap();
    copy_verified(
        &mut source,
        &mut out,
        3,
        &format!("{:x}", Sha256::digest(b"abc")),
    )
    .unwrap();
    assert_ne!(
        source.metadata().unwrap().ino(),
        out.metadata().unwrap().ino()
    );
    assert_eq!(unsafe { libc::fchmod(out.as_raw_fd(), 0o700) }, 0);
    regular(&out, 3, 0o700).unwrap();
    assert_eq!(snapshot(&source.metadata().unwrap()), before);
}

fn execution_fixture(root: &Temp) -> (Selection, PathBuf, PathBuf, Contract) {
    let cache = root.0.join("cache");
    fs::create_dir(&cache).unwrap();
    fs::set_permissions(&cache, fs::Permissions::from_mode(0o700)).unwrap();
    let cache_dir = fsio::directory(&cache).unwrap();
    file(&cache_dir, "incomplete.json", b"{}");
    file(&cache_dir, "complete.json", b"{}");
    let payload = file(&cache_dir, "qodercli", b"authored-not-a-program");
    let metadata = payload.metadata().unwrap();
    let record_bytes = records(&cache_dir).unwrap();
    let selected = Selection {
        cache_dir,
        parent: fsio::directory(&root.0).unwrap(),
        payload,
        metadata,
        records: record_bytes,
        token: String::new(),
    };
    let contract = Contract {
        version: distribution::VERSION,
        bytes: b"authored-not-a-program".len() as u64,
        sha256: format!("{:x}", Sha256::digest(b"authored-not-a-program")),
        authored_fixture: true,
    };
    (selected, cache, root.0.join("new"), contract)
}
#[test]
fn production_execution_stage_success_keeps_source_private() {
    let temp = Temp::new();
    let (selected, cache, destination, contract) = execution_fixture(&temp);
    let outcome = execute(selected, &cache, &destination, &contract, |_| Ok(())).unwrap();
    assert_eq!(outcome.state, "private_executable_deployed_not_activated");
    assert_eq!(
        fs::read(destination.join("qodercli")).unwrap(),
        b"authored-not-a-program"
    );
    assert_eq!(
        fs::metadata(destination.join("qodercli")).unwrap().mode() & 0o7777,
        0o700
    );
    assert_eq!(
        fs::metadata(cache.join("qodercli")).unwrap().mode() & 0o7777,
        0o600
    );
    assert!(destination.join("deployment.complete.json").exists());
    assert!(!destination.join("deployment.pending.json").exists());
}
#[test]
fn production_execution_stage_faults_preserve_partial_without_false_success() {
    for case in [
        "source-write",
        "output-write",
        "payload-swap",
        "directory-swap",
        "post-chmod-error",
        "post-chmod-write",
        "pre-publish-write",
        "post-publish-write",
        "completion-conflict",
    ] {
        let temp = Temp::new();
        let (selected, cache, destination, contract) = execution_fixture(&temp);
        let result = execute(selected, &cache, &destination, &contract, |stage| {
            match (case, stage) {
                ("source-write", Stage::AfterCopy) => {
                    fs::write(cache.join("qodercli"), b"changed--not-a-program").unwrap()
                }
                ("output-write", Stage::AfterCopy)
                | ("post-chmod-write", Stage::AfterModeChange)
                | ("pre-publish-write", Stage::BeforePublication)
                | ("post-publish-write", Stage::AfterPublication) => {
                    fs::write(destination.join("qodercli"), b"changed--not-a-program").unwrap()
                }
                ("payload-swap", Stage::AfterCopy) => {
                    fs::rename(destination.join("qodercli"), destination.join("original")).unwrap();
                    fs::write(destination.join("qodercli"), b"authored-not-a-program").unwrap();
                    fs::set_permissions(
                        destination.join("qodercli"),
                        fs::Permissions::from_mode(0o600),
                    )
                    .unwrap();
                }
                ("directory-swap", Stage::AfterCopy) => {
                    fs::rename(&destination, temp.0.join("moved")).unwrap();
                    fs::create_dir(&destination).unwrap();
                    fs::set_permissions(&destination, fs::Permissions::from_mode(0o700)).unwrap();
                }
                ("post-chmod-error", Stage::AfterModeChange) => {
                    return Err(DeploymentError::Incomplete);
                }
                ("completion-conflict", Stage::BeforePublication) => fs::write(
                    destination.join("deployment.complete.json"),
                    b"do not overwrite",
                )
                .unwrap(),
                _ => {}
            }
            Ok(())
        });
        assert!(matches!(result, Err(DeploymentError::Incomplete)), "{case}");
        assert_eq!(
            fs::metadata(cache.join("qodercli")).unwrap().mode() & 0o7777,
            0o600
        );
        let old_dir = if case == "directory-swap" {
            temp.0.join("moved")
        } else {
            destination.clone()
        };
        let old_payload = if case == "payload-swap" {
            old_dir.join("original")
        } else {
            old_dir.join("qodercli")
        };
        let pre = matches!(
            case,
            "source-write" | "output-write" | "payload-swap" | "directory-swap"
        );
        assert_eq!(
            fs::metadata(old_payload).unwrap().mode() & 0o7777,
            if pre { 0o600 } else { 0o700 },
            "{case}"
        );
        if case == "completion-conflict" {
            assert_eq!(
                fs::read(destination.join("deployment.complete.json")).unwrap(),
                b"do not overwrite"
            );
        } else if case != "post-publish-write" {
            assert!(!old_dir.join("deployment.complete.json").exists(), "{case}");
        }
        assert!(old_dir.join("deployment.intent.json").exists());
    }
}
