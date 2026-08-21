import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import { invokeTauri } from "@/shared/api/tauri";

/** Bounded GitHub login identity returned by the native PR commands. */
export type GithubPullRequestUserDto = { login: string; avatar_url: string };
/** Bounded GitHub repository identity returned by the native PR commands. */
export type GithubPullRequestRepoDto = { full_name: string };
/** One bounded GitHub pull request returned by the native PR commands. */
export type GithubPullRequestDto = {
  number: number;
  title: string;
  body: string;
  html_url: string;
  draft: boolean;
  created_at: number;
  updated_at: number;
  comments: number;
  user: GithubPullRequestUserDto;
  head: { ref: string; sha: string; repo: GithubPullRequestRepoDto };
  base: { ref: string; repo: GithubPullRequestRepoDto };
};
/** One GitHub pull-request page and its first-page truncation signal. */
export type GithubPullRequestListDto = {
  pulls: GithubPullRequestDto[];
  has_more: boolean;
};
/** One bounded read-only GitHub pull-request conversation comment. */
export type GithubPullRequestCommentDto = {
  id: number;
  body: string;
  html_url: string;
  created_at: number;
  user: GithubPullRequestUserDto;
};

async function invokeGithubPulls<T>(
  command: string,
  input: Record<string, unknown>,
): Promise<T> {
  try {
    return await invokeTauri<T>(command, input);
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** List the first bounded page of open GitHub pull requests. */
export function listGithubPullRequests(input: {
  cloneUrl: string;
}): Promise<GithubPullRequestListDto> {
  return invokeGithubPulls("list_github_pull_requests", input);
}

/** Create one ready same-repository GitHub pull request. */
export function createGithubPullRequest(input: {
  cloneUrl: string;
  title: string;
  body: string;
  head: string;
  base: string;
}): Promise<GithubPullRequestDto> {
  return invokeGithubPulls("create_github_pull_request", input);
}

/** List the first read-only issue-comment page for one GitHub pull request. */
export function listGithubPullRequestComments(input: {
  cloneUrl: string;
  number: number;
}): Promise<GithubPullRequestCommentDto[]> {
  return invokeGithubPulls("list_github_pull_request_comments", input);
}
