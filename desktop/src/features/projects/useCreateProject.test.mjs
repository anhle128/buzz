import assert from "node:assert/strict";
import test from "node:test";

import { createProject } from "./useCreateProject.ts";

const OWNER = "a".repeat(64);
const CHANNEL = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50";

function makeRepoHead(dtag) {
  return {
    id: "b".repeat(64),
    kind: 30617,
    pubkey: OWNER,
    created_at: 1,
    content: "",
    tags: [
      ["d", dtag],
      ["name", dtag],
    ],
    sig: "0".repeat(128),
  };
}

function makeRepoHeadWithName(dtag, name) {
  return {
    ...makeRepoHead(dtag),
    tags: [
      ["d", dtag],
      ["name", name],
      ["buzz-channel", CHANNEL],
    ],
  };
}

function signEvent(template) {
  return {
    ...template,
    id: template.kind === 30621 ? "c".repeat(64) : "d".repeat(64),
    pubkey: OWNER,
    created_at: 2,
    sig: "0".repeat(128),
  };
}

test("distinct repo slug with an existing 30617 head throws and publishes nothing", async () => {
  const published = [];
  let signed = 0;
  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        repositoryName: "anhle128/buzz",
      },
      new Map(),
      {
        getIdentity: async () => ({ pubkey: OWNER }),
        fetchProjects: async () => [],
        fetchEvents: async (filter) => {
          assert.deepEqual(filter.kinds, [30617]);
          assert.deepEqual(filter.authors, [OWNER]);
          assert.deepEqual(filter["#d"], ["anhle128-buzz"]);
          assert.equal(filter.limit, 1);
          return [makeRepoHead("anhle128-buzz")];
        },
        signRelayEvent: async () => {
          signed += 1;
          throw new Error("must not sign");
        },
        publishEvent: async (event) => {
          published.push(event);
          throw new Error("must not publish");
        },
      },
    ),
    /A repository named "anhle128-buzz" already exists \(as a standalone repository or in another project\)\. Choose a different name to avoid overwriting it\./,
  );
  assert.equal(published.length, 0);
  assert.equal(signed, 0);
});

test("same project and repo slugs do not query kind 30617", async () => {
  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Sprout",
      },
      new Map(),
      {
        getIdentity: async () => ({ pubkey: OWNER }),
        fetchProjects: async () => [],
        fetchEvents: async () => {
          throw new Error(
            "must not query kind 30617 when repositoryDtag === dtag",
          );
        },
        signRelayEvent: async () => {
          throw new Error("stop-after-clobber");
        },
        publishEvent: async () => {
          throw new Error("must not publish");
        },
      },
    ),
    /stop-after-clobber/,
  );
});

test("filled repositoryName that slugs to the project dtag does not query kind 30617", async () => {
  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        repositoryName: "bee-garden",
      },
      new Map(),
      {
        getIdentity: async () => ({ pubkey: OWNER }),
        fetchProjects: async () => [],
        fetchEvents: async () => {
          throw new Error(
            "must not query kind 30617 when repositoryDtag === dtag",
          );
        },
        signRelayEvent: async () => {
          throw new Error("stop-after-clobber");
        },
        publishEvent: async () => {
          throw new Error("must not publish");
        },
      },
    ),
    /stop-after-clobber/,
  );
});

test("distinct repo slug with no existing 30617 head proceeds to sign", async () => {
  const filters = [];
  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        repositoryName: "anhle128/buzz",
      },
      new Map(),
      {
        getIdentity: async () => ({ pubkey: OWNER }),
        fetchProjects: async () => [],
        fetchEvents: async (filter) => {
          filters.push(filter);
          return [];
        },
        signRelayEvent: async () => {
          throw new Error("stop-after-clobber");
        },
        publishEvent: async () => {
          throw new Error("must not publish");
        },
      },
    ),
    /stop-after-clobber/,
  );
  assert.equal(filters.length, 1);
  assert.deepEqual(filters[0]["#d"], ["anhle128-buzz"]);
});

test("distinct repo slug retry reuses the published 30617 head and publishes only the project", async () => {
  const resumableProjects = new Map();
  const publishedKinds = [];
  let firstProjectPublish = true;
  let returnMatchingHead = false;
  const input = {
    accessChannelId: CHANNEL,
    name: "Bee Garden",
    repositoryName: "anhle128/buzz",
  };

  const deps = {
    getIdentity: async () => ({ pubkey: OWNER }),
    fetchProjects: async () => [],
    fetchEvents: async (filter) => {
      assert.deepEqual(filter.kinds, [30617]);
      assert.deepEqual(filter.authors, [OWNER]);
      assert.deepEqual(filter["#d"], ["anhle128-buzz"]);
      assert.equal(filter.limit, 1);
      if (firstProjectPublish) return [];
      return [
        {
          ...makeRepoHeadWithName("anhle128-buzz", "anhle128/buzz"),
          id: (returnMatchingHead ? "d" : "b").repeat(64),
        },
      ];
    },
    signRelayEvent: async (template) => signEvent(template),
    publishEvent: async (event) => {
      publishedKinds.push(event.kind);
      if (event.kind === 30621 && firstProjectPublish) {
        firstProjectPublish = false;
        throw new Error("mock project event rejection");
      }
    },
  };

  await assert.rejects(
    createProject(input, resumableProjects, deps),
    /mock project event rejection/,
  );
  assert.deepEqual(publishedKinds, [30617, 30621]);
  assert.deepEqual(resumableProjects.get(`${OWNER}:bee-garden`), {
    repositoryAddress: `30617:${OWNER}:anhle128-buzz`,
    repositoryEventId: "d".repeat(64),
  });

  await assert.rejects(
    createProject(input, resumableProjects, deps),
    /A repository named "anhle128-buzz" already exists/,
  );
  assert.deepEqual(publishedKinds, [30617, 30621]);

  returnMatchingHead = true;
  const result = await createProject(input, resumableProjects, deps);
  assert.deepEqual(publishedKinds, [30617, 30621, 30621]);
  assert.equal(result.project.dtag, "bee-garden");
  assert.equal(result.project.repositories[0]?.dtag, "anhle128-buzz");
  assert.equal(result.project.repositories[0]?.name, "anhle128/buzz");
});

test("retry with a changed repository name cannot reuse a different existing head", async () => {
  const resumableProjects = new Map();
  const publishedKinds = [];
  let firstProjectPublish = true;
  const deps = {
    getIdentity: async () => ({ pubkey: OWNER }),
    fetchProjects: async () => [],
    fetchEvents: async (filter) =>
      filter["#d"]?.[0] === "first-repository"
        ? []
        : [makeRepoHead("existing-repository")],
    signRelayEvent: async (template) => signEvent(template),
    publishEvent: async (event) => {
      publishedKinds.push(event.kind);
      if (event.kind === 30621 && firstProjectPublish) {
        firstProjectPublish = false;
        throw new Error("mock project event rejection");
      }
    },
  };

  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        repositoryName: "first/repository",
      },
      resumableProjects,
      deps,
    ),
    /mock project event rejection/,
  );
  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        repositoryName: "existing/repository",
      },
      resumableProjects,
      deps,
    ),
    /A repository named "existing-repository" already exists/,
  );
  assert.deepEqual(publishedKinds, [30617, 30621]);
});
