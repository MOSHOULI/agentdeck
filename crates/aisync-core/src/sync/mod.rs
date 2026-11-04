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
    let mut agent = agent(agent_id)?;
    agent.config_inject.model_config = None;
    let source = crate::config::profile_source_dir()?;
    let report = crate::config::inject_for_agent(&agent, &source)?;
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
}
