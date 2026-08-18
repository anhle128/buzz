# GitHub Issues (list + create) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a Projects repository clone URL is `github.com`, the per-repo Issues tab lists and creates GitHub Issues via `gh api` and never publishes NIP-34 `kind:1621`.

**Architecture:** Add three Tauri commands that reuse `GhRunner` and `GitHubRepoRef`.
`fetchProjectIssues` and `useCreateProjectIssueMutation` become host-aware: `isGitHubCloneUrl` routes to GitHub only, and Buzz `/git/...` remotes keep kind:1621.
The existing Issues tab and create dialog stay; GitHub rows use numeric `#N`, login identities, read-only labels and assignees, and a separate comments query.

**Tech Stack:** Tauri 2 desktop crate, `gh api`, React 19, TanStack Query, Node `node:test` via desktop `pnpm test`, Playwright mock-bridge smoke.

**Spec:** [2026-08-18-github-issues-design.md](../specs/2026-08-18-github-issues-design.md)

**Product contract:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) make GitHub the native issue backend when the clone URL is `github.com`.
Buzz remains the collaboration layer and the only issue backend for Buzz-hosted remotes.

**Phase doc:** [phase-02-github-issues.md](../../../plans/20260818-1211-github-native-host/phase-02-github-issues.md) slice I1 + I2.

## Global Constraints

- I1 + I2 only: list open issues, create title+body, load comments as read-only.
- Do not implement close/reopen (I3), posting comments (I4), label writes (I5), or assignee writes (I6).
- Do not add an Open | Closed filter, `state=all`, a second page, or Load more.
- Do not change the global Projects Issues list, card/activity counts, CLI, mobile, or `buzz://issue`.
- Do not dual-write.
- Do not import GitHub issues into Nostr.
- Do not call `gh issue`.
- Do not add a provider trait.
- Do not store a GitHub token.
- Auth is installed `gh` plus `gh auth status --hostname github.com`.
- Use `gh api` only.
- `list_github_issues` requires `state` of `open` or `closed`.
- I1+I2 UI always sends `state=open`.
- One page, `per_page=100`, `sort=updated`, `direction=desc`, no `--paginate`.
- Drop any item that has a GitHub `pull_request` field.
- `has_more` is `raw_items.len() == 100` before PR/html_url filtering.
- `number` is a `u64` greater than 0, never a raw unparsed path segment.
- Authors and assignees are GitHub logins + avatar URLs, never pubkeys.
- Never pass a login to `ProfileIdentityButton`, `normalizePubkey`, `useUsersBatchQuery`, or assignment mutations.
- Add `"Open"` to `PROJECT_ISSUE_STATUS`.
- GitHub `open` maps to `"Open"`.
- GitHub `closed` maps to `"Closed"`.
- Do not map open to Backlog.
- Copy link uses a validated `html_url` only.
- Create sends `{ "title", "body" }` only.
- Title is required after trim and at most 256 characters.
- Body may be empty.
- On GitHub create success, invalidate only `["project", id, "issues"]`.
- Do not invalidate `work-items` or `activity-summaries` for GitHub creates.
- Buzz create keeps today's invalidation.
- Query key stays `["project", id, "issues"]` in this slice.
- I3–I6 will add `state` to the GitHub key later.
- Error codes from the three new commands: only `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, `github_issues_failed`.
- Remap every `github_merge_failed` and do not leak `github_state_failed`.
- Never invent an empty issue list on CLI or auth failure.
- Check GitHub errors before the empty-success branch.
- GitHub empty success copy is `No open issues.`
- Do not show `No issues yet` or `Could not load issues for this repository.` on a GitHub CLI/auth/repo failure.
- Buzz empty and Buzz error copy stay unchanged.
- `desktop/src-tauri/src/commands/project_github_pull_request.rs` is 998 lines.
- Do not add functions to that file.
- Only mark `GitHubRepoRef` fields `pub(crate)` there.
- `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` is 953 lines and `hooks.ts` is 921 lines.
- Keep those diffs tiny and put new logic in new files.
- No `unsafe`.
- No new `unwrap()` or `expect()` on production paths.
- New public Rust and TypeScript APIs get doc comments.
- Desktop text uses rem tokens (`text-xs`, `text-2xs`), never `text-[Npx]`.
- Activate Hermit in every shell: `. ./bin/activate-hermit && …`.
- CWD does not persist across tool calls.
- Commits use `git commit -s`.
- Before each commit run `if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi`.
- Before editing a symbol, run GitNexus `impact({target, direction: "upstream"})` when MCP tools are available and warn on HIGH or CRITICAL.
- No live GitHub in unit tests.

## Required Impact Checks

Run these before the task's first edit when GitNexus MCP tools are available.
Report direct callers, affected processes, and risk level before editing.

- Task 1: `GitHubRepoRef` field visibility.
- Task 1: new `project_github_issues` module only after that visibility change.
- Task 3: `get_github_repository_state` registration in `lib.rs` as the neighboring invoke handler.
- Task 4: `eventToProjectIssue`, `PROJECT_ISSUE_STATUS`, and `issueShareLink`.
- Task 5: `fetchProjectIssues`, `useProjectIssuesQuery`, `useCreateProjectIssueMutation`, and `publishProjectIssue`.
- Task 6: `ProjectIssuesPanel`, `ProjectIssueDetail`, `IssueRow`, `ProjectIssueCommentTimeline`, and `GitHubRepoStateRecovery`.
- Task 6: `ProjectDetailScreen` `issuesQuery.data` readers and `peoplePubkeys`.
- Task 7: `maybeInstallE2eTauriMocks` / the mock invoke switch in `desktop/src/testing/e2eBridge.ts`, plus the Playwright smoke `testMatch` list.

## Open Questions

1. **List and comment payload size versus `GH_STREAM_LIMIT` (64 KiB).**
   One hundred GitHub issues with bodies will not fit the default `GhRunner::run` cap.
   **Provisional default:** call `run_with_limit` at `2 * 1024 * 1024` bytes and pass `--jq` that projects only DTO fields plus `has_pull_request: has("pull_request")`.
   Do not silently truncate issue or comment bodies.
   If stdout is truncated JSON, return `github_issues_failed`.

2. **Created issue with an invalid `html_url`.**
   The spec says drop a list item when `html_url` fails the repo-bound check.
   **Provisional default:** create fails with `github_issues_failed` instead of returning an issue the tab cannot share.

3. **Comment-load recovery title.**
   The spec names tab copy `Could not load GitHub issues`.
   **Provisional default:** the list uses that title, and the comment section uses `Could not load GitHub comments` so a comment failure is not mistaken for a list failure.

---

## File map

| File | Role |
|------|------|
| `desktop/src-tauri/src/commands/project_github_pull_request.rs` | Mark `GitHubRepoRef.owner` and `GitHubRepoRef.repo` `pub(crate)` only |
| Create: `desktop/src-tauri/src/commands/project_github_issues.rs` | Parse URL, list/create/comments, remap errors, Tauri commands |
| `desktop/src-tauri/src/commands/mod.rs` | `mod project_github_issues;` and `pub use project_github_issues::*;` |
| `desktop/src-tauri/src/lib.rs` | Register `list_github_issues`, `create_github_issue`, `list_github_issue_comments` next to `get_github_repository_state` |
| `desktop/src/shared/api/projectGit.ts` | `listGithubIssues`, `createGithubIssue`, `listGithubIssueComments` |
| `desktop/src/features/projects/projectIssues.mjs` | `PROJECT_ISSUE_STATUS.OPEN`, plus `commentCount`, `htmlUrl`, `authorAvatarUrl`, `assigneeAvatars` on Nostr rows |
| Create: `desktop/src/features/projects/lib/projectGithubIssues.ts` | Host-aware fetch, DTO mapper, number parser, hex identity filter, comments hook |
| Create: `desktop/src/features/projects/lib/projectGithubIssues.test.mjs` | Routing, mapper, number, pubkey-filter tests |
| `desktop/src/features/projects/lib/projectShareLinks.ts` | `issueShareLink` prefers a validated GitHub `htmlUrl` |
| `desktop/src/features/projects/lib/projectShareLinks.test.mjs` | GitHub URL vs hex `buzz://issue` |
| `desktop/src/features/projects/projectIssues.test.mjs` | Nostr mapper sets `commentCount` and `htmlUrl: null` |
| `desktop/src/features/projects/hooks.ts` | `useProjectIssuesQuery` returns `{ issues, hasMore }` via `fetchProjectIssuesWith` |
| `desktop/src/features/projects/issueMutations.ts` | Host-aware create; GitHub skips work-item invalidation |
| Create: `desktop/src/features/projects/issueMutations.test.mjs` | GitHub create does not sign or publish |
| `desktop/src/features/projects/ui/GitHubRepoStateRecovery.tsx` | Optional `unavailableTitle` and `titleId` |
| Create: `desktop/src/features/projects/ui/GitHubIssueIdentity.tsx` | Login + avatar; read-only assignee facepile |
| `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx` | Host-aware list/detail, recovery, `#N`, hide composer |
| `desktop/src/features/projects/ui/ProjectIssueCommentTimeline.tsx` | Optional GitHub identity mode |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Read `data.issues`; hex-only profile lookup |
| `desktop/src/testing/e2eBridge.ts` | Stub the three commands and optional auth error |
| Create: `desktop/tests/e2e/github-issues.spec.ts` | Smoke: `#N` + Open, create, no composer, auth recovery |
| `desktop/playwright.config.ts` | Register the spec in the smoke `testMatch` list |

