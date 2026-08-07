/** Permission policy controlling ACP `session/request_permission` calls. */
export type PermissionPolicy = "ask" | "allow" | "reject";

/** Where an agent's effective permission policy came from. */
export type PermissionPolicySource = "agent" | "global_default" | "built_in";

/** Global agent configuration defaults applied to all agents. */
export type GlobalAgentConfig = {
  /** Global env vars injected into all agents unconditionally. */
  env_vars: Record<string, string>;
  /** Global fallback provider. */
  provider: string | null;
  /** Global fallback model identifier. */
  model: string | null;
  /** Preferred ACP runtime for agents without a persona-specific runtime. */
  preferred_runtime: string | null;
  /** Fleet-wide permission policy fallback. */
  permission_policy: PermissionPolicy | null;
};

/** Result returned by `set_global_agent_config`. */
export type GlobalAgentConfigSaveResult = {
  /** The persisted global config after strip-on-write. */
  config: GlobalAgentConfig;
  /** Number of local agents successfully stopped and restarted. */
  restarted_count: number;
  /** Number of agents whose stop succeeded but respawn failed. */
  failed_restart_count: number;
};
