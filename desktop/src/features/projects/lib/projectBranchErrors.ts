import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";

/**
 * Relay push-policy denial token for a repository with no `buzz-channel`
 * binding. Declared in Rust as `buzz-core::git_perms::
 * GIT_NO_CHANNEL_BINDING_TOKEN`; the relay's denial body starts with it
 * ("no_channel_binding: repository has no channel binding"). The legacy
 * spaced phrase is kept as a second matcher so this build also recognizes
 * denials from relays deployed before the token existed.
 */
const NO_CHANNEL_BINDING_TOKEN = "no_channel_binding";
const NO_CHANNEL_BINDING_LEGACY_PHRASE = "no channel binding";

const NO_CHANNEL_BINDING_COPY =
  "This repository is not linked to a project channel, so the relay cannot " +
  "authorize access. The repository owner can link it with: " +
  "buzz repos bind --id <repo> --channel <channel-uuid>";

/** True when a git/relay error text is the unbound-repository denial. */
export function isNoChannelBindingError(message: string): boolean {
  return (
    message.includes(NO_CHANNEL_BINDING_TOKEN) ||
    message.includes(NO_CHANNEL_BINDING_LEGACY_PHRASE)
  );
}

/** Recovery reason for GitHub-hosted branch create/delete when G1 failed. */
export function githubBranchActionReason(input: {
  githubHosted: boolean;
  error?: unknown;
}): string | null {
  if (!input.githubHosted || input.error == null) return null;
  const parsed = parseProjectPullRequestMergeError(input.error);
  if (parsed) {
    return parsed.code === "github_cli_missing"
      ? "Install GitHub CLI, then retry."
      : parsed.message;
  }
  return input.error instanceof Error && input.error.message.trim()
    ? input.error.message
    : "Could not load GitHub branches.";
}

/** Map a thrown branch-operation error to user-facing dialog copy. */
export function projectBranchErrorMessage(
  error: unknown,
  fallback: string,
): string {
  const structured = parseProjectPullRequestMergeError(error);
  if (structured) return structured.message;
  if (!(error instanceof Error)) return fallback;
  if (isNoChannelBindingError(error.message)) {
    return NO_CHANNEL_BINDING_COPY;
  }
  return error.message;
}
