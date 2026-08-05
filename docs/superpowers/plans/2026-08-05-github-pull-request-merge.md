# GitHub-Backed Pull Request Merge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task, or execute the tasks inline in this session if Kevin chooses Inline Execution.

**Goal:** Make the existing Buzz Desktop Merge action find or create and immediately merge the exact GitHub pull request the user reviewed, while preserving Buzz-hosted merge behavior and publishing Nostr merged status only after GitHub confirms the merge.

**Architecture:** Keep `merge_project_pull_request` as the single Tauri router.
Route strict GitHub URL pairs into one focused Rust module that invokes the installed `gh` executable directly, and keep the existing native Git/Buzz path unchanged when neither URL is GitHub.
Share only the final Nostr status construction and publication tail.
Expose failures through the current structured error envelope with one additional `open_url` recovery variant, then render actionable GitHub states in the existing merge button component.

**Tech Stack:** Rust, Tauri 2, `std::process::Command`, `serde`, `serde_json`, `tempfile`, `url`, GitHub CLI, GitHub REST through `gh api`, React 19, TypeScript, TanStack Query, Node test, and Playwright.

## Working Agreement

- Work in `/Users/dale/.buzz/REPOS/anhle128--buzz-github-pr-merge` on branch `fix/github-backed-pr-merge`.
- Start each execution shell with `. ./bin/activate-hermit` before running Git, hooks, builds, or tests.
- Treat approved design commit `433f14c8` and `docs/superpowers/specs/2026-08-05-github-pull-request-merge-design.md` as the product contract.
- Start with the end-user regression in Task 1 before changing production code.
- Reuse the strict GitHub URL validation already rooted in `project_git_exec.rs`; do not add a second independent validator.
- Reuse `managed_agents::resolve_command`, `crate::util::configure_no_window`, the existing branch and commit normalizers, `useProjectRepoHost`, the opener plugin, and the existing Nostr partial-publication retry.
- Do not add a provider interface, factory, schema, dependency, OAuth flow, feature flag, auto-merge path, merge-queue enrollment, administrator bypass, or direct-push fallback.
- Keep direct child-process execution shell-free, close stdin, bound output, redact diagnostics, and write mutation JSON through permission-restricted temporary files.
- Run the full package test suite after each task that changes a package, not only the focused test.
- Before every commit, run `git diff --check` and inspect `git diff --stat` plus the complete staged diff.
- Use the repository identity `Kevin Le <anhle12892@gmail.com>` for both required trailers, with `Co-authored-by` before `Signed-off-by`.

## Stable Contracts

Use these exact public-to-crate contracts so later tasks do not invent parallel types:

```rust
pub(crate) struct GitHubRepoRef {
    owner: String,
    repo: String,
}

pub(crate) struct GitHubMergeInput {
    pub target: GitHubRepoRef,
    pub source: GitHubRepoRef,
    pub target_branch: String,
    pub source_branch: String,
    pub expected_commit: String,
    pub title: String,
    pub body: String,
}

pub(crate) struct GitHubMergeOutcome {
    pub message: String,
    pub merge_commit: String,
}

pub(crate) fn merge_github_pull_request(
    input: GitHubMergeInput,
) -> Result<GitHubMergeOutcome, ProjectPullRequestMergeError>;
```

Use these exact resource limits:

- 60 seconds per `gh` invocation.
- 64 KiB retained from stdout and 64 KiB retained from stderr.
- 8 KiB maximum user-facing diagnostic after redaction.
- 20 maximum recovery reasons.
- 200 Unicode scalar values maximum per recovery reason.

Use these exact `gh` operations:

```text
gh auth status --hostname github.com
gh api --method GET --hostname github.com --paginate --slurp repos/<target>/pulls -f state=all -f head=<source-owner>:<head> -f base=<base> -f per_page=100
gh api --method POST --hostname github.com repos/<target>/pulls --input <temp-json>
gh pr view <number> --repo github.com/<target> --json number,url,state,headRefOid,isDraft,mergeable,mergeStateStatus,reviewDecision,autoMergeRequest,mergeCommit,mergedAt
gh pr checks <number> --repo github.com/<target> --required --json name,state,bucket,link
gh api --method GET --hostname github.com --paginate --slurp repos/<target>/rules/branches/<encoded-base> -f per_page=100
gh api --method PUT --hostname github.com repos/<target>/pulls/<number>/merge --input <temp-json>
```

The final merge request body must be exactly:

```json
{
  "sha": "<expected-commit>",
  "merge_method": "merge"
}
```

---

### Task 1: Reproduce the GitHub URL rejection at the end-user boundary

**Files:**

- Modify: `desktop/src/testing/e2eBridge.ts`
- Modify: `desktop/tests/e2e/project-pr-review.spec.ts`

**Acceptance criteria:**

- The first committed change is a passing Playwright regression that drives the real Projects PR review UI with `https://github.com/anhle128/buzz` as the project and PR clone URL.
- The test confirms that the current Merge action reaches the workspace-relay URL rejection rather than GitHub behavior.
- The test records and asserts the exact merge invocation payload so later work cannot accidentally omit either repository URL, either branch, or the reviewed commit.

**Step 1: Add only the clone-URL override needed by the existing mock event builder**

Extend the E2E global declaration near the other project overrides:

```ts
declare global {
  interface Window {
    __BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__?: string;
  }
}
```

Inside `buildMockProjectEvents`, derive one clone URL per seeded project and reuse it for both the repository announcement and its PR metadata:

```ts
const cloneUrl =
  projectIndex === 0
    ? window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ ?? seed.cloneUrl
    : seed.cloneUrl;
```

Replace only the two `seed.cloneUrl` uses that create clone tags for that project.
Do not add a second mock project or a GitHub provider abstraction.

**Step 2: Write the passing reproduction before production changes**

Add a focused test beside the existing merge and conflict tests:

```ts
test("reproduces the GitHub-backed PR merge URL rejection", async ({ page }) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/anhle128/buzz";
    window.__BUZZ_E2E_PROJECT_MERGE_ERROR__ = {
      code: "merge_failed",
      message: "clone URL must use the active workspace relay",
      recovery: null,
    };
  });
  await installMockBridge(page);

  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Pull Request" }).click();
  const aliceRow = page
    .getByTestId("project-pull-request-row")
    .filter({ hasText: "alice" })
    .first();
  await aliceRow.getByRole("button", { name: /^#/ }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  await page.getByTestId("merge-pull-request-confirm-button").click();

  await expect(
    page.getByText("clone URL must use the active workspace relay"),
  ).toBeVisible();

  await expect
    .poll(() =>
      page.evaluate(() =>
        window.__BUZZ_E2E_COMMAND_PAYLOADS__?.find(
          (entry) => entry.command === "merge_project_pull_request",
        ),
      ),
    )
    .toMatchObject({
      payload: {
        input: {
          targetCloneUrl: "https://github.com/anhle128/buzz",
          sourceCloneUrl: "https://github.com/anhle128/buzz",
          targetBranch: "main",
          sourceBranch: "feature",
          expectedCommit: expect.any(String),
        },
      },
    });
});
```

Match the existing PR-open locator and exact mock branch values if the neighboring test exposes more specific accessible names.
Do not weaken assertions to generic toasts.

**Step 3: Run the focused E2E reproduction**

Run:

```bash
pnpm -C desktop build:e2e
pnpm -C desktop exec playwright test tests/e2e/project-pr-review.spec.ts --project=smoke --grep "reproduces the GitHub-backed PR merge URL rejection"
```

Expected: PASS, with the GitHub clone URL present in the captured command input and the current relay-validator failure visible in the UI.

If it fails because the exact accessible names differ, inspect the existing merge test and update only the locators.

**Step 4: Run the complete desktop Playwright suite**

Run:

```bash
pnpm -C desktop test:e2e
```

Expected: PASS.

**Step 5: Commit the regression baseline**

Run:

```bash
git add desktop/src/testing/e2eBridge.ts desktop/tests/e2e/project-pr-review.spec.ts
git diff --cached --check
git commit -s -m "test: reproduce GitHub PR merge rejection" \
  -m "Co-authored-by: Kevin Le <anhle12892@gmail.com>"
git log -1 --format=full
```

