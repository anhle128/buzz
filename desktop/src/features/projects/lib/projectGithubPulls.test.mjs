import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchProjectPullRequestsWith,
  hasOpenPullRequestForBranches,
  githubRepoFullNameFromCloneUrl,
  githubPullRequestBranchLabel,
  githubPullRequestCommentsRequest,
  githubPullRequestConversationCount,
  githubPullRequestId,
  mapGithubCommentToProjectPullRequestComment,
  mapGithubPullRequestToProjectPullRequest,
  parseGithubPullRequestNumber,
  pullRequestDisplayNumber,
  pullRequestHeadBelongsToRepository,
  pullRequestIdentityPubkeys,
  requireBuzzPullRequestEventId,
  selectedGithubPullRequestAfterListLoad,
} from "./projectGithubPulls.ts";

const REPO_ADDRESS = `30617:${"a".repeat(64)}:app`;
const dto = {
  number: 42,
  title: "Fix login",
  body: "PR body",
  html_url: "https://github.com/acme/app/pull/42",
  draft: false,
  comments: 3,
  created_at: 1_704_166_645,
  updated_at: 1_704_253_045,
  user: {
    login: "ada",
    avatar_url: "https://avatars.githubusercontent.com/u/1",
  },
  head: {
    ref: "feature",
    sha: "d".repeat(40),
    repo: { full_name: "acme/app" },
  },
  base: {
    ref: "develop",
    repo: { full_name: "acme/app" },
  },
};

test("GitHub pull mapper fills the complete ProjectPullRequest contract", () => {
  const pullRequest = mapGithubPullRequestToProjectPullRequest(dto, {
    repoAddress: REPO_ADDRESS,
    cloneUrl: "https://github.com/acme/app",
  });
  assert.equal(pullRequest.id, "42");
  assert.equal(pullRequest.title, "Fix login");
  assert.equal(pullRequest.content, "PR body");
  assert.equal(pullRequest.author, "ada");
  assert.equal(
    pullRequest.authorAvatarUrl,
    "https://avatars.githubusercontent.com/u/1",
  );
  assert.equal(pullRequest.status, "Open");
  assert.equal(pullRequest.branchName, "feature");
  assert.equal(pullRequest.targetBranch, "develop");
  assert.equal(pullRequest.commit, "d".repeat(40));
  assert.equal(pullRequest.headRepoFullName, "acme/app");
  assert.equal(pullRequest.htmlUrl, "https://github.com/acme/app/pull/42");
  assert.deepEqual(pullRequest.comments, []);
  assert.equal(pullRequest.commentCount, 3);
  assert.equal(pullRequest.updateCount, 0);
  assert.deepEqual(pullRequest.reviewers, []);
  assert.deepEqual(pullRequest.cloneUrls, ["https://github.com/acme/app"]);
});

test("GitHub draft maps to Draft and never to Open", () => {
  const pullRequest = mapGithubPullRequestToProjectPullRequest(
    { ...dto, draft: true },
    { repoAddress: REPO_ADDRESS, cloneUrl: "https://github.com/acme/app" },
  );
  assert.equal(pullRequest.status, "Draft");
});

