//! Zero-write Codex default-home configuration plan. No executable probing,
//! profile initialization, backup, writes or apply authorization.
use super::{
    BUILTIN_PROFILE_FILES, INSTRUCTIONS_START, managed_instructions_text, merge_toml_tables,
    sections_to_toml,
};
use crate::{
    inspection::reader::{NativeSelectedFileReader, validate_path},
    registry::AgentDefinition,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsStr,
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexConfigurationTarget {
    pub kind: &'static str,
    pub path: String,
    pub state: &'static str,
    pub action: &'static str,
    pub conflict_codes: Vec<&'static str>,
    pub conflict_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexConfigurationPreview {
    pub schema_version: u8,
    pub agent_id: &'static str,
    pub mode: &'static str,
    pub targets: Vec<CodexConfigurationTarget>,
    pub conflict_count: usize,
    /// Snapshot fingerprint, never an authorization token or write-readiness claim.
    pub revision: String,
    pub apply_supported: bool,
    pub uses_bundled_defaults: bool,
    pub limitations: &'static str,
}

fn unsupported() -> crate::Error {
    crate::Error::InjectFailed("Codex configuration preview unavailable: unsupported descriptor, location, or file. No configuration values are reported.".into())
}

fn valid_descriptor(agent: &AgentDefinition) -> bool {
    agent.id == "codex"
        && agent
            .config_inject
            .personalization
            .as_ref()
            .is_some_and(|p| p.template == "codex" && p.target == "~/.codex/AGENTS.md")
        && agent
            .config_inject
            .model_config
            .as_ref()
            .is_some_and(|m| m.file == "~/.codex/config.toml" && m.format == "toml_section")
}

/// Missing path handling must still inspect every existing ancestor. Native
/// bounded reader checks the actual existing file, links and handle identity.
fn read_optional(path: &Path, expected: &str) -> crate::Result<Option<Vec<u8>>> {
    validate_path(path, expected).map_err(|_| unsupported())?;
    let mut current = PathBuf::new();
    let count = path.components().count();
    for (index, component) in path.components().enumerate() {
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(unsupported()),
        };
        if metadata.file_type().is_symlink() {
            return Err(unsupported());
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes()
                & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                != 0
            {
                return Err(unsupported());
            }
        }
        if index + 1 < count && !metadata.is_dir() {
            return Err(unsupported());
        }
    }
    NativeSelectedFileReader
        .read_named(path, expected)
        .map(Some)
        .map_err(|_| unsupported())
}

fn frame(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
fn bind_file(hash: &mut Sha256, path: &Path, bytes: Option<&[u8]>) -> crate::Result<()> {
    frame(hash, path.as_os_str().as_encoded_bytes());
    match bytes {
        Some(bytes) => {
            hash.update([1]);
            frame(hash, bytes);
            let metadata = fs::symlink_metadata(path).map_err(|_| unsupported())?;
            frame(
                hash,
                format!(
                    "{:?}:{:?}",
                    metadata.modified().ok(),
                    metadata.permissions()
                )
                .as_bytes(),
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                hash.update(metadata.dev().to_le_bytes());
                hash.update(metadata.ino().to_le_bytes());
            }
        }
        None => hash.update([0]),
    }
    Ok(())
}
fn source(hash: &mut Sha256, profile: &Path, relative: &str) -> crate::Result<(String, bool)> {
    let path = profile.join(relative);
    let bytes = read_optional(
        &path,
        path.file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(unsupported)?,
    )?;
    bind_file(hash, &path, bytes.as_deref())?;
    let fallback = bytes.is_none();
    let bytes = bytes.unwrap_or_else(|| {
        BUILTIN_PROFILE_FILES
            .iter()
            .find(|(p, _)| *p == relative)
            .expect("fixed builtin source")
            .1
            .as_bytes()
            .to_vec()
    });
    frame(hash, &bytes); // Bind the selected fallback as well as source absence.
    Ok((
        String::from_utf8(bytes).map_err(|_| unsupported())?,
        fallback,
    ))
}
fn text(bytes: Option<&[u8]>) -> crate::Result<&str> {
    std::str::from_utf8(bytes.unwrap_or_default()).map_err(|_| unsupported())
}
fn target(
    kind: &'static str,
    path: &Path,
    before: Option<&[u8]>,
) -> crate::Result<CodexConfigurationTarget> {
    let path = path
        .to_str()
        .filter(|s| !s.chars().any(char::is_control))
        .ok_or_else(unsupported)?
        .to_owned();
    Ok(CodexConfigurationTarget {
        kind,
        path,
        state: if before.is_some() {
            "present"
        } else {
            "absent"
        },
        action: if before.is_some() {
            "unchanged"
        } else {
            "create"
        },
        conflict_codes: Vec::new(),
        conflict_count: 0,
    })
}
fn conflict(plan: &mut CodexConfigurationTarget, code: &'static str) {
    plan.conflict_count += 1;
    if !plan.conflict_codes.contains(&code) {
        plan.conflict_codes.push(code);
    }
}
fn toml_conflicts(
    current: &toml::Value,
    additions: &toml::Value,
    plan: &mut CodexConfigurationTarget,
) {
    if let (Some(current), Some(additions)) = (current.as_table(), additions.as_table()) {
        for (key, value) in additions {
            if let Some(old) = current.get(key) {
                toml_conflicts(old, value, plan);
            }
        }
    } else if current != additions {
        let same_type = matches!(
            (current, additions),
            (toml::Value::String(_), toml::Value::String(_))
                | (toml::Value::Integer(_), toml::Value::Integer(_))
                | (toml::Value::Float(_), toml::Value::Float(_))
                | (toml::Value::Boolean(_), toml::Value::Boolean(_))
                | (toml::Value::Datetime(_), toml::Value::Datetime(_))
                | (toml::Value::Array(_), toml::Value::Array(_))
        );
        conflict(
            plan,
            if same_type {
                "differing_toml_value"
            } else {
                "toml_type_conflict"
            },
        );
    }
}

// Legacy merger treats inline tables as values, replacing them when the
// addition is an ordinary table. Never label that structural replacement a
// conflict-free merge, even if semantic TOML table comparison finds no changes.
fn structural_conflicts(
    current: &toml_edit::Table,
    additions: &toml_edit::Table,
    plan: &mut CodexConfigurationTarget,
) {
    for (key, addition) in additions.iter() {
        if let (Some(old), Some(addition_table)) = (current.get(key), addition.as_table()) {
            if let Some(old_table) = old.as_table() {
                structural_conflicts(old_table, addition_table, plan);
            } else if old.as_value().is_some_and(|v| v.is_inline_table()) {
                conflict(plan, "toml_type_conflict");
            }
        }
    }
}

fn build(
    agent: &AgentDefinition,
    profile: &Path,
    home: &Path,
    codex_home: Option<&OsStr>,
) -> crate::Result<CodexConfigurationPreview> {
    if !valid_descriptor(agent) || !home.is_absolute() || !profile.is_absolute() {
        return Err(unsupported());
    }
    let codex_dir = home.join(".codex");
    if codex_home.is_some_and(|v| v.is_empty() || Path::new(v) != codex_dir) {
        return Err(unsupported());
    }
    let mut hash = Sha256::new();
    frame(&mut hash, b"agentdeck/codex-config-plan/1");
    frame(
        &mut hash,
        &serde_json::to_vec(agent).map_err(|_| unsupported())?,
    );
    frame(&mut hash, codex_home.unwrap_or_default().as_encoded_bytes());
    let (instructions, md_fallback) =
        source(&mut hash, profile, "personalization/codex/AGENTS.md")?;
    let (model, model_fallback) = source(&mut hash, profile, "personalization/codex.json")?;
    let model =
        crate::inspection::parse::parse_json(model.as_bytes()).map_err(|_| unsupported())?;
    let sections = model
        .get("sections")
        .filter(|v| v.is_object())
        .ok_or_else(unsupported)?;
    let additions = sections_to_toml(sections).map_err(|_| unsupported())?;
    let additions_doc: toml_edit::Document = additions.parse().map_err(|_| unsupported())?;
    let additions_value: toml::Value = additions.parse().map_err(|_| unsupported())?;
    let md_path = codex_dir.join("AGENTS.md");
    let settings_path = codex_dir.join("config.toml");
    let md_bytes = read_optional(&md_path, "AGENTS.md")?;
    let settings_bytes = read_optional(&settings_path, "config.toml")?;
    bind_file(&mut hash, &md_path, md_bytes.as_deref())?;
    bind_file(&mut hash, &settings_path, settings_bytes.as_deref())?;
    let mut md = target("instructions", &md_path, md_bytes.as_deref())?;
    let old_md = text(md_bytes.as_deref())?;
    // Reserved source markers are invalid profile input, not a target conflict.
    if instructions.contains(super::INSTRUCTIONS_START)
        || instructions.contains(super::INSTRUCTIONS_END)
    {
        return Err(unsupported());
    }
    match managed_instructions_text(old_md, &instructions) {
        Ok(output) if output == old_md => md.action = "unchanged",
        Ok(_) if old_md.contains(INSTRUCTIONS_START) => {
            md.action = "replace_managed_block";
            conflict(&mut md, "managed_block_replacement");
        }
        Ok(_) if md_bytes.is_some() => md.action = "merge",
        Ok(_) => {}
        Err(_) => {
            md.action = "conflict";
            conflict(&mut md, "ambiguous_markers");
        }
    }
    let mut settings = target("settings", &settings_path, settings_bytes.as_deref())?;
    let old_settings = text(settings_bytes.as_deref())?;
    match (
        old_settings.parse::<toml_edit::Document>(),
        old_settings.parse::<toml::Value>(),
    ) {
        (Ok(mut doc), Ok(value)) => {
            toml_conflicts(&value, &additions_value, &mut settings);
            structural_conflicts(doc.as_table(), additions_doc.as_table(), &mut settings);
            if settings.conflict_count > 0 {
                settings.action = "conflict";
            } else {
                merge_toml_tables(doc.as_table_mut(), additions_doc.as_table());
                if settings_bytes.is_some() && doc.to_string() == old_settings {
                    settings.action = "unchanged";
                } else if settings_bytes.is_some() {
                    settings.action = "merge";
                }
            }
        }
        _ => {
            settings.action = "conflict";
            conflict(&mut settings, "invalid_target_toml");
        }
    }
    Ok(CodexConfigurationPreview {
        schema_version: 1,
        agent_id: "codex",
        mode: "read_only_configure_plan",
        conflict_count: md.conflict_count + settings.conflict_count,
        targets: vec![md, settings],
        revision: format!("{:x}", hash.finalize()),
        apply_supported: false,
        uses_bundled_defaults: md_fallback || model_fallback,
        limitations: "Read-only default-home plan; no installation probe, source initialization, backup or writes. Revision is an observation, not write authorization. Managed markers do not prove ownership or absence of manual edits. No apply or restore is supported by this plan. Concurrent edits can invalidate it; this is not effective runtime configuration or a cross-process transaction.",
    })
}

pub fn preview_codex_configuration(
    agent: &AgentDefinition,
    profile: &Path,
    home: &Path,
) -> crate::Result<CodexConfigurationPreview> {
    let codex_home = std::env::var_os("CODEX_HOME");
    let first = build(agent, profile, home, codex_home.as_deref())?;
    let second = build(agent, profile, home, codex_home.as_deref())?;
    if first.revision != second.revision {
        return Err(unsupported());
    }
    Ok(first)
}

#[cfg(test)]
mod tests;