Do not create other files.

---

### Task 1: Map `gh api` issue list JSON in Rust

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_github_pull_request.rs`
- Create: `desktop/src-tauri/src/commands/project_github_issues.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs` (`mod project_github_issues;`)

**Interfaces:**
- Consumes: `pub(crate)` `GitHubRepoRef::{parse, slug, owner, repo}`, `GhRunner::{from_resolved, ensure_auth, run, run_with_limit}`, `GhOutput`, `redact_diagnostic`, `combined_cli_diagnostic`
- Produces:
  - `pub struct GitHubIssueUserDto { pub login: String, pub avatar_url: String }`
  - `pub struct GitHubIssueDto { pub number: u64, pub title: String, pub body: String, pub state: String, pub html_url: String, pub comments: u64, pub created_at: i64, pub updated_at: i64, pub user: GitHubIssueUserDto, pub labels: Vec<String>, pub assignees: Vec<GitHubIssueUserDto> }`
  - `pub struct GitHubIssueListDto { pub issues: Vec<GitHubIssueDto>, pub has_more: bool }`
  - `pub(crate) fn list_github_issues_with(gh: &GhRunner, clone_url: &str, state: &str) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError>`
  - `pub(crate) fn remap_issues_error(error: ProjectPullRequestMergeError, diagnostic: &str) -> ProjectPullRequestMergeError`
  - `pub(crate) fn is_issue_html_url(repo: &GitHubRepoRef, html_url: &str, number: u64) -> bool`

- [ ] **Step 1: Expose owner and repo without growing the runner file**

In `GitHubRepoRef` change only the two field visibilities:

```rust
pub(crate) struct GitHubRepoRef {
    pub(crate) owner: String,
    pub(crate) repo: String,
}
```

Do not add methods, tests, or blank sections to `project_github_pull_request.rs`.
The file is already 998 lines and the desktop ratchet is 1000.

- [ ] **Step 2: Write failing list tests in `project_github_issues.rs`**

Unix `fake_gh` helper copied from `project_github_repository_state.rs`.
Match comments and `--method POST` **before** the issues list path.

```rust
const ISSUE_42: &str = r#"{"number":42,"title":"Broken login","body":"Steps","state":"open","html_url":"https://github.com/acme/app/issues/42","comments":3,"created_at":"2026-01-02T03:04:05Z","updated_at":"2026-01-03T03:04:05Z","user":{"login":"ada","avatar_url":"https://avatars.githubusercontent.com/u/1"},"labels":[{"name":"bug"}],"assignees":[{"login":"linus","avatar_url":"https://avatars.githubusercontent.com/u/2"}]}"#;

#[cfg(unix)]
fn fake_gh(script: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("create fake gh directory");
    let path = dir.path().join("gh");
    std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{script}\n")).expect("write fake gh");
    let mut permissions = std::fs::metadata(&path)
        .expect("stat fake gh")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod fake gh");
    (dir, path)
}

fn error_code(error: &ProjectPullRequestMergeError) -> String {
    serde_json::to_value(error).expect("json")["code"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[cfg(unix)]
#[test]
fn maps_open_issue_and_drops_pull_requests() {
    let script = format!(
        r#"
root=${{0%/gh}}
printf '%s\n' "$*" >> "$root/calls"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/issues/"*"/comments"*) exit 1 ;;
  *"--method POST"*) exit 1 ;;
  *"/repos/acme/app/issues"*)
    printf '%s' '[{{"number":42,"title":"Broken login","body":"Steps","state":"open","html_url":"https://github.com/acme/app/issues/42","comments":3,"created_at":"2026-01-02T03:04:05Z","updated_at":"2026-01-03T03:04:05Z","user":{{"login":"ada","avatar_url":"https://avatars.githubusercontent.com/u/1"}},"labels":[{{"name":"bug"}}],"assignees":[{{"login":"linus","avatar_url":"https://avatars.githubusercontent.com/u/2"}}]}},{{"number":7,"title":"A PR","state":"open","html_url":"https://github.com/acme/app/pull/7","comments":0,"created_at":"2026-01-02T03:04:05Z","updated_at":"2026-01-02T03:04:05Z","user":{{"login":"ada","avatar_url":"https://example.com/a"}},"labels":[],"assignees":[],"pull_request":{{"url":"https://api.github.com/repos/acme/app/pulls/7"}}}}]'
    ;;
  *) exit 1 ;;
esac
"#
    );
    let (dir, path) = fake_gh(&script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let page = list_github_issues_with(&gh, "https://github.com/acme/app", "open").expect("list");
    assert_eq!(page.issues.len(), 1);
    assert_eq!(page.issues[0].number, 42);
    assert_eq!(page.issues[0].state, "open");
    assert_eq!(page.issues[0].user.login, "ada");
    assert_eq!(page.issues[0].labels, vec!["bug"]);
    assert_eq!(page.issues[0].assignees[0].login, "linus");
    assert!(!page.has_more);
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.lines().any(|line| {
        line.contains("/repos/acme/app/issues?state=open&per_page=100&sort=updated&direction=desc")
            && line.contains("--jq")
            && line.contains("has_pull_request")
    }));
}

