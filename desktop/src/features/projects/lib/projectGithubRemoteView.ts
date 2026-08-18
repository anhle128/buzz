import { isGitHubCloneUrl } from "@/features/projects/lib/projectGitError";

/** Return the external-host splash label, but never for GitHub remotes. */
export function githubSplashHost(input: {
  repoSource: "remote" | "local";
  hostKind?: string;
  host?: string;
  cloneUrl?: string | null;
}) {
  if (input.repoSource !== "remote") return undefined;
  if (isGitHubCloneUrl(input.cloneUrl)) return undefined;
  if (input.hostKind === "external" && input.host) return input.host;
  return undefined;
}
