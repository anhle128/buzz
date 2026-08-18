import type { GithubAheadBehind } from "@/shared/api/projectGit";

export type { GithubAheadBehind };

/** Return visible counts only for completed GitHub comparisons. */
export function githubAheadBehindCounts(
  result: GithubAheadBehind | null | undefined,
) {
  if (!result || result.status !== "compared") return null;
  if (typeof result.ahead !== "number" || typeof result.behind !== "number") {
    return null;
  }
  return { ahead: result.ahead, behind: result.behind };
}
