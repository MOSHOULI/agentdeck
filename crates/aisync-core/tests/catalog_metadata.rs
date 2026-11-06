//! Static catalog regressions, not agent installation or authentication tests.
//!
//! Inputs are compile-time repository fixtures. These tests do not resolve HOME,
//! spawn catalog commands, inspect credentials, or contact upstream providers.

use aisync_core::registry::{InstallMethod, RegistryDoc, parse_registry, sorted_agents};
use serde_json::Value;

const BUNDLED: &str = include_str!("../../../agents/registry-v3.json");
const EXPECTATIONS: &str =
    include_str!("../../../tests/fixtures/catalog-verification-2026-11-06.json");
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
    for id in catalog.future_registry.unwrap().agents.keys() {
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
    assert!(
        catalog.agents["claude-code"]
            .install
            .dependencies
            .iter()
            .any(|value| value == "git")
    );
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
