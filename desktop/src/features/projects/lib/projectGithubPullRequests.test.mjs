import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchProjectPullRequestsWith,
  githubPullRequestBranchLabel,
  githubPullRequestCommentsRequest,
  githubPullRequestDetailTab,
  githubPullRequestId,
  mapGithubPullRequestComment,
  mapGithubPullRequestToProjectPullRequest,
  parseGithubPullRequestNumber,
  pullRequestIdentityPubkeys,
  requireNostrPullRequestId,
  selectedGithubPullRequestAfterListLoad,
} from "./projectGithubPullRequests.ts";

const OWNER = "a".repeat(64);
const REPO_ADDRESS = `30617:${OWNER}:app`;
const dto = {
  number: 42,
  title: "Add docs",
  body: "Details",
  html_url: "https://github.com/acme/app/pull/42",
  draft: false,
  created_at: 1_704_166_645,
  updated_at: 1_704_253_045,
  comments: 3,
  user: {
    login: "ada",
    avatar_url: "https://avatars.githubusercontent.com/u/1",
  },
  head: {
    ref: "feature/readme",
    sha: "1".repeat(40),
    repo: { full_name: "acme/app" },
  },
  base: { ref: "main", repo: { full_name: "acme/app" } },
};

test("GitHub pull mapper fills the complete shared contract", () => {
  assert.deepEqual(
    mapGithubPullRequestToProjectPullRequest(
      dto,
      REPO_ADDRESS,
      "https://github.com/acme/app",
    ),
    {
      id: "42",
      title: "Add docs",
      content: "Details",
      tags: [],
      author: "ada",
      authorAvatarUrl: "https://avatars.githubusercontent.com/u/1",
      createdAt: 1_704_166_645,
      repoAddress: REPO_ADDRESS,
      channelId: null,
      originAgentName: null,
      labels: [],
      recipients: [],
      reviewers: [],
      approvals: [],
      changeRequests: [],
      status: "Open",
      statusEventId: null,
      statusCreatedAt: null,
      branchName: "feature/readme",
      targetBranch: "main",
      headRepoFullName: "acme/app",
      initialCommit: "1".repeat(40),
      commit: "1".repeat(40),
      cloneUrls: ["https://github.com/acme/app"],
      updateCount: 0,
      updatedAt: 1_704_253_045,
      updates: [],
      comments: [],
      commentCount: 3,
      htmlUrl: "https://github.com/acme/app/pull/42",
    },
  );
  assert.equal(
    mapGithubPullRequestToProjectPullRequest(
      { ...dto, draft: true },
      REPO_ADDRESS,
      "https://github.com/acme/app",
    ).status,
    "Draft",
  );
});

test("GitHub pull ids accept only positive safe integers", () => {
  assert.equal(githubPullRequestId(42), "42");
  assert.equal(parseGithubPullRequestNumber("42"), 42);
  for (const value of ["0", "01", "0x2", "9007199254740992", "e".repeat(64)]) {
    assert.equal(parseGithubPullRequestNumber(value), null, value);
  }
  for (const number of [0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(
      () => githubPullRequestId(number),
      /invalid pull request number/,
    );
  }
});

test("GitHub comment mapper keeps login and avatar without pubkey conversion", () => {
  assert.deepEqual(
    mapGithubPullRequestComment({
      id: 9,
      body: "Looks good.",
      html_url: "https://github.com/acme/app/pull/42#issuecomment-9",
      created_at: 1_704_253_100,
      user: {
        login: "grace",
        avatar_url: "https://avatars.githubusercontent.com/u/2",
      },
    }),
    {
      id: "9",
      content: "Looks good.",
      tags: [],
      author: "grace",
      authorAvatarUrl: "https://avatars.githubusercontent.com/u/2",
      createdAt: 1_704_253_100,
      commit: null,
      anchor: null,
      inlineCommentStatus: null,
      isInlineComment: false,
      isApproval: false,
      isChangeRequest: false,
      isReviewRequest: false,
      isTrustedReviewDecision: false,
      isTrustedReviewRequest: false,
      reviewDecision: null,
      reviewDecisionStatus: null,
      reviewerPubkeys: [],
    },
  );
});

test("host routing invokes only GitHub for github.com", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectPullRequestsWith(
    {
      id: "p1",
      repoAddress: REPO_ADDRESS,
      cloneUrls: ["https://github.com/acme/app"],
    },
    {
      loadGithub: async ({ cloneUrl }) => {
        calls.github += 1;
        assert.equal(cloneUrl, "https://github.com/acme/app");
        return { pulls: [dto], has_more: true };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [];
      },
    },
  );
  assert.deepEqual(calls, { github: 1, buzz: 0 });
  assert.equal(result.pullRequests[0].id, "42");
  assert.equal(result.hasMore, true);
});

