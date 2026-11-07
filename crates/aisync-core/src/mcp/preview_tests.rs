//! Metadata-only synthetic preview tests; no real agents, credentials or backups.
use super::*;
use std::cell::Cell;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("mcp-preview-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn target(&self) -> crate::registry::McpInject {
        crate::registry::McpInject {
            file: self
                .0
                .join("agent/config.json")
                .to_string_lossy()
                .into_owned(),
            format: "json_merge".into(),
            key: Some("mcpServers".into()),
            section: None,
        }
    }
    fn write(&self, text: &str) {
        std::fs::create_dir_all(self.0.join("agent")).unwrap();
        std::fs::write(self.0.join("agent/config.json"), text).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn definition() -> McpServerDef {
    McpServerDef {
        name: "synthetic".into(),
        description: None,
        url: Some("http://127.0.0.1:12345/mcp?token=synthetic-secret".into()),
        command: None,
        args: vec![],
        disabled: false,
    }
}
fn prepare(
    target: &crate::registry::McpInject,
    def: &McpServerDef,
    enabled: bool,
) -> (PreparedToggle, McpTogglePreview) {
    let prepared = PreparedToggle::new(target, "synthetic", def, enabled).unwrap();
    let preview =
        preview_for_prepared(&prepared, "test-agent", "synthetic", enabled, target, def).unwrap();
    (prepared, preview)
}

#[test]
fn preview_of_missing_destination_creates_nothing_and_returns_only_metadata() {
    let fixture = Fixture::new();
    let (_, preview) = prepare(&fixture.target(), &definition(), true);
    assert!(!fixture.0.join("agent").exists());
    assert!(!preview.file_exists);
    assert!(preview.changed);
    let value = serde_json::to_value(&preview).unwrap();
    assert_eq!(value.as_object().unwrap().len(), 8);
    assert_eq!(value["agentId"], "test-agent");
    assert_eq!(value["serverName"], "synthetic");
    assert_eq!(value["format"], "json_merge");
    assert_eq!(preview.revision.len(), 64);
    let json = value.to_string();
    assert!(!json.contains("synthetic-secret"));
    assert!(!json.contains("12345"));
    assert!(!json.contains("updated"));
}

#[test]
fn existing_configuration_secrets_never_appear_in_preview() {
    let fixture = Fixture::new();
    let text = r#"{"apiKey":"existing-secret","mcpServers":{"custom":{"args":["private-arg"]}}}"#;
    fixture.write(text);
    let (_, preview) = prepare(&fixture.target(), &definition(), true);
    let json = serde_json::to_string(&preview).unwrap();
    for secret in ["existing-secret", "private-arg", "synthetic-secret"] {
        assert!(!json.contains(secret));
    }
    assert_eq!(
        std::fs::read_to_string(fixture.target().file).unwrap(),
        text
    );
    assert_eq!(
        std::fs::read_dir(fixture.0.join("agent")).unwrap().count(),
        1
    );
}

#[test]
fn revision_is_stable_but_binds_every_operation_identity() {
    let fixture = Fixture::new();
    let target = fixture.target();
    let def = definition();
    let (prepared, baseline) = prepare(&target, &def, true);
    assert_eq!(prepare(&target, &def, true).1.revision, baseline.revision);
    for (agent, server, enabled) in [
        ("other-agent", "synthetic", true),
        ("test-agent", "other-server", true),
        ("test-agent", "synthetic", false),
    ] {
        let changed =
            preview_for_prepared(&prepared, agent, server, enabled, &target, &def).unwrap();
        assert_ne!(changed.revision, baseline.revision);
    }
}

#[test]
fn stale_original_file_rejects_before_backup() {
    let fixture = Fixture::new();
    fixture.write("{}");
    let revision = prepare(&fixture.target(), &definition(), true).1.revision;
    fixture.write(r#"{"changed":true}"#);
    let (prepared, current) = prepare(&fixture.target(), &definition(), true);
    let error = apply_prepared_preview(prepared, &current, &revision, || {
        panic!("stale preview cannot back up")
    })
    .unwrap_err();
    assert!(error.to_string().contains("stale"));
    assert_eq!(
        std::fs::read_to_string(fixture.target().file).unwrap(),
        r#"{"changed":true}"#
    );
}

#[test]
fn file_creation_and_removal_invalidate_revision_even_when_empty() {
    let fixture = Fixture::new();
    let target = fixture.target();
    let missing = prepare(&target, &definition(), false).1.revision;
    fixture.write("");
    let (prepared, existing) = prepare(&target, &definition(), false);
    assert_ne!(missing, existing.revision);
    apply_prepared_preview(prepared, &existing, &missing, || panic!("created file")).unwrap_err();
    std::fs::remove_file(&target.file).unwrap();
    let (prepared, absent) = prepare(&target, &definition(), false);
    apply_prepared_preview(prepared, &absent, &existing.revision, || {
        panic!("removed file")
    })
    .unwrap_err();
}

#[test]
fn changed_selected_registry_definition_rejects_before_backup() {
    let fixture = Fixture::new();
    let target = fixture.target();
    let revision = prepare(&target, &definition(), true).1.revision;
    let mut changed = definition();
    changed.url = Some("http://127.0.0.1:54321/changed".into());
    let (prepared, preview) = prepare(&target, &changed, true);
    apply_prepared_preview(prepared, &preview, &revision, || {
        panic!("changed definition")
    })
    .unwrap_err();
    assert!(!fixture.0.join("agent").exists());
}

#[test]
fn selected_registry_target_metadata_binds_even_if_result_bytes_match() {
    let fixture = Fixture::new();
    let target = fixture.target();
    let (prepared, preview) = prepare(&target, &definition(), true);
    let mut changed_target = target;
    // An unused section still records that the selected registry descriptor changed.
    changed_target.section = Some("different-section".into());
    let current = preview_for_prepared(
        &prepared,
        "test-agent",
        "synthetic",
        true,
        &changed_target,
        &definition(),
    )
    .unwrap();
    assert_ne!(preview.revision, current.revision);
    apply_prepared_preview(prepared, &current, &preview.revision, || {
        panic!("changed target")
    })
    .unwrap_err();
}

#[test]
fn valid_preview_applies_exact_prepared_change_after_backup() {
    let fixture = Fixture::new();
    let target = fixture.target();
    let (prepared, preview) = prepare(&target, &definition(), true);
    let expected = prepared.updated.clone();
    let called = Cell::new(false);
    apply_prepared_preview(prepared, &preview, &preview.revision, || {
        assert!(!Path::new(&target.file).exists());
        called.set(true);
        Ok(())
    })
    .unwrap();
    assert!(called.get());
    assert_eq!(std::fs::read_to_string(target.file).unwrap(), expected);
}

#[test]
fn failed_backup_has_sanitized_error_and_leaves_configuration_untouched() {
    let fixture = Fixture::new();
    fixture.write("{}");
    let (prepared, preview) = prepare(&fixture.target(), &definition(), true);
    let error = apply_prepared_preview(prepared, &preview, &preview.revision, || {
        Err(Error::Other("secret-from-backup-error".into()))
    })
    .unwrap_err();
    assert!(!error.to_string().contains("secret-from-backup-error"));
    assert_eq!(
        std::fs::read_to_string(fixture.target().file).unwrap(),
        "{}"
    );
}

#[test]
fn change_during_backup_is_detected_and_never_overwritten() {
    let fixture = Fixture::new();
    fixture.write("{}");
    let (prepared, preview) = prepare(&fixture.target(), &definition(), true);
    apply_prepared_preview(prepared, &preview, &preview.revision, || {
        fixture.write(r#"{"external":true}"#);
        Ok(())
    })
    .unwrap_err();
    assert_eq!(
        std::fs::read_to_string(fixture.target().file).unwrap(),
        r#"{"external":true}"#
    );
}

#[test]
fn noop_preview_reports_no_change_and_preserves_existing_backup_semantics() {
    let fixture = Fixture::new();
    let target = fixture.target();
    let (prepared, _) = prepare(&target, &definition(), true);
    fixture.write(&prepared.updated);
    let (prepared, preview) = prepare(&target, &definition(), true);
    assert!(preview.file_exists);
    assert!(!preview.changed);
    let called = Cell::new(false);
    apply_prepared_preview(prepared, &preview, &preview.revision, || {
        called.set(true);
        Ok(())
    })
    .unwrap();
    assert!(called.get());
}

#[test]
fn invalid_preview_targets_reject_without_calling_backup() {
    for agent in ["opencode", "qwen-code", "not-an-agent"] {
        assert!(preview_toggle(agent, "ragflow", true).is_err());
        apply_preview_with_backup(agent, "ragflow", true, "invalid", || {
            panic!("invalid target cannot invoke backup")
        })
        .unwrap_err();
    }
}
