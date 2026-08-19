# Agent Permission Alerts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When an owned agent emits a new actionable `session/request_permission`, Buzz Desktop alerts the owner outside the already-open matching transcript and opens that existing transcript on click.

**Architecture:** Keep the permission decision UI in `LifecycleActivity` and add an AppShell-level observer-store consumer for alert delivery.
Gate delivery behind the observer subscription's initial EOSE so the existing 300-second replay window seeds nonces instead of notifying them.
Use the existing AppShell notification settings instance, route focused delivery to a sonner toast, route background delivery to the native notification helper, and route both click paths through `useOpenAgentActivity`.

**Tech Stack:** TypeScript, React 19, TanStack Router, sonner, Tauri notification helpers, Node `node:test`, Testing Library only where an existing hook test requires it, Biome, and the repository `just` recipes.

**Spec:** [2026-08-19-agent-permission-alerts-design.md](../specs/2026-08-19-agent-permission-alerts-design.md)

**Product contract:** [VISION.md](../../../VISION.md) gives Stream a zero-notification default but already treats approval work as actionable.
[VISION_ACTIVITY.md](../../../VISION_ACTIVITY.md) defines Permission as the agent activity control gate.
This change fetches the owner to the existing gate without moving grant or deny controls into a notification.
The main-window-only mount is an intentional boundary choice: it preserves owner approval coverage while avoiding duplicate delivery from the huddle companion webview.

## Global Constraints

- Desktop only.
- Treat the user-supplied design document as the approved brainstorm input even though its header still says `Draft — awaiting user review`.
- Do not change `buzz-acp`, ACP admission, nonce binding, `permission_decision`, or the 300-second fail-closed timeout.
- Do not change `LifecycleActivity` or `PermissionDecisionButtons`.
- Do not add grant or deny controls to the toast or OS notification.
- Do not add an event kind, Home `needs_action` row, relay persistence, mobile behavior, or a notification-settings slot.
- Do not withdraw OS notifications after a decision.
- Do not apply channel mute, `notifyWhileViewing`, or huddle silent-channel rules to this owner approval.
- Respect `desktopEnabled` only when selecting the OS surface.
- A focused AppShell still shows the toast when `desktopEnabled` is false.
- `slotAlertsEnabled.needs_action` controls sound only for this feature.
- Play the `needs_action` sound for a toast immediately and for an OS notification only after `sendDesktopNotification` returns `true`.
- Keep seen nonces in React-local state so the `AppReady` community-key remount clears them.
- Do not add a module-level nonce cache or a `resetCommunityState` entry.
- Do not mount the alert consumer in the dedicated huddle companion AppShell.
- The main AppShell remains responsible for alerts while a huddle companion is open, matching the existing main-window notification-action listener boundary and preventing duplicate alerts from two webviews.
- Do not add a Playwright spec.
- Preserve the existing observer permission E2E grant-on-card flow.
- `AppShell.tsx` must remain at or below the repository's 1000-line ratchet.
- Activate Hermit before shell commands with `. ./bin/activate-hermit && ...`.
- Sign implementation commits with `git commit -s`.
- Every exported TypeScript API added by this plan must have a doc comment.
- Keep every task in red-green-refactor order.
- Do not write production code for a task until its named failing test has failed for the expected reason.

## GitNexus Gates

GitNexus MCP tools and `.gitnexus/run.cjs` are absent in this planning session.
The implementation session must still follow the repository GitNexus policy.

- Before changing an existing function, class, method, or exported type, run `impact({ target: "<symbol>", direction: "upstream" })`.
- Report direct callers, affected execution flows, and the returned risk before editing.
- Stop before editing if risk is HIGH or CRITICAL.
- If the implementation session has no GitNexus index, run `npx gitnexus analyze`, then resume in a session where the GitNexus MCP tools are loaded.
- `rg` is useful corroborating evidence but is not a substitute for the required GitNexus impact call.
- Before each commit, stage the task files and run `detect_changes({ scope: "staged" })`.
- Before final handoff, run `detect_changes({ scope: "compare", base_ref: "main" })`.
- There is no GitNexus CLI `detect` command, so do not run `node .gitnexus/run.cjs detect`.
- If `detect_changes` is unavailable, stop before the commit, leave the changes uncommitted, and do not begin the next task until the repository gate is available.

## Resolved Implementation Decisions

- Toast `id` is `authorization.requestNonce`.
- Toast `duration` is `Number.POSITIVE_INFINITY` under the provisional default in Open Question 1.
- Alert title is `{agentName} needs permission` or `{agentName} needs permission in #{channel}` when a trimmed channel name is known.
- Strip every leading `#` from the stored channel name before adding one display prefix.
- Alert body is exactly `Open the process log to allow or deny this request.`.
- Do not include option names, tool-call ids, or the ACP request title in alert copy.
- `extractActionablePermission` still returns the ACP title because it is part of the approved helper contract, but delivery copy ignores it.
- Resolve the agent label with `resolveUserLabel({ pubkey: agentPubkey, profiles, preferResolvedSelfLabel: true })`.
- An empty resolved label falls back to `Agent` inside `permissionAlertCopy`.
- Use the existing AppShell `notificationSettings.settings` object instead of calling `useNotificationSettings` a second time.
- `parseNotificationTarget` is exported.
- A native target is valid when `channelId`, `eventId`, or `agentPubkey` is a non-empty string.
- `pubkey` remains the message author field used by `toSearchHit` and must not be reused for agent identity.
- `agentPubkey` wins over search-hit, Home, and channel routing even when `eventId` is present.
- Native Linux and macOS already forward the target as opaque JSON, so no Rust change is needed.
- `resolveDesktopNotificationAction` and `executeDesktopNotificationAction` live in `desktop/src/app/AppShell.helpers.ts` beside `toSearchHit`.
- `useAppShellDesktopNotifications` calls `useOpenAgentActivity()` itself and passes the real dependencies to `executeDesktopNotificationAction`.
- Export `useObserverIngestionAgents()` from `useAgentObserverIngestion.ts` and use it from both observer ingestion and permission alerts.
- The permission-alert consumer rejects store updates whose normalized agent pubkey is not in that owner-global ingestion list.
- Initial observer history is distinguished from later events by the initial subscription EOSE, not by agent timestamps, because remote agent clocks are not a safe replay boundary.
- `subscribeToAgentObserverFrames` forwards its optional readiness callback to `relayClient.subscribeLive`.
- `observerRelayStore` enqueues the initial-replay-complete marker on `eventProcessingQueue` when readiness is `eose`.
- The queue marker runs after every historical EVENT flushed immediately before EOSE and before later live events queued after EOSE.
- Readiness values `timeout` and `closed` do not open alert delivery.
- The alert controller seeds transcript nonces only after the initial-replay marker is complete.
- Existing reconnect replay remains eligible to alert only for genuinely new missed events; already-ingested frames remain suppressed by observer-store dedup and the seen-nonce set.
- Process each live `update.events` batch in order.
- If one batch both creates and resolves the same nonce, do not post an alert for that nonce under Open Question 2's provisional default.
- Suppress only when the open session pane matches the requesting agent and the design's channel rule.
- Await `revealDesktopAppWindow()` before opening the transcript from either surface.
- Late OS clicks still call `openAgentActivity`; the card may already show a terminal outcome.
- Inaccessible-channel clicks keep the existing warning behavior from `useOpenAgentActivity`.

## File Map

| File | Responsibility |
|------|----------------|
| Create `desktop/src/features/agents/pendingPermissionAlert.ts` | Pure extraction, suppression, copy, seen-state, surface, and store-update controller |
| Create `desktop/src/features/agents/pendingPermissionAlert.test.mjs` | Unit coverage for every permission-alert decision and replay-seed transition |
| Modify `desktop/src/shared/api/observerRelay.ts` | Forward initial live-subscription readiness |
| Modify `desktop/src/shared/api/observerRelay.test.mjs` | Pin readiness forwarding without changing the 300-second replay filter |
| Modify `desktop/src/features/agents/observerRelayStore.ts` | Publish an initial-replay-complete barrier after EOSE history is processed |
| Create `desktop/src/features/agents/observerInitialReplay.test.mjs` | Pin replay readiness initialization, notification, and reset |
| Modify `desktop/src/features/notifications/lib/desktop.ts` | Add optional `agentPubkey` and relax notification-target parsing |
| Modify `desktop/src/features/notifications/lib/desktop.test.mjs` | Pin agent-only, author-only, and event-only target parsing |
| Modify `desktop/src/app/AppShell.helpers.ts` | Resolve and execute desktop-notification click destinations |
| Modify `desktop/src/app/AppShell.helpers.test.mjs` | Pin agent routing precedence and real executor call order |
| Modify `desktop/src/app/useAppShellDesktopNotifications.ts` | Use the tested click executor and `useOpenAgentActivity` |
| Modify `desktop/src/app/routes/channels.$channelId.tsx` | Preserve `agentSessionChannel` in validated channel-route search |
| Create `desktop/src/app/routes/channels.$channelId.test.mjs` | Pin the route-search contract used by suppression |
| Modify `desktop/src/features/agents/useAgentObserverIngestion.ts` | Export the canonical owner-global ingestion-agent hook |
| Modify `desktop/src/features/agents/useAgentObserverIngestion.test.mjs` | Re-run existing ownership-combination characterization tests unchanged |
| Create `desktop/src/features/agents/pendingPermissionAlertDelivery.ts` | Execute one tested toast, OS, sound, bounce, and click effect plan |
| Create `desktop/src/features/agents/pendingPermissionAlertDelivery.test.mjs` | Pin surface effects and reveal-before-open ordering |
| Create `desktop/src/features/agents/usePendingPermissionAlerts.ts` | React adapter for store subscription and toast, OS, sound, and dismissal effects |
| Modify `desktop/src/app/AppShell.tsx` | Mount the alert hook with the existing notification settings and main-window gate |

