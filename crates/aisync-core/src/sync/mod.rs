//! Configuration synchronization entry points.
//!
//! Unsupported adapters return an explicit error; public calls never panic.

/// A declarative plan only: no file reads, process probes, or side effects.
/// Apply still validates installation eligibility and the actual profile data.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlan {
    pub schema_version: u32,
    pub mode: &'static str,
    pub batch_status: &'static str,
    pub agents: Vec<SyncAgentPlan>,
    pub not_checked: Vec<&'static str>,
    pub notices: Vec<&'static str>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncAgentPlan {
    pub agent_id: String,
    pub status: &'static str,
    pub installation_eligibility: &'static str,
    pub operations: Vec<SyncOperationPlan>,
    pub warnings: Vec<&'static str>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOperationPlan {
    pub action: &'static str,
    pub source: Option<String>,
    pub target: String,
}

fn safe_template_name(template: &str) -> bool {
    !template.is_empty()
        && template != "."
        && template != ".."
        && template
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
}

/// Compute metadata for explicitly selected registry definitions. `~` paths are
/// symbolic, not resolved or accessed. Unsupported entries are reported so one
/// unsupported target cannot be mistaken for a successful batch validation.
pub fn plan_sync(
    registry: &crate::registry::RegistryDoc,
    agents: &[crate::registry::AgentDefinition],
) -> SyncPlan {
    let agents: Vec<_> = agents
        .iter()
        .map(|agent| {
            let personalization = crate::config::supports_personalization(agent);
            let mut operations = Vec::new();
            let mut warnings = Vec::new();
            if personalization {
                if let Some(profile) = &agent.config_inject.personalization {
                    operations.push(SyncOperationPlan {
                        action: "update_managed_instruction_block",
                        source: if safe_template_name(&profile.template) {
                        Some(format!("~/.agentdeck/profiles/default/personalization/{}/AGENTS.md", profile.template))
                    } else {
                        warnings.push("Template path is not a simple name; source path withheld and not validated");
                        None
                    },
                    target: profile.target.clone(),
                    });
                }
            } else if agent.config_inject.personalization.is_some() {
                warnings.push("Personalization destination is unsupported and would be skipped");
            }
            let mut invalid_memory = false;
            if agent.config_inject.memory {
                match crate::memory::validate_injection_target_in(&agent.id, registry) {
                    Ok(canonical) => {
                        operations.push(SyncOperationPlan {
                            action: "refresh_local_memory_export",
                            source: Some("~/.agentdeck/memory.db (opted-in entries and retained manual selections)".into()),
                            target: format!("~/.agentdeck/memory/{canonical}.md"),
                        });
                        warnings.push("A local memory export does not prove the external agent consumes it");
                    }
                    Err(_) => {
                        invalid_memory = true;
                        warnings.push("Memory export target is unsupported by the current adapter");
                    }
                }
            }
            SyncAgentPlan {
                agent_id: agent.id.clone(),
                status: if operations.is_empty() || invalid_memory {
                    "unsupported"
                } else {
                    "unverified"
                },
                installation_eligibility: if personalization {
                    "not_checked"
                } else if agent.config_inject.memory && !invalid_memory {
                    "not_required_for_memory_export"
                } else {
                    "not_applicable"
                },
                operations,
                warnings,
            }
        })
        .collect();
    let unsupported = agents.iter().any(|agent| agent.status == "unsupported");
    SyncPlan {
        schema_version: 1,
        mode: "dry_run",
        batch_status: if unsupported || agents.is_empty() {
            "unsupported"
        } else {
            "unverified"
        },
        agents,
        not_checked: vec![
            "installation_eligibility",
            "profile_and_target_contents",
            "path_safety_and_permissions",
            "actual_changes",
            "memory_entries",
            "backup_availability",
        ],
        notices: vec![
            "Metadata plan only, not a diff, an approval token, or permission to apply",
            "Exit 0 means this report was produced; it does not mean synchronization can succeed",
            "No agent, npm, Git, profile, vault, configuration, or memory database is executed, initialized, or changed",
            "An explicit sync --yes will perform its own validation, create an encrypted backup, initialize missing profile templates when needed, and perform supported writes",
            "Model settings, MCP servers, skills, and software installation are not part of sync",
        ],
    }
}

