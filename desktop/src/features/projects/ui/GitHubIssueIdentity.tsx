import type { ProjectIssue } from "@/features/projects/hooks";
import { UserAvatar } from "@/shared/ui/UserAvatar";

/** GitHub login plus optional avatar. Never treats login as a pubkey. */
export function GitHubLoginIdentity({
  avatarUrl,
  login,
  showLabel = true,
}: {
  avatarUrl?: string | null;
  login: string;
  showLabel?: boolean;
}) {
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5">
      <UserAvatar avatarUrl={avatarUrl || null} displayName={login} size="xs" />
      {showLabel ? <span className="truncate text-xs">{login}</span> : null}
    </span>
  );
}

/** Overlapping assignee avatars for a GitHub issue list row. */
export function GitHubAssigneeFacepile({ issue }: { issue: ProjectIssue }) {
  if (issue.assignees.length === 0) return null;
  return (
    <fieldset
      aria-label={`Assigned to ${issue.assignees.join(", ")}`}
      className="m-0 flex -space-x-1.5 border-0 p-0"
    >
      {issue.assignees.slice(0, 3).map((login) => (
        <UserAvatar
          avatarUrl={issue.assigneeAvatars[login] || null}
          className="ring-2 ring-background"
          displayName={login}
          key={login}
          size="xs"
        />
      ))}
    </fieldset>
  );
}
