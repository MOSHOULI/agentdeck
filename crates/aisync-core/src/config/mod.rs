//! 配置注入适配器模块（M2）。
//!
//! 适配器模式：统一接口 → 各格式 Writer。
//! 支持格式：TOML section / JSON merge / JSONC merge / YAML section / plain text。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::registry::AgentDefinition;

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
    let home = dirs::home_dir().ok_or_else(|| crate::Error::Other("无法定位用户主目录".into()))?;
    profile_source_dir_in(&home)
}

fn profile_source_dir_in(home: &Path) -> crate::Result<PathBuf> {
    let profile = home.join(".agentdeck").join("profiles").join("default");
    for (relative, content) in BUILTIN_PROFILE_FILES {
        let target = profile.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
        {
            Ok(mut file) => file.write_all(content.as_bytes())?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(profile)
}

fn expand_home(path: &str) -> PathBuf {
    if let (Some(rest), Some(home)) = (
        path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")),
        dirs::home_dir(),
    ) {
        return home.join(rest);
    }
    PathBuf::from(path)
}

fn backup_file_in(path: &Path, home: &Path) -> crate::Result<Option<PathBuf>> {
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
    fs::copy(path, &dest)?;
    Ok(Some(dest))
}

fn backup_file(path: &Path) -> crate::Result<Option<PathBuf>> {
    if !path.is_file() {
        return Ok(None);
    }
    let home = dirs::home_dir().ok_or_else(|| crate::Error::Other("无法定位用户主目录".into()))?;
    backup_file_in(path, &home)
}

fn write_file(path: &Path, content: &str) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
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

fn json_to_toml_literal(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => {
            let inner: Vec<String> = items.iter().map(json_to_toml_literal).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::Null => "\"\"".into(),
        Value::Object(_) => "\"\"".into(),
    }
}

/// 把 codex.json 的 sections 对象转成 TOML 段文本（嵌套对象转子段）
fn sections_to_toml(sections: &Value) -> String {
    fn emit(map: &serde_json::Map<String, Value>, prefix: &str, out: &mut String) {
        for (name, value) in map {
            let header = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{}.{}", prefix, name)
            };
            if let Value::Object(fields) = value {
                out.push_str(&format!("[{}]\n", header));
                let mut nested = serde_json::Map::new();
                for (k, v) in fields {
                    if v.is_object() {
                        nested.insert(k.clone(), v.clone());
                    } else {
                        out.push_str(&format!("{} = {}\n", k, json_to_toml_literal(v)));
                    }
                }
                if !nested.is_empty() {
                    emit(&nested, &header, out);
                }
                out.push('\n');
            }
        }
    }
    let mut out = String::new();
    if let Value::Object(map) = sections {
        emit(map, "", &mut out);
    }
    out
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
            }
        } else {
            target.insert(key, item.clone());
        }
    }
}

/// 合并 TOML section（不产生重复段，不触碰其他内容）
fn patch_toml_section(target: &Path, section_text: &str) -> crate::Result<()> {
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

fn merge_yaml(target: &Path, additions: &str) -> crate::Result<()> {
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
    let mut merged = base;
    merge_json(&mut merged, additions);
    let out = serde_yml::to_string(&merged)
        .map_err(|e| crate::Error::InjectFailed(format!("YAML 序列化失败: {}", e)))?;
    write_file(target, &out)
}

fn inject_with_backup(
    format: Format,
    target_path: &Path,
    content: &str,
    backup: Option<PathBuf>,
) -> crate::Result<()> {
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
            let additions: Value = serde_json::from_str(content)
                .map_err(|e| crate::Error::InjectFailed(format!("注入内容解析失败: {}", e)))?;
            merge_json(&mut base, additions);
            let out = serde_json::to_string_pretty(&base)
                .map_err(|e| crate::Error::InjectFailed(format!("JSON 序列化失败: {}", e)))?;
            write_file(target_path, &out)
        }
        Format::TomlSection => patch_toml_section(target_path, content),
        Format::YamlSection => merge_yaml(target_path, content),
    };

    if let Err(e) = &result {
        if let Some(backup) = backup {
            if let Some(parent) = target_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(&backup, target_path);
        }
        return Err(crate::Error::InjectFailed(format!(
            "注入失败（已回滚）: {}",
            e
        )));
    }
    Ok(())
}

/// 注入配置到目标文件（先备份，失败自动回滚）
pub fn inject(format: Format, target_path: &Path, content: &str) -> crate::Result<()> {
    let backup = backup_file(target_path)?;
    inject_with_backup(format, target_path, content, backup)
}

#[cfg(test)]
fn inject_in_home(
    format: Format,
    target_path: &Path,
    content: &str,
    home: &Path,
) -> crate::Result<()> {
    let backup = backup_file_in(target_path, home)?;
    inject_with_backup(format, target_path, content, backup)
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
fn model_config_content(agent: &AgentDefinition, source_dir: &Path) -> Option<String> {
    match agent.id.as_str() {
        "opencode" => fs::read_to_string(
            source_dir
                .join("personalization")
                .join("opencode")
                .join("opencode.jsonc"),
        )
        .ok(),
        "codex" => {
            let raw =
                fs::read_to_string(source_dir.join("personalization").join("codex.json")).ok()?;
            let value: Value = serde_json::from_str(&raw).ok()?;
            let sections = value.get("sections").cloned().unwrap_or(Value::Null);
            let toml = sections_to_toml(&sections);
            if toml.trim().is_empty() {
                None
            } else {
                Some(toml)
            }
        }
        _ => None,
    }
}

/// 为单个 agent 执行个性化 + model_config 注入，返回逐项报告
pub fn inject_for_agent(agent: &AgentDefinition, source_dir: &Path) -> crate::Result<InjectReport> {
    let mut personalization = None;
    let mut model_config = None;

    if let Some(p) = &agent.config_inject.personalization {
        if p.target.contains("://") {
            personalization = Some(InjectStatus::Skipped("特殊通道暂不支持".into()));
        } else if !p.target.starts_with("~/") && !p.target.starts_with("~\\") {
            personalization = Some(InjectStatus::Skipped("非绝对路径目标暂不支持".into()));
        } else if let Some(content) = template_agents_md(source_dir, &p.template) {
            let target = expand_home(&p.target);
            match inject(Format::PlainText, &target, &content) {
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
        if mc.file.contains("://") {
            model_config = Some(InjectStatus::Skipped("特殊通道暂不支持".into()));
        } else if let Some(content) = model_config_content(agent, source_dir) {
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
                let target = expand_home(&mc.file);
                match inject(format, &target, &content) {
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
        dir
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
                Some(InjectStatus::Ok),
                "{} model_config 注入应成功",
                id
            );
        }
    }

    /// 真实环境验证：输出全部 9 款 Agent 的注入报告（无 Err 即通过）
    #[test]
    #[ignore = "真实环境验证：对 9 款 Agent 逐一注入（仅 codex/opencode 实际写入）"]
    fn real_inject_all_nine_report() {
        let doc = crate::registry::parse_registry(REGISTRY_JSON).unwrap();
        let home = temp_dir("real-profile-all");
        let source = profile_source_dir_in(&home).unwrap();
        let mut ids: Vec<&String> = doc.agents.keys().collect();
        ids.sort();
        for id in ids {
            let agent = doc.agents.get(id).unwrap();
            let report = inject_for_agent(agent, &source).unwrap();
            println!(
                "{} | personalization={:?} | model_config={:?}",
                report.agent_id, report.personalization, report.model_config
            );
        }
    }
}