fn agent(agent_id: &str) -> crate::Result<crate::registry::AgentDefinition> {
    crate::registry::load_registry_runtime()?
        .agents
        .remove(agent_id)
        .ok_or_else(|| crate::Error::Other(format!("Unknown agent: {agent_id}")))
}

/// Skill distribution has no format/destination adapter yet. Do not report a
/// successful sync based solely on the registry's compatibility flag.
pub fn sync_skills(agent_id: &str) -> crate::Result<()> {
    let agent = agent(agent_id)?;
    Err(crate::Error::Other(format!(
        "Skill synchronization is not supported for {}",
        agent.name
    )))
}

/// Ensure enabled MCP servers are present and other managed servers removed.
pub fn sync_mcp(agent_id: &str, enabled_servers: &[String]) -> crate::Result<()> {
    crate::mcp::sync(agent_id, enabled_servers).map(|_| ())
}

/// Apply the user's default instruction template without changing model settings.
pub fn sync_personalization(agent_id: &str) -> crate::Result<()> {
    sync_personalization_with(
        agent(agent_id)?,
        crate::deploy::inspect_installation,
        crate::config::profile_source_dir,
        crate::config::inject_for_agent,
    )
}

fn sync_personalization_with(
    mut agent: crate::registry::AgentDefinition,
    mut inspect: impl FnMut(&crate::registry::AgentDefinition) -> crate::deploy::InstallationDetails,
    prepare: impl FnOnce() -> crate::Result<std::path::PathBuf>,
    apply: impl FnOnce(
        &crate::registry::AgentDefinition,
        &std::path::Path,
    ) -> crate::Result<crate::config::InjectReport>,
) -> crate::Result<()> {
    agent.config_inject.model_config = None;
    if !crate::config::supports_personalization(&agent) {
        return Err(crate::Error::InjectFailed(format!(
            "{} has no personalization adapter",
            agent.name
        )));
    }
    crate::deploy::validate_configuration_eligibility(&agent, &inspect(&agent))?;
    let source = prepare()?;
    // Profile preparation can take time. Repeat the shared eligibility check at
    // the write boundary, without changing MCP's separate offline semantics.
    crate::deploy::validate_configuration_eligibility(&agent, &inspect(&agent))?;
    let report = apply(&agent, &source)?;
    match report.personalization {
        Some(crate::config::InjectStatus::Ok) => Ok(()),
        Some(crate::config::InjectStatus::Skipped(reason)) => {
            Err(crate::Error::InjectFailed(reason))
        }
        None => Err(crate::Error::InjectFailed(format!(
            "{} has no personalization adapter",
            agent.name
        ))),
    }
}

