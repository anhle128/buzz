import assert from "node:assert/strict";
import { afterEach, test } from "node:test";

import {
  addGithubIssueAssignees,
  addGithubIssueLabels,
  createGithubIssueComment,
  removeGithubIssueAssignee,
  removeGithubIssueLabel,
  updateGithubIssueState,
} from "./projectGithubIssueWrites.ts";

const TARGET = {
  cloneUrl: "https://github.com/acme/app",
  number: 42,
};

function installInvokeRecorder() {
  const calls = [];
  globalThis.window = {
    __TAURI_INTERNALS__: {
      invoke: async (command, input) => {
        calls.push({ command, input });
        return {};
      },
    },
  };
  return calls;
}

afterEach(() => {
  delete globalThis.window;
});

test("blank GitHub issue write values fail before invoking Tauri", async () => {
  const calls = installInvokeRecorder();
  const writes = [
    () => createGithubIssueComment({ ...TARGET, body: "   " }),
    () => addGithubIssueLabels({ ...TARGET, name: "   " }),
    () => removeGithubIssueLabel({ ...TARGET, name: "   " }),
    () => addGithubIssueAssignees({ ...TARGET, login: "   " }),
    () => removeGithubIssueAssignee({ ...TARGET, login: "   " }),
  ];

  for (const write of writes) {
    await assert.rejects(write(), /is required/);
  }
  assert.deepEqual(calls, []);
});

test("GitHub issue write wrappers trim values and reject unsafe numbers", async () => {
  const calls = installInvokeRecorder();

  await createGithubIssueComment({ ...TARGET, body: "  Looks good  " });
  await addGithubIssueLabels({ ...TARGET, name: "  bug  " });
  await addGithubIssueAssignees({ ...TARGET, login: "  ada  " });
  await assert.rejects(
    updateGithubIssueState({
      ...TARGET,
      number: Number.MAX_SAFE_INTEGER + 1,
      state: "closed",
    }),
    /positive safe integer/,
  );

  assert.deepEqual(calls, [
    {
      command: "create_github_issue_comment",
      input: { ...TARGET, body: "Looks good" },
    },
    {
      command: "add_github_issue_labels",
      input: { ...TARGET, name: "bug" },
    },
    {
      command: "add_github_issue_assignees",
      input: { ...TARGET, login: "ada" },
    },
  ]);
});
