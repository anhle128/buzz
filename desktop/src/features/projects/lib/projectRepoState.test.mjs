import assert from "node:assert/strict";
import { test } from "node:test";

import { isGitHubCloneUrl } from "./projectGitError.ts";
import {
  fetchRepoStateWith,
  githubRepositoryStateUnresolved,
} from "./projectRepoState.ts";

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

test("treats GitHub pending and error as unresolved state", () => {
  assert.equal(
    githubRepositoryStateUnresolved(true, { isPending: true, isError: false }),
    true,
  );
  assert.equal(
    githubRepositoryStateUnresolved(true, { isPending: false, isError: true }),
    true,
  );
  assert.equal(
    githubRepositoryStateUnresolved(true, { isPending: false, isError: false }),
    false,
  );
  assert.equal(
    githubRepositoryStateUnresolved(false, { isPending: true, isError: false }),
    false,
  );
});

test("isGitHubCloneUrl accepts https and ssh github hosts", () => {
  assert.equal(isGitHubCloneUrl("https://github.com/acme/app"), true);
  assert.equal(isGitHubCloneUrl("git@github.com:acme/app.git"), true);
  assert.equal(
    isGitHubCloneUrl(`https://relay.example/git/${"ab".repeat(32)}/app`),
    false,
  );
});

test("uses GitHub state for github.com clone URLs", async () => {
  let loadGithubCalls = 0;
  let loadBuzzCalls = 0;
  const githubState = {
    head: "develop",
    branches: [{ name: "develop", commit: "a".repeat(40) }],
    tags: [],
    updatedAt: 1,
  };

  const result = await fetchRepoStateWith(
    repository("https://github.com/acme/app"),
    {
      loadGithub: async (cloneUrl) => {
        loadGithubCalls += 1;
        assert.equal(cloneUrl, "https://github.com/acme/app");
        return githubState;
      },
      loadBuzz: async () => {
        loadBuzzCalls += 1;
        return null;
      },
    },
  );

  assert.equal(loadGithubCalls, 1);
  assert.equal(loadBuzzCalls, 0);
  assert.deepEqual(result, githubState);
});

test("uses kind 30618 for Buzz-hosted clone URLs", async () => {
  let loadGithubCalls = 0;
  let loadBuzzCalls = 0;
  const buzzState = {
    head: "main",
    branches: [{ name: "main", commit: "b".repeat(40) }],
    tags: [],
    updatedAt: 2,
  };
  const cloneUrl = `https://relay.example/git/${"ab".repeat(32)}/app`;

  const result = await fetchRepoStateWith(repository(cloneUrl), {
    loadGithub: async () => {
      loadGithubCalls += 1;
      return {
        head: "develop",
        branches: [],
        tags: [],
        updatedAt: 0,
      };
    },
    loadBuzz: async (project) => {
      loadBuzzCalls += 1;
      assert.equal(project.cloneUrls[0], cloneUrl);
      return buzzState;
    },
  });

  assert.equal(loadGithubCalls, 0);
  assert.equal(loadBuzzCalls, 1);
  assert.deepEqual(result, buzzState);
});
