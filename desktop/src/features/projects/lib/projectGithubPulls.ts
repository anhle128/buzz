import { useQuery } from "@tanstack/react-query";

import type { ProjectPullRequest } from "@/features/projects/projectPullRequests.mjs";
import type { Repository } from "@/features/projects/projectModels";
import {
  listGithubPullRequestComments,
  type GithubPullRequestCommentDto,
  type GithubPullRequestDto,
  type GithubPullRequestListDto,
} from "@/shared/api/projectGit";
import { isGitHubCloneUrl } from "./projectGitError";

const HEX64_RE = /^[a-fA-F0-9]{64}$/;

/** Host-routed pull-request list consumed by the repository Pull Request tab. */
export type ProjectPullRequestsResult = {
  pullRequests: ProjectPullRequest[];
  hasMore: boolean;
};

/** Convert a positive safe GitHub pull-request number into its decimal selection id. */
export function githubPullRequestId(number: number): string {
  if (!Number.isSafeInteger(number) || number <= 0) {
    throw new Error("GitHub returned an invalid pull request number.");
  }
  return String(number);
}

/** Parse a positive GitHub pull-request number that is safe in JavaScript. */
export function parseGithubPullRequestNumber(
  value: string | null | undefined,
): number | null {
  if (!value || !/^[1-9][0-9]*$/.test(value)) return null;
  const number = Number(value);
  return Number.isSafeInteger(number) ? number : null;
}

/** Display a full GitHub number or the existing eight-character Nostr prefix. */
export function pullRequestDisplayNumber(pullRequestId: string): string {
  return parseGithubPullRequestNumber(pullRequestId) === null
    ? pullRequestId.slice(0, 8)
    : pullRequestId;
}

/** Map a bounded native GitHub pull request onto the shared Projects model. */
export function mapGithubPullRequestToProjectPullRequest(
  dto: GithubPullRequestDto,
  input: { repoAddress: string; cloneUrl: string },
): ProjectPullRequest {
  return {
    id: githubPullRequestId(dto.number),
    title: dto.title,
    content: dto.body ?? "",
    tags: [],
    author: dto.user.login,
    authorAvatarUrl: dto.user.avatar_url,
    createdAt: dto.created_at,
    repoAddress: input.repoAddress,
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
    initialCommit: dto.head.sha,
    commit: dto.head.sha,
    cloneUrls: [input.cloneUrl],
    updateCount: 0,
    updatedAt: dto.updated_at,
    updates: [],
    comments: [],
    commentCount: dto.comments,
    headRepoFullName: dto.head.repo.full_name || null,
    htmlUrl: dto.html_url,
  };
}

/** Map a bounded native GitHub comment without interpreting its login as a pubkey. */
export function mapGithubCommentToProjectPullRequestComment(
  dto: GithubPullRequestCommentDto,
): ProjectPullRequest["comments"][number] {
  return {
    id: String(dto.id),
    content: dto.body ?? "",
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
        mapGithubPullRequestToProjectPullRequest(pull, {
          repoAddress: project.repoAddress,
          cloneUrl,
        }),
      ),
      hasMore: page.has_more === true,
    };
  }
  return { pullRequests: await loaders.loadBuzz(), hasMore: false };
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
  ]);
  return [
    ...new Set(
      values
        .filter((value) => HEX64_RE.test(value))
        .map((value) => value.toLowerCase()),
    ),
  ];
}

/** Render `owner:branch → base` only when the head repo is a different non-empty name. */
export function githubPullRequestBranchLabel(
  pullRequest: Pick<
    ProjectPullRequest,
    "branchName" | "targetBranch" | "headRepoFullName" | "cloneUrls"
  >,
): string {
  const head = pullRequest.branchName ?? "";
  const base = pullRequest.targetBranch ?? "";
  const headRepo = pullRequest.headRepoFullName?.trim() ?? "";
  if (!headRepo) return `${head} → ${base}`;
  const targetRepo = githubRepoFullNameFromCloneUrl(
    pullRequest.cloneUrls[0] ?? "",
  );
  if (!targetRepo || headRepo.toLowerCase() === targetRepo.toLowerCase()) {
    return `${head} → ${base}`;
  }
  const owner = headRepo.split("/")[0] ?? headRepo;
  return `${owner}:${head} → ${base}`;
}

/** Parse the target owner/repository from a supported github.com clone URL. */
export function githubRepoFullNameFromCloneUrl(
  cloneUrl: string,
): string | null {
  const ssh = cloneUrl.match(/^git@github\.com:([^/]+)\/(.+?)(?:\.git)?$/i);
  if (ssh) return `${ssh[1]}/${ssh[2]}`;
  try {
    const url = new URL(cloneUrl);
    const isHttps = url.protocol === "https:" && url.username === "";
    const isSsh = url.protocol === "ssh:" && url.username === "git";
    if (
      (!isHttps && !isSsh) ||
      url.hostname.toLowerCase() !== "github.com" ||
      url.port !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== ""
    ) {
      return null;
    }
    const segments = url.pathname.split("/").filter(Boolean);
    if (segments.length !== 2) return null;
    const [owner, rawRepo] = segments;
    const repo = rawRepo?.replace(/\.git$/, "");
    return owner && repo ? `${owner}/${repo}` : null;
  } catch {
    return null;
  }
}

/** Whether a pull-request head belongs to the selected repository. */
export function pullRequestHeadBelongsToRepository(
  pullRequest: Pick<ProjectPullRequest, "headRepoFullName">,
  repository: Pick<Repository, "cloneUrls">,
): boolean {
  const cloneUrl = repository.cloneUrls[0] ?? "";
  if (!isGitHubCloneUrl(cloneUrl)) return true;
  const targetRepo = githubRepoFullNameFromCloneUrl(cloneUrl);
  const headRepo = pullRequest.headRepoFullName?.trim();
  return Boolean(
    targetRepo &&
      headRepo &&
      targetRepo.toLowerCase() === headRepo.toLowerCase(),
  );
}

/** Find an open same-repository pull request for one head/base branch pair. */
export function hasOpenPullRequestForBranches(
  pullRequests: readonly ProjectPullRequest[],
  repository: Pick<Repository, "cloneUrls" | "defaultBranch">,
  head: string,
  base: string,
): boolean {
  return pullRequests.some(
    (pullRequest) =>
      pullRequestHeadBelongsToRepository(pullRequest, repository) &&
      (pullRequest.status === "Open" || pullRequest.status === "Draft") &&
      pullRequest.branchName === head &&
      (pullRequest.targetBranch ?? repository.defaultBranch) === base,
  );
}

/** Conversation badge: list count until comments load, then the greater value. */
export function githubPullRequestConversationCount(input: {
  commentCount: number;
  commentsLength: number;
}): number {
  return Math.max(input.commentCount, input.commentsLength);
}

/** Keep #N until the destination list fetch settles without it. */
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

/** Refuse a GitHub decimal id on a Nostr-only write path. */
export function requireBuzzPullRequestEventId(id: string): string {
  if (!HEX64_RE.test(id)) {
    throw new Error(
      "This GitHub pull request cannot be mutated through Nostr.",
    );
  }
  return id;
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

/** Load the first read-only GitHub conversation comment page for the selected number. */
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
      return comments.map(mapGithubCommentToProjectPullRequestComment);
    },
    staleTime: 30_000,
  });
}
