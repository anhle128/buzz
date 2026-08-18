import assert from "node:assert/strict";
import test from "node:test";

import {
  buildInitialProjectEventTemplates,
  isUnsupportedProjectKindError,
  repositoryDtagFromName,
} from "./projectCreation.ts";

const OWNER = "a".repeat(64);
const CHANNEL = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50";

test("buildInitialProjectEventTemplates emits a NIP-MP project", () => {
  const templates = buildInitialProjectEventTemplates({
    accessChannelId: CHANNEL,
    cloneUrl: "https://relay.example/git/owner/sprout.git",
    description: "A multi-repository workspace",
    name: "Sprout",
    ownerPubkey: OWNER,
    webUrl: "https://example.com/sprout",
  });

  assert.equal(templates.dtag, "sprout");
  assert.equal(templates.repositoryDtag, "sprout");
  assert.equal(templates.project.kind, 30621);
  assert.equal(templates.repository.kind, 30617);
  assert.deepEqual(templates.project.tags, [
    ["d", "sprout"],
    ["name", "Sprout"],
    ["buzz-channel", CHANNEL],
    ["description", "A multi-repository workspace"],
    ["a", `30617:${OWNER}:sprout`],
  ]);
  assert.equal(templates.project.content, "");
  assert.deepEqual(templates.repository.tags, [
    ["d", "sprout"],
    ["name", "Sprout"],
    ["buzz-channel", CHANNEL],
    ["description", "A multi-repository workspace"],
    ["clone", "https://relay.example/git/owner/sprout.git"],
    ["web", "https://example.com/sprout"],
  ]);
});

test("omitted whitespace-only and undefined repositoryName copy the project identity", () => {
  const base = {
    accessChannelId: CHANNEL,
    name: "Sprout",
    ownerPubkey: OWNER,
  };
  const omitted = buildInitialProjectEventTemplates(base);
  const whitespace = buildInitialProjectEventTemplates({
    ...base,
    repositoryName: "   ",
  });
  const undef = buildInitialProjectEventTemplates({
    ...base,
    repositoryName: undefined,
  });
  assert.deepEqual(whitespace.project.tags, omitted.project.tags);
  assert.deepEqual(whitespace.repository.tags, omitted.repository.tags);
  assert.equal(whitespace.repositoryDtag, omitted.dtag);
  assert.deepEqual(undef.repository.tags, omitted.repository.tags);
});

test("repositoryName anhle128/buzz names only the first repository", () => {
  const templates = buildInitialProjectEventTemplates({
    accessChannelId: CHANNEL,
    name: "Bee Garden",
    ownerPubkey: OWNER,
    repositoryName: "  anhle128/buzz  ",
  });

  assert.equal(templates.dtag, "bee-garden");
  assert.equal(templates.repositoryDtag, "anhle128-buzz");
  assert.deepEqual(templates.project.tags, [
    ["d", "bee-garden"],
    ["name", "Bee Garden"],
    ["buzz-channel", CHANNEL],
    ["a", `30617:${OWNER}:anhle128-buzz`],
  ]);
  assert.deepEqual(templates.repository.tags, [
    ["d", "anhle128-buzz"],
    ["name", "anhle128/buzz"],
    ["buzz-channel", CHANNEL],
  ]);
  assert.match(templates.repositoryAddress, /:anhle128-buzz$/);
  assert.equal(templates.project.content, "");
});

test("filled repositoryName that slugs to the project dtag still uses the repo display string", () => {
  const templates = buildInitialProjectEventTemplates({
    accessChannelId: CHANNEL,
    name: "Bee Garden",
    ownerPubkey: OWNER,
    repositoryName: "bee-garden",
  });
  assert.equal(templates.dtag, "bee-garden");
  assert.equal(templates.repositoryDtag, "bee-garden");
  assert.deepEqual(templates.project.tags.slice(0, 2), [
    ["d", "bee-garden"],
    ["name", "Bee Garden"],
  ]);
  assert.deepEqual(templates.repository.tags.slice(0, 2), [
    ["d", "bee-garden"],
    ["name", "bee-garden"],
  ]);
});

test("buildInitialProjectEventTemplates rejects names without an identifier", () => {
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        name: "!!!",
        ownerPubkey: OWNER,
      }),
    /letters or numbers/,
  );
});

test("filled repositoryName /// does not fall back to the project name", () => {
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        ownerPubkey: OWNER,
        repositoryName: "///",
      }),
    /letters or numbers/,
  );
});

test("repository name slug must not exceed 64 characters", () => {
  assert.doesNotThrow(() =>
    buildInitialProjectEventTemplates({
      accessChannelId: CHANNEL,
      name: "Bee Garden",
      ownerPubkey: OWNER,
      repositoryName: "a".repeat(64),
    }),
  );
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        ownerPubkey: OWNER,
        repositoryName: "a".repeat(65),
      }),
    /64 characters/,
  );
});

test("project slug may exceed 64 characters when repositoryName is omitted", () => {
  const name = "a".repeat(65);
  const templates = buildInitialProjectEventTemplates({
    accessChannelId: CHANNEL,
    name,
    ownerPubkey: OWNER,
  });
  assert.equal(templates.dtag, name);
  assert.equal(templates.repositoryDtag, name);
});

test("repository name must not exceed 256 bytes", () => {
  // 64 ASCII + 48×4-byte emoji = 256 bytes; slug stays at 64 chars.
  assert.doesNotThrow(() =>
    buildInitialProjectEventTemplates({
      accessChannelId: CHANNEL,
      name: "Bee Garden",
      ownerPubkey: OWNER,
      repositoryName: "a".repeat(64) + "🙂".repeat(48),
    }),
  );
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        ownerPubkey: OWNER,
        repositoryName: "a".repeat(257),
      }),
    /256 bytes/,
  );
});

test("buildInitialProjectEventTemplates enforces the description tag byte limit", () => {
  assert.doesNotThrow(() =>
    buildInitialProjectEventTemplates({
      accessChannelId: CHANNEL,
      description: "🙂".repeat(512),
      name: "Sprout",
      ownerPubkey: OWNER,
    }),
  );
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        description: "🙂".repeat(513),
        name: "Sprout",
        ownerPubkey: OWNER,
      }),
    /2,048 bytes/,
  );
});

test("repositoryDtagFromName replaces non-alphanumerics with dashes", () => {
  assert.equal(repositoryDtagFromName("anhle128/buzz"), "anhle128-buzz");
  assert.equal(repositoryDtagFromName("Bee Garden"), "bee-garden");
});

test("isUnsupportedProjectKindError recognizes relay kind compatibility failures", () => {
  assert.equal(
    isUnsupportedProjectKindError(
      new Error("restricted: unknown event kind 30621"),
    ),
    true,
  );
  assert.equal(
    isUnsupportedProjectKindError(new Error("mock project event rejection")),
    false,
  );
});