Expected: both human trailers appear once, in the required order.

---

### Task 2: Centralize strict GitHub identity and secure `gh` execution

**Files:**

- Create: `desktop/src-tauri/src/commands/project_github_pull_request.rs`
- Create: `desktop/src-tauri/src/commands/project_github_pull_request/tests.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs`
- Modify: `desktop/src-tauri/src/commands/project_git_exec.rs`
- Test: `desktop/src-tauri/src/commands/project_git_exec.rs`

**Acceptance criteria:**

- One parser owns the exact GitHub repository identity rules.
- Existing clone and workspace validators call that parser instead of retaining a parallel GitHub URL grammar.
- `gh` is resolved once per attempt, invoked without a shell, given closed stdin, killed after 60 seconds, and never returns unbounded or unredacted diagnostics.
- Missing CLI and missing auth map to stable structured codes.
- No new crate is added.

**Step 1: Register the focused module and its test submodule**

Add this sibling declaration in `commands/mod.rs`:

```rust
mod project_github_pull_request;
```

End `project_github_pull_request.rs` with:

```rust
#[cfg(test)]
mod tests;
```

Keep process tests in the child file so the production module stays under the repository's 1,000-line source-file limit.

**Step 2: Write strict URL parser tests first**

Cover all of these inputs in a table-driven test:

```rust
let accepted = [
    ("https://github.com/anhle128/buzz", "anhle128", "buzz"),
    ("https://github.com/anhle128/buzz.git", "anhle128", "buzz"),
    ("https://github.com/Ocean-Labs/buzz_desktop", "Ocean-Labs", "buzz_desktop"),
];

let rejected = [
    "http://github.com/anhle128/buzz",
    "https://user@github.com/anhle128/buzz",
    "https://github.com:443/anhle128/buzz",
    "https://github.com/anhle128/buzz/extra",
    "https://github.com/anhle128/buzz?tab=readme",
    "https://github.com/anhle128/buzz#readme",
    "https://github.com/anhle128",
    "https://github.com/anh_le/buzz",
    "https://github.com/anhle128/buzz%2Freleases",
    "https://github.com/anhle128/buzz\n",
];
```

Also assert that `GitHubRepoRef::is_github_host` returns true for a malformed `github.com` URL such as one with a query string.
That lets the router reject malformed GitHub intent instead of accidentally falling through to the Buzz-hosted path.

Run:

```bash
cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pull_request::tests::github_repo -- --nocapture
```

Expected: FAIL because the parser does not exist.

**Step 3: Implement the minimum shared parser**

Use `url::Url`; do not hand-parse the authority:

```rust
impl GitHubRepoRef {
    pub(crate) fn is_github_host(raw: &str) -> bool {
        Url::parse(raw)
            .ok()
            .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
            .is_some_and(|host| host == "github.com")
    }

    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        if raw.chars().any(char::is_whitespace) {
            return Err("GitHub repository URL must not contain whitespace.".to_string());
        }
        if !raw.starts_with("https://github.com/") {
            return Err("GitHub repository URL must use https://github.com/.".to_string());
        }
        let url = Url::parse(raw).map_err(|_| "Invalid GitHub repository URL.".to_string())?;
        if url.scheme() != "https"
            || url.host_str() != Some("github.com")
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("GitHub repository URL must be a plain github.com HTTPS URL.".to_string());
        }

        let segments = url
            .path_segments()
            .ok_or_else(|| "GitHub repository URL must name an owner and repository.".to_string())?
            .collect::<Vec<_>>();
        if segments.len() != 2 || segments.iter().any(|segment| segment.is_empty()) {
            return Err("GitHub repository URL must contain exactly owner/repository.".to_string());
        }

        let owner = segments[0];
        let repo = segments[1].strip_suffix(".git").unwrap_or(segments[1]);
        let owner_ok = !owner.is_empty()
            && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        let repo_ok = !repo.is_empty()
            && repo
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if !owner_ok || !repo_ok {
            return Err("Invalid GitHub repository owner or name.".to_string());
        }

        Ok(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
    }

    fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    fn gh_repo(&self) -> String {
        format!("github.com/{}", self.slug())
    }

    fn pull_url(&self, number: u64) -> String {
        format!("https://github.com/{}/pull/{number}", self.slug())
    }

    fn pulls_url(&self) -> String {
        format!("https://github.com/{}/pulls", self.slug())
    }
}
```

Because `Url::path_segments` exposes decoded or encoded segments according to the crate behavior, keep the explicit rejected `%2F` test.
If that test reveals a retained percent sequence, reject any `%` in both segments before character validation.

**Step 4: Replace the existing private GitHub clone validator**

In `project_git_exec.rs`, import `GitHubRepoRef` and reduce `validate_github_clone_url` to the shared parser:

```rust
fn validate_github_clone_url(clone_url: &str) -> Result<(), String> {
    GitHubRepoRef::parse(clone_url).map(|_| ())
}
```

Run every existing caller through `rg` before editing:

```bash
rg -n "validate_github_clone_url|validate_local_clone_url_for_workspace|validate_local_clone_url" desktop/src-tauri/src/commands
```

Expected callers remain `build_git_clone_auth_config`, `validate_local_clone_url`, `validate_local_clone_url_for_workspace`, `project_git_workflow.rs`, and `project_terminal.rs`.
Do not change their behavior beyond consolidating the parser.

**Step 5: Write process-runner tests before the runner**

Use a temporary executable script on Unix and the existing test conventions on other platforms.
The fake executable must record argv and verify stdin reads EOF:

```rust
#[cfg(unix)]
fn fake_gh(script: &str) -> (tempfile::TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("create fake gh directory");
    let path = dir.path().join("gh");
    std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{script}\n"))
        .expect("write fake gh");
    let mut permissions = std::fs::metadata(&path)
        .expect("stat fake gh")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod fake gh");
    (dir, path)
}
```

Add tests for:

- Direct argv preservation for spaces and metacharacters.
- EOF on stdin.
- Concurrent stdout and stderr draining.
- 64 KiB retention limit on each stream.
- 60-second production timeout through a test-only shorter timeout.
- Child termination on timeout.
- Redaction of `ghp_`, `github_pat_`, `Authorization:`, `token`, `Bearer`, and URL userinfo.
- 8 KiB diagnostic limit after redaction.
- Exit `127` or resolution failure mapping to `github_cli_missing`.
- Failed auth status mapping to `github_auth_required` without exposing the raw token-bearing output.

Run the focused tests and confirm RED before implementation.

**Step 6: Implement `GhRunner` with existing process patterns**

Mirror `run_git`'s direct `std::process::Command`, pipe-draining threads, poll loop, kill, and no-window setup.
Do not extract a generic subprocess framework.

Use these internal types:

```rust
const GH_TIMEOUT: Duration = Duration::from_secs(60);
const GH_STREAM_LIMIT: usize = 64 * 1024;
const GH_DIAGNOSTIC_LIMIT: usize = 8 * 1024;

struct GhRunner {
    binary: PathBuf,
    timeout: Duration,
}

struct GhOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl GhRunner {
    fn discover() -> Result<Self, ProjectPullRequestMergeError> {
        let binary = managed_agents::resolve_command("gh").ok_or_else(|| {
            ProjectPullRequestMergeError::new(
                "github_cli_missing",
                "GitHub CLI is required. Install gh, then retry.",
            )
        })?;
        Ok(Self {
            binary,
            timeout: GH_TIMEOUT,
        })
    }

    fn ensure_auth(&self) -> Result<(), ProjectPullRequestMergeError> {
        let output = self.run(&[
            OsString::from("auth"),
            OsString::from("status"),
            OsString::from("--hostname"),
            OsString::from("github.com"),
        ])?;
        if output.status.success() {
            return Ok(());
        }
        Err(ProjectPullRequestMergeError::new(
            "github_auth_required",
            "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        ))
    }
}
```

`run` must set `stdin(Stdio::null())`, `stdout(Stdio::piped())`, and `stderr(Stdio::piped())` before spawning.
It must call `configure_no_window(&mut command)` before `spawn`.
The reader threads must retain only the first 64 KiB while continuing to drain the rest.
The timeout branch must call `kill`, then `wait`, then join both readers.

