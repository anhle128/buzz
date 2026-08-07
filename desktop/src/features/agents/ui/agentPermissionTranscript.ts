import { asRecord, asString } from "./agentSessionUtils";

export function describePermissionRequest(payload: Record<string, unknown>) {
  const params = asRecord(payload.params);
  const title =
    asString(params.title) ??
    asString(params.message) ??
    asString(params.reason) ??
    "Permission requested";
  const toolCallId =
    asString(params.toolCallId) ?? asString(params.tool_call_id);

  const optionNames = new Map<string, string>();
  const options: Array<{
    optionId: string;
    kind: string;
    label?: string;
  }> = [];
  const optionDisplayNames: string[] = [];
  if (Array.isArray(params.options)) {
    for (const option of params.options) {
      const record = asRecord(option);
      const optionId = asString(record.optionId);
      const kind = asString(record.kind);
      const label = asString(record.label) ?? asString(record.name);
      const displayName =
        asString(record.name) ??
        asString(record.kind) ??
        asString(record.optionId);
      if (displayName) optionDisplayNames.push(displayName);
      if (optionId && kind) {
        optionNames.set(optionId, kind);
        options.push({ optionId, kind, ...(label ? { label } : {}) });
      }
    }
  }

  const detail: string[] = [];
  if (title !== "Permission requested") detail.push(title);
  if (toolCallId) detail.push(`Tool call: ${toolCallId}`);
  if (optionDisplayNames.length > 0) {
    detail.push(`Options: ${optionDisplayNames.join(", ")}`);
  }

  return {
    title,
    text: detail.join("\n"),
    optionNames,
    options,
    descriptor: {
      renderClass: "permission" as const,
      label: "Permission requested",
      preview: title,
      action: { verb: "Requested", object: title },
      tone: "admin" as const,
      operation: "session/request_permission",
      object: title,
      source: "acp" as const,
      groupKey: "permission:request",
    },
  };
}

/** Format a human-readable ACP permission outcome. */
export function describePermissionOutcome(
  outcome: string,
  optionId: string | null,
  optionNames: Map<string, string>,
): string {
  if (outcome === "cancelled") return "Cancelled";
  if (outcome === "timed_out") return "Timed out";
  if (outcome === "uncertain") {
    return "Approval outcome unknown; agent process stopped before it could continue.";
  }
  if (outcome === "selected" && optionId) {
    const kind = optionNames.get(optionId) ?? optionId;
    return `${kind.startsWith("reject") ? "Denied" : "Approved"} (${kind})`;
  }
  return outcome;
}
