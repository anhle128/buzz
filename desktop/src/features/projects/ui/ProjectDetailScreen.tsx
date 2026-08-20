import { ArrowLeft, FolderGit2 } from "lucide-react";
import * as React from "react";
import { toast } from "sonner";

import { useAppNavigation } from "@/app/navigation/useAppNavigation";
import { useOpenDmMutation } from "@/features/channels/hooks";
import {
  type Project,
  type Repository,
  useProjectQuery,
  useProjectIssuesQuery,
  useProjectLocalRepoDiffQuery,
  useProjectLocalRepoSnapshotQuery,
  useProjectRepoDiffQuery,
  useProjectPullRequestsQuery,
  useProjectRepoSnapshotQuery,
  useProjectsQuery,
  useRepoStateQuery,
} from "@/features/projects/hooks";
import {
  useCloneProjectRepositoryMutation,
  useProjectRepoSyncStatusQuery,
  usePullProjectLocalRepositoryMutation,
  usePushProjectLocalRepositoryMutation,
} from "@/features/projects/repoSyncHooks";
import { useProjectBranchActions } from "@/features/projects/branchMutations";
import { useOptimisticProjectBranches } from "@/features/projects/useOptimisticProjectBranches";
import { useProjectRepositoryRefSelection } from "@/features/projects/useProjectRepositoryRefSelection";
import { useUpdateProjectPullRequestMutation } from "@/features/projects/pullRequestMutations";
import { useCreateProjectIssueMutation } from "@/features/projects/issueMutations";
import { useProfileQuery, useUsersBatchQuery } from "@/features/profile/hooks";
import { mergeCurrentProfileIntoLookup } from "@/features/profile/lib/identity";
import {
  type ProfilePanelTab,
  type ProfilePanelView,
  UserProfilePanel,
} from "@/features/profile/ui/UserProfilePanel";
import {
  profilePanelTabFromSearch,
  profilePanelViewFromSearch,
} from "@/features/profile/ui/UserProfilePanelUtils";
import { useIdentityQuery } from "@/shared/api/hooks";
import { openProjectMergeRecoveryTerminal } from "@/shared/api/projectGit";
import { useMainInsetRef } from "@/shared/layout/MainInsetContext";
import { channelContentTopPaddingMeasurement } from "@/shared/layout/chromeLayout";
import { useMeasuredCssVariable } from "@/shared/layout/useMeasuredCssVariable";
import { ProfilePanelProvider } from "@/shared/context/ProfilePanelContext";
import { useHistorySearchState } from "@/shared/hooks/useHistorySearchState";
import { useThreadPanelWidth } from "@/shared/hooks/useThreadPanelWidth";
import { Button } from "@/shared/ui/button";
import { ViewLoadingFallback } from "@/shared/ui/ViewLoadingFallback";
import { useCommunities } from "@/features/communities/useCommunities";
import { useProjectCommitDiffQuery } from "@/features/projects/useProjectCommitDiff";
import { useGitIdentityQuery } from "@/features/projects/useGitIdentity";
import type { ViewerGitIdentity } from "@/features/projects/lib/projectContributorMatching";
import {
  projectBranchManagementState,
  projectBranchOptionsFromSync,
  resolveProjectDefaultBranch,
} from "@/features/projects/lib/projectBranches";
import { githubRemoteSnapshotEnabled } from "@/features/projects/lib/projectGithubSnapshot";
import { isGitHubCloneUrl } from "@/features/projects/lib/projectGitError";
import {
  type GithubIssueListState,
  issueIdentityPubkeys,
} from "@/features/projects/lib/projectGithubIssues";
import { pullRequestIdentityPubkeys } from "@/features/projects/lib/projectGithubPullRequests";
import { nextGithubIssueListState } from "@/features/projects/lib/projectGithubIssueWrites";
import { githubRepositoryStateUnresolved } from "@/features/projects/lib/projectRepoState";
import { normalizeRepositoryUrl } from "@/features/projects/lib/projectsViewHelpers";
import {
  shareTabForWorkspaceTab,
  workspaceTabForShareTab,
} from "@/features/projects/lib/projectShareLinks";
import { selectProjectRepository } from "@/features/projects/projectModels";
import { KIND_REPO_ANNOUNCEMENT } from "@/shared/constants/kinds";
import type { EntityLinkTab } from "@/shared/lib/entityLink";
import { useProjectRepoPresentation } from "@/features/projects/useProjectRepoHost";
import { WorkspaceTabs } from "./ProjectWorkspaceTabs";
import { showProjectCloneErrorToast } from "./projectGitErrorToast";
import {
  projectTerminalLabel,
  useOpenProjectTerminal,
} from "./useOpenProjectTerminal";
import type { CreateIssueDialogInput } from "./CreateIssueDialog";
import { ProjectBranchActionDialogs } from "./ProjectBranchActionDialogs";
import { ProjectDetailChrome } from "./ProjectDetailChrome";
import { ProjectDetailChromeActions } from "./ProjectDetailChromeActions";
import { UnavailableProjectRepositories } from "./UnavailableProjectRepositories";
import {
  PROJECT_TAB_CRUMB_LABELS,
  projectPeople,
  snapshotHasContent,
} from "./projectDetailHelpers";
import { useProjectRepositorySourceControls } from "./useProjectRepositorySourceControls";