#[cfg(unix)]
#[test]
fn raw_page_of_100_sets_has_more_even_if_prs_are_dropped() {
    let items = (1..=100)
        .map(|n| {
            format!(
                r#"{{"number":{n},"title":"t{n}","body":"","state":"open","html_url":"https://github.com/acme/app/issues/{n}","comments":0,"created_at":"2026-01-02T03:04:05Z","updated_at":"2026-01-02T03:04:05Z","user":{{"login":"ada","avatar_url":"https://example.com/a"}},"labels":[],"assignees":[],"pull_request":{{"url":"https://example.com"}}}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/issues"*) printf '%s' '[{items}]' ;;
  *) exit 1 ;;
esac
"#
    );
    let (_dir, path) = fake_gh(&script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let page = list_github_issues_with(&gh, "https://github.com/acme/app", "open").expect("list");
    assert!(page.issues.is_empty());
    assert!(page.has_more);
}

#[cfg(unix)]
#[test]
fn rejects_foreign_or_malformed_html_url() {
    let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/issues"*)
    printf '%s' '[{"number":42,"title":"x","body":"","state":"open","html_url":"https://evil.example/issues/42","comments":0,"created_at":"2026-01-02T03:04:05Z","updated_at":"2026-01-02T03:04:05Z","user":{"login":"ada","avatar_url":"https://example.com/a"},"labels":[],"assignees":[]}]'
    ;;
  *) exit 1 ;;
esac
"#;
    let (_dir, path) = fake_gh(script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let page = list_github_issues_with(&gh, "https://github.com/acme/app", "open").expect("list");
    assert!(page.issues.is_empty());
}

#[test]
fn rejects_non_github_clone_url_before_runner() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
        .expect("dummy runner unused");
    let err = list_github_issues_with(
        &gh,
        &format!("https://relay.example/git/{}/app", "ab".repeat(32)),
        "open",
    )
    .expect_err("buzz git url");
    assert_eq!(error_code(&err), "github_issues_failed");
}

#[test]
fn rejects_state_all_before_runner() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
        .expect("dummy runner unused");
    let err = list_github_issues_with(&gh, "https://github.com/acme/app", "all").expect_err("all");
    assert_eq!(error_code(&err), "github_issues_failed");
}
```

- [ ] **Step 3: Run tests — expect compile fail**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib maps_open_issue_and_drops_pull_requests
```

Expected: `list_github_issues_with` missing.

- [ ] **Step 4: Implement list mapping**

```rust
const GH_ISSUE_STREAM_LIMIT: usize = 2 * 1024 * 1024;
const ISSUE_LIST_JQ: &str = "[.[] | {number, title, body: (.body // \"\"), state, html_url, comments, created_at, updated_at, user: (if .user == null then null else {login: .user.login, avatar_url: .user.avatar_url} end), labels: [(.labels // [])[] | if type == \"string\" then . else .name end], assignees: [(.assignees // [])[] | {login, avatar_url}], has_pull_request: has(\"pull_request\")}]";

pub(crate) fn list_github_issues_with(
    gh: &GhRunner,
    clone_url: &str,
    state: &str,
) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError> {
    if state != "open" && state != "closed" {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub issue list state must be open or closed.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_issues_error(error, ""))?;
    let path = format!(
        "/repos/{}/issues?state={state}&per_page=100&sort=updated&direction=desc",
        repo.slug()
    );
    let raw: Vec<GitHubIssueWire> = github_api_json(gh, &path, Some(ISSUE_LIST_JQ), "GET", None)?;
    let has_more = raw.len() == 100;
    let issues = raw
        .into_iter()
        .filter_map(|item| map_issue(&repo, item))
        .collect();
    Ok(GitHubIssueListDto { issues, has_more })
}
```

`GitHubIssueWire` is the `--jq` shape, including `has_pull_request: bool` and RFC3339 timestamp strings.
`map_issue` returns `None` when `has_pull_request` is true, `number == 0`, `user.login` is empty, `state` is not `open`/`closed`, timestamps do not parse, or `html_url` fails `is_issue_html_url`.
Null body becomes `""`.

`is_issue_html_url` accepts only `https://github.com/{owner}/{repo}/issues/{number}` for that parsed repo.
Owner and repo compare case-insensitively.
Reject query, fragment, credentials, userinfo, a non-443 port, a different host, or a different path.

Parse RFC3339 with `chrono::DateTime::parse_from_rfc3339` and store `timestamp()`.

`remap_issues_error` is `remap_state_error` with these substitutions:
- keep `github_cli_missing` and `github_auth_required`
- rate/abuse → `github_issues_failed`
- `404` / `not found` / non-rate `403` → `github_repo_unavailable`
- everything else, including truncated JSON and timeouts → `github_issues_failed`

`github_api_json` must call `gh.run_with_limit(..., GH_ISSUE_STREAM_LIMIT)` and remap through `remap_issues_error`.
Do not call `remap_state_error`.

- [ ] **Step 5: Run tests**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_issues
```

Expected: PASS on unix.
Windows runs the two URL/state tests and skips `fake_gh` tests.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_pull_request.rs \
  desktop/src-tauri/src/commands/project_github_issues.rs \
  desktop/src-tauri/src/commands/mod.rs
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git diff --check
git commit -s -m "feat(projects): map GitHub issues from gh api"
```

---

### Task 2: Create issue and list comments in Rust

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_github_issues.rs`

**Interfaces:**
- Consumes: `list_github_issues_with` helpers (`github_api_json`, `map_issue`, `remap_issues_error`, `GitHubRepoRef`)
- Produces:
  - `pub struct GitHubIssueCommentDto { pub id: u64, pub body: String, pub created_at: i64, pub user: GitHubIssueUserDto }`
  - `pub(crate) fn create_github_issue_with(gh: &GhRunner, clone_url: &str, title: &str, body: &str) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>`
  - `pub(crate) fn list_github_issue_comments_with(gh: &GhRunner, clone_url: &str, number: u64) -> Result<Vec<GitHubIssueCommentDto>, ProjectPullRequestMergeError>`

- [ ] **Step 1: Write failing create and comment tests**

```rust
#[test]
fn create_rejects_empty_or_overlong_title_before_runner() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
        .expect("dummy runner unused");
    let empty = create_github_issue_with(&gh, "https://github.com/acme/app", "   ", "body")
        .expect_err("empty");
    assert_eq!(error_code(&empty), "github_issues_failed");
    let long = create_github_issue_with(&gh, "https://github.com/acme/app", &"x".repeat(257), "")
        .expect_err("long");
    assert_eq!(error_code(&long), "github_issues_failed");
}

#[cfg(unix)]
#[test]
fn create_posts_title_and_body_and_returns_number() {
    let script = r#"
root=${0%/gh}
printf '%s\n' "$*" >> "$root/calls"
case "$*" in
  *auth*status*) exit 0 ;;
  *"--method POST"*"/repos/acme/app/issues"*)
    printf '%s' '{"number":43,"title":"New bug","body":"details","state":"open","html_url":"https://github.com/acme/app/issues/43","comments":0,"created_at":"2026-01-02T03:04:05Z","updated_at":"2026-01-02T03:04:05Z","user":{"login":"ada","avatar_url":"https://example.com/a"},"labels":[],"assignees":[]}'
    ;;
  *) exit 1 ;;
