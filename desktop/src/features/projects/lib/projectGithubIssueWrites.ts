import type { GithubIssueListState } from "./projectGithubIssues";

export type GithubIssueWriteAction = "close" | "reopen" | "create";

/** Destination Open or Closed filter after a successful GitHub write. */
export function nextGithubIssueListState(
  action: GithubIssueWriteAction,
): GithubIssueListState {
  return action === "close" ? "closed" : "open";
}

/** Keep #N until the destination list fetch settles without it. */
export function selectedGithubIssueAfterListLoad(input: {
  selectedIssueId: string | null;
  issueIds: readonly string[];
  isSuccess: boolean;
  isFetching: boolean;
}): string | null {
  if (input.selectedIssueId === null) return null;
  if (input.issueIds.includes(input.selectedIssueId)) {
    return input.selectedIssueId;
  }
  if (input.isSuccess && !input.isFetching) return null;
  return input.selectedIssueId;
}