Do not create or modify any other product file.

## Open Questions

1. **Toast duration.**
   The approved design does not specify duration.
   **Provisional default:** use `Number.POSITIVE_INFINITY`, let the user dismiss manually, and dismiss programmatically on terminal resolution.

2. **Request and resolution in one observer batch.**
   The approved design does not define a same-batch outcome.
   **Provisional default:** do not post an alert for a nonce that is terminal in the same batch.

3. **Unknown agent name.**
   The approved design does not define the missing-profile label.
   **Provisional default:** use `resolveUserLabel`, which falls back to a truncated pubkey, and use `Agent` only if the resulting string is empty.

4. **Initial subscription without EOSE.**
   The relay client can report `timeout` or `closed` before EOSE, which does not provide a trustworthy replay boundary.
   **Provisional default:** keep permission-alert delivery closed until an actual EOSE callback establishes the boundary, while transcript ingestion continues normally.
   A late EOSE after `timeout` may establish the boundary, but a pre-EOSE `closed` that clears the readiness callback leaves alerts disabled for that observer-store generation.

---

### Task 1: Build the pure permission-alert controller

**Files:**

- Create: `desktop/src/features/agents/pendingPermissionAlert.test.mjs`
- Create: `desktop/src/features/agents/pendingPermissionAlert.ts`

**Interfaces:**

```ts
export type ActionablePermissionAlert = {
  requestNonce: string;
  channelId: string | null;
  title: string;
};

export type PermissionAlertStoreState = {
  seededAgentPubkeys: Set<string>;
  seenNonces: Set<string>;
};

export type PermissionAlertStoreUpdate = {
  agentPubkey: string;
  events: readonly ObserverEvent[];
};

export function createPermissionAlertStoreState(): PermissionAlertStoreState;
export function extractActionablePermission(event: ObserverEvent): ActionablePermissionAlert | null;
export function extractResolvedPermissionNonce(event: ObserverEvent): string | null;
export function shouldSuppressPermissionAlert(input: {
  agentPubkey: string;
  channelId: string | null;
  openAgentSession: string | null | undefined;
  openAgentSessionChannel: string | null | undefined;
  currentChannelId: string | null | undefined;
}): boolean;
export function permissionAlertCopy(input: {
  agentName: string;
  channelName: string | null | undefined;
}): { title: string; body: string };
export function collectSeenPermissionNonces(items: readonly TranscriptItem[]): string[];
export function applyPermissionAlertUpdate(input: {
  events: readonly ObserverEvent[];
  seenNonces: ReadonlySet<string>;
}): {
  nextSeenNonces: Set<string>;
  alerts: ActionablePermissionAlert[];
  dismissNonces: string[];
};
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
};
export function selectPermissionAlertSurface(input: {
  focused: boolean;
  desktopEnabled: boolean;
}): "toast" | "os" | null;
export function startPermissionAlertStoreSubscription(input: {
  handleUpdate: (update?: PermissionAlertStoreUpdate) => void;
  subscribe: (
    listener: (update?: PermissionAlertStoreUpdate) => void,
  ) => () => void;
}): () => void;
```

- [ ] **Step 1: Run the required impact checks for consumed symbols**

Run GitNexus upstream impact for `normalizePubkey`, `ObserverEvent`, `TranscriptItem`, `asRecord`, and `asString`.
These symbols are consumed but not modified, so record their interfaces and continue unless the proposed use contradicts a HIGH or CRITICAL warning.

- [ ] **Step 2: Write the failing tests**

Create `desktop/src/features/agents/pendingPermissionAlert.test.mjs`.
Use literal observer frames and literal expected values.
The file must contain separate tests with these exact names and assertions:

```js
import assert from "node:assert/strict";
import test from "node:test";

import {
  applyPermissionAlertStoreNotification,
  applyPermissionAlertUpdate,
  collectSeenPermissionNonces,
  createPermissionAlertStoreState,
  extractActionablePermission,
  extractResolvedPermissionNonce,
  permissionAlertCopy,
  selectPermissionAlertSurface,
  shouldSuppressPermissionAlert,
  startPermissionAlertStoreSubscription,
} from "./pendingPermissionAlert.ts";

const AGENT = "a".repeat(64);
const OTHER_AGENT = "b".repeat(64);
const CHANNEL = "11111111-1111-4111-8111-111111111111";
const OTHER_CHANNEL = "22222222-2222-4222-8222-222222222222";

function permissionRead({
  nonce = "nonce-1",
  actionable = true,
  channelId = CHANNEL,
  kind = "acp_read",
  method = "session/request_permission",
  seq = 1,
  title = "Confirm push",
} = {}) {
  return {
    seq,
    timestamp: "2026-08-19T10:00:00.000Z",
    kind,
    agentIndex: 0,
    channelId,
    sessionId: "session-1",
    turnId: "turn-1",
    payload: {
      jsonrpc: "2.0",
      id: "request-1",
      method,
      params: { title },
    },
    authorization: { requestNonce: nonce, actionable },
  };
}

function permissionWrite({ nonce = "nonce-1", seq = 2 } = {}) {
  return {
    ...permissionRead({ nonce, seq, kind: "acp_write" }),
    payload: {
      jsonrpc: "2.0",
      id: "request-1",
      result: { outcome: { outcome: "selected", optionId: "allow_once" } },
    },
    authorization: {
      requestNonce: nonce,
      actionable: false,
      reason: "applied",
    },
  };
}

function permissionItem(nonce) {
  return {
    id: `permission:channel:nonce:${nonce}`,
    type: "lifecycle",
    renderClass: "permission",
    title: "Permission requested",
    text: "Confirm push",
    timestamp: "2026-08-19T10:00:00.000Z",
    requestNonce: nonce,
    actionable: true,
  };
}

test("extractActionablePermission returns the admitted nonce channel and title", () => {
  assert.deepEqual(extractActionablePermission(permissionRead()), {
    requestNonce: "nonce-1",
    channelId: CHANNEL,
    title: "Confirm push",
  });
});

test("extractActionablePermission rejects wrong method wrong kind non-actionable and missing envelope", () => {
  assert.equal(extractActionablePermission(permissionRead({ method: "session/prompt" })), null);
  assert.equal(extractActionablePermission(permissionRead({ kind: "acp_write" })), null);
  assert.equal(extractActionablePermission(permissionRead({ actionable: false })), null);
  const missingEnvelope = permissionRead();
  delete missingEnvelope.authorization;
  assert.equal(extractActionablePermission(missingEnvelope), null);
});

test("extractResolvedPermissionNonce reads terminal acp_write and non-actionable follow-up", () => {
  assert.equal(extractResolvedPermissionNonce(permissionWrite()), "nonce-1");
  assert.equal(extractResolvedPermissionNonce(permissionRead({ actionable: false })), "nonce-1");
  assert.equal(extractResolvedPermissionNonce(permissionRead()), null);
});

test("shouldSuppressPermissionAlert requires the same normalized agent and matching channel scope", () => {
  assert.equal(shouldSuppressPermissionAlert({
    agentPubkey: AGENT,
    channelId: CHANNEL,
    openAgentSession: AGENT.toUpperCase(),
    openAgentSessionChannel: null,
    currentChannelId: CHANNEL,
  }), true);
  assert.equal(shouldSuppressPermissionAlert({
    agentPubkey: AGENT,
    channelId: null,
    openAgentSession: AGENT,
    openAgentSessionChannel: OTHER_CHANNEL,
    currentChannelId: OTHER_CHANNEL,
  }), true);
  assert.equal(shouldSuppressPermissionAlert({
    agentPubkey: AGENT,
    channelId: CHANNEL,
    openAgentSession: null,
    openAgentSessionChannel: null,
    currentChannelId: CHANNEL,
  }), false);
  assert.equal(shouldSuppressPermissionAlert({
    agentPubkey: AGENT,
    channelId: CHANNEL,
    openAgentSession: OTHER_AGENT,
    openAgentSessionChannel: CHANNEL,
    currentChannelId: CHANNEL,
  }), false);
  assert.equal(shouldSuppressPermissionAlert({
    agentPubkey: AGENT,
    channelId: CHANNEL,
    openAgentSession: AGENT,
    openAgentSessionChannel: OTHER_CHANNEL,
    currentChannelId: OTHER_CHANNEL,
  }), false);
});

test("permissionAlertCopy uses pinned copy and strips duplicate channel hashes", () => {
  assert.deepEqual(permissionAlertCopy({ agentName: "Ada", channelName: "#eng" }), {
    title: "Ada needs permission in #eng",
    body: "Open the process log to allow or deny this request.",
  });
  assert.deepEqual(permissionAlertCopy({ agentName: "  ", channelName: null }), {
    title: "Agent needs permission",
    body: "Open the process log to allow or deny this request.",
  });
});

test("applyPermissionAlertUpdate alerts concurrent unseen nonces once and suppresses same-batch resolution", () => {
  const concurrent = applyPermissionAlertUpdate({
    events: [permissionRead({ nonce: "new-1", seq: 1 }), permissionRead({ nonce: "new-2", seq: 2 })],
    seenNonces: new Set(["old-1"]),
  });
  assert.deepEqual(concurrent.alerts.map((alert) => alert.requestNonce), ["new-1", "new-2"]);
  assert.equal(concurrent.nextSeenNonces.has("old-1"), true);
  const replay = applyPermissionAlertUpdate({
    events: [permissionRead({ nonce: "old-1" })],
    seenNonces: new Set(["old-1"]),
  });
  assert.deepEqual(replay.alerts, []);
  const terminal = applyPermissionAlertUpdate({
    events: [
      permissionRead({ nonce: "flash-1", seq: 3 }),
      permissionWrite({ nonce: "flash-1", seq: 4 }),
    ],
    seenNonces: new Set(),
  });
  assert.deepEqual(terminal.alerts, []);
  assert.deepEqual(terminal.dismissNonces, ["flash-1"]);
});

test("store controller seeds initial replay before admitting later owned-agent updates", () => {
  const state = createPermissionAlertStoreState();
  const replayUpdate = applyPermissionAlertStoreNotification({
    state,
    initialReplayComplete: false,
    agents: [{ pubkey: AGENT, transcript: [permissionItem("replayed")] }],
    update: { agentPubkey: AGENT, events: [permissionRead({ nonce: "replayed" })] },
  });
  assert.deepEqual(replayUpdate.alerts, []);
  assert.equal(replayUpdate.nextState.seenNonces.has("replayed"), false);

  const seeded = applyPermissionAlertStoreNotification({
    state: replayUpdate.nextState,
    initialReplayComplete: true,
    agents: [{ pubkey: AGENT, transcript: [permissionItem("replayed")] }],
  });
  assert.equal(seeded.nextState.seenNonces.has("replayed"), true);
  assert.equal(seeded.nextState.seededAgentPubkeys.has(AGENT), true);

  const live = applyPermissionAlertStoreNotification({
    state: seeded.nextState,
    initialReplayComplete: true,
    agents: [{ pubkey: AGENT, transcript: [permissionItem("replayed")] }],
    update: { agentPubkey: AGENT, events: [permissionRead({ nonce: "live" })] },
  });
  assert.deepEqual(live.alerts.map((alert) => alert.requestNonce), ["live"]);

  const outsider = applyPermissionAlertStoreNotification({
    state: live.nextState,
    initialReplayComplete: true,
    agents: [{ pubkey: AGENT, transcript: [permissionItem("replayed")] }],
    update: { agentPubkey: OTHER_AGENT, events: [permissionRead({ nonce: "outsider" })] },
  });
  assert.deepEqual(outsider.alerts, []);
  assert.equal(outsider.nextState.seenNonces.has("outsider"), false);
  assert.deepEqual(collectSeenPermissionNonces([]), []);
});

test("selectPermissionAlertSurface keeps focused toast independent of desktopEnabled", () => {
  assert.equal(selectPermissionAlertSurface({ focused: true, desktopEnabled: false }), "toast");
  assert.equal(selectPermissionAlertSurface({ focused: false, desktopEnabled: true }), "os");
  assert.equal(selectPermissionAlertSurface({ focused: false, desktopEnabled: false }), null);
});

test("startPermissionAlertStoreSubscription processes the current snapshot before live updates", () => {
  const calls = [];
  let listener;
  const unsubscribe = startPermissionAlertStoreSubscription({
    handleUpdate: (update) => calls.push(update?.agentPubkey ?? "snapshot"),
    subscribe: (nextListener) => {
      calls.push("subscribe");
      listener = nextListener;
      return () => calls.push("unsubscribe");
    },
  });

  assert.deepEqual(calls, ["snapshot", "subscribe"]);
  listener({ agentPubkey: AGENT, events: [] });
  unsubscribe();
  assert.deepEqual(calls, ["snapshot", "subscribe", AGENT, "unsubscribe"]);
});
```

