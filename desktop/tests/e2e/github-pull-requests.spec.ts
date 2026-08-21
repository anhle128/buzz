import { expect, test } from "@playwright/test";

import { installMockBridge } from "../helpers/bridge";

async function enableProjectsFeature(page: import("@playwright/test").Page) {
  await page.addInitScript(() => {
    window.localStorage.setItem(
      "buzz-feature-overrides-v1",
      JSON.stringify({ projects: true }),
    );
  });
}

async function openBuzzProject(page: import("@playwright/test").Page) {
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await page.getByTestId("open-projects-view").click();
  await page.getByTestId("projects-section-projects").click();
  const projectEntry = page
    .locator(
      '[data-testid="project-card-buzz"], [data-testid="project-row-buzz"]',
    )
    .first();
  await expect(projectEntry).toBeVisible({ timeout: 10_000 });
  await projectEntry.click();
}

async function openGithubPullRequests(page: import("@playwright/test").Page) {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.evaluate(() => {
    window.__BUZZ_E2E_COMMANDS__ = [];
    window.__BUZZ_E2E_PROJECT_QUERY_FILTERS__ = [];
    window.__BUZZ_E2E_SIGNED_EVENTS__ = [];
  });
  await page.getByRole("tab", { name: "Pull Request", exact: true }).click();
}

test("GitHub pull requests list metadata, load read-only detail, and create #N", async ({
  page,
}) => {
  await openGithubPullRequests(page);
  const row = page.getByTestId("project-github-pull-request-row").first();
  await expect(row).toContainText("#42");
  await expect(row).toContainText("Open");
  await expect(row).toContainText("ada");
  await expect(row).toContainText("feature → develop");

  await row.getByRole("button", { name: "#42", exact: true }).click();
  await expect(
    page.getByText("PR body from GitHub", { exact: true }),
  ).toBeVisible();
  const comments = page.getByTestId("project-issue-comment-timeline-row");
  await expect(comments).toHaveCount(2);
  await expect(comments.nth(0)).toContainText("API-order first comment.");
  await expect(comments.nth(1)).toContainText("API-order second comment.");
  await expect(page.getByText("grace", { exact: true })).toBeVisible();
  await expect(
    page.getByTestId("project-pull-request-comment-composer"),
  ).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Merge" })).toHaveCount(0);
  await expect(page.getByRole("tab", { name: /Files changed/ })).toHaveCount(0);
  await expect(page.getByTestId("pull-request-discussed-in")).toHaveCount(0);

  await page.getByRole("tab", { name: /Commits/ }).click();
  await expect(
    page.getByTestId("project-github-pull-request-commit-row"),
  ).toBeVisible();
  await expect(
    page.getByTestId("project-github-pull-request-commit-row"),
  ).toContainText("ada");

  await page.getByRole("button", { name: "Pull Request", exact: true }).click();
  await page.getByRole("button", { name: "New pull request" }).click();
  await page
    .getByTestId("create-pull-request-compare-branch")
    .selectOption("main");
  await page.getByTestId("create-pull-request-title").fill("New GitHub change");
  await page.getByTestId("create-pull-request-body").fill("Created from Buzz");
  await page.getByTestId("create-pull-request-submit").click();
  await expect(
    page.getByText("New GitHub change", { exact: true }),
  ).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByText("#43", { exact: true })).toBeVisible();

  const commands = await page.evaluate(
    () => window.__BUZZ_E2E_COMMANDS__ ?? [],
  );
  expect(commands).toContain("list_github_pull_requests");
  expect(commands).toContain("list_github_pull_request_comments");
  expect(commands).toContain("create_github_pull_request");
  expect(commands).not.toContain("sign_project_pull_request_status");
  expect(commands).not.toContain("sign_project_pull_request_review_request");
  expect(commands).not.toContain("merge_project_pull_request");
  expect(commands).not.toContain("get_project_repo_diff");
  expect(commands).not.toContain("get_project_local_repo_diff");
  const signedEvents = await page.evaluate(
    () => window.__BUZZ_E2E_SIGNED_EVENTS__ ?? [],
  );
  expect(
    signedEvents.filter(
      (event) =>
        !(
          event.kind === 30078 &&
          event.tags?.some(
            (tag) => tag[0] === "d" && tag[1] === "community-theme",
          )
        ),
    ),
  ).toHaveLength(0);
  const detailFilters = await page.evaluate(
    () => window.__BUZZ_E2E_PROJECT_QUERY_FILTERS__ ?? [],
  );
  expect(detailFilters.some((filter) => filter.kinds?.includes(1618))).toBe(
    false,
  );
});

test("GitHub pull-request auth failure renders recovery before empty state", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
    window.__BUZZ_E2E_GITHUB_PULLS_ERROR__ = {
      code: "github_auth_required",
      message:
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Pull Request", exact: true }).click();
  await expect(page.getByText("GitHub authentication required")).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByText("No pull requests yet.")).toHaveCount(0);
  await expect(page.getByText("No open pull requests.")).toHaveCount(0);
  await expect(page.getByTestId("project-github-pull-request-row")).toHaveCount(
    0,
  );
});

test("GitHub comment failure keeps the pull-request body and retries only comments", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
    window.__BUZZ_E2E_GITHUB_PULL_COMMENTS_ERROR__ = {
      code: "github_pulls_failed",
      message: "Comment request failed.",
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Pull Request", exact: true }).click();
  const row = page.getByTestId("project-github-pull-request-row").first();
  await row.getByRole("button", { name: "#42", exact: true }).click();
  await expect(
    page.getByText("PR body from GitHub", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Could not load GitHub comments", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Could not load GitHub pull requests", { exact: true }),
  ).toHaveCount(0);
  const before = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  const listCallsBefore = before.filter(
    (command) => command === "list_github_pull_requests",
  ).length;
  const commentCallsBefore = before.filter(
    (command) => command === "list_github_pull_request_comments",
  ).length;
  await page
    .locator('[aria-labelledby="github-pull-request-comments-recovery-title"]')
    .getByRole("button", { name: "Retry", exact: true })
    .click();
  await expect(
    page.getByText("Could not load GitHub comments", { exact: true }),
  ).toBeVisible();
  const after = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(
    after.filter((command) => command === "list_github_pull_requests").length,
  ).toBe(listCallsBefore);
  expect(
    after.filter((command) => command === "list_github_pull_request_comments")
      .length,
  ).toBe(commentCallsBefore + 1);
});

test("GitHub comment 404 clears the stale selection and keeps the list", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
    window.__BUZZ_E2E_GITHUB_PULL_COMMENTS_ERROR__ = {
      code: "github_pr_unavailable",
      message: "Pull request is unavailable.",
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Pull Request", exact: true }).click();
  await page
    .getByTestId("project-github-pull-request-row")
    .first()
    .getByRole("button", { name: "#42", exact: true })
    .click();
  await expect(
    page.getByText("Pull request not found.", { exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId("project-github-pull-request-row")).toHaveCount(
    1,
  );
  await expect(
    page.getByText("PR body from GitHub", { exact: true }),
  ).toHaveCount(0);
});
