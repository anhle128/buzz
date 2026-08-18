import assert from "node:assert/strict";
import { test } from "node:test";
import { githubSplashHost } from "./projectGithubRemoteView.ts";

test("github.com never uses the hosted-elsewhere splash", () => {
  assert.equal(
    githubSplashHost({
      repoSource: "remote",
      hostKind: "external",
      host: "github.com",
      cloneUrl: "https://github.com/acme/app",
    }),
    undefined,
  );
  assert.equal(
    githubSplashHost({
      repoSource: "remote",
      hostKind: "external",
      host: "gitlab.com",
      cloneUrl: "https://gitlab.com/acme/app",
    }),
    "gitlab.com",
  );
});
