import assert from "node:assert/strict";
import test from "node:test";

import { canManageMessageForCurrentUser } from "./canManageMessage.ts";

const RELAY = "11".repeat(32);
const OWNER = "22".repeat(32);

function message(overrides = {}) {
  return {
    id: "message-1",
    author: "Archon",
    body: "build passed",
    createdAt: 1_700_000_000,
    depth: 0,
    kind: 9,
    pubkey: RELAY,
    time: "12:00 PM",
    ...overrides,
  };
}

test("App messages deny user-only management even to their relay signer", () => {
  assert.equal(
    canManageMessageForCurrentUser(
      message({ isApp: true, appId: "6eb31227-8ed2-42ec-9024-863497cbeed2" }),
      RELAY,
      undefined,
    ),
    false,
  );
});

test("App messages deny user-only management to an apparent signer owner", () => {
  assert.equal(
    canManageMessageForCurrentUser(
      message({ isApp: true, appId: "6eb31227-8ed2-42ec-9024-863497cbeed2" }),
      OWNER,
      {
        [RELAY]: {
          pubkey: RELAY,
          ownerPubkey: OWNER,
        },
      },
    ),
    false,
  );
});

test("human messages remain manageable by their signer", () => {
  assert.equal(
    canManageMessageForCurrentUser(message(), RELAY, undefined),
    true,
  );
});
