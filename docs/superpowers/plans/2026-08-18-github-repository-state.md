# GitHub repository state Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** For `github.com` clone URLs, load `RepoState` (branch list + default branch) via `gh api` so the Projects picker selects GitHub’s default (for example `develop`) instead of falling back to `main`.

**Architecture:** Make `fetchRepoState` host-aware. GitHub-hosted repos call a new Tauri command that uses `GhRunner` (`gh api` + `ensure_auth`). Buzz-hosted repos still read kind:30618. GitHub `RepoState` replaces 30618; do not merge the two. On GitHub fetch failure, do not feed the announcement default (`"main"`) into the picker.

**Tech Stack:** Tauri 2 (desktop crate), `gh` CLI, React Query, existing `RepoState` / `ProjectPullRequestMergeError` wire format.

**Spec:** [2026-08-18-github-repository-state-design.md](../specs/2026-08-18-github-repository-state-design.md)

## Global Constraints

- G1 + G2 only: no snapshot (G3), fetch/pull/push (G4–G5), create/delete branch (G6), or G7 copy fix.
- Use `gh api` only. Do not call `git ls-remote`.
- Auth: `GhRunner::discover` then `ensure_auth`. Never store a GitHub token.
- Error codes from this command: only `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, `github_state_failed`. Remap `github_merge_failed` from `ensure_auth` / `run` / `run_json` before returning.
- HTTP 404, or 403 that is not rate/abuse → `github_repo_unavailable`. Rate/abuse 403, timeout, truncated JSON → `github_state_failed`.
- Never invent a successful `RepoState` on failure. On GitHub error, `defaultBranch` is `null` and `observedBranches` is `[]` (no silent `main`).
- GitHub `tags` is always `[]` in this slice.
- Buzz `/git/<64-hex>/<id>` path is unchanged (kind:30618 only). Recovery copy is GitHub-only when `isGitHubCloneUrl`.
- Branch list: `gh api --paginate --slurp` (deserialize `Vec<Vec<_>>`, flatten) **or** explicit `page=N` loops. Never deserialize `--paginate` without `--slurp`.
- Do not send the full branch dump through `GH_STREAM_LIMIT` (64 KiB). Prefer per-page fetches (`per_page=100&page=N`) so each stdout is small.
- Activate Hermit in **every** shell: `. ./bin/activate-hermit && …`. CWD does not persist.
- Commits use `git commit -s`. Before each commit run `node .gitnexus/run.cjs` detect path if available; do not skip DCO.

## Codex P1s folded into this revision

1. Expose `discover`, `ensure_auth`, `run`, `run_json`, and `GhOutput` as `pub(crate)`, not only the struct / `from_resolved` / `slug`.
2. `--paginate --slurp` or page loops.
3. Fake-`gh` match `/branches` **before** the repo root path.
4. GitHub query error must not resolve to announced `"main"`.
5. Render recovery in `ProjectWorkspaceTabs.tsx`, `ProjectReadmePanel.tsx`, `ProjectRepositoryPanel.tsx`.
6. Host-gate recovery so Buzz 30618 errors stay generic.
7. Remap every `github_merge_failed` leaving this command.
8. Avoid 64 KiB truncation (page loops).
9. Mock-bridge smoke: picker shows `develop`; auth error shows recovery, not `main`.

---

## File map

| File | Role |
|------|------|
| `desktop/src-tauri/src/commands/project_github_pull_request.rs` | `pub(crate)` on `GhRunner`, `GhOutput`, `discover`, `from_resolved`, `ensure_auth`, `run`, `run_json`, `slug` |
| Create: `desktop/src-tauri/src/commands/project_github_repository_state.rs` | Parse URL, page branches, remap errors, Tauri command |
| `desktop/src-tauri/src/commands/project_git_merge_error.rs` | Keep `code` private; tests assert via `serde_json` like existing merge tests |
| `desktop/src-tauri/src/commands/mod.rs` | `mod` + `pub use` |
| `desktop/src-tauri/src/lib.rs` | Register `get_github_repository_state` |
| `desktop/src/features/projects/lib/projectRepoState.ts` | Move `RepoState` here. Host-aware `fetchRepoState`. |
| `desktop/src/features/projects/hooks.ts` | Re-export `RepoState`. `useRepoStateQuery` key includes clone URL. |
| `desktop/src/shared/api/projectGit.ts` | `getGithubRepositoryState` |
| `desktop/src/features/projects/lib/projectGitError.ts` | Export `isGitHubCloneUrl` |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Null default + empty branches on GitHub error; pass recovery props |
| `desktop/src/features/projects/ui/GitHubRepoStateRecovery.tsx` | Recovery UI |
| `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx` | Render recovery under branch row |
| `desktop/src/features/projects/ui/ProjectReadmePanel.tsx` | Same |
| `desktop/src/features/projects/ui/ProjectRepositoryPanel.tsx` | Same |
| `desktop/src/features/projects/ui/ProjectRepositorySource.tsx` | Add `stateError` / `onRetryState` / `showGithubStateRecovery` on `RepoSourceHeaderControls` |
| `desktop/src/testing/e2eBridge.ts` | Stub success + optional error |
| Create: `desktop/tests/e2e/github-repo-state.spec.ts` | Smoke: `develop` selected; auth error recovery |

---

### Task 1: Map `gh api` JSON to repository state (Rust)

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_github_pull_request.rs`
- Create: `desktop/src-tauri/src/commands/project_github_repository_state.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs` (`mod project_github_repository_state;`)

