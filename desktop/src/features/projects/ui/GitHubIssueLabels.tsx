import * as React from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import type { ProjectIssue, Repository } from "@/features/projects/hooks";
import {
  githubIssueWriteInvalidationKey,
  useAddGithubIssueLabelMutation,
  useGithubRepoLabelsQuery,
  useRemoveGithubIssueLabelMutation,
} from "@/features/projects/lib/projectGithubIssueWrites";
import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import { Button } from "@/shared/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";

/** GitHub label chips plus a catalog picker that never uses Nostr labels. */
export function GitHubIssueLabels({
  issue,
  onSelectedIssueIdChange,
  project,
}: {
  issue: ProjectIssue;
  onSelectedIssueIdChange: (id: string | null) => void;
  project: Repository;
}) {
  const queryClient = useQueryClient();
  const labelsQuery = useGithubRepoLabelsQuery(project, true);
  const addMutation = useAddGithubIssueLabelMutation(project, issue.id);
  const removeMutation = useRemoveGithubIssueLabelMutation(project, issue.id);
  const currentLabels = React.useMemo(
    () => new Set(issue.labels),
    [issue.labels],
  );
  const addable = (labelsQuery.data ?? []).filter(
    (label) => !currentLabels.has(label.name),
  );
  const hideAdd = labelsQuery.isSuccess && addable.length === 0;
  const writePending = addMutation.isPending || removeMutation.isPending;

  const handleWriteError = React.useCallback(
    async (error: unknown, fallback: string) => {
      const parsed = parseProjectPullRequestMergeError(error);
      if (parsed?.code === "github_issue_unavailable") {
        toast.error("Issue not found.");
        onSelectedIssueIdChange(null);
        return;
      }
      if (parsed?.code === "github_issues_failed") {
        await queryClient.invalidateQueries({
          queryKey: ["project", project.id, "github-labels"],
        });
      }
      toast.error(
        parsed?.message ?? (error instanceof Error ? error.message : fallback),
      );
    },
    [onSelectedIssueIdChange, project.id, queryClient],
  );

  const handleAdd = React.useCallback(
    async (name: string) => {
      try {
        await addMutation.mutateAsync(name);
        await queryClient.invalidateQueries({
          queryKey: githubIssueWriteInvalidationKey(project.id),
        });
        toast.success("Label added.");
      } catch (error) {
        await handleWriteError(error, "Failed to add label.");
      }
    },
    [addMutation, handleWriteError, project.id, queryClient],
  );

  const handleRemove = React.useCallback(
    async (name: string) => {
      try {
        await removeMutation.mutateAsync(name);
        await queryClient.invalidateQueries({
          queryKey: githubIssueWriteInvalidationKey(project.id),
        });
        toast.success("Label removed.");
      } catch (error) {
        await handleWriteError(error, "Failed to remove label.");
      }
    },
    [handleWriteError, project.id, queryClient, removeMutation],
  );

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap gap-1.5">
        {issue.labels.map((name) => (
          <button
            aria-label={`Remove label ${name}`}
            className="rounded-full border border-border/60 px-1.5 py-0.5 text-2xs text-muted-foreground hover:border-destructive/60 hover:text-destructive disabled:opacity-50"
            data-label-name={name}
            data-testid="project-github-issue-label"
            disabled={writePending}
            key={name}
            onClick={() => {
              void handleRemove(name);
            }}
            type="button"
          >
            {name}
          </button>
        ))}
      </div>
      {hideAdd ? null : (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              disabled={labelsQuery.isError || writePending}
              size="xs"
              title={labelsQuery.isError ? "Could not load labels" : undefined}
              type="button"
              variant="outline"
            >
              Add label
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="min-w-44">
            {addable.map((label) => (
              <DropdownMenuItem
                aria-label={label.name}
                data-testid={`project-github-issue-label-option-${label.name}`}
                key={label.name}
                onSelect={() => {
                  void handleAdd(label.name);
                }}
              >
                {label.name}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      )}
    </div>
  );
}
