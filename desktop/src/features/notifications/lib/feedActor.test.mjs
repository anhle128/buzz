import assert from "node:assert/strict";
import test from "node:test";
import { getPublicKey } from "nostr-tools/pure";

import { resolveFeedActor } from "./feedActor.ts";

const RELAY_SECRET = new Uint8Array(32).fill(2);
const USER_SECRET = new Uint8Array(32).fill(1);
const RELAY = getPublicKey(RELAY_SECRET);
const USER = getPublicKey(USER_SECRET);
const ATTRIBUTED_USER = "33".repeat(32);
const APP_ID = "6eb31227-8ed2-42ec-9024-863497cbeed2";
const CHANNEL_ID = "36411e44-0e2d-4cfe-bd6e-567eb169db9f";

const APPS = new Map([
  [
    APP_ID,
    {
      appId: APP_ID,
      name: "Archon",
      picture: "https://example.test/archon.png",
      status: "active",
      eventId: "ab".repeat(32),
      relayPubkey: RELAY,
      updatedAt: 1_700_000_000,
    },
  ],
]);

function item(overrides = {}) {
  return {
    id: "cd".repeat(32),
    kind: 9,
    pubkey: RELAY,
    content: "✅ build passed",
    createdAt: 1_700_000_100,
    channelId: CHANNEL_ID,
    channelName: "random",
    channelType: "stream",
    tags: [
      ["h", CHANNEL_ID],
      ["buzz:app", APP_ID],
    ],
    category: "needs_action",
    ...overrides,
  };
}

test("feed items resolve a verified App actor in projection mode", () => {
  const actor = resolveFeedActor({
    item: item(),
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.deepEqual(actor, {
    type: "app",
    appId: APP_ID,
    name: "Archon",
    picture: "https://example.test/archon.png",
    signerPubkey: RELAY,
  });
});

test("feed items skip the unavailable message signature check", () => {
  const actor = resolveFeedActor({
    item: item({ id: "00".repeat(32) }),
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.equal(actor.type, "app");
  assert.equal(actor.appId, APP_ID);
});

test("feed items require the active relay identity", () => {
  const actor = resolveFeedActor({
    item: item(),
    apps: APPS,
  });
  assert.deepEqual(actor, { type: "user", pubkey: RELAY });
});

test("feed items with a p-tag spoof still resolve the App", () => {
  const actor = resolveFeedActor({
    item: item({
      tags: [
        ["p", ATTRIBUTED_USER],
        ["h", CHANNEL_ID],
        ["buzz:app", APP_ID],
      ],
    }),
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.equal(actor.type, "app");
  assert.equal(actor.appId, APP_ID);
});

test("feed items signed by a user fall back without inspecting p tags", () => {
  const actor = resolveFeedActor({
    item: item({
      pubkey: USER,
      tags: [
        ["p", ATTRIBUTED_USER],
        ["h", CHANNEL_ID],
        ["buzz:app", APP_ID],
      ],
    }),
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.deepEqual(actor, { type: "user", pubkey: USER });
});

test("feed items without verified metadata fall back to the signer", () => {
  const actor = resolveFeedActor({
    item: item(),
    apps: new Map(),
    relaySelfPubkey: RELAY,
  });
  assert.deepEqual(actor, { type: "user", pubkey: RELAY });
});
