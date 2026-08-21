/**
 * Share links for the Projects read models.
 *
 * Every builder returns `null` instead of throwing when the entity cannot be
 * addressed by a `buzz://` link — addressable d-tags accept a wider charset
 * (and 1024 bytes) than the link format's `[a-zA-Z0-9._-]{1,64}`, and issues
 * and pull requests loaded outside a repository have no coordinate at all.
 * Callers hide the share affordance on `null` rather than copying a link that
 * would not parse on the receiving side.
 */

import {
  KIND_PROJECT_ANNOUNCEMENT,
  KIND_REPO_ANNOUNCEMENT,
} from "@/shared/constants/kinds";
import {
  buildIssueLink,
  buildProjectLink,
  buildPullRequestLink,
  buildRepoLink,
  type EntityLinkTab,
  isLinkableCoordinate,
} from "@/shared/lib/entityLink";

import type { ProjectIssue } from "../projectIssues.mjs";
import type { Project, Repository } from "../projectModels";
import type { ProjectPullRequest } from "../projectPullRequests.mjs";
import { githubRepoFullNameFromCloneUrl } from "./projectGithubPulls";

type Coordinate = { kind: number; owner: string; dtag: string };

const HEX64_RE = /^[a-fA-F0-9]{64}$/;

/**
 * Split an addressable coordinate (`<kind>:<owner>:<d>`). Only the first two
 * separators are structural — d-tags may themselves contain colons, so the
 * remainder is taken verbatim.
 */
export function parseAddressableCoordinate(
  address: string | null | undefined,
): Coordinate | null {
  if (!address) return null;

  const kindEnd = address.indexOf(":");
  if (kindEnd < 1) return null;
  const ownerEnd = address.indexOf(":", kindEnd + 1);
  if (ownerEnd < 0) return null;

  const kind = Number(address.slice(0, kindEnd));
  const owner = address.slice(kindEnd + 1, ownerEnd);
  const dtag = address.slice(ownerEnd + 1);
  if (!Number.isInteger(kind) || !HEX64_RE.test(owner) || dtag.length === 0) {
    return null;
  }

  return { kind, owner: owner.toLowerCase(), dtag };
}

function repositoryCoordinate(
  repoAddress: string | null | undefined,
): Coordinate | null {
  const coordinate = parseAddressableCoordinate(repoAddress);
  return coordinate?.kind === KIND_REPO_ANNOUNCEMENT ? coordinate : null;
}

/**
 * Map a workspace tab id (`WorkspaceTabs` vocabulary) onto the link format's
 * tab value. The overview tab is the link's default and PR-detail sub-tabs
 * have their own `buzz://pr` links, so both map to `undefined` (no tab).
 */
export function shareTabForWorkspaceTab(
  workspaceTab: string,
): EntityLinkTab | undefined {
  switch (workspaceTab) {
    case "files":
    case "issues":
    case "prs":
    case "contributors":
    case "channels":
      return workspaceTab;
    case "activity":
      return "commits";
    default:
      return undefined;
  }
}

/** Inverse of `shareTabForWorkspaceTab`, for the receiving side. */
export function workspaceTabForShareTab(tab: EntityLinkTab): string {
  return tab === "commits" ? "activity" : tab;
}

/**
 * Link to a project. Legacy (implicit) projects are backed by a repository
 * announcement rather than a kind:30621 event, so they share as `buzz://repo`
 * — which resolves to the same project route on the receiving side.
 */
export function projectShareLink(
  project: Project,
  tab?: EntityLinkTab,
): string | null {
  const coordinate = parseAddressableCoordinate(project.projectAddress);
  if (!coordinate || !isLinkableCoordinate(coordinate.owner, coordinate.dtag)) {
    return null;
  }

  if (coordinate.kind === KIND_PROJECT_ANNOUNCEMENT) {
    return buildProjectLink({ ...coordinate, tab });
  }
  return coordinate.kind === KIND_REPO_ANNOUNCEMENT
    ? buildRepoLink({ ...coordinate, tab })
    : null;
}

