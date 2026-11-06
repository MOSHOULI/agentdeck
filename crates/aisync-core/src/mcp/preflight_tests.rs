//! Synthetic files and fake backup callbacks only. Run through run-isolated.py.
use super::*;
use std::cell::Cell;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("mcp-preflight-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    fn target(&self, format: &str) -> crate::registry::McpInject {
        crate::registry::McpInject {
            file: self.0.join("agent/config").to_string_lossy().into_owned(),
            format: format.into(),
            key: Some("mcpServers".into()),
            section: Some("mcp_servers".into()),
        }
    }

    fn write(&self, content: impl AsRef<[u8]>) {
        std::fs::create_dir_all(self.0.join("agent")).unwrap();
        std::fs::write(self.0.join("agent/config"), content).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn server() -> McpServerDef {
    McpServerDef {
        name: "ragflow".into(),
        description: None,
        url: Some("http://127.0.0.1:9382/mcp".into()),
        command: None,
        args: vec![],
        disabled: false,
    }
}

fn assert_rejected_before_backup(
    target: &crate::registry::McpInject,
    def: &McpServerDef,
    enabled: bool,
) {
    let called = Cell::new(false);
    let result = toggle_target_with_backup(target, "ragflow", def, enabled, || {
        called.set(true);
        Ok(())
    });
    assert!(result.is_err());
    assert!(!called.get(), "invalid requests must not invoke backup");
}

#[test]
fn unsupported_and_unknown_agents_never_invoke_backup() {
    let fixture = Fixture::new();
    for agent in ["opencode", "not-an-agent", "pi", "hermes"] {
        for enabled in [true, false] {
            let error = toggle_with_backup(agent, "ragflow", enabled, || {
                std::fs::create_dir_all(fixture.0.join("fake-backup"))?;
                Ok(())
            })
            .unwrap_err();
            assert!(error.to_string().contains("不支持 MCP"));
            assert!(!fixture.0.join("fake-backup").exists());
        }
    }
}

#[test]
fn missing_and_disabled_servers_never_invoke_backup_when_enabling() {
    for name in ["not-a-server", "example-http-mcp", "example-stdio-mcp"] {
        let error = toggle_with_backup("codex", name, true, || {
            panic!("invalid server must not invoke backup")
        })
        .unwrap_err();
        assert!(error.to_string().contains("不存在或已禁用"));
    }
}

#[test]
fn malformed_config_is_unchanged_and_never_invokes_backup() {
    for (format, text) in [
        ("toml_section", "[broken"),
        ("json_merge", "{broken"),
        ("jsonc_merge", "// comment\n{broken"),
        ("yaml_section", "broken: ["),
    ] {
        let fixture = Fixture::new();
        fixture.write(text);
        let target = fixture.target(format);
        for enabled in [true, false] {
            assert_rejected_before_backup(&target, &server(), enabled);
            assert_eq!(std::fs::read_to_string(&target.file).unwrap(), text);
        }
        assert_eq!(
            std::fs::read_dir(fixture.0.join("agent")).unwrap().count(),
            1
        );
    }
}

#[test]
fn invalid_config_shapes_and_encoding_never_invoke_backup() {
    for (format, text) in [
        ("toml_section", "mcp_servers = false"),
        ("json_merge", "[]"),
        ("json_merge", r#"{"mcpServers":false}"#),
        ("jsonc_merge", r#"{"mcpServers":[]}"#),
        ("yaml_section", "[]"),
        ("yaml_section", "mcp_servers: false"),
    ] {
        let fixture = Fixture::new();
        fixture.write(text);
        let target = fixture.target(format);
        assert_rejected_before_backup(&target, &server(), true);
        assert_eq!(std::fs::read_to_string(&target.file).unwrap(), text);
    }
    let fixture = Fixture::new();
    fixture.write([0xff]);
    assert_rejected_before_backup(&fixture.target("json_merge"), &server(), true);
    assert_eq!(
        std::fs::read(fixture.0.join("agent/config")).unwrap(),
        [0xff]
    );
}

#[test]
fn invalid_command_definition_or_format_creates_no_destination() {
    let fixture = Fixture::new();
    let mut invalid = server();
    invalid.url = None;
    assert_rejected_before_backup(&fixture.target("json_merge"), &invalid, true);
    invalid.command = Some("powershell".into());
    assert_rejected_before_backup(&fixture.target("json_merge"), &invalid, true);
    for enabled in [true, false] {
        assert_rejected_before_backup(&fixture.target("unsupported-format"), &server(), enabled);
    }
    assert!(!fixture.0.join("agent").exists());
}

#[test]
fn unresolved_and_nonregular_paths_reject_even_noop_before_backup() {
    let fixture = Fixture::new();
    let mut target = fixture.target("toml_section");
    target.file = "%UNRESOLVED%/config.toml".into();
    assert_rejected_before_backup(&target, &server(), false);

    let target = fixture.target("toml_section");
    std::fs::create_dir_all(&target.file).unwrap();
    assert_rejected_before_backup(&target, &server(), false);
    std::fs::remove_dir_all(fixture.0.join("agent")).unwrap();
    std::fs::write(fixture.0.join("agent"), "not a directory").unwrap();
    assert_rejected_before_backup(&target, &server(), false);
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("agent")).unwrap(),
        "not a directory"
    );
}

#[test]
fn parent_components_cannot_hide_existing_destination_until_write() {
    let fixture = Fixture::new();
    fixture.write("{\"keep\":true}");
    let mut target = fixture.target("json_merge");
    // The OS cannot resolve this path until `missing` exists. Creating it at
    // commit time would expose a file that was not part of the read snapshot.
    target.file = fixture
        .0
        .join("missing/../agent/config")
        .to_string_lossy()
        .into_owned();
    assert_rejected_before_backup(&target, &server(), true);
    assert!(!fixture.0.join("missing").exists());
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("agent/config")).unwrap(),
        "{\"keep\":true}"
    );
}