Implement redaction with the already-installed `regex = "1"` dependency and bounded string construction.
Do not add a dependency.

**Step 7: Run focused and full Rust tests**

Run:

```bash
cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pull_request -- --nocapture
just desktop-tauri-test
git diff --check
```

Expected: PASS.

**Step 8: Commit the parser and runner foundation**

Run:

```bash
git add desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/commands/project_git_exec.rs \
  desktop/src-tauri/src/commands/project_github_pull_request.rs \
  desktop/src-tauri/src/commands/project_github_pull_request/tests.rs
git diff --cached --check
git commit -s -m "feat: add secure GitHub PR command foundation" \
  -m "Co-authored-by: Kevin Le <anhle12892@gmail.com>"
git log -1 --format=full
```

---

### Task 3: Make GitHub PR lookup and creation idempotent

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_pull_request.rs`
- Modify: `desktop/src-tauri/src/commands/project_github_pull_request/tests.rs`
- Modify: `desktop/src-tauri/src/commands/project_git_merge_error.rs`
- Test: `desktop/src-tauri/src/commands/project_git_merge_error.rs`

**Acceptance criteria:**

- Lookup is pinned to the target repo and filters locally to the exact source repo, base branch, and head branch.
- One open PR is reused.
- Ambiguous results fail closed.
- A merged PR with the reviewed head is recoverable.
- Closed-unmerged PRs block replacement creation.
- No match creates exactly one PR, then re-runs lookup before continuing.
- JSON bodies never appear in argv.

**Step 1: Add the URL recovery variant required by selection failures**

Replace the stringly recovery struct with:

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ProjectPullRequestMergeRecovery {
    OpenTerminal {
        #[serde(rename = "targetBranch")]
        target_branch: String,
        #[serde(rename = "sourceBranch")]
        source_branch: String,
    },
    OpenUrl {
        url: String,
        reasons: Vec<String>,
    },
}
```

Update `conflict` to construct `OpenTerminal`.
Add a crate-visible constructor that caps reason count and length:

```rust
pub(crate) fn open_url(
    code: &str,
    message: impl Into<String>,
    url: String,
    reasons: impl IntoIterator<Item = String>,
) -> Self {
    let reasons = reasons
        .into_iter()
        .take(20)
        .map(|reason| reason.chars().take(200).collect())
        .collect();
    Self {
        code: code.to_string(),
        message: message.into(),
        recovery: Some(ProjectPullRequestMergeRecovery::OpenUrl { url, reasons }),
    }
}
```

Update existing serialization tests to prove `open_terminal` JSON is unchanged.
Add an `open_url` serialization test with capped reasons and the exact URL.

**Step 2: Write pure selection tests first**

Deserialize realistic GitHub REST fixtures into this minimal type:

```rust
#[derive(Clone, Debug, Deserialize)]
struct GitHubPullSummary {
    number: u64,
    html_url: String,
    state: String,
    merged_at: Option<String>,
    head: GitHubPullHead,
    base: GitHubPullBase,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubPullHead {
    sha: String,
    #[serde(rename = "ref")]
    branch: String,
    repo: GitHubNamedRepo,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubPullBase {
    #[serde(rename = "ref")]
    branch: String,
    repo: GitHubNamedRepo,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubNamedRepo {
    full_name: String,
}
```

Test these cases independently:

1. One exact open match returns `Reuse`.
2. Two exact open matches return `github_pr_ambiguous` with the repository `/pulls` recovery.
3. One merged exact match with matching head returns `Reuse`.
4. Two merged exact matches with matching head return ambiguous.
5. One closed-unmerged exact match returns `github_pr_blocked` with that exact PR URL.
6. A server result with a different source repo, source branch case, base branch case, or target repo is ignored.
7. No exact match returns `Create`.

Run:

```bash
cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pull_request::tests::select_pull -- --nocapture
```

Expected: FAIL because selection does not exist.

**Step 3: Implement the pure selector**

Use one internal enum, not a provider abstraction:

```rust
enum PullSelection {
    Reuse(GitHubPullSummary),
    Create,
}

fn select_pull(
    pulls: Vec<GitHubPullSummary>,
    input: &GitHubMergeInput,
) -> Result<PullSelection, ProjectPullRequestMergeError> {
    let mut open = Vec::new();
    let mut merged = Vec::new();
    let mut closed = Vec::new();
    for pull in pulls.into_iter().filter(|pull| {
        pull.base.repo.full_name.eq_ignore_ascii_case(&input.target.slug())
            && pull.head.repo.full_name.eq_ignore_ascii_case(&input.source.slug())
            && pull.base.branch == input.target_branch
            && pull.head.branch == input.source_branch
    }) {
        if pull.state.eq_ignore_ascii_case("open") {
            open.push(pull);
        } else if pull.merged_at.is_some()
            && pull.head.sha.eq_ignore_ascii_case(&input.expected_commit)
        {
            merged.push(pull);
        } else if pull.merged_at.is_none() {
            closed.push(pull);
        }
    }

    match open.len() {
        1 => return Ok(PullSelection::Reuse(open.remove(0))),
        2.. => {
            return Err(ProjectPullRequestMergeError::open_url(
                "github_pr_ambiguous",
                "More than one open GitHub pull request matched.",
                input.target.pulls_url(),
                Vec::new(),
            ));
        }
        0 => {}
    }
    match merged.len() {
        1 => return Ok(PullSelection::Reuse(merged.remove(0))),
        2.. => {
            return Err(ProjectPullRequestMergeError::open_url(
                "github_pr_ambiguous",
                "More than one merged GitHub pull request matched the reviewed commit.",
                input.target.pulls_url(),
                Vec::new(),
            ));
        }
        0 => {}
    }
    if let Some(pull) = closed.into_iter().min_by_key(|pull| pull.number) {
        return Err(ProjectPullRequestMergeError::open_url(
            "github_pr_blocked",
            "The matching GitHub pull request was closed without merging.",
            pull.html_url,
            vec!["Reopen or replace the pull request on GitHub.".to_string()],
        ));
    }
    Ok(PullSelection::Create)
}
```

Keep the matching code in this function so API and retry paths cannot diverge.

**Step 4: Add a bounded JSON helper for `gh` output**

Add:

```rust
impl GhRunner {
    fn run_json<T: DeserializeOwned>(
        &self,
        args: &[OsString],
        accepted_codes: &[i32],
    ) -> Result<T, ProjectPullRequestMergeError> {
        let output = self.run(args)?;
        let code = output.status.code().unwrap_or(-1);
        if !output.status.success() && !accepted_codes.contains(&code) {
            return Err(ProjectPullRequestMergeError::new(
                "github_merge_failed",
                redact_diagnostic(&output.stderr),
            ));
        }
        serde_json::from_str(&output.stdout).map_err(|_| {
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "GitHub CLI returned an unexpected JSON response. Update gh, then retry.",
            )
        })
    }
}
```

Do not include raw JSON in parse errors.

**Step 5: Implement exact list and local filtering**

Build `OsString` arguments directly from validated components.
Flatten the `--slurp` page array before passing it to `select_pull`:

```rust
let pages: Vec<Vec<GitHubPullSummary>> = gh.run_json(&[
    "api".into(),
    "--method".into(),
    "GET".into(),
    "--hostname".into(),
    "github.com".into(),
    "--paginate".into(),
    "--slurp".into(),
    format!("repos/{}/pulls", input.target.slug()).into(),
    "-f".into(),
    "state=all".into(),
    "-f".into(),
    format!("head={}:{}", input.source.owner, input.source_branch).into(),
    "-f".into(),
    format!("base={}", input.target_branch).into(),
    "-f".into(),
    "per_page=100".into(),
], &[])?;
let pulls = pages.into_iter().flatten().collect();
```

**Step 6: Write a permission-restricted JSON input helper**

Use `tempfile::NamedTempFile`, `serde_json::to_writer`, and `flush`:

