//! Static catalog regressions, not agent installation or authentication tests.
//!
//! Inputs are compile-time repository fixtures. These tests do not resolve HOME,
//! spawn catalog commands, inspect credentials, or contact upstream providers.

use aisync_core::registry::{
    AuthType, InstallMethod, RegistryDoc, UiType, parse_registry, sorted_agents,
};
use serde_json::Value;

const BUNDLED: &str = include_str!("../../../agents/registry-v3.json");
const EXPECTATIONS: &str =
    include_str!("../../../tests/fixtures/catalog-verification-2026-11-08.json");
const SOURCES_EN: &str = include_str!("../../../docs/REGISTRY-SOURCES.md");
const SOURCES_ZH: &str = include_str!("../../../docs/REGISTRY-SOURCES.zh-CN.md");

fn catalog() -> RegistryDoc {
    parse_registry(BUNDLED).expect("bundled registry must remain schema-v3 compatible")
}

fn expectations() -> Value {
    serde_json::from_str(EXPECTATIONS).expect("dated static expectations must parse")
}

#[test]
fn bundled_schema_and_core_ids_remain_compatible_without_promoting_future_entries() {
    let catalog = catalog();
    let expected = expectations();
    assert_eq!(
        catalog.version,
        expected["schema_version"].as_u64().unwrap() as u32
    );
    assert_eq!(catalog.updated_at, expected["verification_date"]);
    assert_eq!(expected["runtime_verification"], false);

    let mut ids: Vec<_> = catalog.agents.keys().map(String::as_str).collect();
    ids.sort_unstable();
    let expected_ids: Vec<_> = expected["core_agent_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(ids, expected_ids);
    for (id, agent) in &catalog.agents {
        assert_eq!(&agent.id, id, "parser must still populate the agent ID");
    }
    let future = catalog.future_registry.as_ref().unwrap();
    let mut future_ids: Vec<_> = future.agents.keys().map(String::as_str).collect();
    future_ids.sort_unstable();
    let expected_future_ids: Vec<_> = expected["future_agent_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect();
    assert_eq!(future_ids, expected_future_ids);
    for id in future.agents.keys() {
        assert!(
            !catalog.agents.contains_key(id),
            "future entry was activated: {id}"
        );
    }
}

#[test]
fn reviewed_npm_entries_match_documented_node_minima_and_fixed_recipes() {
    let catalog = catalog();
    for (id, expected) in expectations()["npm"].as_object().unwrap() {
        let install = &catalog.agents[id].install;
        assert_eq!(install.method, InstallMethod::Npm, "{id}");
        let node_requirements: Vec<_> = install
            .dependencies
            .iter()
            .filter(|value| value.starts_with("node"))
            .map(String::as_str)
            .collect();
        assert_eq!(
            node_requirements,
            [expected["node_dependency"].as_str().unwrap()],
            "{id}"
        );
        assert_eq!(
            install.command.as_deref(),
            expected["command"].as_str(),
            "{id}"
        );
        assert_eq!(
            install.uninstall_cmd.as_deref(),
            expected["uninstall_cmd"].as_str(),
            "{id}"
        );
        assert_eq!(
            install.verify_cmd.as_deref(),
            expected["verify_cmd"].as_str(),
            "{id}"
        );
    }
}

#[test]
fn authentication_guidance_uses_documented_entry_points() {
    let catalog = catalog();
    for (id, expected) in expectations()["authentication"].as_object().unwrap() {
        let instructions = catalog.agents[id]
            .auth_guide
            .as_ref()
            .unwrap()
            .steps
            .join("\n");
        for fragment in expected["required_fragments"].as_array().unwrap() {
            assert!(
                instructions.contains(fragment.as_str().unwrap()),
                "{id}: {fragment}"
            );
        }
        for fragment in expected["forbidden_fragments"].as_array().unwrap() {
            assert!(
                !instructions.contains(fragment.as_str().unwrap()),
                "{id}: {fragment}"
            );
        }
    }
}

#[test]
fn qwen_is_manual_with_passive_detection_and_no_authentication_or_write_claims() {
    let catalog = catalog();
    let qwen = &catalog.agents["qwen-code"];
    assert_eq!(qwen.id, "qwen-code");
    assert_eq!(qwen.name, "Qwen Code");
    assert_eq!(qwen.source, "open");
    assert_eq!(qwen.license.as_deref(), Some("Apache-2.0"));
    assert_eq!(qwen.github.as_deref(), Some("QwenLM/qwen-code"));
    assert_eq!(
        qwen.homepage.as_deref(),
        Some("https://github.com/QwenLM/qwen-code")
    );
    assert_eq!(qwen.ui_type, UiType::Cli);
    assert_eq!(qwen.category, "ai-coding");
    assert_eq!(qwen.icon, "qwen-code");
    assert!(qwen.stars.is_none());
    assert!(!qwen.popular);
    assert_eq!(qwen.install.method, InstallMethod::Manual);
    assert!(qwen.install.command.is_none());
    assert_eq!(qwen.install.verify_cmd.as_deref(), Some("qwen --version"));
    assert_eq!(qwen.install.dependencies, ["node >= 22"]);
    assert!(qwen.install.uninstall_cmd.is_none());
    assert!(qwen.install.extension_id.is_none());
    assert_eq!(
        qwen.install.url.as_deref(),
        expectations()["qwen_source_review"]["manual_install_url"].as_str()
    );
    assert_eq!(qwen.auth_type, AuthType::AccountLogin);
    // No completion hint: do not invent a credential path or infer logged-in state.
    assert!(qwen.auth_guide.as_ref().unwrap().completion_check.is_none());
    assert!(qwen.config_inject.model_config.is_none());
    assert!(qwen.config_inject.personalization.is_none());
    assert!(qwen.config_inject.mcp.is_none());
    assert!(!qwen.config_inject.skills);
    assert!(!qwen.config_inject.memory);
    assert!(qwen.memory_collector.is_none());
    // Capability queries only: none probe PATH, initialize a profile, or spawn an installer.
    assert!(!aisync_core::deploy::can_install(qwen));
    assert!(!aisync_core::deploy::can_configure(qwen));
    assert!(!aisync_core::deploy::can_uninstall(qwen));
    assert!(!aisync_core::config::supports_personalization(qwen));
    let bundled: Value = serde_json::from_str(BUNDLED).unwrap();
    let raw = &bundled["agents"]["qwen-code"];
    assert!(raw["auth_guide"]["completion_check"].is_null());
    assert!(
        raw.get("version").is_none(),
        "reviewed release is not an observed installed version"
    );
    for field in ["upgrade_cmd", "update_cmd"] {
        assert!(raw["install"].get(field).is_none());
    }
}

#[test]
fn qwen_source_snapshot_is_not_runtime_or_npm_publication_evidence() {
    let expected = expectations();
    let review = &expected["qwen_source_review"];
    assert_eq!(review["upstream_release"], "v0.26.9");
    assert_eq!(
        review["source_commit"],
        "600aff0592bcfc21668ca4aba95a5b20c6491b86"
    );
    assert_eq!(review["npm_package"], "@qwen-code/qwen-code");
    assert_eq!(review["published_bin"], "cli-entry.js");
    assert_eq!(review["install_method"], "manual");
    for field in [
        "npm_registry_verified",
        "runtime_verification",
        "automatic_installation",
        "runtime_probe",
        "source_association",
    ] {
        assert_eq!(review[field], false, "{field}");
    }
    let catalog = catalog();
    let qwen = &catalog.agents["qwen-code"];
    assert_eq!(
        qwen.install.dependencies,
        [review["node_dependency"].as_str().unwrap()]
    );
    assert_eq!(
        qwen.install.verify_cmd.as_deref(),
        review["verification_marker"].as_str()
    );
    let bundled: Value = serde_json::from_str(BUNDLED).unwrap();
    for field in ["command", "uninstall_cmd"] {
        assert_eq!(
            bundled["agents"]["qwen-code"]["install"].get(field),
            Some(&Value::Null)
        );
    }
    for document in [SOURCES_EN, SOURCES_ZH] {
        for field in [
            "upstream_release",
            "source_commit",
            "npm_package",
            "documented_npm_command",
            "node_dependency",
            "published_bin",
            "manual_install_url",
            "published_release_excluded",
        ] {
            assert!(
                document.contains(review[field].as_str().unwrap()),
                "missing Qwen source context: {field}"
            );
        }
        for boundary in [
            "install.method: manual",
            "install.command: null",
            "uninstall_cmd: null",
            "verify_cmd: qwen --version",
            "completion_check: null",
            "QWEN_CODE_RELAUNCH_ARGS",
            "CLI_VERSION",
            "QWEN_HOME",
            "Unsupported",
            "NotDetected",
            "Plus/Pro",
            "/auth",
        ] {
            assert!(
                document.contains(boundary),
                "missing Qwen boundary: {boundary}"
            );
        }
    }
    assert!(SOURCES_EN.contains("not included in the published v1.1.1 installer"));
    assert!(SOURCES_ZH.contains("不包含在已发布的 v1.1.1 安装包中"));
    assert!(SOURCES_EN.contains("AgentDeck does not run `qwen --version`"));
    assert!(SOURCES_ZH.contains("AgentDeck 对该条目不运行 `qwen --version`"));
    assert!(
        SOURCES_EN.contains("not verify the npm dist-tag, tarball integrity or npm publication")
    );
    assert!(SOURCES_ZH.contains("没有验证 npm dist-tag、tarball 完整性或 npm 发布状态"));
}

#[test]
fn pi_keeps_unverified_write_adapters_disabled_without_guessing_new_paths() {
    let catalog = catalog();
    let pi = &catalog.agents["pi"];
    assert_eq!(pi.github.as_deref(), Some("earendil-works/pi"));
    assert!(pi.config_inject.model_config.is_none());
    assert!(pi.config_inject.personalization.is_none());
    assert!(pi.config_inject.mcp.is_none());
    assert!(!pi.config_inject.skills);
    assert!(!pi.config_inject.memory);
    assert!(pi.memory_collector.is_none());
    assert!(!BUNDLED.contains("~/.pi/config.json"));
}

#[test]
fn opencode_keeps_v1_recipe_and_instructions_without_claiming_v2_config_support() {
    let catalog = catalog();
    let opencode = &catalog.agents["opencode"];
    let expected = expectations();
    let boundary = &expected["opencode_version_boundary"];
    assert_eq!(opencode.install.method, InstallMethod::Npm);
    assert_eq!(
        opencode.install.command.as_deref(),
        boundary["install_command"].as_str()
    );
    assert_eq!(
        opencode.install.uninstall_cmd.as_deref(),
        boundary["uninstall_command"].as_str()
    );
    assert_eq!(
        opencode.install.verify_cmd.as_deref(),
        boundary["verify_command"].as_str()
    );
    assert!(opencode.description.contains("V1"));
    assert!(opencode.description.contains("V2"));
    assert!(opencode.config_inject.mcp.is_none());
    assert!(opencode.config_inject.model_config.is_none());
    let instructions = opencode.config_inject.personalization.as_ref().unwrap();
    assert_eq!(instructions.target, boundary["personalization_target"]);
    assert_eq!(instructions.template, boundary["personalization_template"]);
    for document in [SOURCES_EN, SOURCES_ZH] {
        for fact in [
            "opencode-ai",
            "@opencode/cli",
            "mcp.servers.<name>",
            "~/.config/opencode/AGENTS.md",
        ] {
            assert!(document.contains(fact), "missing version boundary: {fact}");
        }
    }
}

#[test]
fn hermes_stays_manual_without_reenabling_unverified_write_adapters() {
    let catalog = catalog();
    let hermes = &catalog.agents["hermes"];
    assert_eq!(hermes.install.method, InstallMethod::Manual);
    assert!(hermes.install.command.is_none());
    assert!(hermes.install.uninstall_cmd.is_none());
    assert_eq!(
        hermes.install.verify_cmd.as_deref(),
        Some("hermes --version")
    );
    assert!(hermes.config_inject.model_config.is_none());
    assert!(hermes.config_inject.personalization.is_none());
    assert!(hermes.config_inject.mcp.is_none());
}

#[test]
fn corrected_source_only_entries_have_no_executable_recipe_or_config_adapter() {
    let catalog = catalog();
    let future = catalog.future_registry.as_ref().unwrap();
    for (id, expected) in expectations()["source_only"].as_object().unwrap() {
        assert!(!catalog.agents.contains_key(id));
        let entry = &future.agents[id];
        assert_eq!(entry["github"], expected["github"], "{id}");
        assert_eq!(entry["install"]["method"], expected["method"], "{id}");
        assert_eq!(entry["status"], expected["status"], "{id}");
        assert_ne!(entry["popular"], true, "{id}");
        for field in ["command", "uninstall_cmd", "verify_cmd", "extension_id"] {
            assert!(entry["install"][field].is_null(), "{id}: {field}");
        }
        assert!(entry["config_inject"].is_null(), "{id}");
    }
    let roo = &future.agents["roocode"];
    assert_eq!(roo["status"], "archived");
    assert!(roo["description"].as_str().unwrap().contains("2026-05-15"));
    assert!(!BUNDLED.contains("charms/crush"));
    assert!(!BUNDLED.contains("oh-my-pi/oh-my-pi"));
    assert!(!BUNDLED.contains("RooVetGit/Roo-Code"));
}

#[test]
fn reviewed_descriptions_do_not_repeat_unverified_metrics_or_rankings() {
    let bundled: Value = serde_json::from_str(BUNDLED).unwrap();
    let expected = expectations();
    for (id, entry) in bundled["agents"]
        .as_object()
        .unwrap()
        .iter()
        .chain(bundled["future_registry"]["agents"].as_object().unwrap())
    {
        let mut description = entry["description"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if let Some(verified_numbers) =
            expected["popularity_evidence"]["description_numeric_allowlist"][id].as_array()
        {
            for verified in verified_numbers {
                description = description.replace(verified.as_str().unwrap(), "");
            }
        }
        assert!(
            !description
                .chars()
                .any(|character| character.is_ascii_digit()),
            "{id}"
        );
        for claim in ["最佳", "最大", "登顶", "慷慨", "永久免费", "全平台覆盖"] {
            assert!(!description.contains(claim), "{id}: {claim}");
        }
    }
}

#[test]
fn all_core_and_reference_entries_withdraw_unverified_popularity_claims() {
    let bundled: Value = serde_json::from_str(BUNDLED).unwrap();
    let expected = expectations();
    assert_eq!(expected["popularity_evidence"]["verified"], false);
    for (id, entry) in bundled["agents"]
        .as_object()
        .unwrap()
        .iter()
        .chain(bundled["future_registry"]["agents"].as_object().unwrap())
    {
        assert_eq!(entry.get("stars"), Some(&Value::Null), "{id}");
        assert_eq!(entry["popular"], false, "{id}");
    }
    for agent in catalog().agents.values() {
        assert!(agent.stars.is_none(), "null must stay unknown, not zero");
        assert!(!agent.popular);
    }
}

#[test]
fn unranked_catalog_retains_installed_priority_without_inventing_a_tie_order() {
    let catalog = catalog();
    let sorted = sorted_agents(&catalog, &["pi".to_string()]);
    assert_eq!(sorted.len(), catalog.agents.len());
    assert_eq!(sorted[0].0, "pi");
    assert!(
        sorted
            .iter()
            .all(|(_, agent)| !agent.popular && agent.stars.is_none())
    );
    // Equal unranked, uninstalled entries do not carry a certified ranking.
}

#[test]
fn bilingual_source_notes_record_scope_and_do_not_claim_runtime_verification() {
    for document in [SOURCES_EN, SOURCES_ZH] {
        for source in expectations()["sources"].as_array().unwrap() {
            assert!(
                document.contains(source.as_str().unwrap()),
                "missing source: {source}"
            );
        }
        for fact in [
            "2026-11-06",
            "2026-11-02",
            "2026-05-15",
            "PI_CODING_AGENT_DIR",
            "~/.pi/agent",
            "completion_check",
        ] {
            assert!(
                document.contains(fact),
                "missing verification context: {fact}"
            );
        }
    }
    assert!(SOURCES_EN.contains("No agent, installer, login flow"));
    assert!(SOURCES_ZH.contains("没有运行 Agent、安装器、登录流程"));
    assert!(SOURCES_EN.contains("No popularity ranking is certified"));
    assert!(SOURCES_ZH.contains("本目录不认证热度排名"));
}
