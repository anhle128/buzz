# Agent permission alerts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When an owned agent emits an actionable `session/request_permission`, Buzz Desktop tells the owner even if they are in another thread, and a click opens the existing process transcript.

**Architecture:** Keep grant/deny on the existing `LifecycleActivity` card.
Subscribe to the already app-wide observer store from a new AppShell hook.
Post a sonner toast when the app is focused, or an OS notification when it is not.
Route either click through `useOpenAgentActivity`.
Do not add a Nostr kind, Home inbox row, grant buttons on the notification, or a `resetCommunityState` singleton.

**Tech Stack:** TypeScript, React 19, sonner, existing Tauri `sendDesktopNotification` / `show_native_notification` JSON target blob, Node `node:test` via `just desktop-test`.

**Spec:** [2026-08-19-agent-permission-alerts-design.md](../specs/2026-08-19-agent-permission-alerts-design.md)

**Product contract:** [VISION.md](../../../VISION.md) keeps Stream at zero notifications for chat and already notifies for workflow approvals.
Agent permission is an owner approval, not a channel message, so this alert is in the same class as those approvals.
[VISION_ACTIVITY.md](../../../VISION_ACTIVITY.md) treats Permission as the control gate and says grant stays on the transcript card.
This plan advances that contract: the owner is fetched to the existing card, and the card's option list is unchanged.

## Global Constraints

- Desktop only.
- Do not change `buzz-acp`, nonce binding, ACP admission, `permission_decision`, or the 300s fail-closed timeout.
- Do not change `LifecycleActivity` / `PermissionDecisionButtons`.
- Do not add grant/deny controls to the toast or OS notification.
- Do not add a new event kind, Home `needs_action` row, or relay persistence.
- Do not implement mobile.
- Do not add a notification-settings slot.
- Do not withdraw OS notifications after a decision.
- Do not notify anyone except the owner.
- Do not apply channel mute or `notifyWhileViewing`.
- Respect `desktopEnabled` on the OS path only.
- Still fire the in-app toast when desktop notifications are off.
- Play `needs_action` sound only when that slot is enabled, on both toast and successful OS paths.
- Do not add a module-level seen-nonce cache or a `resetCommunityState` entry.
- Seen nonces live in React state on the new hook so `AppReady` community remount clears them.
- `AppShell.tsx` is 956 lines and must stay under the 1000-line ratchet.
- Add only the import plus `usePendingPermissionAlerts();` immediately after `useAgentObserverIngestion();`.
- Activate Hermit in every shell: `. ./bin/activate-hermit && …`.
- Shell CWD does not persist across commands.
- No production `unsafe`, `unwrap()`, or `expect()`.
- New public TypeScript APIs get doc comments.
- Use rem-based text tokens if any UI text is added; this slice should not add new visible text nodes outside sonner / OS notification copy.
- Sign every commit with `git commit -s`.
- GitNexus MCP and `.gitnexus/run.cjs` are not available in the planning workspace.
- Before each existing-symbol edit, run `rg -n '<symbol>' desktop/src` and report direct callers.
- If GitNexus later becomes available, run `impact({ target, direction: "upstream" })` before those same edits and stop if risk is HIGH or CRITICAL.
- Before every commit, run `git diff --stat && git diff --name-only && git diff --check` after `git add`.
- If `.gitnexus/run.cjs` exists at commit time, also run `node .gitnexus/run.cjs detect` on the staged scope.

## Resolved Implementation Decisions

- Toast `id` is the authorization `requestNonce`.
- Toast `duration` is `Number.POSITIVE_INFINITY` so the toast lasts until `toast.dismiss(requestNonce)` or the user dismisses it (Open Question 1).
- Notification title is `{agentName} needs permission` or `{agentName} needs permission in #{channel}` when a trimmed channel name is known.
- Channel names in the title never keep a second `#`.
- Notification body is exactly `Open the process log to allow or deny this request.`
- Do not put ACP option names, tool call ids, or the request title in the toast or OS body.
- `extractActionablePermission` still returns the ACP title for tests; the hook must ignore it when posting copy.
- Agent display name is `resolveUserLabel({ pubkey: agentPubkey, profiles, preferResolvedSelfLabel: true })`.
- Empty resolved name falls back to `Agent`.
- `parseNotificationTarget` is exported.
- A target is valid when `channelId`, `eventId`, or `agentPubkey` is a non-empty string.
- Do not reuse `pubkey` as the agent identity; that field remains the message author for `toSearchHit`.
- `resolveDesktopNotificationAction` lives in `AppShell.helpers.ts` next to `toSearchHit`.
- `agentPubkey` wins over search-hit / home / channel routing even when `eventId` is also present.
- Native Linux/macOS code already forwards `target` as `serde_json::Value`; do not change Rust.
- `useAppShellDesktopNotifications` calls `useOpenAgentActivity()` itself so `AppShell.tsx` does not grow a new argument.
- `usePendingPermissionAlerts` is self-contained and takes no props.
- Export `useObserverIngestionAgents()` from `useAgentObserverIngestion.ts` and call it from both ingestion and the alert hook so the owner-global agent list cannot drift.
- Seed seen nonces from `getAgentTranscript(pubkey)` for every ingestion agent whenever that list changes.
- Subscribe with `subscribeAgentObserverStore`.
- Ignore connection-only notifies (`update` missing or `events` empty) for alerting.
- Process a live `update.events` batch in order through `applyPermissionAlertUpdate`.
- If the same batch both creates and resolves a nonce, do not post an alert for that nonce (Open Question 2).
- Suppress only when the open session pane is that agent and that channel, using the spec's matching rule.
- Being in the same channel with the session pane closed still alerts.
- Huddle rooms still alert; do not gate the hook on `isHuddleRoom` (Open Question 4).
- Late OS clicks still call `openAgentActivity`; the card may already show an outcome.
- Inaccessible-channel clicks keep the existing warning toast from `useOpenAgentActivity`.

## File Map