esac
"#;
    let (dir, path) = fake_gh(script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let issue = create_github_issue_with(&gh, "https://github.com/acme/app", "New bug", "details")
        .expect("create");
    assert_eq!(issue.number, 43);
    assert_eq!(issue.title, "New bug");
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.lines().any(|line| {
        line.contains("--method POST")
            && line.contains("/repos/acme/app/issues")
            && line.contains("--input")
    }));
}

#[test]
fn comments_reject_number_zero_before_runner() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
        .expect("dummy runner unused");
    let err = list_github_issue_comments_with(&gh, "https://github.com/acme/app", 0)
        .expect_err("zero");
    assert_eq!(error_code(&err), "github_issues_failed");
}

#[cfg(unix)]
#[test]
fn lists_first_page_of_comments_in_github_order() {
    let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/issues/42/comments"*)
    printf '%s' '[{"id":1,"body":"first","created_at":"2026-01-02T03:04:05Z","user":{"login":"ada","avatar_url":"https://example.com/a"}},{"id":2,"body":"second","created_at":"2026-01-02T04:04:05Z","user":{"login":"linus","avatar_url":"https://example.com/b"}}]'
    ;;
  *) exit 1 ;;
esac
"#;
    let (_dir, path) = fake_gh(script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let comments =
        list_github_issue_comments_with(&gh, "https://github.com/acme/app", 42).expect("comments");
    assert_eq!(comments.len(), 2);
    assert_eq!(comments[0].body, "first");
    assert_eq!(comments[1].user.login, "linus");
}
```

- [ ] **Step 2: Run — expect compile fail**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib create_posts_title_and_body_and_returns_number
```

Expected: `create_github_issue_with` missing.

- [ ] **Step 3: Implement create and comments**

Create validation before `gh`:
- `title.trim()` empty → `github_issues_failed` (`Issue title is required.`)
- trimmed length `> 256` → `github_issues_failed` (`Issue title must be 256 characters or fewer.`)

Write JSON `{ "title": trimmed, "body": body }` to a `tempfile` prefix `buzz-gh-` the same way `json_input` does in the merge module.
Do not call that private helper.
Map tempfile failures to `github_issues_failed`.

POST:

```text
gh api --hostname github.com --method POST /repos/{slug}/issues --input {tempfile} --jq '{ISSUE_ITEM_JQ}'
```

`ISSUE_ITEM_JQ` is the object form of `ISSUE_LIST_JQ` (no wrapping array).
Keep the tempfile alive until `run_with_limit` returns.
If `map_issue` returns `None` (including invalid `html_url`), return `github_issues_failed`.

Comments:
- `number == 0` fails before `gh`
- `GET /repos/{slug}/issues/{number}/comments?per_page=100`
- `--jq '[.[] | {id, body: (.body // ""), created_at, user: (if .user == null then null else {login: .user.login, avatar_url: .user.avatar_url} end)}]'`
- Keep GitHub order
- Drop a comment when `id == 0`, login is empty, or `created_at` does not parse
- Do not fetch page 2

- [ ] **Step 4: Run tests**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_issues
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_issues.rs
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git diff --check
git commit -s -m "feat(projects): create GitHub issues and list comments"
```

---

### Task 3: Remap errors and register Tauri commands

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_github_issues.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs` (`pub use project_github_issues::*;`)
- Modify: `desktop/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `list_github_issues_with`, `create_github_issue_with`, `list_github_issue_comments_with`, `GhRunner::discover`
- Produces:
  - `#[tauri::command] pub async fn list_github_issues(clone_url: String, state: String) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError>`
  - `#[tauri::command] pub async fn create_github_issue(clone_url: String, title: String, body: String) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>`
  - `#[tauri::command] pub async fn list_github_issue_comments(clone_url: String, number: u64) -> Result<Vec<GitHubIssueCommentDto>, ProjectPullRequestMergeError>`

- [ ] **Step 1: Write failing wrapper tests**

```rust
#[test]
fn wrapper_maps_discover_failure() {
    let err = list_github_issues_with_runner(
        "https://github.com/acme/app".into(),
        "open".into(),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&err), "github_cli_missing");
}

#[cfg(unix)]
#[test]
fn missing_auth_is_auth_required() {
    let script = r#"
case "$*" in
  *auth*status*) exit 1 ;;
  *) exit 1 ;;
esac
"#;
    let (_dir, path) = fake_gh(script);
    let err = list_github_issues_with(
        &GhRunner::from_resolved(Some(path)).expect("runner"),
        "https://github.com/acme/app",
        "open",
    )
    .expect_err("auth");
    assert_eq!(error_code(&err), "github_auth_required");
}

#[cfg(unix)]
#[test]
fn http_404_is_repo_unavailable_and_rate_limit_is_issues_failed() {
    let not_found = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/issues"*)
    printf 'gh: HTTP 404\n{"message":"Not Found"}\n' >&2
    exit 1
    ;;
  *) exit 1 ;;
esac
"#;
    let (_dir, path) = fake_gh(not_found);
    let err = list_github_issues_with(
        &GhRunner::from_resolved(Some(path)).expect("runner"),
        "https://github.com/acme/app",
        "open",
    )
    .expect_err("404");
    assert_eq!(error_code(&err), "github_repo_unavailable");

    let limited = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/issues"*)
    printf 'gh: HTTP 403\nAPI rate limit exceeded\n' >&2
    exit 1
    ;;
  *) exit 1 ;;
esac
"#;
    let (_dir, path) = fake_gh(limited);
    let err = list_github_issues_with(
        &GhRunner::from_resolved(Some(path)).expect("runner"),
        "https://github.com/acme/app",
        "open",
    )
    .expect_err("rate");
    assert_eq!(error_code(&err), "github_issues_failed");
}
```

- [ ] **Step 2: Implement wrappers and register commands**

```rust
pub(crate) fn list_github_issues_with_runner(
    clone_url: String,
    state: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issues_error(error, ""))?;
    list_github_issues_with(&gh, &clone_url, &state)
}

#[tauri::command]
pub async fn list_github_issues(
    clone_url: String,
    state: String,
) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_issues_with_runner(clone_url, state, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}
```

Mirror that pattern for `create_github_issue` and `list_github_issue_comments`.
Register all three next to `get_github_repository_state` in `desktop/src-tauri/src/lib.rs`.
Add `pub use project_github_issues::*;` in `mod.rs`.

Every return path of this module must be one of the four allowed codes.