**Interfaces:**
- Consumes: `pub(crate)` `GhRunner::{discover, from_resolved, ensure_auth, run, run_json}`, `GhOutput`, `GitHubRepoRef::{parse, slug}`
- Produces:
  - `pub struct GitHubRepositoryState { pub head: String, pub branches: Vec<GitHubRepositoryBranch>, pub tags: Vec<GitHubRepositoryBranch>, pub updated_at: i64 }`
  - `pub struct GitHubRepositoryBranch { pub name: String, pub commit: String }`
  - `pub(crate) fn github_repository_state_with(gh: &GhRunner, clone_url: &str) -> Result<GitHubRepositoryState, ProjectPullRequestMergeError>`
  - `fn remap_state_error(error: ProjectPullRequestMergeError, stderr: &str) -> ProjectPullRequestMergeError`

- [ ] **Step 1: Make runner API visible to a sibling module**

Change in `project_github_pull_request.rs`:

```rust
pub(crate) struct GhRunner { /* unchanged fields */ }
pub(crate) struct GhOutput { /* unchanged fields */ }

impl GhRunner {
    pub(crate) fn discover() -> Result<Self, ProjectPullRequestMergeError> { ... }
    pub(crate) fn from_resolved(binary: Option<PathBuf>) -> Result<Self, ...> { ... }
    pub(crate) fn ensure_auth(&self) -> Result<(), ...> { ... }
    pub(crate) fn run_json<T: DeserializeOwned>(...) -> Result<T, ...> { ... }
    pub(crate) fn run(&self, args: &[OsString]) -> Result<GhOutput, ...> { ... }
}

impl GitHubRepoRef {
    pub(crate) fn slug(&self) -> String { ... }
}
```

Do not add `code()`. Tests deserialize the error with `serde_json` like `project_github_pull_request/tests.rs`.

- [ ] **Step 2: Write failing tests in `project_github_repository_state.rs`**

Unix `fake_gh` helper copied from `runner_tests.rs`. **Match `/branches` before the repo path.**

