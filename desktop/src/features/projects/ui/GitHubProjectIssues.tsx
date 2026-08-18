import { CircleDot, CircleX, MessageSquare } from "lucide-react";

import type { ProjectIssue, Repository } from "@/features/projects/hooks";
import {
  issueDisplayNumber,
  type GithubIssueListState,
  useGithubIssueCommentsQuery,
} from "@/features/projects/lib/projectGithubIssues";
import { issueShareLink } from "@/features/projects/lib/projectShareLinks";
import { relativeTime } from "@/features/projects/lib/projectsViewHelpers";
import { GitHubIssueAssignees } from "./GitHubIssueAssignees";
import { GitHubIssueCommentComposer } from "./GitHubIssueCommentComposer";
import {
  GitHubAssigneeFacepile,
  GitHubLoginIdentity,
} from "./GitHubIssueIdentity";
import { GitHubIssueLabels } from "./GitHubIssueLabels";
import { GitHubIssueStateButton } from "./GitHubIssueStateButton";
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

function githubIssueStatusVisual(status: ProjectIssue["status"]) {
  if (status === "Closed") {
    return { className: "text-destructive", icon: CircleX };
  }
  return { className: "text-green-500", icon: CircleDot };
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
  const status = githubIssueStatusVisual(issue.status);
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
          <span>{issue.status}</span>
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
      statusIcon={
        <status.icon className={`h-3.5 w-3.5 shrink-0 ${status.className}`} />
      }
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

/** GitHub issue detail with host-native write chrome and retryable comments. */
export function GitHubIssueDetail({
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
  const commentsQuery = useGithubIssueCommentsQuery(project, issue.id);
  const number = issueDisplayNumber(issue.id);
  const status = githubIssueStatusVisual(issue.status);
  return (
    <div className="grid xl:grid-cols-[minmax(0,1fr)_18rem]">
      <div className="min-w-0">
        <header className="space-y-3 p-4">
          <div className="flex items-start justify-between gap-3">
            <div className="min-w-0">
              <p className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                <status.icon className={`h-3.5 w-3.5 ${status.className}`} />
                Issue from {issue.author}
              </p>
              <h3 className="mt-1 line-clamp-2 text-base font-semibold text-foreground">
                {issue.title}{" "}
                <span className="font-normal text-muted-foreground">
                  #{number}
                </span>
                <ShareLinkButton
                  className="ml-1 inline-flex h-6 w-6 align-text-bottom"
                  label="Copy issue link"
                  link={issueShareLink(issue)}
                  testId="project-issue-copy-link"
                />
              </h3>
            </div>
            <GitHubIssueStateButton
              issue={issue}
              onListStateChange={onListStateChange}
              onSelectedIssueIdChange={onSelectedIssueIdChange}
              project={project}
            />
          </div>
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
          <GitHubIssueCommentComposer
            issue={issue}
            onSelectedIssueIdChange={onSelectedIssueIdChange}
            project={project}
          />
        </section>
      </div>
      <aside className="space-y-6 border-t border-border/60 p-4 xl:border-l xl:border-t-0">
        <OverviewRailSection title="Status">
          <span
            className={`inline-flex items-center gap-1.5 text-xs font-medium ${status.className}`}
          >
            <status.icon className="h-3.5 w-3.5" /> {issue.status}
          </span>
        </OverviewRailSection>
        <OverviewRailSection title="Assignees">
          <GitHubIssueAssignees
            issue={issue}
            onSelectedIssueIdChange={onSelectedIssueIdChange}
            project={project}
          />
        </OverviewRailSection>
        <OverviewRailSection title="Author">
          <GitHubLoginIdentity
            avatarUrl={issue.authorAvatarUrl}
            login={issue.author}
          />
        </OverviewRailSection>
        <OverviewRailSection title="Labels">
          <GitHubIssueLabels
            issue={issue}
            onSelectedIssueIdChange={onSelectedIssueIdChange}
            project={project}
          />
        </OverviewRailSection>
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