- [ ] **Step 3: Run the test and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlert.test.mjs
```

Expected: FAIL because `pendingPermissionAlert.ts` does not exist.

- [ ] **Step 4: Implement the minimum pure module**

Create `desktop/src/features/agents/pendingPermissionAlert.ts`.
Use this implementation:

```ts
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
```

Implementation rules:

- `extractActionablePermission` accepts only `kind === "acp_read"`, payload method `session/request_permission`, `authorization.actionable === true`, and a non-empty nonce.
- Read title from `params.title`, then `params.message`, then `params.reason`, then `Permission requested`.
- `extractResolvedPermissionNonce` accepts any enveloped `acp_write`, plus a non-actionable enveloped permission `acp_read`.
- `shouldSuppressPermissionAlert` normalizes only agent pubkeys and applies the design's channel OR rule.
- `collectSeenPermissionNonces` includes actionable and already-resolved permission transcript items.
- `applyPermissionAlertUpdate` adds every terminal nonce to `dismissNonces`, deduplicates dismissals, and filters alerts whose nonce resolves in the same batch.
- `createPermissionAlertStoreState` returns two new empty sets.
- `applyPermissionAlertStoreNotification` clones both sets before changing them.
- `startPermissionAlertStoreSubscription` processes an update-less current snapshot before registering the same callback for live store notifications.
- When `initialReplayComplete` is false, the store controller returns no alerts and does not seed or consume the update nonce.
- When replay is complete, seed only agents not already in `seededAgentPubkeys`.
- Build the owned-agent set from the same `agents` input and reject any update outside it.
- After seeding, delegate the live batch to `applyPermissionAlertUpdate`.
- Add doc comments to every exported type or function.

- [ ] **Step 5: Run the test and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlert.test.mjs
```

Expected: PASS.

- [ ] **Step 6: Refactor and verify**

Remove duplication only if the same test remains green.
Do not change pinned copy or add option text.

- [ ] **Step 7: Run staged change detection and commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/agents/pendingPermissionAlert.ts desktop/src/features/agents/pendingPermissionAlert.test.mjs && git diff --cached --stat && git diff --cached --name-only && git diff --cached --check
```

Run `detect_changes({ scope: "staged" })` and confirm only the new permission-alert helper symbols are affected.

```bash
. ./bin/activate-hermit && git commit -s -m "feat(desktop): add permission alert controller"
```

---

### Task 2: Establish an initial observer-replay barrier

**Files:**

- Modify: `desktop/src/shared/api/observerRelay.ts`
- Modify: `desktop/src/shared/api/observerRelay.test.mjs`
- Modify: `desktop/src/features/agents/observerRelayStore.ts`
- Create: `desktop/src/features/agents/observerInitialReplay.test.mjs`

**Interfaces:**

```ts
export function subscribeToAgentObserverFrames(
  ownerPubkey: string,
  onEvent: (event: RelayEvent) => void,
  onReady?: (readiness: LiveSubscriptionReadiness) => void,
): Promise<() => Promise<void>>;

export function isAgentObserverInitialReplayComplete(): boolean;

export function _testEnqueueObserverStoreWork(
  work: () => Promise<void>,
): void;

export function _testQueueInitialObserverReplayComplete(): Promise<void>;
```

The store may expose only the two documented `_test*` helpers above for the focused readiness test.

- [ ] **Step 1: Run required impact analysis**

Run upstream impact for `subscribeToAgentObserverFrames`, `ensureRelayObserverSubscription`, `notifyListeners`, `resetAgentObserverStore`, and `AgentObserverStoreUpdate`.
Confirm `subscribeToAgentObserverFrames` is consumed by `observerRelayStore`, and confirm store listeners tolerate an update-less `notifyListeners()` because connection and archive paths already use it.

- [ ] **Step 2: Write failing readiness-forwarding coverage**

Append this test to `desktop/src/shared/api/observerRelay.test.mjs`:

```js
test("subscribeToAgentObserverFrames forwards initial subscription readiness", () => {
  const readiness = [];
  mock.method(relayClient, "subscribeLive", (_filter, _onEvent, onReady) => {
    onReady?.("eose");
    return async () => {};
  });

  subscribeToAgentObserverFrames(
    "owner-pubkey",
    () => {},
    (value) => readiness.push(value),
  );

  assert.deepEqual(readiness, ["eose"]);
  mock.reset();
});
```

Create `desktop/src/features/agents/observerInitialReplay.test.mjs` with:

```js
import assert from "node:assert/strict";
import test from "node:test";

import {
  _testEnqueueObserverStoreWork,
  _testQueueInitialObserverReplayComplete,
  isAgentObserverInitialReplayComplete,
  resetAgentObserverStore,
  subscribeAgentObserverStore,
} from "./observerRelayStore.ts";