type ProjectDetailScreenProps = {
  commitHash?: string;
  entityNavigationId?: string;
  projectId: string;
  pullRequestId?: string;
  issueId?: string;
  repositoryId?: string;
  /** Workspace tab requested by a share link (link vocabulary). */
  tab?: EntityLinkTab;
};

const PROJECT_DETAIL_PANEL_SEARCH_KEYS = [
  "profile",
  "profileTab",
  "profileView",
] as const;
const PROJECT_REPOSITORY_SEARCH_KEYS = [
  "repositoryId",
  "issueId",
  "pullRequestId",
  "commitHash",
] as const;

export function ProjectDetailScreen(props: ProjectDetailScreenProps) {
  const {
    commitHash,
    entityNavigationId,
    projectId,
    pullRequestId,
    issueId,
    repositoryId,
    tab,
  } = props;
  const { goChannel, goProject, goProjects } = useAppNavigation();
  const { activeCommunity } = useCommunities();
  const mainInsetRef = useMainInsetRef();
  const projectDetailHeaderChromeRef = useMeasuredCssVariable({
    targetRef: mainInsetRef,
    resetKey: projectId,
    ...channelContentTopPaddingMeasurement,
  });
  const projectQuery = useProjectQuery(projectId);
  const projectsQuery = useProjectsQuery();
  const project = projectQuery.data;
  // When the projectId is a canonical 30617:<owner>:<d> coordinate (emitted by
  // entity links in #4695), derive the repository selection directly from the
  // <owner>:<d> portion rather than falling back to the project's primary
  // repository. Repository.id is "<owner>:<dtag>", so stripping the kind+colon
  // prefix gives the exact repository id. This ensures a linked PR/issue on a
  // non-primary member opens from the correct repository instead of the primary.
  const routeRepositoryId: string | undefined = React.useMemo(() => {
    if (repositoryId) return repositoryId;
    const kindStr = `${String(KIND_REPO_ANNOUNCEMENT)}:`;
    if (!projectId.startsWith(kindStr)) return undefined;
    // projectId is "30617:<owner>:<dtag>" — strip "30617:" to get "<owner>:<dtag>"
    return projectId.slice(kindStr.length);
  }, [projectId, repositoryId]);
  const repository = selectProjectRepository(project, routeRepositoryId);
  const repoRemote = useProjectRepoPresentation(repository);
  const { applyPatch: applyRepositorySearch } = useHistorySearchState(
    PROJECT_REPOSITORY_SEARCH_KEYS,
  );
  const repoStateQuery = useRepoStateQuery(repository);
  const pullRequestsQuery = useProjectPullRequestsQuery(repository);
  const pullRequests = React.useMemo(
    () => pullRequestsQuery.data?.pullRequests ?? [],
    [pullRequestsQuery.data?.pullRequests],
  );
  // GitHub pending/error must not fall back to the announcement default ("main").
  const githubHosted = isGitHubCloneUrl(repository?.cloneUrls[0]);
  const githubStateUnresolved = githubRepositoryStateUnresolved(
    githubHosted,
    repoStateQuery,
  );
  const defaultBranch =
    !repository || githubStateUnresolved
      ? null
      : resolveProjectDefaultBranch(
          repository.defaultBranch,
          repoStateQuery.data,
        );
  const { branchOptions, forgetBranch, managedBranches, rememberBranch } =
    useOptimisticProjectBranches({
      defaultBranch,
      observedBranches: githubStateUnresolved
        ? []
        : (repoStateQuery.data?.branches ?? []),
      projectId: repository?.id ?? projectId,
      referencedBranches: githubStateUnresolved
        ? []
        : pullRequests.map((pullRequest) => pullRequest.branchName ?? null),
    });
  const repoTags = githubStateUnresolved
    ? []
    : (repoStateQuery.data?.tags ?? []);
  const { activeBranch, selectBranch, selectedTag, selectTag } =
    useProjectRepositoryRefSelection({
      branchOptions,
      defaultBranch,
      projectAvailable: Boolean(repository),
      projectPending: projectQuery.isPending,
      tags: repoTags,
    });
  const activeTag = repoTags.find((tag) => tag.name === selectedTag) ?? null;
  const [selectedPullRequestId, setSelectedPullRequestId] = React.useState<
    string | null
  >(pullRequestId ?? null);
  const [selectedIssueId, setSelectedIssueId] = React.useState<string | null>(
    issueId ?? null,
  );
  const [githubIssueListState, setGithubIssueListState] =
    React.useState<GithubIssueListState>("open");
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new repository must reset the GitHub Open/Closed filter.
  React.useEffect(() => {
    setGithubIssueListState("open");
  }, [repository?.id]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the transient request ID deliberately reapplies an unchanged entity selection.
  React.useEffect(() => {
    setSelectedPullRequestId(pullRequestId ?? null);
    setSelectedIssueId(issueId ?? null);
  }, [entityNavigationId, issueId, pullRequestId]);
  const [selectedCommitHash, setSelectedCommitHash] = React.useState<
    string | null
  >(commitHash ?? null);
  React.useEffect(
    () => setSelectedCommitHash(commitHash ?? null),
    [commitHash],
  );
  // Remounts WorkspaceTabs when breadcrumb navigation should open Overview.
  const [tabsResetKey, setTabsResetKey] = React.useState(0);
  // Local state lets breadcrumb and repository resets drop a share-link tab.
  const [requestedTab, setRequestedTab] = React.useState<
    EntityLinkTab | undefined
  >(tab);
  // biome-ignore lint/correctness/useExhaustiveDependencies: the transient request ID deliberately reapplies an unchanged share-link tab.
  React.useEffect(() => setRequestedTab(tab), [entityNavigationId, tab]);
  // Mirror of the WorkspaceTabs selection so the breadcrumb can name the
  // active sub-tab. The Overview (readme) tab is "home" and gets no crumb.
  const [activeTab, setActiveTab] = React.useState("overview");
  // Commit, PR, and issue details are mutually exclusive views, so opening
  // one clears the others.
  const handleSelectedPullRequestIdChange = React.useCallback(
    (id: string | null) => {
      setSelectedPullRequestId(id);
      if (id) setSelectedCommitHash(null);
    },
    [],
  );
  const handleSelectedIssueIdChange = React.useCallback((id: string | null) => {
    setSelectedIssueId(id);
    if (id) setSelectedCommitHash(null);
  }, []);
  const handleSelectedCommitHashChange = React.useCallback(
    (hash: string | null) => {
      setSelectedCommitHash(hash);
      if (hash) {
        setSelectedPullRequestId(null);
        setSelectedIssueId(null);
      }
    },
    [],
  );
  const issuesQuery = useProjectIssuesQuery(repository, githubIssueListState);
  const selectedBranchPullRequest = React.useMemo(() => {
    const projectRepositories = new Set(
      (repository?.cloneUrls ?? []).map(normalizeRepositoryUrl),
    );
    const matches = pullRequests.filter(
      (pullRequest) =>
        pullRequest.branchName === activeBranch &&
        pullRequest.cloneUrls.some((cloneUrl) =>
          projectRepositories.has(normalizeRepositoryUrl(cloneUrl)),
        ),
    );
    return matches.length === 1 ? matches[0] : null;
  }, [activeBranch, pullRequests, repository?.cloneUrls]);
  const openBranchPullRequest =
    selectedBranchPullRequest?.status === "Open" ||
    selectedBranchPullRequest?.status === "Draft"
      ? selectedBranchPullRequest
      : null;
  const activeRepoPullRequest =
    pullRequests.find((item) => item.id === selectedPullRequestId) ??
    selectedBranchPullRequest;
  const [repoSource, setRepoSource] = React.useState<"remote" | "local">(
    "remote",
  );
  const snapshotEnabled = githubRemoteSnapshotEnabled({
    cloneUrl: repository?.cloneUrls[0],
    buzzHost: repoRemote.host.kind === "buzz",
    githubStateReady: repoStateQuery.isSuccess,
  });
  const repoSnapshotQuery = useProjectRepoSnapshotQuery(
    repository,
    activeBranch,
    selectedTag ? null : selectedBranchPullRequest,
    activeTag,
    snapshotEnabled,
  );
  const repoDiffQuery = useProjectRepoDiffQuery(
    repository,
    activeBranch,
    activeRepoPullRequest,
    repoSource === "remote",
  );
  const localRepoDiffQuery = useProjectLocalRepoDiffQuery(
    repository,
    activeCommunity?.reposDir,
    activeBranch,
    activeRepoPullRequest,
    repoSource === "local" && Boolean(activeRepoPullRequest),
  );
  const commitDiffQuery = useProjectCommitDiffQuery(
    repository,
    selectedCommitHash,
    repoSource,
    activeCommunity?.reposDir,
  );
  const localRepoSnapshotQuery = useProjectLocalRepoSnapshotQuery(
    repository,
    activeCommunity?.reposDir,
    activeBranch,
  );
  const repoSyncStatusQuery = useProjectRepoSyncStatusQuery(
    repository,
    activeCommunity?.reposDir,
    activeBranch,
    undefined,
    { githubStateReady: repoStateQuery.isSuccess },
  );
  const pushLocalRepoMutation = usePushProjectLocalRepositoryMutation(
    repository,
    activeCommunity?.reposDir,
    activeBranch,
    openBranchPullRequest,
  );
  const pullLocalRepoMutation = usePullProjectLocalRepositoryMutation(
    repository,
    activeCommunity?.reposDir,
    activeBranch,
  );
  const cloneRepoMutation = useCloneProjectRepositoryMutation(
    repository,
    activeCommunity?.reposDir,
  );
  const createIssueMutation = useCreateProjectIssueMutation(repository);
  const updatePullRequestMutation = useUpdateProjectPullRequestMutation(
    repository,
    openBranchPullRequest,
  );
  const hasLocalCheckout = Boolean(
    localRepoSnapshotQuery.data || repoSyncStatusQuery.data?.localPath,
  );
  const hasRemoteSnapshot = snapshotHasContent(repoSnapshotQuery.data);
  const displayedRepoDiff =
    repoSource === "local" ? localRepoDiffQuery.data : repoDiffQuery.data;
  const displayedRepoDiffError =
    repoSource === "local" ? localRepoDiffQuery.error : repoDiffQuery.error;
  const displayedRepoDiffLoading =
    repoSource === "local"
      ? localRepoDiffQuery.isLoading
      : repoDiffQuery.isLoading;
  const branchOptionsWithLocal = projectBranchOptionsFromSync(
    branchOptions,
    repoSyncStatusQuery.data,
  );
  const { activeBranchCommit, activeRemoteBranch, deleteBranchReason } =
    projectBranchManagementState({
      activeBranch,
      branches: managedBranches,
      defaultBranch,
      hasOpenPullRequest: pullRequests.some(
        (pullRequest) =>
          pullRequest.branchName === activeBranch &&
          (pullRequest.status === "Open" || pullRequest.status === "Draft"),
      ),
      remoteBranch: repoSyncStatusQuery.data?.remoteBranch,
      remoteHead: repoSyncStatusQuery.data?.remoteHead,
      snapshotCommit: repoSnapshotQuery.data?.latestCommit?.hash,
    });
  const handleBranchChange = React.useCallback(
    (branch: string | null) => {
      selectBranch(branch);
      if (
        branch &&
        repoSource === "local" &&
        branch !== repoSyncStatusQuery.data?.localBranch
      ) {
        setRepoSource("remote");
      }
    },
    [repoSource, repoSyncStatusQuery.data?.localBranch, selectBranch],
  );
  const handleTagChange = React.useCallback(
    (tag: string) => {
      selectTag(tag);
      setRepoSource("remote");
    },
    [selectTag],
  );
  const branchActions = useProjectBranchActions({
    activeBranch,
    activeBranchCommit,
    activeRemoteBranch,
    defaultBranch,
    deleteBranchReason,
    forgetBranch,
    project: repository,
    refetchRepoState: repoStateQuery.refetch,
    rememberBranch,
    selectBranch: handleBranchChange,
  });
  const projectPending = projectQuery.isPending;
  React.useEffect(() => {
    if (!repository) {
      // While the project query is still loading, keep the URL-seeded
      // pullRequestId/issueId selections — clearing here would discard them
      // before the detail view ever gets a chance to open.
      if (projectPending) return;
      setSelectedPullRequestId(null);
      setSelectedIssueId(null);
      setSelectedCommitHash(null);
    }
  }, [projectPending, repository]);
  React.useEffect(() => {
    setRepoSource((currentSource) => {
      if (selectedTag) return "remote";
      if (currentSource === "local" && !hasLocalCheckout) return "remote";
      if (
        currentSource === "remote" &&
        !githubHosted &&
        !hasRemoteSnapshot &&
        hasLocalCheckout
      ) {
        return "local";
      }
      return currentSource;
    });
  }, [githubHosted, hasLocalCheckout, hasRemoteSnapshot, selectedTag]);
  const peoplePubkeys = React.useMemo(() => {
    if (!repository) return [];
    // Include PR authors/updaters so commit rows can resolve avatars for
    // publishers who are not listed as project contributors.
    const pullRequestPubkeys = pullRequestIdentityPubkeys(pullRequests);
    const issuePubkeys = issueIdentityPubkeys(issuesQuery.data?.issues ?? []);
    return [
      ...new Set([
        ...projectPeople(repository),
        ...pullRequestPubkeys,
        ...issuePubkeys,
      ]),
    ];
  }, [issuesQuery.data, pullRequests, repository]);
  const profilesQuery = useUsersBatchQuery(peoplePubkeys, {
    enabled: peoplePubkeys.length > 0,
  });
  const currentProfileQuery = useProfileQuery();
  const profiles = React.useMemo(
    () =>
      mergeCurrentProfileIntoLookup(
        profilesQuery.data?.profiles,
        currentProfileQuery.data,
      ),
    [currentProfileQuery.data, profilesQuery.data?.profiles],
  );
  const identityQuery = useIdentityQuery();
  const gitIdentityQuery = useGitIdentityQuery();
  const viewerGitIdentity = React.useMemo<ViewerGitIdentity | null>(() => {
    const pubkey = identityQuery.data?.pubkey ?? null;
    if (!pubkey || !gitIdentityQuery.data) return null;
    return {
      pubkey,
      name: gitIdentityQuery.data.name,
      email: gitIdentityQuery.data.email,
    };
  }, [gitIdentityQuery.data, identityQuery.data?.pubkey]);
  const { applyPatch, values } = useHistorySearchState(
    PROJECT_DETAIL_PANEL_SEARCH_KEYS,
  );
  const profilePanelPubkey = values.profile;
  const profilePanelTab = profilePanelTabFromSearch(values.profileTab);
  const profilePanelView = profilePanelViewFromSearch(values.profileView);
  const handleOpenProfilePanel = React.useCallback(
    (pubkey: string) =>
      applyPatch({ profile: pubkey, profileTab: null, profileView: null }),
    [applyPatch],
  );
  const handleCloseProfilePanel = React.useCallback(
    () => applyPatch({ profile: null, profileTab: null, profileView: null }),
    [applyPatch],
  );
  const handleProfilePanelViewChange = React.useCallback(
    (view: ProfilePanelView, options?: { replace?: boolean }) =>
      applyPatch({ profileView: view === "summary" ? null : view }, options),
    [applyPatch],
  );
  const handleProfilePanelTabChange = React.useCallback(
    (tab: ProfilePanelTab, options?: { replace?: boolean }) =>
      applyPatch({ profileTab: tab === "info" ? null : tab }, options),
    [applyPatch],
  );
  const threadPanelWidth = useThreadPanelWidth();
  const openDmMutation = useOpenDmMutation();
  const handleOpenDm = React.useCallback(
    async (pubkeys: string[]) => {
      const dm = await openDmMutation.mutateAsync({ pubkeys });
      await goChannel(dm.id);
    },
    [goChannel, openDmMutation],
  );
  const handlePushLocalRepo = React.useCallback(async () => {
    try {
      const result = await pushLocalRepoMutation.mutateAsync();
      if (result.pullRequestUpdate.status === "failed") {
        toast.warning(result.message, {
          description: result.pullRequestUpdate.error,
        });
      } else {
        toast.success(
          result.pullRequestUpdate.status === "updated"
            ? `${result.message} Pull request updated.`
            : result.message,
        );
      }
      await Promise.all([
        repoSnapshotQuery.refetch(),
        localRepoSnapshotQuery.refetch(),
        repoSyncStatusQuery.refetch(),
        repoStateQuery.refetch(),
      ]);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Failed to push repository",
      );
    }
  }, [
    localRepoSnapshotQuery,
    pushLocalRepoMutation,
    repoSnapshotQuery,
    repoStateQuery,
    repoSyncStatusQuery,
  ]);
  const handleCloneRepo = React.useCallback(async () => {
    try {
      const result = await cloneRepoMutation.mutateAsync();
      toast.success(result.message);
      setRepoSource("local");
    } catch (error) {
      showProjectCloneErrorToast(error, repository?.cloneUrls[0]);
    }
  }, [cloneRepoMutation, repository?.cloneUrls]);
  const handlePullRequestCreated = React.useCallback(
    async (
      createdProject: Project,
      createdRepository: Repository,
      pullRequestId: string,
    ) => {
      if (createdProject.id !== projectId) {
        await goProject(createdProject.id, {
          pullRequestId,
          repositoryId: createdRepository.id,
        });
        return;
      }
      if (createdRepository.id === repository?.id) {
        await pullRequestsQuery.refetch();
      } else {
        applyRepositorySearch({ repositoryId: createdRepository.id });
      }
      setSelectedPullRequestId(pullRequestId);
    },
    [
      applyRepositorySearch,
      goProject,
      projectId,
      pullRequestsQuery,
      repository?.id,
    ],
  );
  const handleCreateIssue = React.useCallback(
    async ({ body, title }: CreateIssueDialogInput) => {
      const issueId = await createIssueMutation.mutateAsync({ body, title });
      toast.success("Issue created.");
      setGithubIssueListState(nextGithubIssueListState("create"));
      setSelectedIssueId(issueId);
    },
    [createIssueMutation],
  );
  const handleUpdatePullRequest = React.useCallback(async () => {
    const commit = repoSyncStatusQuery.data?.remoteHead;
    if (!commit) return;
    try {
      const updated = await updatePullRequestMutation.mutateAsync({
        commit,
        mergeBase: repoSyncStatusQuery.data?.mergeBase ?? null,
      });
      toast.success(
        updated ? "Pull request updated." : "Pull request is already current.",
      );
      await pullRequestsQuery.refetch();
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : "Failed to update pull request",
      );
    }
  }, [
    pullRequestsQuery,
    repoSyncStatusQuery.data?.mergeBase,
    repoSyncStatusQuery.data?.remoteHead,
    updatePullRequestMutation,
  ]);
  const handlePullLocalRepo = React.useCallback(async () => {
    try {
      const result = await pullLocalRepoMutation.mutateAsync();
      toast.success(result.message);
      await Promise.all([
        repoSnapshotQuery.refetch(),
        localRepoSnapshotQuery.refetch(),
        repoSyncStatusQuery.refetch(),
        repoStateQuery.refetch(),
      ]);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Failed to pull repository",
      );
    }
  }, [
    localRepoSnapshotQuery,
    pullLocalRepoMutation,
    repoSnapshotQuery,
    repoStateQuery,
    repoSyncStatusQuery,
  ]);
  const filesSourceControls = useProjectRepositorySourceControls({
    activeBranch,
    activeBranchCommit,
    branchActions,
    branchOptions: branchOptionsWithLocal,
    clonePending: cloneRepoMutation.isPending,
    deleteBranchReason,
    githubHosted,
    localRepoSnapshotQuery,
    onBranchChange: handleBranchChange,
    onClone: handleCloneRepo,
    onPull: handlePullLocalRepo,
    onPush: handlePushLocalRepo,
    onTagChange: handleTagChange,
    project: repository,
    pullPending: pullLocalRepoMutation.isPending,
    pushPending: pushLocalRepoMutation.isPending,
    repoRemote,
    repoSnapshotQuery,
    repoSource,
    repoStateQuery,
    repoSyncStatusQuery,
    selectedTag,
    setRepoSource,
    tagOptions: repoTags,
  });
  const openTerminal = useOpenProjectTerminal(activeCommunity?.reposDir);
  const handleOpenTerminal = React.useCallback(() => {
    if (!repository) return Promise.resolve();
    return openTerminal(repository, {
      branch: activeBranch,
      hasLocalCheckout,
    });
  }, [activeBranch, hasLocalCheckout, openTerminal, repository]);
  const handleOpenMergeRecoveryTerminal = React.useCallback(
    async (input: {
      expectedCommit: string;
      sourceBranch: string;
      sourceCloneUrl: string;
      targetBranch: string;
    }) => {
      const targetCloneUrl = repository?.cloneUrls[0];
      if (!repository || !targetCloneUrl) {
        throw new Error("No project selected.");
      }
      return openProjectMergeRecoveryTerminal({
        ...input,
        projectDtag: repository.dtag,
        reposDir: activeCommunity?.reposDir,
        targetCloneUrl,
      });
    },
    [activeCommunity?.reposDir, repository],
  );

  if (projectQuery.isLoading) {
    return <ViewLoadingFallback kind="projects" />;
  }
  if (projectQuery.isError) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 px-4 py-16 text-center">
        <FolderGit2 className="h-10 w-10 text-muted-foreground/40" />
        <p className="text-sm text-red-400">Failed to load project</p>
        <div className="flex items-center gap-2">
          <Button
            onClick={() => void projectQuery.refetch()}
            size="sm"
            variant="outline"
          >
            Retry
          </Button>
          <Button
            onClick={() => {
              void goProjects();
            }}
            size="sm"
            variant="ghost"
          >
            <ArrowLeft className="mr-1.5 h-4 w-4" />
            Back to Projects
          </Button>
        </div>
      </div>
    );
  }
  if (!project) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 px-4 py-16 text-center">
        <FolderGit2 className="h-10 w-10 text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">
          This project could not be found.
        </p>
        <Button
          onClick={() => {
            void goProjects();
          }}
          size="sm"
          variant="outline"
        >
          <ArrowLeft className="mr-1.5 h-4 w-4" />
          Back to Projects
        </Button>
      </div>
    );
  }
  if (!repository) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 px-4 py-16 text-center">
        <FolderGit2 className="h-10 w-10 text-muted-foreground/40" />
        <p className="text-sm font-medium text-foreground">{project.name}</p>
        <p className="text-sm text-muted-foreground">
          This project does not have any available repositories yet.
        </p>
        <UnavailableProjectRepositories project={project} />
      </div>
    );
  }

  const repoContributors = repoSnapshotQuery.data?.contributors ?? [];
  const selectedPullRequest =
    pullRequests.find((item) => item.id === selectedPullRequestId) ?? null;
  const selectedIssue =
    issuesQuery.data?.issues.find((item) => item.id === selectedIssueId) ??
    null;
  const displayedSnapshotCommits =
    repoSource === "local"
      ? (localRepoSnapshotQuery.data?.snapshot.commits ?? [])
      : (repoSnapshotQuery.data?.commits ?? []);
  const selectedCommit = selectedCommitHash
    ? (displayedSnapshotCommits.find(
        (commit) => commit.hash === selectedCommitHash,
      ) ?? null)
    : null;

  // The active work item drives the breadcrumb trail: Projects › project ›
  // sub-tab › title. `clear` steps back to the item's list tab. Categories
  // match the workspace tab labels.
  const activeWorkItemCrumb = selectedPullRequest
    ? {
        category: "Pull Request",
        title: selectedPullRequest.title,
        clear: () => setSelectedPullRequestId(null),
      }
    : selectedIssue
      ? {
          category: "Issues",
          title: selectedIssue.title,
          clear: () => setSelectedIssueId(null),
        }
      : selectedCommitHash
        ? {
            category: "Commits",
            title: selectedCommit?.subject ?? selectedCommitHash.slice(0, 7),
            clear: () => setSelectedCommitHash(null),
          }
        : null;
  // Sub-tab crumb when no work item is open. Overview (readme) is home.
  const activeTabCrumb = activeWorkItemCrumb
    ? null
    : (PROJECT_TAB_CRUMB_LABELS[activeTab] ?? null);
  const handleGoToProjectHome = () => {
    setSelectedPullRequestId(null);
    setSelectedIssueId(null);
    setSelectedCommitHash(null);
    setRequestedTab(undefined);
    // Remount the workspace tabs so the project page opens on Overview
    // instead of whatever tab the work item left behind.
    setTabsResetKey((key) => key + 1);
  };
  const handleRepositoryChange = (nextRepositoryId: string) => {
    applyRepositorySearch({
      repositoryId: nextRepositoryId,
      issueId: null,
      pullRequestId: null,
      commitHash: null,
    });
    setSelectedPullRequestId(null);
    setSelectedIssueId(null);
    setSelectedCommitHash(null);
    setRequestedTab(undefined);
    setRepoSource("remote");
    setTabsResetKey((key) => key + 1);
  };

  return (
    <ProfilePanelProvider onOpenProfilePanel={handleOpenProfilePanel}>
      <ProjectBranchActionDialogs
        actions={branchActions}
        activeBranch={activeBranch}
        activeBranchCommit={activeBranchCommit}
        existingBranches={branchOptionsWithLocal}
      />
      <div className="flex min-h-0 min-w-0 flex-1 flex-row overflow-hidden">
        <div className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
          <ProjectDetailChrome
            activeTabCrumb={activeTabCrumb}
            activeWorkItemCrumb={activeWorkItemCrumb}
            chromeRef={projectDetailHeaderChromeRef}
            onGoProjectHome={handleGoToProjectHome}
            onGoProjects={() => {
              void goProjects();
            }}
            project={project}
            shareTab={
              activeWorkItemCrumb
                ? undefined
                : shareTabForWorkspaceTab(activeTab)
            }
          />

          <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto px-4 pb-4">
            <div className="w-full space-y-3 pt-[calc(var(--buzz-channel-content-top-padding,5.75rem)_+_1px)]">
              <WorkspaceTabs
                key={`${project.id}:${repository.id}:${tabsResetKey}`}
                initialTab={
                  requestedTab
                    ? workspaceTabForShareTab(requestedTab)
                    : undefined
                }
                initialTabRequestKey={entityNavigationId}
                commitDiff={commitDiffQuery.data}
                commitDiffError={commitDiffQuery.error}
                commitDiffLoading={commitDiffQuery.isLoading}
                createIssueAction={{
                  onCreate: handleCreateIssue,
                  pending: createIssueMutation.isPending,
                }}
                createPullRequestAction={{
                  onCreated: handlePullRequestCreated,
                  projects: projectsQuery.data ?? [project],
                  reposDir: activeCommunity?.reposDir,
                }}
                updatePullRequestAction={
                  openBranchPullRequest &&
                  repoSyncStatusQuery.data?.remoteHead &&
                  repoSyncStatusQuery.data.remoteHead !==
                    openBranchPullRequest.commit
                    ? {
                        onUpdate: () => {
                          void handleUpdatePullRequest();
                        },
                        pending: updatePullRequestMutation.isPending,
                      }
                    : undefined
                }
                localSnapshot={localRepoSnapshotQuery.data}
                localSnapshotError={localRepoSnapshotQuery.error}
                localSnapshotLoading={localRepoSnapshotQuery.isLoading}
                onBranchChange={handleBranchChange}
                onOpenMergeRecoveryTerminal={handleOpenMergeRecoveryTerminal}
                onOpenTerminal={() => {
                  void handleOpenTerminal();
                }}
                terminalTitle={projectTerminalLabel(hasLocalCheckout)}
                onSelectedCommitHashChange={handleSelectedCommitHashChange}
                githubIssueListState={githubIssueListState}
                onGithubIssueListStateChange={setGithubIssueListState}
                onSelectedIssueIdChange={handleSelectedIssueIdChange}
                onSelectedPullRequestIdChange={
                  handleSelectedPullRequestIdChange
                }
                onSelectedTabChange={setActiveTab}
                profiles={profiles}
                project={repository}
                repositoryControls={
                  <ProjectDetailChromeActions
                    identityPubkey={identityQuery.data?.pubkey}
                    onRepositoryChange={handleRepositoryChange}
                    project={project}
                    projects={projectsQuery.data ?? []}
                    repository={repository}
                  />
                }
                projectId={project.id}
                repoDiff={displayedRepoDiff}
                repoDiffError={displayedRepoDiffError}
                repoDiffLoading={displayedRepoDiffLoading}
                pullRequests={pullRequests}
                pullRequestsError={pullRequestsQuery.error}
                pullRequestsLoading={pullRequestsQuery.isLoading}
                repoContributors={repoContributors}
                repoHost={repoRemote.host}
                repoSource={repoSource}
                selectedCommitHash={selectedCommitHash}
                selectedIssueId={selectedIssueId}
                selectedPullRequestId={selectedPullRequestId}
                snapshot={repoSnapshotQuery.data}
                snapshotError={
                  repoSnapshotQuery.error ??
                  (githubHosted && repoStateQuery.isError
                    ? repoStateQuery.error
                    : undefined)
                }
                snapshotLoading={
                  repoSnapshotQuery.isLoading ||
                  (githubHosted && repoStateQuery.isPending)
                }
                sourceControls={filesSourceControls}
                viewerGitIdentity={viewerGitIdentity}
              />
            </div>
          </div>
        </div>
        {profilePanelPubkey ? (
          <UserProfilePanel
            canResetWidth={threadPanelWidth.canReset}
            currentPubkey={identityQuery.data?.pubkey}
            onClose={handleCloseProfilePanel}
            onOpenDm={handleOpenDm}
            onOpenProfile={handleOpenProfilePanel}
            onResetWidth={threadPanelWidth.onResetWidth}
            onResizeStart={threadPanelWidth.onResizeStart}
            onTabChange={handleProfilePanelTabChange}
            onViewChange={handleProfilePanelViewChange}
            pubkey={profilePanelPubkey}
            tab={profilePanelTab}
            view={profilePanelView}
            widthPx={threadPanelWidth.widthPx}
          />
        ) : null}
      </div>
    </ProfilePanelProvider>
  );
}
