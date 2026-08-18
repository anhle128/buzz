import assert from "node:assert/strict";
import test from "node:test";

const notifications = [];

class WorkingNotification {
  static permission = "granted";

  constructor(title, options) {
    notifications.push({ title, options });
  }

  close() {}
}

class ThrowingNotification {
  static permission = "granted";

  constructor() {
    throw new Error("notification backend unavailable");
  }
}

globalThis.window = { Notification: ThrowingNotification };

const { parseNotificationTarget, sendDesktopNotification } = await import("./desktop.ts");

test("constructor failure is a delivery miss and does not prevent a later notification", async (t) => {
  const warnings = [];
  t.mock.method(console, "warn", (...args) => warnings.push(args));

  const failed = await sendDesktopNotification({ title: "First" });

  assert.equal(failed, false);
  assert.equal(warnings.length, 1);
  assert.match(String(warnings[0][1]), /notification backend unavailable/);

  window.Notification = WorkingNotification;

  const delivered = await sendDesktopNotification({
    title: "Second",
    body: "Recovered",
  });

  assert.equal(delivered, true);
  assert.deepEqual(notifications, [
    {
      title: "Second",
      options: { body: "Recovered", silent: true, extra: undefined },
    },
  ]);
});

test("parseNotificationTarget accepts agentPubkey without channel or event", () => {
  assert.deepEqual(parseNotificationTarget({
    agentPubkey: "aa".repeat(32),
    channelId: null,
    eventId: null,
    kind: null,
  }), {
    agentPubkey: "aa".repeat(32),
    channelId: null,
    channelName: null,
    content: undefined,
    createdAt: null,
    eventId: null,
    kind: null,
    pubkey: undefined,
    threadRootId: null,
  });
});

test("parseNotificationTarget does not reinterpret message pubkey as agent identity", () => {
  assert.equal(parseNotificationTarget({
    pubkey: "aa".repeat(32),
    channelId: null,
    eventId: null,
    kind: 1,
  }), null);
});

test("parseNotificationTarget keeps event-only join targets valid", () => {
  const target = parseNotificationTarget({
    channelId: null,
    eventId: "ab".repeat(32),
    kind: 8000,
  });
  assert.equal(target?.eventId, "ab".repeat(32));
  assert.equal(target?.agentPubkey, undefined);
});

test("parseNotificationTarget rejects empty agentPubkey without another anchor", () => {
  assert.equal(parseNotificationTarget({
    agentPubkey: "",
    channelId: null,
    eventId: null,
    kind: null,
  }), null);
});