test("initial observer replay completes once after the queued EOSE barrier and resets", async () => {
  resetAgentObserverStore();
  const notifications = [];
  const unsubscribe = subscribeAgentObserverStore((update) => notifications.push(update));

  assert.equal(isAgentObserverInitialReplayComplete(), false);
  let releaseQueuedWork;
  const queuedWork = new Promise((resolve) => {
    releaseQueuedWork = resolve;
  });
  _testEnqueueObserverStoreWork(() => queuedWork);
  const completion = _testQueueInitialObserverReplayComplete();
  await Promise.resolve();
  assert.equal(isAgentObserverInitialReplayComplete(), false);
  assert.deepEqual(notifications, []);
  releaseQueuedWork();
  await completion;
  assert.equal(isAgentObserverInitialReplayComplete(), true);
  assert.deepEqual(notifications, [undefined]);

  await _testQueueInitialObserverReplayComplete();
  assert.deepEqual(notifications, [undefined]);

  resetAgentObserverStore();
  assert.equal(isAgentObserverInitialReplayComplete(), false);
  unsubscribe();
});
```

- [ ] **Step 3: Run both tests and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/shared/api/observerRelay.test.mjs src/features/agents/observerInitialReplay.test.mjs
```

Expected: FAIL because readiness is not forwarded and the store readiness APIs do not exist.

- [ ] **Step 4: Forward readiness from the relay wrapper**

In `desktop/src/shared/api/observerRelay.ts`, add `import type { LiveSubscriptionReadiness } from "./relayClientShared";`.
Add the optional `onReady` parameter and pass it as the third argument to `relayClient.subscribeLive`.
Do not change `limit: 1000`, the 300-second `since`, or the event callback.
The resulting function call must have this shape:

```ts
export function subscribeToAgentObserverFrames(
  ownerPubkey: string,
  onEvent: (event: RelayEvent) => void,
  onReady?: (readiness: LiveSubscriptionReadiness) => void,
) {
  return relayClient.subscribeLive(
    {
      kinds: [KIND_AGENT_OBSERVER_FRAME],
      "#p": [ownerPubkey],
      limit: 1000,
      since: Math.floor(Date.now() / 1_000) - OBSERVER_LIVE_LOOKBACK_SECS,
    },
    onEvent,
    onReady,
  );
}
```

- [ ] **Step 5: Add the queued EOSE barrier to the observer store**

In `desktop/src/features/agents/observerRelayStore.ts`:

1. Add module state `let initialReplayComplete = false;` beside the other connection state.
2. Add a private `queueInitialObserverReplayComplete(activeGeneration: number)` function.
3. Return the current `eventProcessingQueue` immediately when the flag is already true.
4. Otherwise append a callback to `eventProcessingQueue`.
5. In the callback, return when `activeGeneration !== generation` or the flag became true.
6. Set the flag to true and call `notifyListeners()` without an event update.
7. Export a documented `isAgentObserverInitialReplayComplete()` getter.
8. Pass a readiness callback to `subscribeToAgentObserverFrames` inside `ensureRelayObserverSubscription`.
9. Queue completion only for `readiness === "eose"` and pass the captured `activeGeneration`.
10. Set `initialReplayComplete = false` in `resetAgentObserverStore()` before its final `notifyListeners()`.
11. Export a documented `_testEnqueueObserverStoreWork(work)` helper that appends controlled work to `eventProcessingQueue` without notifying listeners.
12. Export a documented `_testQueueInitialObserverReplayComplete()` that calls the private queue helper with the current generation and returns its promise.

Use these exact store functions:

```ts
let initialReplayComplete = false;

function queueInitialObserverReplayComplete(
  activeGeneration: number,
): Promise<void> {
  if (initialReplayComplete) {
    return eventProcessingQueue;
  }
  eventProcessingQueue = eventProcessingQueue.then(() => {
    if (activeGeneration !== generation || initialReplayComplete) {
      return;
    }
    initialReplayComplete = true;
    notifyListeners();
  });
  return eventProcessingQueue;
}

/** Return whether the first observer subscription replay reached EOSE. */
export function isAgentObserverInitialReplayComplete(): boolean {
  return initialReplayComplete;
}

/** Test-only: append controlled work before the initial replay barrier. */
export function _testEnqueueObserverStoreWork(
  work: () => Promise<void>,
): void {
  eventProcessingQueue = eventProcessingQueue.then(work);
}

/** Test-only: queue and await initial replay completion. */
export function _testQueueInitialObserverReplayComplete(): Promise<void> {
  return queueInitialObserverReplayComplete(generation);
}
```

Pass this third callback in `ensureRelayObserverSubscription`:

```ts
(readiness) => {
  if (readiness === "eose") {
    void queueInitialObserverReplayComplete(activeGeneration);
  }
},
```

Do not put readiness on `AgentObserverStoreUpdate`.
The existing update-less store notification is the readiness signal, and consumers read the getter.

- [ ] **Step 6: Run both tests and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/shared/api/observerRelay.test.mjs src/features/agents/observerInitialReplay.test.mjs
```

Expected: PASS, including the existing replay-filter tests.

- [ ] **Step 7: Run observer-store regression tests**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/activeAgentTurnsStore.test.mjs src/features/agents/ingestArchivedObserverEvents.test.mjs src/features/agents/observerTranscriptRetention.test.mjs
```

Expected: PASS.

- [ ] **Step 8: Run staged change detection and commit**

```bash
. ./bin/activate-hermit && git add desktop/src/shared/api/observerRelay.ts desktop/src/shared/api/observerRelay.test.mjs desktop/src/features/agents/observerRelayStore.ts desktop/src/features/agents/observerInitialReplay.test.mjs && git diff --cached --stat && git diff --cached --name-only && git diff --cached --check
```

Run `detect_changes({ scope: "staged" })` and confirm the observer subscription, observer ingestion, active-turn, and transcript flows are the expected affected scope.

```bash
. ./bin/activate-hermit && git commit -s -m "fix(desktop): gate permission alerts after observer replay"
```

---

### Task 3: Accept `agentPubkey` in desktop notification targets

**Files:**

- Modify: `desktop/src/features/notifications/lib/desktop.test.mjs`
- Modify: `desktop/src/features/notifications/lib/desktop.ts`

**Interface change:**

```ts
export type DesktopNotificationTarget = {
  agentPubkey?: string;
  channelId: string | null;
  channelName?: string | null;
  content?: string;
  createdAt?: number | null;
  eventId: string | null;
  kind: number | null;
  pubkey?: string;
  threadRootId?: string | null;
};

export function parseNotificationTarget(value: unknown): DesktopNotificationTarget | null;
```

- [ ] **Step 1: Run required impact analysis**

Run upstream impact for `DesktopNotificationTarget`, `parseNotificationTarget`, `listenForDesktopNotificationActions`, and `sendDesktopNotification`.
Confirm native Linux and macOS accept `Option<serde_json::Value>` and need no Rust schema edit.

- [ ] **Step 2: Write failing parser tests**

In `desktop.test.mjs`, preserve the constructor-failure test and change the dynamic import to bind both exports:

```js
const { parseNotificationTarget, sendDesktopNotification } = await import("./desktop.ts");
```

Append:

```js
test("parseNotificationTarget accepts agentPubkey without channel or event", () => {
  assert.deepEqual(parseNotificationTarget({
    agentPubkey: "aa".repeat(32),
    channelId: null,
    eventId: null,
    kind: null,
  }), {
    agentPubkey: "aa".repeat(32),
    channelId: null,
    channelName: null,
    content: undefined,
    createdAt: null,
    eventId: null,
    kind: null,
    pubkey: undefined,
    threadRootId: null,
  });
});

test("parseNotificationTarget does not reinterpret message pubkey as agent identity", () => {
  assert.equal(parseNotificationTarget({
    pubkey: "aa".repeat(32),
    channelId: null,
    eventId: null,
    kind: 1,
  }), null);
});

test("parseNotificationTarget keeps event-only join targets valid", () => {
  const target = parseNotificationTarget({
    channelId: null,
    eventId: "ab".repeat(32),
    kind: 8000,
  });
  assert.equal(target?.eventId, "ab".repeat(32));
  assert.equal(target?.agentPubkey, undefined);
});

test("parseNotificationTarget rejects empty agentPubkey without another anchor", () => {
  assert.equal(parseNotificationTarget({
    agentPubkey: "",
    channelId: null,
    eventId: null,
    kind: null,
  }), null);
});
```

- [ ] **Step 3: Run the test and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/notifications/lib/desktop.test.mjs
```

Expected: FAIL because `parseNotificationTarget` is not exported or the agent-only target is rejected.

- [ ] **Step 4: Implement the parser change**

In `desktop.ts`:

1. Add `agentPubkey?: string` to `DesktopNotificationTarget`.
2. Export `parseNotificationTarget` with a doc comment.
3. Parse `agentPubkey` only when it is a string with `length > 0`.
4. Replace the validity guard with `if (!channelId && !eventId && !agentPubkey) return null;`.
5. Include `agentPubkey` in the returned object.
6. Leave every other parsed field unchanged.

Use this exact addition inside the parser:

```ts
const agentPubkey =
  typeof candidate.agentPubkey === "string" &&
  candidate.agentPubkey.length > 0
    ? candidate.agentPubkey
    : undefined;

if (!channelId && !eventId && !agentPubkey) {
  return null;
}

return {
  agentPubkey,
  channelId,
  channelName,
  content,
  createdAt,
  eventId,
  kind,
  pubkey,
  threadRootId,
};
```

- [ ] **Step 5: Run the test and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/notifications/lib/desktop.test.mjs
```

Expected: PASS.