```rust
#[cfg(unix)]
fn fake_gh(script: &str) -> (tempfile::TempDir, PathBuf) { /* same chmod 0700 helper */ }

#[cfg(unix)]
#[test]
fn maps_repo_and_branch_payloads_to_develop_head() {
    let develop = "d".repeat(40);
    let main = "m".repeat(40);
    let script = format!(
        r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/branches"*)
    printf '%s' '[{{"name":"develop","commit":{{"sha":"{develop}"}}}},{{"name":"main","commit":{{"sha":"{main}"}}}}]'
    ;;
  *"/repos/acme/app"*)
    printf '%s' '{{"default_branch":"develop"}}'
    ;;
  *) exit 1 ;;
esac
"#
    );
    let (_dir, path) = fake_gh(&script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let state = github_repository_state_with(&gh, "https://github.com/acme/app").expect("state");
    assert_eq!(state.head, "develop");
    assert_eq!(state.tags.len(), 0);
    assert_eq!(state.branches.len(), 2);
}

#[test]
fn rejects_non_github_clone_url_before_runner() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
        .expect("dummy runner unused");
    let err = github_repository_state_with(&gh, "https://gitlab.com/acme/app").expect_err("gitlab");
    let value = serde_json::to_value(err).expect("json");
    assert_eq!(value["code"], "github_state_failed");
}

#[cfg(unix)]
#[test]
fn missing_gh_binary_is_cli_missing() {
    let err = GhRunner::from_resolved(None).expect_err("missing");
    let value = serde_json::to_value(err).expect("json");
    assert_eq!(value["code"], "github_cli_missing");
}
```

- [ ] **Step 3: Run tests — expect compile fail**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml maps_repo_and_branch_payloads_to_develop_head
```

Expected: `github_repository_state_with` missing.

- [ ] **Step 4: Implement `github_repository_state_with`**

```rust
pub(crate) fn github_repository_state_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<GitHubRepositoryState, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_state_failed", message))?;
    gh.ensure_auth().map_err(|error| remap_state_error(error, ""))?;
    let repo_json: GitHubRepoPayload = get_json(gh, &format!("/repos/{}", repo.slug()))?;
    let head = repo_json.default_branch.trim().to_string();
    if head.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_state_failed",
            "GitHub did not return a default branch.",
        ));
    }
    let branches = list_branch_pages(gh, &repo.slug())?;
    Ok(GitHubRepositoryState {
        head,
        branches,
        tags: vec![],
        updated_at: unix_now(),
    })
}
```

`list_branch_pages`: loop `page = 1..` with

```text
gh api --hostname github.com --method GET /repos/{slug}/branches?per_page=100&page={n}
```

Deserialize each page as `Vec<GitHubBranchPayload>`. Stop when the page is empty or shorter than 100. Do **not** use `--paginate` without `--slurp`. If you use `--paginate --slurp`, type is `Vec<Vec<GitHubBranchPayload>>` then flatten.

`get_json` / `list_branch_pages` call `gh.run` (not `run_json`) so you have `stderr` for remapping:

```rust
fn remap_state_error(
    error: ProjectPullRequestMergeError,
    stderr: &str,
) -> ProjectPullRequestMergeError {
    let value = serde_json::to_value(&error).unwrap_or_default();
    let code = value.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let blob = format!("{stderr} {}", value.get("message").and_then(|v| v.as_str()).unwrap_or(""));
    let lower = blob.to_ascii_lowercase();
    if code == "github_cli_missing" || code == "github_auth_required" {
        return error;
    }
    if lower.contains("rate limit") || lower.contains("abuse");
    {
        return ProjectPullRequestMergeError::new("github_state_failed", redact(stderr));
    }
    if lower.contains("404") || lower.contains("not found")
        || (lower.contains("403") && !lower.contains("rate"))
    {
        return ProjectPullRequestMergeError::new("github_repo_unavailable", redact(stderr));
    }
    ProjectPullRequestMergeError::new("github_state_failed", redact(stderr))
}
```

Fix the stray semicolon after `abuse` when implementing (that is a plan typo — write valid Rust).

Every return path of this module must be one of the four allowed codes. `github_merge_failed` must not leak.

- [ ] **Step 5: Run tests**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_repository_state
```

Expected: PASS on unix. Windows: `#[cfg(unix)]` on fake-gh tests; keep the gitlab URL test everywhere.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_pull_request.rs \
  desktop/src-tauri/src/commands/project_github_repository_state.rs \
  desktop/src-tauri/src/commands/mod.rs
git commit -s -m "feat(projects): map GitHub repo state from gh api"
```

---

### Task 2: Tauri command + register

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_github_repository_state.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs` (`pub use project_github_repository_state::*;`)
- Modify: `desktop/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `github_repository_state_with`, `GhRunner::discover`
- Produces: `#[tauri::command] pub async fn get_github_repository_state(clone_url: String) -> Result<GitHubRepositoryState, ProjectPullRequestMergeError>`

