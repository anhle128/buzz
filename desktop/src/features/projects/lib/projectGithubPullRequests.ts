import { useQuery } from "@tanstack/react-query";

import type { Repository } from "@/features/projects/projectModels";
import type {
  ProjectPullRequest,
  ProjectPullRequestComment,
} from "@/features/projects/projectPullRequests.mjs";
import {
  listGithubPullRequestComments,
  type GithubPullRequestCommentDto,
  type GithubPullRequestDto,
  type GithubPullRequestListDto,
} from "@/shared/api/projectGithubPulls";
import { isGitHubCloneUrl } from "./projectGitError";

/** Host-routed pull-request list consumed by one repository PR tab. */
export type ProjectPullRequestsResult = {
  pullRequests: ProjectPullRequest[];
  hasMore: boolean;
};

/** Convert a positive safe GitHub PR number into its decimal selection id. */
export function githubPullRequestId(number: number): string {
  if (!Number.isSafeInteger(number) || number <= 0) {
    throw new Error("GitHub returned an invalid pull request number.");
  }
  return String(number);
}

/** Parse a positive safe decimal GitHub PR selection id. */
export function parseGithubPullRequestNumber(
  value: string | null | undefined,
): number | null {
  if (!value || !/^[1-9][0-9]*$/.test(value)) return null;
  const number = Number(value);
  return Number.isSafeInteger(number) ? number : null;
}

/** Map one bounded GitHub PR onto the shared Projects PR model. */
export function mapGithubPullRequestToProjectPullRequest(
  dto: GithubPullRequestDto,
  repoAddress: string,
  cloneUrl: string,
): ProjectPullRequest {
  return {
    id: githubPullRequestId(dto.number),
    title: dto.title,
    content: dto.body,
    tags: [],
    author: dto.user.login,
    authorAvatarUrl: dto.user.avatar_url,
    createdAt: dto.created_at,
    repoAddress,
    channelId: null,
    originAgentName: null,
    labels: [],
    recipients: [],
    reviewers: [],
    approvals: [],
    changeRequests: [],
    status: dto.draft ? "Draft" : "Open",
    statusEventId: null,
    statusCreatedAt: null,
    branchName: dto.head.ref,
    targetBranch: dto.base.ref,
    headRepoFullName: dto.head.repo.full_name,
    initialCommit: dto.head.sha,
    commit: dto.head.sha,
    cloneUrls: [cloneUrl],
    updateCount: 0,
    updatedAt: dto.updated_at,
    updates: [],
    comments: [],
    commentCount: dto.comments,
    htmlUrl: dto.html_url,
  };
}

