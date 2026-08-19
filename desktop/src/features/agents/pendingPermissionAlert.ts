import { normalizePubkey } from "@/shared/lib/pubkey";
import type { ObserverEvent, TranscriptItem } from "./ui/agentSessionTypes";
import { asRecord, asString } from "./ui/agentSessionUtils";

/** An admitted owner-actionable permission request. */
export type ActionablePermissionAlert = {
  requestNonce: string;
  channelId: string | null;
  title: string;
};

/** React-local replay and duplicate suppression state. */
export type PermissionAlertStoreState = {
  seededAgentPubkeys: Set<string>;
  seenNonces: Set<string>;
};

/** The event-bearing subset of an observer-store notification. */
export type PermissionAlertStoreUpdate = {
  agentPubkey: string;
  events: readonly ObserverEvent[];
};

const PERMISSION_METHOD = "session/request_permission";
const ALERT_BODY = "Open the process log to allow or deny this request.";

function payloadMethod(payload: unknown): string | null {
  return asString(asRecord(payload).method);
}

function payloadTitle(payload: unknown): string {
  const params = asRecord(asRecord(payload).params);
  return (
    asString(params.title) ??
    asString(params.message) ??
    asString(params.reason) ??
    "Permission requested"
  );
}

function nonEmptyNonce(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

/** Create empty React-local permission-alert state. */
export function createPermissionAlertStoreState(): PermissionAlertStoreState {
  return { seededAgentPubkeys: new Set(), seenNonces: new Set() };
}

/** Extract an admitted actionable permission request from an observer frame. */
export function extractActionablePermission(
  event: ObserverEvent,
): ActionablePermissionAlert | null {
  if (
    event.kind !== "acp_read" ||
    payloadMethod(event.payload) !== PERMISSION_METHOD ||
    event.authorization?.actionable !== true
  ) {
    return null;
  }
  const requestNonce = nonEmptyNonce(event.authorization.requestNonce);
  if (!requestNonce) {
    return null;
  }
  return {
    requestNonce,
    channelId: event.channelId,
    title: payloadTitle(event.payload),
  };
}

/** Extract a terminal permission nonce from an observer frame. */
export function extractResolvedPermissionNonce(
  event: ObserverEvent,
): string | null {
  const requestNonce = nonEmptyNonce(event.authorization?.requestNonce);
  if (!requestNonce) {
    return null;
  }
  if (event.kind === "acp_write") {
    return requestNonce;
  }
  if (
    event.kind === "acp_read" &&
    payloadMethod(event.payload) === PERMISSION_METHOD &&
    event.authorization?.actionable === false
  ) {
    return requestNonce;
  }
  return null;
}

/** Return true only for the already-open matching agent and channel pane. */
export function shouldSuppressPermissionAlert(input: {
  agentPubkey: string;
  channelId: string | null;
  openAgentSession: string | null | undefined;
  openAgentSessionChannel: string | null | undefined;
  currentChannelId: string | null | undefined;
}): boolean {
  if (
    !input.openAgentSession ||
    normalizePubkey(input.openAgentSession) !==
      normalizePubkey(input.agentPubkey)
  ) {
    return false;
  }
  return (
    input.channelId === null ||
    input.channelId === input.currentChannelId ||
    input.channelId === input.openAgentSessionChannel
  );
}

/** Build pinned toast and native-notification copy. */
export function permissionAlertCopy(input: {
  agentName: string;
  channelName: string | null | undefined;
}): { title: string; body: string } {
  const agentName = input.agentName.trim() || "Agent";
  const channelName = (input.channelName?.trim() ?? "").replace(/^#+/, "");
  return {
    title: channelName
      ? `${agentName} needs permission in #${channelName}`
      : `${agentName} needs permission`,
    body: ALERT_BODY,
  };
}

/** Collect permission nonces already represented in a transcript. */
export function collectSeenPermissionNonces(
  items: readonly TranscriptItem[],
): string[] {
  const nonces: string[] = [];
  for (const item of items) {
    if (!("requestNonce" in item)) {
      continue;
    }
    const requestNonce = nonEmptyNonce(item.requestNonce);
    if (requestNonce) {
      nonces.push(requestNonce);
    }
  }
  return nonces;
}

/** Fold one admitted live batch into alerts, seen nonces, and dismissals. */
export function applyPermissionAlertUpdate(input: {
  events: readonly ObserverEvent[];
  seenNonces: ReadonlySet<string>;
}): {
  nextSeenNonces: Set<string>;
  alerts: ActionablePermissionAlert[];
  dismissNonces: string[];
} {
  const nextSeenNonces = new Set(input.seenNonces);
  const alerts: ActionablePermissionAlert[] = [];
  const dismissNonces: string[] = [];
  const dismissSet = new Set<string>();

  for (const event of input.events) {
    const resolved = extractResolvedPermissionNonce(event);
    if (resolved) {
      nextSeenNonces.add(resolved);
      if (!dismissSet.has(resolved)) {
        dismissSet.add(resolved);
        dismissNonces.push(resolved);
      }
    }
    const alert = extractActionablePermission(event);
    if (!alert || nextSeenNonces.has(alert.requestNonce)) {
      continue;
    }
    nextSeenNonces.add(alert.requestNonce);
    alerts.push(alert);
  }

  return {
    nextSeenNonces,
    alerts: alerts.filter((alert) => !dismissSet.has(alert.requestNonce)),
    dismissNonces,
  };
}

/** Gate initial replay, seed each owned agent once, and apply one live update. */
export function applyPermissionAlertStoreNotification(input: {
  state: PermissionAlertStoreState;
  initialReplayComplete: boolean;
  agents: readonly {
    pubkey: string;
    transcript: readonly TranscriptItem[];
  }[];
  update?: PermissionAlertStoreUpdate;
}): {
  nextState: PermissionAlertStoreState;
  alerts: ActionablePermissionAlert[];
  dismissNonces: string[];
} {
  const nextState: PermissionAlertStoreState = {
    seededAgentPubkeys: new Set(input.state.seededAgentPubkeys),
    seenNonces: new Set(input.state.seenNonces),
  };
  if (!input.initialReplayComplete) {
    return { nextState, alerts: [], dismissNonces: [] };
  }

  const ownedAgentPubkeys = new Set<string>();
  for (const agent of input.agents) {
    const pubkey = normalizePubkey(agent.pubkey);
    ownedAgentPubkeys.add(pubkey);
    if (nextState.seededAgentPubkeys.has(pubkey)) {
      continue;
    }
    for (const nonce of collectSeenPermissionNonces(agent.transcript)) {
      nextState.seenNonces.add(nonce);
    }
    nextState.seededAgentPubkeys.add(pubkey);
  }

  if (
    !input.update ||
    !ownedAgentPubkeys.has(normalizePubkey(input.update.agentPubkey))
  ) {
    return { nextState, alerts: [], dismissNonces: [] };
  }

  const applied = applyPermissionAlertUpdate({
    events: input.update.events,
    seenNonces: nextState.seenNonces,
  });
  nextState.seenNonces = applied.nextSeenNonces;
  return {
    nextState,
    alerts: applied.alerts,
    dismissNonces: applied.dismissNonces,
  };
}

/** Choose focused toast, background OS delivery, or no surface. */
export function selectPermissionAlertSurface(input: {
  focused: boolean;
  desktopEnabled: boolean;
}): "toast" | "os" | null {
  if (input.focused) {
    return "toast";
  }
  return input.desktopEnabled ? "os" : null;
}

/** Process the current snapshot before subscribing to later store updates. */
export function startPermissionAlertStoreSubscription(input: {
  handleUpdate: (update?: PermissionAlertStoreUpdate) => void;
  subscribe: (
    listener: (update?: PermissionAlertStoreUpdate) => void,
  ) => () => void;
}): () => void {
  input.handleUpdate();
  return input.subscribe(input.handleUpdate);
}
