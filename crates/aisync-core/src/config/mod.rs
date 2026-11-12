//! 配置注入适配器模块（M2）。
//!
//! 适配器模式：统一接口 → 各格式 Writer。
//! 支持格式：TOML section / JSON merge / JSONC merge / YAML section / plain text。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::registry::AgentDefinition;

pub mod codex_animations;
pub mod codex_animations_fs;
pub mod codex_animations_observation;
mod codex_preview;
mod destination;
mod metadata;
#[cfg(all(test, target_os = "linux"))]
mod metadata_tests;
#[cfg(any(windows, test))]
mod windows_acl_policy;
#[cfg(all(test, windows))]
mod windows_metadata_tests;
#[cfg(windows)]
mod windows_security;
pub use codex_preview::{
    CodexConfigurationPreview, CodexConfigurationTarget, preview_codex_configuration,
};

// Shared with MCP writers: config read/modify/write and transaction rollback
// must not interleave within this process.
static CONFIG_WRITE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(crate) fn write_lock() -> std::sync::MutexGuard<'static, ()> {
    CONFIG_WRITE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    TomlSection,
    JsonMerge,
    JsoncMerge,
    YamlSection,
    PlainText,
}

/// 单条注入结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectStatus {
    Ok,
    Skipped(String),
}

/// 单个 agent 的注入报告
#[derive(Debug, Clone)]
pub struct InjectReport {
    pub agent_id: String,
    pub personalization: Option<InjectStatus>,
    pub model_config: Option<InjectStatus>,
}

const BUILTIN_PROFILE_FILES: &[(&str, &str)] = &[
    (
        "personalization/codex/AGENTS.md",
        include_str!("../../../../profiles/default/personalization/codex/AGENTS.md"),
    ),
    (
        "personalization/opencode/AGENTS.md",
        include_str!("../../../../profiles/default/personalization/opencode/AGENTS.md"),
    ),
    (
        "personalization/claude/AGENTS.md",
        include_str!("../../../../profiles/default/personalization/claude/AGENTS.md"),
    ),
    (
        "personalization/cline/AGENTS.md",
        include_str!("../../../../profiles/default/personalization/cline/AGENTS.md"),
    ),
    (
        "personalization/hermes/AGENTS.md",
        include_str!("../../../../profiles/default/personalization/hermes/AGENTS.md"),
    ),
    (
        "personalization/gemini/AGENTS.md",
        include_str!("../../../../profiles/default/personalization/gemini/AGENTS.md"),
    ),
    (
        "personalization/zcode/AGENTS.md",
        include_str!("../../../../profiles/default/personalization/zcode/AGENTS.md"),
    ),
    (
        "personalization/codex.json",
        include_str!("../../../../profiles/default/personalization/codex.json"),
    ),
    (
        "personalization/opencode/opencode.jsonc",
        include_str!("../../../../profiles/default/personalization/opencode/opencode.jsonc"),
    ),
];

/// 返回 AgentDeck 默认 profile；缺失文件会从内置模板初始化，已有文件保持不变。
pub fn profile_source_dir() -> crate::Result<PathBuf> {
    let home = crate::home_dir().ok_or_else(|| crate::Error::Other("无法定位用户主目录".into()))?;
    profile_source_dir_in(&home)
}

fn profile_source_dir_in(home: &Path) -> crate::Result<PathBuf> {
    let profile = home.join(".agentdeck").join("profiles").join("default");
    for (relative, content) in BUILTIN_PROFILE_FILES {
        let target = profile.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        create_profile_file_with(&target, |file| file.write_all(content.as_bytes()))?;
    }
    Ok(profile)
}

fn create_profile_file_with(
    target: &Path,
    write: impl FnOnce(&mut fs::File) -> std::io::Result<()>,
) -> crate::Result<()> {
    match OpenOptions::new().write(true).create_new(true).open(target) {
        Ok(mut file) => {
            if let Err(error) = write(&mut file) {
                drop(file);
                let _ = fs::remove_file(target);
                return Err(error.into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn expand_home_in(path: &str, home: &Path) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        return home.join(rest);
    }
    PathBuf::from(path)
}

fn expand_home(path: &str) -> PathBuf {
    crate::home_dir()
        .map(|home| expand_home_in(path, &home))
        .unwrap_or_else(|| PathBuf::from(path))
}

fn backup_file_in(path: &Path, home: &Path) -> crate::Result<Option<PathBuf>> {
    destination::check(path)?;
    if !path.is_file() {
        return Ok(None);
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let backup_root = home
        .join(".agentdeck")
        .join("backups")
        .join(format!("inject-{}", nanos));
    fs::create_dir_all(&backup_root)?;
    let name = path
        .file_name()
        .ok_or_else(|| crate::Error::Other("目标路径无文件名".into()))?;
    let dest = backup_root.join(name);
    #[cfg(any(target_os = "linux", windows))]
    {
        #[cfg(windows)]
        let security = windows_security::Security::read_path(path)?;
        let mut created = false;
        let result = (|| -> crate::Result<()> {
            let mut output = create_staging_file(&dest, &mut created)?;
            #[cfg(windows)]
            security.apply_before_content(&output)?;
            destination::check(path)?;
            let mut input = fs::File::open(path)?;
            #[cfg(windows)]
            security.verify_file(&input)?;
            metadata::check_file(&input)?;
            output.set_permissions(input.metadata()?.permissions())?;
            metadata::check_file(&output)?;
            std::io::copy(&mut input, &mut output)?;
            metadata::check_file(&input)?;
            metadata::check_file(&output)?;
            destination::check(path)?;
            #[cfg(windows)]
            {
                security.verify_file(&input)?;
                security.verify_file(&output)?;
                security.verify_path(path)?;
                security.verify_path(&dest)?;
            }
            Ok(())
        })();
        if result.is_err() && created {
            let _ = fs::remove_file(&dest);
        }
        result?;
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    fs::copy(path, &dest)?;
    Ok(Some(dest))
}

fn backup_file(path: &Path) -> crate::Result<Option<PathBuf>> {
    destination::check(path)?;
    if !path.is_file() {
        return Ok(None);
    }
    let home = crate::home_dir().ok_or_else(|| crate::Error::Other("无法定位用户主目录".into()))?;
    backup_file_in(path, &home)
}

#[derive(Debug)]
struct WriteEvidence {
    #[cfg(windows)]
    security: windows_security::Security,
}

fn write_file(path: &Path, content: &str) -> crate::Result<WriteEvidence> {
    write_bytes(path, content.as_bytes())
}

fn write_bytes(path: &Path, content: &[u8]) -> crate::Result<WriteEvidence> {
    destination::check(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    #[cfg(windows)]
    let security = if path.exists() {
        Some(windows_security::Security::read_path(path)?)
    } else {
        None
    };
    let temporary = path.with_file_name(format!(".agentdeck-{}.tmp", uuid::Uuid::new_v4()));
    let mut created = false;
    let result = (|| -> crate::Result<WriteEvidence> {
        let mut file = create_staging_file(&temporary, &mut created)?;
        #[cfg(windows)]
        if let Some(security) = &security {
            security.apply_before_content(&file)?;
        }
        if let Ok(metadata) = fs::metadata(path) {
            fs::set_permissions(&temporary, metadata.permissions())?;
        }
        metadata::check_file(&file)?;
        file.write_all(content)?;
        file.sync_all()?;
        metadata::check_file(&file)?;
        #[cfg(windows)]
        if let Some(security) = &security {
            security.verify_path(path)?;
            security.verify_path(&temporary)?;
        }
        #[cfg(windows)]
        let security = windows_security::Security::read(&file)?;
        drop(file);
        replace_staged_file(path, &temporary)?;
        Ok(WriteEvidence {
            #[cfg(windows)]
            security,
        })
    })();
    if result.is_err() && created {
        let _ = fs::remove_file(&temporary);
    }
    result
}

// Check inherited metadata before chmod or content writes. The injectable creator
// keeps the same production guard testable without requiring filesystem ACL support.
fn checked_staging_file(
    create: impl FnOnce() -> std::io::Result<fs::File>,
) -> crate::Result<fs::File> {
    let file = create()?;
    metadata::check_file(&file)?;
    Ok(file)
}

fn create_staging_file(path: &Path, created: &mut bool) -> crate::Result<fs::File> {
    checked_staging_file(|| {
        #[cfg(windows)]
        let file = windows_security::create(path)?;
        #[cfg(not(windows))]
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        *created = true;
        Ok(file)
    })
}

fn replace_staged_file(path: &Path, temporary: &Path) -> crate::Result<()> {
    destination::check(path)?;
    #[cfg(target_os = "linux")]
    destination::check(temporary)?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MoveFileExW};
        let src: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(src.as_ptr(), dst.as_ptr(), MOVEFILE_REPLACE_EXISTING) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        fs::rename(temporary, path)?;
        Ok(())
    }
}

/// 去掉 JSONC 的 // 与 /* */ 注释（字符串内不处理）
pub(crate) fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut in_line = false;
    let mut in_block = false;
    while let Some(c) = chars.next() {
        if in_line {
            if c == '\n' {
                in_line = false;
                out.push(c);
            }
            continue;
        }
        if in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            }
            continue;
        }
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                in_line = true;
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                in_block = true;
            }
            _ => out.push(c),
        }
    }
    out
}