- [ ] **Step 1: Implement the command**

```rust
pub(crate) fn get_github_repository_state_with_runner(
    clone_url: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubRepositoryState, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_state_error(error, ""))?;
    github_repository_state_with(&gh, &clone_url)
}

#[tauri::command]
pub async fn get_github_repository_state(
    clone_url: String,
) -> Result<GitHubRepositoryState, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        get_github_repository_state_with_runner(clone_url, GhRunner::discover())
    })
    .await
    .map_err(|error| {
        ProjectPullRequestMergeError::new("github_state_failed", error.to_string())
    })?
}
```

Register `get_github_repository_state` next to `get_project_repo_snapshot` in `lib.rs`.

- [ ] **Step 2: Test missing CLI through the wrapper**

```rust
#[test]
fn wrapper_maps_discover_failure() {
    let err = get_github_repository_state_with_runner(
        "https://github.com/acme/app".into(),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    let value = serde_json::to_value(err).expect("json");
    assert_eq!(value["code"], "github_cli_missing");
}
```

- [ ] **Step 3: Run**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_repository_state
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_repository_state.rs \
  desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/lib.rs
git commit -s -m "feat(projects): expose get_github_repository_state command"
```

---

### Task 3: TypeScript wrapper + host-aware `fetchRepoState` (no silent `main`)

**Files:**
- Create: `desktop/src/features/projects/lib/projectRepoState.ts` — **move `RepoState` here first**
- Modify: `desktop/src/features/projects/hooks.ts` — `export type { RepoState } from "./lib/projectRepoState"`; `useRepoStateQuery` uses new fetcher
- Modify: `desktop/src/shared/api/projectGit.ts`
- Modify: `desktop/src/features/projects/lib/projectGitError.ts` — export `isGitHubCloneUrl`
- Create: `desktop/src/features/projects/lib/projectRepoState.test.mjs`
- Modify: `desktop/src/features/projects/lib/projectBranches.test.mjs`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`

**Interfaces:**
- Consumes: `invokeTauri`, `parseProjectPullRequestMergeError`, `isGitHubCloneUrl`
- Produces:
  - `export type RepoState`
  - `export async function getGithubRepositoryState(cloneUrl: string): Promise<RepoState>`
  - `export async function fetchRepoState(project: Repository): Promise<RepoState | null>`
  - `export async function fetchRepoStateWith(...)`

- [ ] **Step 1: Move `RepoState` to `projectRepoState.ts` and re-export from `hooks.ts` so existing imports keep compiling.**

- [ ] **Step 2: Routing tests**

```js
test("uses GitHub state for github.com clone URLs", async () => { /* loadGithub called; loadBuzz must not run */ });
test("uses kind 30618 for Buzz-hosted clone URLs", async () => { /* loadGithub must not run */ });
test("isGitHubCloneUrl accepts https and ssh github hosts", () => {
  assert.equal(isGitHubCloneUrl("https://github.com/acme/app"), true);
  assert.equal(isGitHubCloneUrl("git@github.com:acme/app.git"), true);
  assert.equal(isGitHubCloneUrl("https://relay.example/git/" + "ab".repeat(32) + "/app"), false);
});
```

Gate on `isGitHubCloneUrl(project.cloneUrls[0])`. Do **not** use `projectRepoHost(..., relayOrigin)` (missing origin marks GitHub `unresolved`).

- [ ] **Step 3: `getGithubRepositoryState` + `useRepoStateQuery` key**

```ts
export async function getGithubRepositoryState(cloneUrl: string): Promise<RepoState> {
  try {
    const raw = await invokeTauri<{
      head: string;
      branches: Array<{ name: string; commit: string }>;
      tags: Array<{ name: string; commit: string }>;
      updated_at: number;
    }>("get_github_repository_state", { cloneUrl });
    return {
      head: raw.head,
      branches: raw.branches,
      tags: raw.tags ?? [],
      updatedAt: raw.updated_at,
    };
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}
```