#[cfg(unix)]
#[test]
fn symlink_leaf_dangling_leaf_and_parent_reject_before_backup() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let target = fixture.target("toml_section");
    std::fs::create_dir(fixture.0.join("agent")).unwrap();
    let outside = fixture.0.join("outside");
    symlink(&outside, &target.file).unwrap();
    assert_rejected_before_backup(&target, &server(), false);
    assert!(!outside.exists());
    std::fs::write(&outside, "").unwrap();
    assert_rejected_before_backup(&target, &server(), false);
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "");

    std::fs::remove_dir_all(fixture.0.join("agent")).unwrap();
    std::fs::create_dir(fixture.0.join("real-agent")).unwrap();
    std::fs::write(fixture.0.join("real-agent/config"), "").unwrap();
    symlink(fixture.0.join("real-agent"), fixture.0.join("agent")).unwrap();
    assert_rejected_before_backup(&target, &server(), false);
    assert_eq!(
        std::fs::read_to_string(fixture.0.join("real-agent/config")).unwrap(),
        ""
    );
}

#[test]
fn valid_toggle_backs_up_once_before_write_for_each_format() {
    for (format, original) in [
        (
            "toml_section",
            "keep = true\n[mcp_servers.private]\ncommand = 'mine'\n",
        ),
        (
            "json_merge",
            r#"{"keep":true,"mcpServers":{"private":{"command":"mine"}}}"#,
        ),
        (
            "jsonc_merge",
            "// comment\n{\"keep\":true,\"mcpServers\":{\"private\":{\"command\":\"mine\"}}}",
        ),
        (
            "yaml_section",
            "keep: true\nmcp_servers:\n  private:\n    command: mine\n",
        ),
    ] {
        let fixture = Fixture::new();
        fixture.write(original);
        let target = fixture.target(format);
        let called = Cell::new(0);
        toggle_target_with_backup(&target, "ragflow", &server(), true, || {
            called.set(called.get() + 1);
            assert_eq!(std::fs::read_to_string(&target.file).unwrap(), original);
            Ok(())
        })
        .unwrap();
        assert_eq!(called.get(), 1);
        let enabled = std::fs::read_to_string(&target.file).unwrap();
        assert!(enabled.contains("ragflow"));
        assert!(enabled.contains("private"));
        assert!(enabled.contains("keep"));

        // Removal needs no active/valid registry definition, including commands
        // that could not be enabled. User-owned entries remain untouched.
        let mut removed = server();
        removed.disabled = true;
        removed.url = None;
        removed.command = Some("not-allowed".into());
        toggle_target_with_backup(&target, "ragflow", &removed, false, || Ok(())).unwrap();
        let disabled = std::fs::read_to_string(&target.file).unwrap();
        assert!(!disabled.contains("ragflow"));
        assert!(disabled.contains("private"));
        assert!(disabled.contains("keep"));
    }
}

#[test]
fn missing_destination_is_created_only_after_successful_backup() {
    let fixture = Fixture::new();
    let target = fixture.target("json_merge");
    toggle_target_with_backup(&target, "ragflow", &server(), true, || {
        assert!(!fixture.0.join("agent").exists());
        Ok(())
    })
    .unwrap();
    assert!(
        std::fs::read_to_string(&target.file)
            .unwrap()
            .contains("ragflow")
    );
}

