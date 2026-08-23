import assert from "node:assert/strict";
import test from "node:test";
import { finalizeEvent, getPublicKey } from "nostr-tools/pure";

import { resolveAppActor } from "./appActor.ts";

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
      name: "Buildkite",
      picture: "https://example.test/icon.png",
      status: "disabled",
      eventId: "ab".repeat(32),
      relayPubkey: RELAY,
      updatedAt: 1_700_000_000,
    },
  ],
]);

function flipHexNibble(hex) {
  const last = hex.at(-1);
  return `${hex.slice(0, -1)}${last === "0" ? "1" : "0"}`;
}

function signMessage({
  secret = RELAY_SECRET,
  kind = 9,
  tags = [
    ["h", CHANNEL_ID],
    ["buzz:app", APP_ID],
  ],
  content = "✅ build passed",
  createdAt = 1_700_000_100,
} = {}) {
  return finalizeEvent(
    {
      kind,
      created_at: createdAt,
      content,
      tags,
    },
    secret,
  );
}

test("valid live App message resolves the App actor", () => {
  const event = signMessage();
  const actor = resolveAppActor({
    event,
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.deepEqual(actor, {
    type: "app",
    appId: APP_ID,
    name: "Buildkite",
    picture: "https://example.test/icon.png",
    signerPubkey: RELAY,
  });
});

test("p-tag spoofing cannot replace a valid App actor", () => {
  const event = signMessage({
    tags: [
      ["p", ATTRIBUTED_USER],
      ["h", CHANNEL_ID],
      ["buzz:app", APP_ID],
    ],
  });
  const actor = resolveAppActor({
    event,
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.equal(actor.type, "app");
  assert.equal(actor.appId, APP_ID);
  assert.notEqual(actor.signerPubkey, ATTRIBUTED_USER);
});

test("wrong message signer falls back without inspecting p tags", () => {
  const event = signMessage({
    secret: USER_SECRET,
    tags: [
      ["p", ATTRIBUTED_USER],
      ["h", CHANNEL_ID],
      ["buzz:app", APP_ID],
    ],
  });
  const actor = resolveAppActor({
    event,
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.deepEqual(actor, { type: "user", pubkey: USER });
});

test("invalid live message signature falls back to the signer", () => {
  const signed = signMessage({
    tags: [
      ["p", ATTRIBUTED_USER],
      ["h", CHANNEL_ID],
      ["buzz:app", APP_ID],
    ],
  });
  const event = { ...JSON.parse(JSON.stringify(signed)), content: "tampered" };
  const actor = resolveAppActor({
    event,
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.deepEqual(actor, { type: "user", pubkey: RELAY });
});

test("duplicate buzz:app falls back without inspecting p tags", () => {
  const event = signMessage({
    tags: [
      ["p", ATTRIBUTED_USER],
      ["h", CHANNEL_ID],
      ["buzz:app", APP_ID],
      ["buzz:app", APP_ID],
    ],
  });
  const actor = resolveAppActor({
    event,
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.deepEqual(actor, { type: "user", pubkey: RELAY });
});

test("missing metadata falls back without inspecting p tags", () => {
  const event = signMessage({
    tags: [
      ["p", ATTRIBUTED_USER],
      ["h", CHANNEL_ID],
      ["buzz:app", "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"],
    ],
  });
  const actor = resolveAppActor({
    event,
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.deepEqual(actor, { type: "user", pubkey: RELAY });
});

test("non-kind-9 App marker falls back to its signer without inspecting p tags", () => {
  const event = signMessage({
    kind: 40002,
    tags: [
      ["p", ATTRIBUTED_USER],
      ["h", CHANNEL_ID],
      ["buzz:app", APP_ID],
    ],
  });
  const actor = resolveAppActor({
    event,
    apps: APPS,
    relaySelfPubkey: RELAY,
  });
  assert.deepEqual(actor, { type: "user", pubkey: RELAY });
});

test("metadata keyed under the wrong App UUID falls back to the signer", () => {
  const event = signMessage({
    tags: [
      ["p", ATTRIBUTED_USER],
      ["h", CHANNEL_ID],
      ["buzz:app", APP_ID],
    ],
  });
  const apps = new Map([
    [
      APP_ID,
      { ...APPS.get(APP_ID), appId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa" },
    ],
  ]);
  const actor = resolveAppActor({ event, apps, relaySelfPubkey: RELAY });
  assert.deepEqual(actor, { type: "user", pubkey: RELAY });
});

test("metadata from another relay falls back to the signer", () => {
  const event = signMessage({
    tags: [
      ["p", ATTRIBUTED_USER],
      ["h", CHANNEL_ID],
      ["buzz:app", APP_ID],
    ],
  });
  const apps = new Map([[APP_ID, { ...APPS.get(APP_ID), relayPubkey: USER }]]);
  const actor = resolveAppActor({ event, apps, relaySelfPubkey: RELAY });
  assert.deepEqual(actor, { type: "user", pubkey: RELAY });
});

test("search and feed modes skip only the unavailable signature check", () => {
  const signed = signMessage();
  const event = JSON.parse(JSON.stringify(signed));
  event.sig = flipHexNibble(event.sig);
  const live = resolveAppActor({
    event,
    apps: APPS,
    relaySelfPubkey: RELAY,
    mode: "live",
  });
  assert.deepEqual(live, { type: "user", pubkey: RELAY });

  for (const mode of ["search", "feed"]) {
    const projected = resolveAppActor({
      event,
      apps: APPS,
      relaySelfPubkey: RELAY,
      mode,
    });
    assert.equal(projected.type, "app", mode);
    assert.equal(projected.appId, APP_ID, mode);
  }
});
