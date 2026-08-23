import assert from "node:assert/strict";
import test from "node:test";

import { fromRawSearchHit } from "./searchWire.ts";

const TAGS = [
  ["h", "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50"],
  ["buzz:app", "6eb31227-8ed2-42ec-9024-863497cbeed2"],
  ["p", "33".repeat(32)],
];

test("fromRawSearchHit preserves tags through snake_case conversion", () => {
  const hit = fromRawSearchHit({
    event_id: "ab".repeat(32),
    content: "✅ build passed",
    kind: 9,
    pubkey: "cd".repeat(32),
    channel_id: "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50",
    channel_name: "general",
    created_at: 1_700_000_100,
    score: 0.75,
    tags: TAGS,
  });

  assert.equal(hit.eventId, "ab".repeat(32));
  assert.equal(hit.content, "✅ build passed");
  assert.equal(hit.kind, 9);
  assert.equal(hit.pubkey, "cd".repeat(32));
  assert.equal(hit.channelId, "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50");
  assert.equal(hit.channelName, "general");
  assert.equal(hit.createdAt, 1_700_000_100);
  assert.equal(hit.score, 0.75);
  assert.deepEqual(hit.tags, TAGS);
});

test("fromRawSearchHit defaults missing tags to an empty array", () => {
  const hit = fromRawSearchHit({
    event_id: "ef".repeat(32),
    content: "hello",
    kind: 9,
    pubkey: "aa".repeat(32),
    channel_id: null,
    channel_name: null,
    created_at: 1,
    score: 1,
  });
  assert.deepEqual(hit.tags, []);
});
