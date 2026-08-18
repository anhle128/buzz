import assert from "node:assert/strict";
import { test } from "node:test";

import {
  createProjectIssueWith,
  projectIssueInvalidationKeys,
} from "./issueMutations.ts";

const REPO_ADDRESS = `30617:${"a".repeat(64)}:app`;

test("GitHub create never calls the Buzz issue publisher", async () => {
  const calls = { github: 0, buzz: 0 };
  const id = await createProjectIssueWith(
    {
      id: "p1",
      owner: "a".repeat(64),
      repoAddress: REPO_ADDRESS,
      cloneUrls: ["https://github.com/acme/app"],
    },
    { title: "Broken login", body: "steps" },
    {
      createGithub: async (input) => {
        calls.github += 1;
        assert.deepEqual(input, {
          cloneUrl: "https://github.com/acme/app",
          title: "Broken login",
          body: "steps",
        });
        return { number: 43 };
      },
      publishBuzz: async () => {
        calls.buzz += 1;
        return "e".repeat(64);
      },
    },
  );
  assert.equal(id, "43");
  assert.deepEqual(calls, { github: 1, buzz: 0 });
});

test("GitHub create rejects an unsafe native issue number", async () => {
  await assert.rejects(
    createProjectIssueWith(
      {
        id: "p1",
        owner: "a".repeat(64),
        repoAddress: REPO_ADDRESS,
        cloneUrls: ["https://github.com/acme/app"],
      },
      { title: "Broken login", body: "steps" },
      {
        createGithub: async () => ({
          number: Number.MAX_SAFE_INTEGER + 1,
        }),
        publishBuzz: async () => "e".repeat(64),
      },
    ),
    /GitHub returned an invalid issue number/,
  );
});

test("Buzz create never calls the GitHub creator", async () => {
  const calls = { github: 0, buzz: 0 };
  const id = await createProjectIssueWith(
    {
      id: "p2",
      owner: "a".repeat(64),
      repoAddress: REPO_ADDRESS,
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
    },
    { title: "Buzz bug", body: "" },
    {
      createGithub: async () => {
        calls.github += 1;
        return { number: 1 };
      },
      publishBuzz: async () => {
        calls.buzz += 1;
        return "e".repeat(64);
      },
    },
  );
  assert.equal(id, "e".repeat(64));
  assert.deepEqual(calls, { github: 0, buzz: 1 });
});
test("GitHub create invalidates only its repository issue query", () => {
  assert.deepEqual(
    projectIssueInvalidationKeys({
      id: "p1",
      cloneUrls: ["https://github.com/acme/app"],
    }),
    [["project", "p1", "issues"]],
  );
});

test("Buzz create preserves all existing invalidations", () => {
  assert.deepEqual(
    projectIssueInvalidationKeys({
      id: "p2",
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
    }),
    [
      ["project", "p2", "issues"],
      ["projects", "work-items"],
      ["projects", "activity-summaries"],
    ],
  );
});
