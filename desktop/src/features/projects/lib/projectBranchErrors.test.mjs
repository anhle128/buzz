import assert from "node:assert/strict";
import { test } from "node:test";

import { ProjectPullRequestMergeError } from "../../../shared/api/projectGit.ts";
import {
  githubBranchActionReason,
  isNoChannelBindingError,
  projectBranchErrorMessage,
} from "./projectBranchErrors.ts";

test("recognizes the relay's stable denial token", () => {
  // Body produced by the relay push policy (buzz-core
  // GIT_NO_CHANNEL_BINDING_BODY), as it arrives wrapped in git stderr.
  assert.ok(
    isNoChannelBindingError(
      "remote: no_channel_binding: repository has no channel binding\nerror: failed to push some refs",
    ),
  );
});

test("recognizes the legacy spaced phrase from older relays", () => {
  assert.ok(isNoChannelBindingError("push denied: no channel binding"));
});

test("does not match unrelated errors", () => {
  assert.ok(!isNoChannelBindingError("connection reset by peer"));
  assert.ok(!isNoChannelBindingError("no channel"));
});

test("maps binding denials to remediation copy", () => {
  const message = projectBranchErrorMessage(
    new Error("remote: no_channel_binding: repository has no channel binding"),
    "Failed to create branch.",
  );
  assert.ok(message.includes("buzz repos bind"));
});

test("passes through other errors and falls back for non-errors", () => {
  assert.equal(
    projectBranchErrorMessage(new Error("boom"), "fallback"),
    "boom",
  );
  assert.equal(projectBranchErrorMessage("boom", "fallback"), "fallback");
  assert.equal(projectBranchErrorMessage(null, "fallback"), "fallback");
});

test("maps structured GitHub errors in branch dialogs", () => {
  assert.equal(
    projectBranchErrorMessage(
      new ProjectPullRequestMergeError(
        "github_auth_required",
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        null,
      ),
      "Failed to create branch.",
    ),
    "Authenticate GitHub CLI with: gh auth login --hostname github.com",
  );
});

test("derives a GitHub branch-action recovery reason and gates it by host", () => {
  const error = new ProjectPullRequestMergeError(
    "github_cli_missing",
    "Install the GitHub CLI to continue.",
    null,
  );
  assert.equal(
    githubBranchActionReason({ githubHosted: true, error }),
    "Install GitHub CLI, then retry.",
  );
  assert.equal(githubBranchActionReason({ githubHosted: false, error }), null);
});

test("disables GitHub branch actions for an unstructured G1 failure", () => {
  assert.equal(
    githubBranchActionReason({
      githubHosted: true,
      error: new Error("GitHub state bridge failed."),
    }),
    "GitHub state bridge failed.",
  );
});