test("host routing invokes only Nostr for a Buzz clone URL", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectPullRequestsWith(
    {
      id: "p2",
      repoAddress: REPO_ADDRESS,
      cloneUrls: [`https://relay.example/git/${OWNER}/app`],
    },
    {
      loadGithub: async () => {
        calls.github += 1;
        return { pulls: [dto], has_more: false };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [{ id: "e".repeat(64) }];
      },
    },
  );
  assert.deepEqual(calls, { github: 0, buzz: 1 });
  assert.equal(result.pullRequests[0].id, "e".repeat(64));
  assert.equal(result.hasMore, false);
});

test("branch labels prefix only an inbound fork owner", () => {
  const pull = mapGithubPullRequestToProjectPullRequest(
    { ...dto, head: { ...dto.head, repo: { full_name: "fork-owner/app" } } },
    REPO_ADDRESS,
    "https://github.com/acme/app",
  );
  assert.equal(
    githubPullRequestBranchLabel(pull, "https://github.com/acme/app"),
    "fork-owner:feature/readme",
  );
  assert.equal(
    githubPullRequestBranchLabel(
      { ...pull, headRepoFullName: "ACME/APP" },
      "https://github.com/acme/app",
    ),
    "feature/readme",
  );
});

test("identity collection drops GitHub logins and keeps lowercase Nostr pubkeys", () => {
  const github = mapGithubPullRequestToProjectPullRequest(
    dto,
    REPO_ADDRESS,
    "https://github.com/acme/app",
  );
  const nostr = {
    ...github,
    id: "f".repeat(64),
    author: "A".repeat(64),
    recipients: ["B".repeat(64)],
    reviewers: ["C".repeat(64)],
    comments: [
      {
        ...mapGithubPullRequestComment({
          id: 1,
          body: "x",
          html_url: "",
          created_at: 1,
          user: { login: "x", avatar_url: "" },
        }),
        author: "D".repeat(64),
      },
    ],
  };
  assert.deepEqual(pullRequestIdentityPubkeys([github]), []);
  assert.deepEqual(pullRequestIdentityPubkeys([nostr]), [
    "a".repeat(64),
    "b".repeat(64),
    "c".repeat(64),
    "d".repeat(64),
  ]);
});

test("comment requests validate host, number, and exact query key", () => {
  assert.deepEqual(
    githubPullRequestCommentsRequest(
      { id: "p1", cloneUrls: ["https://github.com/acme/app"] },
      "42",
    ),
    {
      cloneUrl: "https://github.com/acme/app",
      number: 42,
      queryKey: ["project", "p1", "pull-requests", 42, "comments"],
    },
  );
  assert.equal(
    githubPullRequestCommentsRequest(
      { id: "p1", cloneUrls: ["https://github.com/acme/app"] },
      "0",
    ),
    null,
  );
});

test("Nostr write guard refuses GitHub numbers", () => {
  assert.equal(requireNostrPullRequestId("f".repeat(64)), "f".repeat(64));
  assert.throws(() => requireNostrPullRequestId("42"), /64-hex Nostr event id/);
});

test("created selection waits for a settled list before clearing", () => {
  assert.equal(
    selectedGithubPullRequestAfterListLoad({
      selectedPullRequestId: "43",
      pullRequestIds: ["42"],
      isSuccess: true,
      isFetching: true,
    }),
    "43",
  );
  assert.equal(
    selectedGithubPullRequestAfterListLoad({
      selectedPullRequestId: "43",
      pullRequestIds: ["42", "43"],
      isSuccess: true,
      isFetching: false,
    }),
    "43",
  );
  assert.equal(
    selectedGithubPullRequestAfterListLoad({
      selectedPullRequestId: "43",
      pullRequestIds: ["42"],
      isSuccess: true,
      isFetching: false,
    }),
    null,
  );
});

test("GitHub detail snaps stale Files changed state to Conversation", () => {
  assert.equal(githubPullRequestDetailTab("pr-files", true), "pr-conversation");
  assert.equal(githubPullRequestDetailTab("pr-files", false), "pr-files");
  assert.equal(githubPullRequestDetailTab("pr-commits", true), "pr-commits");
});