- [ ] **Step 6: Run staged change detection and commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/notifications/lib/desktop.ts desktop/src/features/notifications/lib/desktop.test.mjs && git diff --cached --stat && git diff --cached --name-only && git diff --cached --check
```

Run `detect_changes({ scope: "staged" })` and confirm only desktop notification construction and click-target parsing are affected.

```bash
. ./bin/activate-hermit && git commit -s -m "fix(desktop): accept agent notification targets"
```

---

### Task 4: Route notification clicks through agent activity

**Files:**

- Modify: `desktop/src/app/AppShell.helpers.test.mjs`
- Modify: `desktop/src/app/AppShell.helpers.ts`
- Modify: `desktop/src/app/useAppShellDesktopNotifications.ts`

**Interfaces:**

```ts
export type DesktopNotificationAction =
  | { type: "agent-activity"; agentPubkey: string; channelId: string | null }
  | { type: "home" }
  | { type: "channel"; channelId: string }
  | { type: "search-hit"; hit: SearchHit };

export function resolveDesktopNotificationAction(
  target: DesktopNotificationTarget,
): DesktopNotificationAction;

export type DesktopNotificationActionDependencies = {
  revealDesktopAppWindow: () => Promise<void>;
  openAgentActivity: (
    pubkey: string,
    options: { channelId: string | null },
  ) => unknown;
  goHome: () => Promise<unknown>;
  goChannel: (channelId: string) => Promise<unknown>;
  openSearchHit: (hit: SearchHit) => Promise<unknown>;
};

export async function executeDesktopNotificationAction(
  target: DesktopNotificationTarget,
  dependencies: DesktopNotificationActionDependencies,
): Promise<void>;
```

- [ ] **Step 1: Run required impact analysis**

Run upstream impact for `toSearchHit`, `useAppShellDesktopNotifications`, and the local `handleDesktopNotificationAction` effect event.
Confirm the hook is called by `AppShell` and the action listener remains main-window-only through its existing `enabled` parameter.

- [ ] **Step 2: Write failing routing and executor tests**

Add both new exports to the existing single import from `./AppShell.helpers.ts`.
Do not add a second import from the same module.

Append:

```js
test("resolveDesktopNotificationAction gives agentPubkey precedence over a search hit", () => {
  assert.deepEqual(resolveDesktopNotificationAction({
    agentPubkey: "aa".repeat(32),
    channelId: "channel-1",
    content: "ignored",
    createdAt: 1,
    eventId: "ab".repeat(32),
    kind: 9,
    pubkey: "cc".repeat(32),
    threadRootId: null,
  }), {
    type: "agent-activity",
    agentPubkey: "aa".repeat(32),
    channelId: "channel-1",
  });
});

test("resolveDesktopNotificationAction keeps no-channel event targets on Home", () => {
  assert.deepEqual(resolveDesktopNotificationAction({
    channelId: null,
    eventId: "ab".repeat(32),
    kind: 8000,
  }), { type: "home" });
});

test("resolveDesktopNotificationAction keeps channel-only targets on channel routing", () => {
  assert.deepEqual(resolveDesktopNotificationAction({
    channelId: "channel-1",
    eventId: null,
    kind: null,
  }), { type: "channel", channelId: "channel-1" });
});

test("executeDesktopNotificationAction reveals before opening agent activity and never opens a search hit", async () => {
  const calls = [];
  await executeDesktopNotificationAction({
    agentPubkey: "aa".repeat(32),
    channelId: null,
    eventId: "ab".repeat(32),
    kind: 9,
  }, {
    revealDesktopAppWindow: async () => calls.push("reveal"),
    openAgentActivity: (pubkey, options) => calls.push(`agent:${pubkey}:${options.channelId}`),
    goHome: async () => calls.push("home"),
    goChannel: async (channelId) => calls.push(`channel:${channelId}`),
    openSearchHit: async () => calls.push("search"),
  });
  assert.deepEqual(calls, ["reveal", `agent:${"aa".repeat(32)}:null`]);
});

test("executeDesktopNotificationAction preserves message search-hit routing", async () => {
  const calls = [];
  await executeDesktopNotificationAction({
    channelId: "channel-1",
    eventId: "ab".repeat(32),
    kind: 9,
    pubkey: "cc".repeat(32),
  }, {
    revealDesktopAppWindow: async () => calls.push("reveal"),
    openAgentActivity: () => calls.push("agent"),
    goHome: async () => calls.push("home"),
    goChannel: async () => calls.push("channel"),
    openSearchHit: async (hit) => calls.push(`search:${hit.eventId}`),
  });
  assert.deepEqual(calls, ["reveal", `search:${"ab".repeat(32)}`]);
});
```

- [ ] **Step 3: Run the helper test and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/app/AppShell.helpers.test.mjs
```

Expected: FAIL because the resolver and executor are not exported.

- [ ] **Step 4: Implement the tested resolver and executor**

Add both documented exports after `toSearchHit` in `AppShell.helpers.ts`.
Use this routing order:

1. Non-empty `agentPubkey`.
2. Missing `channelId` to Home.
3. A valid `toSearchHit` result.
4. Channel fallback.

The executor must await reveal first, then execute exactly one resolved action.
Await Home, channel, and search navigation promises.
Call `openAgentActivity(agentPubkey, { channelId })` synchronously after reveal and return.
Use this implementation:

```ts
/** Resolve a desktop notification target to one navigation action. */
export function resolveDesktopNotificationAction(
  target: DesktopNotificationTarget,
): DesktopNotificationAction {
  if (target.agentPubkey) {
    return {
      type: "agent-activity",
      agentPubkey: target.agentPubkey,
      channelId: target.channelId,
    };
  }
  if (!target.channelId) {
    return { type: "home" };
  }
  const hit = toSearchHit(target);
  return hit
    ? { type: "search-hit", hit }
    : { type: "channel", channelId: target.channelId };
}

/** Reveal Buzz and execute exactly one resolved notification action. */
export async function executeDesktopNotificationAction(
  target: DesktopNotificationTarget,
  dependencies: DesktopNotificationActionDependencies,
): Promise<void> {
  await dependencies.revealDesktopAppWindow();
  const action = resolveDesktopNotificationAction(target);
  if (action.type === "agent-activity") {
    dependencies.openAgentActivity(action.agentPubkey, {
      channelId: action.channelId,
    });
    return;
  }
  if (action.type === "home") {
    await dependencies.goHome();
    return;
  }
  if (action.type === "channel") {
    await dependencies.goChannel(action.channelId);
    return;
  }
  await dependencies.openSearchHit(action.hit);
}
```

Define and document `DesktopNotificationActionDependencies` with the exact dependency signatures already declared in this task's Interfaces block.

- [ ] **Step 5: Replace the hook's inline branch with the executor**

In `useAppShellDesktopNotifications.ts`:

1. Import `executeDesktopNotificationAction` instead of `toSearchHit`.
2. Import `useOpenAgentActivity`.
3. Call `const { openAgentActivity } = useOpenAgentActivity();` unconditionally at hook scope.
4. Make the effect event return `executeDesktopNotificationAction(target, { revealDesktopAppWindow, openAgentActivity, goHome, goChannel, openSearchHit })`.
5. Keep the listener effect's existing `if (!enabled) return;` guard so the huddle companion does not also consume native clicks.
6. Do not add arguments to `useAppShellDesktopNotifications`.

The replacement effect event is:

```ts
const handleDesktopNotificationAction = React.useEffectEvent(
  (target: DesktopNotificationTarget) =>
    executeDesktopNotificationAction(target, {
      revealDesktopAppWindow,
      openAgentActivity,
      goHome,
      goChannel,
      openSearchHit,
    }),
);
```

- [ ] **Step 6: Run tests and typecheck**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/app/AppShell.helpers.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 7: Run staged change detection and commit**

```bash
. ./bin/activate-hermit && git add desktop/src/app/AppShell.helpers.ts desktop/src/app/AppShell.helpers.test.mjs desktop/src/app/useAppShellDesktopNotifications.ts && git diff --cached --stat && git diff --cached --name-only && git diff --cached --check
```

Run `detect_changes({ scope: "staged" })` and confirm agent, message search-hit, join-alert Home, and channel fallback click flows are the expected scope.

```bash
. ./bin/activate-hermit && git commit -s -m "feat(desktop): open agent activity from notifications"
```

---

### Task 5: Preserve the `agentSessionChannel` route contract

**Files:**

- Create: `desktop/src/app/routes/channels.$channelId.test.mjs`
- Modify: `desktop/src/app/routes/channels.$channelId.tsx`

**Interface:**

```ts
type ChannelRouteSearch = {
  agentSession?: string;
  agentSessionChannel?: string;
  // Existing fields remain unchanged.
};

export function validateChannelSearch(
  search: Record<string, unknown>,
): ChannelRouteSearch;
```

- [ ] **Step 1: Run required impact analysis**

Run upstream impact for the channel route `Route`, `useChannelPanelHistoryState`, and `openAgentSessionChannelId`.
Confirm `agentSessionChannel` is already written and read by `useChannelPanelHistoryState` but is missing from the route validator.

- [ ] **Step 2: Write the failing route-search test**

Create `desktop/src/app/routes/channels.$channelId.test.mjs` with:

