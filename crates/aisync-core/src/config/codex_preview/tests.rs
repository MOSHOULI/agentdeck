use super::*;
use std::collections::BTreeMap;
struct Fixture {
    home: PathBuf,
    profile: PathBuf,
    agent: AgentDefinition,
}
impl Fixture {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!("codex-plan-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&home).unwrap();
        let profile = home.join(".agentdeck/profiles/default");
        let registry =
            crate::registry::parse_registry(include_str!("../../../../../agents/registry-v3.json"))
                .unwrap();
        Self {
            home,
            profile,
            agent: registry.agents["codex"].clone(),
        }
    }
    fn write(&self, relative: &str, text: &str) {
        let path = self.home.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn plan(&self) -> crate::Result<CodexConfigurationPreview> {
        build(&self.agent, &self.profile, &self.home, None)
    }
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn scan(path: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for e in fs::read_dir(path).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    out.insert(p.clone(), vec![]);
                    scan(&p, out);
                } else {
                    out.insert(p.clone(), fs::read(&p).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        scan(&self.home, &mut out);
        out
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.home);
    }
}
#[test]
fn default_plan_creates_nothing_and_runs_nothing() {
    let f = Fixture::new();
    let before = f.snapshot();
    let p = f.plan().unwrap();
    assert_eq!(p.mode, "read_only_configure_plan");
    assert!(!p.apply_supported);
    assert!(p.uses_bundled_defaults);
    assert_eq!(
        p.targets.iter().map(|t| t.action).collect::<Vec<_>>(),
        vec!["create", "create"]
    );
    assert_eq!(p.conflict_count, 0);
    assert_eq!(f.snapshot(), before);
    assert_eq!(p.revision, f.plan().unwrap().revision);
}
#[test]
fn existing_values_conflict_and_arbitrary_values_never_escape() {
    let f = Fixture::new();
    f.write(".codex/AGENTS.md", "PRIVATE_INSTRUCTIONS\n");
    f.write(
        ".codex/config.toml",
        "features = false\nSECRET_KEY = 'SECRET_VALUE'\n",
    );
    let before = f.snapshot();
    let p = f.plan().unwrap();
    let json = serde_json::to_string(&p).unwrap();
    for canary in ["PRIVATE_INSTRUCTIONS", "SECRET_KEY", "SECRET_VALUE"] {
        assert!(!json.contains(canary));
    }
    assert_eq!(p.targets[0].action, "merge");
    assert_eq!(p.targets[1].action, "conflict");
    assert_eq!(p.targets[1].conflict_codes, vec!["toml_type_conflict"]);
    assert_eq!(f.snapshot(), before);
}
#[test]
fn custom_profile_scalar_conflict_counts_are_redacted() {
    let f = Fixture::new();
    f.write(
        ".agentdeck/profiles/default/personalization/codex.json",
        r#"{"sections":{"secret_key":"new","other":true}}"#,
    );
    f.write(".codex/config.toml", "secret_key='old'\nother=false\n");
    let p = f.plan().unwrap();
    assert_eq!(p.targets[1].conflict_count, 2);
    assert_eq!(p.targets[1].conflict_codes, vec!["differing_toml_value"]);
    assert!(!serde_json::to_string(&p).unwrap().contains("secret_key"));
}
#[test]
fn managed_markers_report_replacement_or_malformed_conflict() {
    let f = Fixture::new();
    f.write(
        ".codex/AGENTS.md",
        &format!(
            "prefix\n{}\nold manual text\n{}\nsuffix",
            super::super::INSTRUCTIONS_START,
            super::super::INSTRUCTIONS_END
        ),
    );
    let p = f.plan().unwrap();
    assert_eq!(p.targets[0].action, "replace_managed_block");
    assert_eq!(p.targets[0].conflict_count, 1);
    f.write(".codex/AGENTS.md", super::super::INSTRUCTIONS_START);
    assert_eq!(
        f.plan().unwrap().targets[0].conflict_codes,
        vec!["ambiguous_markers"]
    );
}
#[test]
fn unchanged_managed_and_toml_files_are_reported_without_writes() {
    let f = Fixture::new();
    let template = BUILTIN_PROFILE_FILES
        .iter()
        .find(|(p, _)| *p == "personalization/codex/AGENTS.md")
        .unwrap()
        .1;
    f.write(
        ".codex/AGENTS.md",
        &managed_instructions_text("", template).unwrap(),
    );
    f.write(".codex/config.toml", "[features]\n");
    let before = f.snapshot();
    let p = f.plan().unwrap();
    assert!(p.targets.iter().all(|t| t.action == "unchanged"));
    assert_eq!(before, f.snapshot());
}
#[test]
fn revision_binds_source_presence_bytes_target_and_descriptor() {
    let f = Fixture::new();
    let first = f.plan().unwrap().revision;
    let builtin = BUILTIN_PROFILE_FILES
        .iter()
        .find(|(p, _)| *p == "personalization/codex.json")
        .unwrap()
        .1;
    f.write(
        ".agentdeck/profiles/default/personalization/codex.json",
        builtin,
    );
    let present = f.plan().unwrap().revision;
    assert_ne!(first, present);
    f.write(
        ".agentdeck/profiles/default/personalization/codex.json",
        r#"{"sections":{"features":{},"model":"private"}}"#,
    );
    let changed = f.plan().unwrap().revision;
    assert_ne!(present, changed);
    f.write(".codex/AGENTS.md", "authored");
    assert_ne!(changed, f.plan().unwrap().revision);
    let mut agent = f.agent.clone();
    agent.name.push_str("changed");
    assert_ne!(
        f.plan().unwrap().revision,
        build(&agent, &f.profile, &f.home, None).unwrap().revision
    );
}
#[test]
fn invalid_descriptor_override_and_profile_fail_closed_redacted() {
    let f = Fixture::new();
    let mut agent = f.agent.clone();
    agent.config_inject.model_config.as_mut().unwrap().file = "~/other/config.toml".into();
    assert!(build(&agent, &f.profile, &f.home, None).is_err());
    assert!(build(&f.agent, &f.profile, &f.home, Some(OsStr::new(""))).is_err());
    assert!(
        build(
            &f.agent,
            &f.profile,
            &f.home,
            Some(OsStr::new("/other/private"))
        )
        .is_err()
    );
    assert!(
        build(
            &f.agent,
            &f.profile,
            &f.home,
            Some(f.home.join(".codex").as_os_str())
        )
        .is_ok()
    );
    for source in [
        "SECRET malformed",
        r#"{"sections":{},"sections":{"private":true}}"#,
        r#"{"sections":"SECRET"}"#,
    ] {
        f.write(
            ".agentdeck/profiles/default/personalization/codex.json",
            source,
        );
        assert!(!f.plan().unwrap_err().to_string().contains("SECRET"));
    }
}
#[test]
fn invalid_target_toml_is_a_fixed_conflict() {
    let f = Fixture::new();
    f.write(".codex/config.toml", "SECRET = [not valid");
    let p = f.plan().unwrap();
    assert_eq!(p.targets[1].conflict_codes, vec!["invalid_target_toml"]);
    assert!(!serde_json::to_string(&p).unwrap().contains("SECRET"));
}
#[test]
fn nonregular_oversized_and_traversing_paths_fail_closed() {
    let f = Fixture::new();
    fs::create_dir_all(f.home.join(".codex/AGENTS.md")).unwrap();
    assert!(f.plan().is_err());
    fs::remove_dir(f.home.join(".codex/AGENTS.md")).unwrap();
    f.write(".codex/AGENTS.md", &"x".repeat(2 * 1024 * 1024));
    assert!(f.plan().is_err());
    assert!(build(&f.agent, &f.profile.join("../default"), &f.home, None).is_err());
}
#[cfg(unix)]
#[test]
fn symlink_ancestors_leaves_and_hardlinks_fail_closed() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let other = f.home.join("other");
    fs::create_dir(&other).unwrap();
    symlink(&other, f.home.join(".codex")).unwrap();
    assert!(f.plan().is_err());
    fs::remove_file(f.home.join(".codex")).unwrap();
    fs::create_dir(f.home.join(".codex")).unwrap();
    fs::write(other.join("private"), "secret").unwrap();
    symlink(other.join("private"), f.home.join(".codex/AGENTS.md")).unwrap();
    assert!(f.plan().is_err());
    fs::remove_file(f.home.join(".codex/AGENTS.md")).unwrap();
    fs::hard_link(other.join("private"), f.home.join(".codex/AGENTS.md")).unwrap();
    assert!(f.plan().is_err());
}