| File | Role |
|------|------|
| Create `desktop/src/features/agents/pendingPermissionAlert.ts` | Pure extract / suppress / copy / seed / batch-apply helpers |
| Create `desktop/src/features/agents/pendingPermissionAlert.test.mjs` | Unit tests for those helpers |
| `desktop/src/features/notifications/lib/desktop.ts` | Optional `agentPubkey` on `DesktopNotificationTarget`; export and relax `parseNotificationTarget` |
| `desktop/src/features/notifications/lib/desktop.test.mjs` | Parse / validity cases; keep the existing constructor-failure test |
| `desktop/src/app/AppShell.helpers.ts` | `resolveDesktopNotificationAction` |
| `desktop/src/app/AppShell.helpers.test.mjs` | Agent-activity vs search-hit vs home routing |
| `desktop/src/app/useAppShellDesktopNotifications.ts` | OS click branch through `openAgentActivity` |
| `desktop/src/features/agents/useAgentObserverIngestion.ts` | Export `useObserverIngestionAgents()` and reuse it |
| Create `desktop/src/features/agents/usePendingPermissionAlerts.ts` | Mount-time seed, store subscription, toast / OS / sound / dismiss |
| `desktop/src/app/AppShell.tsx` | Mount the hook next to observer ingestion |

Do not create other files.
Do not add a Playwright spec.

## Required Impact Checks

Run these before the named task's first production edit.
GitNexus MCP is unavailable in this workspace, so use `rg` unless a later session has GitNexus.

- Task 2: `parseNotificationTarget`, `DesktopNotificationTarget`, `sendDesktopNotification`.
- Task 3: `toSearchHit`, `handleDesktopNotificationAction`, `useAppShellDesktopNotifications`.
- Task 4: `useAgentObserverIngestion`, `combineObserverIngestionAgents`, `subscribeAgentObserverStore`, `getAgentTranscript`, `useOpenAgentActivity`, `AppShell`.
- Direct callers of `parseNotificationTarget` today are only `listenForDesktopNotificationActions` in `desktop.ts`.
- Direct callers of `handleDesktopNotificationAction` are the desktop-notification action listener in `useAppShellDesktopNotifications`.
- `toSearchHit` is only used by that same click path.
- `useAgentObserverIngestion` is mounted only from `AppShell.tsx`.
- Warn and stop if a later GitNexus `impact` report is HIGH or CRITICAL.

## Open Questions

1. **Toast duration.**
   The spec does not say how long the in-app toast stays up.
   **Provisional default:** `duration: Number.POSITIVE_INFINITY`, dismissed by `toast.dismiss(requestNonce)` on resolution or by the user.
   A default 4s sonner toast would recreate the original miss for an owner who is mid-keystroke.

2. **Request and resolution in one observer batch.**
   The spec does not say what to do if `update.events` contains both the actionable `acp_read` and its terminal `acp_write`.
   **Provisional default:** do not post an alert for a nonce that is also dismissed in the same batch.

3. **Unknown agent name.**
   The spec says `{agentName}` but does not define the missing-profile fallback.
   **Provisional default:** `resolveUserLabel` (display name, then nip05, then truncated pubkey), then `Agent` if that string is empty.

4. **Huddle window.**
   Channel notifications are disabled in a huddle room.
   **Provisional default:** permission alerts still fire.
   This is an owner approval, not a channel message.

5. **Spec header status.**
   `docs/superpowers/specs/2026-08-19-agent-permission-alerts-design.md` is still marked `Draft — awaiting user review`.
   **Provisional default:** treat that file as the approved product contract for this plan.

---

### Task 1: Pure permission-alert helpers

**Files:**

- Create: `desktop/src/features/agents/pendingPermissionAlert.ts`
- Create: `desktop/src/features/agents/pendingPermissionAlert.test.mjs`

**Interfaces:**

- Consumes: `ObserverEvent`, `TranscriptItem`, `normalizePubkey`, `asRecord`, `asString`.
- Produces:

```ts
export type ActionablePermissionAlert = {
  requestNonce: string;
  channelId: string | null;
  title: string;
};

export function extractActionablePermission(
  event: ObserverEvent,
): ActionablePermissionAlert | null;

export function extractResolvedPermissionNonce(
  event: ObserverEvent,
): string | null;

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

export function collectSeenPermissionNonces(
  items: readonly TranscriptItem[],
): string[];

export function seedSeenPermissionNonces(
  transcripts: readonly (readonly TranscriptItem[])[],
): Set<string>;

export function applyPermissionAlertUpdate(input: {
  events: readonly ObserverEvent[];
  seenNonces: ReadonlySet<string>;
}): {
  nextSeenNonces: Set<string>;
  alerts: ActionablePermissionAlert[];
  dismissNonces: string[];
};

export function selectPermissionAlertSurface(input: {
  focused: boolean;
  desktopEnabled: boolean;
}): "toast" | "os" | null;
```

- [ ] **Step 1: Write the failing tests**

Create `desktop/src/features/agents/pendingPermissionAlert.test.mjs` with the exact contents below.
Do not create the production module yet.

