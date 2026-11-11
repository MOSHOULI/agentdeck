//! Narrow Linux npm layout association. No PATH lookup or executable invocation.
use super::*;
use crate::inspection::{parse::parse_json, reader::NativeSelectedFileReader};
use std::fs;
use std::os::unix::fs::MetadataExt;

// Compare identities and metadata as well as contents. This detects observed changes,
// not a sandbox against a same-privilege adversary changing ancestors concurrently.
#[derive(PartialEq, Eq)]
struct Stamp(u64, u64, u32, u64, i64, i64, i64, i64);
fn stamp(metadata: &fs::Metadata) -> Stamp {
    Stamp(
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        metadata.nlink(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}
fn directory_stamp(metadata: &fs::Metadata) -> Stamp {
    Stamp(
        metadata.dev(),
        metadata.ino(),
        metadata.mode(),
        0,
        0,
        0,
        0,
        0,
    )
}
fn directories(path: &Path) -> Result<Vec<(PathBuf, Stamp)>> {
    let mut current = PathBuf::new();
    let mut result = Vec::new();
    for component in path.components() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|_| StaticObservationError)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(StaticObservationError);
        }
        result.push((current.clone(), directory_stamp(&metadata)));
    }
    Ok(result)
}
fn launcher(path: &Path) -> Result<Stamp> {
    let m = fs::symlink_metadata(path).map_err(|_| StaticObservationError)?;
    if !m.file_type().is_symlink()
        || m.nlink() != 1
        || fs::read_link(path).map_err(|_| StaticObservationError)?
            != Path::new("../lib/node_modules/@openai/codex/bin/codex.js")
    {
        return Err(StaticObservationError);
    }
    Ok(stamp(&m))
}

pub(super) fn inspect(prefix: &Path, node: &Path) -> Result<CodexStaticObservation> {
    for path in [prefix, node] {
        let text = path.to_str().ok_or(StaticObservationError)?;
        if !path.is_absolute()
            || text.len() > 4096
            || text.chars().any(char::is_control)
            || path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        {
            return Err(StaticObservationError);
        }
    }
    let selected = prefix.join("bin/codex");
    let mut dirs = directories(&prefix.join("bin"))?;
    let root = prefix.join("lib/node_modules");
    let package = root.join("@openai/codex");
    dirs.extend(directories(&package.join("bin"))?);
    dirs.extend(directories(node.parent().ok_or(StaticObservationError)?)?);
    let link_before = launcher(&selected)?;
    let reader = NativeSelectedFileReader;
    let node_before = reader
        .linux_node_observation(node)
        .map_err(|_| StaticObservationError)?;
    let header = &node_before.0;
    // ELF64, little endian, current version, x86_64, executable/shared-object.
    // This is a native-format observation, not Node identity or publisher proof.
    if header.len() != 64
        || &header[..7] != b"\x7fELF\x02\x01\x01"
        || ![2, 3].contains(&u16::from_le_bytes([header[16], header[17]]))
        || &header[18..20] != b"\x3e\x00"
        || &header[20..24] != b"\x01\x00\x00\x00"
        || fs::symlink_metadata(node)
            .map_err(|_| StaticObservationError)?
            .mode()
            & 0o111
            == 0
    {
        return Err(StaticObservationError);
    }
    let manifest = package.join("package.json");
    let bin_path = package.join("bin/codex.js");
    let file_stamps = [&manifest, &bin_path, node]
        .into_iter()
        .map(|p| {
            fs::symlink_metadata(p)
                .map(|m| (p, stamp(&m)))
                .map_err(|_| StaticObservationError)
        })
        .collect::<Result<Vec<_>>>()?;
    let manifest_bytes = reader
        .read_named(&manifest, "package.json")
        .map_err(|_| StaticObservationError)?;
    let bin_bytes = reader
        .read_named(&bin_path, "codex.js")
        .map_err(|_| StaticObservationError)?;
    let value = parse_json(&manifest_bytes).map_err(|_| StaticObservationError)?;
    if value.get("bin") != Some(&serde_json::json!({"codex": "bin/codex.js"}))
        || !matches!(
            bin_bytes.split(|b| *b == b'\n').next(),
            Some(b"#!/usr/bin/env node" | b"#!/usr/bin/env node\r")
        )
    {
        return Err(StaticObservationError);
    }
    let (package_root, bin, relative_bin) =
        inspect_package_manifest(&root, "@openai/codex", "codex", &manifest_bytes)
            .map_err(|_| StaticObservationError)?;
    if bin != bin_path {
        return Err(StaticObservationError);
    }
    let search = SearchPath {
        directories: vec![prefix.join("bin")],
        suffixes: vec![String::new()],
        platform: Platform::Unix,
    };
    let mut candidates = search
        .candidates("codex")
        .map_err(|_| StaticObservationError)?;
    if candidates.len() != 1 || candidates[0].path != selected {
        return Err(StaticObservationError);
    }
    let layout = NpmLayout {
        package: "@openai/codex".into(),
        package_root,
        global_root: root,
        prefix: prefix.to_path_buf(),
        manager_path: PathBuf::new(),
        bin,
        relative_bin,
    };
    layout
        .bind(&mut candidates[0], Platform::Unix)
        .map_err(|_| StaticObservationError)?;
    if candidates[0].version.is_some() {
        return Err(StaticObservationError);
    }
    if reader
        .read_named(&manifest, "package.json")
        .map_err(|_| StaticObservationError)?
        != manifest_bytes
        || reader
            .read_named(&bin_path, "codex.js")
            .map_err(|_| StaticObservationError)?
            != bin_bytes
        || reader
            .linux_node_observation(node)
            .map_err(|_| StaticObservationError)?
            != node_before
        || launcher(&selected)? != link_before
    {
        return Err(StaticObservationError);
    }
    for (path, before) in file_stamps {
        if stamp(&fs::symlink_metadata(path).map_err(|_| StaticObservationError)?) != before {
            return Err(StaticObservationError);
        }
    }
    for (path, before) in dirs {
        let metadata = fs::symlink_metadata(path).map_err(|_| StaticObservationError)?;
        if directory_stamp(&metadata) != before {
            return Err(StaticObservationError);
        }
    }
    Ok(CodexStaticObservation {
        schema_version: 1,
        agent_id: "codex",
        observation: "selected_layout_matched",
        runtime_status: "runtime_not_run",
        search_scope: "selected_prefix_only",
        selected_launcher: "bin/codex",
        apply_supported: false,
        upgrade_supported: false,
        uninstall_supported: false,
    })
}

#[cfg(test)]
mod tests;