/** Map one bounded GitHub issue comment onto the shared PR comment model. */
export function mapGithubPullRequestComment(
  dto: GithubPullRequestCommentDto,
): ProjectPullRequestComment {
  return {
    id: String(dto.id),
    content: dto.body,
    tags: [],
    author: dto.user.login,
    authorAvatarUrl: dto.user.avatar_url,
    createdAt: dto.created_at,
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
}

/** Route one repository to exactly one pull-request backend. */
export async function fetchProjectPullRequestsWith(
  project: Pick<Repository, "id" | "repoAddress" | "cloneUrls">,
  loaders: {
    loadGithub: (input: {
      cloneUrl: string;
    }) => Promise<GithubPullRequestListDto>;
    loadBuzz: () => Promise<ProjectPullRequest[]>;
  },
): Promise<ProjectPullRequestsResult> {
  const cloneUrl = project.cloneUrls[0] ?? "";
  if (isGitHubCloneUrl(cloneUrl)) {
    const page = await loaders.loadGithub({ cloneUrl });
    return {
      pullRequests: page.pulls.map((pull) =>
        mapGithubPullRequestToProjectPullRequest(
          pull,
          project.repoAddress,
          cloneUrl,
        ),
      ),
      hasMore: page.has_more === true,
    };
  }
  return { pullRequests: await loaders.loadBuzz(), hasMore: false };
}

function githubRepoFullNameFromCloneUrl(cloneUrl: string): string | null {
  try {
    const normalized = cloneUrl.startsWith("git@github.com:")
      ? cloneUrl.replace("git@github.com:", "ssh://git@github.com/")
      : cloneUrl;
    const url = new URL(normalized);
    if (url.hostname.toLowerCase() !== "github.com") return null;
    const [owner, repo] = url.pathname
      .replace(/\.git$/i, "")
      .split("/")
      .filter(Boolean);
    return owner && repo ? `${owner}/${repo}` : null;
  } catch {
    return null;
  }
}

/** Display the fork owner prefix only when the PR head repository differs. */
export function githubPullRequestBranchLabel(
  pullRequest: Pick<ProjectPullRequest, "branchName" | "headRepoFullName">,
  targetCloneUrl: string,
): string {
  const branchName = pullRequest.branchName ?? "";
  const headRepo = pullRequest.headRepoFullName;
  if (!headRepo) return branchName;
  const targetRepo = githubRepoFullNameFromCloneUrl(targetCloneUrl);
  if (!targetRepo || headRepo.toLowerCase() === targetRepo.toLowerCase()) {
    return branchName;
  }
  const owner = headRepo.split("/")[0];
  return owner ? `${owner}:${branchName}` : branchName;
}

/** Collect only valid Nostr identities for profile batch lookup. */
export function pullRequestIdentityPubkeys(
  pullRequests: ProjectPullRequest[],
): string[] {
  const values = pullRequests.flatMap((pullRequest) => [
    pullRequest.author,
    ...pullRequest.recipients,
    ...pullRequest.reviewers,
    ...pullRequest.updates.map((update) => update.author),
    ...pullRequest.comments.map((comment) => comment.author),
    ...pullRequest.approvals.map((approval) => approval.author),
    ...pullRequest.changeRequests.map((changeRequest) => changeRequest.author),
  ]);
  return [
    ...new Set(
      values
        .filter((value) => /^[a-fA-F0-9]{64}$/.test(value))
        .map((value) => value.toLowerCase()),
    ),
  ];
}

/** Resolve stale Files changed state away from a GitHub PR detail. */
export function githubPullRequestDetailTab(
  selectedTab: string,
  githubHosted: boolean,
): string {
  return githubHosted && selectedTab === "pr-files"
    ? "pr-conversation"
    : selectedTab;
}

/** Resolve a valid GitHub comment request and its cache key. */
export function githubPullRequestCommentsRequest(
  project: Pick<Repository, "id" | "cloneUrls"> | null | undefined,
  selectedPullRequestId: string | null | undefined,
): {
  cloneUrl: string;
  number: number;
  queryKey: readonly ["project", string, "pull-requests", number, "comments"];
} | null {
  const cloneUrl = project?.cloneUrls[0] ?? "";
  const number = parseGithubPullRequestNumber(selectedPullRequestId);
  if (!project || !isGitHubCloneUrl(cloneUrl) || number === null) return null;
  return {
    cloneUrl,
    number,
    queryKey: ["project", project.id, "pull-requests", number, "comments"],
  };
}

/** Load the first read-only GitHub comment page for the selected numeric PR. */
export function useGithubPullRequestCommentsQuery(
  project: Pick<Repository, "id" | "cloneUrls"> | null | undefined,
  selectedPullRequestId: string | null | undefined,
) {
  const request = githubPullRequestCommentsRequest(
    project,
    selectedPullRequestId,
  );
  return useQuery({
    enabled: request !== null,
    queryKey: request?.queryKey ?? [
      "project",
      project?.id ?? "none",
      "pull-requests",
      "none",
      "comments",
    ],
    queryFn: async () => {
      if (!request) throw new Error("No GitHub pull request selected.");
      const comments = await listGithubPullRequestComments({
        cloneUrl: request.cloneUrl,
        number: request.number,
      });
      return comments.map(mapGithubPullRequestComment);
    },
    staleTime: 30_000,
  });
}

export function selectedGithubPullRequestAfterListLoad(input: {
  selectedPullRequestId: string | null;
  pullRequestIds: readonly string[];
  isSuccess: boolean;
  isFetching: boolean;
}): string | null {
  if (input.selectedPullRequestId === null) return null;
  if (input.pullRequestIds.includes(input.selectedPullRequestId)) {
    return input.selectedPullRequestId;
  }
  if (input.isSuccess && !input.isFetching) return null;
  return input.selectedPullRequestId;
}

/** Require a canonical Nostr event id before entering a Nostr PR write. */
export function requireNostrPullRequestId(id: string): string {
  if (!/^[a-fA-F0-9]{64}$/.test(id)) {
    throw new Error("Pull request writes require a 64-hex Nostr event id.");
  }
  return id.toLowerCase();
}
