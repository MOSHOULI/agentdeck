use super::*;
const BINDING: &[u8] = b"caller-selected-instance";
fn change(bytes: &[u8], after: bool) -> PreparedChange {
    let p = preview(bytes, BINDING, after).unwrap();
    prepare(bytes, BINDING, after, &p.token).unwrap().unwrap()
}
#[test]
fn exact_token_only_preserves_comments_unicode_and_other_sections() {
    let before=b"# animations = true, not the field\ntext = \"true false\"\n[tui]\nanimations  = true # keep me\nother = true\n[tui.extra]\nanimations = true\n";
    let c = change(before, false);
    assert_eq!(c.output,b"# animations = true, not the field\ntext = \"true false\"\n[tui]\nanimations  = false # keep me\nother = true\n[tui.extra]\nanimations = true\n");
    assert_eq!(
        prepare_restore(&c.output, BINDING, &c.receipt)
            .unwrap()
            .unwrap()
            .output,
        before
    );
}
#[test]
fn crlf_quoted_keys_and_multiline_decoys_preserve_exact_bytes() {
    let before = "# 中文 true\r\ntext = '''\r\n[tui]\r\nanimations = true\r\n'''\r\n['tui']\r\n'animations' = false # false\r\n";
    let c = change(before.as_bytes(), true);
    assert_eq!(
        std::str::from_utf8(&c.output).unwrap(),
        "# 中文 true\r\ntext = '''\r\n[tui]\r\nanimations = true\r\n'''\r\n['tui']\r\n'animations' = true # false\r\n"
    );
    assert_eq!(
        prepare_restore(&c.output, BINDING, &c.receipt)
            .unwrap()
            .unwrap()
            .output,
        before.as_bytes()
    );
}
#[test]
fn missing_inline_dotted_wrong_type_or_profile_only_is_unsupported() {
    for input in [
        "",
        "[tui]\n",
        "tui.animations=true\n",
        "tui={animations=true}\n",
        "[tui]\nanimations='true'\n",
        "[tui]\nanimations=1\n",
        "[tui]\nanimations.foo=true\n",
        "[profiles.demo.tui]\nanimations=true\n",
        "[tui]\nanimations=true\nanimations=false\n",
        "[tui]\nanimations = true false\n",
    ] {
        assert_eq!(
            preview(input.as_bytes(), BINDING, false).unwrap_err(),
            ProtocolError::Unsupported,
            "{input}"
        );
    }
}
#[test]
fn noop_returns_original_observation_and_never_takes_ownership() {
    let b = b"[tui]\nanimations=true\n";
    let p = preview(b, BINDING, true).unwrap();
    assert!(!p.changes);
    assert!(prepare(b, BINDING, true, &p.token).unwrap().is_none());
}
#[test]
fn tokens_bind_operation_input_target_and_selection() {
    let b = b"[tui]\nanimations=true\n";
    let p = preview(b, BINDING, false).unwrap();
    assert_eq!(
        prepare(b, b"other", false, &p.token).unwrap_err(),
        ProtocolError::Changed
    );
    assert_eq!(
        prepare(b, BINDING, true, &p.token).unwrap_err(),
        ProtocolError::Changed
    );
    assert_eq!(
        prepare(
            b"# edit\n[tui]\nanimations=true\n",
            BINDING,
            false,
            &p.token
        )
        .unwrap_err(),
        ProtocolError::Changed
    );
    assert_eq!(
        prepare(b, BINDING, false, &digest(b)).unwrap_err(),
        ProtocolError::Changed
    );
}
#[test]
fn every_prepared_applied_and_restoring_image_is_explicit() {
    let b = b"[tui]\nanimations=true\n";
    let c = change(b, false);
    assert_eq!(
        recovery(b, BINDING, &c.receipt).unwrap(),
        RecoveryAction::CleanupReceipt
    );
    assert_eq!(
        recovery(&c.output, BINDING, &c.receipt).unwrap(),
        RecoveryAction::RestoreOriginal
    );
    assert_eq!(
        mark_applied(b, BINDING, &c.receipt).unwrap_err(),
        ProtocolError::Conflict
    );
    let applied = mark_applied(&c.output, BINDING, &c.receipt).unwrap();
    assert_eq!(*applied.phase(), Phase::Applied);
    assert_eq!(
        recovery(b, BINDING, &applied).unwrap_err(),
        ProtocolError::Conflict
    );
    assert_eq!(
        recovery(&c.output, BINDING, &applied).unwrap(),
        RecoveryAction::RestoreOriginal
    );
    let restoring = prepare_restore(&c.output, BINDING, &applied)
        .unwrap()
        .unwrap();
    assert_eq!(*restoring.receipt.phase(), Phase::Restoring);
    assert_eq!(
        recovery(&c.output, BINDING, &restoring.receipt).unwrap(),
        RecoveryAction::RestoreOriginal
    );
    assert_eq!(
        recovery(&restoring.output, BINDING, &restoring.receipt).unwrap(),
        RecoveryAction::CleanupReceipt
    );
    assert_eq!(restoring.output, b);
    assert!(
        prepare_restore(&restoring.output, BINDING, &restoring.receipt)
            .unwrap()
            .is_none()
    );
}
#[test]
fn outside_changes_always_conflict_without_rewriting() {
    let b = b"[tui]\nanimations=true\n";
    let c = change(b, false);
    let applied = mark_applied(&c.output, BINDING, &c.receipt).unwrap();
    let restoring = prepare_restore(&c.output, BINDING, &applied)
        .unwrap()
        .unwrap()
        .receipt;
    for receipt in [&c.receipt, &applied, &restoring] {
        for changed in [
            b"# comment\n[tui]\nanimations=false\n".as_slice(),
            b"[tui]\nanimations = false\n",
            b"[tui]\nanimations=false\nsecret='x'\n",
        ] {
            assert_eq!(
                recovery(changed, BINDING, receipt).unwrap_err(),
                ProtocolError::Conflict
            );
            assert_eq!(
                prepare_restore(changed, BINDING, receipt).unwrap_err(),
                ProtocolError::Conflict
            );
        }
    }
}
#[test]
fn receipt_roundtrip_is_redacted_and_debug_never_exposes_document() {
    let b = b"secret='SUPER_SECRET'\n[tui]\nanimations=true\n";
    let c = change(b, false);
    let serialized = serde_json::to_vec(&c.receipt).unwrap();
    assert!(!String::from_utf8_lossy(&serialized).contains("SUPER_SECRET"));
    assert!(!format!("{c:?}").contains("SUPER_SECRET"));
    let decoded = decode_receipt(&serialized).unwrap();
    assert_eq!(
        recovery(&c.output, BINDING, &decoded).unwrap(),
        RecoveryAction::RestoreOriginal
    );
    assert_eq!(
        recovery(&c.output, b"other", &decoded).unwrap_err(),
        ProtocolError::Changed
    );
}
#[test]
fn malformed_receipt_fields_and_duplicate_keys_are_rejected() {
    let c = change(b"[tui]\nanimations=true\n", false);
    let valid = serde_json::to_value(&c.receipt).unwrap();
    for (key, value) in [
        ("format", serde_json::json!(2)),
        ("schema_commit", serde_json::json!("other")),
        ("field", serde_json::json!("private")),
        ("phase", serde_json::json!("unknown")),
        ("before", serde_json::json!(false)),
        ("binding_digest", serde_json::json!("bad")),
        ("restore_digest", serde_json::json!("bad")),
        ("extra", serde_json::json!(true)),
    ] {
        let mut altered = valid.clone();
        altered[key] = value;
        assert_eq!(
            decode_receipt(&serde_json::to_vec(&altered).unwrap()).unwrap_err(),
            ProtocolError::Unsupported
        );
    }
    let json = String::from_utf8(serde_json::to_vec(&valid).unwrap()).unwrap();
    let duplicated = json.replacen('{', "{\"format\":1,", 1);
    assert_eq!(
        decode_receipt(duplicated.as_bytes()).unwrap_err(),
        ProtocolError::Unsupported
    );
}
#[test]
fn consistent_field_but_bad_inverse_digest_is_not_recoverable() {
    let b = b"[tui]\nanimations=true\n";
    let mut c = change(b, false);
    c.receipt.before_digest = digest(b"other before");
    assert_eq!(
        recovery(&c.output, BINDING, &c.receipt).unwrap_err(),
        ProtocolError::Conflict
    );
    c.receipt.after = true; // No-op receipt is never ownership.
    assert_eq!(
        recovery(&c.output, BINDING, &c.receipt).unwrap_err(),
        ProtocolError::Unsupported
    );
}
#[test]
fn inputs_and_structure_are_bounded() {
    assert_eq!(
        preview(&vec![b'x'; MAX_BYTES + 1], BINDING, true).unwrap_err(),
        ProtocolError::Unsupported
    );
    assert_eq!(
        preview(&[255], BINDING, true).unwrap_err(),
        ProtocolError::Unsupported
    );
    assert_eq!(
        preview(b"[tui]\nanimations=true\n", b"", false).unwrap_err(),
        ProtocolError::Unsupported
    );
    let many = format!(
        "values=[{}]\n[tui]\nanimations=true\n",
        vec!["0"; MAX_NODES + 1].join(",")
    );
    assert_eq!(
        preview(many.as_bytes(), BINDING, false).unwrap_err(),
        ProtocolError::Unsupported
    );
    let deep = format!(
        "value={}0{}\n[tui]\nanimations=true\n",
        "[".repeat(MAX_DEPTH + 1),
        "]".repeat(MAX_DEPTH + 1)
    );
    assert_eq!(
        preview(deep.as_bytes(), BINDING, false).unwrap_err(),
        ProtocolError::Unsupported
    );
}

