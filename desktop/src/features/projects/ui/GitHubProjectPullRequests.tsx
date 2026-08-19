import {
  Check,
  GitBranch,
  GitCommitHorizontal,
  GitPullRequest,
  MessageSquare,
  X,
} from "lucide-react";
import * as React from "react";
import { toast } from "sonner";

import type { ProjectPullRequest } from "@/features/projects/hooks";
import {
  githubPullRequestBranchLabel,
  pullRequestDisplayNumber,
  selectedGithubPullRequestAfterListLoad,
  type useGithubPullRequestCommentsQuery,
} from "@/features/projects/lib/projectGithubPulls";
import { pullRequestShareLink } from "@/features/projects/lib/projectShareLinks";
import {
  formatExactTimestamp,
  relativeTime,
} from "@/features/projects/lib/projectsViewHelpers";
import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import { GitHubLoginIdentity } from "./GitHubIssueIdentity";
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

function githubPullStatusClassName(status: ProjectPullRequest["status"]) {
  return status === "Draft" ? "text-muted-foreground" : "text-green-500";
}

/** One GitHub pull-request list row with numeric identity and login metadata. */
export function GitHubPullRequestRow({
  onOpen,
  pullRequest,
}: {
  onOpen: () => void;
  pullRequest: ProjectPullRequest;
}) {
  const number = pullRequestDisplayNumber(pullRequest.id);
  const StatusIcon = pullRequest.status === "Draft" ? X : Check;
  const statusClassName = githubPullStatusClassName(pullRequest.status);
  return (
    <ProjectFeedRow
      eventId={pullRequest.id}
      meta={
        <>
          <GitHubLoginIdentity
            avatarUrl={pullRequest.authorAvatarUrl}
            login={pullRequest.author}
          />
          <span className="inline-flex min-w-0 items-center gap-1 rounded-full border border-border/60 px-1.5 py-0.5 font-mono text-2xs">
            <GitBranch className="h-3 w-3 shrink-0" />
            <span className="truncate">
              {githubPullRequestBranchLabel(pullRequest)}
            </span>
          </span>
          <span
            className={`rounded-full border border-border/60 px-1.5 py-0.5 text-2xs font-medium ${statusClassName}`}
          >
            {pullRequest.status}
          </span>
        </>
      }
      onOpen={onOpen}
      statusIcon={
        <StatusIcon className={`h-3.5 w-3.5 shrink-0 ${statusClassName}`} />
      }
      testId="project-github-pull-request-row"
      title={pullRequest.title}
      trailing={
        <>
          {pullRequest.commentCount > 0 ? (
            <span className="flex items-center gap-1 text-xs text-muted-foreground">
              <MessageSquare className="h-3.5 w-3.5" />
              {pullRequest.commentCount}
            </span>
          ) : null}
          <ProjectFeedRowCluster>
            <ProjectFeedRowMonoCell
              label={`#${number}`}
              onClick={onOpen}
              title={`View GitHub pull request #${number}`}
            />
          </ProjectFeedRowCluster>
          <span
            className="hidden w-20 shrink-0 text-right text-xs text-muted-foreground sm:block"
            title={formatExactTimestamp(pullRequest.createdAt)}
          >
            {relativeTime(pullRequest.createdAt)}
          </span>
        </>
      }
    />
  );
}

/** GitHub pull-request conversation, one-commit list, or empty checks state. */
export function GitHubPullRequestDetail({
  commentsQuery,
  mode,
  onSelectedPullRequestIdChange,
  pullRequest,
}: {
  commentsQuery: ReturnType<typeof useGithubPullRequestCommentsQuery>;
  mode: "conversation" | "commits" | "checks";
  onSelectedPullRequestIdChange: (id: string | null) => void;
  pullRequest: ProjectPullRequest;
}) {
  const number = pullRequestDisplayNumber(pullRequest.id);
  const comments = commentsQuery.data ?? [];
  const parsed = parseProjectPullRequestMergeError(commentsQuery.error);
  React.useEffect(() => {
    if (parsed?.code === "github_pr_unavailable") {
      toast.error("Pull request not found.");
      onSelectedPullRequestIdChange(null);
    }
  }, [onSelectedPullRequestIdChange, parsed?.code]);

  if (mode === "commits") {
    const short = (pullRequest.commit ?? "").slice(0, 7);
    return (
      <section>
        <header className="flex min-h-10 items-center gap-2 border-b border-border/50 bg-muted/20 px-4">
          <GitCommitHorizontal className="h-4 w-4 text-muted-foreground" />
          <h4 className="text-sm font-medium text-foreground">Commits</h4>
          <span className="rounded-full bg-muted px-1.5 py-0.5 text-2xs text-muted-foreground">
            1
          </span>
        </header>
        <div className="divide-y divide-border/50">
          <ProjectFeedRow
            meta={
              <>
                <GitHubLoginIdentity
                  avatarUrl={pullRequest.authorAvatarUrl}
                  login={pullRequest.author}
                  showLabel={false}
                />
                <span className="truncate">{pullRequest.author} authored</span>
              </>
            }
            testId="project-github-pull-request-commit-row"
            title={pullRequest.title}
            trailing={
              <ProjectFeedRowCluster>
                <ProjectFeedRowMonoCell
                  label={short || "unknown"}
                  title={pullRequest.commit ?? ""}
                />
              </ProjectFeedRowCluster>
            }
          />
        </div>
      </section>
    );
  }

  if (mode === "checks") {
    return (
      <p className="p-4 text-sm text-muted-foreground">
        No checks have been reported for this pull request yet.
      </p>
    );
  }

  return (
    <section className="space-y-3 p-4">
      {commentsQuery.isLoading ? (
        <p className="text-sm text-muted-foreground">Loading comments…</p>
      ) : commentsQuery.isError && parsed?.code !== "github_pr_unavailable" ? (
        <GitHubRepoStateRecovery
          error={commentsQuery.error}
          onRetry={() => void commentsQuery.refetch()}
          titleId="github-pull-request-comments-recovery-title"
          unavailableTitle="Could not load GitHub comments"
        />
      ) : (
        <ProjectIssueCommentTimeline
          comments={comments}
          githubMode
          key={pullRequest.id}
        />
      )}
      <p className="sr-only">{`GitHub pull request #${number}`}</p>
    </section>
  );
}

