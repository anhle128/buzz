import { getRelaySelf } from "@/features/moderation/lib/relaySelf";
import { getGithubRepositoryState } from "@/shared/api/projectGit";
import { relayClient } from "@/shared/api/relayClient";
import type { RelayEvent } from "@/shared/api/types";
import { KIND_REPO_STATE } from "@/shared/constants/kinds";
import type { Repository } from "@/features/projects/projectModels";
import { isGitHubCloneUrl } from "@/features/projects/lib/projectGitError";

export type RepoState = {
  branches: Array<{ name: string; commit: string }>;
  tags: Array<{ name: string; commit: string }>;
  head: string | null;
  updatedAt: number;
};

export type FetchRepoStateLoaders = {
  loadGithub: (cloneUrl: string) => Promise<RepoState>;
  loadBuzz: (project: Repository) => Promise<RepoState | null>;
};

function eventToRepoState(event: RelayEvent): RepoState {
  const branches: RepoState["branches"] = [];
  const tags: RepoState["tags"] = [];
  let head: string | null = null;

  for (const tag of event.tags) {
    const [name, value] = tag;
    if (!name || !value) continue;

    if (name.startsWith("refs/heads/")) {
      branches.push({ name: name.slice("refs/heads/".length), commit: value });
    } else if (name.startsWith("refs/tags/")) {
      tags.push({ name: name.slice("refs/tags/".length), commit: value });
    } else if (name === "HEAD") {
      head = value.replace(/^ref:\s*/, "").replace(/^refs\/heads\//, "");
    }
  }

  return {
    branches,
    tags,
    head,
    updatedAt: event.created_at,
  };
}

/** Load kind:30618 repository state for Buzz-hosted remotes. */
export async function fetchBuzzRepoState(
  project: Repository,
): Promise<RepoState | null> {
  const relaySelf = await getRelaySelf();
  const trustedAuthors = [
    ...new Set(
      [project.owner, relaySelf].filter((value): value is string =>
        Boolean(value),
      ),
    ),
  ];
  const events = await relayClient.fetchEvents({
    kinds: [KIND_REPO_STATE],
    authors: trustedAuthors,
    "#d": [project.dtag],
    limit: 1,
  });

  return events.length > 0 ? eventToRepoState(events[0]) : null;
}

/**
 * Route repository state loading by clone URL host.
 * GitHub remotes use `gh api`; everything else keeps kind:30618.
 */
export async function fetchRepoStateWith(
  project: Repository,
  loaders: FetchRepoStateLoaders,
): Promise<RepoState | null> {
  const cloneUrl = project.cloneUrls[0];
  if (isGitHubCloneUrl(cloneUrl)) {
    return loaders.loadGithub(cloneUrl);
  }
  return loaders.loadBuzz(project);
}

/** Load repository branch state for the Projects picker. */
export async function fetchRepoState(
  project: Repository,
): Promise<RepoState | null> {
  return fetchRepoStateWith(project, {
    loadGithub: getGithubRepositoryState,
    loadBuzz: fetchBuzzRepoState,
  });
}
