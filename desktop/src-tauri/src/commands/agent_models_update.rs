use crate::managed_agents::{permission_policy::PermissionPolicy, BackendKind, ManagedAgentRecord};

pub(super) fn apply_model_provider_prompt_update(
    record: &mut ManagedAgentRecord,
    model: Option<Option<String>>,
    provider: Option<Option<String>>,
    system_prompt: Option<Option<String>>,
) {
    if record.persona_id.is_some() {
        return;
    }
    if let Some(model_update) = model {
        record.model = model_update;
    }
    if let Some(provider_update) = provider {
        record.provider = provider_update;
    }
    if let Some(prompt_update) = system_prompt {
        record.system_prompt = prompt_update;
    }
}

pub(super) fn apply_permission_policy_update(
    record: &mut ManagedAgentRecord,
    update: Option<Option<PermissionPolicy>>,
) -> Result<(), String> {
    let Some(policy) = update else {
        return Ok(());
    };
    if matches!(&record.backend, BackendKind::Provider { .. }) && record.backend_agent_id.is_some()
    {
        return Err("permission_policy is read-only while the agent is deployed remotely; shut down and redeploy to change it".to_string());
    }
    record.permission_policy = policy;
    Ok(())
}
