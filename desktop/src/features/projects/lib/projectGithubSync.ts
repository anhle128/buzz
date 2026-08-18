import { isGitHubCloneUrl } from "@/features/projects/lib/projectGitError";

/** Decide whether local-vs-remote sync status may run for this repository. */
export function projectRepoSyncStatusEnabled(input: {
  cloneUrl?: string | null;
  buzzHost: boolean;
  githubStateReady: boolean;
}): boolean {
  if (!input.cloneUrl) return false;
  if (input.buzzHost) return true;
  return isGitHubCloneUrl(input.cloneUrl) && input.githubStateReady;
}

/** Return displayable GitHub counts only when sync examined a checkout. */
export function githubSyncCountDisplay(input: {
  githubHosted: boolean;
  localPath?: string | null;
  aheadCount?: number | null;
  behindCount?: number | null;
}): { ahead: number; behind: number } | null {
  if (!input.githubHosted || !input.localPath) return null;
  if (
    typeof input.aheadCount !== "number" ||
    typeof input.behindCount !== "number"
  ) {
    return null;
  }
  return { ahead: input.aheadCount, behind: input.behindCount };
}

/** Keep Nostr PR-update publication on Buzz pushes only. */
export function shouldPublishPullRequestUpdateAfterPush(
  cloneUrl?: string | null,
): boolean {
  return !isGitHubCloneUrl(cloneUrl);
}

/** Select the single primary repository sync action for the header. */
export function repoSyncPrimaryAction(input: {
  githubHosted: boolean;
  remoteKind?: "buzz" | "external";
  hasExternalUrl?: boolean;
  canPull?: boolean;
  canPush?: boolean;
  hasFetch?: boolean;
}): "pull" | "push" | "fetch" | "open" | null {
  if (!input.githubHosted && input.remoteKind === "external") {
    return input.hasExternalUrl ? "open" : null;
  }
  if (input.canPull) return "pull";
  if (input.canPush) return "push";
  return input.hasFetch ? "fetch" : null;
}
