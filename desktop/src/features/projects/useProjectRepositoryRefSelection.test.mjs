import assert from "node:assert/strict";
import { after, before, test } from "node:test";

import { JSDOM } from "jsdom";

const dom = new JSDOM("<!doctype html><html><body></body></html>", {
  url: "http://localhost",
});

before(() => {
  Object.assign(globalThis, {
    document: dom.window.document,
    HTMLElement: dom.window.HTMLElement,
    IS_REACT_ACT_ENVIRONMENT: true,
    window: dom.window,
  });
});

after(() => dom.window.close());

test("follows GitHub HEAD after pending announced main stays in the option list", async () => {
  const { renderHook } = await import("@testing-library/react");
  const { useProjectRepositoryRefSelection } = await import(
    "./useProjectRepositoryRefSelection.ts"
  );

  const { result, rerender, unmount } = renderHook(
    (input) => useProjectRepositoryRefSelection(input),
    {
      initialProps: {
        branchOptions: ["main"],
        defaultBranch: "main",
        projectAvailable: true,
        projectPending: false,
        tags: [],
      },
    },
  );

  assert.equal(result.current.activeBranch, "main");

  rerender({
    branchOptions: ["develop", "main"],
    defaultBranch: "develop",
    projectAvailable: true,
    projectPending: false,
    tags: [],
  });

  assert.equal(result.current.activeBranch, "develop");
  unmount();
});

test("keeps an explicit main pick after GitHub HEAD becomes develop", async () => {
  const { act, renderHook } = await import("@testing-library/react");
  const { useProjectRepositoryRefSelection } = await import(
    "./useProjectRepositoryRefSelection.ts"
  );

  const { result, rerender, unmount } = renderHook(
    (input) => useProjectRepositoryRefSelection(input),
    {
      initialProps: {
        branchOptions: ["develop", "main"],
        defaultBranch: "develop",
        projectAvailable: true,
        projectPending: false,
        tags: [],
      },
    },
  );

  act(() => {
    result.current.selectBranch("main");
  });
  assert.equal(result.current.activeBranch, "main");

  rerender({
    branchOptions: ["develop", "main"],
    defaultBranch: "develop",
    projectAvailable: true,
    projectPending: false,
    tags: [],
  });

  assert.equal(result.current.activeBranch, "main");
  unmount();
});
