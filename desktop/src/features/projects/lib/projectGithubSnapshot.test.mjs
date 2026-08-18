import assert from "node:assert/strict";
import { test } from "node:test";
import {
  fetchProjectRepoSnapshotWith,
  githubRemoteSnapshotEnabled,
} from "./projectGithubSnapshot.ts";

function repository(cloneUrl) {
  return {
    id: "owner:app",
    dtag: "app",
    name: "app",
    description: "",
    cloneUrls: [cloneUrl],
    webUrl: null,
    owner: "ab".repeat(32),
    contributors: [],
    createdAt: 0,
    status: "active",
    defaultBranch: "main",
    repoAddress: `30617:${"ab".repeat(32)}:app`,
  };
}

test("GitHub clone URL uses the GitHub snapshot command", async () => {
  let githubCalls = 0;
  let buzzCalls = 0;
  const snapshot = {
    latestCommit: null,
    commits: [],
    files: [],
    contributors: [],
  };
  const result = await fetchProjectRepoSnapshotWith(
    repository("https://github.com/acme/app"),
    "develop",
    null,
    null,
    {
      loadGithub: async ({ cloneUrl, ref }) => {
        githubCalls += 1;
        assert.equal(cloneUrl, "https://github.com/acme/app");
        assert.equal(ref, "develop");
        return snapshot;
      },
      loadBuzz: async () => {
        buzzCalls += 1;
        return snapshot;
      },
    },
  );
  assert.equal(githubCalls, 1);
  assert.equal(buzzCalls, 0);
  assert.equal(result, snapshot);
});

test("Buzz clone URL does not call the GitHub snapshot command", async () => {
  let githubCalls = 0;
  let buzzCalls = 0;
  const cloneUrl = `https://relay.example/git/${"ab".repeat(32)}/app`;
  await fetchProjectRepoSnapshotWith(repository(cloneUrl), "main", null, null, {
    loadGithub: async () => {
      githubCalls += 1;
      return { latestCommit: null, commits: [], files: [], contributors: [] };
    },
    loadBuzz: async () => {
      buzzCalls += 1;
      return { latestCommit: null, commits: [], files: [], contributors: [] };
    },
  });
  assert.equal(githubCalls, 0);
  assert.equal(buzzCalls, 1);
});

test("GitHub snapshot ignores Buzz tag, nostr refs, and PR source clone URLs", async () => {
  let seenRef = null;
  let seenCloneUrl = null;
  await fetchProjectRepoSnapshotWith(
    repository("https://github.com/acme/app"),
    "develop",
    {
      id: "pr1",
      cloneUrls: ["https://github.com/fork/app"],
      commit: "c".repeat(40),
    },
    { name: "v1", commit: "d".repeat(40) },
    {
      loadGithub: async ({ cloneUrl, ref }) => {
        seenCloneUrl = cloneUrl;
        seenRef = ref;
        return { latestCommit: null, commits: [], files: [], contributors: [] };
      },
      loadBuzz: async () => {
        throw new Error("buzz snapshot must not run");
      },
    },
  );
  assert.equal(seenCloneUrl, "https://github.com/acme/app");
  assert.equal(seenRef, "develop");
});

test("snapshot query enablement requires G1 success on GitHub", () => {
  assert.equal(
    githubRemoteSnapshotEnabled({
      cloneUrl: "https://github.com/acme/app",
      buzzHost: false,
      githubStateReady: false,
    }),
    false,
  );
  assert.equal(
    githubRemoteSnapshotEnabled({
      cloneUrl: "https://github.com/acme/app",
      buzzHost: false,
      githubStateReady: true,
    }),
    true,
  );
  assert.equal(
    githubRemoteSnapshotEnabled({
      cloneUrl: `https://relay.example/git/${"ab".repeat(32)}/app`,
      buzzHost: true,
      githubStateReady: false,
    }),
    true,
  );
});
