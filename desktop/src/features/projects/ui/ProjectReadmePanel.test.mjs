import assert from "node:assert/strict";
import { test } from "node:test";
import { findReadmeFile } from "./ProjectReadmePanel.tsx";

function file(path, previewContent = null) {
  return {
    path,
    kind: "file",
    size: 1,
    previewContent,
    lastChangedAt: null,
    latestCommit: null,
  };
}

test("unique preview without a README name is used", () => {
  const found = findReadmeFile([
    file("docs/overview.md", "# Hello"),
    file("src/main.rs"),
  ]);
  assert.equal(found?.path, "docs/overview.md");
});

test("multiple previews without a README name stay empty", () => {
  assert.equal(
    findReadmeFile([file(".gitignore", "target"), file("LICENSE", "MIT")]),
    null,
  );
});

test("README name still wins over a unique preview", () => {
  assert.equal(
    findReadmeFile([file("README.md"), file("docs/overview.md", "# Hello")])
      ?.path,
    "README.md",
  );
});