/** GitHub pull-request list, empty, and error states for the repository tab. */
export function GitHubPullRequestsPanel({
  error,
  hasMore,
  isFetching,
  isLoading,
  isSuccess,
  onRetry,
  onSelectedPullRequestIdChange,
  pullRequests,
  selectedPullRequestId,
}: {
  error: unknown;
  hasMore: boolean;
  isFetching: boolean;
  isLoading: boolean;
  isSuccess: boolean;
  onRetry: () => void | Promise<unknown>;
  onSelectedPullRequestIdChange: (id: string | null) => void;
  pullRequests: ProjectPullRequest[];
  selectedPullRequestId: string | null;
}) {
  React.useEffect(() => {
    const nextSelected = selectedGithubPullRequestAfterListLoad({
      selectedPullRequestId,
      pullRequestIds: pullRequests.map((pullRequest) => pullRequest.id),
      isSuccess,
      isFetching,
    });
    if (nextSelected !== selectedPullRequestId) {
      onSelectedPullRequestIdChange(nextSelected);
    }
  }, [
    isFetching,
    isSuccess,
    onSelectedPullRequestIdChange,
    pullRequests,
    selectedPullRequestId,
  ]);

  if (isLoading) {
    return (
      <p className="p-4 text-sm text-muted-foreground">
        Loading pull requests…
      </p>
    );
  }
  if (error) {
    return (
      <div className="p-4">
        <GitHubRepoStateRecovery
          error={error}
          onRetry={onRetry}
          titleId="github-pull-requests-recovery-title"
          unavailableTitle="Could not load GitHub pull requests"
        />
      </div>
    );
  }
  if (pullRequests.length === 0) {
    return (
      <div className="space-y-2 p-4 text-sm text-muted-foreground">
        <p>No open pull requests.</p>
        {hasMore ? <p>More open pull requests exist on GitHub.</p> : null}
      </div>
    );
  }
  return (
    <div>
      <div className="divide-y divide-border/50">
        {pullRequests.map((pullRequest) => (
          <GitHubPullRequestRow
            key={pullRequest.id}
            onOpen={() => onSelectedPullRequestIdChange(pullRequest.id)}
            pullRequest={pullRequest}
          />
        ))}
      </div>
      {hasMore ? (
        <p className="border-t border-border/50 p-4 text-xs text-muted-foreground">
          More open pull requests exist on GitHub.
        </p>
      ) : null}
    </div>
  );
}

/** GitHub detail header: title, #N, login author, no origin or channel chrome. */
export function GitHubPullRequestDetailHeader({
  pullRequest,
}: {
  pullRequest: ProjectPullRequest;
}) {
  const number = pullRequestDisplayNumber(pullRequest.id);
  return (
    <header className="min-w-0 space-y-1 p-4 pb-4">
      <h3 className="line-clamp-2 min-w-0 text-xl font-semibold text-foreground">
        {pullRequest.title}{" "}
        <span className="font-normal text-muted-foreground">#{number}</span>
        <ShareLinkButton
          className="ml-1 inline-flex h-7 w-7 align-text-bottom"
          label="Copy pull request link"
          link={pullRequestShareLink(pullRequest)}
          testId="project-pull-request-copy-link"
        />
      </h3>
      <p className="flex flex-wrap items-center gap-x-1 gap-y-1 text-xs font-medium text-muted-foreground">
        <GitPullRequest className="h-3.5 w-3.5 shrink-0" />
        <GitHubLoginIdentity
          avatarUrl={pullRequest.authorAvatarUrl}
          login={pullRequest.author}
        />
        <span title={formatExactTimestamp(pullRequest.createdAt)}>
          created {relativeTime(pullRequest.createdAt)}
        </span>
      </p>
      {pullRequest.content ? (
        <ProjectRichContent
          className="pt-3"
          content={pullRequest.content}
          tags={[]}
        />
      ) : null}
    </header>
  );
}

/** GitHub meta rail: status, login author, branches. No reviewers. */
export function GitHubPullRequestMetaRail({
  pullRequest,
}: {
  pullRequest: ProjectPullRequest;
}) {
  return (
    <aside className="min-w-0 space-y-6 border-t border-border/60 p-4 xl:border-l xl:border-t-0">
      <OverviewRailSection title="Status">
        <span
          className={`inline-flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium text-white ${
            pullRequest.status === "Draft"
              ? "bg-muted-foreground/80"
              : "bg-green-600"
          }`}
        >
          <GitPullRequest className="h-3.5 w-3.5" />
          {pullRequest.status}
        </span>
      </OverviewRailSection>
      <OverviewRailSection title="Author">
        <GitHubLoginIdentity
          avatarUrl={pullRequest.authorAvatarUrl}
          login={pullRequest.author}
        />
      </OverviewRailSection>
      <OverviewRailSection title="Branches">
        <p className="font-mono text-xs text-muted-foreground">
          {githubPullRequestBranchLabel(pullRequest)}
        </p>
      </OverviewRailSection>
    </aside>
  );
}
