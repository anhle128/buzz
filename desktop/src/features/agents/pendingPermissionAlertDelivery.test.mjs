import assert from "node:assert/strict";
import test from "node:test";

import { deliverPermissionAlert } from "./pendingPermissionAlertDelivery.ts";

const AGENT = "a".repeat(64);

function dependencies(calls, { capture, didSend = true } = {}) {
  return {
    showToast: (toast) => {
      calls.push(
        `toast:${toast.id}:${toast.title}:${toast.body}:${toast.duration}`,
      );
      if (capture) {
        capture.toastClick = toast.onClick;
      }
    },
    sendOsNotification: async (notification) => {
      calls.push(["os", notification]);
      return didSend;
    },
    revealDesktopAppWindow: async () => {
      calls.push("reveal");
    },
    openAgentActivity: (pubkey, options) => {
      calls.push(`open:${pubkey}:${options.channelId}`);
    },
    requestDockBounce: async () => {
      calls.push("bounce");
    },
    playSound: () => {
      calls.push("sound");
    },
  };
}

test("focused delivery shows a persistent toast sounds and reveals before opening", async () => {
  const calls = [];
  const capture = {};
  await deliverPermissionAlert(
    {
      agentPubkey: AGENT,
      channelId: "channel-1",
      copy: { title: "Ada needs permission", body: "Open the process log." },
      requestNonce: "nonce-1",
      soundEnabled: true,
      surface: "toast",
    },
    dependencies(calls, { capture }),
  );

  assert.deepEqual(calls, [
    "toast:nonce-1:Ada needs permission:Open the process log.:Infinity",
    "sound",
  ]);
  await capture.toastClick();
  assert.deepEqual(calls.slice(-2), ["reveal", `open:${AGENT}:channel-1`]);
});

test("successful OS delivery bounces and sounds after send", async () => {
  const calls = [];
  await deliverPermissionAlert(
    {
      agentPubkey: AGENT,
      channelId: null,
      copy: { title: "Ada needs permission", body: "Open the process log." },
      requestNonce: "nonce-2",
      soundEnabled: true,
      surface: "os",
    },
    dependencies(calls),
  );
  assert.deepEqual(calls, [
    [
      "os",
      {
        title: "Ada needs permission",
        body: "Open the process log.",
        target: {
          agentPubkey: AGENT,
          channelId: null,
          eventId: null,
          kind: null,
        },
      },
    ],
    "bounce",
    "sound",
  ]);
});

test("failed OS delivery and null surface have no follow-up effects", async () => {
  const failed = [];
  await deliverPermissionAlert(
    {
      agentPubkey: AGENT,
      channelId: null,
      copy: { title: "Ada needs permission", body: "Open the process log." },
      requestNonce: "nonce-3",
      soundEnabled: true,
      surface: "os",
    },
    dependencies(failed, { didSend: false }),
  );
  assert.deepEqual(failed, [
    [
      "os",
      {
        title: "Ada needs permission",
        body: "Open the process log.",
        target: {
          agentPubkey: AGENT,
          channelId: null,
          eventId: null,
          kind: null,
        },
      },
    ],
  ]);

  const mutedToast = [];
  await deliverPermissionAlert(
    {
      agentPubkey: AGENT,
      channelId: null,
      copy: { title: "Ada needs permission", body: "Open the process log." },
      requestNonce: "nonce-muted",
      soundEnabled: false,
      surface: "toast",
    },
    dependencies(mutedToast),
  );
  assert.deepEqual(mutedToast, [
    "toast:nonce-muted:Ada needs permission:Open the process log.:Infinity",
  ]);

  const silent = [];
  await deliverPermissionAlert(
    {
      agentPubkey: AGENT,
      channelId: null,
      copy: { title: "Ada needs permission", body: "Open the process log." },
      requestNonce: "nonce-4",
      soundEnabled: false,
      surface: null,
    },
    dependencies(silent),
  );
  assert.deepEqual(silent, []);
});

test("successful muted OS delivery bounces without playing sound", async () => {
  const calls = [];
  await deliverPermissionAlert(
    {
      agentPubkey: AGENT,
      channelId: null,
      copy: { title: "Ada needs permission", body: "Open the process log." },
      requestNonce: "nonce-muted-os",
      soundEnabled: false,
      surface: "os",
    },
    dependencies(calls),
  );
  assert.deepEqual(calls, [
    [
      "os",
      {
        title: "Ada needs permission",
        body: "Open the process log.",
        target: {
          agentPubkey: AGENT,
          channelId: null,
          eventId: null,
          kind: null,
        },
      },
    ],
    "bounce",
  ]);
});
