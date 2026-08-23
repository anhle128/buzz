import assert from "node:assert/strict";
import test from "node:test";

import { RelayPublishTracker, publishWithAck } from "./relayPublishTracker.ts";

const EVENT = {
  id: "ab".repeat(32),
  pubkey: "cd".repeat(32),
  created_at: 1_700_000_000,
  kind: 9038,
  tags: [],
  content: '{"action":"create","name":"Pager"}',
  sig: "ef".repeat(32),
};

function fakeTimers() {
  let now = 0;
  const pending = new Map();
  let nextId = 1;
  return {
    now: () => now,
    setTimeout(fn, ms) {
      const id = nextId++;
      pending.set(id, { fn, fireAt: now + ms });
      return id;
    },
    clearTimeout(id) {
      pending.delete(id);
    },
    tick(ms) {
      now += ms;
      for (const [id, timer] of [...pending.entries()]) {
        if (timer.fireAt <= now) {
          pending.delete(id);
          timer.fn();
        }
      }
    },
  };
}

test("accepted OK resolves ack event and message", async () => {
  const timers = fakeTimers();
  const tracker = new RelayPublishTracker(timers);
  const pending = tracker.begin(EVENT, {
    timeoutMs: 1_000,
    timeoutMessage: "timed out",
  });
  assert.equal(
    tracker.handleOk(EVENT.id, true, 'response:{"app_id":"x"}'),
    true,
  );
  assert.deepEqual(await pending, {
    event: EVENT,
    message: 'response:{"app_id":"x"}',
  });
});

test("rejected OK rejects with the relay message", async () => {
  const timers = fakeTimers();
  const tracker = new RelayPublishTracker(timers);
  const pending = tracker.begin(EVENT, {
    timeoutMs: 1_000,
    timeoutMessage: "timed out",
  });
  tracker.handleOk(
    EVENT.id,
    false,
    "forbidden: must be a community owner or admin",
  );
  await assert.rejects(
    pending,
    /forbidden: must be a community owner or admin/,
  );
});

test("timeout rejects with the timeout message", async () => {
  const timers = fakeTimers();
  const tracker = new RelayPublishTracker(timers);
  const pending = tracker.begin(EVENT, {
    timeoutMs: 25_000,
    timeoutMessage: "Timed out while publishing.",
  });
  timers.tick(25_000);
  await assert.rejects(pending, /Timed out while publishing/);
});

test("reconnect retry resends after the first send fails", async () => {
  const timers = fakeTimers();
  const tracker = new RelayPublishTracker(timers);
  let sends = 0;
  let ensureCalls = 0;
  const ackPromise = publishWithAck({
    tracker,
    event: EVENT,
    timeoutMs: 25_000,
    timeoutMessage: "timed out",
    sendErrorMessage: "Failed to publish.",
    waitForGate: async () => {},
    sendEvent: async () => {
      sends += 1;
      if (sends === 1) {
        throw new Error("socket closed");
      }
    },
    ensureConnected: async () => {
      ensureCalls += 1;
    },
    recoverFromFailure: (error, fallback) =>
      error instanceof Error ? error : new Error(fallback),
  });

  for (let i = 0; i < 20 && sends < 2; i += 1) {
    await Promise.resolve();
  }
  assert.equal(sends, 2);
  assert.equal(ensureCalls, 1);
  tracker.handleOk(EVENT.id, true, "");
  const ack = await ackPromise;
  assert.equal(ack.event, EVENT);
  assert.equal(ack.message, "");
});

test("reject-all cleanup rejects every pending publish", async () => {
  const timers = fakeTimers();
  const tracker = new RelayPublishTracker(timers);
  const first = tracker.begin(EVENT, {
    timeoutMs: 1_000,
    timeoutMessage: "timed out",
  });
  const secondEvent = { ...EVENT, id: "11".repeat(32) };
  const second = tracker.begin(secondEvent, {
    timeoutMs: 1_000,
    timeoutMessage: "timed out",
  });
  tracker.rejectAll(new Error("Relay disconnected for community switch."));
  await assert.rejects(first, /Relay disconnected for community switch/);
  await assert.rejects(second, /Relay disconnected for community switch/);
  assert.equal(tracker.handleOk(EVENT.id, true, ""), false);
});
