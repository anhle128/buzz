import assert from "node:assert/strict";
import { test } from "node:test";

import {
  mergeBoundDiscussionChannel,
  resolveProjectDiscussionChannel,
} from "./projectDiscussionChannel.ts";

const GENERAL = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50";
const DESIGN = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb51";
const MISSING = "11111111-1111-4111-8111-111111111111";

function channel(id, overrides = {}) {
  return {
    id,
    name: id === GENERAL ? "general" : "design",
    archivedAt: null,
    channelType: "stream",
    ...overrides,
  };
}

test("repository binding wins over the project binding", () => {
  assert.deepEqual(
    resolveProjectDiscussionChannel({
      repositoryChannelId: GENERAL,
      projectChannelId: DESIGN,
      channels: [channel(GENERAL), channel(DESIGN)],
    }),
    { id: GENERAL, name: "general" },
  );
});

test("project binding is the fallback when the repository has no binding", () => {
  assert.deepEqual(
    resolveProjectDiscussionChannel({
      repositoryChannelId: null,
      projectChannelId: DESIGN,
      channels: [channel(DESIGN)],
    }),
    { id: DESIGN, name: "design" },
  );
});

test("an unusable repository binding falls through to the project binding", () => {
  assert.deepEqual(
    resolveProjectDiscussionChannel({
      repositoryChannelId: DESIGN,
      projectChannelId: GENERAL,
      channels: [
        channel(GENERAL),
        channel(DESIGN, { archivedAt: "2026-08-20T00:00:00Z" }),
      ],
    }),
    { id: GENERAL, name: "general" },
  );
});

test("malformed and viewer-unresolved bindings do not resolve", () => {
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: "general",
      projectChannelId: null,
      channels: [channel(GENERAL)],
    }),
    null,
  );
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: MISSING,
      projectChannelId: null,
      channels: [channel(GENERAL)],
    }),
    null,
  );
});

test("archived and DM bindings do not resolve", () => {
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: GENERAL,
      projectChannelId: null,
      channels: [channel(GENERAL, { archivedAt: "2026-08-20T00:00:00Z" })],
    }),
    null,
  );
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: GENERAL,
      projectChannelId: null,
      channels: [channel(GENERAL, { channelType: "dm" })],
    }),
    null,
  );
});

test("non-DM channels resolve without a membership check", () => {
  assert.deepEqual(
    resolveProjectDiscussionChannel({
      repositoryChannelId: GENERAL,
      projectChannelId: null,
      channels: [channel(GENERAL, { channelType: "forum", isMember: false })],
    }),
    { id: GENERAL, name: "general" },
  );
});

test("a zero-hit bound channel is inserted first", () => {
  const discovered = [
    {
      id: DESIGN,
      name: "design",
      messageCount: 4,
      lastActivityAt: 200,
      participants: ["aa"],
    },
  ];
  const merged = mergeBoundDiscussionChannel(
    { id: GENERAL, name: "general" },
    discovered,
  );
  assert.deepEqual(merged[0], {
    id: GENERAL,
    name: "general",
    messageCount: 0,
    lastActivityAt: 0,
    participants: [],
  });
  assert.equal(merged[1], discovered[0]);
});

test("an already-discovered bound channel moves first without losing hit data", () => {
  const general = {
    id: GENERAL,
    name: "general",
    messageCount: 1,
    lastActivityAt: 1,
    participants: [],
  };
  const design = {
    id: DESIGN,
    name: "design",
    messageCount: 9,
    lastActivityAt: 9,
    participants: ["aa"],
  };
  const merged = mergeBoundDiscussionChannel({ id: DESIGN, name: "design" }, [
    general,
    design,
  ]);
  assert.equal(merged.length, 2);
  assert.equal(merged[0], design);
  assert.equal(merged[1], general);
});

test("no bound channel preserves the discovered order", () => {
  const discovered = [
    {
      id: DESIGN,
      name: "design",
      messageCount: 2,
      lastActivityAt: 2,
      participants: [],
    },
  ];
  assert.deepEqual(mergeBoundDiscussionChannel(null, discovered), discovered);
});