export function repositoryShareLink(repository: Repository): string | null {
  const coordinate = repositoryCoordinate(repository.repoAddress);
  return coordinate && isLinkableCoordinate(coordinate.owner, coordinate.dtag)
    ? buildRepoLink(coordinate)
    : null;
}

function isSafeGitHubIssueUrl(raw: string): boolean {
  try {
    if (
      raw !== raw.trim() ||
      !raw.startsWith("https://github.com/") ||
      raw.endsWith("/") ||
      raw.includes("\\")
    ) {
      return false;
    }
    const url = new URL(raw);
    if (
      url.protocol !== "https:" ||
      url.hostname !== "github.com" ||
      url.port !== "" ||
      url.username !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== "" ||
      url.pathname.includes("%") ||
      url.pathname.includes("//")
    ) {
      return false;
    }
    const [owner, repo, segment, number, ...rest] = url.pathname
      .split("/")
      .filter(Boolean);
    return (
      rest.length === 0 &&
      segment === "issues" &&
      /^[A-Za-z0-9-]+$/.test(owner ?? "") &&
      /^[A-Za-z0-9._-]+$/.test(repo ?? "") &&
      /^[1-9][0-9]*$/.test(number ?? "") &&
      raw === `https://github.com/${owner}/${repo}/issues/${number}`
    );
  } catch {
    return false;
  }
}

export function issueShareLink(issue: ProjectIssue): string | null {
  if (issue.htmlUrl && isSafeGitHubIssueUrl(issue.htmlUrl)) {
    return issue.htmlUrl;
  }
  const coordinate = repositoryCoordinate(issue.repoAddress);
  return coordinate &&
    HEX64_RE.test(issue.id) &&
    isLinkableCoordinate(coordinate.owner, coordinate.dtag)
    ? buildIssueLink({ ...coordinate, id: issue.id })
    : null;
}

function isSafeGitHubPullUrl(
  raw: string,
  pullRequest: Pick<ProjectPullRequest, "cloneUrls" | "id">,
): boolean {
  try {
    if (
      raw !== raw.trim() ||
      !raw.startsWith("https://github.com/") ||
      raw.endsWith("/") ||
      raw.includes("\\")
    ) {
      return false;
    }
    const url = new URL(raw);
    if (
      url.protocol !== "https:" ||
      url.hostname !== "github.com" ||
      url.port !== "" ||
      url.username !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== "" ||
      url.pathname.includes("%") ||
      url.pathname.includes("//")
    ) {
      return false;
    }
    const [owner, repo, segment, number, ...rest] = url.pathname
      .split("/")
      .filter(Boolean);
    const targetRepo = githubRepoFullNameFromCloneUrl(
      pullRequest.cloneUrls[0] ?? "",
    );
    return (
      rest.length === 0 &&
      segment === "pull" &&
      /^[A-Za-z0-9-]+$/.test(owner ?? "") &&
      /^[A-Za-z0-9._-]+$/.test(repo ?? "") &&
      /^[1-9][0-9]*$/.test(number ?? "") &&
      number === pullRequest.id &&
      targetRepo?.toLowerCase() === `${owner}/${repo}`.toLowerCase() &&
      raw === `https://github.com/${owner}/${repo}/pull/${number}`
    );
  } catch {
    return false;
  }
}

export function pullRequestShareLink(
  pullRequest: ProjectPullRequest,
): string | null {
  if (
    pullRequest.htmlUrl &&
    isSafeGitHubPullUrl(pullRequest.htmlUrl, pullRequest)
  ) {
    return pullRequest.htmlUrl;
  }
  const coordinate = repositoryCoordinate(pullRequest.repoAddress);
  return coordinate &&
    HEX64_RE.test(pullRequest.id) &&
    isLinkableCoordinate(coordinate.owner, coordinate.dtag)
    ? buildPullRequestLink({ ...coordinate, id: pullRequest.id })
    : null;
}
