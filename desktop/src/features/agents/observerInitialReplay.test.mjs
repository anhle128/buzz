import assert from "node:assert/strict";
import test from "node:test";

import {
  _testEnqueueObserverStoreWork,
  _testQueueInitialObserverReplayComplete,
  isAgentObserverInitialReplayComplete,
  resetAgentObserverStore,
  subscribeAgentObserverStore,
} from "./observerRelayStore.ts";

test("initial observer replay completes once after the queued EOSE barrier and resets", async () => {
  resetAgentObserverStore();
  const notifications = [];
  const unsubscribe = subscribeAgentObserverStore((update) =>
    notifications.push(update),
  );

  assert.equal(isAgentObserverInitialReplayComplete(), false);
  let releaseQueuedWork;
  const queuedWork = new Promise((resolve) => {
    releaseQueuedWork = resolve;
  });
  _testEnqueueObserverStoreWork(() => queuedWork);
  const completion = _testQueueInitialObserverReplayComplete();
  await Promise.resolve();
  assert.equal(isAgentObserverInitialReplayComplete(), false);
  assert.deepEqual(notifications, []);
  releaseQueuedWork();
  await completion;
  assert.equal(isAgentObserverInitialReplayComplete(), true);
  assert.deepEqual(notifications, [undefined]);

  await _testQueueInitialObserverReplayComplete();
  assert.deepEqual(notifications, [undefined]);

  resetAgentObserverStore();
  assert.equal(isAgentObserverInitialReplayComplete(), false);
  unsubscribe();
});