fn merge_json(base: &mut Value, additions: Value) {
    if let (Value::Object(base_map), Value::Object(add_map)) = (base, additions) {
        for (key, value) in add_map {
            if let Some(existing) = base_map.get_mut(&key)
                && existing.is_object()
                && value.is_object()
            {
                merge_json(existing, value);
                continue;
            }
            base_map.insert(key, value);
        }
    }
}

/// Serialize section objects through TOML rather than interpolating keys/strings.
fn sections_to_toml(sections: &Value) -> crate::Result<String> {
    let value = toml::Value::try_from(sections)
        .map_err(|e| crate::Error::InjectFailed(format!("Invalid TOML profile: {e}")))?;
    toml::to_string(&value)
        .map_err(|e| crate::Error::InjectFailed(format!("Invalid TOML profile: {e}")))
}

/// 把注入内容合并进目标 TOML（逐 key 合并，保留目标文件的注释与顺序）
fn merge_toml_tables(target: &mut toml_edit::Table, additions: &toml_edit::Table) {
    for (key, item) in additions.iter() {
        if let Some(add_table) = item.as_table() {
            let entry = target
                .entry(key)
                .or_insert(toml_edit::Item::Table(toml_edit::Table::new()));
            if let Some(target_table) = entry.as_table_mut() {
                merge_toml_tables(target_table, add_table);
            } else {
                *entry = item.clone();
            }
        } else {
            target.insert(key, item.clone());
        }
    }
}

/// 合并 TOML section（不产生重复段，不触碰其他内容）
fn patch_toml_section(target: &Path, section_text: &str) -> crate::Result<WriteEvidence> {
    destination::check(target)?;
    let mut doc: toml_edit::Document = if target.exists() {
        let content = fs::read_to_string(target)?;
        content
            .parse()
            .map_err(|e| crate::Error::InjectFailed(format!("TOML 解析失败: {}", e)))?
    } else {
        toml_edit::Document::new()
    };
    let additions: toml_edit::Document = section_text
        .parse()
        .map_err(|e| crate::Error::InjectFailed(format!("TOML 注入内容解析失败: {}", e)))?;

    merge_toml_tables(doc.as_table_mut(), additions.as_table());
    write_file(target, &doc.to_string())
}

fn merge_yaml(target: &Path, additions: &str) -> crate::Result<WriteEvidence> {
    destination::check(target)?;
    let base: Value = if target.exists() {
        let text = fs::read_to_string(target)?;
        if text.trim().is_empty() {
            Value::Null
        } else {
            serde_yml::from_str(&text)
                .map_err(|e| crate::Error::InjectFailed(format!("YAML 解析失败: {}", e)))?
        }
    } else {
        Value::Null
    };
    let additions: Value = serde_yml::from_str(additions)
        .map_err(|e| crate::Error::InjectFailed(format!("YAML 注入内容解析失败: {}", e)))?;
    let mut merged = if base.is_null() {
        Value::Object(Default::default())
    } else {
        base
    };
    if !merged.is_object() || !additions.is_object() {
        return Err(crate::Error::InjectFailed(
            "YAML merge requires mapping objects".into(),
        ));
    }
    merge_json(&mut merged, additions);
    let out = serde_yml::to_string(&merged)
        .map_err(|e| crate::Error::InjectFailed(format!("YAML 序列化失败: {}", e)))?;
    write_file(target, &out)
}

fn inject_with_backup(
    format: Format,
    target_path: &Path,
    content: &str,
    _backup: Option<PathBuf>,
) -> crate::Result<WriteEvidence> {
    destination::check(target_path)?;
    let result = match format {
        Format::PlainText => write_file(target_path, content),
        Format::JsonMerge | Format::JsoncMerge => {
            let text = if target_path.exists() {
                fs::read_to_string(target_path)?
            } else {
                String::new()
            };
            let cleaned = strip_jsonc_comments(&text);
            let mut base: Value = if cleaned.trim().is_empty() {
                Value::Object(Default::default())
            } else {
                serde_json::from_str(&cleaned)
                    .map_err(|e| crate::Error::InjectFailed(format!("JSON 解析失败: {}", e)))?
            };
            let cleaned_additions = if format == Format::JsoncMerge {
                strip_jsonc_comments(content)
            } else {
                content.to_owned()
            };
            let additions: Value = serde_json::from_str(&cleaned_additions)
                .map_err(|e| crate::Error::InjectFailed(format!("注入内容解析失败: {}", e)))?;
            if !base.is_object() || !additions.is_object() {
                return Err(crate::Error::InjectFailed(
                    "JSON merge requires objects".into(),
                ));
            }
            merge_json(&mut base, additions);
            let out = serde_json::to_string_pretty(&base)
                .map_err(|e| crate::Error::InjectFailed(format!("JSON 序列化失败: {}", e)))?;
            write_file(target_path, &out)
        }
        Format::TomlSection => patch_toml_section(target_path, content),
        Format::YamlSection => merge_yaml(target_path, content),
    };

    // Each writer stages bytes beside the destination, then replaces it as its
    // final fallible step. Do not speculatively rewrite a failed destination:
    // pre-replacement failures leave it untouched, while uncertain filesystem
    // replacement outcomes must be inspected rather than called rolled back.
    result
}

/// 注入配置到目标文件（先备份，失败自动回滚）
pub fn inject(format: Format, target_path: &Path, content: &str) -> crate::Result<()> {
    let _guard = write_lock();
    let backup = backup_file(target_path)?;
    inject_with_backup(format, target_path, content, backup).map(|_| ())
}

