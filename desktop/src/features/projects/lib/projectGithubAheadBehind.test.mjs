import assert from "node:assert/strict";
import { test } from "node:test";
import { githubAheadBehindCounts } from "./projectGithubAheadBehind.ts";

test("compared counts are visible and unpushed hides them", () => {
  assert.deepEqual(
    githubAheadBehindCounts({ status: "compared", ahead: 0, behind: 0 }),
    { ahead: 0, behind: 0 },
  );
  assert.equal(
    githubAheadBehindCounts({ status: "unpushed" }),
    null,
  );
  assert.equal(githubAheadBehindCounts(undefined), null);
});