```js
import assert from "node:assert/strict";
import test from "node:test";

import { validateChannelSearch } from "./channels.$channelId.tsx";

test("channel route preserves non-empty agentSessionChannel search", () => {
  assert.equal(validateChannelSearch({
    agentSession: "aa".repeat(32),
    agentSessionChannel: "channel-2",
  }).agentSessionChannel, "channel-2");
});

test("channel route drops empty agentSessionChannel search", () => {
  assert.equal(validateChannelSearch({
    agentSessionChannel: "",
  }).agentSessionChannel, undefined);
});
```

- [ ] **Step 3: Run the test and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test 'src/app/routes/channels.$channelId.test.mjs'
```

Expected: FAIL because `validateChannelSearch` is not exported and the validated type omits `agentSessionChannel`.

- [ ] **Step 4: Implement the route contract**

In `channels.$channelId.tsx`:

1. Add `agentSessionChannel?: string` beside `agentSession` in `ChannelRouteSearch`.
2. Export `validateChannelSearch` with a doc comment.
3. Return `agentSessionChannel: nonEmptyString(search.agentSessionChannel)` beside `agentSession`.
4. Leave every existing search field and route component unchanged.

- [ ] **Step 5: Run the test and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test 'src/app/routes/channels.$channelId.test.mjs' && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 6: Run staged change detection and commit**

```bash
. ./bin/activate-hermit && git add 'desktop/src/app/routes/channels.$channelId.tsx' 'desktop/src/app/routes/channels.$channelId.test.mjs' && git diff --cached --stat && git diff --cached --name-only && git diff --cached --check
```

Run `detect_changes({ scope: "staged" })` and confirm only channel-route search validation and its existing panel consumers are affected.

```bash
. ./bin/activate-hermit && git commit -s -m "fix(desktop): preserve agent session channel search"
```

---

### Task 6: Share the canonical owner-global ingestion-agent list

**Files:**

- Modify: `desktop/src/features/agents/useAgentObserverIngestion.ts`
- Test unchanged: `desktop/src/features/agents/useAgentObserverIngestion.test.mjs`

**Interface:**

```ts
export function useObserverIngestionAgents(): Array<
  Pick<ManagedAgent, "pubkey" | "status">
>;
```

- [ ] **Step 1: Run required impact analysis**

Run upstream impact for `useAgentObserverIngestion` and `combineObserverIngestionAgents`.
Confirm `useAgentObserverIngestion` has one production caller in `AppShell.tsx`.

- [ ] **Step 2: Establish the green refactor baseline**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/useAgentObserverIngestion.test.mjs
```

Expected: PASS.

- [ ] **Step 3: Extract the hook without changing behavior**

Move the identity, managed-agent, relay-agent, profile, and `React.useMemo` query graph from `useAgentObserverIngestion` into the documented exported hook.
Return the existing `ingestionAgents` value.
Do not change `combineObserverIngestionAgents`.
Keep `useAgentObserverIngestion` as:

```ts
export function useAgentObserverIngestion() {
  const ingestionAgents = useObserverIngestionAgents();
  useManagedAgentObserverBridge(ingestionAgents);
  useActiveAgentTurnsBridge(ingestionAgents);
}
```

- [ ] **Step 4: Re-run characterization tests and typecheck**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/useAgentObserverIngestion.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 5: Run staged change detection and commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/agents/useAgentObserverIngestion.ts && git diff --cached --stat && git diff --cached --name-only && git diff --cached --check
```

Run `detect_changes({ scope: "staged" })` and confirm observer ingestion and active-turn bridges are unchanged consumers of the extracted list.

```bash
. ./bin/activate-hermit && git commit -s -m "refactor(desktop): share observer ingestion agents"
```

---

### Task 7: Mount and deliver pending-permission alerts

**Files:**

- Create: `desktop/src/features/agents/pendingPermissionAlertDelivery.test.mjs`
- Create: `desktop/src/features/agents/pendingPermissionAlertDelivery.ts`
- Create: `desktop/src/features/agents/usePendingPermissionAlerts.ts`
- Modify: `desktop/src/app/AppShell.tsx`

**Interface:**

```ts
export function usePendingPermissionAlerts(input: {
  enabled: boolean;
  notificationSettings: NotificationSettings;
}): void;
```

```ts
export type PermissionAlertDeliveryInput = {
  agentPubkey: string;
  channelId: string | null;
  copy: { title: string; body: string };
  requestNonce: string;
  soundEnabled: boolean;
  surface: "toast" | "os" | null;
};

export type PermissionAlertDeliveryDependencies = {
  showToast: (toast: {
    body: string;
    duration: number;
    id: string;
    onClick: () => Promise<void>;
    title: string;
  }) => void;
  sendOsNotification: (notification: {
    title: string;
    body: string;
    target: {
      agentPubkey: string;
      channelId: string | null;
      eventId: null;
      kind: null;
    };
  }) => Promise<boolean>;
  revealDesktopAppWindow: () => Promise<void>;
  openAgentActivity: (
    pubkey: string,
    options: { channelId: string | null },
  ) => unknown;
  requestDockBounce: () => Promise<void>;
  playSound: () => void;
};

export async function deliverPermissionAlert(
  input: PermissionAlertDeliveryInput,
  dependencies: PermissionAlertDeliveryDependencies,
): Promise<void>;
```

The hook consumes the Task 1 controller, Task 2 readiness getter, Task 3 target, Task 4 click ingress, Task 5 route contract, and Task 6 agent list.

- [ ] **Step 1: Run required impact analysis**

Run upstream impact for `subscribeAgentObserverStore`, `getAgentTranscript`, `isAgentObserverInitialReplayComplete`, `useOpenAgentActivity`, `resolveUserLabel`, `sendDesktopNotification`, `requestDockBounce`, `playNotificationSound`, `useChannelPanelHistoryState`, and `AppShell`.
Warn and stop before editing if `AppShell` or any notification path is HIGH or CRITICAL.

- [ ] **Step 2: Write failing delivery-effect tests**

Create `desktop/src/features/agents/pendingPermissionAlertDelivery.test.mjs` with:

```js
import assert from "node:assert/strict";
import test from "node:test";

import { deliverPermissionAlert } from "./pendingPermissionAlertDelivery.ts";

const AGENT = "a".repeat(64);

function dependencies(calls, { capture, didSend = true } = {}) {
  return {
    showToast: (toast) => {
      calls.push(`toast:${toast.id}:${toast.title}:${toast.body}:${toast.duration}`);
      if (capture) {
        capture.toastClick = toast.onClick;
      }
    },
    sendOsNotification: async (notification) => {
      calls.push(["os", notification]);
      return didSend;
    },
    revealDesktopAppWindow: async () => {
      calls.push("reveal");
    },
    openAgentActivity: (pubkey, options) => {
      calls.push(`open:${pubkey}:${options.channelId}`);
    },
    requestDockBounce: async () => {
      calls.push("bounce");
    },
    playSound: () => {
      calls.push("sound");
    },
  };
}

test("focused delivery shows a persistent toast sounds and reveals before opening", async () => {
  const calls = [];
  const capture = {};
  await deliverPermissionAlert({
    agentPubkey: AGENT,
    channelId: "channel-1",
    copy: { title: "Ada needs permission", body: "Open the process log." },
    requestNonce: "nonce-1",
    soundEnabled: true,
    surface: "toast",
  }, dependencies(calls, { capture }));

  assert.deepEqual(calls, [
    "toast:nonce-1:Ada needs permission:Open the process log.:Infinity",
    "sound",
  ]);
  await capture.toastClick();
  assert.deepEqual(calls.slice(-2), ["reveal", `open:${AGENT}:channel-1`]);
});

test("successful OS delivery bounces and sounds after send", async () => {
  const calls = [];
  await deliverPermissionAlert({
    agentPubkey: AGENT,
    channelId: null,
    copy: { title: "Ada needs permission", body: "Open the process log." },
    requestNonce: "nonce-2",
    soundEnabled: true,
    surface: "os",
  }, dependencies(calls));
  assert.deepEqual(calls, [
    ["os", {
      title: "Ada needs permission",
      body: "Open the process log.",
      target: {
        agentPubkey: AGENT,
        channelId: null,
        eventId: null,
        kind: null,
      },
    }],
    "bounce",
    "sound",
  ]);
});

test("failed OS delivery and null surface have no follow-up effects", async () => {
  const failed = [];
  await deliverPermissionAlert({
    agentPubkey: AGENT,
    channelId: null,
    copy: { title: "Ada needs permission", body: "Open the process log." },
    requestNonce: "nonce-3",
    soundEnabled: true,
    surface: "os",
  }, dependencies(failed, { didSend: false }));
  assert.deepEqual(failed, [["os", {
    title: "Ada needs permission",
    body: "Open the process log.",
    target: {
      agentPubkey: AGENT,
      channelId: null,
      eventId: null,
      kind: null,
    },
  }]]);

  const mutedToast = [];
  await deliverPermissionAlert({
    agentPubkey: AGENT,
    channelId: null,
    copy: { title: "Ada needs permission", body: "Open the process log." },
    requestNonce: "nonce-muted",
    soundEnabled: false,
    surface: "toast",
  }, dependencies(mutedToast));
  assert.deepEqual(mutedToast, [
    "toast:nonce-muted:Ada needs permission:Open the process log.:Infinity",
  ]);

  const silent = [];
  await deliverPermissionAlert({
    agentPubkey: AGENT,
    channelId: null,
    copy: { title: "Ada needs permission", body: "Open the process log." },
    requestNonce: "nonce-4",
    soundEnabled: false,
    surface: null,
  }, dependencies(silent));
  assert.deepEqual(silent, []);
});