```rust
fn json_input<T: Serialize>(value: &T) -> Result<NamedTempFile, ProjectPullRequestMergeError> {
    let mut file = tempfile::Builder::new()
        .prefix("buzz-gh-")
        .tempfile()
        .map_err(|_| ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "Buzz could not prepare the GitHub request.",
        ))?;
    serde_json::to_writer(file.as_file_mut(), value).map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "Buzz could not encode the GitHub request.",
        )
    })?;
    file.as_file_mut().flush().map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "Buzz could not finish the GitHub request.",
        )
    })?;
    Ok(file)
}
```

The file handle remains alive through `gh api --input` and drops immediately after the command.

**Step 7: Implement create, then exact re-lookup**

Use this request shape:

```rust
#[derive(Serialize)]
struct CreatePullRequest<'a> {
    title: &'a str,
    body: &'a str,
    head: String,
    base: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    head_repo: Option<String>,
}
```

Set `head` to `<source-owner>:<source-branch>`.
Set `head_repo` to `Some(input.source.slug())` only when target and source repositories differ case-insensitively.
Call the POST endpoint with `--input <temp-path>` and no title or body in argv.
Whether POST succeeds or returns an uncertain transport failure, call the exact list function once before deciding the outcome.
If lookup finds the exact PR, proceed because GitHub created it even though the original acknowledgement was lost.
If lookup still yields `Create` after a successful POST, return `github_merge_failed` rather than issuing a second POST.
If lookup still yields `Create` after a failed POST, return the sanitized original POST error.

**Step 8: Test command shape and lost-response idempotency**

Use the fake `gh` to assert:

- Every API call contains `--hostname github.com`.
- The list call has the exact target slug, head, base, and pagination fields.
- The POST body contains title, body, head, base, and conditional `head_repo`.
- The title/body do not appear in argv.
- A simulated failed POST followed by a lookup that finds the created PR does not issue a second POST.
- A retry with an existing open PR never POSTs.

Do not log or snapshot the temporary file path beyond the test process.

**Step 9: Run focused and full Rust tests**

Run:

```bash
cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pull_request -- --nocapture
just desktop-tauri-test
git diff --check
```

Expected: PASS.

**Step 10: Commit idempotent PR lifecycle support**

Run:

```bash
git add desktop/src-tauri/src/commands/project_git_merge_error.rs \
  desktop/src-tauri/src/commands/project_github_pull_request.rs \
  desktop/src-tauri/src/commands/project_github_pull_request/tests.rs
git diff --cached --check
git commit -s -m "feat: find or create exact GitHub pull requests" \
  -m "Co-authored-by: Kevin Le <anhle12892@gmail.com>"
git log -1 --format=full
```

---

### Task 4: Enforce GitHub gates and perform one atomic immediate merge

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_pull_request.rs`
- Modify: `desktop/src-tauri/src/commands/project_github_pull_request/tests.rs`
- Modify: `desktop/src-tauri/src/commands/project_git_merge_error.rs`
- Test: `desktop/src-tauri/src/commands/project_git_merge_error.rs`

**Acceptance criteria:**

- The reviewed head SHA is checked before every merge decision.
- Drafts, conflicts, missing reviews, requested changes, failing or pending required checks, existing auto-merge, unknown state, and merge queues all fail closed.
- Rule-discovery failure fails closed.
- The REST merge request includes the reviewed SHA and `merge_method: merge` in a temporary JSON body.
- No `gh pr merge`, `--admin`, auto-merge, queue, native push, or merge fallback exists.
- Post-query verification is mandatory before returning a merge commit.

**Step 1: Write the pure gate-state table before implementation**

Use these minimal GitHub CLI response types:

```rust
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubPullView {
    number: u64,
    url: String,
    state: String,
    head_ref_oid: String,
    is_draft: bool,
    mergeable: String,
    merge_state_status: String,
    review_decision: String,
    auto_merge_request: Option<serde_json::Value>,
    merge_commit: Option<GitHubOid>,
    merged_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubOid {
    oid: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubCheck {
    name: String,
    state: String,
    bucket: String,
    link: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubRule {
    #[serde(rename = "type")]
    kind: String,
}
```

Represent the decision with:

```rust
enum PullDecision {
    AlreadyMerged { merge_commit: String },
    Ready,
    Blocked(Vec<String>),
}
```

Table-test:

- Head mismatch returns `github_branch_changed` before considering any other state.
- `MERGED` with matching head and merge commit returns `AlreadyMerged`.
- `MERGED` without `mergedAt` or merge commit blocks finalization.
- `CLOSED` without merge blocks.
- Draft blocks.
- `CONFLICTING` and `UNKNOWN` mergeability block.
- `REVIEW_REQUIRED` and `CHANGES_REQUESTED` block.
- Any required check whose bucket is not `pass` blocks with its name and state.
- Existing `autoMergeRequest` blocks.
- `merge_queue` rule blocks.
- Rule discovery unavailable blocks.
- `CLEAN` with passing required checks is ready.
- `UNSTABLE` is ready only when all required checks pass, because non-required failures may be mergeable.
- Any other merge-state status blocks.

Run the focused test and confirm RED.

**Step 2: Implement exact path-segment encoding without a dependency**

Branch names may contain `/`, so encode one path segment byte-by-byte:

```rust
fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}").expect("write to string");
        }
    }
    encoded
}
```

Add tests for `main`, `release/2026.08`, and an already normalized branch containing `@`.
Do not introduce a percent-encoding crate.

**Step 3: Implement view, required-check, and active-rule reads**

`gh pr checks` exit code `8` is accepted as a valid pending-check response.
Normalize GitHub CLI's documented no-required-check output to an empty vector only when its bounded stderr matches the known no-check condition.
Every other nonzero code is `github_merge_failed` with a PR URL recovery.

Flatten rule pages and treat inability to query rules as a blocker rather than ready.
The rule endpoint must use `encode_path_segment(&input.target_branch)`.

**Step 4: Implement pure gate evaluation**

Use one function that accepts already-parsed view, checks, and rule result:

```rust
fn decide_pull(
    view: &GitHubPullView,
    checks: &[GitHubCheck],
    rules: Result<&[GitHubRule], &str>,
    expected_commit: &str,
) -> Result<PullDecision, ProjectPullRequestMergeError> {
    if !view.head_ref_oid.eq_ignore_ascii_case(expected_commit) {
        return Err(ProjectPullRequestMergeError::new(
            "github_branch_changed",
            "The GitHub pull request branch changed. Refresh before merging.",
        ));
    }
    if view.state == "MERGED" {
        let merge_commit = view
            .merge_commit
            .as_ref()
            .filter(|_| view.merged_at.is_some())
            .map(|commit| commit.oid.clone())
            .ok_or_else(|| ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "GitHub reported a merge without a verifiable merge commit.",
            ))?;
        return Ok(PullDecision::AlreadyMerged { merge_commit });
    }

    let mut reasons = Vec::new();
    if view.state != "OPEN" {
        reasons.push("The pull request is closed without merging.".to_string());
    }
    if view.is_draft {
        reasons.push("The pull request is still a draft.".to_string());
    }
    if matches!(view.mergeable.as_str(), "CONFLICTING" | "UNKNOWN") {
        reasons.push(format!("Mergeability is {}.", view.mergeable));
    }
    if matches!(view.review_decision.as_str(), "REVIEW_REQUIRED" | "CHANGES_REQUESTED") {
        reasons.push(format!("Review state is {}.", view.review_decision));
    }
    if view.auto_merge_request.is_some() {
        reasons.push("Auto-merge is already enabled on GitHub.".to_string());
    }
    for check in checks.iter().filter(|check| check.bucket != "pass") {
        reasons.push(format!("Required check {} is {}.", check.name, check.state));
    }
    match rules {
        Ok(rules) if rules.iter().any(|rule| rule.kind == "merge_queue") => {
            reasons.push("The target branch requires GitHub's merge queue.".to_string());
        }
        Err(message) => reasons.push(format!("Branch rules could not be verified: {message}")),
        Ok(_) => {}
    }
    let merge_state_ready = view.merge_state_status == "CLEAN"
        || (view.merge_state_status == "UNSTABLE"
            && checks.iter().all(|check| check.bucket == "pass"));
    if !merge_state_ready {
        reasons.push(format!("Merge state is {}.", view.merge_state_status));
    }

    if reasons.is_empty() {
        Ok(PullDecision::Ready)
    } else {
        Ok(PullDecision::Blocked(reasons))
    }
}
```

Do not duplicate gate decisions in the process-orchestration function.

**Step 5: Implement the atomic immediate merge**

Use:

```rust
#[derive(Serialize)]
struct MergePullRequest<'a> {
    sha: &'a str,
    merge_method: &'static str,
}