```js
import assert from "node:assert/strict";
import test from "node:test";

import {
  applyPermissionAlertUpdate,
  collectSeenPermissionNonces,
  extractActionablePermission,
  extractResolvedPermissionNonce,
  permissionAlertCopy,
  seedSeenPermissionNonces,
  selectPermissionAlertSurface,
  shouldSuppressPermissionAlert,
} from "./pendingPermissionAlert.ts";

const AGENT = "a".repeat(64);
const AGENT_UPPER = AGENT.toUpperCase();
const OTHER = "b".repeat(64);
const CHANNEL = "11111111-1111-4111-8111-111111111111";
const OTHER_CHANNEL = "22222222-2222-4222-8222-222222222222";

function permissionRead({
  nonce = "nonce-1",
  actionable = true,
  channelId = CHANNEL,
  method = "session/request_permission",
  title = "Confirm push",
  kind = "acp_read",
} = {}) {
  return {
    seq: 1,
    timestamp: "2026-08-19T10:00:00.000Z",
    kind,
    agentIndex: 0,
    channelId,
    sessionId: "session-1",
    turnId: "turn-1",
    payload: {
      jsonrpc: "2.0",
      id: "req-1",
      method,
      params: { title },
    },
    authorization: { requestNonce: nonce, actionable },
  };
}

test("extractActionablePermission returns nonce, channel, and ACP title", () => {
  const extracted = extractActionablePermission(permissionRead());
  assert.deepEqual(extracted, {
    requestNonce: "nonce-1",
    channelId: CHANNEL,
    title: "Confirm push",
  });
});

test("extractActionablePermission returns null for non-permission and non-actionable frames", () => {
  assert.equal(
    extractActionablePermission(
      permissionRead({ method: "session/prompt" }),
    ),
    null,
  );
  assert.equal(
    extractActionablePermission(permissionRead({ actionable: false })),
    null,
  );
  assert.equal(
    extractActionablePermission(permissionRead({ kind: "acp_write" })),
    null,
  );
  const missingEnvelope = permissionRead();
  delete missingEnvelope.authorization;
  assert.equal(extractActionablePermission(missingEnvelope), null);
});

test("extractResolvedPermissionNonce reads acp_write and non-actionable follow-up", () => {
  assert.equal(
    extractResolvedPermissionNonce(permissionRead({ kind: "acp_write" })),
    "nonce-1",
  );
  assert.equal(
    extractResolvedPermissionNonce(permissionRead({ actionable: false })),
    "nonce-1",
  );
  assert.equal(extractResolvedPermissionNonce(permissionRead()), null);
});

test("shouldSuppressPermissionAlert is true only for that agent and that channel", () => {
  assert.equal(
    shouldSuppressPermissionAlert({
      agentPubkey: AGENT,
      channelId: CHANNEL,
      openAgentSession: AGENT,
      openAgentSessionChannel: CHANNEL,
      currentChannelId: CHANNEL,
    }),
    true,
  );
  assert.equal(
    shouldSuppressPermissionAlert({
      agentPubkey: AGENT,
      channelId: CHANNEL,
      openAgentSession: AGENT_UPPER,
      openAgentSessionChannel: null,
      currentChannelId: CHANNEL,
    }),
    true,
  );
  assert.equal(
    shouldSuppressPermissionAlert({
      agentPubkey: AGENT,
      channelId: null,
      openAgentSession: AGENT,
      openAgentSessionChannel: OTHER_CHANNEL,
      currentChannelId: OTHER_CHANNEL,
    }),
    true,
  );
  assert.equal(
    shouldSuppressPermissionAlert({
      agentPubkey: AGENT,
      channelId: CHANNEL,
      openAgentSession: null,
      openAgentSessionChannel: null,
      currentChannelId: CHANNEL,
    }),
    false,
  );
  assert.equal(
    shouldSuppressPermissionAlert({
      agentPubkey: AGENT,
      channelId: CHANNEL,
      openAgentSession: OTHER,
      openAgentSessionChannel: CHANNEL,
      currentChannelId: CHANNEL,
    }),
    false,
  );
  assert.equal(
    shouldSuppressPermissionAlert({
      agentPubkey: AGENT,
      channelId: CHANNEL,
      openAgentSession: AGENT,
      openAgentSessionChannel: OTHER_CHANNEL,
      currentChannelId: OTHER_CHANNEL,
    }),
    false,
  );
});

test("permissionAlertCopy never lists options", () => {
  assert.deepEqual(permissionAlertCopy({ agentName: "Ada", channelName: "eng" }), {
    title: "Ada needs permission in #eng",
    body: "Open the process log to allow or deny this request.",
  });
  assert.deepEqual(
    permissionAlertCopy({ agentName: "Ada", channelName: "#eng" }),
    {
      title: "Ada needs permission in #eng",
      body: "Open the process log to allow or deny this request.",
    },
  );
  assert.deepEqual(
    permissionAlertCopy({ agentName: "  ", channelName: null }),
    {
      title: "Agent needs permission",
      body: "Open the process log to allow or deny this request.",
    },
  );
});

test("seedSeenPermissionNonces collects existing transcript nonces", () => {
  const seeded = seedSeenPermissionNonces([
    [
      {
        id: "permission:ch:nonce:old-1",
        type: "lifecycle",
        renderClass: "permission",
        title: "Permission requested",
        text: "Confirm push",
        timestamp: "2026-08-19T10:00:00.000Z",
        requestNonce: "old-1",
        actionable: true,
      },
      {
        id: "message:1",
        type: "message",
        renderClass: "message",
        role: "assistant",
        title: "Ada",
        text: "hi",
        timestamp: "2026-08-19T10:00:01.000Z",
      },
    ],
    [
      {
        id: "permission:ch:nonce:old-2",
        type: "lifecycle",
        renderClass: "permission",
        title: "Permission requested",
        text: "done",
        timestamp: "2026-08-19T10:00:02.000Z",
        requestNonce: "old-2",
        actionable: false,
        outcome: "Approved (allow_once)",
      },
    ],
  ]);
  assert.deepEqual([...seeded].sort(), ["old-1", "old-2"]);
  assert.deepEqual(collectSeenPermissionNonces([]), []);
});

test("applyPermissionAlertUpdate skips seen nonces and same-batch resolutions", () => {
  const first = applyPermissionAlertUpdate({
    events: [permissionRead({ nonce: "new-1" })],
    seenNonces: new Set(["old-1"]),
  });
  assert.deepEqual(
    first.alerts.map((alert) => alert.requestNonce),
    ["new-1"],
  );
  assert.equal(first.nextSeenNonces.has("new-1"), true);
  assert.equal(first.nextSeenNonces.has("old-1"), true);

  const replay = applyPermissionAlertUpdate({
    events: [permissionRead({ nonce: "old-1" })],
    seenNonces: new Set(["old-1"]),
  });
  assert.deepEqual(replay.alerts, []);

  const resolved = applyPermissionAlertUpdate({
    events: [
      permissionRead({ nonce: "flash-1" }),
      permissionRead({ nonce: "flash-1", kind: "acp_write" }),
    ],
    seenNonces: new Set(),
  });
  assert.deepEqual(resolved.alerts, []);
  assert.deepEqual(resolved.dismissNonces, ["flash-1"]);
  assert.equal(resolved.nextSeenNonces.has("flash-1"), true);
});

test("selectPermissionAlertSurface splits focus and desktopEnabled", () => {
  assert.equal(
    selectPermissionAlertSurface({ focused: true, desktopEnabled: false }),
    "toast",
  );
  assert.equal(
    selectPermissionAlertSurface({ focused: false, desktopEnabled: true }),
    "os",
  );
  assert.equal(
    selectPermissionAlertSurface({ focused: false, desktopEnabled: false }),
    null,
  );
});
```

