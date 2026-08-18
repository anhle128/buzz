import { Copy } from "lucide-react";

import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import { copyTextToClipboard } from "@/shared/lib/clipboard";
import { Button } from "@/shared/ui/button";

function githubStateErrorTitle(
  code: string | undefined,
  unavailableTitle: string,
): string {
  switch (code) {
    case "github_cli_missing":
      return "GitHub CLI is required";
    case "github_auth_required":
      return "GitHub authentication required";
    default:
      return unavailableTitle;
  }
}

/** Recovery guidance when GitHub repository state fails to load. */
export function GitHubRepoStateRecovery({
  error,
  onRetry,
  titleId = "github-repo-state-recovery-title",
  unavailableTitle = "Could not load GitHub branches",
}: {
  error?: unknown;
  onRetry?: () => void;
  titleId?: string;
  unavailableTitle?: string;
}) {
  const parsed = parseProjectPullRequestMergeError(error);
  const code = parsed?.code;
  const message =
    parsed?.message ??
    (error instanceof Error ? error.message : unavailableTitle);
  const title = githubStateErrorTitle(code, unavailableTitle);

  return (
    <section
      aria-labelledby={titleId}
      className="w-full rounded-md border border-border bg-muted/40 p-3"
      role="status"
    >
      <h3 className="text-sm font-medium" id={titleId}>
        {title}
      </h3>
      <p className="mt-1 text-sm text-muted-foreground">{message}</p>
      {code === "github_cli_missing" ? (
        <p className="mt-2 text-sm text-muted-foreground">
          Install GitHub CLI, then retry.
        </p>
      ) : null}
      {code === "github_auth_required" ? (
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <code className="rounded bg-background/80 px-2 py-1 font-mono text-sm text-foreground">
            gh auth login --hostname github.com
          </code>
          <Button
            aria-label="Copy GitHub login command"
            onClick={() =>
              copyTextToClipboard(
                "gh auth login --hostname github.com",
                "GitHub login command copied",
              )
            }
            size="xs"
            type="button"
            variant="ghost"
          >
            <Copy className="h-3.5 w-3.5" />
            Copy
          </Button>
        </div>
      ) : null}
      <div className="mt-3 flex flex-wrap gap-2">
        <Button onClick={onRetry} size="xs" type="button" variant="outline">
          Retry
        </Button>
      </div>
    </section>
  );
}
