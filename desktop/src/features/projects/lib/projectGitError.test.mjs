import assert from "node:assert/strict";
import { test } from "node:test";

import { ProjectPullRequestMergeError } from "../../../shared/api/projectGit.ts";
import { projectCloneErrorPresentation } from "./projectGitError.ts";

test("explains unsupported authenticated GitHub clones without exposing git output", () => {
  assert.deepEqual(
    projectCloneErrorPresentation(
      new Error(
        "Cloning into '/Users/person/repos/app'... remote: repository requires SSH certificate authentication. fatal: requested URL returned error: 403",
      ),
      "https://github.com/example/app.git",
    ),
    {
      title: "Repository access required",
      description:
        "This repository requires GitHub authentication. Check that your GitHub SSH key or credential has repository access.",
    },
  );
});

test("recognizes authentication errors from GitHub SCP clone URLs", () => {
  assert.equal(
    projectCloneErrorPresentation(
      new Error("Permission denied (publickey)"),
      "git@github.com:oceanlabs-holding/x10.oh.agentic-os-plan.git",
    ).title,
    "Repository access required",
  );
});

test("presents missing and network failures clearly", () => {
  assert.equal(
    projectCloneErrorPresentation(new Error("Repository not found")).title,
    "Repository not found",
  );
  assert.deepEqual(
    projectCloneErrorPresentation(
      new Error("Repository not found"),
      "https://relay.example/git/owner/repo",
      "access",
    ),
    {
      title: "Repository access restricted",
      description:
        "You need access to the repository’s channel before you can clone it.",
    },
  );
  assert.equal(
    projectCloneErrorPresentation(new Error("Could not resolve host")).title,
    "Couldn’t reach the repository",
  );
});

test("uses a concise fallback", () => {
  assert.deepEqual(projectCloneErrorPresentation(new Error("git failed")), {
    title: "Couldn’t clone repository",
    description:
      "Try again. If the problem continues, contact the repository owner.",
  });
});

test("presents structured GitHub CLI and auth failures for clone", () => {
  assert.deepEqual(
    projectCloneErrorPresentation(
      new ProjectPullRequestMergeError(
        "github_cli_missing",
        "Install the GitHub CLI to continue.",
        null,
      ),
      "https://github.com/acme/app",
    ),
    {
      title: "GitHub CLI is required",
      description: "Install GitHub CLI, then retry.",
    },
  );
  assert.deepEqual(
    projectCloneErrorPresentation(
      new ProjectPullRequestMergeError(
        "github_auth_required",
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        null,
      ),
      "https://github.com/acme/app",
    ),
    {
      title: "GitHub authentication required",
      description:
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
    },
  );
});

test("does not use GitHub recovery copy for a Buzz clone URL", () => {
  const owner = "ab".repeat(32);
  assert.deepEqual(
    projectCloneErrorPresentation(
      new ProjectPullRequestMergeError(
        "github_auth_required",
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        null,
      ),
      `https://relay.example/git/${owner}/app`,
    ),
    {
      title: "Repository access required",
      description:
        "Buzz could not authenticate with this repository. Check your access and try again.",
    },
  );
});
