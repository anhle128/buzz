import { CircleDot, MessageSquare } from "lucide-react";

import type { ProjectIssue, Repository } from "@/features/projects/hooks";
import {
  issueDisplayNumber,
  useGithubIssueCommentsQuery,
} from "@/features/projects/lib/projectGithubIssues";
import { issueShareLink } from "@/features/projects/lib/projectShareLinks";
import { relativeTime } from "@/features/projects/lib/projectsViewHelpers";
import { UserAvatar } from "@/shared/ui/UserAvatar";
import { GitHubRepoStateRecovery } from "./GitHubRepoStateRecovery";
import {
  ProjectFeedRow,
  ProjectFeedRowCluster,
  ProjectFeedRowMonoCell,
} from "./ProjectFeedRow";
import { ProjectIssueCommentTimeline } from "./ProjectIssueCommentTimeline";
import { OverviewRailSection } from "./ProjectOverviewPanel";
import { ProjectRichContent } from "./ProjectRichContent";
import { ShareLinkButton } from "./ShareLinkButton";

function GitHubLoginIdentity({
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

function GitHubAssigneeFacepile({ issue }: { issue: ProjectIssue }) {
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

/** One GitHub issue list row with numeric identity and login metadata. */
export function GitHubIssueRow({
  issue,
  onOpen,
}: {
  issue: ProjectIssue;
  onOpen: () => void;
}) {
  const number = issueDisplayNumber(issue.id);
  return (
    <ProjectFeedRow
      eventId={issue.id}
      meta={
        <>
          <GitHubLoginIdentity
            avatarUrl={issue.authorAvatarUrl}
            login={issue.author}
          />
          <span>·</span>
          <span>Open</span>
          {issue.labels.map((label) => (
            <span
              className="rounded-full border border-border/60 px-1.5 py-0.5 text-2xs"
              key={label}
            >
              {label}
            </span>
          ))}
        </>
      }
      onOpen={onOpen}
      statusIcon={<CircleDot className="h-3.5 w-3.5 shrink-0 text-green-500" />}
      testId="project-github-issue-row"
      title={issue.title}
      trailing={
        <>
          <GitHubAssigneeFacepile issue={issue} />
          {issue.commentCount > 0 ? (
            <span className="flex items-center gap-1 text-xs text-muted-foreground">
              <MessageSquare className="h-3.5 w-3.5" />
              {issue.commentCount}
            </span>
          ) : null}
          <ProjectFeedRowCluster>
            <ProjectFeedRowMonoCell
              label={`#${number}`}
              onClick={onOpen}
              title={`View GitHub issue #${number}`}
            />
          </ProjectFeedRowCluster>
          <span className="hidden w-20 shrink-0 text-right text-xs text-muted-foreground sm:block">
            {relativeTime(issue.createdAt)}
          </span>
        </>
      }
    />
  );
}

/** Read-only GitHub issue detail with an independently retryable comment query. */
export function GitHubIssueDetail({
  issue,
  project,
}: {
  issue: ProjectIssue;
  project: Repository;
}) {
  const commentsQuery = useGithubIssueCommentsQuery(project, issue.id);
  const number = issueDisplayNumber(issue.id);
  return (
    <div className="grid xl:grid-cols-[minmax(0,1fr)_18rem]">
      <div className="min-w-0">
        <header className="space-y-3 p-4">
          <p className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
            <CircleDot className="h-3.5 w-3.5 text-green-500" />
            Issue from {issue.author}
          </p>
          <h3 className="mt-1 line-clamp-2 text-base font-semibold text-foreground">
            {issue.title}{" "}
            <span className="font-normal text-muted-foreground">#{number}</span>
            <ShareLinkButton
              className="ml-1 inline-flex h-6 w-6 align-text-bottom"
              label="Copy issue link"
              link={issueShareLink(issue)}
              testId="project-issue-copy-link"
            />
          </h3>
          {issue.content ? (
            <ProjectRichContent content={issue.content} tags={[]} />
          ) : null}
        </header>
        <section className="space-y-3 p-4">
          {commentsQuery.isLoading ? (
            <p className="text-sm text-muted-foreground">Loading comments…</p>
          ) : commentsQuery.isError ? (
            <GitHubRepoStateRecovery
              error={commentsQuery.error}
              onRetry={() => void commentsQuery.refetch()}
              titleId="github-issue-comments-recovery-title"
              unavailableTitle="Could not load GitHub comments"
            />
          ) : (
            <ProjectIssueCommentTimeline
              comments={commentsQuery.data ?? []}
              githubMode
              key={issue.id}
            />
          )}
        </section>
      </div>
      <aside className="space-y-6 border-t border-border/60 p-4 xl:border-l xl:border-t-0">
        <OverviewRailSection title="Status">
          <span className="inline-flex items-center gap-1.5 text-xs font-medium text-green-500">
            <CircleDot className="h-3.5 w-3.5" /> Open
          </span>
        </OverviewRailSection>
        {issue.assignees.length > 0 ? (
          <OverviewRailSection title="Assignees">
            <div className="space-y-2">
              {issue.assignees.map((login) => (
                <GitHubLoginIdentity
                  avatarUrl={issue.assigneeAvatars[login]}
                  key={login}
                  login={login}
                />
              ))}
            </div>
          </OverviewRailSection>
        ) : null}
        <OverviewRailSection title="Author">
          <GitHubLoginIdentity
            avatarUrl={issue.authorAvatarUrl}
            login={issue.author}
          />
        </OverviewRailSection>
        {issue.labels.length > 0 ? (
          <OverviewRailSection title="Labels">
            <div className="flex flex-wrap gap-1.5">
              {issue.labels.map((label) => (
                <span
                  className="rounded-full border border-border/60 px-1.5 py-0.5 text-2xs text-muted-foreground"
                  key={label}
                >
                  {label}
                </span>
              ))}
            </div>
          </OverviewRailSection>
        ) : null}
        <OverviewRailSection title="Activity">
          <dl className="space-y-1.5 text-xs text-muted-foreground">
            <div className="flex items-center justify-between gap-3">
              <dt>Created</dt>
              <dd className="font-medium text-foreground">
                {relativeTime(issue.createdAt)}
              </dd>
            </div>
            <div className="flex items-center justify-between gap-3">
              <dt>Updated</dt>
              <dd className="font-medium text-foreground">
                {relativeTime(issue.updatedAt)}
              </dd>
            </div>
          </dl>
        </OverviewRailSection>
      </aside>
    </div>
  );
}
