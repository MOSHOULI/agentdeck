//! Configuration synchronization entry points.
//!
//! Unsupported adapters return an explicit error; public calls never panic.

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
