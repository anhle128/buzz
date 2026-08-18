import * as React from "react";
import { toast } from "sonner";

import type { useProjectBranchActions } from "@/features/projects/branchMutations";
import type {
  Repository,
  useProjectLocalRepoSnapshotQuery,
  useProjectRepoSnapshotQuery,
  useRepoStateQuery,
} from "@/features/projects/hooks";
import { projectBranchCreationReason } from "@/features/projects/lib/projectBranches";
import { githubAheadBehindCounts } from "@/features/projects/lib/projectGithubAheadBehind";
import type { useProjectRepoSyncStatusQuery } from "@/features/projects/repoSyncHooks";
import { useGithubAheadBehindQuery } from "@/features/projects/repoSyncHooks";
import type { useProjectRepoPresentation } from "@/features/projects/useProjectRepoHost";
import type { RepoSourceHeaderControls } from "./ProjectRepositorySource";
import { pushPullTitle } from "./projectDetailHelpers";

type ProjectBranchActions = ReturnType<typeof useProjectBranchActions>;
type ProjectLocalRepoSnapshotQuery = ReturnType<
  typeof useProjectLocalRepoSnapshotQuery
>;
type ProjectRepoSnapshotQuery = ReturnType<typeof useProjectRepoSnapshotQuery>;
type ProjectRepoStateQuery = ReturnType<typeof useRepoStateQuery>;
type ProjectRepoSyncStatusQuery = ReturnType<
  typeof useProjectRepoSyncStatusQuery
>;
type ProjectRepoPresentation = ReturnType<typeof useProjectRepoPresentation>;

type UseProjectRepositorySourceControlsInput = {
  activeBranch: string | null;
  activeBranchCommit: string | null;
  branchActions: ProjectBranchActions;
  branchOptions: string[];
  clonePending: boolean;
  deleteBranchReason: string | null;
  githubHosted: boolean;
  localRepoSnapshotQuery: ProjectLocalRepoSnapshotQuery;
  onBranchChange: (branch: string | null) => void;
  onClone: () => Promise<void>;
  onPull: () => Promise<void>;
  onPush: () => Promise<void>;
  onTagChange: (tag: string) => void;
  project: Repository | null | undefined;
  pullPending: boolean;
  pushPending: boolean;
  repoRemote: ProjectRepoPresentation;
  repoSnapshotQuery: ProjectRepoSnapshotQuery;
  repoSource: "remote" | "local";
  repoStateQuery: ProjectRepoStateQuery;
  repoSyncStatusQuery: ProjectRepoSyncStatusQuery;
  selectedTag: string | null;
  setRepoSource: React.Dispatch<React.SetStateAction<"remote" | "local">>;
  tagOptions: Array<{ name: string; commit: string }>;
};

