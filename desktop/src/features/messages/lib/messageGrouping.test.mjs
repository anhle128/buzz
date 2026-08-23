import assert from "node:assert/strict";
import test from "node:test";

import {
  MESSAGE_GROUPING_WINDOW_SECONDS,
  hasSameMessageAuthor,
  isWithinGroupingWindow,
  startsNewMessageGroup,
} from "./messageGrouping.ts";

test("startsNewMessageGroup: sent-from-thread messages start a fresh group", () => {
  assert.equal(
    startsNewMessageGroup({
      tags: [["buzz:sent-from-thread", "root-event", "Root summary"]],
    }),
    true,
  );
  assert.equal(startsNewMessageGroup({ tags: [["h", "channel-id"]] }), false);
  assert.equal(startsNewMessageGroup(undefined), false);
});

test("hasSameMessageAuthor: matches case-insensitively and trims", () => {
  assert.equal(
    hasSameMessageAuthor({ pubkey: " ABC " }, { pubkey: "abc" }),
    true,
  );
  assert.equal(
    hasSameMessageAuthor({ pubkey: "abc" }, { pubkey: "def" }),
    false,
  );
});

test("hasSameMessageAuthor: missing pubkeys never match", () => {
  assert.equal(hasSameMessageAuthor(null, { pubkey: "abc" }), false);
  assert.equal(hasSameMessageAuthor({ pubkey: "abc" }, undefined), false);
  assert.equal(hasSameMessageAuthor({ pubkey: "" }, { pubkey: "" }), false);
});

const RELAY =
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const APP_A = "6eb31227-8ed2-42ec-9024-863497cbeed2";
const APP_B = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

test("hasSameMessageAuthor: matching relay pubkeys with different App UUIDs do not group", () => {
  assert.equal(
    hasSameMessageAuthor(
      { pubkey: RELAY, appId: APP_A },
      { pubkey: RELAY, appId: APP_B },
    ),
    false,
  );
});

test("hasSameMessageAuthor: matching App UUIDs group even under one relay signer", () => {
  assert.equal(
    hasSameMessageAuthor(
      { pubkey: RELAY, appId: APP_A },
      { pubkey: RELAY, appId: APP_A },
    ),
    true,
  );
});

test("hasSameMessageAuthor: an App does not group with a human using the same relay pubkey", () => {
  assert.equal(
    hasSameMessageAuthor({ pubkey: RELAY, appId: APP_A }, { pubkey: RELAY }),
    false,
  );
});

test("isWithinGroupingWindow: at or under the boundary is in window", () => {
  const base = 1_000_000;
  assert.equal(isWithinGroupingWindow(base, base), true);
  assert.equal(
    isWithinGroupingWindow(base, base + MESSAGE_GROUPING_WINDOW_SECONDS),
    true,
  );
});

test("isWithinGroupingWindow: past the boundary is out of window", () => {
  const base = 1_000_000;
  assert.equal(
    isWithinGroupingWindow(base, base + MESSAGE_GROUPING_WINDOW_SECONDS + 1),
    false,
  );
});

test("isWithinGroupingWindow: out-of-order (negative gap) is out of window", () => {
  const base = 1_000_000;
  assert.equal(isWithinGroupingWindow(base + 60, base), false);
});

test("isWithinGroupingWindow: missing timestamps are out of window", () => {
  assert.equal(isWithinGroupingWindow(null, 1_000_000), false);
  assert.equal(isWithinGroupingWindow(1_000_000, undefined), false);
  assert.equal(isWithinGroupingWindow(undefined, undefined), false);
});
