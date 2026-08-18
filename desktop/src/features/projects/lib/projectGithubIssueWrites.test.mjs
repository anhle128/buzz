import assert from "node:assert/strict";
import { test } from "node:test";

import {
  nextGithubIssueListState,
  selectedGithubIssueAfterListLoad,
} from "./projectGithubIssueWrites.ts";

test("settled selection waits for the destination list fetch", () => {
  assert.equal(
    selectedGithubIssueAfterListLoad({
      selectedIssueId: "42",
      issueIds: [],
      isSuccess: true,
      isFetching: true,
    }),
    "42",
  );
  assert.equal(
    selectedGithubIssueAfterListLoad({
      selectedIssueId: "42",
      issueIds: [],
      isSuccess: true,
      isFetching: false,
    }),
    null,
  );
  assert.equal(
    selectedGithubIssueAfterListLoad({
      selectedIssueId: "42",
      issueIds: ["42"],
      isSuccess: true,
      isFetching: false,
    }),
    "42",
  );
  assert.equal(
    selectedGithubIssueAfterListLoad({
      selectedIssueId: "42",
      issueIds: [],
      isSuccess: false,
      isFetching: false,
    }),
    "42",
  );
  assert.equal(
    selectedGithubIssueAfterListLoad({
      selectedIssueId: null,
      issueIds: ["42"],
      isSuccess: true,
      isFetching: false,
    }),
    null,
  );
});

test("write actions pick the destination GitHub list state", () => {
  assert.equal(nextGithubIssueListState("close"), "closed");
  assert.equal(nextGithubIssueListState("reopen"), "open");
  assert.equal(nextGithubIssueListState("create"), "open");
});