#[test]
fn control_character_paths_are_not_exposed() {
    let f = Fixture::new();
    let unsafe_home = f.home.join("private\nspoof");
    fs::create_dir(&unsafe_home).unwrap();
    let error = build(&f.agent, &f.profile, &unsafe_home, None)
        .unwrap_err()
        .to_string();
    assert!(!error.contains("private"));
    assert!(!error.contains("spoof"));
}
#[cfg(unix)]
#[test]
fn non_utf8_home_and_missing_sources_under_symlinks_are_rejected() {
    use std::os::unix::{ffi::OsStrExt, fs::symlink};
    let f = Fixture::new();
    let home = f.home.join(OsStr::from_bytes(b"invalid-\xff"));
    fs::create_dir(&home).unwrap();
    assert!(build(&f.agent, &f.profile, &home, None).is_err());
    let redirected = f.home.join("redirected");
    fs::create_dir(&redirected).unwrap();
    symlink(redirected, f.home.join(".agentdeck")).unwrap();
    assert!(f.plan().is_err());
}

#[test]
fn inline_table_replacement_is_not_advertised_as_safe_merge() {
    let f = Fixture::new();
    f.write(".codex/config.toml", "features = { private_flag = true }\n");
    let p = f.plan().unwrap();
    assert_eq!(p.targets[1].action, "conflict");
    assert!(p.targets[1].conflict_codes.contains(&"toml_type_conflict"));
    assert!(!serde_json::to_string(&p).unwrap().contains("private_flag"));
}