- [ ] **Step 2: Run the tests and confirm they fail**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlert.test.mjs
```

Expected: FAIL because `pendingPermissionAlert.ts` does not exist.

- [ ] **Step 3: Implement the helpers**

Create `desktop/src/features/agents/pendingPermissionAlert.ts` with the exact production behavior below.

```ts
import { normalizePubkey } from "@/shared/lib/pubkey";
import type {
  ObserverEvent,
  TranscriptItem,
} from "./ui/agentSessionTypes";
import { asRecord, asString } from "./ui/agentSessionUtils";

export type ActionablePermissionAlert = {
  requestNonce: string;
  channelId: string | null;
  title: string;
};

const PERMISSION_METHOD = "session/request_permission";
const DEFAULT_AGENT_NAME = "Agent";
const ALERT_BODY =
  "Open the process log to allow or deny this request.";

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

function nonEmptyNonce(value: string | undefined): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

/**
 * Return the actionable permission payload from an observer frame, or null.
 */
export function extractActionablePermission(
  event: ObserverEvent,
): ActionablePermissionAlert | null {
  if (event.kind !== "acp_read") {
    return null;
  }
  if (payloadMethod(event.payload) !== PERMISSION_METHOD) {
    return null;
  }
  if (event.authorization?.actionable !== true) {
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

/**
 * Return the nonce of a resolved permission frame, or null.
 */
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

/**
 * True when the owner is already looking at this agent's session pane
 * for this request's channel.
 */
export function shouldSuppressPermissionAlert(input: {
  agentPubkey: string;
  channelId: string | null;
  openAgentSession: string | null | undefined;
  openAgentSessionChannel: string | null | undefined;
  currentChannelId: string | null | undefined;
}): boolean {
  if (!input.openAgentSession) {
    return false;
  }
  if (
    normalizePubkey(input.openAgentSession) !==
    normalizePubkey(input.agentPubkey)
  ) {
    return false;
  }
  if (input.channelId == null) {
    return true;
  }
  return (
    input.channelId === input.currentChannelId ||
    input.channelId === input.openAgentSessionChannel
  );
}

/**
 * Build in-app / OS copy for an owner permission alert.
 */
export function permissionAlertCopy(input: {
  agentName: string;
  channelName: string | null | undefined;
}): { title: string; body: string } {
  const agentName = input.agentName.trim() || DEFAULT_AGENT_NAME;
  const rawChannel = input.channelName?.trim() ?? "";
  const channelName = rawChannel.replace(/^#+/, "");
  return {
    title: channelName
      ? `${agentName} needs permission in #${channelName}`
      : `${agentName} needs permission`,
    body: ALERT_BODY,
  };
}

/**
 * Collect request nonces already present on a transcript.
 */
export function collectSeenPermissionNonces(
  items: readonly TranscriptItem[],
): string[] {
  const nonces: string[] = [];
  for (const item of items) {
    if ("requestNonce" in item && typeof item.requestNonce === "string") {
      const nonce = nonEmptyNonce(item.requestNonce);
      if (nonce) {
        nonces.push(nonce);
      }
    }
  }
  return nonces;
}

/**
 * Seed the seen-nonce set from current transcripts so reconnect snapshots
 * do not alert.
 */
export function seedSeenPermissionNonces(
  transcripts: readonly (readonly TranscriptItem[])[],
): Set<string> {
  const seen = new Set<string>();
  for (const items of transcripts) {
    for (const nonce of collectSeenPermissionNonces(items)) {
      seen.add(nonce);
    }
  }
  return seen;
}

/**
 * Fold a live observer batch into new alerts and toast dismissals.
 */
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
    const extracted = extractActionablePermission(event);
    if (!extracted) {
      continue;
    }
    if (nextSeenNonces.has(extracted.requestNonce)) {
      continue;
    }
    nextSeenNonces.add(extracted.requestNonce);
    alerts.push(extracted);
  }

  return {
    nextSeenNonces,
    alerts: alerts.filter((alert) => !dismissSet.has(alert.requestNonce)),
    dismissNonces,
  };
}

/**
 * Choose the focused toast path, the background OS path, or silence.
 */
export function selectPermissionAlertSurface(input: {
  focused: boolean;
  desktopEnabled: boolean;
}): "toast" | "os" | null {
  if (input.focused) {
    return "toast";
  }
  if (input.desktopEnabled) {
    return "os";
  }
  return null;
}
```

- [ ] **Step 4: Run the tests and confirm they pass**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlert.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Refactor if needed and re-run the same command**

Do not change the pinned copy strings.
Do not fold option lists into the body.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/agents/pendingPermissionAlert.ts desktop/src/features/agents/pendingPermissionAlert.test.mjs && git diff --stat && git diff --name-only && git diff --check && git commit -s -m "$(cat <<'EOF'
test(desktop): add agent permission alert helpers

Pure extract, suppress, copy, seed, and batch-apply helpers for owner
permission toasts. No hook wiring yet.
EOF
)"
```

---

### Task 2: Accept `agentPubkey` on desktop notification targets

**Files:**

- Modify: `desktop/src/features/notifications/lib/desktop.ts`
- Modify: `desktop/src/features/notifications/lib/desktop.test.mjs`

**Interfaces:**

- Change `DesktopNotificationTarget` to:

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
```

- Export `parseNotificationTarget(value: unknown): DesktopNotificationTarget | null`.
- A target is valid when `channelId`, `eventId`, or `agentPubkey` is a non-empty string.
- Empty-string `agentPubkey` does not count.
- `pubkey` without `agentPubkey` still requires `channelId` or `eventId`.
- Return `agentPubkey` only when it is a non-empty string.

- [ ] **Step 1: Run the impact grep**

```bash
. ./bin/activate-hermit && rg -n "parseNotificationTarget|DesktopNotificationTarget" desktop/src desktop/src-tauri
```

Confirm Rust still takes `Option<serde_json::Value>` and does not need a schema change.
If a later GitNexus impact on `parseNotificationTarget` is HIGH or CRITICAL, stop.

- [ ] **Step 2: Write the failing parse tests**

Keep the existing constructor-failure test in `desktop/src/features/notifications/lib/desktop.test.mjs`.
Change the dynamic import to also bind `parseNotificationTarget`.
Append these tests after the existing one:

```js
test("parseNotificationTarget accepts agentPubkey without channel or event", () => {
  const target = parseNotificationTarget({
    agentPubkey: "aa".repeat(32),
    channelId: null,
    eventId: null,
    kind: null,
  });
  assert.equal(target?.agentPubkey, "aa".repeat(32));
  assert.equal(target?.channelId, null);
  assert.equal(target?.eventId, null);
});

test("parseNotificationTarget still requires channel or event when only pubkey is set", () => {
  assert.equal(
    parseNotificationTarget({
      pubkey: "aa".repeat(32),
      channelId: null,
      eventId: null,
      kind: 1,
    }),
    null,
  );
});

test("parseNotificationTarget still accepts eventId-only join-alert targets", () => {
  const target = parseNotificationTarget({
    channelId: null,
    eventId: "ab".repeat(32),
    kind: 8000,
  });
  assert.equal(target?.eventId, "ab".repeat(32));
  assert.equal(target?.agentPubkey, undefined);
});

test("parseNotificationTarget rejects empty agentPubkey without other anchors", () => {
  assert.equal(
    parseNotificationTarget({
      agentPubkey: "",
      channelId: null,
      eventId: null,
      kind: null,
    }),
    null,
  );
});
```

- [ ] **Step 3: Run the tests and confirm the new cases fail**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/notifications/lib/desktop.test.mjs
```

Expected: FAIL on `parseNotificationTarget is not a function` or on the agentPubkey-only assertion.

- [ ] **Step 4: Implement the type and parse changes**

In `desktop.ts`:

1. Add `agentPubkey?: string` to `DesktopNotificationTarget`.
2. Change `function parseNotificationTarget` to `export function parseNotificationTarget`.
3. Parse `agentPubkey` as a non-empty string or `undefined`.
4. Replace `if (!channelId && !eventId) return null;` with `if (!channelId && !eventId && !agentPubkey) return null;`.
5. Include `agentPubkey` on the returned object.

Add this doc comment on the exported parser:

```ts
/**
 * Parse a native or in-process desktop-notification click target.
 * A target is valid when it names a channel, an event, or an agent session.
 */
```

- [ ] **Step 5: Re-run the desktop notification tests**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/notifications/lib/desktop.test.mjs
```

Expected: PASS, including the original constructor-failure test.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/notifications/lib/desktop.ts desktop/src/features/notifications/lib/desktop.test.mjs && git diff --stat && git diff --name-only && git diff --check && git commit -s -m "$(cat <<'EOF'
fix(desktop): accept agentPubkey notification targets

Permission-alert clicks can name an agent session without a channel or
message event. Join-alert eventId-only targets still parse.
EOF
)"
```

---

### Task 3: Route OS notification clicks to the process transcript

**Files:**

- Modify: `desktop/src/app/AppShell.helpers.ts`
- Modify: `desktop/src/app/AppShell.helpers.test.mjs`
- Modify: `desktop/src/app/useAppShellDesktopNotifications.ts`

**Interfaces:**

```ts
export type DesktopNotificationAction =
  | {
      type: "agent-activity";
      agentPubkey: string;
      channelId: string | null;
    }
  | { type: "home" }
  | { type: "channel"; channelId: string }
  | { type: "search-hit"; hit: SearchHit };

export function resolveDesktopNotificationAction(
  target: DesktopNotificationTarget,
): DesktopNotificationAction;
```

Routing order:

1. Non-empty `target.agentPubkey` → `{ type: "agent-activity", agentPubkey, channelId: target.channelId }`.
2. Else missing `channelId` → `{ type: "home" }`.
3. Else `toSearchHit(target)` → `{ type: "search-hit", hit }`.
4. Else `{ type: "channel", channelId: target.channelId }`.

`handleDesktopNotificationAction` must:

1. `await revealDesktopAppWindow()`.
2. Switch on `resolveDesktopNotificationAction(target)`.
3. On `agent-activity`, call `openAgentActivity(agentPubkey, { channelId })` and return.
4. On `home`, `void goHome()`.
5. On `channel`, `await goChannel(channelId)`.
6. On `search-hit`, `await openSearchHit(hit)`.

Call `useOpenAgentActivity()` inside `useAppShellDesktopNotifications`.
Do not add parameters to the hook.
Do not call `openSearchHit` when `agentPubkey` is set.

- [ ] **Step 1: Run the impact grep**

```bash
. ./bin/activate-hermit && rg -n "toSearchHit|handleDesktopNotificationAction|useAppShellDesktopNotifications" desktop/src
```

Stop if a later GitNexus impact on `handleDesktopNotificationAction` is HIGH or CRITICAL.

- [ ] **Step 2: Write the failing routing tests**

Append these tests to `desktop/src/app/AppShell.helpers.test.mjs`.
Add `resolveDesktopNotificationAction` to the import list.

```js
import { resolveDesktopNotificationAction } from "./AppShell.helpers.ts";

test("resolveDesktopNotificationAction prefers agentPubkey over a search hit", () => {
  const action = resolveDesktopNotificationAction({
    agentPubkey: "aa".repeat(32),
    channelId: "channel-1",
    content: "ignored",
    createdAt: 1,
    eventId: "ab".repeat(32),
    kind: 9,
    pubkey: "cc".repeat(32),
    threadRootId: null,
  });
  assert.deepEqual(action, {
    type: "agent-activity",
    agentPubkey: "aa".repeat(32),
    channelId: "channel-1",
  });
});

test("resolveDesktopNotificationAction opens activity with a null channel", () => {
  const action = resolveDesktopNotificationAction({
    agentPubkey: "aa".repeat(32),
    channelId: null,
    eventId: null,
    kind: null,
  });
  assert.deepEqual(action, {
    type: "agent-activity",
    agentPubkey: "aa".repeat(32),
    channelId: null,
  });
});

test("resolveDesktopNotificationAction keeps eventId-only targets on home", () => {
  const action = resolveDesktopNotificationAction({
    channelId: null,
    eventId: "ab".repeat(32),
    kind: 8000,
    pubkey: "cc".repeat(32),
  });
  assert.deepEqual(action, { type: "home" });
});

test("resolveDesktopNotificationAction still builds a search hit without agentPubkey", () => {
  const action = resolveDesktopNotificationAction({
    channelId: "channel-1",
    content: "hello",
    createdAt: 42,
    eventId: "ab".repeat(32),
    kind: 9,
    pubkey: "cc".repeat(32),
    threadRootId: "cd".repeat(32),
  });
  assert.equal(action.type, "search-hit");
  if (action.type === "search-hit") {
    assert.equal(action.hit.eventId, "ab".repeat(32));
    assert.equal(action.hit.channelId, "channel-1");
    assert.equal(action.hit.pubkey, "cc".repeat(32));
  }
});
```

- [ ] **Step 3: Run the helper tests and confirm the new cases fail**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/app/AppShell.helpers.test.mjs
```

Expected: FAIL because `resolveDesktopNotificationAction` is not exported.

- [ ] **Step 4: Implement the helper and the click branch**

Add this to `AppShell.helpers.ts` after `toSearchHit`:

```ts
export type DesktopNotificationAction =
  | {
      type: "agent-activity";
      agentPubkey: string;
      channelId: string | null;
    }
  | { type: "home" }
  | { type: "channel"; channelId: string }
  | { type: "search-hit"; hit: SearchHit };

/**
 * Decide where a desktop-notification click should land.
 * Agent session targets never go through search-hit routing.
 */
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
  if (!hit) {
    return { type: "channel", channelId: target.channelId };
  }
  return { type: "search-hit", hit };
}
```

In `useAppShellDesktopNotifications.ts`:

1. Import `useOpenAgentActivity` from `@/features/agents/useOpenAgentActivity`.
2. Import `resolveDesktopNotificationAction` from `@/app/AppShell.helpers`.
3. Call `const { openAgentActivity } = useOpenAgentActivity();` at the top of the hook.
4. Replace the body of `handleDesktopNotificationAction` with:

```ts
await revealDesktopAppWindow();
const action = resolveDesktopNotificationAction(target);
if (action.type === "agent-activity") {
  openAgentActivity(action.agentPubkey, { channelId: action.channelId });
  return;
}
if (action.type === "home") {
  void goHome();
  return;
}
if (action.type === "channel") {
  await goChannel(action.channelId);
  return;
}
await openSearchHit(action.hit);
```

Do not keep the old `if (!target.channelId) goHome()` path in front of the agent branch.

- [ ] **Step 5: Re-run helper tests**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/app/AppShell.helpers.test.mjs
```

