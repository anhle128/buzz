import type { ProjectIssue } from "@/features/projects/projectIssues.mjs";
import type { Repository } from "@/features/projects/projectModels";
import type {
  GithubIssueCommentDto,
  GithubIssueDto,
  GithubIssueListDto,
} from "@/shared/api/projectGit";
import { isGitHubCloneUrl } from "./projectGitError";

/** Host-routed issue list consumed by the repository Issues tab. */
export type ProjectIssuesResult = { issues: ProjectIssue[]; hasMore: boolean };

/** Map a bounded native GitHub issue onto the shared Projects issue model. */
export function mapGithubIssueToProjectIssue(
  dto: GithubIssueDto,
  repoAddress: string,
): ProjectIssue {
  return {
    id: String(dto.number),
    title: dto.title,
    content: dto.body ?? "",
    tags: [],
    author: dto.user.login,
    authorAvatarUrl: dto.user.avatar_url,
    createdAt: dto.created_at,
    repoAddress,
    channelId: null,
    originAgentName: null,
    labels: [...dto.labels],
    recipients: [],
    assignees: dto.assignees.map((assignee) => assignee.login),
    assigneeAvatars: Object.fromEntries(
      dto.assignees.map((assignee) => [assignee.login, assignee.avatar_url]),
    ),
    assigneeOperationHeads: {},
    status: dto.state === "closed" ? "Closed" : "Open",
    statusEventId: null,
    updatedAt: dto.updated_at,
    comments: [],
    commentCount: dto.comments,
    htmlUrl: dto.html_url,
  };
}

/** Map a bounded native GitHub comment without interpreting its login as a pubkey. */
export function mapGithubCommentToProjectIssueComment(
  dto: GithubIssueCommentDto,
): ProjectIssue["comments"][number] {
  return {
    id: String(dto.id),
    content: dto.body ?? "",
    tags: [],
    author: dto.user.login,
    authorAvatarUrl: dto.user.avatar_url,
    createdAt: dto.created_at,
  };
}

/** Route one repository to exactly one issue backend. */
export async function fetchProjectIssuesWith(
  project: Pick<Repository, "id" | "repoAddress" | "cloneUrls">,
  loaders: {
    loadGithub: (input: {
      cloneUrl: string;
      state: "open";
    }) => Promise<GithubIssueListDto>;
    loadBuzz: () => Promise<ProjectIssue[]>;
  },
): Promise<ProjectIssuesResult> {
  const cloneUrl = project.cloneUrls[0] ?? "";
  if (isGitHubCloneUrl(cloneUrl)) {
    const page = await loaders.loadGithub({ cloneUrl, state: "open" });
    return {
      issues: page.issues.map((issue) =>
        mapGithubIssueToProjectIssue(issue, project.repoAddress),
      ),
      hasMore: page.has_more === true,
    };
  }
  return { issues: await loaders.loadBuzz(), hasMore: false };
}

/** Parse a positive GitHub issue number that is safe in JavaScript. */
export function parseGithubIssueNumber(
  value: string | null | undefined,
): number | null {
  if (!value || !/^[1-9][0-9]*$/.test(value)) return null;
  const number = Number(value);
  return Number.isSafeInteger(number) ? number : null;
}

/** Display a full GitHub number or the existing eight-character Nostr prefix. */
export function issueDisplayNumber(issueId: string): string {
  return parseGithubIssueNumber(issueId) === null
    ? issueId.slice(0, 8)
    : issueId;
}

/** Collect only valid Nostr identities for profile batch lookup. */
export function issueIdentityPubkeys(issues: ProjectIssue[]): string[] {
  const values = issues.flatMap((issue) => [
    issue.author,
    ...issue.recipients,
    ...issue.assignees,
    ...issue.comments.map((comment) => comment.author),
  ]);
  return [
    ...new Set(
      values
        .filter((value) => /^[a-fA-F0-9]{64}$/.test(value))
        .map((value) => value.toLowerCase()),
    ),
  ];
}
