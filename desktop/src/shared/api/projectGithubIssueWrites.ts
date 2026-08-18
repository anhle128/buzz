import { invokeTauri } from "@/shared/api/tauri";

import {
  parseProjectPullRequestMergeError,
  type GithubIssueCommentDto,
  type GithubIssueDto,
  type GithubIssueUserDto,
} from "./projectGit";

/** One repository label from the GitHub catalog. */
export type GithubRepoLabelDto = { name: string; color: string };

/** Close or reopen one GitHub issue for a github.com clone URL. */
export async function updateGithubIssueState(input: {
  cloneUrl: string;
  number: number;
  state: "open" | "closed";
}): Promise<GithubIssueDto> {
  try {
    return await invokeTauri<GithubIssueDto>(
      "update_github_issue_state",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** Create one GitHub issue comment for a github.com clone URL. */
export async function createGithubIssueComment(input: {
  cloneUrl: string;
  number: number;
  body: string;
}): Promise<GithubIssueCommentDto> {
  try {
    return await invokeTauri<GithubIssueCommentDto>(
      "create_github_issue_comment",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** List repository labels for a github.com clone URL. */
export async function listGithubRepoLabels(input: {
  cloneUrl: string;
}): Promise<GithubRepoLabelDto[]> {
  try {
    return await invokeTauri<GithubRepoLabelDto[]>(
      "list_github_repo_labels",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** Add one label to a GitHub issue for a github.com clone URL. */
export async function addGithubIssueLabels(input: {
  cloneUrl: string;
  number: number;
  name: string;
}): Promise<GithubIssueDto> {
  try {
    return await invokeTauri<GithubIssueDto>("add_github_issue_labels", input);
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** Remove one label from a GitHub issue for a github.com clone URL. */
export async function removeGithubIssueLabel(input: {
  cloneUrl: string;
  number: number;
  name: string;
}): Promise<GithubIssueDto> {
  try {
    return await invokeTauri<GithubIssueDto>(
      "remove_github_issue_label",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** List assignable users for a github.com clone URL. */
export async function listGithubRepoAssignees(input: {
  cloneUrl: string;
}): Promise<GithubIssueUserDto[]> {
  try {
    return await invokeTauri<GithubIssueUserDto[]>(
      "list_github_repo_assignees",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** Add one assignee to a GitHub issue for a github.com clone URL. */
export async function addGithubIssueAssignees(input: {
  cloneUrl: string;
  number: number;
  login: string;
}): Promise<GithubIssueDto> {
  try {
    return await invokeTauri<GithubIssueDto>(
      "add_github_issue_assignees",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** Remove one assignee from a GitHub issue for a github.com clone URL. */
export async function removeGithubIssueAssignee(input: {
  cloneUrl: string;
  number: number;
  login: string;
}): Promise<GithubIssueDto> {
  try {
    return await invokeTauri<GithubIssueDto>(
      "remove_github_issue_assignee",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** Return the GitHub user authenticated to `gh`. */
export async function getGithubAuthenticatedUser(): Promise<GithubIssueUserDto> {
  try {
    return await invokeTauri<GithubIssueUserDto>(
      "get_github_authenticated_user",
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}
