import * as React from "react";

import type {
  ManagedAgent,
  PermissionPolicy,
  PermissionPolicySource,
} from "@/shared/api/types";

const SOURCE_LABELS: Record<PermissionPolicySource, string> = {
  agent: "agent override",
  global_default: "global default",
  built_in: "built-in",
};

function savedOverride(agent: ManagedAgent): PermissionPolicy | null {
  return agent.permissionPolicySource === "agent"
    ? agent.permissionPolicy
    : null;
}

export function useEditAgentPermissionPolicy(
  agent: ManagedAgent,
  open: boolean,
) {
  const saved = savedOverride(agent);
  const [value, setValue] = React.useState<PermissionPolicy | null>(saved);

  // Match the dialog's reset boundary so background polling never wipes edits.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset only on open or agent switch
  React.useEffect(() => {
    if (open) setValue(savedOverride(agent));
  }, [open, agent.pubkey]);

  return {
    value,
    setValue,
    update: value !== saved ? value : undefined,
  };
}

export function EditAgentPermissionPolicy({
  agent,
  disabled,
  value,
  onChange,
}: {
  agent: ManagedAgent;
  disabled: boolean;
  value: PermissionPolicy | null;
  onChange: (value: PermissionPolicy | null) => void;
}) {
  const sourceLabel = SOURCE_LABELS[agent.permissionPolicySource];
  const isRemoteDeployed =
    agent.backend.type === "provider" && agent.backendAgentId !== null;

  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-1.5">
        <label
          className="text-sm font-medium text-foreground"
          htmlFor="edit-agent-permission-policy"
        >
          Permission policy
        </label>
        <span className="text-xs text-muted-foreground">
          ({agent.permissionPolicy} · from {sourceLabel})
        </span>
      </div>
      {isRemoteDeployed ? (
        <p className="text-xs text-muted-foreground">
          Read-only while deployed. To change, shut down and redeploy the agent.
        </p>
      ) : (
        <select
          className="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm shadow-sm focus:outline-none focus:ring-1 focus:ring-ring disabled:opacity-50"
          disabled={disabled}
          id="edit-agent-permission-policy"
          value={value ?? ""}
          onChange={(event) => {
            const next = event.target.value;
            onChange(next === "" ? null : (next as PermissionPolicy));
          }}
        >
          <option value="">
            Inherit ({agent.permissionPolicy} · from {sourceLabel})
          </option>
          <option value="ask">Ask — show Allow/Deny card</option>
          <option value="allow">Allow — auto-approve (explicit opt-in)</option>
          <option value="reject">Reject — auto-deny</option>
        </select>
      )}
    </div>
  );
}
