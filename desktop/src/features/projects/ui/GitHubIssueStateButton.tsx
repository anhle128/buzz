import * as React from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import type { ProjectIssue, Repository } from "@/features/projects/hooks";
import type { GithubIssueListState } from "@/features/projects/lib/projectGithubIssues";
import {
  githubIssueWriteInvalidationKey,
  nextGithubIssueListState,
  useUpdateGithubIssueStateMutation,
} from "@/features/projects/lib/projectGithubIssueWrites";
import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import { Button } from "@/shared/ui/button";

/** Close an open GitHub issue or reopen a closed one. */
export function GitHubIssueStateButton({
  issue,
  onListStateChange,
  onSelectedIssueIdChange,
  project,
}: {
  issue: ProjectIssue;
  onListStateChange: (state: GithubIssueListState) => void;
  onSelectedIssueIdChange: (id: string | null) => void;
  project: Repository;
}) {
  const queryClient = useQueryClient();
  const mutation = useUpdateGithubIssueStateMutation(project, issue.id);
  const action = issue.status === "Closed" ? "reopen" : "close";
  const destination = nextGithubIssueListState(action);

  const handleClick = React.useCallback(async () => {
    try {
      await mutation.mutateAsync(destination);
      onListStateChange(destination);
      onSelectedIssueIdChange(issue.id);
      await queryClient.invalidateQueries({
        queryKey: githubIssueWriteInvalidationKey(project.id),
      });
    } catch (error) {
      const parsed = parseProjectPullRequestMergeError(error);
      if (parsed?.code === "github_issue_unavailable") {
        toast.error("Issue not found.");
        onSelectedIssueIdChange(null);
        return;
      }
      toast.error(
        parsed?.message ??
          (error instanceof Error ? error.message : "Failed to update issue."),
      );
    }
  }, [
    destination,
    issue.id,
    mutation,
    onListStateChange,
    onSelectedIssueIdChange,
    project.id,
    queryClient,
  ]);

  return (
    <Button
      data-testid={
        action === "close"
          ? "project-github-issue-close"
          : "project-github-issue-reopen"
      }
      disabled={mutation.isPending}
      onClick={() => {
        void handleClick();
      }}
      size="sm"
      type="button"
      variant="outline"
    >
      {action === "close" ? "Close" : "Reopen"}
    </Button>
  );
}