/** Build the repository header state and coordinate its refresh action. */
export function useProjectRepositorySourceControls(
  input: UseProjectRepositorySourceControlsInput,
): RepoSourceHeaderControls {
  const {
    activeBranch,
    activeBranchCommit,
    branchActions,
    clonePending,
    deleteBranchReason,
    githubHosted,
    localRepoSnapshotQuery,
    project,
    pullPending,
    pushPending,
    repoRemote,
    repoSnapshotQuery,
    repoStateQuery,
    repoSyncStatusQuery,
    selectedTag,
  } = input;
  const activeRemoteSha =
    repoStateQuery.data?.branches.find((item) => item.name === activeBranch)
      ?.commit ?? null;
  const localHeadSha =
    localRepoSnapshotQuery.data?.snapshot.latestCommit?.hash ?? null;
  const githubAheadBehindQuery = useGithubAheadBehindQuery({
    projectId: project?.id,
    cloneUrl: project?.cloneUrls[0],
    branch: activeBranch,
    localSha: localHeadSha,
    remoteSha: activeRemoteSha,
    enabled: githubHosted && repoStateQuery.isSuccess,
  });
  const githubCounts = githubAheadBehindCounts(githubAheadBehindQuery.data);
  const handleFetchRepo = React.useCallback(async () => {
    const tasks = githubHosted
      ? [
          repoStateQuery.refetch(),
          ...(repoStateQuery.isError
            ? []
            : [
                repoSnapshotQuery.refetch(),
                ...(githubAheadBehindQuery.isFetched ||
                (localHeadSha && activeRemoteSha)
                  ? [githubAheadBehindQuery.refetch()]
                  : []),
              ]),
        ]
      : [
          repoSnapshotQuery.refetch(),
          repoStateQuery.refetch(),
          repoSyncStatusQuery.refetch(),
        ];
    const results = await Promise.all(tasks);
    const error = results.find((result) => result.error)?.error;
    if (error) {
      toast.error(
        githubHosted
          ? "Could not refresh GitHub repository."
          : "Could not fetch repository.",
        {
          description:
            error instanceof Error ? error.message : "The refresh failed.",
        },
      );
      return;
    }
    toast.success("Remote state refreshed.");
  }, [
    activeRemoteSha,
    githubAheadBehindQuery.isFetched,
    githubAheadBehindQuery.refetch,
    githubHosted,
    localHeadSha,
    repoSnapshotQuery.refetch,
    repoStateQuery.isError,
    repoStateQuery.refetch,
    repoSyncStatusQuery.refetch,
  ]);
  const createBranchReason = projectBranchCreationReason({
    activeBranch,
    activeBranchCommit,
    localHead: repoSyncStatusQuery.data?.localHead,
  });

  return {
    branch: activeBranch ?? "",
    branchOptions: input.branchOptions,
    selectedTag,
    tagOptions: input.tagOptions,
    onBranchChange: input.onBranchChange,
    onTagChange: input.onTagChange,
    onCreateBranch: githubHosted
      ? undefined
      : () => branchActions.setCreateOpen(true),
    createBranchDisabled: branchActions.createPending || !activeBranchCommit,
    createBranchTitle: createBranchReason ?? "Create a remote branch",
    onDeleteBranch: githubHosted
      ? undefined
      : () => branchActions.setDeleteOpen(true),
    deleteBranchDisabled:
      branchActions.deletePending || Boolean(deleteBranchReason),
    deleteBranchTitle: deleteBranchReason ?? "Delete this remote branch",
    source: selectedTag ? "remote" : input.repoSource,
    onSourceChange: input.setRepoSource,
    localDisabled:
      Boolean(selectedTag) ||
      (!repoSyncStatusQuery.data?.localPath &&
        !localRepoSnapshotQuery.data &&
        !localRepoSnapshotQuery.isLoading),
    localLabel: localRepoSnapshotQuery.isLoading
      ? "Local checking"
      : repoSyncStatusQuery.data?.localPath || localRepoSnapshotQuery.data
        ? "Local"
        : "Local missing",
    ...repoRemote.controls,
    githubHosted,
    onCloneLocal:
      !selectedTag && project?.cloneUrls[0] && repoRemote.canCloneLocally
        ? () => {
            void input.onClone();
          }
        : undefined,
    clonePending,
    canPush: githubHosted
      ? false
      : !selectedTag && (repoSyncStatusQuery.data?.canPush ?? false),
    onPush:
      githubHosted || selectedTag
        ? undefined
        : () => {
            void input.onPush();
          },
    pushDisabled: pushPending || !repoSyncStatusQuery.data?.canPush,
    pushPending,
    pushTitle:
      repoSyncStatusQuery.data?.pushBlockReason ??
      pushPullTitle("Push", repoSyncStatusQuery.data?.aheadCount, "local"),
    canPull: githubHosted
      ? false
      : !selectedTag && (repoSyncStatusQuery.data?.canPull ?? false),
    onPull:
      githubHosted || selectedTag
        ? undefined
        : () => {
            void input.onPull();
          },
    pullDisabled: pullPending || !repoSyncStatusQuery.data?.canPull,
    pullPending,
    pullTitle:
      repoSyncStatusQuery.data?.pullBlockReason ??
      pushPullTitle("Pull", repoSyncStatusQuery.data?.behindCount, "remote"),
    aheadCount: githubHosted
      ? (githubCounts?.ahead ?? null)
      : (repoSyncStatusQuery.data?.aheadCount ?? null),
    behindCount: githubHosted
      ? (githubCounts?.behind ?? null)
      : (repoSyncStatusQuery.data?.behindCount ?? null),
    onFetch: () => {
      void handleFetchRepo();
    },
    fetchPending: githubHosted
      ? repoSnapshotQuery.isFetching ||
        repoStateQuery.isFetching ||
        githubAheadBehindQuery.isFetching
      : repoSnapshotQuery.isFetching ||
        repoStateQuery.isFetching ||
        repoSyncStatusQuery.isFetching,
    fetchTitle: githubHosted
      ? "Refresh GitHub README, files, and compare"
      : (repoSyncStatusQuery.data?.pullBlockReason ??
        "Check for remote changes"),
    showGithubStateRecovery:
      githubHosted &&
      (repoStateQuery.isError ||
        (repoStateQuery.isSuccess && repoSnapshotQuery.isError)),
    stateError:
      repoStateQuery.error ??
      (githubHosted ? repoSnapshotQuery.error : undefined),
    onRetryState: () => {
      if (githubHosted && repoStateQuery.isError) {
        void repoStateQuery.refetch();
        return;
      }
      if (githubHosted) {
        void repoSnapshotQuery.refetch();
        return;
      }
      void repoStateQuery.refetch();
    },
  };
}