- [ ] **Step 3: Run**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_issues
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_issues.rs \
  desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/lib.rs
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git diff --check
git commit -s -m "feat(projects): expose GitHub issue Tauri commands"
```

---

### Task 4: TypeScript DTOs, ProjectIssue fields, and share links

**Files:**
- Modify: `desktop/src/shared/api/projectGit.ts`
- Modify: `desktop/src/features/projects/projectIssues.mjs`
- Modify: `desktop/src/features/projects/projectIssues.test.mjs`
- Create: `desktop/src/features/projects/lib/projectGithubIssues.ts`
- Create: `desktop/src/features/projects/lib/projectGithubIssues.test.mjs`
- Modify: `desktop/src/features/projects/lib/projectShareLinks.ts`
- Modify: `desktop/src/features/projects/lib/projectShareLinks.test.mjs`

**Interfaces:**
- Consumes: `invokeTauri`, `parseProjectPullRequestMergeError`, `isGitHubCloneUrl`
- Produces:
  - `export type GithubIssueDto`
  - `export type GithubIssueListDto = { issues: GithubIssueDto[]; has_more: boolean }`
  - `export type GithubIssueCommentDto`
  - `export async function listGithubIssues(input: { cloneUrl: string; state: "open" | "closed" }): Promise<GithubIssueListDto>`
  - `export async function createGithubIssue(input: { cloneUrl: string; title: string; body: string }): Promise<GithubIssueDto>`
  - `export async function listGithubIssueComments(input: { cloneUrl: string; number: number }): Promise<GithubIssueCommentDto[]>`
  - `export function mapGithubIssueToProjectIssue(dto: GithubIssueDto, repoAddress: string | null): ProjectIssue`
  - `export function mapGithubCommentToProjectIssueComment(dto: GithubIssueCommentDto): ProjectIssue["comments"][number]`
  - `export function parseGithubIssueNumber(value: string | null | undefined): number | null`
  - `export function issueDisplayNumber(issueId: string): string`
  - `export function issueIdentityPubkeys(issues: ProjectIssue[]): string[]`
  - `export function isSafeGitHubIssueUrl(raw: string): boolean`
  - `PROJECT_ISSUE_STATUS.OPEN = "Open"`

- [ ] **Step 1: Extend the Nostr mapper and write a failing test**

In `PROJECT_ISSUE_STATUS` add `OPEN: "Open"`.
In `eventToProjectIssue` add:

```js
commentCount: comments.length,
htmlUrl: null,
authorAvatarUrl: null,
assigneeAvatars: {},
```

Add to `projectIssues.test.mjs`:

```js
test("nostr issues expose commentCount and a null GitHub htmlUrl", () => {
  const issue = eventToProjectIssue(issueEvent(), [], [
    {
      id: "c".repeat(64),
      kind: 1,
      pubkey: AUTHOR,
      created_at: 110,
      content: "hi",
      tags: [["e", "e".repeat(64)]],
    },
  ]);
  assert.equal(issue.commentCount, 1);
  assert.equal(issue.htmlUrl, null);
  assert.equal(issue.status, PROJECT_ISSUE_STATUS.BACKLOG);
  assert.equal(PROJECT_ISSUE_STATUS.OPEN, "Open");
});
```

- [ ] **Step 2: Run the Nostr test**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/projectIssues.test.mjs
```

Expected: FAIL until the new fields exist, then PASS after Step 1's implementation.

- [ ] **Step 3: Write failing mapper, routing, and share-link tests**

`projectGithubIssues.test.mjs`:

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import { isGitHubCloneUrl } from "./projectGitError.ts";
import {
  fetchProjectIssuesWith,
  issueDisplayNumber,
  issueIdentityPubkeys,
  mapGithubIssueToProjectIssue,
  parseGithubIssueNumber,
} from "./projectGithubIssues.ts";

const REPO_ADDRESS = `30617:${"a".repeat(64)}:app`;

const dto = {
  number: 42,
  title: "Broken login",
  body: "Steps",
  state: "open",
  html_url: "https://github.com/acme/app/issues/42",
  comments: 3,
  created_at: 1_704_166_645,
  updated_at: 1_704_253_045,
  user: { login: "ada", avatar_url: "https://avatars.githubusercontent.com/u/1" },
  labels: ["bug"],
  assignees: [{ login: "linus", avatar_url: "https://avatars.githubusercontent.com/u/2" }],
};

test("mapper builds #42 Open with login identities", () => {
  const issue = mapGithubIssueToProjectIssue(dto, REPO_ADDRESS);
  assert.equal(issue.id, "42");
  assert.equal(issue.status, "Open");
  assert.equal(issue.author, "ada");
  assert.equal(issue.authorAvatarUrl, "https://avatars.githubusercontent.com/u/1");
  assert.deepEqual(issue.assignees, ["linus"]);
  assert.equal(issue.assigneeAvatars.linus, "https://avatars.githubusercontent.com/u/2");
  assert.equal(issue.commentCount, 3);
  assert.deepEqual(issue.comments, []);
  assert.equal(issue.htmlUrl, "https://github.com/acme/app/issues/42");
  assert.equal(issueDisplayNumber(issue.id), "42");
});

test("fetchProjectIssuesWith uses GitHub only for github.com clone URLs", async () => {
  const calls = { github: 0, buzz: 0 };
  const githubProject = { id: "p1", repoAddress: REPO_ADDRESS, cloneUrls: ["https://github.com/acme/app"] };
  const buzzProject = {
    id: "p2",
    repoAddress: REPO_ADDRESS,
    cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
  };
  const github = await fetchProjectIssuesWith(githubProject, {
    loadGithub: async () => {
      calls.github += 1;
      return { issues: [dto], has_more: true };
    },
    loadBuzz: async () => {
      calls.buzz += 1;
      return { issues: [], hasMore: false };
    },
  });
  assert.equal(calls.github, 1);
  assert.equal(calls.buzz, 0);
  assert.equal(github.issues[0].id, "42");
  assert.equal(github.hasMore, true);

  calls.github = 0;
  await fetchProjectIssuesWith(buzzProject, {
    loadGithub: async () => {
      calls.github += 1;
      return { issues: [dto], has_more: false };
    },
    loadBuzz: async () => {
      calls.buzz += 1;
      return { issues: [], hasMore: false };
    },
  });
  assert.equal(calls.github, 0);
  assert.equal(calls.buzz, 1);
});

test("isGitHubCloneUrl accepts https and ssh github hosts", () => {
  assert.equal(isGitHubCloneUrl("https://github.com/acme/app"), true);
  assert.equal(isGitHubCloneUrl("git@github.com:acme/app.git"), true);
  assert.equal(isGitHubCloneUrl(`https://relay.example/git/${"ab".repeat(32)}/app`), false);
});

test("parseGithubIssueNumber accepts only positive decimal ids", () => {
  assert.equal(parseGithubIssueNumber("42"), 42);
  assert.equal(parseGithubIssueNumber("0"), null);
  assert.equal(parseGithubIssueNumber("0x2"), null);
  assert.equal(parseGithubIssueNumber("e".repeat(64)), null);
});