Expected: PASS.

- [ ] **Step 6: Typecheck the desktop app**

```bash
. ./bin/activate-hermit && cd desktop && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/app/AppShell.helpers.ts desktop/src/app/AppShell.helpers.test.mjs desktop/src/app/useAppShellDesktopNotifications.ts && git diff --stat && git diff --name-only && git diff --check && git commit -s -m "$(cat <<'EOF'
feat(desktop): open agent transcripts from notification clicks

OS permission-alert targets carry agentPubkey and skip search-hit
routing. Existing message and join-alert clicks stay unchanged.
EOF
)"
```

---

### Task 4: Mount the pending-permission alert hook

**Files:**

- Modify: `desktop/src/features/agents/useAgentObserverIngestion.ts`
- Create: `desktop/src/features/agents/usePendingPermissionAlerts.ts`
- Modify: `desktop/src/app/AppShell.tsx`

**Interfaces:**

```ts
export function useObserverIngestionAgents(): Array<
  Pick<ManagedAgent, "pubkey" | "status">
>;

export function usePendingPermissionAlerts(): void;
```

`useAgentObserverIngestion` must call `useObserverIngestionAgents()` instead of inlining the same query graph.
`usePendingPermissionAlerts` must use that same list for seed and name lookup.

Hook behavior:

1. Hold `const [seenNonces] = React.useState(() => new Set<string>());`.
2. On `ingestionAgents` change, add `collectSeenPermissionNonces(getAgentTranscript(agent.pubkey))` into `seenNonces`.
3. Subscribe to `subscribeAgentObserverStore` for the hook lifetime.
4. On `update` with events, run `applyPermissionAlertUpdate` and copy returned nonces into `seenNonces`.
5. `toast.dismiss(nonce)` for each `dismissNonces` entry.
6. For each remaining alert, skip when `shouldSuppressPermissionAlert` is true using `useChannelPanelHistoryState()` plus `deriveShellRoute(useLocation().pathname).selectedChannelId`.
7. Build copy with `permissionAlertCopy` and `resolveUserLabel`.
8. `selectPermissionAlertSurface({ focused: isAppFocused(), desktopEnabled: settings.desktopEnabled })`.
9. Toast path:

```ts
toast(copy.title, {
  id: alert.requestNonce,
  description: copy.body,
  duration: Number.POSITIVE_INFINITY,
  onClick: () => {
    void revealDesktopAppWindow();
    openAgentActivity(update.agentPubkey, { channelId: alert.channelId });
  },
});
```

10. OS path:

```ts
void sendDesktopNotification({
  title: copy.title,
  body: copy.body,
  target: {
    agentPubkey: update.agentPubkey,
    channelId: alert.channelId,
    eventId: null,
    kind: null,
  },
}).then((didSend) => {
  if (!didSend) return;
  void requestDockBounce();
  if (settings.slotAlertsEnabled.needs_action) {
    playNotificationSound(resolveSlotSound(settings, "needs_action"));
  }
});
```

