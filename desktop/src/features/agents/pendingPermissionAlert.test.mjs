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
  assert.equal(
    extractActionablePermission(permissionRead({ method: "session/prompt" })),
    null,
  );
  assert.equal(
    extractActionablePermission(permissionRead({ kind: "acp_write" })),
    null,
  );
  assert.equal(
    extractActionablePermission(permissionRead({ actionable: false })),
    null,
  );
  const missingEnvelope = permissionRead();
  delete missingEnvelope.authorization;
  assert.equal(extractActionablePermission(missingEnvelope), null);
});

test("extractResolvedPermissionNonce reads terminal acp_write and non-actionable follow-up", () => {
  assert.equal(extractResolvedPermissionNonce(permissionWrite()), "nonce-1");
  assert.equal(
    extractResolvedPermissionNonce(permissionRead({ actionable: false })),
    "nonce-1",
  );
  assert.equal(extractResolvedPermissionNonce(permissionRead()), null);
});

test("shouldSuppressPermissionAlert requires the same normalized agent and matching channel scope", () => {
  assert.equal(
    shouldSuppressPermissionAlert({
      agentPubkey: AGENT,
      channelId: CHANNEL,
      openAgentSession: AGENT.toUpperCase(),
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
      openAgentSession: OTHER_AGENT,
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

test("permissionAlertCopy uses pinned copy and strips duplicate channel hashes", () => {
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

test("applyPermissionAlertUpdate alerts concurrent unseen nonces once and suppresses same-batch resolution", () => {
  const concurrent = applyPermissionAlertUpdate({
    events: [
      permissionRead({ nonce: "new-1", seq: 1 }),
      permissionRead({ nonce: "new-2", seq: 2 }),
    ],
    seenNonces: new Set(["old-1"]),
  });
  assert.deepEqual(
    concurrent.alerts.map((alert) => alert.requestNonce),
    ["new-1", "new-2"],
  );
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
    update: {
      agentPubkey: AGENT,
      events: [permissionRead({ nonce: "replayed" })],
    },
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
  assert.deepEqual(
    live.alerts.map((alert) => alert.requestNonce),
    ["live"],
  );

  const outsider = applyPermissionAlertStoreNotification({
    state: live.nextState,
    initialReplayComplete: true,
    agents: [{ pubkey: AGENT, transcript: [permissionItem("replayed")] }],
    update: {
      agentPubkey: OTHER_AGENT,
      events: [permissionRead({ nonce: "outsider" })],
    },
  });
  assert.deepEqual(outsider.alerts, []);
  assert.equal(outsider.nextState.seenNonces.has("outsider"), false);
  assert.deepEqual(collectSeenPermissionNonces([]), []);
});

test("selectPermissionAlertSurface keeps focused toast independent of desktopEnabled", () => {
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