#[test]
fn backup_failure_preserves_existing_or_absent_destination() {
    for exists in [true, false] {
        let fixture = Fixture::new();
        if exists {
            fixture.write("{}");
        }
        let target = fixture.target("json_merge");
        let calls = Cell::new(0);
        let error = toggle_target_with_backup(&target, "ragflow", &server(), true, || {
            calls.set(calls.get() + 1);
            Err(Error::Other("fake backup failed".into()))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "fake backup failed");
        assert_eq!(calls.get(), 1);
        if exists {
            assert_eq!(std::fs::read_to_string(&target.file).unwrap(), "{}");
        } else {
            assert!(!fixture.0.join("agent").exists());
        }
    }
}

#[test]
fn valid_noop_still_calls_backup_and_propagates_its_failure() {
    for exists in [true, false] {
        let fixture = Fixture::new();
        if exists {
            fixture.write("");
        }
        let target = fixture.target("toml_section");
        let calls = Cell::new(0);
        toggle_target_with_backup(&target, "ragflow", &server(), false, || {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(calls.get(), 1);
        let error = toggle_target_with_backup(&target, "ragflow", &server(), false, || {
            Err(Error::Other("noop backup failed".into()))
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "noop backup failed");
        assert_eq!(Path::new(&target.file).exists(), exists);
    }
}

#[test]
fn destination_changes_during_backup_are_not_overwritten() {
    for (original, external) in [
        (Some("{}"), Some("{\"external\":true}")),
        (Some("{}"), None),
        (None, Some("")),
        (Some(""), None),
    ] {
        let fixture = Fixture::new();
        if let Some(text) = original {
            fixture.write(text);
        }
        let target = fixture.target("json_merge");
        let error = toggle_target_with_backup(&target, "ragflow", &server(), true, || {
            if let Some(text) = external {
                fixture.write(text);
            } else {
                std::fs::remove_file(&target.file)?;
            }
            Ok(())
        })
        .unwrap_err();
        assert!(error.to_string().contains("changed during backup"));
        assert_eq!(
            std::fs::read_to_string(&target.file).ok().as_deref(),
            external
        );
    }
}

#[test]
fn destination_becoming_a_directory_during_backup_is_rejected() {
    let fixture = Fixture::new();
    fixture.write("{}");
    let target = fixture.target("json_merge");
    let error = toggle_target_with_backup(&target, "ragflow", &server(), true, || {
        std::fs::remove_file(&target.file)?;
        std::fs::create_dir(&target.file)?;
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("regular file"));
    assert!(Path::new(&target.file).is_dir());
}

#[cfg(unix)]
#[test]
fn destination_becoming_a_symlink_during_backup_is_rejected() {
    let fixture = Fixture::new();
    fixture.write("{}");
    let target = fixture.target("json_merge");
    let outside = fixture.0.join("outside");
    std::fs::write(&outside, "{}").unwrap();
    let error = toggle_target_with_backup(&target, "ragflow", &server(), true, || {
        std::fs::remove_file(&target.file)?;
        std::os::unix::fs::symlink(&outside, &target.file)?;
        Ok(())
    })
    .unwrap_err();
    assert!(error.to_string().contains("symlinks"));
    assert!(
        std::fs::symlink_metadata(&target.file)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "{}");
}

#[test]
fn prepared_change_uses_original_target_and_definition_after_registry_changes() {
    let fixture = Fixture::new();
    let paths = crate::registry::DataPaths::from_data_dir(fixture.0.join("data"));
    std::fs::create_dir_all(&paths.registry_dir).unwrap();
    let mut doc =
        crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
            .unwrap();
    doc.agents.get_mut("codex").unwrap().config_inject.mcp = Some(fixture.target("json_merge"));
    let registry = paths.registry_dir.join("registry-v3.json");
    std::fs::write(&registry, serde_json::to_vec(&doc).unwrap()).unwrap();
    let definitions = paths.registry_dir.join("mcp-registry.json");
    std::fs::write(
        &definitions,
        r#"{"servers":[{"name":"ragflow","command":"npx","args":["original"]}]}"#,
    )
    .unwrap();
    let target =
        agent_target_from(crate::registry::load_registry_at(&paths).unwrap(), "codex").unwrap();
    #[derive(Deserialize)]
    struct RegistryFile {
        servers: Vec<McpServerDef>,
    }
    let definitions_snapshot: RegistryFile =
        serde_json::from_str(&crate::registry::load_mcp_registry_at(&paths).unwrap()).unwrap();
    let def = &definitions_snapshot.servers[0];
    let redirected = fixture.0.join("unexpected-config");
    toggle_target_with_backup(&target, "ragflow", def, true, || {
        doc.agents
            .get_mut("codex")
            .unwrap()
            .config_inject
            .mcp
            .as_mut()
            .unwrap()
            .file = redirected.to_string_lossy().into_owned();
        std::fs::write(&registry, serde_json::to_vec(&doc)?)?;
        std::fs::write(
            &definitions,
            r#"{"servers":[{"name":"ragflow","command":"powershell"}]}"#,
        )?;
        Ok(())
    })
    .unwrap();
    let written = std::fs::read_to_string(&target.file).unwrap();
    assert!(written.contains("npx"));
    assert!(written.contains("original"));
    assert!(!written.contains("powershell"));
    assert!(!redirected.exists());
}
