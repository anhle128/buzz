import type { GithubIssueListState } from "@/features/projects/lib/projectGithubIssues";
import { cn } from "@/shared/lib/cn";

/** Controlled Open or Closed list filter for GitHub-hosted issues. */
export function GitHubIssueStateFilter({
  onChange,
  value,
}: {
  onChange: (state: GithubIssueListState) => void;
  value: GithubIssueListState;
}) {
  return (
    <div
      aria-label="Issue state"
      className="flex items-center gap-1 border-border/50 border-b px-4 py-2"
      data-testid="project-github-issue-state-filter"
      role="tablist"
    >
      {(["open", "closed"] as const).map((state) => {
        const selected = value === state;
        return (
          <button
            aria-selected={selected}
            className={cn(
              "rounded-md px-2.5 py-1 text-xs font-medium",
              selected
                ? "bg-muted text-foreground"
                : "text-muted-foreground hover:text-foreground",
            )}
            data-testid={`project-github-issue-filter-${state}`}
            key={state}
            onClick={() => onChange(state)}
            role="tab"
            type="button"
          >
            {state === "open" ? "Open" : "Closed"}
          </button>
        );
      })}
    </div>
  );
}