test("issueIdentityPubkeys drops GitHub logins", () => {
  const github = mapGithubIssueToProjectIssue(dto, REPO_ADDRESS);
  assert.deepEqual(issueIdentityPubkeys([github]), []);
});
```

Add to `projectShareLinks.test.mjs`:

```js
test("issueShareLink prefers a validated GitHub htmlUrl", () => {
  assert.equal(
    issueShareLink({
      id: "42",
      repoAddress: REPO_ADDRESS,
      htmlUrl: "https://github.com/acme/app/issues/42",
    }),
    "https://github.com/acme/app/issues/42",
  );
  assert.equal(
    issueShareLink({
      id: "42",
      repoAddress: REPO_ADDRESS,
      htmlUrl: "https://evil.example/issues/42",
    }),
    null,
  );
  assert.equal(
    issueShareLink({ id: EVENT_ID, repoAddress: REPO_ADDRESS, htmlUrl: null }),
    `buzz://issue?id=${EVENT_ID}&owner=${OWNER}&d=flappy-bee`,
  );
});
```

- [ ] **Step 4: Implement wrappers and mappers**

`listGithubIssues` invokes `"list_github_issues"` with `{ cloneUrl, state }` and rethrows `parseProjectPullRequestMergeError`.
`createGithubIssue` invokes `"create_github_issue"` with `{ cloneUrl, title, body }`.
`listGithubIssueComments` invokes `"list_github_issue_comments"` with `{ cloneUrl, number }`.

`mapGithubIssueToProjectIssue`:
- `id = String(dto.number)`
- `content = dto.body ?? ""`
- `tags = []`
- `author = dto.user.login`
- `authorAvatarUrl = dto.user.avatar_url`
- `assignees = dto.assignees.map((a) => a.login)`
- `assigneeAvatars = Object.fromEntries(dto.assignees.map((a) => [a.login, a.avatar_url]))`
- `status = dto.state === "closed" ? "Closed" : "Open"`
- `comments = []`
- `commentCount = dto.comments`
- `htmlUrl = dto.html_url`
- `repoAddress` from the announcement coordinate
- `channelId`, `originAgentName`, `recipients` empty
- `assigneeOperationHeads = {}`
- `statusEventId = null`

`fetchProjectIssuesWith` gates on `isGitHubCloneUrl(project.cloneUrls[0])`, not `projectRepoHost`.
GitHub path maps DTOs and sets `hasMore = Boolean(page.has_more)`.
Buzz path returns the loader result unchanged.

`isSafeGitHubIssueUrl` mirrors `isSafeGitHubRecoveryUrl` but requires exactly `/{owner}/{repo}/issues/{n}` with `n` matching `/^[1-9][0-9]*$/`.
`issueShareLink` returns `issue.htmlUrl` when that check passes; otherwise keep the hex `buzz://issue` path.

`parseGithubIssueNumber` matches `/^[1-9][0-9]*$/` and rejects values above `Number.MAX_SAFE_INTEGER`.
`issueDisplayNumber` returns the full id when that pattern matches, otherwise `issueId.slice(0, 8)`.
`issueIdentityPubkeys` keeps only `/^[a-fA-F0-9]{64}$/` values from author, recipients, assignees, and comment authors.

- [ ] **Step 5: Run**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectShareLinks.test.mjs src/features/projects/projectIssues.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/shared/api/projectGit.ts \
  desktop/src/features/projects/projectIssues.mjs \
  desktop/src/features/projects/projectIssues.test.mjs \
  desktop/src/features/projects/lib/projectGithubIssues.ts \
  desktop/src/features/projects/lib/projectGithubIssues.test.mjs \
  desktop/src/features/projects/lib/projectShareLinks.ts \
  desktop/src/features/projects/lib/projectShareLinks.test.mjs
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git diff --check
git commit -s -m "feat(projects): map GitHub issue DTOs onto ProjectIssue"
```

---

### Task 5: Host-aware fetch and create mutations

**Files:**
- Modify: `desktop/src/features/projects/hooks.ts`
- Modify: `desktop/src/features/projects/issueMutations.ts`
- Create: `desktop/src/features/projects/issueMutations.test.mjs`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx` (read `data.issues` only)
- Modify: `desktop/src/features/projects/lib/projectGithubIssues.ts` (comments hook)

**Interfaces:**
- Consumes: `fetchProjectIssuesWith`, `listGithubIssues`, `createGithubIssue`, `listGithubIssueComments`, `parseGithubIssueNumber`, `issueIdentityPubkeys`
- Produces:
  - `useProjectIssuesQuery` data type `{ issues: ProjectIssue[]; hasMore: boolean }`
  - `useCreateProjectIssueMutation` returns `string` (GitHub number or event id)
  - `export function useGithubIssueCommentsQuery(project, selectedIssueId)`
  - `export async function createProjectIssueWith(...)`

- [ ] **Step 1: Write the failing create-routing test**

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import { createProjectIssueWith } from "./issueMutations.ts";

test("GitHub create does not sign or publish kind 1621", async () => {
  const calls = { sign: 0, publish: 0, github: 0 };
  const id = await createProjectIssueWith(
    {
      id: "p1",
      owner: "a".repeat(64),
      repoAddress: `30617:${"a".repeat(64)}:app`,
      cloneUrls: ["https://github.com/acme/app"],
    },
    { title: "Broken login", body: "steps" },
    {
      createGithub: async () => {
        calls.github += 1;
        return { number: 43 };
      },
      publishBuzz: async () => {
        calls.sign += 1;
        calls.publish += 1;
        return "e".repeat(64);
      },
    },
  );
  assert.equal(id, "43");
  assert.equal(calls.github, 1);
  assert.equal(calls.sign, 0);
  assert.equal(calls.publish, 0);
});

test("Buzz create still publishes kind 1621", async () => {
  const calls = { github: 0, buzz: 0 };
  const id = await createProjectIssueWith(
    {
      id: "p2",
      owner: "a".repeat(64),
      repoAddress: `30617:${"a".repeat(64)}:app`,
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
    },
    { title: "Buzz bug", body: "" },
    {
      createGithub: async () => {
        calls.github += 1;
        return { number: 1 };
      },
      publishBuzz: async () => {
        calls.buzz += 1;
        return "e".repeat(64);
      },
    },
  );
  assert.equal(id, "e".repeat(64));
  assert.equal(calls.github, 0);
  assert.equal(calls.buzz, 1);
});
```

- [ ] **Step 2: Run — expect fail**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/issueMutations.test.mjs
```

Expected: `createProjectIssueWith` missing.

- [ ] **Step 3: Implement routing**

In `issueMutations.ts`:

```ts
export async function createProjectIssueWith(
  project: Project,
  input: CreateProjectIssueInput,
  loaders: {
    createGithub: (input: {
      cloneUrl: string;
      title: string;
      body: string;
    }) => Promise<{ number: number }>;
    publishBuzz: typeof publishProjectIssue;
  },
) {
  const cloneUrl = project.cloneUrls[0];
  if (isGitHubCloneUrl(cloneUrl)) {
    const created = await loaders.createGithub({
      cloneUrl,
      title: input.title,
      body: input.body,
    });
    return String(created.number);
  }
  return loaders.publishBuzz(project, input);
}

export function useCreateProjectIssueMutation(
  project: Project | null | undefined,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateProjectIssueInput) => {
      if (!project) throw new Error("No project selected.");
      return createProjectIssueWith(project, input, {
        createGithub: createGithubIssue,
        publishBuzz: publishProjectIssue,
      });
    },
    onSuccess: async () => {
      const tasks = [
        queryClient.invalidateQueries({
          queryKey: ["project", project?.id ?? "none", "issues"],
        }),
      ];
      if (!isGitHubCloneUrl(project?.cloneUrls[0])) {
        tasks.push(
          queryClient.invalidateQueries({ queryKey: ["projects", "work-items"] }),
          queryClient.invalidateQueries({
            queryKey: ["projects", "activity-summaries"],
          }),
        );
      }
      await Promise.all(tasks);
    },
  });
}
```

