//! Explicit-prefix Windows association using the existing production parsers.
//! This does not query npm, run Node/Codex, inspect the user's PATH, or authorize writes.
use super::*;
#[cfg(any(windows, test))]
use crate::inspection::reader::NativeSelectedFileReader;

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexStaticObservation {
    pub schema_version: u8,
    pub agent_id: &'static str,
    pub observation: &'static str,
    pub runtime_status: &'static str,
    pub search_scope: &'static str,
    pub selected_launcher: &'static str,
    pub apply_supported: bool,
    pub upgrade_supported: bool,
    pub uninstall_supported: bool,
}
#[derive(Debug, thiserror::Error)]
#[error("Unsupported or changed explicit Codex layout; runtime was not executed")]
pub struct StaticObservationError;

type Result<T> = std::result::Result<T, StaticObservationError>;

/// Windows-only observation of one caller-selected prefix, NOT queried npm provenance.
pub fn inspect_codex_windows_static(prefix: &Path, node: &Path) -> Result<CodexStaticObservation> {
    #[cfg(windows)]
    {
        inspect(prefix, node)
    }
    #[cfg(not(windows))]
    {
        let _ = (prefix, node);
        Err(StaticObservationError)
    }
}
#[cfg(any(windows, test))]
fn reader_path(path: &Path) -> Result<PathBuf> {
    #[cfg(windows)]
    if let Some(value) = path.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        let bytes = value.as_bytes();
        if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || &bytes[1..3] != b":\\" {
            return Err(StaticObservationError);
        }
        return Ok(PathBuf::from(value));
    }
    Ok(path.to_path_buf())
}
#[cfg(any(windows, test))]
fn read_selected(path: &Path, name: &str) -> Result<Vec<u8>> {
    NativeSelectedFileReader
        .read_named(&reader_path(path)?, name)
        .map_err(|_| StaticObservationError)
}
#[cfg(any(windows, test))]
fn inspect(prefix: &Path, node: &Path) -> Result<CodexStaticObservation> {
    for path in [prefix, node] {
        let text = path.to_str().ok_or(StaticObservationError)?;
        if !path.is_absolute()
            || text.len() > 4096
            || text.chars().any(char::is_control)
            || path.components().any(|c| matches!(c, Component::ParentDir))
        {
            return Err(StaticObservationError);
        }
    }
    read_selected(&prefix.join("codex.cmd"), "codex.cmd")?;
    let prefix = canonical_file_or_dir(prefix).map_err(|_| StaticObservationError)?;
    let selected_node = canonical_windows_program(node).map_err(|_| StaticObservationError)?;
    // The safe reader rejects reparse/linked ancestors, multiple links and unsafe
    // file types. Hashes are consistency observations, not publisher signatures.
    let reader = NativeSelectedFileReader;
    let node_before = reader
        .hash_archive(&reader_path(node)?, "node.exe")
        .map_err(|_| StaticObservationError)?;
    let mut file = std::fs::File::open(node).map_err(|_| StaticObservationError)?;
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)
        .map_err(|_| StaticObservationError)?;
    if &dos[..2] != b"MZ" {
        return Err(StaticObservationError);
    }
    let offset =
        u32::from_le_bytes(dos[60..64].try_into().map_err(|_| StaticObservationError)?) as u64;
    if !(64..=1024 * 1024).contains(&offset) {
        return Err(StaticObservationError);
    }
    use std::io::{Seek, SeekFrom};
    file.seek(SeekFrom::Start(offset))
        .map_err(|_| StaticObservationError)?;
    let mut pe = [0u8; 6];
    file.read_exact(&mut pe)
        .map_err(|_| StaticObservationError)?;
    if &pe[..4] != b"PE\0\0" || u16::from_le_bytes([pe[4], pe[5]]) != 0x8664 {
        return Err(StaticObservationError);
    }
    let search = SearchPath {
        directories: vec![prefix.clone()],
        suffixes: windows_suffixes(".COM;.EXE;.BAT;.CMD").map_err(|_| StaticObservationError)?,
        platform: Platform::Windows,
    };
    let candidates = search
        .candidates("codex")
        .map_err(|_| StaticObservationError)?;
    if candidates.len() != 1 || candidates[0].path != prefix.join("codex.cmd") {
        return Err(StaticObservationError);
    }
    let mut instance = candidates
        .into_iter()
        .next()
        .ok_or(StaticObservationError)?;
    // Official npm has POSIX/PowerShell companions. Neither is an active candidate
    // under the fixed standard PATHEXT above; their content is NOT authenticated here.
    let mut snapshots = Vec::new();
    for name in ["codex", "codex.cmd", "codex.ps1"] {
        snapshots.push((
            prefix.join(name),
            name.to_owned(),
            read_selected(&prefix.join(name), name)?,
        ));
    }
    let root =
        canonical_file_or_dir(&prefix.join("node_modules")).map_err(|_| StaticObservationError)?;
    if root != prefix.join("node_modules") {
        return Err(StaticObservationError);
    }
    let manifest = root.join("@openai/codex/package.json");
    snapshots.push((
        manifest.clone(),
        "package.json".into(),
        read_selected(&manifest, "package.json")?,
    ));
    let (package_root, bin, relative_bin) =
        inspect_package(&root, "@openai/codex", "codex").map_err(|_| StaticObservationError)?;
    let name = bin
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or(StaticObservationError)?
        .to_owned();
    snapshots.push((bin.clone(), name.clone(), read_selected(&bin, &name)?));
    let parsed = windows_entrypoint(&instance, "@openai/codex", "codex")
        .map_err(|_| StaticObservationError)?;
    if parsed != bin {
        return Err(StaticObservationError);
    }
    let node_search = SearchPath {
        directories: vec![
            selected_node
                .parent()
                .ok_or(StaticObservationError)?
                .to_path_buf(),
        ],
        suffixes: vec![".exe".into()],
        platform: Platform::Windows,
    };
    if node_search
        .node_for(&prefix)
        .map_err(|_| StaticObservationError)?
        != selected_node
    {
        return Err(StaticObservationError);
    }
    // Share the production binding checks but do not expose its Npm source enum:
    // this prefix is explicit input, not a prefix/root response from an npm process.
    let layout = NpmLayout {
        package: "@openai/codex".into(),
        package_root,
        global_root: root,
        prefix: prefix.clone(),
        manager_path: PathBuf::new(),
        bin,
        relative_bin,
    };
    layout
        .bind(&mut instance, Platform::Windows)
        .map_err(|_| StaticObservationError)?;
    if instance.version.is_some() {
        return Err(StaticObservationError);
    }
    for (path, name, bytes) in snapshots {
        if read_selected(&path, &name)? != bytes {
            return Err(StaticObservationError);
        }
    }
    if reader
        .hash_archive(&reader_path(node)?, "node.exe")
        .map_err(|_| StaticObservationError)?
        != node_before
    {
        return Err(StaticObservationError);
    }
    Ok(CodexStaticObservation {
        schema_version: 1,
        agent_id: "codex",
        observation: "selected_layout_matched",
        runtime_status: "runtime_not_run",
        search_scope: "selected_prefix_only",
        selected_launcher: "codex.cmd",
        apply_supported: false,
        upgrade_supported: false,
        uninstall_supported: false,
    })
}
#[cfg(test)]
mod tests;