Query key **must** include host + clone URL:

```ts
queryKey: [
  "project",
  project?.id ?? "none",
  "repo-state",
  project?.cloneUrls[0] ?? "no-clone",
]
```

- [ ] **Step 4: Kill silent `main` in `ProjectDetailScreen`**

```ts
const githubHosted = isGitHubCloneUrl(repository?.cloneUrls[0]);
const githubStateFailed = githubHosted && repoStateQuery.isError;
const defaultBranch =
  !repository || githubStateFailed
    ? null
    : resolveProjectDefaultBranch(
        repository.defaultBranch,
        repoStateQuery.data,
      );
const { branchOptions, ... } = useOptimisticProjectBranches({
  defaultBranch,
  observedBranches: githubStateFailed ? [] : (repoStateQuery.data?.branches ?? []),
  projectId: repository?.id ?? projectId,
  referencedBranches: githubStateFailed
    ? []
    : (pullRequestsQuery.data?.map((pr) => pr.branchName ?? null) ?? []),
});
```

Add a unit test in `projectBranches.test.mjs` that `resolveProjectDefaultBranch("main", { head: "develop", branches: [...] }) === "develop"`.

- [ ] **Step 5: Run**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test -- src/features/projects/lib/projectRepoState.test.mjs src/features/projects/lib/projectBranches.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/features/projects/lib/projectRepoState.ts \
  desktop/src/features/projects/lib/projectRepoState.test.mjs \
  desktop/src/features/projects/lib/projectGitError.ts \
  desktop/src/features/projects/lib/projectBranches.test.mjs \
  desktop/src/features/projects/hooks.ts \
  desktop/src/features/projects/ui/ProjectDetailScreen.tsx \
  desktop/src/shared/api/projectGit.ts
git commit -s -m "feat(projects): load GitHub repo state instead of kind 30618"
```

---

### Task 4: Recovery UI on the real header rows

**Files:**
- Create: `desktop/src/features/projects/ui/GitHubRepoStateRecovery.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectRepositorySource.tsx` — extend `RepoSourceHeaderControls` with `stateError?: unknown`, `onRetryState?: () => void`, `showGithubStateRecovery?: boolean`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx` — after the branch dropdown row, if `showGithubStateRecovery`, render `<GitHubRepoStateRecovery error={stateError} onRetry={onRetryState} />`
- Modify: `desktop/src/features/projects/ui/ProjectReadmePanel.tsx` — same
- Modify: `desktop/src/features/projects/ui/ProjectRepositoryPanel.tsx` — both header sites that render `RepositoryBranchDropdown`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` — set the three new control fields

**Interfaces:**
- Consumes: `parseProjectPullRequestMergeError`, `copyTextToClipboard`
- Produces: visible recovery under every header that already shows the branch picker

- [ ] **Step 1: Recovery component**

Copy strings from `MergePullRequestButton` (do not import that file):

- `github_cli_missing` → title `GitHub CLI is required`; line `Install GitHub CLI, then retry.`
- `github_auth_required` → title `GitHub authentication required`; `gh auth login --hostname github.com` + Copy
- `github_repo_unavailable` / `github_state_failed` → title `Could not load GitHub branches`; body = message
- Always Retry

- [ ] **Step 2: Wire only for GitHub**

```ts
showGithubStateRecovery:
  isGitHubCloneUrl(repository?.cloneUrls[0]) && repoStateQuery.isError,
stateError: repoStateQuery.error,
onRetryState: () => { void repoStateQuery.refetch(); },
```

If `showGithubStateRecovery` is false, render nothing. Buzz 30618 errors keep existing behavior.

- [ ] **Step 3: Typecheck**

```bash
. ./bin/activate-hermit && cd desktop && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/features/projects/ui/GitHubRepoStateRecovery.tsx \
  desktop/src/features/projects/ui/ProjectRepositorySource.tsx \
  desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx \
  desktop/src/features/projects/ui/ProjectReadmePanel.tsx \
  desktop/src/features/projects/ui/ProjectRepositoryPanel.tsx \
  desktop/src/features/projects/ui/ProjectDetailScreen.tsx