test("successful muted OS delivery bounces without playing sound", async () => {
  const calls = [];
  await deliverPermissionAlert({
    agentPubkey: AGENT,
    channelId: null,
    copy: { title: "Ada needs permission", body: "Open the process log." },
    requestNonce: "nonce-muted-os",
    soundEnabled: false,
    surface: "os",
  }, dependencies(calls));
  assert.deepEqual(calls, [
    ["os", {
      title: "Ada needs permission",
      body: "Open the process log.",
      target: {
        agentPubkey: AGENT,
        channelId: null,
        eventId: null,
        kind: null,
      },
    }],
    "bounce",
  ]);
});
```

- [ ] **Step 3: Run the delivery test and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlertDelivery.test.mjs
```

Expected: FAIL because `pendingPermissionAlertDelivery.ts` does not exist.

- [ ] **Step 4: Implement the minimum tested delivery executor**

Create `desktop/src/features/agents/pendingPermissionAlertDelivery.ts` with the declared documented type and function.
For a null surface, return without calling a dependency.
For `toast`, call `showToast` with `Number.POSITIVE_INFINITY`, call `playSound` only when enabled, and make `onClick` await reveal before opening activity.
For `os`, await `sendOsNotification`, return on false, await `requestDockBounce` on true, and call `playSound` only when enabled.
Use this implementation after the documented type declarations:

```ts
/** Execute one permission-alert surface and its follow-up effects. */
export async function deliverPermissionAlert(
  input: PermissionAlertDeliveryInput,
  dependencies: PermissionAlertDeliveryDependencies,
): Promise<void> {
  if (input.surface === null) {
    return;
  }
  if (input.surface === "toast") {
    dependencies.showToast({
      body: input.copy.body,
      duration: Number.POSITIVE_INFINITY,
      id: input.requestNonce,
      title: input.copy.title,
      onClick: async () => {
        await dependencies.revealDesktopAppWindow();
        dependencies.openAgentActivity(input.agentPubkey, {
          channelId: input.channelId,
        });
      },
    });
    if (input.soundEnabled) {
      dependencies.playSound();
    }
    return;
  }

  const didSend = await dependencies.sendOsNotification({
    title: input.copy.title,
    body: input.copy.body,
    target: {
      agentPubkey: input.agentPubkey,
      channelId: input.channelId,
      eventId: null,
      kind: null,
    },
  });
  if (!didSend) {
    return;
  }
  await dependencies.requestDockBounce();
  if (input.soundEnabled) {
    dependencies.playSound();
  }
}
```

Name and export the dependency type as `PermissionAlertDeliveryDependencies`, using the exact dependency signatures from this task's Interfaces block.

- [ ] **Step 5: Run the delivery test and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlertDelivery.test.mjs
```

Expected: PASS.

- [ ] **Step 6: Create the React adapter**

Create `desktop/src/features/agents/usePendingPermissionAlerts.ts` with these imports and responsibilities:

- React and sonner `toast`.
- `useLocation` and `deriveShellRoute` for `selectedChannelId`.
- `useChannelPanelHistoryState` for `openAgentSessionPubkey` and `openAgentSessionChannelId`.
- `useChannelsQuery` for the optional channel name.
- `NotificationSettings` as a type only.
- `requestDockBounce`, `revealDesktopAppWindow`, and `sendDesktopNotification`.
- `playNotificationSound` and `resolveSlotSound`.
- `resolveUserLabel` and `useUsersBatchQuery`.
- `isAppFocused`.
- `getAgentTranscript`, `isAgentObserverInitialReplayComplete`, `subscribeAgentObserverStore`, and `AgentObserverStoreUpdate`.
- Task 1 helper/controller exports.
- `deliverPermissionAlert` from the tested delivery executor.
- `useObserverIngestionAgents` and `useOpenAgentActivity`.

Use this exact state and subscription shape:

```ts
/** Deliver app-level owner permission alerts from the observer store. */
export function usePendingPermissionAlerts({
  enabled,
  notificationSettings,
}: {
  enabled: boolean;
  notificationSettings: NotificationSettings;
}): void {
  const ingestionAgents = useObserverIngestionAgents();
  const agentPubkeys = React.useMemo(
    () => ingestionAgents.map((agent) => agent.pubkey),
    [ingestionAgents],
  );
  const profilesQuery = useUsersBatchQuery(agentPubkeys, {
    enabled: agentPubkeys.length > 0,
  });
  const channelsQuery = useChannelsQuery();
  const channels = channelsQuery.data ?? [];
  const { openAgentActivity } = useOpenAgentActivity();
  const location = useLocation();
  const { selectedChannelId } = deriveShellRoute(location.pathname);
  const { openAgentSessionChannelId, openAgentSessionPubkey } =
    useChannelPanelHistoryState();
  const [alertState] = React.useState(createPermissionAlertStoreState);

  const handleStoreUpdate = React.useEffectEvent(
    (update?: AgentObserverStoreUpdate) => {
      const applied = applyPermissionAlertStoreNotification({
        state: alertState,
        initialReplayComplete: isAgentObserverInitialReplayComplete(),
        agents: ingestionAgents.map((agent) => ({
          pubkey: agent.pubkey,
          transcript: getAgentTranscript(agent.pubkey),
        })),
        update,
      });

      alertState.seededAgentPubkeys.clear();
      for (const pubkey of applied.nextState.seededAgentPubkeys) {
        alertState.seededAgentPubkeys.add(pubkey);
      }
      alertState.seenNonces.clear();
      for (const nonce of applied.nextState.seenNonces) {
        alertState.seenNonces.add(nonce);
      }

      for (const nonce of applied.dismissNonces) {
        toast.dismiss(nonce);
      }

      if (!update) {
        return;
      }

      const agentName = resolveUserLabel({
        pubkey: update.agentPubkey,
        profiles: profilesQuery.data?.profiles,
        preferResolvedSelfLabel: true,
      });

      for (const alert of applied.alerts) {
        if (shouldSuppressPermissionAlert({
          agentPubkey: update.agentPubkey,
          channelId: alert.channelId,
          openAgentSession: openAgentSessionPubkey,
          openAgentSessionChannel: openAgentSessionChannelId,
          currentChannelId: selectedChannelId,
        })) {
          continue;
        }

        const channel = channels.find((entry) => entry.id === alert.channelId);
        const copy = permissionAlertCopy({
          agentName,
          channelName: channel?.name ?? null,
        });
        const surface = selectPermissionAlertSurface({
          focused: isAppFocused(),
          desktopEnabled: notificationSettings.desktopEnabled,
        });

        void deliverPermissionAlert({
          agentPubkey: update.agentPubkey,
          channelId: alert.channelId,
          copy,
          requestNonce: alert.requestNonce,
          soundEnabled: notificationSettings.slotAlertsEnabled.needs_action,
          surface,
        }, {
          showToast: (alertToast) => {
            toast(alertToast.title, {
              id: alertToast.id,
              description: alertToast.body,
              duration: alertToast.duration,
              onClick: alertToast.onClick,
            });
          },
          sendOsNotification: sendDesktopNotification,
          revealDesktopAppWindow,
          openAgentActivity,
          requestDockBounce,
          playSound: () => {
            playNotificationSound(
              resolveSlotSound(notificationSettings, "needs_action"),
            );
          },
        });
      }
    },
  );

  React.useEffect(() => {
    if (!enabled) {
      return;
    }
    return startPermissionAlertStoreSubscription({
      handleUpdate: handleStoreUpdate,
      subscribe: subscribeAgentObserverStore,
    });
  }, [enabled, ingestionAgents]);
}
```

Import `startPermissionAlertStoreSubscription` with the other Task 1 helpers.
The tested subscription helper makes the update-less current-snapshot call before it attaches the live listener.
The update-less notification from Task 2 covers a hook already mounted when EOSE completes.
Because the controller seeds each ingestion agent only once, a later live store update cannot seed its own nonce from the transcript that was just updated.

- [ ] **Step 7: Verify suppression, copy, surface, and effects in the adapter**

For each `applied.alerts` entry:

1. Call `shouldSuppressPermissionAlert` with `update.agentPubkey`, alert channel, `openAgentSessionPubkey`, `openAgentSessionChannelId`, and `selectedChannelId`.
2. Skip on true.
3. Resolve the channel name from `channels.find((channel) => channel.id === alert.channelId)?.name ?? null`.
4. Resolve the agent name from the batch profiles with `preferResolvedSelfLabel: true`.
5. Build pinned copy with `permissionAlertCopy`.
6. Select the surface with `isAppFocused()` and `notificationSettings.desktopEnabled`.
7. Delegate the concrete effects to `deliverPermissionAlert` exactly as shown in Step 6.

The tested delivery executor does nothing for a null surface.
Do not call `useNotificationSettings` in this hook.
Do not consult mute sets, `notifyWhileViewing`, or huddle silent-channel ids.

- [ ] **Step 8: Mount with live AppShell settings and the main-window gate**

In `AppShell.tsx`, add the import for `usePendingPermissionAlerts`.
Immediately after the existing `useHomeFeedNotifications(identityQuery.data?.pubkey)` call has produced `notificationSettings`, add:

```ts
usePendingPermissionAlerts({
  enabled: !isHuddleRoom,
  notificationSettings: notificationSettings.settings,
});
```

Do not call the hook beside `useAgentObserverIngestion` before settings exist.
Do not call `useNotificationSettings` again.
Do not add a `startupReady` guard.

- [ ] **Step 9: Format the touched desktop files**

Run the repository formatter before the no-write static check so the exact snippets above can be normalized to the repository's line wrapping.

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec biome check --write src/features/agents/pendingPermissionAlert.ts src/features/agents/pendingPermissionAlert.test.mjs src/features/agents/pendingPermissionAlertDelivery.ts src/features/agents/pendingPermissionAlertDelivery.test.mjs src/features/agents/observerRelayStore.ts src/features/agents/observerInitialReplay.test.mjs src/features/agents/useAgentObserverIngestion.ts src/features/agents/usePendingPermissionAlerts.ts src/shared/api/observerRelay.ts src/shared/api/observerRelay.test.mjs src/features/notifications/lib/desktop.ts src/features/notifications/lib/desktop.test.mjs src/app/AppShell.helpers.ts src/app/AppShell.helpers.test.mjs 'src/app/routes/channels.$channelId.tsx' 'src/app/routes/channels.$channelId.test.mjs' src/app/useAppShellDesktopNotifications.ts src/app/AppShell.tsx
```

