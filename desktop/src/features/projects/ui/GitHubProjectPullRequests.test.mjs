import assert from "node:assert/strict";
import { test } from "node:test";

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterContextProvider,
} from "@tanstack/react-router";

import { Tabs } from "@/shared/ui/tabs";
import { TooltipProvider } from "@/shared/ui/tooltip";
import {
  GitHubPullRequestDetail,
  GitHubPullRequestDetailHeader,
  GitHubPullRequestMetaRail,
  GitHubPullRequestRow,
} from "./GitHubProjectPullRequests.tsx";
import { PullRequestTabsList } from "./ProjectWorkspaceTabList.tsx";

const ssrRouter = createRouter({
  history: createMemoryHistory({ initialEntries: ["/"] }),
  routeTree: createRootRoute({}),
});

function renderGithub(node) {
  return renderToStaticMarkup(
    React.createElement(
      RouterContextProvider,
      { router: ssrRouter },
      React.createElement(TooltipProvider, null, node),
    ),
  );
}

const pullRequest = {
  id: "42",
  title: "Fix login",
  content: "PR body from GitHub",
  tags: [],
  author: "ada",
  authorAvatarUrl: "https://avatars.githubusercontent.com/u/1",
  createdAt: 1_704_166_645,
  repoAddress: `30617:${"a".repeat(64)}:app`,
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
  branchName: "feature",
  targetBranch: "develop",
  initialCommit: "d".repeat(40),
  commit: "d".repeat(40),
  cloneUrls: ["https://github.com/acme/app"],
  updateCount: 0,
  updatedAt: 1_704_253_045,
  updates: [],
  comments: [],
  commentCount: 3,
  headRepoFullName: "acme/app",
  htmlUrl: "https://github.com/acme/app/pull/42",
};

const githubComment = {
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
};

test("GitHub row renders #N, login, host-native branches, and status", () => {
  const html = renderToStaticMarkup(
    React.createElement(GitHubPullRequestRow, {
      onOpen() {},
      pullRequest,
    }),
  );
  assert.match(html, /#42/);
  assert.match(html, /ada/);
  assert.match(html, /feature/);
  assert.match(html, /develop/);
  assert.match(html, /Open/);
});

test("GitHub conversation renders body and login comments without write chrome", () => {
  const commentsQuery = {
    data: [githubComment],
    error: null,
    isError: false,
    isLoading: false,
    refetch: async () => {},
  };
  const html = renderGithub(
    React.createElement(
      React.Fragment,
      null,
      React.createElement(GitHubPullRequestDetailHeader, { pullRequest }),
      React.createElement(GitHubPullRequestDetail, {
        commentsQuery,
        mode: "conversation",
        onSelectedPullRequestIdChange() {},
        pullRequest,
      }),
      React.createElement(GitHubPullRequestMetaRail, { pullRequest }),
    ),
  );
  assert.match(html, /PR body from GitHub/);
  assert.match(html, /Looks good\./);
  assert.match(html, /grace/);
  for (const forbidden of [
    "Merge",
    "Request changes",
    "Reviewers",
    "Discussed in channels",
    "Add a comment",
  ]) {
    assert.equal(html.includes(forbidden), false, forbidden);
  }
});

test("GitHub pull tabs hide Files changed and pin one commit", () => {
  const html = renderToStaticMarkup(
    React.createElement(
      Tabs,
      { defaultValue: "pr-conversation" },
      React.createElement(PullRequestTabsList, {
        conversationCount: 5,
        filesCount: 99,
        hideFiles: true,
        pullRequest,
      }),
    ),
  );
  assert.match(html, /Conversation/);
  assert.match(html, /Commits/);
  assert.equal(html.includes("Files changed"), false);
  assert.ok(html.includes(">5<"), "conversation badge uses the supplied count");
  assert.ok(html.includes(">1<"), "GitHub commit badge is exactly one");
});
