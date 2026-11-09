use super::*;

#[test]
fn fixed_report_preserves_all_root_edges_without_resolving_ranges() {
    let report = report().unwrap();
    assert_eq!(report.schema_version, 1);
    assert_eq!(report.root.direct_dependency_count, 90);
    assert_eq!(report.root.direct_exact_version_declaration_count, 82);
    assert_eq!(report.root.direct_unresolved_ranges.len(), 8);
    assert_eq!(report.root.direct_unresolved_ranges["commander"], "^15.0.0");
    assert_eq!(
        report.root.direct_unresolved_ranges["@deepseek-ai/cordis"],
        "~4.1.0"
    );
    assert_eq!(report.root.direct_unresolved_ranges["js-yaml"], "^4.3.2");
    assert!(report.root.declarations.scripts.is_none());
    assert!(report.root.declarations.peer_dependencies.is_none());
    assert!(report.root.declarations.optional_dependencies.is_none());
    assert!(!report.root.archive_verified_during_this_command);
    assert!(!report.root.full_implicit_lifecycle_assessed);
    assert_eq!(
        report.root.recorded_archive_state,
        "root_archive_verified_at_review"
    );
    assert!(!report.closure_complete && !report.install_ready && !report.dependencies_resolved);
    assert!(!report.signature_verified && !report.runtime_verified);
    assert_eq!(report.peer_resolution, "unresolved");
    assert_eq!(report.platform_compatibility, "unverified");
}

#[test]
fn illustrative_samples_preserve_lifecycle_peers_and_optional_specs() {
    let report = report().unwrap();
    let samples = &report.illustrative_exact_metadata;
    assert_eq!(samples.len(), 3);
    assert!(
        report
            .sample_scope
            .contains("Root reachability is not established")
    );
    let subprocess = &samples[0];
    assert_eq!(subprocess.package, "@deepseek-ai/dsh-subprocess-local");
    assert!(
        !report
            .root
            .declarations
            .dependencies
            .as_ref()
            .unwrap()
            .contains_key(subprocess.package)
    );
    assert_eq!(
        subprocess.declarations.dependencies.as_ref().unwrap()["node-pty"],
        "1.2.0-beta.15"
    );
    assert_eq!(
        subprocess.declarations.dependencies.as_ref().unwrap()["koffi"],
        "3.1.1"
    );
    assert_eq!(
        subprocess
            .declarations
            .peer_dependencies
            .as_ref()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        subprocess.declared_lifecycle_scripts["postinstall"],
        "node scripts/ensure-spawn-helper.mjs"
    );
    assert_eq!(
        samples[1].declarations.dependencies.as_ref().unwrap()["node-addon-api"],
        "^7.1.0"
    );
    assert_eq!(
        samples[1].declared_lifecycle_scripts["install"],
        "node scripts/prebuild.js || node-gyp rebuild"
    );
    assert!(
        samples[1]
            .declared_lifecycle_scripts
            .contains_key("postinstall")
    );
    assert!(
        samples[1]
            .declared_lifecycle_scripts
            .contains_key("prepare")
    );
    assert!(!samples[1].declared_lifecycle_scripts.contains_key("test"));
    assert!(
        samples[2]
            .declared_lifecycle_scripts
            .contains_key("install")
    );
    assert_eq!(
        samples[2]
            .declarations
            .optional_dependencies
            .as_ref()
            .unwrap()
            .len(),
        15
    );
    for sample in samples {
        assert_eq!(sample.state, "registry_metadata_observed");
        assert!(!sample.dependency_archive_verified);
        assert!(!sample.full_implicit_lifecycle_assessed);
        assert_eq!(sample.metadata_archive_agreement, "unchecked");
        assert!(sample.declarations.os.is_none());
        assert!(sample.declarations.cpu.is_none());
        assert!(sample.declarations.libc.is_none());
        assert!(sample.declarations.engines.is_none());
        assert!(sample.declarations.peer_dependencies_meta.is_none());
    }
}

#[test]
fn hash_drift_identity_and_invalid_declaration_shape_fail_closed() {
    let snapshot = &SAMPLES[0];
    let mut changed = snapshot.bytes.to_vec();
    changed[0] ^= 1;
    assert!(
        parse_metadata(
            &changed,
            snapshot.sha256,
            snapshot.package,
            snapshot.version
        )
        .is_err()
    );
    assert!(parse_metadata(snapshot.bytes, snapshot.sha256, "other", snapshot.version).is_err());
    assert!(parse_metadata(snapshot.bytes, snapshot.sha256, snapshot.package, "latest").is_err());
    for invalid in [
        b"not json".as_slice(),
        br#"{"name":"x","version":"1","dependencies":42}"#,
    ] {
        let hash = format!("{:x}", Sha256::digest(invalid));
        assert!(parse_metadata(invalid, &hash, "x", "1").is_err());
    }
    assert!(validate_hash(&vec![0; 65537], "ignored").is_err());
    assert!(!EvidenceError.to_string().contains(snapshot.source));
}

#[test]
fn serialization_is_deterministic_and_preserves_undeclared_fields_as_null() {
    let first = serde_json::to_value(report().unwrap()).unwrap();
    assert_eq!(first, serde_json::to_value(report().unwrap()).unwrap());
    assert!(first["root"]["declarations"]["optionalDependencies"].is_null());
    assert!(first["root"]["declarations"]["peerDependenciesMeta"].is_null());
    assert!(first["root"]["declarations"]["scripts"].is_null());
    assert_eq!(first["root"]["declarations"]["engines"]["node"], ">=24.9.0");
}
