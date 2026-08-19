import * as React from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import type { ProjectIssue, Repository } from "@/features/projects/hooks";
import {
  githubIssueWriteInvalidationKey,
  useAddGithubIssueAssigneeMutation,
  useGithubAuthenticatedUserQuery,
  useGithubRepoAssigneesQuery,
  useRemoveGithubIssueAssigneeMutation,
} from "@/features/projects/lib/projectGithubIssueWrites";
import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import { Button } from "@/shared/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/shared/ui/dropdown-menu";
import { GitHubLoginIdentity } from "./GitHubIssueIdentity";
import { GitHubRepoStateRecovery } from "./GitHubRepoStateRecovery";

/** GitHub login assignees, catalog picker, and Assign me via GET /user. */
export function GitHubIssueAssignees({
  issue,
  onSelectedIssueIdChange,
  project,
}: {
  issue: ProjectIssue;
  onSelectedIssueIdChange: (id: string | null) => void;
  project: Repository;
}) {
  const queryClient = useQueryClient();
  const catalogQuery = useGithubRepoAssigneesQuery(project, true);
  const userQuery = useGithubAuthenticatedUserQuery(true);
  const addMutation = useAddGithubIssueAssigneeMutation(project, issue.id);
  const removeMutation = useRemoveGithubIssueAssigneeMutation(
    project,
    issue.id,
  );
  const currentLogins = React.useMemo(
    () => new Set(issue.assignees),
    [issue.assignees],
  );
  const addable = (catalogQuery.data ?? []).filter(
    (user) => !currentLogins.has(user.login),
  );
  const authenticatedLogin = userQuery.data?.login;
  const viewerAssigned = Boolean(
    authenticatedLogin && currentLogins.has(authenticatedLogin),
  );
  const writePending = addMutation.isPending || removeMutation.isPending;
  const catalogAvatars = React.useMemo(() => {
    return Object.fromEntries(
      (catalogQuery.data ?? []).map((user) => [user.login, user.avatar_url]),
    );
  }, [catalogQuery.data]);

  const handleWriteError = React.useCallback(
    (error: unknown, fallback: string) => {
      const parsed = parseProjectPullRequestMergeError(error);
      if (parsed?.code === "github_issue_unavailable") {
        toast.error("Issue not found.");
        onSelectedIssueIdChange(null);
        return;
      }
      toast.error(
        parsed?.message ?? (error instanceof Error ? error.message : fallback),
      );
    },
    [onSelectedIssueIdChange],
  );

  const handleAdd = React.useCallback(
    async (login: string) => {
      try {
        await addMutation.mutateAsync(login);
        await queryClient.invalidateQueries({
          queryKey: githubIssueWriteInvalidationKey(project.id),
        });
        toast.success("Issue assigned.");
      } catch (error) {
        handleWriteError(error, "Failed to assign issue.");
      }
    },
    [addMutation, handleWriteError, project.id, queryClient],
  );

  const handleRemove = React.useCallback(
    async (login: string) => {
      try {
        await removeMutation.mutateAsync(login);
        await queryClient.invalidateQueries({
          queryKey: githubIssueWriteInvalidationKey(project.id),
        });
        toast.success("Issue unassigned.");
      } catch (error) {
        handleWriteError(error, "Failed to unassign issue.");
      }
    },
    [handleWriteError, project.id, queryClient, removeMutation],
  );

  return (
    <div className="space-y-2">
      <div className="space-y-2">
        {issue.assignees.map((login) => (
          <div
            className="flex min-w-0 items-center justify-between gap-2"
            key={login}
          >
            <GitHubLoginIdentity
              avatarUrl={issue.assigneeAvatars[login] ?? catalogAvatars[login]}
              login={login}
            />
            <Button
              aria-label={`Unassign ${login}`}
              data-testid={`project-github-issue-unassign-${login}`}
              disabled={writePending}
              onClick={() => {
                void handleRemove(login);
              }}
              size="xs"
              type="button"
              variant="ghost"
            >
              Unassign
            </Button>
          </div>
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-1.5">
        {userQuery.isError ? (
          <Button
            data-testid="project-github-issue-assign-me"
            disabled
            size="xs"
            title="Could not load GitHub user"
            type="button"
            variant="ghost"
          >
            Assign me
          </Button>
        ) : userQuery.isSuccess && authenticatedLogin ? (
          viewerAssigned ? (
            <Button
              data-testid="project-github-issue-unassign-me"
              disabled={writePending}
              onClick={() => {
                void handleRemove(authenticatedLogin);
              }}
              size="xs"
              type="button"
              variant="ghost"
            >
              Unassign me
            </Button>
          ) : (
            <Button
              data-testid="project-github-issue-assign-me"
              disabled={writePending}
              onClick={() => {
                void handleAdd(authenticatedLogin);
              }}
              size="xs"
              type="button"
              variant="ghost"
            >
              Assign me
            </Button>
          )
        ) : null}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              disabled={catalogQuery.isError || writePending}
              size="xs"
              title={
                catalogQuery.isError ? "Could not load assignees" : undefined
              }
              type="button"
              variant="outline"
            >
              Assign
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="min-w-44">
            {addable.map((user) => (
              <DropdownMenuItem
                aria-label={user.login}
                key={user.login}
                onSelect={() => {
                  void handleAdd(user.login);
                }}
              >
                <GitHubLoginIdentity
                  avatarUrl={user.avatar_url}
                  login={user.login}
                />
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      {userQuery.isError ? (
        <GitHubRepoStateRecovery
          error={userQuery.error}
          onRetry={() => void userQuery.refetch()}
          titleId="github-issue-user-recovery-title"
          unavailableTitle="Could not load GitHub user"
        />
      ) : null}
    </div>
  );
}
