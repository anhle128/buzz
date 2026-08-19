import * as React from "react";
import { toast } from "sonner";

import {
  type ProjectIssue,
  type Repository,
  useCreateProjectIssueCommentMutation,
} from "@/features/projects/hooks";
import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import { Button } from "@/shared/ui/button";

/** Body-only GitHub issue comment composer. No mentions or media. */
export function GitHubIssueCommentComposer({
  issue,
  onSelectedIssueIdChange,
  project,
}: {
  issue: ProjectIssue;
  onSelectedIssueIdChange: (id: string | null) => void;
  project: Repository;
}) {
  const [content, setContent] = React.useState("");
  const commentMutation = useCreateProjectIssueCommentMutation(project);

  const handleSubmit = React.useCallback(
    async (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (!content.trim() || commentMutation.isPending) return;
      try {
        await commentMutation.mutateAsync({ content, issue });
        setContent("");
        toast.success("Comment posted.");
      } catch (error) {
        const parsed = parseProjectPullRequestMergeError(error);
        if (parsed?.code === "github_issue_unavailable") {
          toast.error("Issue not found.");
          onSelectedIssueIdChange(null);
          return;
        }
        toast.error(
          parsed?.message ??
            (error instanceof Error
              ? error.message
              : "Failed to post comment."),
        );
      }
    },
    [commentMutation, content, issue, onSelectedIssueIdChange],
  );

  return (
    <form
      className="space-y-2"
      data-testid="project-issue-comment-composer"
      onSubmit={(event) => {
        void handleSubmit(event);
      }}
    >
      <textarea
        className="flex min-h-20 w-full rounded-lg border border-input/40 bg-background px-3 py-2 text-base placeholder:text-muted-foreground focus-visible:outline-hidden focus-visible:ring-1 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-50 md:text-sm"
        data-testid="project-github-issue-comment-input"
        disabled={commentMutation.isPending}
        onChange={(event) => setContent(event.target.value)}
        placeholder="Add a comment…"
        value={content}
      />
      <div className="flex justify-end">
        <Button
          data-testid="project-github-issue-comment-submit"
          disabled={commentMutation.isPending || !content.trim()}
          size="sm"
          type="submit"
        >
          Comment
        </Button>
      </div>
    </form>
  );
}