#[derive(Deserialize)]
struct GitHubMergeResponse {
    sha: String,
    merged: bool,
    message: String,
}
```

Write `MergePullRequest { sha: &input.expected_commit, merge_method: "merge" }` through `json_input`.
Call only the REST `PUT` operation from the Stable Contracts section.
Map GitHub stale-head or conflict rejection to `github_branch_changed` when the bounded response identifies SHA mismatch.
Map all other rejections to `github_merge_failed` with the exact PR URL.

After `merged: true`, re-run `gh pr view`.
Accept success only when state is `MERGED`, `headRefOid` still matches, `mergedAt` exists, and `mergeCommit.oid` exists.
Return the re-query merge commit, not the unverified response SHA.

**Step 6: Compose the one GitHub orchestrator**

Implement the Stable Contracts function in this order:

```rust
pub(crate) fn merge_github_pull_request(
    input: GitHubMergeInput,
) -> Result<GitHubMergeOutcome, ProjectPullRequestMergeError> {
    let gh = GhRunner::discover()?;
    gh.ensure_auth()?;
    let pull = ensure_pull(&gh, &input)?;
    let view = load_pull_view(&gh, &input.target, pull.number)?;
    let checks = load_required_checks(&gh, &input.target, pull.number)?;
    let rules = load_active_rules(&gh, &input.target, &input.target_branch);

    match decide_pull(&view, &checks, rules.as_deref(), &input.expected_commit)? {
        PullDecision::AlreadyMerged { merge_commit } => Ok(GitHubMergeOutcome {
            message: format!(
                "GitHub pull request #{} was already merged.",
                pull.number
            ),
            merge_commit,
        }),
        PullDecision::Blocked(reasons) => Err(ProjectPullRequestMergeError::open_url(
            "github_pr_blocked",
            "GitHub does not allow an immediate merge yet.",
            view.url,
            reasons,
        )),
        PullDecision::Ready => merge_and_verify(&gh, &input, &view),
    }
}
```

Make `load_active_rules` return a typed result that preserves failure as a blocker.
The call site above may need `rules.as_deref().map_err(String::as_str)` to match the pure decision signature; keep the logic equivalent and explicit.

**Step 7: Test every process and state transition**

Fake-`gh` tests must assert:

- No merge call when head changed.
- No merge call for every blocked state.
- No merge call and correct merge commit for already-merged recovery.
- Merge argv contains the target PR endpoint but no title, body, or token.
- Merge temp JSON contains exactly `sha` and `merge_method`.
- No `gh pr merge`, `--admin`, `--auto`, or queue command appears in the captured argv log.
- A successful PUT followed by non-merged re-query is still an error.
- A successful PUT followed by verified merged state returns the verified merge commit.
- An auth or process error cannot return a merge outcome.

**Step 8: Run focused and full Rust tests**

Run:

```bash
cargo test --manifest-path desktop/src-tauri/Cargo.toml project_git_merge_error -- --nocapture
cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pull_request -- --nocapture
just desktop-tauri-test
git diff --check
```

Expected: PASS.

**Step 9: Commit the gate and merge state machine**

Run:

```bash
git add desktop/src-tauri/src/commands/project_git_merge_error.rs \
  desktop/src-tauri/src/commands/project_github_pull_request.rs \
  desktop/src-tauri/src/commands/project_github_pull_request/tests.rs
git diff --cached --check
git commit -s -m "feat: enforce GitHub gates before immediate merge" \
  -m "Co-authored-by: Kevin Le <anhle12892@gmail.com>"
git log -1 --format=full
```

---

### Task 5: Route GitHub pairs without changing Buzz-hosted merge semantics

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_git_workflow.rs`
- Test: `desktop/src-tauri/src/commands/project_git_workflow.rs`
- Modify: `desktop/src/shared/api/projectGit.ts`
- Modify: `desktop/src/features/projects/pullRequestMutations.ts`
- Modify: `desktop/src/testing/e2eBridge.ts`

**Acceptance criteria:**

- Nostr owner authorization and PR metadata validation happen before any GitHub process starts.
- Both valid GitHub URLs use the GitHub state machine.
- A malformed GitHub URL or mixed GitHub/Buzz pair fails before any clone, push, `gh`, or status publication.
- Neither GitHub URL uses the byte-for-byte-equivalent existing Buzz native merge path.
- The GitHub repository owner is never compared with the Nostr pubkey owner.
- Both paths share only final merged-status construction and relay publication.
- Title and body cross the frontend/Tauri boundary unchanged.

**Step 1: Extend the input contract and tests**

Add `title` and `body` to the Rust `ProjectPullRequestMergeInput` and the TypeScript input:

```rust
pub struct ProjectPullRequestMergeInput {
    target_clone_url: String,
    source_clone_url: String,
    target_owner: String,
    repo_address: String,
    pull_request_id: String,
    pull_request_author: String,
    status_created_at: u64,
    target_branch: String,
    source_branch: String,
    expected_commit: String,
    title: String,
    body: String,
}
```

```ts
export async function mergeProjectPullRequest(input: {
  targetCloneUrl: string;
  sourceCloneUrl: string;
  targetOwner: string;
  repoAddress: string;
  pullRequestId: string;
  pullRequestAuthor: string;
  statusCreatedAt: number;
  targetBranch: string;
  sourceBranch: string;
  expectedCommit: string;
  title: string;
  body: string;
}): Promise<ProjectRepoMergeResult> {
  let result: RawProjectRepoMergeResult;
  try {
    result = await invokeTauri<RawProjectRepoMergeResult>(
      "merge_project_pull_request",
      { input },
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
  return {
    message: result.message,
    mergeCommit: result.merge_commit,
    statusEvent: result.status_event,
    statusPublicationError: result.status_publication_error,
  };
}
```

In `pullRequestMutations.ts`, pass:

```ts
title: pullRequest.title,
body: pullRequest.content,
```

Update the E2E bridge command input type and captured-payload assertions.
Add a test proving body whitespace and Markdown remain unchanged.

**Step 2: Extract the existing native body without changing it**

Move only the current clone, fetch, expected-head check, merge, conflict classification, merge-commit lookup, and push block into a private blocking helper in `project_git_workflow.rs`:

```rust
fn merge_buzz_pull_request(
    target_clone_url: String,
    source_clone_url: String,
    target_branch: String,
    source_branch: String,
    expected_commit: String,
    merger_pubkey: String,
    auth: GitAuthConfig,
) -> Result<ProjectRepoMergeGitResult, ProjectPullRequestMergeError> {
    let temp_dir = tempfile::tempdir().map_err(|error| format!("create temp dir: {error}"))?;
    let repo_dir = temp_dir.path().join("repo");
    let repo_path = repo_dir
        .to_str()
        .ok_or_else(|| "temporary repository path is not UTF-8".to_string())?;
    run_git(
        &[
            "clone",
            "--filter=blob:none",
            "--no-tags",
            "--branch",
            target_branch.as_str(),
            "--single-branch",
            "--end-of-options",
            target_clone_url.as_str(),
            repo_path,
        ],
        None,
        &auth,
    )?;
    run_git(
        &[
            "fetch",
            "--quiet",
            "--end-of-options",
            source_clone_url.as_str(),
            source_branch.as_str(),
        ],
        Some(&repo_dir),
        &auth,
    )?;
    let source_head = run_git(&["rev-parse", "FETCH_HEAD"], Some(&repo_dir), &auth)
        .ok()
        .and_then(|output| first_output_line(&output))
        .ok_or_else(|| "Could not resolve the pull request branch.".to_string())?;
    if source_head.to_ascii_lowercase() != expected_commit {
        return Err(ProjectPullRequestMergeError::new(
            "branch_changed",
            "The pull request branch changed. Refresh the pull request before merging.",
        ));
    }
    let merge_email = format!("{merger_pubkey}@users.noreply.buzz");
    if let Err(error) = run_git(
        &[
            "-c",
            "user.name=Buzz User",
            "-c",
            format!("user.email={merge_email}").as_str(),
            "merge",
            "--no-edit",
            "--end-of-options",
            expected_commit.as_str(),
        ],
        Some(&repo_dir),
        &auth,
    ) {
        let has_conflicts = run_git(
            &["diff", "--name-only", "--diff-filter=U"],
            Some(&repo_dir),
            &auth,
        )
        .is_ok_and(|output| !output.trim().is_empty());
        return Err(classify_merge_error(
            error,
            has_conflicts,
            &target_branch,
            &source_branch,
        ));
    }
    let merge_commit = run_git(&["rev-parse", "HEAD"], Some(&repo_dir), &auth)
        .ok()
        .and_then(|output| first_output_line(&output))
        .ok_or_else(|| "Could not resolve the merge commit.".to_string())?;
    run_git(
        &[
            "push",
            "--end-of-options",
            "origin",
            format!("HEAD:{target_branch}").as_str(),
        ],
        Some(&repo_dir),
        &auth,
    )?;
    Ok(ProjectRepoMergeGitResult {
        message: format!("Merged {source_branch} into {target_branch}."),
        merge_commit,
    })
}
```

`GitAuthConfig` is already `pub(crate)` in `project_git_exec.rs`; do not widen its visibility.
Do not create a provider trait.

**Step 3: Write router tests before changing the command**

Extract a pure host decision only if it reduces duplicated branching:

```rust
enum PullRequestRepoRoute {
    Buzz,
    GitHub {
        target: GitHubRepoRef,
        source: GitHubRepoRef,
    },
}

fn classify_pull_request_route(
    target_clone_url: &str,
    source_clone_url: &str,
) -> Result<PullRequestRepoRoute, ProjectPullRequestMergeError> {
    let target_is_github = GitHubRepoRef::is_github_host(target_clone_url);
    let source_is_github = GitHubRepoRef::is_github_host(source_clone_url);
    match (target_is_github, source_is_github) {
        (true, true) => Ok(PullRequestRepoRoute::GitHub {
            target: GitHubRepoRef::parse(target_clone_url).map_err(|message| {
                ProjectPullRequestMergeError::new("github_merge_failed", message)
            })?,
            source: GitHubRepoRef::parse(source_clone_url).map_err(|message| {
                ProjectPullRequestMergeError::new("github_merge_failed", message)
            })?,
        }),
        (false, false) => Ok(PullRequestRepoRoute::Buzz),
        _ => Err(ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "Source and target repositories must use the same supported host.",
        )),
    }
}
```

Test:

- Two strict GitHub URLs select GitHub.
- Two Buzz workspace URLs select Buzz.
- GitHub target plus Buzz source returns a structured error.
- Buzz target plus GitHub source returns a structured error.
- A `github.com` URL with query, fragment, userinfo, port, or extra path returns an invalid-GitHub error and never selects Buzz.

Run the focused test and confirm RED.

**Step 4: Reorder validation and route once**

The command body must follow this order:

```rust
let target_owner = target_owner.trim().to_ascii_lowercase();
if target_owner.len() != 64 || !target_owner.chars().all(|c| c.is_ascii_hexdigit()) {
    return Err("Invalid target repository owner.".to_string().into());
}
let owner_identity = project_owner_identity(&app, &state, &target_owner)?;
let merger_pubkey = owner_identity.keys.public_key().to_hex();
let target_branch = normalize_branch_option(Some(&target_branch))
    .ok_or_else(|| "Invalid target branch.".to_string())?;
let source_branch = normalize_branch_option(Some(&source_branch))
    .ok_or_else(|| "Invalid source branch.".to_string())?;
let expected_commit = normalize_commit(&expected_commit)
    .ok_or_else(|| "Invalid pull request commit.".to_string())?;
let (pull_request_id, pull_request_author) = validate_merge_status_metadata(
    &repo_address,
    &merger_pubkey,
    &pull_request_id,
    &pull_request_author,
)?;

let route = classify_pull_request_route(&target_clone_url, &source_clone_url)?;
```

Validate `title.trim().is_empty()` only inside the GitHub route because the existing Buzz path does not need a GitHub title.
After validation, pass the original title and body without trimming or rewriting them.
Keep `target_owner` versus clone-owner equality and `build_git_auth_config_for_keys` only inside the Buzz route.

**Step 5: Return one common git result and publish once**

Use the existing `ProjectRepoMergeGitResult` as the common internal shape:

```rust
let git_result = match route {
    PullRequestRepoRoute::GitHub { target, source } => {
        let github_input = GitHubMergeInput {
            target,
            source,
            target_branch,
            source_branch,
            expected_commit,
            title,
            body,
        };
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            merge_github_pull_request(github_input)
        })
        .await
        .map_err(|error| ProjectPullRequestMergeError::new(
            "merge_task_failed",
            format!("GitHub merge task failed: {error}"),
        ))??;
        ProjectRepoMergeGitResult {
            message: outcome.message,
            merge_commit: outcome.merge_commit,
        }
    }
    PullRequestRepoRoute::Buzz => {
        validate_workspace_clone_url(&target_clone_url, &state)?;
        validate_workspace_clone_url(&source_clone_url, &state)?;
        if clone_url_owner(&target_clone_url).as_deref() != Some(target_owner.as_str()) {
            return Err("Target clone URL does not match the repository owner."
                .to_string()
                .into());
        }
        let auth = build_git_auth_config_for_keys(&owner_identity.keys)?;
        tauri::async_runtime::spawn_blocking(move || {
            merge_buzz_pull_request(
                target_clone_url,
                source_clone_url,
                target_branch,
                source_branch,
                expected_commit,
                merger_pubkey,
                auth,
            )
        })
        .await
        .map_err(|error| ProjectPullRequestMergeError::new(
            "merge_task_failed",
            format!("pull request merge task failed: {error}"),
        ))??
    }
};
```

Leave the existing `build_merged_status_event`, `Event::from_json`, `submit_signed_event_with_keys`, and `ProjectRepoMergeResult` construction after this match.
That shared tail is the only place allowed to construct or publish kind `1631`.

**Step 6: Prove publication ordering**

Add tests or a test-only injectable GitHub runner seam local to the module that assert:

- Every GitHub error leaves the status builder uncalled.
- A blocked PR produces no signed status.
- An unverified merge response produces no signed status.
- A verified GitHub merge supplies its merge commit to the existing status builder.
- A relay publication failure still returns the signed event and error.
- Retrying the existing Publish merged status action does not invoke the merge command again.

Do not add a production dependency-injection interface.
Prefer pure function tests plus the existing E2E bridge observation points.

**Step 7: Update the original regression to the new route**

Remove the injected old `merge_failed` result from Task 1.
Inject `github_auth_required` instead and assert:

- The GitHub-specific confirmation copy appears before the invoke.
- Confirming no longer surfaces the workspace-relay URL rejection.
- The captured payload includes both GitHub URLs, title, body, both branches, and expected commit.
- The stable GitHub auth error reaches the frontend parser even before the persistent card lands in Task 6.

At this task boundary a toast is acceptable; Task 6 replaces it with persistent recovery UX.

**Step 8: Run focused and full package tests**

Run:

```bash
cargo test --manifest-path desktop/src-tauri/Cargo.toml project_git_workflow -- --nocapture
cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pull_request -- --nocapture
just desktop-tauri-test
pnpm -C desktop test
pnpm -C desktop build:e2e
pnpm -C desktop exec playwright test tests/e2e/project-pr-review.spec.ts --project=smoke
pnpm -C desktop test:e2e
git diff --check
```

Expected: PASS.

**Step 9: Commit the host router and shared publication tail**

Run:

```bash
git add desktop/src-tauri/src/commands/project_git_workflow.rs \
  desktop/src/shared/api/projectGit.ts \
  desktop/src/features/projects/pullRequestMutations.ts \
  desktop/src/testing/e2eBridge.ts \
  desktop/tests/e2e/project-pr-review.spec.ts
git diff --cached --check
git commit -s -m "feat: route GitHub-backed PR merges through GitHub" \
  -m "Co-authored-by: Kevin Le <anhle12892@gmail.com>"
git log -1 --format=full
```

---

### Task 6: Render safe persistent GitHub recovery UX

**Files:**

- Modify: `desktop/src/shared/api/projectGit.ts`
- Modify: `desktop/src/shared/api/projectGitMergeError.test.mjs`
- Modify: `desktop/src/features/projects/ui/MergePullRequestButton.tsx`
- Modify: `desktop/src/testing/e2eBridge.ts`
- Modify: `desktop/tests/e2e/project-pr-review.spec.ts`

**Acceptance criteria:**

- The frontend accepts only exact GitHub PR or repository PR-list recovery URLs.
- Existing Terminal recovery remains unchanged.
- GitHub errors persist below the Merge action until success, retry, PR change, or dismissal according to the component's existing lifecycle.
- Missing CLI, missing auth, blocked gates, changed head, ambiguous PRs, and generic merge failure each give actionable copy.
- Open GitHub uses the installed opener plugin only after parser validation.
- Retry repeats the merge mutation; Publish merged status retains its current distinct behavior.
- All controls remain keyboard-accessible and have visible focus and unambiguous accessible names.

**Step 1: Write TypeScript parser tests first**

Extend `projectGitMergeError.test.mjs` with:

```js
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
```

Assert both parse.
Reject `http`, subdomains, userinfo, ports, query, fragment, trailing slash, extra segments, nonnumeric PR numbers, encoded slashes, more than 20 reasons, and reasons longer than 200 characters.
Assert `open_terminal` still parses to the same camel-case field names.

Run:

```bash
node --test desktop/src/shared/api/projectGitMergeError.test.mjs
```

Expected: FAIL before the parser change.

**Step 2: Implement the discriminated recovery union**

Use:

```ts
export type ProjectPullRequestMergeRecovery =
  | {
      action: "open_terminal";
      targetBranch: string;
      sourceBranch: string;
    }
  | {
      action: "open_url";
      url: string;
      reasons: string[];
    };
```

Add one private validator:

```ts
function isSafeGitHubRecoveryUrl(raw: string): boolean {
  try {
    if (!raw.startsWith("https://github.com/") || raw.endsWith("/")) {
      return false;
    }
    const url = new URL(raw);
    if (
      url.protocol !== "https:" ||
      url.hostname !== "github.com" ||
      url.port !== "" ||
      url.username !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== ""
    ) {
      return false;
    }
    const segments = url.pathname.split("/").filter(Boolean);
    return (
      segments.length === 3 && segments[2] === "pulls"
    ) || (
      segments.length === 4 &&
      segments[2] === "pull" &&
      /^[1-9]\d*$/.test(segments[3])
    );
  } catch {
    return false;
  }
}
```

Before accepting, also validate owner/repo segments with the same ASCII character rules as Rust and reject any raw `%` in `url.pathname`.
Bound reasons even though Rust already does so.
Do not export the URL validator or create a URL utility module.

**Step 3: Store one actionable GitHub error in the existing component**

In `MergePullRequestButton.tsx`, add:

```ts
const [githubErrorState, setGitHubErrorState] = React.useState<{
  pullRequestId: string;
  error: ProjectPullRequestMergeError;
} | null>(null);
const githubError =
  githubErrorState?.pullRequestId === pullRequest.id
    ? githubErrorState.error
    : null;
const githubRecovery =
  githubError?.recovery?.action === "open_url"
    ? githubError.recovery
    : null;
```

The pull-request ID key prevents one PR's error from rendering on another PR.
Clear the state after a successful merge and when retry begins.
In the mutation catch, require `error.recovery?.action === "open_terminal"` before setting the existing conflict state.
Change `conflictRecoveryState.recovery` to `Extract<ProjectPullRequestMergeRecovery, { action: "open_terminal" }>` so the existing branch fields stay type-safe.
Store only codes beginning with `github_` through `setGitHubErrorState({ pullRequestId: pullRequest.id, error })`.
Continue sending unexpected errors through the existing toast path.

Use `useProjectRepoHost(project)` for host-specific confirmation copy.
Do not add another clone-URL parser to the component.

**Step 4: Implement the persistent card in the existing component**

Render a semantic status region below the merge action:

```tsx
{githubError ? (
  <section
    aria-labelledby="github-merge-recovery-title"
    className="rounded-md border border-border bg-muted/40 p-3"
  >
    <h3 id="github-merge-recovery-title" className="text-sm font-medium">
      {githubMergeErrorTitle(githubError.code)}
    </h3>
    <p className="mt-1 text-sm text-muted-foreground">
      {githubError.message}
    </p>
    {githubRecovery && githubRecovery.reasons.length > 0 ? (
      <ul className="mt-2 list-disc space-y-1 pl-5 text-sm">
        {githubRecovery.reasons.map((reason) => (
          <li key={reason}>{reason}</li>
        ))}
      </ul>
    ) : null}
    <div className="mt-3 flex flex-wrap gap-2">
      <Button type="button" variant="outline" onClick={() => void handleMerge()}>
        Retry
      </Button>
      {githubRecovery ? (
        <Button
          type="button"
          variant="outline"
          onClick={() => void openUrl(githubRecovery.url)}
        >
          Open GitHub PR
        </Button>
      ) : null}
    </div>
  </section>
) : null}
```

Use the repository's existing `Button` import and class conventions.
Do not copy this into a new component unless the source file would exceed 1,000 lines.

For `github_auth_required`, include a read-only code row containing exactly `gh auth login --hostname github.com` and an accessible Copy button using the existing clipboard helper.
For `github_cli_missing`, link only to the already-approved GitHub CLI install documentation if an external URL is already used elsewhere; otherwise show install guidance without introducing a second external domain recovery contract.

**Step 5: Make confirmation copy host-specific**

For a GitHub host, use exactly:

```text
Buzz will find or create the matching GitHub pull request, verify its checks, reviews, and branch rules, then merge it only if GitHub allows an immediate merge.
```

Keep the existing local merge and push copy for Buzz-hosted repositories.

**Step 6: Extend the E2E bridge and tests**

Extend `__BUZZ_E2E_PROJECT_MERGE_ERROR__` recovery to the same discriminated union.
Use the existing opener bridge and `get_e2e_opened_external_urls`; do not add a mock-only opener.

Cover:

1. Missing CLI shows a persistent install card and Retry.
2. Missing auth shows the exact copyable command and Retry.
3. Blocked recovery lists reasons, opens the exact PR URL, and Retry invokes merge again.
4. Branch changed tells the user to refresh and does not offer a stale merge action.
5. Ambiguous recovery opens the exact repository `/pulls` URL.
6. Invalid `open_url` payload never renders an Open button.
7. Existing Buzz conflict still renders Resolve in Terminal.
8. GitHub confirmation copy appears only for GitHub-hosted projects.

Update the original Task 1 test from expecting the old relay error to expecting the auth card.
Retain an assertion that the old relay-validator message is absent.

**Step 7: Add light and dark visual captures**

For the blocked-gates card, loop over the repository's two standard themes:

```ts
for (const theme of ["buzz", "buzz-dark"] as const) {
  test(`renders GitHub merge recovery in ${theme}`, async ({ page }) => {
    await page.addInitScript((selectedTheme) => {
      window.localStorage.setItem("buzz-theme", selectedTheme);
      window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
        "https://github.com/anhle128/buzz";
      window.__BUZZ_E2E_PROJECT_MERGE_ERROR__ = {
        code: "github_pr_blocked",
        message: "GitHub does not allow an immediate merge yet.",
        recovery: {
          action: "open_url",
          url: "https://github.com/anhle128/buzz/pull/42",
          reasons: ["Required check desktop-ci is pending."],
        },
      };
    }, theme);
    await enableProjectsFeature(page);
    await installMockBridge(page);
    await openBuzzProject(page);
    await page.getByRole("tab", { name: "Pull Request" }).click();
    const aliceRow = page
      .getByTestId("project-pull-request-row")
      .filter({ hasText: "alice" })
      .first();
    await aliceRow.getByRole("button", { name: /^#/ }).click();
    await page.getByRole("button", { name: "Merge", exact: true }).click();
    await page.getByTestId("merge-pull-request-confirm-button").click();
    await expect(page.getByText("Required check desktop-ci is pending."))
      .toBeVisible();
    await page.screenshot({
      path: `test-results/github-merge/${theme}-blocked.png`,
      fullPage: true,
    });
  });
}
```