/// Refresh AgentDeck's local memory-rule export. This does not claim that an
/// external agent automatically discovers or consumes the generated Markdown.
pub fn sync_memory_rules(agent_id: &str) -> crate::Result<()> {
    let agent = agent(agent_id)?;
    if !agent.config_inject.memory {
        return Err(crate::Error::Other(format!(
            "{} does not support memory rules",
            agent.name
        )));
    }
    crate::memory::inject_auto(agent_id).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_plan_uses_the_apply_memory_adapter_rules() {
        let mut registry =
            crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
                .unwrap();
        let mut alias = registry.agents["claude-code"].clone();
        alias.id = "claude".into();
        registry.agents.insert(alias.id.clone(), alias.clone());
        let plan = plan_sync(&registry, &[alias.clone()]);
        assert!(
            plan.agents[0]
                .operations
                .iter()
                .any(|op| op.target == "~/.agentdeck/memory/claude-code.md")
        );
        for id in ["custom.name", "custom_name"] {
            let mut invalid = alias.clone();
            invalid.id = id.into();
            registry.agents.insert(id.into(), invalid.clone());
            let plan = plan_sync(&registry, &[invalid]);
            assert_eq!(plan.batch_status, "unsupported");
            assert!(
                !plan.agents[0]
                    .operations
                    .iter()
                    .any(|op| op.action == "refresh_local_memory_export")
            );
        }
        registry
            .agents
            .get_mut("claude-code")
            .unwrap()
            .config_inject
            .memory = false;
        assert_eq!(plan_sync(&registry, &[alias]).batch_status, "unsupported");
    }

    #[test]
    fn metadata_plan_withholds_nonportable_or_escaping_template_sources() {
        let registry =
            crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
                .unwrap();
        for template in [
            "../private",
            "/absolute",
            "C:\\private",
            "nested/name",
            "nested\\name",
            ".",
            "..",
            "",
        ] {
            let mut agent = registry.agents["codex"].clone();
            agent
                .config_inject
                .personalization
                .as_mut()
                .unwrap()
                .template = template.into();
            let plan = plan_sync(&registry, &[agent]);
            assert!(plan.agents[0].operations[0].source.is_none(), "{template}");
            assert_eq!(plan.agents[0].status, "unverified");
            assert!(
                plan.agents[0]
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("withheld"))
            );
        }
    }

    #[test]
    fn metadata_plan_reports_memory_only_and_empty_selection_truthfully() {
        let mut registry =
            crate::registry::parse_registry(include_str!("../../../../agents/registry-v3.json"))
                .unwrap();
        let mut agent = registry.agents["codex"].clone();
        agent.config_inject.personalization = None;
        registry.agents.insert(agent.id.clone(), agent.clone());
        let plan = plan_sync(&registry, &[agent]);
        assert_eq!(
            plan.agents[0].installation_eligibility,
            "not_required_for_memory_export"
        );
        assert_eq!(plan.agents[0].operations.len(), 1);
        assert_eq!(plan_sync(&registry, &[]).batch_status, "unsupported");
        let plan = plan_sync(&registry, &[registry.agents["qwen-code"].clone()]);
        assert_eq!(plan.agents[0].installation_eligibility, "not_applicable");
    }

    #[test]
    fn unsupported_sync_calls_return_errors_instead_of_panicking() {
        assert!(sync_skills("not-an-agent").is_err());
        assert!(sync_personalization("not-an-agent").is_err());
        assert!(sync_memory_rules("not-an-agent").is_err());
        assert!(sync_skills("codex").is_err());
        assert!(sync_memory_rules("pi").is_err());
    }

    #[test]
    fn personalization_rejects_unknown_and_ambiguous_before_profile_or_write() {
        let agent = crate::registry::load_registry_runtime().unwrap().agents["codex"].clone();
        for (status, ambiguous) in [
            (crate::deploy::InstallationStatus::NotDetected, false),
            (crate::deploy::InstallationStatus::Error, false),
            (crate::deploy::InstallationStatus::Unsupported, false),
            (crate::deploy::InstallationStatus::Installed, true),
        ] {
            assert!(
                sync_personalization_with(
                    agent.clone(),
                    |_| crate::deploy::InstallationDetails {
                        probe: crate::deploy::InstallationProbe {
                            status,
                            message: None
                        },
                        instances: Vec::new(),
                        ambiguous,
                        message: None,
                    },
                    || panic!("Blocked synchronization must not initialize a profile"),
                    |_, _| panic!("Blocked synchronization must not write configuration"),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn unsupported_personalization_does_not_probe_or_initialize() {
        let agent = crate::registry::load_registry_runtime().unwrap().agents["pi"].clone();
        assert!(
            sync_personalization_with(
                agent,
                |_| panic!("Unsupported adapter must not execute a probe"),
                || panic!("Unsupported adapter must not initialize a profile"),
                |_, _| panic!("Unsupported adapter must not write configuration"),
            )
            .is_err()
        );
    }

    #[test]
    fn personalization_rechecks_after_preparation_before_writing() {
        let agent = crate::registry::load_registry_runtime().unwrap().agents["codex"].clone();
        let mut probes = 0;
        assert!(
            sync_personalization_with(
                agent,
                |_| {
                    probes += 1;
                    crate::deploy::InstallationDetails {
                        probe: crate::deploy::InstallationProbe {
                            status: crate::deploy::InstallationStatus::Installed,
                            message: None,
                        },
                        instances: Vec::new(),
                        ambiguous: probes > 1,
                        message: None,
                    }
                },
                || Ok(std::env::temp_dir().join("synthetic-profile-not-created")),
                |_, _| panic!("Changed installation must not write configuration"),
            )
            .is_err()
        );
        assert_eq!(probes, 2);
    }
}
