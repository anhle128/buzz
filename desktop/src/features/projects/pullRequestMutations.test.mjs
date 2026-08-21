import assert from "node:assert/strict";
import test from "node:test";

import {
  canPublishProjectPullRequestUpdate,
  createProjectPullRequestWith,
  projectPullRequestInvalidationKeys,
  projectPullRequestMergedTags,
  projectPullRequestTags,
  projectPullRequestUpdateTags,
} from "./pullRequestMutations.ts";

const OWNER = "a".repeat(64);
const AUTHOR = "b".repeat(64);
const REVIEWER = "c".repeat(64);
const PR_ID = "d".repeat(64);
const COMMIT = "e".repeat(40);
const MERGE_BASE = "f".repeat(40);

const project = {
  owner: OWNER,
  repoAddress: `30617:${OWNER}:buzz`,
  cloneUrls: [`https://relay.example/git/${OWNER}/buzz`],
};
const pullRequest = {
  author: AUTHOR,
};

test("only the repository owner or PR author can publish an update", () => {
  assert.equal(
    canPublishProjectPullRequestUpdate(OWNER, project, pullRequest),
    true,
  );
  assert.equal(
    canPublishProjectPullRequestUpdate(AUTHOR, project, pullRequest),
    true,
  );
  assert.equal(
    canPublishProjectPullRequestUpdate(REVIEWER, project, pullRequest),
    false,
  );
});

test("projectPullRequestTags builds a NIP-34 kind 1618 tag set", () => {
  const tags = projectPullRequestTags(project, {
    title: "Add Projects workflow",
    body: "",
    branch: "projects-workflow",
    targetBranch: "main",
    commit: COMMIT,
    mergeBase: MERGE_BASE,
    reviewers: [REVIEWER, REVIEWER.toUpperCase()],
  });

  assert.deepEqual(tags, [
    ["a", project.repoAddress],
    ["p", OWNER],
    ["p", REVIEWER],
    ["subject", "Add Projects workflow"],
    ["c", COMMIT],
    ["clone", project.cloneUrls[0]],
    ["branch-name", "projects-workflow"],
    ["target-branch", "main"],
    ["merge-base", MERGE_BASE],
  ]);
});

test("projectPullRequestUpdateTags uses uppercase NIP-22 root tags", () => {
  const forkCloneUrl = `https://relay.example/git/${AUTHOR}/buzz`;
  const tags = projectPullRequestUpdateTags(
    project,
    { id: PR_ID, author: AUTHOR, cloneUrls: [forkCloneUrl] },
    COMMIT,
    MERGE_BASE,
  );

  assert.ok(tags.some((tag) => tag[0] === "E" && tag[1] === PR_ID));
  assert.ok(tags.some((tag) => tag[0] === "P" && tag[1] === AUTHOR));
  assert.ok(tags.some((tag) => tag[0] === "c" && tag[1] === COMMIT));
  assert.ok(
    tags.some(
      (tag) =>
        tag[0] === "clone" &&
        tag[1] === forkCloneUrl &&
        !tag.includes(project.cloneUrls[0]),
    ),
  );
});

test("projectPullRequestMergedTags records the pushed merge commit", () => {
  const tags = projectPullRequestMergedTags(
    project,
    { id: PR_ID, author: AUTHOR },
    COMMIT,
  );

  assert.deepEqual(tags, [
    ["e", PR_ID, "", "root"],
    ["a", project.repoAddress],
    ["p", OWNER],
    ["p", AUTHOR],
    ["merge-commit", COMMIT],
    ["r", COMMIT],
  ]);
});

test("GitHub create never calls the Buzz pull request publisher", async () => {
  const calls = { github: 0, buzz: 0 };
  const project = {
    id: "p1",
    owner: OWNER,
    repoAddress: `30617:${OWNER}:app`,
    cloneUrls: ["https://github.com/acme/app"],
  };
  const id = await createProjectPullRequestWith(
    project,
    {
      title: "Add docs",
      body: "Details",
      branch: "feature/readme",
      targetBranch: "main",
      commit: COMMIT,
      mergeBase: MERGE_BASE,
      reviewers: [REVIEWER],
    },
    {
      createGithub: async (input) => {
        calls.github += 1;
        assert.deepEqual(input, {
          cloneUrl: "https://github.com/acme/app",
          title: "Add docs",
          body: "Details",
          head: "feature/readme",
          base: "main",
        });
        return { number: 44 };
      },
      publishBuzz: async () => {
        calls.buzz += 1;
        return "e".repeat(64);
      },
    },
  );
  assert.equal(id, "44");
  assert.deepEqual(calls, { github: 1, buzz: 0 });
});

test("Buzz create never calls the GitHub creator", async () => {
  const calls = { github: 0, buzz: 0 };
  const id = await createProjectPullRequestWith(
    project,
    {
      title: "Buzz PR",
      body: "",
      branch: "feature",
      targetBranch: "main",
      commit: COMMIT,
      mergeBase: null,
      reviewers: [],
    },
    {
      createGithub: async () => {
        calls.github += 1;
        return { number: 1 };
      },
      publishBuzz: async () => {
        calls.buzz += 1;
        return PR_ID;
      },
    },
  );
  assert.equal(id, PR_ID);
  assert.deepEqual(calls, { github: 0, buzz: 1 });
});

test("GitHub create invalidates only its repository pull request query", () => {
  assert.deepEqual(
    projectPullRequestInvalidationKeys({
      id: "p1",
      cloneUrls: ["https://github.com/acme/app"],
    }),
    [["project", "p1", "pull-requests"]],
  );
});

test("Buzz create preserves all existing invalidations", () => {
  assert.deepEqual(
    projectPullRequestInvalidationKeys({
      id: "p2",
      cloneUrls: project.cloneUrls,
    }),
    [
      ["project", "p2", "pull-requests"],
      ["projects", "work-items"],
      ["projects", "activity-summaries"],
    ],
  );
});

test("Nostr tag builders refuse a numeric GitHub pull request id", () => {
  assert.throws(
    () =>
      projectPullRequestUpdateTags(
        project,
        { id: "42", author: AUTHOR, cloneUrls: project.cloneUrls },
        COMMIT,
        null,
      ),
    /64-hex Nostr event id/,
  );
  assert.throws(
    () =>
      projectPullRequestMergedTags(
        project,
        { id: "42", author: AUTHOR },
        COMMIT,
      ),
    /64-hex Nostr event id/,
  );
});