git commit -s -m "feat(projects): show GitHub CLI recovery on repo state errors"
```

---

### Task 5: e2e mock + smoke (user-visible)

**Files:**
- Modify: `desktop/src/testing/e2eBridge.ts`
- Create: `desktop/tests/e2e/github-repo-state.spec.ts`
- Modify: `desktop/playwright.config.ts` only if the new spec must be registered in the `smoke` `testMatch`

**Interfaces:**
- Stub returns `head: "develop"` plus `develop`/`main` branches
- Optional invoke flag or second path throws `{ code: "github_auth_required", message: "Authenticate GitHub CLI with: gh auth login --hostname github.com" }`

- [ ] **Step 1: Stub**

```ts
case "get_github_repository_state":
  return {
    head: "develop",
    branches: [
      { name: "develop", commit: "d".repeat(40) },
      { name: "main", commit: "m".repeat(40) },
    ],
    tags: [],
    updated_at: Math.floor(Date.now() / 1000),
  };
```

If existing smoke projects are Buzz-hosted, this stub is unused until the spec below sets a GitHub clone URL in mock state. Follow `installMockBridge` + `addInitScript` **before** the bridge (AGENTS.md). Build with `pnpm build:e2e`, never `pnpm run build`.

- [ ] **Step 2: Smoke spec**

One file, two tests, mock bridge only:

1. Given a project whose clone URL is `https://github.com/acme/app` and the stub above: open the project, wait for animations, assert the branch trigger contains `develop` and the open menu lists `develop` and `main`.
2. Given the same project but the mock command rejects with `github_auth_required`: assert the page shows `GitHub authentication required` and does **not** show the branch trigger as `main`.

Reuse helpers from `desktop/tests/e2e/` (`installMockBridge`, `waitForAnimations`). Seed clone URL the same way other project specs seed repository announcements. If that seed path does not exist, add the minimum mock repository field used by `eventToRepository.cloneUrls`.

- [ ] **Step 3: Run smoke spec**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test:e2e:smoke -- github-repo-state
```

Expected: PASS. If `test:e2e:smoke` does not accept a file filter, run the playwright command the repo already uses for a single spec.

- [ ] **Step 4: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/testing/e2eBridge.ts \
  desktop/tests/e2e/github-repo-state.spec.ts \
  desktop/playwright.config.ts
git commit -s -m "test(e2e): cover GitHub default branch and auth recovery"
```

---

## Spec coverage

| Spec / Codex P1 | Task |
|-----------------|------|
| List GitHub branches | 1–3 |
| Select `default_branch` | 1 + 3 |
| `gh api` only, `--slurp` or page loop | 1 |
| Expose full `GhRunner` API | 1 |
| Fake-gh `/branches` first | 1 |
| Remap all `github_merge_failed` | 1–2 |
| 64 KiB / pagination | 1 page loop |
| No silent `main` | 3 |
| Recovery on real headers + host gate | 4 |
| User-visible e2e | 5 |
| Query key includes clone URL | 3 |
| `RepoState` not stuck in `hooks.ts` | 3 |
| 403 rate-limit ≠ unavailable | 1 `remap_state_error` |

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope & strategy | 0 | skipped | — |
| Codex Review | `/codex review` | Independent 2nd opinion | 1 | FAIL then folded | 9 P1 + 5 P2; P1s folded into Tasks 1–5 |
| Eng Review | `/plan-eng-review` | Architecture & tests (required) | 0 | skipped | — |
| Design Review | `/plan-design-review` | UI/UX gaps | 0 | skipped | — |
| DX Review | `/plan-devex-review` | Developer experience gaps | 0 | skipped | — |

- **CODEX:** P1s applied: full `GhRunner` visibility, page loops / `--slurp`, fake-gh order, no announced `main` on error, recovery in WorkspaceTabs/Readme/RepositoryPanel, host-gated copy, remap merge errors, avoid 64KiB dump, mock e2e for `develop` + auth recovery.
- **VERDICT:** Plan revised after Codex FAIL. Ready to implement this revision.

NO UNRESOLVED DECISIONS
