import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchProjectIssuesWith,
  githubIssueCommentsRequest,
  githubIssueId,
  issueDisplayNumber,
  issueIdentityPubkeys,
  mapGithubCommentToProjectIssueComment,
  mapGithubIssueToProjectIssue,
  parseGithubIssueNumber,
} from "./projectGithubIssues.ts";

const REPO_ADDRESS = `30617:${"a".repeat(64)}:app`;
const dto = {
  number: 42,
  title: "Broken login",
  body: "Steps",
  state: "open",
  html_url: "https://github.com/acme/app/issues/42",
  comments: 3,
  created_at: 1_704_166_645,
  updated_at: 1_704_253_045,
  user: {
    login: "ada",
    avatar_url: "https://avatars.githubusercontent.com/u/1",
  },
  labels: ["bug"],
  assignees: [
    {
      login: "linus",
      avatar_url: "https://avatars.githubusercontent.com/u/2",
    },
  ],
};

test("GitHub issue mapper fills the complete ProjectIssue contract", () => {
  const issue = mapGithubIssueToProjectIssue(dto, REPO_ADDRESS);
  assert.deepEqual(issue, {
    id: "42",
    title: "Broken login",
    content: "Steps",
    tags: [],
    author: "ada",
    authorAvatarUrl: "https://avatars.githubusercontent.com/u/1",
    createdAt: 1_704_166_645,
    repoAddress: REPO_ADDRESS,
    channelId: null,
    originAgentName: null,
    labels: ["bug"],
    recipients: [],
    assignees: ["linus"],
    assigneeAvatars: { linus: "https://avatars.githubusercontent.com/u/2" },
    assigneeOperationHeads: {},
    status: "Open",
    statusEventId: null,
    updatedAt: 1_704_253_045,
    comments: [],
    commentCount: 3,
    htmlUrl: "https://github.com/acme/app/issues/42",
  });
});

test("GitHub issue ids reject non-positive and unsafe native numbers", () => {
  assert.equal(githubIssueId(42), "42");
  for (const number of [0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(
      () => githubIssueId(number),
      /GitHub returned an invalid issue number/,
    );
    assert.throws(
      () => mapGithubIssueToProjectIssue({ ...dto, number }, REPO_ADDRESS),
      /GitHub returned an invalid issue number/,
    );
  }
});

test("GitHub comment mapper keeps login and avatar without pubkey conversion", () => {
  assert.deepEqual(
    mapGithubCommentToProjectIssueComment({
      id: 9,
      body: "I can reproduce this.",
      html_url: "https://github.com/acme/app/issues/42#issuecomment-9",
      created_at: 1_704_253_100,
      user: {
        login: "grace",
        avatar_url: "https://avatars.githubusercontent.com/u/3",
      },
    }),
    {
      id: "9",
      content: "I can reproduce this.",
      tags: [],
      author: "grace",
      authorAvatarUrl: "https://avatars.githubusercontent.com/u/3",
      createdAt: 1_704_253_100,
    },
  );
});

test("host routing invokes only GitHub for github.com", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectIssuesWith(
    {
      id: "p1",
      repoAddress: REPO_ADDRESS,
      cloneUrls: ["https://github.com/acme/app"],
    },
    {
      loadGithub: async (input) => {
        calls.github += 1;
        assert.deepEqual(input, {
          cloneUrl: "https://github.com/acme/app",
          state: "open",
        });
        return { issues: [dto], has_more: true };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [];
      },
    },
  );
  assert.equal(calls.github, 1);
  assert.equal(calls.buzz, 0);
  assert.equal(result.issues[0].id, "42");
  assert.equal(result.hasMore, true);
});

test("host routing forwards closed state only to GitHub", async () => {
  let received = null;
  await fetchProjectIssuesWith(
    {
      id: "p1",
      repoAddress: REPO_ADDRESS,
      cloneUrls: ["https://github.com/acme/app"],
    },
    {
      loadGithub: async (input) => {
        received = input;
        return { issues: [], has_more: false };
      },
      loadBuzz: async () => {
        throw new Error("Buzz loader must not run");
      },
    },
    "closed",
  );
  assert.deepEqual(received, {
    cloneUrl: "https://github.com/acme/app",
    state: "closed",
  });
});

test("host routing invokes only Nostr for a Buzz clone URL", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectIssuesWith(
    {
      id: "p2",
      repoAddress: REPO_ADDRESS,
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
    },
    {
      loadGithub: async () => {
        calls.github += 1;
        return { issues: [dto], has_more: false };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [{ id: "e".repeat(64) }];
      },
    },
  );
  assert.equal(calls.github, 0);
  assert.equal(calls.buzz, 1);
  assert.equal(result.issues[0].id, "e".repeat(64));
  assert.equal(result.hasMore, false);
});

test("GitHub issue number parser accepts only positive safe decimal integers", () => {
  assert.equal(parseGithubIssueNumber("42"), 42);
  assert.equal(parseGithubIssueNumber("0"), null);
  assert.equal(parseGithubIssueNumber("01"), null);
  assert.equal(parseGithubIssueNumber("0x2"), null);
  assert.equal(parseGithubIssueNumber("9007199254740992"), null);
  assert.equal(parseGithubIssueNumber("e".repeat(64)), null);
  assert.equal(issueDisplayNumber("42"), "42");
  assert.equal(issueDisplayNumber("e".repeat(64)), "eeeeeeee");
});

test("identity collection drops GitHub logins and keeps lowercase Nostr pubkeys", () => {
  const github = mapGithubIssueToProjectIssue(dto, REPO_ADDRESS);
  const nostr = {
    ...github,
    id: "f".repeat(64),
    author: "A".repeat(64),
    recipients: ["B".repeat(64)],
    assignees: ["C".repeat(64)],
    comments: [
      {
        ...mapGithubCommentToProjectIssueComment({
          id: 1,
          body: "x",
          html_url: "https://github.com/acme/app/issues/42#issuecomment-1",
          created_at: 1,
          user: { login: "x", avatar_url: "" },
        }),
        author: "D".repeat(64),
      },
    ],
  };
  assert.deepEqual(issueIdentityPubkeys([github]), []);
  assert.deepEqual(issueIdentityPubkeys([nostr]), [
    "a".repeat(64),
    "b".repeat(64),
    "c".repeat(64),
    "d".repeat(64),
  ]);
});

test("GitHub comment request validates host, number, and exact query key", () => {
  assert.deepEqual(
    githubIssueCommentsRequest(
      { id: "p1", cloneUrls: ["https://github.com/acme/app"] },
      "42",
    ),
    {
      cloneUrl: "https://github.com/acme/app",
      number: 42,
      queryKey: ["project", "p1", "issues", 42, "comments"],
    },
  );
  assert.equal(
    githubIssueCommentsRequest(
      { id: "p1", cloneUrls: ["https://github.com/acme/app"] },
      "0",
    ),
    null,
  );
  assert.equal(
    githubIssueCommentsRequest(
      {
        id: "p2",
        cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
      },
      "42",
    ),
    null,
  );
});