11. Toast path plays the same sound immediately when `settings.slotAlertsEnabled.needs_action` is true.
12. Do not consult mute sets, huddle silent channels, or `notifyWhileViewing`.
13. Read current `agentSession` / `agentSessionChannel` from `useChannelPanelHistoryState()`.
14. Mount in `AppShell` immediately after `useAgentObserverIngestion();` with no readiness guard.

`AppShell.tsx` is 956 lines.
The only allowed production edits there are one import and one hook call.

- [ ] **Step 1: Run the impact grep**

```bash
. ./bin/activate-hermit && rg -n "useAgentObserverIngestion|combineObserverIngestionAgents|subscribeAgentObserverStore|getAgentTranscript|useOpenAgentActivity" desktop/src
```

Report that `useAgentObserverIngestion` has one production caller (`AppShell`).
Stop if a later GitNexus impact is HIGH or CRITICAL.

- [ ] **Step 2: Extract `useObserverIngestionAgents` first**

In `useAgentObserverIngestion.ts`, move the identity / managed / relay / profile query graph into:

```ts
export function useObserverIngestionAgents(): IngestionAgent[] {
  // existing query + combineObserverIngestionAgents body
}
```

Keep `useAgentObserverIngestion` as:

```ts
export function useAgentObserverIngestion() {
  const ingestionAgents = useObserverIngestionAgents();
  useManagedAgentObserverBridge(ingestionAgents);
  useActiveAgentTurnsBridge(ingestionAgents);
}
```

Do not change the exported combine helper.
Do not add a readiness guard.

- [ ] **Step 3: Write the hook**

Create `desktop/src/features/agents/usePendingPermissionAlerts.ts`:

```ts
import * as React from "react";
import { useLocation } from "@tanstack/react-router";
import { toast } from "sonner";

import { deriveShellRoute } from "@/app/AppShell.helpers";
import { useChannelPanelHistoryState } from "@/features/channels/ui/useChannelPanelHistoryState";
import { useChannelsQuery } from "@/features/channels/hooks";
import { useNotificationSettings } from "@/features/notifications/hooks";
import {
  requestDockBounce,
  revealDesktopAppWindow,
  sendDesktopNotification,
} from "@/features/notifications/lib/desktop";
import {
  playNotificationSound,
  resolveSlotSound,
} from "@/features/notifications/lib/sound";
import { resolveUserLabel } from "@/features/profile/lib/identity";
import { useUsersBatchQuery } from "@/features/profile/hooks";
import { useIdentityQuery } from "@/shared/api/hooks";
import { isAppFocused } from "@/shared/lib/useDocumentVisible";

import {
  getAgentTranscript,
  subscribeAgentObserverStore,
  type AgentObserverStoreUpdate,
} from "./observerRelayStore";
import {
  applyPermissionAlertUpdate,
  collectSeenPermissionNonces,
  permissionAlertCopy,
  selectPermissionAlertSurface,
  shouldSuppressPermissionAlert,
} from "./pendingPermissionAlert";
import { useObserverIngestionAgents } from "./useAgentObserverIngestion";
import { useOpenAgentActivity } from "./useOpenAgentActivity";

/**
 * App-level owner permission alerts.
 * Mount once in AppShell next to observer ingestion.
 */
export function usePendingPermissionAlerts(): void {
  const identityQuery = useIdentityQuery();
  const currentPubkey = identityQuery.data?.pubkey;
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
  const notificationSettings = useNotificationSettings(currentPubkey);
  const { openAgentActivity } = useOpenAgentActivity();
  const location = useLocation();
  const { selectedChannelId } = deriveShellRoute(location.pathname);
  const { openAgentSessionPubkey, openAgentSessionChannelId } =
    useChannelPanelHistoryState();
  const [seenNonces] = React.useState(() => new Set<string>());

  React.useEffect(() => {
    for (const agent of ingestionAgents) {
      for (const nonce of collectSeenPermissionNonces(
        getAgentTranscript(agent.pubkey),
      )) {
        seenNonces.add(nonce);
      }
    }
  }, [ingestionAgents, seenNonces]);

  const handleStoreUpdate = React.useEffectEvent(
    (update?: AgentObserverStoreUpdate) => {
      if (!update || update.events.length === 0) {
        return;
      }
      const applied = applyPermissionAlertUpdate({
        events: update.events,
        seenNonces,
      });
      for (const nonce of applied.nextSeenNonces) {
        seenNonces.add(nonce);
      }
      for (const nonce of applied.dismissNonces) {
        toast.dismiss(nonce);
      }

      const settings = notificationSettings.settings;
      const profiles = profilesQuery.data?.profiles;
      const agentName = resolveUserLabel({
        pubkey: update.agentPubkey,
        profiles,
        preferResolvedSelfLabel: true,
      });

      for (const alert of applied.alerts) {
        if (
          shouldSuppressPermissionAlert({
            agentPubkey: update.agentPubkey,
            channelId: alert.channelId,
            openAgentSession: openAgentSessionPubkey,
            openAgentSessionChannel: openAgentSessionChannelId,
            currentChannelId: selectedChannelId,
          })
        ) {
          continue;
        }

        const channel = channels.find(
          (entry) => entry.id === alert.channelId,
        );
        const copy = permissionAlertCopy({
          agentName,
          channelName: channel?.name ?? null,
        });
        const surface = selectPermissionAlertSurface({
          focused: isAppFocused(),
          desktopEnabled: settings.desktopEnabled,
        });
        if (surface === "toast") {
          toast(copy.title, {
            id: alert.requestNonce,
            description: copy.body,
            duration: Number.POSITIVE_INFINITY,
            onClick: () => {
              void revealDesktopAppWindow();
              openAgentActivity(update.agentPubkey, {
                channelId: alert.channelId,
              });
            },
          });
          if (settings.slotAlertsEnabled.needs_action) {
            playNotificationSound(
              resolveSlotSound(settings, "needs_action"),
            );
          }
          continue;
        }
        if (surface !== "os") {
          continue;
        }
        void sendDesktopNotification({
          title: copy.title,
          body: copy.body,
          target: {
            agentPubkey: update.agentPubkey,
            channelId: alert.channelId,
            eventId: null,
            kind: null,
          },
        }).then((didSend) => {
          if (!didSend) {
            return;
          }
          void requestDockBounce();
          if (settings.slotAlertsEnabled.needs_action) {
            playNotificationSound(
              resolveSlotSound(settings, "needs_action"),
            );
          }
        });
      }
    },
  );

  React.useEffect(() => {
    return subscribeAgentObserverStore(handleStoreUpdate);
  }, [handleStoreUpdate]);
}
```