fn inject_in_home(
    format: Format,
    target_path: &Path,
    content: &str,
    home: &Path,
) -> crate::Result<WriteEvidence> {
    let backup = backup_file_in(target_path, home)?;
    inject_with_backup(format, target_path, content, backup)
}

fn target_path(path: &str, target_home: Option<&Path>) -> PathBuf {
    target_home
        .map(|home| expand_home_in(path, home))
        .unwrap_or_else(|| expand_home(path))
}

fn inject_target(
    format: Format,
    target: &Path,
    content: &str,
    target_home: Option<&Path>,
) -> crate::Result<WriteEvidence> {
    match target_home {
        Some(home) => inject_in_home(format, target, content, home),
        None => {
            let backup = backup_file(target)?;
            inject_with_backup(format, target, content, backup)
        }
    }
}

const INSTRUCTIONS_START: &str = "<!-- agentdeck:personalization:start -->";
const INSTRUCTIONS_END: &str = "<!-- agentdeck:personalization:end -->";

/// Replace only our managed block; never replace user-authored instructions.
fn managed_instructions(target: &Path, template: &str) -> crate::Result<String> {
    validate_instruction_template(template)?;
    destination::check(target)?;
    let existing = match fs::read_to_string(target) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    managed_instructions_text(&existing, template)
}

fn validate_instruction_template(template: &str) -> crate::Result<()> {
    if template.contains(INSTRUCTIONS_START) || template.contains(INSTRUCTIONS_END) {
        return Err(crate::Error::InjectFailed(
            "Instruction template contains reserved AgentDeck markers".into(),
        ));
    }
    Ok(())
}

fn managed_instructions_text(existing: &str, template: &str) -> crate::Result<String> {
    validate_instruction_template(template)?;
    let block = format!(
        "{INSTRUCTIONS_START}\n{}\n{INSTRUCTIONS_END}",
        template.trim_end()
    );
    let starts: Vec<_> = existing.match_indices(INSTRUCTIONS_START).collect();
    let ends: Vec<_> = existing.match_indices(INSTRUCTIONS_END).collect();
    match (starts.as_slice(), ends.as_slice()) {
        ([], []) if existing.is_empty() => Ok(format!("{block}\n")),
        ([], []) => Ok(format!("{existing}\n\n{block}\n")),
        ([(start, _)], [(end, _)]) if start < end => {
            let after = end + INSTRUCTIONS_END.len();
            Ok(format!(
                "{}{}{}",
                &existing[..*start],
                block,
                &existing[after..]
            ))
        }
        _ => Err(crate::Error::InjectFailed(
            "Instruction file has ambiguous or incomplete AgentDeck markers; left unchanged".into(),
        )),
    }
}

/// 模板注入内容源：personalization/<template>/AGENTS.md
fn template_agents_md(source_dir: &Path, template: &str) -> Option<String> {
    let path = source_dir
        .join("personalization")
        .join(template)
        .join("AGENTS.md");
    fs::read_to_string(path).ok()
}

/// model_config 注入内容源（按 agent 特化；缺失返回 None）
fn model_config_content(
    agent: &AgentDefinition,
    source_dir: &Path,
) -> crate::Result<Option<String>> {
    let relative = match agent.id.as_str() {
        "codex" => "personalization/codex.json",
        _ => return Ok(None),
    };
    let raw = match fs::read_to_string(source_dir.join(relative)) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if agent.id == "codex" {
        let value: Value = serde_json::from_str(&raw)
            .map_err(|e| crate::Error::InjectFailed(format!("Invalid Codex profile: {e}")))?;
        let sections = value
            .get("sections")
            .filter(|v| v.is_object())
            .ok_or_else(|| {
                crate::Error::InjectFailed("Codex profile requires a sections object".into())
            })?;
        return sections_to_toml(sections).map(Some);
    }
    Ok(Some(raw))
}

/// Whether this agent has a personalization destination handled by our adapter.
/// This is a non-mutating capability check; profile content is validated on apply.
pub fn supports_personalization(agent: &AgentDefinition) -> bool {
    agent
        .config_inject
        .personalization
        .as_ref()
        .is_some_and(|p| {
            !p.target.contains("://")
                && (p.target.starts_with("~/") || p.target.starts_with("~\\"))
                && p.target.ends_with(".md")
        })
}

// Only called after adapter, source and destination eligibility checks. Each
// writer performs one replacement and has no fallible steps after it succeeds.
// Keep that contract when adding writers: errors with uncertain commit outcomes
// require explicit recovery handling, not an assumption that nothing was written.
struct JournalEntry {
    path: PathBuf,
    before: Option<Vec<u8>>,
    #[cfg(windows)]
    security: Option<windows_security::Security>,
    #[cfg(windows)]
    committed_security: windows_security::Security,
}

fn inject_journaled(
    format: Format,
    target: &Path,
    content: &str,
    target_home: Option<&Path>,
    committed: &mut Vec<JournalEntry>,
) -> crate::Result<()> {
    destination::check(target)?;
    let before = match fs::read(target) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.into()),
    };
    #[cfg(windows)]
    let security = if before.is_some() {
        Some(windows_security::Security::read_path(target)?)
    } else {
        None
    };
    let evidence = inject_target(format, target, content, target_home)?;
    committed.push(JournalEntry {
        path: target.to_path_buf(),
        before,
        #[cfg(windows)]
        security,
        #[cfg(windows)]
        committed_security: evidence.security,
    });
    #[cfg(not(windows))]
    let _ = evidence;
    Ok(())
}

fn inject_for_agent_apply(
    agent: &AgentDefinition,
    source_dir: &Path,
    target_home: Option<&Path>,
    committed: &mut Vec<JournalEntry>,
) -> crate::Result<InjectReport> {
    let mut personalization = None;
    let mut model_config = None;

    if let Some(p) = &agent.config_inject.personalization {
        if p.target.contains("://") {
            personalization = Some(InjectStatus::Skipped("特殊通道暂不支持".into()));
        } else if !p.target.starts_with("~/") && !p.target.starts_with("~\\") {
            personalization = Some(InjectStatus::Skipped("非绝对路径目标暂不支持".into()));
        } else if !p.target.ends_with(".md") {
            // An instruction markdown template must never overwrite structured settings.
            personalization = Some(InjectStatus::Skipped("目标不是 Markdown 指令文件".into()));
        } else if let Some(content) = template_agents_md(source_dir, &p.template) {
            let target = target_path(&p.target, target_home);
            let content = managed_instructions(&target, &content)?;
            match inject_journaled(Format::PlainText, &target, &content, target_home, committed) {
                Ok(()) => personalization = Some(InjectStatus::Ok),
                Err(e) => return Err(e),
            }
        } else {
            personalization = Some(InjectStatus::Skipped(format!(
                "模板缺失: personalization/{}/AGENTS.md",
                p.template
            )));
        }
    }

    if let Some(mc) = &agent.config_inject.model_config {
        if agent.id == "opencode" {
            // Retained runtime registries may still advertise the old unvalidated
            // JSON merge adapter. Neither its profile nor its target is safe to use
            // until an adapter validates the supported OpenCode schema.
            model_config = Some(InjectStatus::Skipped(
                "OpenCode model configuration adapter is not supported".into(),
            ));
        } else if mc.file.contains("://")
            || !(mc.file.starts_with("~/") || mc.file.starts_with("~\\"))
        {
            model_config = Some(InjectStatus::Skipped("特殊通道暂不支持".into()));
        } else if let Some(content) = model_config_content(agent, source_dir)? {
            let format = match mc.format.as_str() {
                "json_merge" => Some(Format::JsonMerge),
                "jsonc_merge" => Some(Format::JsoncMerge),
                "toml_section" => Some(Format::TomlSection),
                "yaml_section" => Some(Format::YamlSection),
                other => {
                    model_config = Some(InjectStatus::Skipped(format!("未知格式: {}", other)));
                    None
                }
            };
            if let Some(format) = format {
                let target = target_path(&mc.file, target_home);
                match inject_journaled(format, &target, &content, target_home, committed) {
                    Ok(()) => model_config = Some(InjectStatus::Ok),
                    Err(e) => return Err(e),
                }
            }
        } else {
            model_config = Some(InjectStatus::Skipped("缺少配置内容源".into()));
        }
    }

    Ok(InjectReport {
        agent_id: agent.id.clone(),
        personalization,
        model_config,
    })
}