In `hooks.ts` keep the current kind:1621 fetch as `fetchBuzzProjectIssues` and make it return `{ issues, hasMore: false }`.
`useProjectIssuesQuery` calls `fetchProjectIssuesWith` with `loadGithub: ({ cloneUrl }) => listGithubIssues({ cloneUrl, state: "open" })`.
Do not add `state` to the query key in this slice.

Add `useGithubIssueCommentsQuery` in `projectGithubIssues.ts`:
- enabled when `isGitHubCloneUrl(project.cloneUrls[0])` and `parseGithubIssueNumber(selectedIssueId)` is a number
- key `["project", project.id, "issues", number, "comments"]`
- invoke `listGithubIssueComments`
- `staleTime` 30s

In `ProjectDetailScreen.tsx` only:
- `issuesQuery.data?.issues.find(...)` for `selectedIssue`
- `issueIdentityPubkeys(issuesQuery.data?.issues ?? [])` instead of flattening GitHub logins into `useUsersBatchQuery`

Do not add other logic to that 953-line file.

In `ProjectIssuesPanel.tsx` change only the data read so typecheck passes:

```ts
const issues = issuesQuery.data?.issues ?? [];
```

Leave GitHub chrome, recovery, and composer hiding for Task 6.

- [ ] **Step 4: Run**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test -- src/features/projects/issueMutations.test.mjs src/features/projects/lib/projectGithubIssues.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/features/projects/hooks.ts \
  desktop/src/features/projects/issueMutations.ts \
  desktop/src/features/projects/issueMutations.test.mjs \
  desktop/src/features/projects/lib/projectGithubIssues.ts \
  desktop/src/features/projects/ui/ProjectDetailScreen.tsx \
  desktop/src/features/projects/ui/ProjectIssuesPanel.tsx
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git diff --check
git commit -s -m "feat(projects): route GitHub issue list and create by clone URL"
```

---

### Task 6: Issues tab UI for GitHub rows

**Files:**
- Modify: `desktop/src/features/projects/ui/GitHubRepoStateRecovery.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueIdentity.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectIssueCommentTimeline.tsx`

**Interfaces:**
- Consumes: `useProjectIssuesQuery` `{ issues, hasMore }`, `useGithubIssueCommentsQuery`, `isGitHubCloneUrl`, `issueDisplayNumber`, `issueShareLink`, `GitHubRepoStateRecovery`
- Produces: GitHub list rows with `#N` and Open, read-only login chrome, hidden composer, comment load/retry

- [ ] **Step 1: Extend recovery without breaking branch recovery**

Add optional props `unavailableTitle` (default `Could not load GitHub branches`) and `titleId` (default `github-repo-state-recovery-title`).
Use `unavailableTitle` for `github_repo_unavailable`, `github_state_failed`, `github_issues_failed`, and the default title.
Keep `GitHub CLI is required` and `GitHub authentication required`.
Use `titleId` for `aria-labelledby` / heading `id` so an issues recovery and a branch recovery can exist on the same page.

- [ ] **Step 2: Add GitHub identity chrome**

`GitHubIssueIdentity.tsx` exports:
- `GitHubLoginIdentity({ login, avatarUrl, showLabel?: boolean })` using `UserAvatar` + a `text-xs` login span
- `GitHubAssigneeFacepile({ logins, avatars })` overlapping up to three avatars from `assigneeAvatars`

Do not call `normalizePubkey` or `ProfileIdentityButton`.

- [ ] **Step 3: Host-gate the panel**

`const issues = issuesQuery.data?.issues ?? [];`
`const githubHosted = isGitHubCloneUrl(project.cloneUrls[0]);`

Render order:
1. Loading → `Loading issues…`
2. `githubHosted && issuesQuery.isError` → `<GitHubRepoStateRecovery unavailableTitle="Could not load GitHub issues" titleId="github-issues-recovery-title" error={issuesQuery.error} onRetry={() => void issuesQuery.refetch()} />`
3. `issuesQuery.isError` → `Could not load issues for this repository.`
4. `issues.length === 0` → `githubHosted ? "No open issues." : "No issues yet."`
   If `githubHosted && issuesQuery.data?.hasMore`, still show the muted more line under that empty copy.

List rows when `githubHosted`:
- status icon stays the green `CircleDot` for `"Open"`
- status text is `Open`
- `#` uses `issueDisplayNumber(issue.id)` so GitHub shows `#42`, not eight hex chars
- author is `GitHubLoginIdentity`
- assignees are `GitHubAssigneeFacepile`
- comment badge uses `issue.commentCount ?? issue.comments.length`

When `githubHosted && hasMore`, one muted `text-xs` line: `More open issues exist on GitHub.`

Detail when `githubHosted`:
- title + `#${issueDisplayNumber(issue.id)}`
- body via `ProjectRichContent` with `tags={[]}`
- hide `DiscussedInChannels`
- hide `ForumComposer` and do not render `data-testid="project-issue-comment-composer"`
- hide `IssueAssigneesRow`
- author and assignees in the rail use GitHub identity chrome
- `useGithubIssueCommentsQuery(project, issue.id)`
- while comments load: `Loading comments…`
- if comments fail: keep the body and render `GitHubRepoStateRecovery` with `unavailableTitle="Could not load GitHub comments"` and retry that query only
- if comments succeed: pass mapped comments to `ProjectIssueCommentTimeline` with `githubMode`

`ProjectIssueCommentTimeline` gains `githubMode?: boolean`.
When true, render `comment.author` as a login, use `comment.authorAvatarUrl`, and do not call `normalizePubkey` or `ProfileAuthorName`.

Buzz rows keep today's `ProfileIdentityButton`, facepile, composer, and discussed-in-channels.

- [ ] **Step 4: Typecheck and unit tests**

```bash
. ./bin/activate-hermit
cd desktop && pnpm typecheck && pnpm check:px-text && pnpm check:file-sizes && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/issueMutations.test.mjs src/features/projects/lib/projectShareLinks.test.mjs src/features/projects/projectIssues.test.mjs
```

Expected: PASS.
`ProjectIssuesPanel.tsx` must stay under 1000 lines.
If it crosses the ratchet, move GitHub detail chrome into `GitHubIssueIdentity.tsx` instead of raising the limit.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/features/projects/ui/GitHubRepoStateRecovery.tsx \
  desktop/src/features/projects/ui/GitHubIssueIdentity.tsx \
  desktop/src/features/projects/ui/ProjectIssuesPanel.tsx \
  desktop/src/features/projects/ui/ProjectIssueCommentTimeline.tsx
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git diff --check
git commit -s -m "feat(projects): show GitHub issues as #N with read-only metadata"
```

---

### Task 7: e2e mock + smoke

**Files:**
- Modify: `desktop/src/testing/e2eBridge.ts`
- Create: `desktop/tests/e2e/github-issues.spec.ts`
- Modify: `desktop/playwright.config.ts` (`smoke` `testMatch` adds `**/github-issues.spec.ts`)

**Interfaces:**
- Stub `list_github_issues` returns open `#42` plus any created issues
- Stub `create_github_issue` returns `#43` from the submitted title
- Stub `list_github_issue_comments` returns one login comment
- `window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__` throws `{ code, message }`