`normalizePubkey` is not needed in the hook; `shouldSuppressPermissionAlert` already normalizes.

If `AgentObserverStoreUpdate` is not already exported from `observerRelayStore.ts`, export the existing type instead of duplicating it.
It is already exported.

- [ ] **Step 4: Mount the hook in AppShell**

In `desktop/src/app/AppShell.tsx` add:

```ts
import { usePendingPermissionAlerts } from "@/features/agents/usePendingPermissionAlerts";
```

Immediately after `useAgentObserverIngestion();` add:

```ts
  usePendingPermissionAlerts();
```

Do not wrap it in `startupReady`.
Do not pass props.
Confirm `wc -l desktop/src/app/AppShell.tsx` is still `<= 1000`.

- [ ] **Step 5: Run the helper tests plus typecheck**

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlert.test.mjs src/app/AppShell.helpers.test.mjs src/features/notifications/lib/desktop.test.mjs && pnpm typecheck && pnpm exec biome check src/features/agents/pendingPermissionAlert.ts src/features/agents/pendingPermissionAlert.test.mjs src/features/agents/usePendingPermissionAlerts.ts src/features/agents/useAgentObserverIngestion.ts src/app/AppShell.tsx src/app/AppShell.helpers.ts src/app/useAppShellDesktopNotifications.ts src/features/notifications/lib/desktop.ts src/features/notifications/lib/desktop.test.mjs
```

Expected: PASS.
If biome wants import-order or unused-import fixes, apply them and re-run.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/agents/usePendingPermissionAlerts.ts desktop/src/features/agents/useAgentObserverIngestion.ts desktop/src/app/AppShell.tsx && git diff --stat && git diff --name-only && git diff --check && git commit -s -m "$(cat <<'EOF'
feat(desktop): alert owners about pending agent permissions

Subscribe to the observer store from AppShell and toast or OS-notify
when a new actionable request arrives outside the open session pane.
EOF
)"
```

---

### Task 5: Full desktop validation

**Files:** none, unless a check fails.

- [ ] **Step 1: Run the desktop unit suite**

```bash
. ./bin/activate-hermit && just desktop-test
```

Expected: PASS.

- [ ] **Step 2: Run desktop typecheck and static checks**

```bash
. ./bin/activate-hermit && just desktop-typecheck && just desktop-check
```

Expected: PASS.
`just desktop-check` includes the 1000-line ratchet and `pnpm check:px-text`.

- [ ] **Step 3: Confirm the existing observer permission e2e spec is untouched**

```bash
. ./bin/activate-hermit && git diff --name-only -- desktop/tests/e2e/observer-feed-screenshots.spec.ts desktop/src/features/agents/ui/activityRenderClasses/LifecycleActivity.tsx
```

Expected: empty.

- [ ] **Step 4: Confirm file-size and scope**

```bash
. ./bin/activate-hermit && wc -l desktop/src/app/AppShell.tsx desktop/src/app/useAppShellDesktopNotifications.ts desktop/src/features/notifications/lib/desktop.ts desktop/src/features/agents/usePendingPermissionAlerts.ts && git diff --stat origin/main -- desktop/src/features/agents/pendingPermissionAlert.ts desktop/src/features/agents/pendingPermissionAlert.test.mjs desktop/src/features/agents/usePendingPermissionAlerts.ts desktop/src/features/agents/useAgentObserverIngestion.ts desktop/src/features/notifications/lib/desktop.ts desktop/src/features/notifications/lib/desktop.test.mjs desktop/src/app/AppShell.helpers.ts desktop/src/app/AppShell.helpers.test.mjs desktop/src/app/useAppShellDesktopNotifications.ts desktop/src/app/AppShell.tsx
```

Expected: `AppShell.tsx` `<= 1000`.
Expected: no mobile, relay, CLI, or Home-feed files.

If `origin/main` is unavailable, use `git diff --stat HEAD~3` or the branch base instead.

## Acceptance Criteria

- An owner who is not already on that agent-and-channel session pane gets a sonner toast when Buzz is focused and an OS notification when it is not.
- Clicking either surface reveals the window and opens the existing process transcript via `openAgentActivity(agentPubkey, { channelId })`.
- Grant/deny still happen only on the existing transcript card.
- Matching open pane suppresses the alert.
- Same channel with the pane closed still alerts.
- Concurrent requests with different nonces each alert once.
- Replayed / already-seeded nonces do not alert.
- `actionable: false` and missing envelopes do not alert.
- App focused + `desktopEnabled: false` still toasts.
- App background + `desktopEnabled: false` does not toast and does not OS-notify.
- `needs_action` sound plays for toast and successful OS delivery only when that slot is enabled.
- Channel mute and `notifyWhileViewing` do not suppress the alert.
- Resolution / timeout / cancel dismisses the toast by nonce and leaves any OS notification alone.
- `parseNotificationTarget({ agentPubkey })` is valid.
- `parseNotificationTarget({ pubkey })` without channel or event is still null.
- Join-alert `{ eventId }` targets still parse and still route home.
- No new Playwright spec.
- No new event kind, Home row, mobile change, or `resetCommunityState` entry.

## Validation Commands

```bash
. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/agents/pendingPermissionAlert.test.mjs src/app/AppShell.helpers.test.mjs src/features/notifications/lib/desktop.test.mjs
. ./bin/activate-hermit && just desktop-test
. ./bin/activate-hermit && just desktop-typecheck
. ./bin/activate-hermit && just desktop-check
```

Do not run `just ci` unless those desktop gates are already green and the implementer is opening a PR.
Do not start Postgres, Redis, or the relay for this slice.
