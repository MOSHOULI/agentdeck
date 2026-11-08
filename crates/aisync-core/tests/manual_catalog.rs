//! The newly catalogued product surfaces are guidance, not executable adapters.
use aisync_core::{deploy, registry};

const IDS: [&str; 6] = [
    "cursor",
    "cursor-cli",
    "github-copilot-cli",
    "kimi-code",
    "kimi-code-desktop",
    "qoder-cli",
];

#[test]
fn mainstream_guidance_entries_have_no_executable_or_write_capabilities() {
    let doc = registry::parse_registry(include_str!("../../../agents/registry-v3.json")).unwrap();
    for id in IDS {
        let agent = &doc.agents[id];
        assert_eq!(agent.install.method, registry::InstallMethod::Manual);
        assert!(agent.install.command.is_none());
        assert!(agent.install.verify_cmd.is_none());
        assert!(agent.install.uninstall_cmd.is_none());
        assert!(agent.install.extension_id.is_none());
        assert!(
            agent
                .auth_guide
                .as_ref()
                .unwrap()
                .completion_check
                .is_none()
        );
        assert!(!agent.config_inject.skills);
        assert!(agent.config_inject.mcp.is_none());
        assert!(!agent.config_inject.memory);
        assert!(agent.config_inject.personalization.is_none());
        assert!(agent.config_inject.model_config.is_none());
        assert!(agent.memory_collector.is_none());
        assert!(agent.stars.is_none());
        assert!(!agent.popular);
        assert!(!deploy::can_install(agent));
        assert!(!deploy::can_configure(agent));
        assert!(!deploy::can_uninstall(agent));
        assert!(!deploy::check_auth(agent).unwrap());
        let details = deploy::inspect_installation(agent);
        assert_eq!(
            details.probe.status,
            deploy::InstallationStatus::Unsupported
        );
        assert!(details.instances.is_empty());
        assert!(!details.ambiguous);
        assert_eq!(agent.install.url, agent.homepage);
        assert!(agent.homepage.as_deref().unwrap().starts_with("https://"));
    }
}

#[test]
fn desktop_and_cli_guidance_remain_distinct_without_legacy_kimi_alias() {
    let doc = registry::parse_registry(include_str!("../../../agents/registry-v3.json")).unwrap();
    for id in ["cursor", "kimi-code-desktop"] {
        assert_eq!(doc.agents[id].ui_type, registry::UiType::Desktop);
    }
    for id in ["cursor-cli", "github-copilot-cli", "kimi-code"] {
        assert_eq!(doc.agents[id].ui_type, registry::UiType::Cli);
    }
    assert_eq!(
        doc.agents["kimi-code"].github.as_deref(),
        Some("MoonshotAI/kimi-code")
    );
    assert_eq!(doc.agents["kimi-code"].license.as_deref(), Some("MIT"));
    assert!(doc.agents["kimi-code-desktop"].license.is_none());
    assert!(!doc.agents.contains_key("kimi-cli"));
    assert_eq!(doc.agents["github-copilot-cli"].source, "closed");
}

#[test]
fn legacy_install_and_authentication_claims_are_withdrawn() {
    let registry =
        registry::parse_registry(include_str!("../../../agents/registry-v3.json")).unwrap();
    let claude = &registry.agents["claude-code"];
    assert_eq!(claude.install.method, registry::InstallMethod::Manual);
    assert!(claude.install.command.is_none());
    assert!(deploy::can_configure(claude));
    assert_eq!(claude.homepage, claude.install.url);
    assert_eq!(
        claude.install.verify_cmd.as_deref(),
        Some("claude --version")
    );
    let mut legacy = claude.clone();
    legacy.install.method = registry::InstallMethod::Npm;
    legacy.install.command = Some("npm install -g @anthropic-ai/claude-code".into());
    assert!(!deploy::can_install(&legacy));
    assert!(deploy::install_agent(&legacy).is_err());
    assert!(claude.install.uninstall_cmd.is_none());
    assert!(!aisync_core::deploy::can_install(claude));
    assert!(!aisync_core::deploy::can_uninstall(claude));
    for id in ["claude-code", "gemini-cli"] {
        assert!(
            registry.agents[id]
                .auth_guide
                .as_ref()
                .unwrap()
                .completion_check
                .is_none()
        );
        assert!(!aisync_core::deploy::check_auth(&registry.agents[id]).unwrap());
    }
    assert!(
        registry.agents["gemini-cli"]
            .description
            .contains("Antigravity")
    );
}
