use super::*;

#[test]
fn fixed_report_preserves_all_root_edges_without_resolving_ranges() {
    let report = report().unwrap();
    assert_eq!(report.schema_version, 2);
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
            .contains("not a resolved or complete dependency closure")
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

#[test]
fn sdk_minimal_establishes_declared_paths_but_never_closure() {
    let r = report().unwrap();
    assert_eq!(r.production_paths.len(), 2);
    assert_eq!(
        r.sdk_minimal_metadata.package,
        "@deepseek-ai/dsh-sdk-minimal"
    );
    for path in &r.production_paths {
        assert_eq!(path.packages.len(), 4);
        assert_eq!(path.versions.len(), 4);
        assert_eq!(path.packages[0], distribution::NAME);
        assert_eq!(path.packages[1], SDK.package);
        assert_eq!(path.packages[2], SAMPLES[0].package);
        assert_eq!(path.edge_kind, "dependencies_exact_declaration");
    }
    assert!(!r.closure_complete && !r.install_ready && !r.dependencies_resolved);
}
#[test]
fn production_edge_rejects_missing_wrong_version_and_peer_or_optional_impersonation() {
    for snapshot in [&SDK, &SAMPLES[0], &SAMPLES[1], &SAMPLES[2]] {
        let child = parse_metadata(
            snapshot.bytes,
            snapshot.sha256,
            snapshot.package,
            snapshot.version,
        )
        .unwrap();
        let parent_value = serde_json::json!({"name":"parent","version":"0.0.0","dependencies": {snapshot.package: snapshot.version}});
        let parent: Metadata = serde_json::from_value(parent_value.clone()).unwrap();
        assert!(require_edge(&parent, &child).is_ok());
        for field in [
            "peerDependencies",
            "optionalDependencies",
            "devDependencies",
        ] {
            let mut v = parent_value.clone();
            v[field] = v["dependencies"].take();
            let parent: Metadata = serde_json::from_value(v).unwrap();
            assert!(require_edge(&parent, &child).is_err());
        }
        for version in ["*", "^0.2.2-rc.1", "0.2.2", "1.0.0", ""] {
            let mut v = parent_value.clone();
            v["dependencies"][snapshot.package] = serde_json::json!(version);
            let parent: Metadata = serde_json::from_value(v).unwrap();
            assert!(require_edge(&parent, &child).is_err());
        }
        let mut v = parent_value;
        v["dependencies"] = serde_json::json!({"wrong-name":snapshot.version});
        assert!(require_edge(&serde_json::from_value(v).unwrap(), &child).is_err());
    }
}
#[test]
fn actual_subprocess_manifest_matches_reviewed_registry_declarations() {
    let bytes = include_bytes!(
        "../../../../../docs/upstream/dsh/dependency-evidence/subprocess-package-0.2.2-rc.1.json"
    );
    validate_subprocess_manifest(bytes).unwrap();
    let mut modified = bytes.to_vec();
    modified[0] = b' ';
    assert!(validate_subprocess_manifest(&modified).is_err());
}
