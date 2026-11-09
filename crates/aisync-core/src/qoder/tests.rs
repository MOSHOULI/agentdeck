use super::*;
use std::{cell::RefCell, collections::BTreeMap, path::PathBuf};
struct Fake {
    files: BTreeMap<PathBuf, Vec<u8>>,
    reads: RefCell<Vec<PathBuf>>,
}
impl SelectedFileReader for Fake {
    fn read_selected(&self, path: &Path) -> Result<Vec<u8>, InspectionIssue> {
        self.reads.borrow_mut().push(path.to_owned());
        self.files
            .get(path)
            .cloned()
            .ok_or(InspectionIssue::Missing)
    }
}
fn path(dir: &str, name: &str) -> PathBuf {
    std::env::temp_dir().join("qoder-unit").join(dir).join(name)
}
fn fake(items: &[(&Path, &str)]) -> Fake {
    Fake {
        files: items
            .iter()
            .map(|(p, s)| (p.to_path_buf(), s.as_bytes().to_vec()))
            .collect(),
        reads: RefCell::new(vec![]),
    }
}
#[test]
fn scopes_count_overlap_without_exposing_values_names_or_paths() {
    let user = path("user", "settings.json");
    let project = path("project", ".mcp.json");
    let reader = fake(&[
        (
            &user,
            r#"{"mcpServers":{"SECRET_SERVER":{"command":"SECRET_CMD"}},"model":{"name":"SECRET_MODEL"},"PRIVATE":"SECRET_VALUE"}"#,
        ),
        (
            &project,
            r#"{"mcpServers":{"SECRET_SERVER":{"url":"SECRET_URL"},"OTHER_SECRET":{}}}"#,
        ),
    ]);
    let report = inspect_with(
        &reader,
        &Selections {
            user_settings: Some(&user),
            project_mcp: Some(&project),
            ..Default::default()
        },
    );
    assert_eq!(report.repeated_mcp_identifier_count, Some(1));
    assert_eq!(
        report.scopes[0].summary.as_ref().unwrap().mcp_server_count,
        1
    );
    assert_eq!(
        report.scopes[3].summary.as_ref().unwrap().mcp_server_count,
        2
    );
    assert_eq!(report.scopes[1].status, Status::NotSelected);
    let serialized = serde_json::to_string(&report).unwrap();
    for text in [
        "SECRET",
        "PRIVATE",
        user.to_str().unwrap(),
        project.to_str().unwrap(),
    ] {
        assert!(!serialized.contains(text));
    }
    assert!(!report.can_apply && !report.can_restore);
    assert_eq!(*reader.reads.borrow(), vec![user, project]);
}
#[test]
fn invalid_sibling_path_rejects_all_reads() {
    let user = path("user", "settings.json");
    let bad = path("other", "credentials.json");
    let reader = fake(&[(&user, "{}")]);
    let report = inspect_with(
        &reader,
        &Selections {
            user_settings: Some(&user),
            local_settings: Some(&bad),
            ..Default::default()
        },
    );
    assert_eq!(report.selection_issue, Some(SelectionIssue::InvalidPath));
    assert_eq!(report.scopes[0].status, Status::NotRead);
    assert!(reader.reads.borrow().is_empty());
}
#[test]
fn duplicate_path_is_not_a_scope_overlap() {
    let user = path("user", "settings.json");
    let reader = fake(&[(&user, "{}")]);
    let report = inspect_with(
        &reader,
        &Selections {
            user_settings: Some(&user),
            project_settings: Some(&user),
            ..Default::default()
        },
    );
    assert_eq!(report.selection_issue, Some(SelectionIssue::RepeatedPath));
    assert!(reader.reads.borrow().is_empty());
}
#[test]
fn nothing_selected_and_relative_paths_rejected() {
    assert_eq!(
        validate_selections(&Selections::default()),
        Err(SelectionIssue::NothingSelected)
    );
    assert_eq!(
        validate_path(Path::new("settings.json"), Scope::UserSettings),
        Err(InspectionIssue::PathMustBeAbsolute)
    );
    assert_eq!(
        validate_path(&path("..", "settings.json"), Scope::UserSettings),
        Err(InspectionIssue::ParentTraversal)
    );
}
#[test]
fn missing_or_invalid_selected_layer_hides_overlap_count() {
    let user = path("user", "settings.json");
    let local = path("project", "settings.local.json");
    for contents in [
        None,
        Some("[]"),
        Some("{SECRET invalid"),
        Some(r#"{"mcpServers":null}"#),
        Some(r#"{"mcpServers":{"secret":"wrong"}}"#),
    ] {
        let mut reader = fake(&[(&user, "{}")]);
        if let Some(contents) = contents {
            reader
                .files
                .insert(local.clone(), contents.as_bytes().to_vec());
        }
        let report = inspect_with(
            &reader,
            &Selections {
                user_settings: Some(&user),
                local_settings: Some(&local),
                ..Default::default()
            },
        );
        assert_eq!(report.repeated_mcp_identifier_count, None);
        assert_eq!(report.scopes[0].status, Status::Read);
        assert!(report.scopes[2].summary.is_none());
        assert!(!serde_json::to_string(&report).unwrap().contains("SECRET"));
    }
}
#[test]
fn comments_urls_and_escaped_quotes_are_distinguished() {
    let json=b"{ // comment SECRET\r\n \"mcpServers\": {\"x\":{\"url\":\"https://example.test\",\"x\":\"\\\"//not comment\"}}\n} // trailing";
    let value = parse_jsonc(json).unwrap();
    assert_eq!(value["mcpServers"]["x"]["url"], "https://example.test");
    assert_eq!(value["mcpServers"]["x"]["x"], "\"//not comment");
}
#[test]
fn ambiguous_or_unsupported_syntax_is_not_silently_fixed() {
    for json in [
        r#"{"x":1,"x":2}"#,
        r#"{"x":1,"\u0078":2}"#,
        r#"{"x":1,}"#,
        "{/*comment*/}",
        "{\"x\":\"bad\ntext\"}",
        "{} {}",
        "\u{feff}{}",
    ] {
        assert!(parse_jsonc(json.as_bytes()).is_err(), "{json}");
    }
}
#[test]
fn parser_enforces_limits_before_and_after_comment_removal() {
    assert_eq!(
        parse_jsonc(&vec![b' '; LIMITS.max_manifest_bytes + 1]),
        Err(InspectionIssue::FileTooLarge)
    );
    let long = format!(
        "{{\"x\":\"{}\"}}",
        "x".repeat(LIMITS.max_json_string_bytes + 1)
    );
    assert_eq!(
        parse_jsonc(long.as_bytes()),
        Err(InspectionIssue::JsonLimit)
    );
    let deep = format!("{}0{}", "[".repeat(17), "]".repeat(17));
    assert_eq!(
        parse_jsonc(deep.as_bytes()),
        Err(InspectionIssue::JsonLimit)
    );
}
#[test]
fn all_four_scopes_are_observed_and_three_way_repeat_count_is_unique() {
    let u = path("u", "settings.json");
    let p = path("p", "settings.json");
    let l = path("p", "settings.local.json");
    let m = path("p", ".mcp.json");
    let text = r#"{"mcpServers":{"x":{}}}"#;
    let reader = fake(&[(&u, text), (&p, text), (&l, text), (&m, text)]);
    let report = inspect_with(
        &reader,
        &Selections {
            user_settings: Some(&u),
            project_settings: Some(&p),
            local_settings: Some(&l),
            project_mcp: Some(&m),
        },
    );
    assert_eq!(reader.reads.borrow().len(), 4);
    assert!(report.scopes.iter().all(|s| s.status == Status::Read));
    assert_eq!(report.repeated_mcp_identifier_count, Some(1));
}
#[test]
fn fixed_section_presence_is_not_value_validation() {
    let (summary,_) = summarize(serde_json::json!({"permissions":null,"hooks":[],"model":0,"security":false,"private_key":"secret"})).unwrap();
    assert!(
        summary.permissions_present
            && summary.hooks_present
            && summary.model_present
            && summary.security_present
            && summary.other_top_level_fields_present
    );
    assert_eq!(summary.mcp_server_count, 0);
}

#[test]
fn invalid_utf8_in_comments_is_rejected() {
    assert_eq!(
        parse_jsonc(b"{//\xff\n}"),
        Err(InspectionIssue::UnsupportedJson)
    );
}