- [ ] **Step 1: Stub commands**

Add to the Window type next to the other GitHub flags:

```ts
__BUZZ_E2E_GITHUB_ISSUES_ERROR__?: { code: string; message: string };
__BUZZ_E2E_GITHUB_CREATED_ISSUES__?: Array<Record<string, unknown>>;
```

In the invoke switch, **before** any default that would miss them:

```ts
case "list_github_issues": {
  if (window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__) {
    throw window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__;
  }
  const created = window.__BUZZ_E2E_GITHUB_CREATED_ISSUES__ ?? [];
  return {
    issues: [
      {
        number: 42,
        title: "Broken login",
        body: "Repro steps",
        state: "open",
        html_url: "https://github.com/acme/app/issues/42",
        comments: 1,
        created_at: Math.floor(Date.now() / 1000) - 3600,
        updated_at: Math.floor(Date.now() / 1000) - 60,
        user: { login: "ada", avatar_url: "" },
        labels: ["bug"],
        assignees: [{ login: "linus", avatar_url: "" }],
      },
      ...created,
    ],
    has_more: false,
  };
}
case "create_github_issue": {
  if (window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__) {
    throw window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__;
  }
  const input = payload as { title?: string; body?: string };
  const created = {
    number: 43,
    title: input.title ?? "Untitled",
    body: input.body ?? "",
    state: "open",
    html_url: "https://github.com/acme/app/issues/43",
    comments: 0,
    created_at: Math.floor(Date.now() / 1000),
    updated_at: Math.floor(Date.now() / 1000),
    user: { login: "ada", avatar_url: "" },
    labels: [],
    assignees: [],
  };
  window.__BUZZ_E2E_GITHUB_CREATED_ISSUES__ = [
    ...(window.__BUZZ_E2E_GITHUB_CREATED_ISSUES__ ?? []),
    created,
  ];
  return created;
}
case "list_github_issue_comments": {
  if (window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__) {
    throw window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__;
  }
  return [
    {
      id: 1,
      body: "I can reproduce this.",
      created_at: Math.floor(Date.now() / 1000) - 30,
      user: { login: "ada", avatar_url: "" },
    },
  ];
}
```

Follow `installMockBridge` + `addInitScript` **before** the bridge (AGENTS.md).
Build with `pnpm build:e2e`, never `pnpm run build`.

- [ ] **Step 2: Write the smoke spec**

```ts
import { expect, test } from "@playwright/test";

import { waitForAnimations } from "../helpers/animations";
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

test("GitHub-hosted Issues tab lists #N and creates without a comment composer", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Issues", exact: true }).click();

  const issueRow = page.getByTestId("project-issue-row").first();
  await expect(issueRow).toBeVisible({ timeout: 10_000 });
  await expect(issueRow).toContainText("#42");
  await expect(issueRow).toContainText("Open");
  await waitForAnimations(page);

  await page.getByRole("button", { name: "New issue" }).click();
  await page
    .getByTestId("create-issue-dialog")
    .getByPlaceholder("Describe the issue")
    .fill("New GitHub bug");
  await page.getByTestId("create-issue-submit").click();
  await expect(page.getByText("#43")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText("New GitHub bug")).toBeVisible();
  await expect(page.getByTestId("project-issue-comment-composer")).toHaveCount(0);
  const commands = await page.evaluate(
    () => window.__BUZZ_E2E_COMMANDS__ ?? [],
  );
  expect(commands).toContain("list_github_issues");
  expect(commands).toContain("create_github_issue");
});

test("GitHub issues auth recovery is not an empty Buzz list", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
    window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__ = {
      code: "github_auth_required",
      message:
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  await expect(page.getByText("GitHub authentication required")).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByText("No issues yet")).toHaveCount(0);
  await expect(page.getByText("No open issues.")).toHaveCount(0);
  await expect(page.getByTestId("project-issue-row")).toHaveCount(0);
});
```

Register `**/github-issues.spec.ts` in `desktop/playwright.config.ts` smoke `testMatch` next to `github-repo-state.spec.ts`.

- [ ] **Step 3: Run smoke**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test:e2e:smoke -- github-issues
```

Expected: PASS.
If `test:e2e:smoke` does not accept a file filter, run `pnpm build:e2e && pnpm exec playwright test --project=smoke github-issues`.

Also run the existing Buzz issue spec so host gating did not break kind:1621:

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec playwright test --project=smoke project-issue-comments
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/testing/e2eBridge.ts \
  desktop/tests/e2e/github-issues.spec.ts \
  desktop/playwright.config.ts
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git diff --check
git commit -s -m "test(e2e): cover GitHub issue list, create, and auth recovery"
```

---

## Spec coverage

| Spec requirement | Task |
|------------------|------|
| Host split via `isGitHubCloneUrl` / `GitHubRepoRef::parse`; no 1621 union | 1, 5 |
| `gh api` list with required `state`, 100 items, `sort=updated`, drop PRs | 1 |
| `has_more` from raw page length 100 | 1 |
| Validated repo-bound `html_url` | 1, 4 |
| Create title+body via `--input`; empty/overlong title before `gh` | 2 |
| Comments first 100, oldest first, no prefetch of the whole list | 2, 6 |
| Error codes + remap; no empty list on auth/CLI failure | 3, 6, 7 |
| `ProjectIssue` `#N`, `"Open"`, logins, `commentCount`, `htmlUrl` | 4, 6 |
| `issueShareLink` uses GitHub URL; hex Nostr still `buzz://issue` | 4 |
| GitHub create does not sign/publish; invalidates issues only | 5 |
| Query key stays `["project", id, "issues"]`; UI sends `open` | 5 |
| Global work-items list unchanged | 5 (no edits) |
| List `#N` + Open + login + read-only facepile + more note | 6 |
| Hide composer, discussed-in, assign row; comment load/retry | 6 |
| Empty `No open issues.`; recovery titles | 6, 7 |
| e2e `#N`, create, no composer, auth recovery | 7 |
| Existing kind:1621 tests stay green | 4, 7 |

## Acceptance criteria

On a GitHub-hosted project (`harness-service` or the mock `https://github.com/acme/app`) with `gh` installed and authenticated, or with the e2e stubs:

- The Issues tab shows GitHub open issues as `#N` with status **Open**.
- Creating an issue creates a GitHub issue and does not publish kind:1621.
- Copy link copies `https://github.com/<owner>/<repo>/issues/<N>`.
- Opening `#N` shows the list body and fetched comments, with no comment composer.
- Without `gh` or without auth, the tab shows merge-style recovery, not an empty Buzz list.

Buzz-hosted repositories still list and create kind:1621 only.

## Validation commands

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_issues
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectShareLinks.test.mjs src/features/projects/issueMutations.test.mjs src/features/projects/projectIssues.test.mjs && pnpm typecheck && pnpm check:px-text && pnpm check:file-sizes
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues
. ./bin/activate-hermit && cd desktop && pnpm exec playwright test --project=smoke project-issue-comments
```
