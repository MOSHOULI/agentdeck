use super::*;
use std::io::Write;

fn artifact(id: NativePackage) -> Artifact {
    catalog()
        .unwrap()
        .artifacts
        .into_iter()
        .find(|a| a.id == id.id())
        .unwrap()
}
#[test]
fn all_exact_source_snapshots_and_metadata_agree() {
    let c = catalog().unwrap();
    assert_eq!(
        c.artifacts
            .iter()
            .map(|a| a.members.len())
            .collect::<Vec<_>>(),
        [49, 81, 5, 5]
    );
    let pty = &c.artifacts[0];
    assert_eq!(pty.declarations["dependencies"]["node-addon-api"], "^7.1.0");
    assert_eq!(
        pty.declarations["scripts"]["install"],
        "node scripts/prebuild.js || node-gyp rebuild"
    );
    assert!(
        pty.members
            .iter()
            .any(|m| m.path == "package/prebuilds/linux-x64/pty.node")
    );
    assert!(
        !pty.members
            .iter()
            .any(|m| m.path == "package/prebuilds/linux-x64/spawn-helper")
    );
    // Pin the documented platform distinction, not just a missing member name.
    let native = String::from_utf8_lossy(source("node-pty/src__unix__pty.cc.txt").unwrap());
    let branch = native
        .split("  int master = -1;\n#if defined(__APPLE__)")
        .nth(1)
        .unwrap();
    let (apple, other) = branch.split_once("#else").unwrap();
    assert!(apple.contains("argv[0] = strdup(helper_path.c_str())"));
    assert!(apple.contains("pty_posix_spawn(argv"));
    assert!(other.contains("argv[0] = strdup(file.c_str())"));
    assert!(other.contains("pid = forkpty(&master"));
    let koffi = &c.artifacts[1];
    assert_eq!(
        koffi.declarations["optionalDependencies"]["@koromix/koffi-linux-x64"],
        "3.1.1"
    );
    assert_eq!(
        koffi.declarations["optionalDependencies"]["@koromix/koffi-win32-x64"],
        "3.1.1"
    );
    assert_eq!(
        koffi.declarations["optionalDependencies"]
            .as_object()
            .unwrap()
            .len(),
        15
    );
    for a in &c.artifacts[2..] {
        assert!(a.declarations["scripts"].is_null());
        assert_eq!(a.declarations["cpu"], serde_json::json!(["x64"]));
        assert!(a.declarations["libc"].is_null());
    }
    assert_eq!(
        c.artifacts[2].declarations["os"],
        serde_json::json!(["linux"])
    );
    assert_eq!(
        c.artifacts[3].declarations["os"],
        serde_json::json!(["win32"])
    );
    let s = String::from_utf8_lossy(source("koffi/cnoke.cjs.txt").unwrap());
    assert!(s.contains("spawnSync)(process.execPath"));
    assert!(s.contains("require(process.argv[1])"));
    assert!(s.contains("Failed to load prebuilt binary, rebuilding from source"));
    assert!(source("unknown").is_err());
    let mut changed = artifact(NativePackage::Koffi);
    changed.declarations["scripts"]["install"] = serde_json::json!("other");
    assert!(validate_sources(&changed).is_err());
    changed = artifact(NativePackage::Koffi);
    changed.metadata_url.push_str("?latest");
    assert!(validate_sources(&changed).is_err());
}
fn checksum(h: &mut [u8]) {
    h[148..156].fill(b' ');
    let sum: usize = h.iter().map(|b| *b as usize).sum();
    h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
}
fn header(member: &Member) -> Vec<u8> {
    let mut h = vec![0; 512];
    h[..member.path.len()].copy_from_slice(member.path.as_bytes());
    h[100..108].copy_from_slice(format!("{:07o}\0", member.mode).as_bytes());
    h[124..136].copy_from_slice(format!("{:011o}\0", member.size).as_bytes());
    h[156] = b'0';
    h[257..265].copy_from_slice(b"ustar\x0000");
    checksum(&mut h);
    h
}
fn synthetic(a: &Artifact) -> (Vec<u8>, Vec<usize>) {
    let mut t = vec![];
    let mut positions = vec![];
    for m in &a.members {
        positions.push(t.len());
        t.extend(header(m));
        t.resize(t.len() + m.size.div_ceil(512) * 512, 0);
    }
    t.resize(t.len() + 1024, 0);
    (t, positions)
}
#[test]
fn bounded_structure_rejects_link_extension_mode_path_and_checksum_changes() {
    let a = artifact(NativePackage::KoffiLinuxX64);
    let member = &a.members[0];
    let valid = header(member);
    assert!(validate_header(&valid, member).is_ok());
    for kind in [
        0, b'1', b'2', b'3', b'4', b'5', b'6', b'x', b'g', b'L', b'K',
    ] {
        let mut h = valid.clone();
        h[156] = kind;
        checksum(&mut h);
        assert!(validate_header(&h, member).is_err());
    }
    for at in [100, 124, 157, 257, 345] {
        let mut h = valid.clone();
        h[at] = b'7';
        checksum(&mut h);
        assert!(validate_header(&h, member).is_err());
    }
    let mut h = valid.clone();
    h[148] ^= 1;
    assert!(validate_header(&h, member).is_err());
    for path in [
        "../file",
        "/package/file",
        "package/../file",
        "package//file",
        "package/./file",
        "package/a\\b",
        "package/",
        "package/a\nb",
    ] {
        let mut m = member.clone();
        m.path = path.into();
        assert!(validate_header(&header(&m), &m).is_err());
    }
    let mut m = member.clone();
    m.mode = 0o777;
    assert!(validate_header(&header(&m), &m).is_err());
}
#[test]
fn structural_inventory_is_exact_including_duplicates_padding_and_trailer() {
    for id in [
        NativePackage::NodePty,
        NativePackage::Koffi,
        NativePackage::KoffiLinuxX64,
        NativePackage::KoffiWin32X64,
    ] {
        let a = artifact(id);
        let (t, positions) = synthetic(&a);
        assert_eq!(scan_tar(&t, &a).unwrap().len(), a.members.len());
        let mut bad = t.clone();
        bad[positions[0] + 512 + a.members[0].size] = 1;
        assert!(scan_tar(&bad, &a).is_err());
        let mut bad = t.clone();
        *bad.last_mut().unwrap() = 1;
        assert!(scan_tar(&bad, &a).is_err());
        assert!(scan_tar(&t[..t.len() - 1], &a).is_err());
        let mut duplicate = artifact(id);
        duplicate.members[1] = duplicate.members[0].clone();
        let (bad, _) = synthetic(&duplicate);
        assert!(scan_tar(&bad, &duplicate).is_err());
        // Inert structural fixtures are not official content and cannot pass hashes.
        assert_ne!(sha(scan_tar(&t, &a).unwrap()[0]), a.members[0].sha256);
    }
}
fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(bytes).unwrap();
    gz.finish().unwrap()
}
#[test]
fn gzip_bounds_crc_truncation_and_extra_members_are_rejected() {
    let data = gzip(b"inert");
    assert_eq!(decode(&data, 5).unwrap(), b"inert");
    assert!(decode(&data, 4).is_err());
    assert!(decode(&data, 6).is_err());
    assert!(decode(&data, 32 * 1024 * 1024 + 1).is_err());
    let mut crc = data.clone();
    let at = crc.len() - 8;
    crc[at] ^= 1;
    assert!(decode(&crc, 5).is_err());
    assert!(decode(&data[..data.len() - 1], 5).is_err());
    let mut extra = data.clone();
    extra.extend(&data);
    assert!(decode(&extra, 5).is_err());
    let mut extra = data.clone();
    extra.push(0);
    assert!(decode(&extra, 5).is_err());
}
#[test]
fn selected_bytes_must_match_before_any_archive_parsing() {
    for id in [
        NativePackage::NodePty,
        NativePackage::Koffi,
        NativePackage::KoffiLinuxX64,
        NativePackage::KoffiWin32X64,
    ] {
        let a = artifact(id);
        let size = a.archive_bytes;
        for bytes in [Vec::new(), vec![0; size], vec![0; size + 1]] {
            assert!(matches!(
                verify_bytes(&bytes, artifact(id)),
                Err(VerifyError::IntegrityMismatch)
            ));
        }
    }
}
#[cfg(unix)]
#[test]
fn selected_reader_refuses_links_wrong_names_and_size_overrun_without_writes() {
    use std::{fs, os::unix::fs::symlink};
    let dir = std::env::temp_dir().join(format!("dsh-native-reader-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let a = artifact(NativePackage::Koffi);
    let path = dir.join(&a.archive_name);
    fs::write(&path, b"PRIVATE SYNTHETIC").unwrap();
    assert!(verify_native_package(NativePackage::NodePty, &path).is_err());
    let linkdir = dir.join("links");
    fs::create_dir(&linkdir).unwrap();
    let link = linkdir.join(&a.archive_name);
    symlink(&path, &link).unwrap();
    assert!(verify_native_package(NativePackage::Koffi, &link).is_err());
    fs::remove_file(&link).unwrap();
    fs::hard_link(&path, &link).unwrap();
    assert!(verify_native_package(NativePackage::Koffi, &path).is_err());
    fs::remove_file(link).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"PRIVATE SYNTHETIC");
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len((a.archive_bytes + 1) as u64)
        .unwrap();
    assert!(matches!(
        verify_native_package(NativePackage::Koffi, &path),
        Err(VerifyError::UnsafeArchive)
    ));
    fs::remove_dir_all(dir).unwrap();
}