#[test]
fn restoring_digest_and_transition_tampering_fail_closed() {
    let c = change(b"[tui]\nanimations=true\n", false);
    let r = prepare_restore(&c.output, BINDING, &c.receipt)
        .unwrap()
        .unwrap()
        .receipt;
    assert_eq!(
        mark_applied(&c.output, BINDING, &r).unwrap_err(),
        ProtocolError::Unsupported
    );
    for restore_digest in [None, Some(digest(b"other"))] {
        let mut bad = r.clone();
        bad.restore_digest = restore_digest;
        assert_eq!(
            recovery(&c.output, BINDING, &bad).unwrap_err(),
            ProtocolError::Unsupported
        );
    }
    for b in [
        b"".as_slice(),
        b"INVALID TOML",
        b"[tui]\nanimations='false'\n",
    ] {
        assert_eq!(
            recovery(b, BINDING, &r).unwrap_err(),
            ProtocolError::Conflict
        );
    }
}

#[test]
fn empty_array_tables_count_toward_complexity_limit() {
    let many = format!(
        "[tui]\nanimations=true\n{}",
        "[[other]]\n".repeat(MAX_NODES + 1)
    );
    assert_eq!(
        preview(many.as_bytes(), BINDING, false).unwrap_err(),
        ProtocolError::Unsupported
    );
}

#[test]
fn mixed_line_endings_preserve_exact_bytes_in_both_directions() {
    let before = b"# true\r\ntext = '''first\nanimations = true\r\nlast'''\n[tui]\r\nanimations = true # keep\nother = false\r\n";
    let c = change(before, false);
    assert_eq!(c.output, b"# true\r\ntext = '''first\nanimations = true\r\nlast'''\n[tui]\r\nanimations = false # keep\nother = false\r\n");
    assert_eq!(
        prepare_restore(&c.output, BINDING, &c.receipt)
            .unwrap()
            .unwrap()
            .output,
        before
    );
}

#[test]
fn bom_and_tui_array_tables_are_explicitly_unsupported() {
    for input in [
        "\u{feff}[tui]\nanimations=true\n",
        "[[tui]]\nanimations=true\n",
    ] {
        assert_eq!(
            preview(input.as_bytes(), BINDING, false).unwrap_err(),
            ProtocolError::Unsupported
        );
    }
}