fn rollback_target(path: &Path, before: Option<Vec<u8>>) -> crate::Result<()> {
    destination::check(path)?;
    match before {
        Some(bytes) => write_bytes(path, &bytes).map(|_| ()),
        None if path.is_file() => fs::remove_file(path).map_err(crate::Error::from),
        None => Ok(()),
    }
}

#[cfg(windows)]
fn rollback_security_checked(
    path: &Path,
    before: Option<Vec<u8>>,
    security: Option<windows_security::Security>,
    committed: windows_security::Security,
) -> crate::Result<()> {
    destination::check(path)?;
    committed.verify_path(path)?;
    if let Some(original) = security
        && original != committed
    {
        return Err(crate::Error::InjectFailed(
            "Rollback refused: Windows DACL changed during configuration transaction".into(),
        ));
    }
    rollback_target(path, before)
}

/// Journal only completed replacements. Validation failures and skipped adapters
/// must not cause an untouched destination to be rewritten during rollback.
fn inject_for_agent_with_home(
    agent: &AgentDefinition,
    source_dir: &Path,
    target_home: Option<&Path>,
) -> crate::Result<InjectReport> {
    let _guard = write_lock();
    let mut committed = Vec::new();
    let result = inject_for_agent_apply(agent, source_dir, target_home, &mut committed);
    if let Err(error) = result {
        let mut failures = Vec::new();
        for entry in committed.into_iter().rev() {
            let path = entry.path;
            #[cfg(windows)]
            let restore = rollback_security_checked(
                &path,
                entry.before,
                entry.security,
                entry.committed_security,
            );
            #[cfg(not(windows))]
            let restore = rollback_target(&path, entry.before);
            if let Err(e) = restore {
                failures.push(format!("{}: {}", path.display(), e));
            }
        }
        if !failures.is_empty() {
            return Err(crate::Error::InjectFailed(format!(
                "{}; rollback failed: {}",
                error,
                failures.join("; ")
            )));
        }
        return Err(error);
    }
    result
}

/// 为单个 agent 执行个性化 + model_config 注入，返回逐项报告
pub fn inject_for_agent(agent: &AgentDefinition, source_dir: &Path) -> crate::Result<InjectReport> {
    inject_for_agent_with_home(agent, source_dir, None)
}

