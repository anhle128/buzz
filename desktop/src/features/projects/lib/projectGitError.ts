import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import type { ProjectRepoUnavailableReason } from "./projectRepoAvailability";

export type ProjectGitErrorPresentation = {
  title: string;
  description: string;
};

function errorText(error: unknown) {
  if (error instanceof Error) return error.message.toLowerCase();
  return typeof error === "string" ? error.toLowerCase() : "";
}

/** True when the clone URL points at github.com (https or SSH). */
export function isGitHubCloneUrl(cloneUrl: string | null | undefined) {
  if (cloneUrl?.startsWith("git@github.com:")) return true;
  try {
    return new URL(cloneUrl ?? "").hostname.toLowerCase() === "github.com";
  } catch {
    return false;
  }
}

export function projectCloneErrorPresentation(
  error: unknown,
  cloneUrl?: string | null,
  unavailableReason?: ProjectRepoUnavailableReason,
): ProjectGitErrorPresentation {
  const message = errorText(error);
  const github = isGitHubCloneUrl(cloneUrl);

  const structured = github ? parseProjectPullRequestMergeError(error) : null;
  if (structured) {
    switch (structured.code) {
      case "github_cli_missing":
        return {
          title: "GitHub CLI is required",
          description: "Install GitHub CLI, then retry.",
        };
      case "github_auth_required":
        return {
          title: "GitHub authentication required",
          description: structured.message,
        };
      case "github_repo_unavailable":
      case "github_state_failed":
        return {
          title: "Could not load GitHub branches",
          description: structured.message,
        };
    }
  }

  if (unavailableReason === "access") {
    return {
      title: "Repository access restricted",
      description:
        "You need access to the repository’s channel before you can clone it.",
    };
  }
  if (
    /\b(?:401|403)\b|authenticat|authoriz|permission denied|access denied|ssh certificate/.test(
      message,
    )
  ) {
    return {
      title: "Repository access required",
      description: github
        ? "This repository requires GitHub authentication. Check that your GitHub SSH key or credential has repository access."
        : "Buzz could not authenticate with this repository. Check your access and try again.",
    };
  }
  if (/\b404\b|repository not found|repository does not exist/.test(message)) {
    return {
      title: "Repository not found",
      description:
        "Check that the repository link is correct and that the repository still exists.",
    };
  }
  if (
    /timed? out|could not resolve host|failed to connect|connection (?:refused|reset)|network is unreachable|offline/.test(
      message,
    )
  ) {
    return {
      title: "Couldn’t reach the repository",
      description: "Check your connection and try cloning again.",
    };
  }
  if (
    /already exists and is not an empty directory|destination path .* exists/.test(
      message,
    )
  ) {
    return {
      title: "Local folder already exists",
      description:
        "Choose a different repositories directory or remove the existing checkout.",
    };
  }
  return {
    title: "Couldn’t clone repository",
    description: github
      ? "Try again, or open the repository on GitHub for more information."
      : "Try again. If the problem continues, contact the repository owner.",
  };
}