- [ ] **Step 10: Run focused tests, typecheck, and Biome**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlert.test.mjs src/features/agents/pendingPermissionAlertDelivery.test.mjs src/features/agents/observerInitialReplay.test.mjs src/shared/api/observerRelay.test.mjs src/features/notifications/lib/desktop.test.mjs src/app/AppShell.helpers.test.mjs 'src/app/routes/channels.$channelId.test.mjs' src/features/agents/useAgentObserverIngestion.test.mjs && pnpm typecheck && pnpm exec biome check src/features/agents/pendingPermissionAlert.ts src/features/agents/pendingPermissionAlert.test.mjs src/features/agents/pendingPermissionAlertDelivery.ts src/features/agents/pendingPermissionAlertDelivery.test.mjs src/features/agents/observerRelayStore.ts src/features/agents/observerInitialReplay.test.mjs src/features/agents/useAgentObserverIngestion.ts src/features/agents/usePendingPermissionAlerts.ts src/shared/api/observerRelay.ts src/shared/api/observerRelay.test.mjs src/features/notifications/lib/desktop.ts src/features/notifications/lib/desktop.test.mjs src/app/AppShell.helpers.ts src/app/AppShell.helpers.test.mjs 'src/app/routes/channels.$channelId.tsx' 'src/app/routes/channels.$channelId.test.mjs' src/app/useAppShellDesktopNotifications.ts src/app/AppShell.tsx
```

Expected: PASS with no warnings.

- [ ] **Step 11: Check the AppShell ratchet**

```bash
. ./bin/activate-hermit && wc -l desktop/src/app/AppShell.tsx
```

Expected: no more than 1000 lines.

- [ ] **Step 12: Run staged change detection and commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/agents/pendingPermissionAlertDelivery.ts desktop/src/features/agents/pendingPermissionAlertDelivery.test.mjs desktop/src/features/agents/usePendingPermissionAlerts.ts desktop/src/app/AppShell.tsx && git diff --cached --stat && git diff --cached --name-only && git diff --cached --check
```

Run `detect_changes({ scope: "staged" })` and confirm the new AppShell alert consumer is the only new execution flow.

```bash
. ./bin/activate-hermit && git commit -s -m "feat(desktop): alert owners to agent permission requests"
```

---

### Task 8: Validate the complete desktop slice

**Files:** none unless a validation failure requires a test-first correction.

- [ ] **Step 1: Run all desktop unit tests**

```bash
. ./bin/activate-hermit && just desktop-test
```

Expected: PASS.

- [ ] **Step 2: Run desktop type and static checks**

```bash
. ./bin/activate-hermit && just desktop-typecheck && just desktop-check
```

Expected: PASS.
`just desktop-check` includes Biome, the file-size ratchet, the px-text guard, and the pubkey-truncation guard.

- [ ] **Step 3: Run the existing permission-card E2E regression**

Use the required E2E build mode and keep the existing spec unchanged:

```bash
. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test --project=smoke --grep "permission request stays actionable for a human decision"
```

Expected: PASS.
Do not replace `pnpm build:e2e` with `pnpm build`.

- [ ] **Step 4: Confirm scope and unchanged decision UI**

```bash
. ./bin/activate-hermit && git diff --name-only main -- desktop/src/features/agents/ui/activityRenderClasses/LifecycleActivity.tsx desktop/tests/e2e/observer-feed-screenshots.spec.ts crates mobile web
```

Expected: empty.

```bash
. ./bin/activate-hermit && git diff --stat main -- desktop/src/features/agents/pendingPermissionAlert.ts desktop/src/features/agents/pendingPermissionAlert.test.mjs desktop/src/features/agents/pendingPermissionAlertDelivery.ts desktop/src/features/agents/pendingPermissionAlertDelivery.test.mjs desktop/src/features/agents/observerRelayStore.ts desktop/src/features/agents/observerInitialReplay.test.mjs desktop/src/features/agents/useAgentObserverIngestion.ts desktop/src/features/agents/usePendingPermissionAlerts.ts desktop/src/shared/api/observerRelay.ts desktop/src/shared/api/observerRelay.test.mjs desktop/src/features/notifications/lib/desktop.ts desktop/src/features/notifications/lib/desktop.test.mjs desktop/src/app/AppShell.helpers.ts desktop/src/app/AppShell.helpers.test.mjs 'desktop/src/app/routes/channels.$channelId.tsx' 'desktop/src/app/routes/channels.$channelId.test.mjs' desktop/src/app/useAppShellDesktopNotifications.ts desktop/src/app/AppShell.tsx
```

Expected: only the listed desktop files.

- [ ] **Step 5: Run final GitNexus comparison**

Run `detect_changes({ scope: "compare", base_ref: "main" })`.
Confirm the report covers observer ingestion, permission-alert delivery, desktop notification target parsing, and click routing only.
Stop and investigate any unrelated process.

## Acceptance Criteria

- A new actionable permission request for an owned ingestion agent alerts once per request nonce.
- A focused main AppShell uses a sonner toast even when `desktopEnabled` is false.
- An unfocused main AppShell uses an OS notification only when `desktopEnabled` is true.
- A background app with `desktopEnabled` false produces neither surface.
- The dedicated huddle companion does not duplicate alerts or consume native click actions.
- The main AppShell continues to own permission alerts while a huddle companion exists.
- The alert title uses the resolved agent label and optional normalized `#channel` suffix.
- The alert body contains no ACP options, tool-call ids, or request details.
- Clicking either alert surface reveals the desktop window before opening `openAgentActivity(agentPubkey, { channelId })`.
- A no-channel click uses the existing open-activity fallback.
- An inaccessible explicit channel uses the existing warning and does not invent a destination.
- Grant and deny controls remain only on the existing transcript card.
- The alert is suppressed only when the open pane matches the normalized agent and the approved channel rule.
- `agentSessionChannel` survives channel-route validation and reload so the alternate-channel suppression branch is observable.
- Being in the same channel with the agent pane closed still alerts.
- Different concurrent nonces each alert once.
- Duplicate nonces do not re-alert.
- Initial 300-second observer subscription replay is seeded after EOSE and does not alert.
- Existing reconnect duplicates do not alert.
- A genuinely new frame recovered after a later reconnect remains eligible to alert.
- Updates for agents outside the canonical owner-global ingestion list do not alert.
- `actionable: false`, missing authorization, wrong method, and wrong kind do not alert.
- A same-batch terminal result does not flash an alert.
- Terminal `acp_write`, timeout, cancellation, or a non-actionable follow-up dismisses the toast by nonce.
- Terminal events do not withdraw OS notifications.
- `needs_action` sound plays for a toast or successful OS delivery only when its slot is enabled.
- Channel mute, `notifyWhileViewing`, and huddle silent-channel ids do not suppress the main-window owner approval.
- Notification settings changed during the current app session take effect immediately because the hook receives AppShell's live settings object.
- `parseNotificationTarget` accepts `agentPubkey` without a channel or event.
- Message-author `pubkey` alone remains invalid without a channel or event.
- Event-only join-alert targets still parse and route Home.
- Existing message notification clicks still route through `openSearchHit`.
- No Rust, relay, Home, mobile, web, or decision-card file changes.
- No new Playwright spec.
- `AppShell.tsx` remains within the 1000-line ratchet.

## Validation Commands

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlert.test.mjs src/features/agents/pendingPermissionAlertDelivery.test.mjs src/features/agents/observerInitialReplay.test.mjs src/shared/api/observerRelay.test.mjs src/features/notifications/lib/desktop.test.mjs src/app/AppShell.helpers.test.mjs 'src/app/routes/channels.$channelId.test.mjs' src/features/agents/useAgentObserverIngestion.test.mjs
. ./bin/activate-hermit && just desktop-test
. ./bin/activate-hermit && just desktop-typecheck
. ./bin/activate-hermit && just desktop-check
. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test --project=smoke --grep "permission request stays actionable for a human decision"
```

Do not start Postgres, Redis, or the relay for this desktop-only slice.
Run `just ci` only after these focused gates are green and only when preparing the PR-wide final gate.