#[cfg(test)]
fn inject_for_agent_in_home(
    agent: &AgentDefinition,
    source_dir: &Path,
    target_home: &Path,
) -> crate::Result<InjectReport> {
    inject_for_agent_with_home(agent, source_dir, Some(target_home))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("agentdeck-{}-{}", tag, nanos));
        fs::create_dir_all(&dir).unwrap();
        // macOS temporary directories can have platform-owned linked ancestors.
        fs::canonicalize(dir).unwrap()
    }

    fn temp_home_target(tag: &str, file_name: &str) -> (PathBuf, PathBuf) {
        let home = temp_dir(tag);
        let target = home.join("target").join(file_name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        assert!(target.starts_with(&home), "测试目标必须位于临时 home");
        (home, target)
    }

    fn assert_temp_backup_exists(home: &Path, file_name: &str) {
        let backups = home.join(".agentdeck").join("backups");
        assert!(backups.starts_with(home), "测试备份必须位于临时 home");
        let found = fs::read_dir(backups)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .any(|entry| entry.path().join(file_name).is_file());
        assert!(found, "临时 home 中应存在 {} 的备份", file_name);
    }

    #[test]
    fn json_merge_updates_and_preserves_other_keys() {
        let (home, target) = temp_home_target("json-home", "settings.json");
        fs::write(&target, r#"{ "a": 1, "nested": { "x": 1 } }"#).unwrap();

        inject_in_home(
            Format::JsonMerge,
            &target,
            r#"{ "a": 2, "b": 3, "nested": { "y": 2 } }"#,
            &home,
        )
        .unwrap();

        let value: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(value["a"], 2);
        assert_eq!(value["b"], 3);
        assert_eq!(value["nested"]["x"], 1);
        assert_eq!(value["nested"]["y"], 2);
        assert_temp_backup_exists(&home, "settings.json");
    }

    #[test]
    fn jsonc_merge_strips_comments() {
        let (home, target) = temp_home_target("jsonc-home", "config.jsonc");
        fs::write(&target, "// 头部注释\n{ \"a\": 1 /* 行内 */ }\n// 尾部").unwrap();

        inject_in_home(Format::JsoncMerge, &target, r#"{ "b": 2 }"#, &home).unwrap();

        let value: Value = serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(value["a"], 1);
        assert_eq!(value["b"], 2);
        assert_temp_backup_exists(&home, "config.jsonc");
    }

    #[test]
    fn toml_section_appends_then_replaces() {
        let (home, target) = temp_home_target("toml-home", "config.toml");
        fs::write(&target, "[existing]\nkeep = true\n").unwrap();

        inject_in_home(
            Format::TomlSection,
            &target,
            "[features]\nmemories = true\njs_repl = false\n",
            &home,
        )
        .unwrap();
        let first = fs::read_to_string(&target).unwrap();
        assert!(first.contains("[features]"));
        assert!(first.contains("memories = true"));
        assert!(first.contains("[existing]"));
        assert_eq!(first.matches("[features]").count(), 1, "不应产生重复段");

        inject_in_home(
            Format::TomlSection,
            &target,
            "[features]\nmemories = false\n",
            &home,
        )
        .unwrap();
        let second = fs::read_to_string(&target).unwrap();
        assert!(second.contains("memories = false"));
        assert!(!second.contains("memories = true"));
        assert!(second.contains("[existing]"));
        assert!(second.contains("keep = true"));
        assert_eq!(second.matches("[features]").count(), 1);

        let parsed: toml::Value = toml::from_str(&second).expect("合并后 TOML 应可解析");
        assert_eq!(parsed["features"]["memories"], toml::Value::Boolean(false));
        assert_eq!(parsed["existing"]["keep"], toml::Value::Boolean(true));
        assert_temp_backup_exists(&home, "config.toml");
    }

    #[test]
    fn yaml_section_merges() {
        let (home, target) = temp_home_target("yaml-home", "config.yaml");
        fs::write(&target, "a: 1\n").unwrap();

        inject_in_home(Format::YamlSection, &target, "a: 3\nb: 2\n", &home).unwrap();

        let value: Value = serde_yml::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(value["a"], 3);
        assert_eq!(value["b"], 2);
        assert_temp_backup_exists(&home, "config.yaml");
    }

    #[test]
    fn plain_text_writes_and_keeps_backup_in_temp_home() {
        let (home, target) = temp_home_target("plain-home", "AGENTS.md");
        fs::write(&target, "旧内容").unwrap();

        inject_in_home(Format::PlainText, &target, "新内容", &home).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "新内容");
        assert_temp_backup_exists(&home, "AGENTS.md");
    }

    #[test]
    fn profile_source_dir_initializes_builtin_profile_in_temp_home() {
        let home = temp_dir("profile-home");

        let profile = profile_source_dir_in(&home).unwrap();

        assert_eq!(
            profile,
            home.join(".agentdeck").join("profiles").join("default")
        );
        for relative in [
            "personalization/codex/AGENTS.md",
            "personalization/opencode/AGENTS.md",
            "personalization/claude/AGENTS.md",
            "personalization/cline/AGENTS.md",
            "personalization/hermes/AGENTS.md",
            "personalization/gemini/AGENTS.md",
            "personalization/zcode/AGENTS.md",
            "personalization/codex.json",
            "personalization/opencode/opencode.jsonc",
        ] {
            let path = profile.join(relative);
            assert!(path.is_file(), "内置 profile 文件缺失: {}", relative);
            assert!(
                !fs::read_to_string(path).unwrap().trim().is_empty(),
                "内置 profile 文件不应为空: {}",
                relative
            );
        }

        let codex: Value = serde_json::from_str(
            &fs::read_to_string(profile.join("personalization/codex.json")).unwrap(),
        )
        .expect("codex.json 应为有效 JSON");
        assert!(codex.is_object());

        let opencode =
            fs::read_to_string(profile.join("personalization/opencode/opencode.jsonc")).unwrap();
        let opencode: Value = serde_json::from_str(&strip_jsonc_comments(&opencode))
            .expect("opencode.jsonc 去除注释后应为有效 JSON");
        assert!(opencode.is_object());
    }

    #[test]
    fn profile_source_dir_preserves_existing_files_and_restores_missing_files() {
        let home = temp_dir("profile-preserve");
        let profile = profile_source_dir_in(&home).unwrap();
        let customized = profile.join("personalization/codex/AGENTS.md");
        let removed = profile.join("personalization/opencode/opencode.jsonc");
        fs::write(&customized, "用户自定义内容").unwrap();
        fs::remove_file(&removed).unwrap();

        let second = profile_source_dir_in(&home).unwrap();

        assert_eq!(second, profile);
        assert_eq!(fs::read_to_string(customized).unwrap(), "用户自定义内容");
        assert!(removed.is_file(), "再次初始化应补齐缺失文件");
    }

    #[test]
    fn profile_file_write_failure_removes_only_the_new_file() {
        let home = temp_dir("profile-write-failure");
        let target = home.join("personalization/codex/AGENTS.md");
        fs::create_dir_all(target.parent().unwrap()).unwrap();

        let error = create_profile_file_with(&target, |file| {
            file.write_all(b"partial")?;
            Err(std::io::Error::other("simulated profile write failure"))
        })
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("simulated profile write failure")
        );
        assert!(!target.exists(), "写失败时应移除本次新建的半文件");

        fs::write(&target, "用户已有内容").unwrap();
        create_profile_file_with(&target, |_file| {
            panic!("预先存在的用户文件不应进入写入分支")
        })
        .unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "用户已有内容");
    }

    #[test]
    fn inject_for_agent_in_home_keeps_all_supported_targets_in_temp_home() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let profile_home = temp_dir("all-agents-profile-home");
        let source = profile_source_dir_in(&profile_home).unwrap();
        let target_home = temp_dir("all-agents-target-home");
        let mut personalization_writes = 0;

        for agent in doc.agents.values() {
            let report = inject_for_agent_in_home(agent, &source, &target_home).unwrap();
            if report.personalization == Some(InjectStatus::Ok) {
                personalization_writes += 1;
            }
        }

        assert_eq!(personalization_writes, 4);
        for relative in [
            ".codex/AGENTS.md",
            ".codex/config.toml",
            ".config/opencode/AGENTS.md",
            ".claude/CLAUDE.md",
            ".gemini/GEMINI.md",
        ] {
            let target = target_home.join(relative);
            assert!(target.starts_with(&target_home));
            assert!(target.is_file(), "临时目标缺失: {}", relative);
        }
        assert!(!target_home.join(".config/opencode/opencode.jsonc").exists());
    }

    fn legacy_opencode_definition() -> AgentDefinition {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let mut agent = doc.agents.get("opencode").unwrap().clone();
        // A retained/custom runtime registry can still contain this retired adapter.
        agent.config_inject.model_config = Some(crate::registry::ModelConfigInject {
            file: "~/.config/opencode/opencode.jsonc".into(),
            format: "jsonc_merge".into(),
        });
        agent
    }

    #[test]
    fn legacy_opencode_model_descriptor_preserves_v2_bytes_and_personalization() {
        let home = temp_dir("opencode-retired-model");
        let source = profile_source_dir_in(&home).unwrap();
        let agent = legacy_opencode_definition();
        let target = home.join(".config/opencode/opencode.jsonc");
        let instructions = home.join(".config/opencode/AGENTS.md");
        let original = b"// Keep this user's V2 configuration byte-for-byte.\n{\n  \"mcp\": {\"servers\": {\"ragflow\": {\"type\": \"remote\", \"url\": \"http://localhost:9382/mcp\"}}},\n  \"model\": \"user/model\"\n}\n";
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, original).unwrap();
        fs::write(&instructions, "# User instructions\nKeep this.\n").unwrap();

        let report = inject_for_agent_in_home(&agent, &source, &home).unwrap();
        assert_eq!(report.personalization, Some(InjectStatus::Ok));
        assert_eq!(
            report.model_config,
            Some(InjectStatus::Skipped(
                "OpenCode model configuration adapter is not supported".into()
            ))
        );
        assert_eq!(fs::read(&target).unwrap(), original);
        let personalized = fs::read_to_string(&instructions).unwrap();
        assert!(personalized.starts_with("# User instructions\nKeep this.\n"));
        assert_eq!(personalized.matches(INSTRUCTIONS_START).count(), 1);
        inject_for_agent_in_home(&agent, &source, &home).unwrap();
        assert_eq!(fs::read_to_string(&instructions).unwrap(), personalized);
        assert_eq!(fs::read(&target).unwrap(), original);
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn legacy_opencode_model_descriptor_does_not_read_profile_or_snapshot_target() {
        let home = temp_dir("opencode-retired-model-unreadable");
        let source = profile_source_dir_in(&home).unwrap();
        let agent = legacy_opencode_definition();
        let profile = source.join("personalization/opencode/opencode.jsonc");
        fs::remove_file(&profile).unwrap();
        // Reading either directory as a model file would fail, even with elevated
        // test privileges. Personalization must still succeed without those reads.
        fs::create_dir_all(&profile).unwrap();
        let target = home.join(".config/opencode/opencode.jsonc");
        fs::create_dir_all(&target).unwrap();
        assert!(model_config_content(&agent, &source).unwrap().is_none());
        let report = inject_for_agent_in_home(&agent, &source, &home).unwrap();
        assert_eq!(report.personalization, Some(InjectStatus::Ok));
        assert!(matches!(
            report.model_config,
            Some(InjectStatus::Skipped(_))
        ));
        assert!(target.is_dir());
        assert!(!home.join(".agentdeck/backups").exists());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn inject_for_agent_skips_missing_template() {
        let agent = AgentDefinition {
            id: "claude-code".into(),
            name: "Claude Code".into(),
            source: "closed".into(),
            license: None,
            github: None,
            stars: Some(140000),
            ui_type: crate::registry::UiType::All,
            category: "ai-coding".into(),
            popular: true,
            description: "t".into(),
            install: crate::registry::InstallConfig {
                method: crate::registry::InstallMethod::Npm,
                extension_id: None,
                command: None,
                url: None,
                verify_cmd: None,
                uninstall_cmd: None,
                dependencies: vec![],
            },
            auth_type: crate::registry::AuthType::None,
            auth_guide: None,
            config_inject: crate::registry::ConfigInject {
                skills: true,
                mcp: Some(crate::registry::McpInject {
                    file: "~/.claude/settings.json".into(),
                    format: "json_merge".into(),
                    key: Some("mcpServers".into()),
                    section: None,
                }),
                memory: true,
                personalization: Some(crate::registry::PersonalizationInject {
                    template: "claude".into(),
                    target: "~/.claude/CLAUDE.md".into(),
                }),
                model_config: Some(crate::registry::ModelConfigInject {
                    file: "~/.claude/settings.json".into(),
                    format: "json_merge".into(),
                }),
            },
            memory_collector: None,
            icon: "claude-code".into(),
            homepage: None,
        };

        let dir = temp_dir("source");
        let report = inject_for_agent(&agent, &dir).unwrap();
        assert_eq!(
            report.personalization,
            Some(InjectStatus::Skipped(
                "模板缺失: personalization/claude/AGENTS.md".into()
            ))
        );
        assert_eq!(
            report.model_config,
            Some(InjectStatus::Skipped("缺少配置内容源".into()))
        );
    }

    #[test]
    fn linked_destination_hardlinks_refused_before_backup() {
        let home = temp_dir("linked-destination");
        let target = home.join("instructions.md");
        let alias = home.join("alias.md");
        fs::write(&target, "user instructions").unwrap();
        fs::hard_link(&target, &alias).unwrap();
        let before = fs::symlink_metadata(&target).unwrap();
        let result = inject_in_home(Format::PlainText, &target, "replacement", &home);
        assert!(result.is_err(), "linked destination must be refused");
        assert_untouched(&target, &before, b"user instructions");
        assert_eq!(fs::read(&alias).unwrap(), b"user instructions");
        assert!(!home.join(".agentdeck/backups").exists());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn linked_destination_agents_preserve_valid_hardlinks() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        for id in ["claude-code", "gemini-cli", "codex"] {
            let home = temp_dir("linked-agent");
            let source = profile_source_dir_in(&home).unwrap();
            let agent = doc.agents.get(id).unwrap();
            let target = expand_home_in(
                &agent.config_inject.personalization.as_ref().unwrap().target,
                &home,
            );
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, "valid user instructions").unwrap();
            let alias = home.join("alias.md");
            fs::hard_link(&target, &alias).unwrap();
            let before = fs::symlink_metadata(&target).unwrap();
            let error = inject_for_agent_in_home(agent, &source, &home).unwrap_err();
            assert!(error.to_string().contains("Manage it with the tool"));
            assert_untouched(&target, &before, b"valid user instructions");
            assert_eq!(fs::read(&alias).unwrap(), b"valid user instructions");
            assert!(!home.join(".agentdeck/backups").exists());
            fs::remove_dir_all(home).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_destination_symlink_dangling_and_parent_are_refused() {
        use std::os::unix::fs::symlink;
        for kind in ["leaf", "dangling", "parent"] {
            let home = temp_dir("linked-path");
            let original = home.join("original");
            let target = if kind == "parent" {
                fs::create_dir(&original).unwrap();
                fs::write(original.join("instructions.md"), "user instructions").unwrap();
                symlink(&original, home.join("parent")).unwrap();
                home.join("parent/instructions.md")
            } else {
                if kind == "leaf" {
                    fs::write(&original, "user instructions").unwrap();
                }
                symlink(&original, home.join("instructions.md")).unwrap();
                home.join("instructions.md")
            };
            assert!(inject_in_home(Format::PlainText, &target, "replacement", &home).is_err());
            assert!(!home.join(".agentdeck/backups").exists());
            if kind == "parent" {
                assert!(home.join("parent").is_symlink());
                assert_eq!(
                    fs::read(original.join("instructions.md")).unwrap(),
                    b"user instructions"
                );
            } else {
                assert_eq!(fs::read_link(&target).unwrap(), original);
                if kind == "leaf" {
                    assert_eq!(fs::read(&original).unwrap(), b"user instructions");
                } else {
                    assert!(!original.exists());
                }
            }
            fs::remove_dir_all(home).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_destination_missing_then_parent_traversal_is_refused() {
        use std::os::unix::fs::symlink;
        let home = temp_dir("linked-parent-traversal");
        let original = home.join("original");
        fs::create_dir(&original).unwrap();
        let original_file = original.join("instructions.md");
        fs::write(&original_file, "user instructions").unwrap();
        symlink(&original, home.join("linked-parent")).unwrap();
        let target = home.join("missing/../linked-parent/instructions.md");
        let before = fs::symlink_metadata(&original_file).unwrap();
        assert!(inject_in_home(Format::PlainText, &target, "replacement", &home).is_err());
        assert!(write_bytes(&target, b"replacement").is_err());
        assert_untouched(&original_file, &before, b"user instructions");
        assert_eq!(fs::read_link(home.join("linked-parent")).unwrap(), original);
        assert!(!home.join("missing").exists());
        assert!(!home.join(".agentdeck/backups").exists());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn linked_destination_write_and_rollback_primitive_refuse_new_links() {
        let home = temp_dir("linked-recheck");
        let target = home.join("instructions.md");
        write_bytes(&target, b"first write").unwrap();
        let alias = home.join("alias.md");
        fs::hard_link(&target, &alias).unwrap();
        let before = fs::symlink_metadata(&target).unwrap();
        assert!(write_bytes(&target, b"original rollback bytes").is_err());
        assert!(rollback_target(&target, Some(b"original rollback bytes".to_vec())).is_err());
        assert!(rollback_target(&target, None).is_err());
        assert_untouched(&target, &before, b"first write");
        assert_eq!(fs::read(&alias).unwrap(), b"first write");
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn linked_destination_skipped_template_and_adapter_are_not_inspected() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        for id in ["claude-code", "gemini-cli", "opencode", "codex"] {
            let home = temp_dir("linked-skipped");
            let source = profile_source_dir_in(&home).unwrap();
            let mut agent = doc.agents.get(id).unwrap().clone();
            let p = agent.config_inject.personalization.as_ref().unwrap();
            let target = expand_home_in(&p.target, &home);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(&target, "user instructions").unwrap();
            fs::hard_link(&target, home.join("alias")).unwrap();
            fs::remove_file(
                source
                    .join("personalization")
                    .join(&p.template)
                    .join("AGENTS.md"),
            )
            .unwrap();
            agent.config_inject.model_config = Some(crate::registry::ModelConfigInject {
                file: p.target.clone(),
                format: "unknown".into(),
            });
            let before = fs::symlink_metadata(&target).unwrap();
            let report = inject_for_agent_in_home(&agent, &source, &home).unwrap();
            assert!(matches!(
                report.personalization,
                Some(InjectStatus::Skipped(_))
            ));
            assert!(matches!(
                report.model_config,
                Some(InjectStatus::Skipped(_))
            ));
            assert_untouched(&target, &before, b"user instructions");
            assert!(!home.join(".agentdeck/backups").exists());
            fs::remove_dir_all(home).unwrap();
        }
    }

    fn assert_untouched(path: &Path, before: &fs::Metadata, bytes: &[u8]) {
        let after = fs::symlink_metadata(path).unwrap();
        assert_eq!(fs::read(path).unwrap(), bytes);
        assert_eq!(after.modified().unwrap(), before.modified().unwrap());
        assert_eq!(
            after.permissions().readonly(),
            before.permissions().readonly()
        );
        assert_eq!(after.file_type(), before.file_type());
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(after.ino(), before.ino());
            assert_eq!(after.dev(), before.dev());
            assert_eq!(after.mode(), before.mode());
        }
    }

    #[test]
    fn transaction_validation_errors_leave_claude_and_gemini_files_untouched() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        for id in ["claude-code", "gemini-cli"] {
            for (existing, template) in [
                (INSTRUCTIONS_START.to_owned(), "replacement".to_owned()),
                (
                    format!("{INSTRUCTIONS_END}{INSTRUCTIONS_START}"),
                    "replacement".into(),
                ),
                (
                    format!(
                        "{INSTRUCTIONS_START}{INSTRUCTIONS_END}{INSTRUCTIONS_START}{INSTRUCTIONS_END}"
                    ),
                    "replacement".into(),
                ),
                ("user instructions".into(), INSTRUCTIONS_START.into()),
                ("user instructions".into(), INSTRUCTIONS_END.into()),
            ] {
                let home = temp_dir("transaction-validation");
                let source = profile_source_dir_in(&home).unwrap();
                let agent = doc.agents.get(id).unwrap();
                let p = agent.config_inject.personalization.as_ref().unwrap();
                let target = expand_home_in(&p.target, &home);
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                fs::write(&target, &existing).unwrap();
                fs::File::options()
                    .write(true)
                    .open(&target)
                    .unwrap()
                    .set_times(fs::FileTimes::new().set_modified(
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000),
                    ))
                    .unwrap();
                let settings = target.parent().unwrap().join("settings.json");
                fs::write(&settings, "{\"untouched\": true}\n").unwrap();
                fs::File::options()
                    .write(true)
                    .open(&settings)
                    .unwrap()
                    .set_times(fs::FileTimes::new().set_modified(
                        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000),
                    ))
                    .unwrap();
                let before = fs::symlink_metadata(&target).unwrap();
                let settings_before = fs::symlink_metadata(&settings).unwrap();
                fs::write(
                    source
                        .join("personalization")
                        .join(&p.template)
                        .join("AGENTS.md"),
                    template,
                )
                .unwrap();
                let error = inject_for_agent_in_home(agent, &source, &home).unwrap_err();
                assert!(error.to_string().contains("markers"), "{error}");
                assert_untouched(&target, &before, existing.as_bytes());
                assert_untouched(&settings, &settings_before, b"{\"untouched\": true}\n");
                assert!(!home.join(".agentdeck/backups").exists());
                fs::remove_dir_all(home).unwrap();
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn transaction_validation_error_preserves_instruction_and_inactive_model_symlinks() {
        use std::os::unix::fs::symlink;
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        for id in ["claude-code", "gemini-cli"] {
            for reserved_template in [false, true] {
                let home = temp_dir("transaction-symlinks");
                let source = profile_source_dir_in(&home).unwrap();
                let agent = doc.agents.get(id).unwrap();
                let p = agent.config_inject.personalization.as_ref().unwrap();
                let target = expand_home_in(&p.target, &home);
                fs::create_dir_all(target.parent().unwrap()).unwrap();
                let original = if reserved_template {
                    "user instructions"
                } else {
                    INSTRUCTIONS_START
                };
                let referent = home.join("instructions-original");
                fs::write(&referent, original).unwrap();
                symlink(&referent, &target).unwrap();
                let settings = target.parent().unwrap().join("settings.json");
                let settings_referent = home.join("settings-original");
                fs::write(&settings_referent, "{}").unwrap();
                symlink(&settings_referent, &settings).unwrap();
                if reserved_template {
                    fs::write(
                        source
                            .join("personalization")
                            .join(&p.template)
                            .join("AGENTS.md"),
                        INSTRUCTIONS_END,
                    )
                    .unwrap();
                }
                let before = fs::symlink_metadata(&target).unwrap();
                let settings_before = fs::symlink_metadata(&settings).unwrap();
                assert!(inject_for_agent_in_home(agent, &source, &home).is_err());
                assert_untouched(&target, &before, original.as_bytes());
                assert_untouched(&settings, &settings_before, b"{}");
                assert_eq!(fs::read_link(&target).unwrap(), referent);
                assert_eq!(fs::read_link(&settings).unwrap(), settings_referent);
                fs::remove_dir_all(home).unwrap();
            }
        }
    }

    #[test]
    fn transaction_skipped_adapters_do_not_read_destinations() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        for id in ["claude-code", "gemini-cli", "codex", "opencode"] {
            for missing_template in [false, true] {
                let home = temp_dir("transaction-skipped");
                let source = profile_source_dir_in(&home).unwrap();
                let mut agent = doc.agents.get(id).unwrap().clone();
                let p = agent.config_inject.personalization.as_mut().unwrap();
                let target = expand_home_in(&p.target, &home);
                if missing_template {
                    fs::remove_file(
                        source
                            .join("personalization")
                            .join(&p.template)
                            .join("AGENTS.md"),
                    )
                    .unwrap();
                    fs::create_dir_all(&target).unwrap();
                }
                agent.config_inject.model_config = Some(crate::registry::ModelConfigInject {
                    file: "~/inactive-model".into(),
                    format: "toml_section".into(),
                });
                // Directories reliably fail fs::read even under elevated privileges.
                fs::create_dir_all(home.join("inactive-model")).unwrap();
                if id == "codex" {
                    fs::remove_file(source.join("personalization/codex.json")).unwrap();
                }
                let report = inject_for_agent_in_home(&agent, &source, &home).unwrap();
                assert!(matches!(
                    report.model_config,
                    Some(InjectStatus::Skipped(_))
                ));
                assert_eq!(
                    report.personalization == Some(InjectStatus::Ok),
                    !missing_template
                );
                assert!(home.join("inactive-model").is_dir());
                fs::remove_dir_all(home).unwrap();
            }
        }
    }

    #[test]
    fn transaction_unknown_format_does_not_read_destination() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let mut agent = doc.agents.get("codex").unwrap().clone();
        let home = temp_dir("transaction-format");
        let source = profile_source_dir_in(&home).unwrap();
        let mc = agent.config_inject.model_config.as_mut().unwrap();
        mc.format = "unknown".into();
        let target = expand_home_in(&mc.file, &home);
        fs::create_dir_all(&target).unwrap();
        let report = inject_for_agent_in_home(&agent, &source, &home).unwrap();
        assert_eq!(report.personalization, Some(InjectStatus::Ok));
        assert!(matches!(
            report.model_config,
            Some(InjectStatus::Skipped(_))
        ));
        assert!(target.is_dir());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn transaction_later_snapshot_failure_rolls_back_committed_first_write() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let agent = doc.agents.get("codex").unwrap();
        for existing in [false, true] {
            let home = temp_dir("transaction-later-snapshot");
            let source = profile_source_dir_in(&home).unwrap();
            let first = home.join(".codex/AGENTS.md");
            let second = home.join(".codex/config.toml");
            fs::create_dir_all(&second).unwrap();
            if existing {
                fs::write(&first, "user instructions").unwrap();
            }
            assert!(inject_for_agent_in_home(agent, &source, &home).is_err());
            if existing {
                assert_eq!(fs::read_to_string(&first).unwrap(), "user instructions");
                // The first write was actually attempted before the later read failed.
                assert_temp_backup_exists(&home, "AGENTS.md");
            } else {
                assert!(!first.exists());
            }
            assert!(second.is_dir());
            fs::remove_dir_all(home).unwrap();
        }
    }

    #[test]
    fn failed_second_target_restores_first_target() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let agent = doc.agents.get("codex").unwrap();
        let home = temp_dir("transaction");
        let source = profile_source_dir_in(&home).unwrap();
        let first = home.join(".codex/AGENTS.md");
        let second = home.join(".codex/config.toml");
        fs::create_dir_all(first.parent().unwrap()).unwrap();
        fs::write(&first, "user instructions").unwrap();
        fs::write(&second, "not valid TOML [[[}").unwrap();
        assert!(inject_for_agent_in_home(agent, &source, &home).is_err());
        assert_eq!(fs::read_to_string(&first).unwrap(), "user instructions");
        assert_eq!(fs::read_to_string(&second).unwrap(), "not valid TOML [[[}");
        fs::remove_file(&first).unwrap();
        assert!(inject_for_agent_in_home(agent, &source, &home).is_err());
        assert!(!first.exists(), "new target must be removed on rollback");
    }

    #[test]
    fn yaml_merge_initializes_missing_target_and_rejects_scalar() {
        let (home, target) = temp_home_target("yaml-empty", "settings.yaml");
        inject_in_home(Format::YamlSection, &target, "enabled: true", &home).unwrap();
        let value: Value = serde_yml::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
        assert_eq!(value["enabled"], true);
        fs::write(&target, "42").unwrap();
        assert!(inject_in_home(Format::YamlSection, &target, "enabled: true", &home).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "42");
    }

    #[test]
    fn markdown_never_overwrites_structured_agent_config() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let home = temp_dir("hermes-safe");
        let source = profile_source_dir_in(&home).unwrap();
        let target = home.join(".hermes/config.yaml");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "user_setting: true").unwrap();
        let mut unsafe_definition = doc.agents.get("hermes").unwrap().clone();
        unsafe_definition.config_inject.personalization =
            Some(crate::registry::PersonalizationInject {
                template: "hermes".into(),
                target: "~/.hermes/config.yaml".into(),
            });
        let report = inject_for_agent_in_home(&unsafe_definition, &source, &home).unwrap();
        assert!(matches!(
            report.personalization,
            Some(InjectStatus::Skipped(_))
        ));
        assert_eq!(fs::read_to_string(target).unwrap(), "user_setting: true");
    }

    #[test]
    fn invalid_profile_is_an_error_and_rolls_back_instructions() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let home = temp_dir("invalid-profile");
        let source = profile_source_dir_in(&home).unwrap();
        fs::write(source.join("personalization/codex.json"), "invalid JSON").unwrap();
        assert!(
            inject_for_agent_in_home(doc.agents.get("codex").unwrap(), &source, &home).is_err()
        );
        assert!(!home.join(".codex/AGENTS.md").exists());
    }

    #[test]
    fn toml_profile_escapes_keys_and_multiline_strings() {
        let sections = serde_json::json!({"a.b": {"odd key": "line one\nline two"}});
        let output = sections_to_toml(&sections).unwrap();
        let parsed: toml::Value = toml::from_str(&output).unwrap();
        assert_eq!(
            parsed["a.b"]["odd key"].as_str(),
            Some("line one\nline two")
        );
    }

    #[test]
    fn personalization_preserves_user_content_and_updates_only_managed_block() {
        let home = temp_dir("managed-instructions");
        let target = home.join("AGENTS.md");
        let original = "# My instructions\nNever delete these.\n";
        fs::write(&target, original).unwrap();
        let first = managed_instructions(&target, "Version one").unwrap();
        assert!(first.starts_with(original));
        fs::write(&target, format!("{first}User footer\n")).unwrap();
        let second = managed_instructions(&target, "Version two").unwrap();
        assert!(second.starts_with(original));
        assert!(second.ends_with("User footer\n"));
        assert!(!second.contains("Version one"));
        assert_eq!(second.matches(INSTRUCTIONS_START).count(), 1);
        assert!(second.contains("Version two"));
        fs::write(&target, &second).unwrap();
        assert_eq!(
            managed_instructions(&target, "Version two").unwrap(),
            second
        );
    }

    #[test]
    fn malformed_managed_markers_do_not_modify_instructions() {
        let home = temp_dir("managed-invalid");
        let target = home.join("AGENTS.md");
        for text in [
            INSTRUCTIONS_START.to_owned(),
            format!("{INSTRUCTIONS_END}{INSTRUCTIONS_START}"),
            format!("{INSTRUCTIONS_START}{INSTRUCTIONS_END}{INSTRUCTIONS_START}{INSTRUCTIONS_END}"),
        ] {
            fs::write(&target, &text).unwrap();
            assert!(managed_instructions(&target, "replacement").is_err());
            assert_eq!(fs::read_to_string(&target).unwrap(), text);
        }
    }

    #[test]
    fn personalization_capability_matches_supported_destinations() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        for id in ["codex", "claude-code", "opencode", "gemini-cli"] {
            assert!(
                supports_personalization(doc.agents.get(id).unwrap()),
                "{id}"
            );
        }
        for id in ["hermes", "cline", "zcode", "openclaw", "pi"] {
            assert!(
                !supports_personalization(doc.agents.get(id).unwrap()),
                "{id}"
            );
        }
    }

    const REGISTRY_JSON: &str = include_str!("../../../../agents/registry-v3.json");

    /// 真实环境验证：codex/opencode 注入真实配置（带备份）
    #[test]
    #[ignore = "真实环境验证：修改 ~/.codex 与 ~/.config/opencode 配置（带备份）"]
    fn real_inject_codex_and_opencode() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let home = temp_dir("real-profile-codex-opencode");
        let source = profile_source_dir_in(&home).unwrap();
        for id in ["codex", "opencode"] {
            let agent = doc.agents.get(id).unwrap();
            let report = inject_for_agent(agent, &source).unwrap();
            assert_eq!(
                report.personalization,
                Some(InjectStatus::Ok),
                "{} 个性化注入应成功",
                id
            );
            assert_eq!(
                report.model_config,
                if id == "codex" {
                    Some(InjectStatus::Ok)
                } else {
                    None
                },
                "{} model_config 报告应符合已支持的能力",
                id
            );
        }
    }

    /// 全注册表诊断：在临时 profile 与临时目标 home 中输出逐 Agent 注入报告。
    #[test]
    #[ignore = "全注册表诊断：仅写临时目录，手动查看逐 Agent 注入报告"]
    fn inject_all_agents_in_temp_home_report() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let profile_home = temp_dir("report-profile-home");
        let source = profile_source_dir_in(&profile_home).unwrap();
        let target_home = temp_dir("report-target-home");
        let mut ids: Vec<&String> = doc.agents.keys().collect();
        ids.sort();
        for id in ids {
            let agent = doc.agents.get(id).unwrap();
            let report = inject_for_agent_in_home(agent, &source, &target_home).unwrap();
            println!(
                "{} | personalization={:?} | model_config={:?}",
                report.agent_id, report.personalization, report.model_config
            );
        }
    }
}
