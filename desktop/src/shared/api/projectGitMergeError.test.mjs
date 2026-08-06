import assert from "node:assert/strict";
import test from "node:test";

import {
  parseProjectPullRequestMergeError,
  ProjectPullRequestMergeError,
} from "./projectGit.ts";

const CONFLICT = {
  code: "merge_conflict",
  message: "Pull request has merge conflicts.",
  recovery: {
    action: "open_terminal",
    targetBranch: "main",
    sourceBranch: "feature/demo",
  },
};

const validPullRecovery = {
  code: "github_pr_blocked",
  message: "GitHub does not allow an immediate merge yet.",
  recovery: {
    action: "open_url",
    url: "https://github.com/anhle128/buzz/pull/42",
    reasons: ["Required check ci is pending."],
  },
};

const validListRecovery = {
  code: "github_pr_ambiguous",
  message: "More than one GitHub pull request matched.",
  recovery: {
    action: "open_url",
    url: "https://github.com/anhle128/buzz/pulls",
    reasons: [],
  },
};

test("parses structured merge conflict recovery metadata", () => {
  const error = parseProjectPullRequestMergeError(CONFLICT);

  assert.ok(error instanceof ProjectPullRequestMergeError);
  assert.equal(error.code, "merge_conflict");
  assert.deepEqual(error.recovery, CONFLICT.recovery);
});

test("parses JSON-serialized Tauri merge errors", () => {
  const error = parseProjectPullRequestMergeError(JSON.stringify(CONFLICT));

  assert.ok(error instanceof ProjectPullRequestMergeError);
  assert.equal(error.message, "Pull request has merge conflicts.");
});

test("parses safe GitHub pull-request recovery URLs", () => {
  for (const value of [validPullRecovery, validListRecovery]) {
    const error = parseProjectPullRequestMergeError(value);

    assert.ok(error instanceof ProjectPullRequestMergeError);
    assert.deepEqual(error.recovery, value.recovery);
  }
});

test("rejects unsafe GitHub pull-request recovery URLs and reasons", () => {
  const invalidUrls = [
    "http://github.com/anhle128/buzz/pull/42",
    "https://api.github.com/anhle128/buzz/pull/42",
    "https://user@github.com/anhle128/buzz/pull/42",
    "https://github.com:443/anhle128/buzz/pull/42",
    "https://github.com/anhle128/buzz/pull/42?tab=checks",
    "https://github.com/anhle128/buzz/pull/42#checks",
    "https://github.com/anhle128/buzz/pull/42/",
    "https://github.com/anhle128/buzz/pull/42/files",
    "https://github.com//anhle128/buzz/pull/42",
    "https://github.com/anhle128/buzz//pull/42",
    "https://github.com/anhle128\\buzz/pull/42",
    "https://github.com/anhle128/buzz/pull/zero",
    "https://github.com/anhle128/buzz/pull/0",
    "https://github.com/anhle128/buzz%2Fother/pull/42",
    "https://github.com/anh_le/buzz/pull/42",
    "https://github.com/anhle128/buzz%2Fother/pulls",
  ];

  for (const url of invalidUrls) {
    assert.equal(
      parseProjectPullRequestMergeError({
        ...validPullRecovery,
        recovery: { ...validPullRecovery.recovery, url },
      }),
      null,
      url,
    );
  }

  assert.equal(
    parseProjectPullRequestMergeError({
      ...validPullRecovery,
      recovery: {
        ...validPullRecovery.recovery,
        reasons: Array.from({ length: 21 }, (_, index) => String(index)),
      },
    }),
    null,
  );
  assert.equal(
    parseProjectPullRequestMergeError({
      ...validPullRecovery,
      recovery: {
        ...validPullRecovery.recovery,
        reasons: ["x".repeat(201)],
      },
    }),
    null,
  );
});

test("rejects malformed recovery metadata", () => {
  assert.equal(
    parseProjectPullRequestMergeError({
      ...CONFLICT,
      recovery: { ...CONFLICT.recovery, targetBranch: null },
    }),
    null,
  );
  assert.equal(parseProjectPullRequestMergeError(new Error("offline")), null);
});