test("GitHub pull ids reject non-positive, fractional, and unsafe numbers", () => {
  assert.equal(githubPullRequestId(42), "42");
  for (const number of [0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(
      () => githubPullRequestId(number),
      /GitHub returned an invalid pull request number/,
    );
    assert.throws(
      () =>
        mapGithubPullRequestToProjectPullRequest(
          { ...dto, number },
          {
            repoAddress: REPO_ADDRESS,
            cloneUrl: "https://github.com/acme/app",
          },
        ),
      /GitHub returned an invalid pull request number/,
    );
  }
});

test("GitHub comment mapper keeps login and avatar without pubkey conversion", () => {
  assert.deepEqual(
    mapGithubCommentToProjectPullRequestComment({
      id: 9,
      body: "Looks good.",
      html_url: "https://github.com/acme/app/issues/42#issuecomment-9",
      created_at: 1_704_253_100,
      user: {
        login: "grace",
        avatar_url: "https://avatars.githubusercontent.com/u/3",
      },
    }),
    {
      id: "9",
      content: "Looks good.",
      tags: [],
      author: "grace",
      authorAvatarUrl: "https://avatars.githubusercontent.com/u/3",
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
      loadGithub: async (input) => {
        calls.github += 1;
        assert.deepEqual(input, { cloneUrl: "https://github.com/acme/app" });
        return { pulls: [dto], has_more: true };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [];
      },
    },
  );
  assert.equal(calls.github, 1);
  assert.equal(calls.buzz, 0);
  assert.equal(result.pullRequests[0].id, "42");
  assert.equal(result.hasMore, true);
});

test("host routing invokes only Nostr for a Buzz clone URL", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectPullRequestsWith(
    {
      id: "p2",
      repoAddress: REPO_ADDRESS,
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
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
  assert.equal(calls.github, 0);
  assert.equal(calls.buzz, 1);
  assert.equal(result.pullRequests[0].id, "e".repeat(64));
  assert.equal(result.hasMore, false);
});

test("GitHub pull number parser accepts only positive safe decimal integers", () => {
  assert.equal(parseGithubPullRequestNumber("42"), 42);
  assert.equal(parseGithubPullRequestNumber("0"), null);
  assert.equal(parseGithubPullRequestNumber("01"), null);
  assert.equal(parseGithubPullRequestNumber("0x2"), null);
  assert.equal(parseGithubPullRequestNumber("9007199254740992"), null);
  assert.equal(parseGithubPullRequestNumber("e".repeat(64)), null);
  assert.equal(pullRequestDisplayNumber("42"), "42");
  assert.equal(pullRequestDisplayNumber("e".repeat(64)), "eeeeeeee");
  assert.equal(githubPullRequestId(42), "42");
});

test("Nostr-only writes reject GitHub decimal ids", () => {
  assert.equal(requireBuzzPullRequestEventId("e".repeat(64)), "e".repeat(64));
  assert.throws(
    () => requireBuzzPullRequestEventId("42"),
    /cannot be mutated through Nostr/,
  );
});

test("identity collection drops GitHub logins and keeps lowercase Nostr pubkeys", () => {
  const github = mapGithubPullRequestToProjectPullRequest(dto, {
    repoAddress: REPO_ADDRESS,
    cloneUrl: "https://github.com/acme/app",
  });
  const nostr = {
    ...github,
    id: "f".repeat(64),
    author: "A".repeat(64),
    reviewers: ["B".repeat(64)],
    comments: [
      {
        ...mapGithubCommentToProjectPullRequestComment({
          id: 1,
          body: "x",
          html_url: "https://github.com/acme/app/issues/1#issuecomment-1",
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
    "d".repeat(64),
  ]);
});

test("fork heads render owner:branch and empty head repos stay same-repo", () => {
  const same = mapGithubPullRequestToProjectPullRequest(dto, {
    repoAddress: REPO_ADDRESS,
    cloneUrl: "https://github.com/acme/app",
  });
  assert.equal(githubPullRequestBranchLabel(same), "feature → develop");
  const fork = mapGithubPullRequestToProjectPullRequest(
    {
      ...dto,
      head: { ...dto.head, repo: { full_name: "other/app" } },
    },
    { repoAddress: REPO_ADDRESS, cloneUrl: "https://github.com/acme/app" },
  );
  assert.equal(githubPullRequestBranchLabel(fork), "other:feature → develop");
  const deleted = mapGithubPullRequestToProjectPullRequest(
    {
      ...dto,
      head: { ...dto.head, repo: { full_name: "" } },
    },
    { repoAddress: REPO_ADDRESS, cloneUrl: "https://github.com/acme/app" },
  );
  assert.equal(githubPullRequestBranchLabel(deleted), "feature → develop");
  assert.equal(
    githubRepoFullNameFromCloneUrl("git@github.com:acme/app.git"),
    "acme/app",
  );
  assert.equal(
    githubRepoFullNameFromCloneUrl("ssh://git@github.com/acme/app.git"),
    "acme/app",
  );
  assert.equal(
    githubRepoFullNameFromCloneUrl("https://github.com/acme/app/extra"),
    null,
  );
});

test("only target-repository heads participate in branch workflows", () => {
  const repository = {
    cloneUrls: ["https://github.com/acme/app"],
    defaultBranch: "develop",
  };
  const same = mapGithubPullRequestToProjectPullRequest(dto, {
    repoAddress: REPO_ADDRESS,
    cloneUrl: repository.cloneUrls[0],
  });
  const fork = mapGithubPullRequestToProjectPullRequest(
    {
      ...dto,
      head: { ...dto.head, repo: { full_name: "other/app" } },
    },
    { repoAddress: REPO_ADDRESS, cloneUrl: repository.cloneUrls[0] },
  );
  const deletedFork = { ...fork, headRepoFullName: null };

  assert.equal(pullRequestHeadBelongsToRepository(same, repository), true);
  assert.equal(
    pullRequestHeadBelongsToRepository(
      { ...same, headRepoFullName: "ACME/APP" },
      repository,
    ),
    true,
  );
  assert.equal(pullRequestHeadBelongsToRepository(fork, repository), false);
  assert.equal(
    pullRequestHeadBelongsToRepository(deletedFork, repository),
    false,
  );
  assert.equal(
    hasOpenPullRequestForBranches([fork], repository, "feature", "develop"),
    false,
  );
  assert.equal(
    hasOpenPullRequestForBranches([same], repository, "feature", "develop"),
    true,
  );
  assert.equal(
    pullRequestHeadBelongsToRepository(
      { ...same, headRepoFullName: null },
      {
        cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
      },
    ),
    true,
  );
});

test("conversation badge uses the greater of list count and loaded comments", () => {
  assert.equal(
    githubPullRequestConversationCount({ commentCount: 3, commentsLength: 0 }),
    3,
  );
  assert.equal(
    githubPullRequestConversationCount({ commentCount: 3, commentsLength: 5 }),
    5,
  );
});

test("comment request is enabled only for GitHub numeric ids", () => {
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
      "e".repeat(64),
    ),
    null,
  );
});

test("selection survives refetch and clears when the number is absent", () => {
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
      pullRequestIds: ["42"],
      isSuccess: true,
      isFetching: false,
    }),
    null,
  );
});
