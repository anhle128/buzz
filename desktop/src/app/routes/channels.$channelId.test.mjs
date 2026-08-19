import assert from "node:assert/strict";
import test from "node:test";

import { validateChannelSearch } from "./channels.$channelId.tsx";

test("channel route preserves non-empty agentSessionChannel search", () => {
  assert.equal(
    validateChannelSearch({
      agentSession: "aa".repeat(32),
      agentSessionChannel: "channel-2",
    }).agentSessionChannel,
    "channel-2",
  );
});

test("channel route drops empty agentSessionChannel search", () => {
  assert.equal(
    validateChannelSearch({
      agentSessionChannel: "",
    }).agentSessionChannel,
    undefined,
  );
});