Keep the project-opening sequence identical to the neighboring PR review tests.
Inspect both captures for truncation, low contrast, overflow, uneven spacing, and ambiguous buttons.
Fix visible defects in scope before moving on.

**Step 8: Run focused and full frontend tests**

Run:

```bash
node --test desktop/src/shared/api/projectGitMergeError.test.mjs
pnpm -C desktop test
pnpm -C desktop build:e2e
pnpm -C desktop exec playwright test tests/e2e/project-pr-review.spec.ts --project=smoke
pnpm -C desktop test:e2e
git diff --check
```

Expected: PASS.

**Step 9: Commit the recovery UX**

Run:

```bash
git add desktop/src/shared/api/projectGit.ts \
  desktop/src/shared/api/projectGitMergeError.test.mjs \
  desktop/src/features/projects/ui/MergePullRequestButton.tsx \
  desktop/src/testing/e2eBridge.ts \
  desktop/tests/e2e/project-pr-review.spec.ts
git diff --cached --check
git commit -s -m "feat: add GitHub merge recovery guidance" \
  -m "Co-authored-by: Kevin Le <anhle12892@gmail.com>"
git log -1 --format=full
```

---

### Task 7: Prove idempotency, non-regression, and release readiness

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_pull_request/tests.rs`
- Modify: `desktop/tests/e2e/project-pr-review.spec.ts`
- Do not modify: `CHANGELOG.md`

**Acceptance criteria:**

- Every approved design requirement maps to a runnable test or an explicit authorized live check.
- The full Rust, TypeScript, Playwright, and repository CI gates pass at one recorded HEAD.
- Buzz-hosted merge, conflict recovery, ownership, and Nostr partial-publication paths remain green.
- No token-like value appears in UI snapshots, process diagnostics, captured argv, or committed fixtures.
- Optional live smoke changes only a disposable, explicitly authorized GitHub branch and PR.

**Step 1: Build a design-to-test checklist before adding tests**

Use `rg` to map every Definition of Done bullet from the approved spec to an existing test name.
Add only missing checks.
At minimum, prove:

- Create timeout followed by retry finds the existing PR without another POST.
- Already-merged retry skips PUT and returns the existing merge commit.
- Nostr publication retry skips the GitHub command entirely.
- Changed head skips PUT and status publication.
- Merge queue skips PUT.
- Required-check exit code `8` is parsed as pending rather than a process failure.
- No-required-checks is treated as an empty passing set.
- Mixed hosts and malformed GitHub URLs cannot reach either merge implementation.
- Existing Buzz native merge and conflict recovery still pass unchanged.

**Step 2: Add one GitHub success E2E**

With the GitHub clone override and no injected merge error, exercise the current mock success path.
Assert:

- GitHub confirmation copy.
- One `merge_project_pull_request` invoke.
- Merged UI state.
- One Nostr kind `1631` event with the returned `merge-commit` tag.
- No persistent error card.

Use existing E2E bridge event inspection rather than adding a new global.

**Step 3: Add one GitHub partial-publication E2E**

Reuse the existing relay publication-failure injection.
Assert the first action reaches merged GitHub result and changes the button to Publish merged status.
Click Publish merged status and assert:

- The saved event is published.
- The merge invoke count remains one.
- The UI clears the publication error.

Do not call `merge_project_pull_request` on the second action.

**Step 4: Scan the implementation for forbidden paths and leaked diagnostics**

Run:

```bash
rg -n "gh pr merge|--admin|--auto|merge_queue.*(enable|enqueue)|GITHUB_TOKEN|GH_TOKEN|Authorization:" \
  desktop/src-tauri/src/commands/project_github_pull_request.rs \
  desktop/src-tauri/src/commands/project_github_pull_request/tests.rs \
  desktop/src/features/projects/ui/MergePullRequestButton.tsx \
  desktop/tests/e2e/project-pr-review.spec.ts
```

Expected:

- No production call to `gh pr merge`, `--admin`, `--auto`, queue enrollment, or token environment handling.
- Token strings occur only in redaction tests, never expected output snapshots.
- `merge_queue` occurs only in detection and refusal code/tests.

**Step 5: Run all package suites at one HEAD**

Run:

```bash
pnpm -C desktop test
just desktop-tauri-test
pnpm -C desktop build:e2e
pnpm -C desktop test:e2e
git diff --check
git rev-parse HEAD
```

Record the returned HEAD with the results.
If the worktree changes while tests run, restart the affected suite at the new HEAD before making any claim.

**Step 6: Run repository-wide CI**

Run:

```bash
just ci
git diff --check
git status --short
git rev-parse HEAD
```

Expected: all commands pass and only intentional test updates, if any, remain.

If an unrelated lint, test, or flaky failure appears, fix it only when it is safely understood and does not widen product scope.
Otherwise document the exact command, failure, and searched scope before asking Kevin for direction.

**Step 7: Inspect the visual artifacts**

Open and inspect:

```text
desktop/test-results/github-merge/buzz-blocked.png
desktop/test-results/github-merge/buzz-dark-blocked.png
```

Verify readable contrast, complete reason text, no horizontal clipping, stable card position, visible focus rings, and distinct Retry/Open actions.
Do not commit generated screenshots unless the repository's neighboring screenshot tests already version those artifacts.

**Step 8: Optionally run a disposable live smoke test only after explicit authorization**

The smoke target must be a permitted repository and disposable branch supplied or approved by Kevin.
Create one test branch and PR, verify Buzz finds or creates exactly one PR, then merge only if checks and rules allow an immediate merge.
Delete or close only resources created for the smoke test.
Never use the active user PR from the original report.

If no authorization is provided, record the live smoke as not run; this does not block automated completion.

**Step 9: Commit any final missing tests**

Skip this commit when Steps 1–7 required no source changes.
Otherwise run:

```bash
git add desktop/src-tauri/src/commands/project_github_pull_request/tests.rs \
  desktop/tests/e2e/project-pr-review.spec.ts
git diff --cached --check
git commit -s -m "test: verify GitHub PR merge recovery paths" \
  -m "Co-authored-by: Kevin Le <anhle12892@gmail.com>"
git log -1 --format=full
```

Re-run every full suite from Steps 5 and 6 after this commit and record the new HEAD.

## Final Self-Review Checklist

- Compare the final diff against every Goal, Non-Goal, trust-boundary rule, state, error code, and Definition of Done item in the approved spec.
- Confirm the only production GitHub process entry is the focused Rust module.
- Confirm every caller of the former private GitHub URL validator uses the shared parser.
- Confirm no Nostr owner/GitHub owner comparison remains in the GitHub route.
- Confirm only the verified merge outcome reaches `build_merged_status_event`.
- Confirm all recovery URLs pass the frontend exact-path validator before `openUrl`.
- Confirm raw `gh` stdout, stderr, request bodies, environment, and tokens never cross the Tauri boundary.
- Confirm Task 1's original end-user reproduction now passes with GitHub behavior and explicitly proves the old relay error is absent.
- Confirm `git status --short` contains no generated screenshots, temporary request bodies, debug output, or unrelated user changes.
- Confirm the final commit has both required Kevin trailers and no agent co-author trailer.

## Handoff

When every required automated gate passes, report:

- Final HEAD and commit list.
- Files changed by task.
- Exact full-suite commands and results tied to that HEAD.
- Whether the optional live GitHub smoke test ran and which disposable resources it used.
- The two light/dark screenshot paths and the visual inspection result.
- Any deliberate simplification retained by this plan, including the single `github.com` host, fixed merge-commit method, and absence of auto-merge or queue enrollment.
