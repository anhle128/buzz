import assert from "node:assert/strict";
import test from "node:test";

import {
  KIND_APP_ADMIN_COMMAND,
  appCallbackUrl,
  buildCreateCommand,
  buildDisableCommand,
  buildEnableCommand,
  buildRotateSecretCommand,
  buildUpdateCommand,
  closeAppCredentialCaches,
  createAppCredentialStore,
  mutationHoldsAppSecret,
  parseAppSecretAck,
  publicAppAck,
  serializeAppCommand,
} from "./appCommands.ts";

const APP_ID = "6eb31227-8ed2-42ec-9024-863497cbeed2";
const RELAY = "http://localhost:3000";

function jsonEq(actual, expected) {
  assert.deepEqual(JSON.parse(actual), expected);
}

test("kind 9038 is the app admin command", () => {
  assert.equal(KIND_APP_ADMIN_COMMAND, 9038);
});

test("create command json is exact kind 9038 body", () => {
  jsonEq(
    serializeAppCommand(
      buildCreateCommand({
        name: "Buildkite",
        description: "Build notifications",
        iconUrl: "https://example.test/icon.png",
      }),
    ),
    {
      action: "create",
      name: "Buildkite",
      description: "Build notifications",
      icon_url: "https://example.test/icon.png",
    },
  );
});

test("create omits null optional fields", () => {
  jsonEq(serializeAppCommand(buildCreateCommand({ name: "Pager" })), {
    action: "create",
    name: "Pager",
  });
});

test("update rotate enable disable json is exact", () => {
  jsonEq(
    serializeAppCommand(buildUpdateCommand({ appId: APP_ID, name: "New" })),
    { action: "update", app_id: APP_ID, name: "New" },
  );
  jsonEq(
    serializeAppCommand(
      buildUpdateCommand({ appId: APP_ID, clearDescription: true }),
    ),
    { action: "update", app_id: APP_ID, description: "" },
  );
  jsonEq(
    serializeAppCommand(buildUpdateCommand({ appId: APP_ID, clearIcon: true })),
    { action: "update", app_id: APP_ID, icon_url: "" },
  );
  jsonEq(serializeAppCommand(buildRotateSecretCommand(APP_ID)), {
    action: "rotate_secret",
    app_id: APP_ID,
  });
  jsonEq(serializeAppCommand(buildEnableCommand(APP_ID)), {
    action: "enable",
    app_id: APP_ID,
  });
  jsonEq(serializeAppCommand(buildDisableCommand(APP_ID)), {
    action: "disable",
    app_id: APP_ID,
  });
});

test("update without mutation is rejected", () => {
  assert.throws(
    () => buildUpdateCommand({ appId: APP_ID }),
    /at least one of name, description, or icon_url/,
  );
});

test("create rejects empty name", () => {
  assert.throws(
    () => buildCreateCommand({ name: "   " }),
    /name must be non-empty/,
  );
});

test("icon url limit is measured in UTF-8 bytes", () => {
  assert.throws(
    () =>
      buildCreateCommand({
        name: "Buildkite",
        iconUrl: `https://example.test/${"€".repeat(1_400)}`,
      }),
    /4096 UTF-8 bytes/,
  );
});

test("icon url rejects Unicode whitespace", () => {
  assert.throws(
    () =>
      buildCreateCommand({
        name: "Buildkite",
        iconUrl: "https://example.test/icon\u00a0name.png",
      }),
    /invalid characters/,
  );
});

test("app id must be canonical lowercase uuid", () => {
  assert.throws(
    () => buildRotateSecretCommand(APP_ID.toUpperCase()),
    /canonical/i,
  );
  assert.throws(() => buildEnableCommand("not-a-uuid"), /canonical/i);
});

test("callback url uses relay http base and app id", () => {
  assert.equal(appCallbackUrl(RELAY, APP_ID), `${RELAY}/hooks/apps/${APP_ID}`);
  assert.equal(
    appCallbackUrl("https://relay.example.com/", APP_ID),
    `https://relay.example.com/hooks/apps/${APP_ID}`,
  );
});

test("successful create parses secret and callback only", () => {
  const message = `response:${JSON.stringify({
    app_id: APP_ID,
    webhook_secret: "one-time-secret",
  })}`;
  assert.deepEqual(parseAppSecretAck(message, RELAY), {
    appId: APP_ID,
    callbackUrl: `${RELAY}/hooks/apps/${APP_ID}`,
    webhookSecret: "one-time-secret",
  });
});

test("duplicate create or rotate without secret is an error", () => {
  assert.throws(
    () => parseAppSecretAck("duplicate: already processed", RELAY),
    (error) => {
      assert.match(String(error), /rotate/);
      assert.match(String(error), /no secret/);
      return true;
    },
  );

  assert.throws(
    () =>
      parseAppSecretAck(
        `response:${JSON.stringify({ app_id: APP_ID })}`,
        RELAY,
      ),
    (error) => {
      assert.match(String(error), /rotate-secret/);
      assert.match(String(error), new RegExp(APP_ID));
      return true;
    },
  );
});

test("credential helper clears on close and unmount", () => {
  const store = createAppCredentialStore();
  const credentials = {
    appId: APP_ID,
    callbackUrl: `${RELAY}/hooks/apps/${APP_ID}`,
    webhookSecret: "one-time-secret",
  };

  store.set(credentials);
  assert.deepEqual(store.get(), credentials);

  store.clearOnClose();
  assert.equal(store.get(), null);

  store.set(credentials);
  store.clearOnUnmount();
  assert.equal(store.get(), null);
});

test("mutation cache result omits the one-time secret", () => {
  const credentials = parseAppSecretAck(
    `response:${JSON.stringify({
      app_id: APP_ID,
      webhook_secret: "one-time-secret",
    })}`,
    RELAY,
  );
  const cached = publicAppAck(credentials);
  assert.deepEqual(cached, {
    appId: APP_ID,
    callbackUrl: `${RELAY}/hooks/apps/${APP_ID}`,
  });
  assert.equal("webhookSecret" in cached, false);
  assert.equal(mutationHoldsAppSecret(cached), false);
  assert.equal(mutationHoldsAppSecret(credentials), true);
});

test("close clears a leaked secret from mutation cache and dialog store", () => {
  const store = createAppCredentialStore();
  const credentials = parseAppSecretAck(
    `response:${JSON.stringify({
      app_id: APP_ID,
      webhook_secret: "one-time-secret",
    })}`,
    RELAY,
  );
  store.set(credentials);
  const leakedMutationData = credentials;
  assert.equal(mutationHoldsAppSecret(leakedMutationData), true);

  const closed = closeAppCredentialCaches({
    store,
    mutationData: leakedMutationData,
  });
  assert.equal(store.get(), null);
  assert.equal(closed.mutationData, null);
  assert.equal(mutationHoldsAppSecret(closed.mutationData), false);
});
