import assert from "node:assert/strict";
import test from "node:test";

import {
  executeDesktopNotificationAction,
  markAllReadSources,
  resolveDesktopNotificationAction,
  shouldBounceForChannelNotification,
} from "./AppShell.helpers.ts";

test("shouldBounceForChannelNotification_allowsTopLevelChannelMessages", () => {
  assert.equal(shouldBounceForChannelNotification([["h", "channel"]]), true);
});

test("shouldBounceForChannelNotification_suppressesThreadReplies", () => {
  assert.equal(
    shouldBounceForChannelNotification([
      ["h", "channel"],
      ["e", "root", "", "reply"],
    ]),
    false,
  );
});

test("shouldBounceForChannelNotification_allowsBroadcastReplies", () => {
  assert.equal(
    shouldBounceForChannelNotification([
      ["h", "channel"],
      ["e", "root", "", "reply"],
      ["broadcast", "1"],
    ]),
    true,
  );
});

test("markAllReadSources clears Inbox overrides and active thread activity", () => {
  const calls = [];

  markAllReadSources({
    activeChannelId: "active-channel",
    channelActivityItems: [
      { channelId: "another-channel", createdAt: 100 },
      { channelId: "active-channel", createdAt: 200 },
      { channelId: "active-channel", createdAt: 300 },
    ],
    unreadFeedItemIds: new Set(["first-inbox-item", "second-inbox-item"]),
    undoUnreadFeedItem: (itemId) => calls.push(`inbox:${itemId}`),
    markAllChannelReadMarkers: () => calls.push("channels"),
    markActiveChannelRead: (channelId, createdAt) =>
      calls.push(`active:${channelId}:${createdAt}`),
  });

  assert.deepEqual(calls, [
    "inbox:first-inbox-item",
    "inbox:second-inbox-item",
    "channels",
    "active:active-channel:300",
  ]);
});

test("markAllReadSources skips the active marker without projected activity", () => {
  const calls = [];

  markAllReadSources({
    activeChannelId: "active-channel",
    channelActivityItems: [],
    unreadFeedItemIds: new Set(),
    undoUnreadFeedItem: () => calls.push("inbox"),
    markAllChannelReadMarkers: () => calls.push("channels"),
    markActiveChannelRead: () => calls.push("active"),
  });

  assert.deepEqual(calls, ["channels"]);
});

test("resolveDesktopNotificationAction gives agentPubkey precedence over a search hit", () => {
  assert.deepEqual(
    resolveDesktopNotificationAction({
      agentPubkey: "aa".repeat(32),
      channelId: "channel-1",
      content: "ignored",
      createdAt: 1,
      eventId: "ab".repeat(32),
      kind: 9,
      pubkey: "cc".repeat(32),
      threadRootId: null,
    }),
    {
      type: "agent-activity",
      agentPubkey: "aa".repeat(32),
      channelId: "channel-1",
    },
  );
});

test("resolveDesktopNotificationAction keeps no-channel event targets on Home", () => {
  assert.deepEqual(
    resolveDesktopNotificationAction({
      channelId: null,
      eventId: "ab".repeat(32),
      kind: 8000,
    }),
    { type: "home" },
  );
});

test("resolveDesktopNotificationAction keeps channel-only targets on channel routing", () => {
  assert.deepEqual(
    resolveDesktopNotificationAction({
      channelId: "channel-1",
      eventId: null,
      kind: null,
    }),
    { type: "channel", channelId: "channel-1" },
  );
});

test("executeDesktopNotificationAction reveals before opening agent activity and never opens a search hit", async () => {
  const calls = [];
  await executeDesktopNotificationAction(
    {
      agentPubkey: "aa".repeat(32),
      channelId: null,
      eventId: "ab".repeat(32),
      kind: 9,
    },
    {
      revealDesktopAppWindow: async () => calls.push("reveal"),
      openAgentActivity: (pubkey, options) =>
        calls.push(`agent:${pubkey}:${options.channelId}`),
      goHome: async () => calls.push("home"),
      goChannel: async (channelId) => calls.push(`channel:${channelId}`),
      openSearchHit: async () => calls.push("search"),
    },
  );
  assert.deepEqual(calls, ["reveal", `agent:${"aa".repeat(32)}:null`]);
});

test("executeDesktopNotificationAction preserves message search-hit routing", async () => {
  const calls = [];
  await executeDesktopNotificationAction(
    {
      channelId: "channel-1",
      eventId: "ab".repeat(32),
      kind: 9,
      pubkey: "cc".repeat(32),
    },
    {
      revealDesktopAppWindow: async () => calls.push("reveal"),
      openAgentActivity: () => calls.push("agent"),
      goHome: async () => calls.push("home"),
      goChannel: async () => calls.push("channel"),
      openSearchHit: async (hit) => calls.push(`search:${hit.eventId}`),
    },
  );
  assert.deepEqual(calls, ["reveal", `search:${"ab".repeat(32)}`]);
});
