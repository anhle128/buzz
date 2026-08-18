# GitHub issue writes (close, comment, labels, assignees) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a Projects repository uses a plain `github.com` clone URL, its per-repository Issues tab can list Open or Closed GitHub issues, close and reopen an issue, post a body-only comment, and add or remove one label or assignee at a time through `gh api`, without publishing Nostr events.

**Architecture:** Extend the I1+I2 `GhRunner` issue module with a sibling write module.
Route close, comment, label, and assignee mutations by `isGitHubCloneUrl`.
Keep Buzz-hosted close, comment, label, and assign paths unchanged.
Put GitHub-only filter, close/reopen, composer, label, and assignee chrome in sibling UI files so GitHub logins never reach pubkey helpers.

**Tech Stack:** Rust, Tauri 2, `gh api`, React 19, TanStack Query, Node `node:test`, and Playwright with the E2E mock bridge.

**Spec:** [2026-08-18-github-issue-writes-design.md](../specs/2026-08-18-github-issue-writes-design.md)

**Depends on:** [2026-08-18-github-issues-design.md](../specs/2026-08-18-github-issues-design.md) and the I1+I2 implementation in `desktop/src-tauri/src/commands/project_github_issues.rs`.

**Product contract:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) make GitHub the native issue backend for GitHub-hosted repositories while Buzz remains the collaboration layer.

**Phase doc:** [phase-02-github-issues.md](../../../plans/20260818-1211-github-native-host/phase-02-github-issues.md), slice I3–I6.

**Canonical save path after plan mode:** `docs/superpowers/plans/2026-08-19-github-issue-writes.md`

## Global Constraints

- Implement I3–I6 only: Open | Closed filter, close/reopen, body-only comments, add/remove one label, add/remove one assignee, and Assign me via `GET /user`.
- Do not implement comment edit or delete, close `state_reason`, title or body edit after create, `state=all`, a second page, or Load more.
- Do not change the global Projects Issues list, project card/activity counts, CLI, mobile, or the `buzz://issue` scheme.
- Do not union GitHub Issues with kind `1621`, import GitHub issues into Nostr, or dual-write.
- Do not call `gh issue`, add a provider trait, or store a GitHub token.
- Authenticate only through installed `gh` and `gh auth status --hostname github.com`.
- Use `gh api` with one page, `per_page=100`, and no `--paginate`.
- Require `list_github_issues.state` to be exactly `open` or `closed`.
- Accept only positive `u64` issue numbers in Rust and positive safe decimal integers in TypeScript.
- Treat GitHub authors and assignees as logins plus avatar URLs, never as Nostr pubkeys.
- Never pass a GitHub login to `ProfileIdentityButton`, `ProfileAuthorName`, `normalizePubkey`, `useUsersBatchQuery`, `IssueAssigneesRow`, or Nostr assignment/comment mutations.
- Do not mount `IssueAssigneesRow` or `ForumComposer` on GitHub-hosted issue detail.
- Do not add Close / Reopen to Buzz-hosted issues.
- Close and reopen send only `{ "state": "closed" | "open" }`.
- Comments send only a trimmed non-empty `{ "body" }` through a tempfile passed to `gh api --input`.
- Labels and assignees add or remove one name or login per action.
- Do not replace the whole label or assignee set.
- Assign me uses `GET /user` and never the Buzz pubkey.
- Percent-encode label names on DELETE.
- Send assignee logins only in JSON bodies, never as path segments.
- On GitHub writes, invalidate only the prefix `["project", project.id, "issues"]`.
- Do not invalidate `work-items` or `activity-summaries` for GitHub writes.
- GitHub list query keys are `["project", id, "issues", "open"]` and `["project", id, "issues", "closed"]`.
- Buzz list query keys stay `["project", id, "issues"]`.
- Open | Closed is GitHub-only, local tab state, default Open, not a URL param, and not persisted.
- After close: set filter to Closed and keep `#N` selected.
- After reopen or create: set filter to Open and keep or select `#N`.
- If the loaded page does not contain `#N`, clear selection and stay on that filter.
- Failed writes do not change the Open | Closed filter.
- Failed comments do not clear composer text.
- Check GitHub list errors before empty-success rendering.
- Empty Open copy is `No open issues.`
- Empty Closed copy is `No closed issues.`
- Auth or CLI failure must not show those empty strings.
- Use only `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, `github_issue_unavailable`, and `github_issues_failed`.
- Do not leak `github_merge_failed` or `github_state_failed`.
- Do not run live GitHub requests in automated tests.
- Do not add production `unsafe`, `unwrap()`, or `expect()` calls.
- Add doc comments to every new public Rust or TypeScript API.
- Use named rem-based text tokens and do not add arbitrary text sizes.
- Keep every edited desktop file at or below 1,000 lines.
- Activate Hermit in every shell command with `. ./bin/activate-hermit && ...` or as the first line of the same shell block.
- Run commands from the repository root because shell working directories do not persist between tool calls.
- Run GitNexus `impact({ target, direction: "upstream" })` before editing each existing symbol when the MCP tools are available.
- Warn before proceeding if GitNexus reports HIGH or CRITICAL risk.
- Run GitNexus `detect_changes({ scope: "staged" })` before every commit and `detect_changes({ scope: "compare", base_ref: "main" })` before final handoff when the MCP tools are available.
- If GitNexus tools are unavailable, record that fact in the implementation handoff and use `git diff --stat`, `git diff --name-only`, and `git diff --check` as the fallback scope evidence.
- Sign every commit with `git commit -s`.

## Resolved Implementation Decisions

- Keep list, create, and comment-list in `project_github_issues.rs`.
- Put write and catalog commands in `desktop/src-tauri/src/commands/project_github_issue_writes.rs` because the existing module is already 766 lines and the 1,000-line ratchet is hard.
- Expose `github_api_json`, `github_json_input`, `map_issue`, `remap_issues_error`, `is_issue_html_url`, `is_issue_comment_html_url`, `ISSUE_ITEM_JQ`, `GitHubIssueWire`, and `GitHubIssueCommentWire` as `pub(crate)` from the existing module.
- Replace `issue_json_input(title, body)` with `github_json_input(&serde_json::Value)` so state, comment, label, and assignee bodies share one tempfile helper.
- Add `html_url` to the shared comment DTO and validate it on both list and create.
- Accept a comment `html_url` only when it is `https://github.com/{owner}/{repo}/issues/{number}#issuecomment-{id}` for that repo, number, and comment id.
- Drop a comment whose `html_url` fails that check.
- `github_issue_unavailable` is used only for issue-targeted writes: state, comment, and assignee add/remove.
- Label add/remove 404 maps to `github_issues_failed` so the UI keeps the selection and refetches the label catalog.
- List and catalog 404 still map to `github_repo_unavailable`.
- Non-rate 403 still maps to `github_repo_unavailable`.
- GitHub label POST and DELETE return a label array, not an issue.
- After a successful label POST or DELETE, GET `/repos/{owner}/{repo}/issues/{number}` and return that mapped issue DTO.
- Percent-encode DELETE label names with `percent_encoding::NON_ALPHANUMERIC`.
- Label catalog color must be a 6-digit hex string.
- Strip a leading `#` if GitHub sends one, then store the color without `#`.
- Drop catalog labels with an empty name or invalid color.
- Assignee catalog stale time is 60 seconds, matching labels.
- Authenticated-user query key is `["github", "authenticated-user"]` with `staleTime: Number.POSITIVE_INFINITY`.
- Do not add that query to `resetCommunityState`.
- `ForumComposer` cannot hide the mention picker or paperclip control, so GitHub comments use a textarea composer.
- Lift Open | Closed state to `ProjectDetailScreen` so the breadcrumb query and the panel query share the same key.
- Reset the filter to Open when `repository.id` changes.
- `desktop/src-tauri/src/lib.rs` is 999 lines.
- Adding nine command names would break the 1,000-line ratchet, so Task 4 moves `tauri::generate_handler![...]` into `desktop/src-tauri/src/invoke.rs`.
- Move existing GitHub issue DTOs and invoke wrappers from `projectGit.ts` into `desktop/src/shared/api/projectGithubIssues.ts` so the 881-line API file stays under 1,000 lines.
- Re-export the I1+I2 symbols from `projectGit.ts` so current imports keep compiling.
- Do not add write hooks to `hooks.ts`.
- GitNexus MCP tools are not available in the planning session.
- Executors should try them once, then use `git diff` fallback if they are still missing.

## Open Questions

### 1. Label mutation response shape

GitHub `POST`/`DELETE` `/issues/{n}/labels` returns labels, while the spec asks for an issue DTO.

**Provisional default:** After a successful label mutation, GET the issue and return that DTO.

### 2. Label 404 versus issue 404

A missing label and a missing issue both return HTTP 404.

**Provisional default:** Label writes map 404 to `github_issues_failed`, keep selection, and refetch the catalog.
State, comment, and assignee writes map 404 to `github_issue_unavailable`, toast “Issue not found.”, and clear selection.

### 3. GitHub comment composer

`ForumComposer` cannot hide mention or attach controls.

**Provisional default:** Use a body-only textarea.
Do not add hide-flags to `ForumComposer` in this slice.

## File Map

| File | Responsibility |
|------|----------------|
| `desktop/src-tauri/src/commands/project_github_issues.rs` | Expose shared `gh api` helpers, add comment `html_url`, send `state=closed`, and remap helpers |
| Create `desktop/src-tauri/src/commands/project_github_issue_writes.rs` | Close/reopen, comment create, label/assignee catalogs and writes, `GET /user` |
| `desktop/src-tauri/src/commands/mod.rs` | Declare and re-export the write module |
| Create `desktop/src-tauri/src/invoke.rs` | Own `generate_handler!` including the nine new commands |
| `desktop/src-tauri/src/lib.rs` | `mod invoke` and call `invoke::desktop_invoke_handler()` |
| Create `desktop/src/shared/api/projectGithubIssues.ts` | All GitHub issue DTOs and Tauri invoke wrappers |
| `desktop/src/shared/api/projectGit.ts` | Re-export the I1+I2 issue symbols |
| `desktop/src/features/projects/lib/projectGithubIssues.ts` | Pass `state` through `fetchProjectIssuesWith` and list query keys |
| `desktop/src/features/projects/lib/projectGithubIssues.test.mjs` | Protect open/closed routing and query keys |
| Create `desktop/src/features/projects/lib/projectGithubIssueWrites.ts` | Write targets, filter/selection policy, host-routed mutations, catalog hooks |
| Create `desktop/src/features/projects/lib/projectGithubIssueWrites.test.mjs` | Protect host routing, invalidation, Assign me login, and selection policy |
| `desktop/src/features/projects/hooks.ts` | `useProjectIssuesQuery(project, listState)` |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Own Open \| Closed state, reset on repo change, set Open after create |
| `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx` | Pass list state into the Issues panel |
| `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx` | Mount the filter, empty copy, and selection clearing |
| Create `desktop/src/features/projects/ui/GitHubIssueIdentity.tsx` | Login identity and assignee facepile |
| Create `desktop/src/features/projects/ui/GitHubIssueStateFilter.tsx` | Open \| Closed control |
| Create `desktop/src/features/projects/ui/GitHubIssueStateButton.tsx` | Close / Reopen |
| Create `desktop/src/features/projects/ui/GitHubIssueCommentComposer.tsx` | Body-only textarea |
| Create `desktop/src/features/projects/ui/GitHubIssueLabels.tsx` | Chips plus catalog add |
| Create `desktop/src/features/projects/ui/GitHubIssueAssignees.tsx` | Facepile, catalog, Assign me |
| `desktop/src/features/projects/ui/GitHubProjectIssues.tsx` | Compose the GitHub detail chrome and show Closed status |
| `desktop/src/testing/e2eBridge.ts` | Mutable GitHub issue store and write-command stubs |
| `desktop/tests/e2e/github-issues.spec.ts` | Expect the body composer and keep auth/comment-load recovery |
| Create `desktop/tests/e2e/github-issue-writes.spec.ts` | Close, comment, labels, assignees, reopen, and failed-close filter |
| `desktop/playwright.config.ts` | Register the write spec in smoke `testMatch` |

Do not modify `issueAssignments.ts`, Buzz `createProjectIssueComment` in `hooks.ts`, or `project-issue-comments.spec.ts` except to keep them green without behavior changes.

## Required Impact Checks

Run these before the first edit in each task when GitNexus MCP tools are available.
Report direct callers, affected processes, and the risk level before editing.
If GitNexus is unavailable, record that fact and continue with direct source inspection.

- Task 1: `github_api_json`, `remap_issues_error`, `list_github_issue_comments_with`, `GitHubIssueCommentDto`, and `issue_json_input`.
- Task 2: `map_issue`, `ISSUE_ITEM_JQ`, and `GhRunner::run_with_limit`.
- Task 3: `GitHubRepoRef::slug` and `percent_encoding`.
- Task 4: `tauri::generate_handler!` in `lib.rs`, `commands` re-exports, `listGithubIssues`, and `parseProjectPullRequestMergeError`.
- Task 5: `fetchProjectIssuesWith`, `useProjectIssuesQuery`, `useCreateProjectIssueMutation`, `createProjectIssueComment`, and `projectIssueInvalidationKeys`.
- Task 6: `ProjectIssuesPanel`, `GitHubIssueDetail`, `GitHubIssueRow`, `ProjectDetailScreen`, and `WorkspaceTabs`.
- Task 7: E2E invoke switch, `Window` GitHub issue flags, and Playwright smoke `testMatch`.

---

### Task 1: Shared Rust helpers, comment `html_url`, and `state=closed`

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_issues.rs`

**Interfaces:**

- Consumes: existing `GhRunner`, `GitHubRepoRef`, `ISSUE_COMMENTS_JQ`, and `remap_issues_error`.
- Produces: `pub(crate) fn github_api_json`, `pub(crate) fn github_json_input`, `pub(crate) fn is_issue_comment_html_url`, `pub(crate) fn remap_issue_write_error`, `pub(crate) fn remap_label_write_error`, `html_url` on `GitHubIssueCommentDto`, and list support for `state=closed`.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `github_api_json`, `remap_issues_error`, `list_github_issue_comments_with`, and `GitHubIssueCommentDto`.
Stop and warn before editing if any result is HIGH or CRITICAL.

- [ ] **Step 2: Write failing helper and comment-URL tests first**

Add these tests next to `comments_keep_projected_github_order`.
Update that existing test fixture so each comment includes a valid `html_url`.

```rust
#[test]
fn accepts_only_repo_bound_comment_urls() {
    let repo = GitHubRepoRef::parse("https://github.com/Acme/App").expect("repo");
    for (raw, number, comment_id, expected) in [
        ("https://github.com/acme/app/issues/42#issuecomment-9", 42, 9, true),
        ("https://github.com/ACME/APP/issues/42#issuecomment-9", 42, 9, true),
        ("https://github.com/acme/app/issues/42#issuecomment-8", 42, 9, false),
        ("https://github.com/acme/app/issues/43#issuecomment-9", 42, 9, false),
        ("https://github.com/acme/app/issues/42", 42, 9, false),
        ("https://evil.example/acme/app/issues/42#issuecomment-9", 42, 9, false),
        ("https://github.com/acme/app/issues/42?x=1#issuecomment-9", 42, 9, false),
    ] {
        assert_eq!(
            is_issue_comment_html_url(&repo, raw, number, comment_id),
            expected,
            "{raw}"
        );
    }
}

#[cfg(unix)]
#[test]
fn list_sends_state_closed() {
    let output = json!([projected_issue(7, false)]);
    let (dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let page = list_github_issues_with(&gh, "https://github.com/acme/app", "closed").expect("list");
    assert_eq!(page.issues[0].number, 7);
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains(
        "/repos/acme/app/issues?state=closed&per_page=100&sort=updated&direction=desc"
    ));
}

#[cfg(unix)]
#[test]
fn comments_drop_foreign_html_url() {
    let output = serde_json::json!([{
        "id": 1,
        "body": "first",
        "html_url": "https://evil.example/issues/42#issuecomment-1",
        "created_at": "2026-01-02T03:04:05Z",
        "user": { "login": "ada", "avatar_url": "https://example.com/a" }
    }]);
    let (_dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let comments = list_github_issue_comments_with(&gh, "https://github.com/acme/app", 42)
        .expect("comments");
    assert!(comments.is_empty());
}

#[test]
fn write_remap_turns_issue_404_into_issue_unavailable() {
    let not_found = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 404");
    assert_eq!(
        error_code(&remap_issue_write_error(not_found, "Not Found")),
        "github_issue_unavailable"
    );
    let forbidden = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
    assert_eq!(
        error_code(&remap_issue_write_error(forbidden, "Forbidden")),
        "github_repo_unavailable"
    );
}

#[test]
fn label_remap_turns_404_into_issues_failed() {
    let not_found = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 404");
    assert_eq!(
        error_code(&remap_label_write_error(not_found, "Not Found")),
        "github_issues_failed"
    );
}
```

Also add `html_url` to `comments_keep_projected_github_order` fixtures:

```rust
{ "id": 1, "body": "first", "html_url": "https://github.com/acme/app/issues/42#issuecomment-1", "created_at": "2026-01-02T03:04:05Z", "user": { "login": "ada", "avatar_url": "https://example.com/a" } }
```

- [ ] **Step 3: Run the focused test and verify RED**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib accepts_only_repo_bound_comment_urls
```

Expected: compilation fails because `is_issue_comment_html_url` does not exist.

- [ ] **Step 4: Implement the helpers and comment DTO change**

Change `ISSUE_COMMENTS_JQ` to include `html_url`.

Add `html_url: String` to `GitHubIssueCommentDto` and `GitHubIssueCommentWire`.

Replace `issue_json_input` with `github_json_input(&serde_json::Value)` that writes JSON to a `buzz-gh-` tempfile and flushes it.
Update `create_github_issue_with` to call `github_json_input(&serde_json::json!({ "title": title, "body": body }))`.

Make `github_api_json`, `map_issue`, `remap_issues_error`, `ISSUE_ITEM_JQ`, `GitHubIssueWire`, and `GitHubIssueCommentWire` `pub(crate)`.

Implement `is_issue_comment_html_url` with `url::Url`.
Require HTTPS, host `github.com`, no username/password/query/port, no `%` or `\`, case-insensitive owner/repo, literal `issues`, matching number, and fragment exactly `issuecomment-{id}`.

In `list_github_issue_comments_with`, drop comments whose `html_url` fails that check.

Implement remappers:

```rust
pub(crate) fn remap_issue_write_error(
    error: ProjectPullRequestMergeError,
    diagnostic: &str,
) -> ProjectPullRequestMergeError {
    let remapped = remap_issues_error(error, diagnostic);
    let value = serde_json::to_value(&remapped).unwrap_or_default();
    if value.get("code").and_then(|value| value.as_str()) != Some("github_repo_unavailable") {
        return remapped;
    }
    let message = value
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("GitHub issue request failed.");
    let combined = format!("{diagnostic} {message}").to_ascii_lowercase();
    if combined.contains("404") || combined.contains("not found") {
        return ProjectPullRequestMergeError::new("github_issue_unavailable", message);
    }
    remapped
}

pub(crate) fn remap_label_write_error(
    error: ProjectPullRequestMergeError,
    diagnostic: &str,
) -> ProjectPullRequestMergeError {
    let remapped = remap_issues_error(error, diagnostic);
    let value = serde_json::to_value(&remapped).unwrap_or_default();
    if value.get("code").and_then(|value| value.as_str()) != Some("github_repo_unavailable") {
        return remapped;
    }
    let message = value
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("GitHub issue request failed.");
    let combined = format!("{diagnostic} {message}").to_ascii_lowercase();
    if combined.contains("404") || combined.contains("not found") {
        return ProjectPullRequestMergeError::new("github_issues_failed", message);
    }
    remapped
}
```

- [ ] **Step 5: Run all issue-module tests and verify GREEN**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
```

Expected: existing list/create/comment tests and the new URL, closed-state, and remap tests pass.

- [ ] **Step 6: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_issues.rs
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): validate GitHub issue comment URLs"
```

---

### Task 2: Close, reopen, and create comment in Rust

**Files:**

- Create: `desktop/src-tauri/src/commands/project_github_issue_writes.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs` to add only `mod project_github_issue_writes;` in this task

**Interfaces:**

- Consumes: `github_api_json`, `github_json_input`, `map_issue`, `remap_issues_error`, `remap_issue_write_error`, `is_issue_comment_html_url`, `ISSUE_ITEM_JQ`, `GitHubIssueWire`, `GitHubRepoRef`, and `GhRunner`.
- Produces: `update_github_issue_state_with` and `create_github_issue_comment_with`.

- [ ] **Step 1: Add the module declaration and write failing tests first**

Add `mod project_github_issue_writes;` beside `mod project_github_issues;`.
Create the new file with test-only fixtures first.

Required tests:

- `state_rejects_unknown_and_zero_before_running_gh` using `/bin/false`.
- `close_patches_state_only` asserts PATCH `/repos/acme/app/issues/42` and tempfile JSON `{ "state": "closed" }`.
- `comment_rejects_blank_body_before_running_gh`.
- `comment_posts_trimmed_body_and_validates_html_url` asserts POST `/repos/acme/app/issues/42/comments` and `{ "body": "Looks good" }`.
- `comment_rejects_foreign_html_url` returns `github_issues_failed`.
- `writes_reject_buzz_url_before_running_gh`.

Reuse a `fake_gh_input` helper that copies the `--input` tempfile to `$root/input.json`.

- [ ] **Step 2: Run and verify RED**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib close_patches_state_only
```

Expected: compilation fails because `update_github_issue_state_with` does not exist.

- [ ] **Step 3: Implement close/reopen and comment create**

```rust
pub(crate) fn update_github_issue_state_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    state: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>

pub(crate) fn create_github_issue_comment_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    body: &str,
) -> Result<GitHubIssueCommentDto, ProjectPullRequestMergeError>
```

Validation order: unknown state or empty body, then `number == 0`, then `GitHubRepoRef::parse`, then `ensure_auth`, then `gh api`.
State body is exactly `{ "state": "open" | "closed" }`.
Comment body is the trimmed string.
Created comments must pass `is_issue_comment_html_url`.
Write-path 404 remaps through `remap_issue_write_error`.

- [ ] **Step 4: Run write-module tests and verify GREEN**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issue_writes
```

Expected: state and comment tests pass on Unix.

- [ ] **Step 5: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_issue_writes.rs \
  desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/commands/project_github_issues.rs
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): close and comment on GitHub issues"
```

---

### Task 3: Labels, assignees, and authenticated user in Rust

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_issue_writes.rs`

**Interfaces:**

- Consumes: helpers from Tasks 1 and 2 plus `percent_encoding::NON_ALPHANUMERIC`.
- Produces: label and assignee catalog/write functions and `get_github_authenticated_user_with`.

- [ ] **Step 1: Write failing catalog and write tests**

Required tests:

- `label_delete_percent_encodes_spaces_and_hash` asserts `/labels/good%20first%20%23issue` and no raw space in the path.
- `label_rejects_empty_name_before_running_gh`.
- `assignee_add_sends_login_in_json_body` asserts `{ "assignees": ["linus"] }` and `/issues/42/assignees` with no `/assignees/linus` path.
- `user_maps_login_and_avatar`.
- `labels_keep_six_hex_colors_without_hash` keeps `bug/d73a4a` and `docs/0075ca`, drops `color: "red"`.

- [ ] **Step 2: Run and verify RED**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib label_delete_percent_encodes_spaces_and_hash
```

Expected: compilation fails because `remove_github_issue_label_with` does not exist.

- [ ] **Step 3: Implement catalogs and writes**

```rust
pub(crate) fn list_github_repo_labels_with(gh: &GhRunner, clone_url: &str) -> Result<Vec<GitHubRepoLabelDto>, ProjectPullRequestMergeError>
pub(crate) fn add_github_issue_labels_with(gh: &GhRunner, clone_url: &str, number: u64, name: &str) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>
pub(crate) fn remove_github_issue_label_with(gh: &GhRunner, clone_url: &str, number: u64, name: &str) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>
pub(crate) fn list_github_repo_assignees_with(gh: &GhRunner, clone_url: &str) -> Result<Vec<GitHubIssueUserDto>, ProjectPullRequestMergeError>
pub(crate) fn add_github_issue_assignees_with(gh: &GhRunner, clone_url: &str, number: u64, login: &str) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>
pub(crate) fn remove_github_issue_assignee_with(gh: &GhRunner, clone_url: &str, number: u64, login: &str) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>
pub(crate) fn get_github_authenticated_user_with(gh: &GhRunner) -> Result<GitHubIssueUserDto, ProjectPullRequestMergeError>
```

Encode DELETE label names with `utf8_percent_encode(name, NON_ALPHANUMERIC)`.
After successful label POST or DELETE, GET `/repos/{owner}/{repo}/issues/{number}` and `map_issue`.
Label 404 uses `remap_label_write_error`.
Assignee add/remove send JSON `{ "assignees": [login] }` via POST or DELETE on `/issues/{number}/assignees`.
`GET /user` uses `{login, avatar_url}` and rejects an empty login.

- [ ] **Step 4: Run all write-module tests and verify GREEN**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issue_writes
```

- [ ] **Step 5: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_issue_writes.rs
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): write GitHub issue labels and assignees"
```

---

### Task 4: Register commands and TypeScript wrappers

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_issue_writes.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs`
- Create: `desktop/src-tauri/src/invoke.rs`
- Modify: `desktop/src-tauri/src/lib.rs`
- Create: `desktop/src/shared/api/projectGithubIssues.ts`
- Modify: `desktop/src/shared/api/projectGit.ts`

**Interfaces:**

- Consumes: the `*_with` functions from Tasks 2 and 3 and `GhRunner::discover`.
- Produces: nine Tauri commands plus TypeScript wrappers that pass camelCase `cloneUrl`, `number`, `state`, `name`, `login`, and `body`.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `list_github_issues`, `generate_handler` in `lib.rs`, and `listGithubIssues`.

- [ ] **Step 2: Write failing discovery-wrapper tests**

```rust
#[test]
fn state_wrapper_maps_missing_discovered_cli() {
    let error = update_github_issue_state_with_runner(
        "https://github.com/acme/app".to_string(),
        42,
        "closed".to_string(),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}

#[test]
fn user_wrapper_maps_missing_discovered_cli() {
    let error = get_github_authenticated_user_with_runner(GhRunner::from_resolved(None))
        .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}
```

- [ ] **Step 3: Run and verify RED**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib state_wrapper_maps_missing_discovered_cli
```

Expected: compilation fails because `update_github_issue_state_with_runner` does not exist.

- [ ] **Step 4: Implement wrappers, commands, invoke extraction, and TypeScript API**

Add one `*_with_runner` and one `#[tauri::command]` for each function from Tasks 2 and 3, using the same `spawn_blocking` + `GhRunner::discover()` pattern as `list_github_issues`.

Exact command signatures:

```rust
pub async fn update_github_issue_state(clone_url: String, number: u64, state: String) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>;
pub async fn create_github_issue_comment(clone_url: String, number: u64, body: String) -> Result<GitHubIssueCommentDto, ProjectPullRequestMergeError>;
pub async fn list_github_repo_labels(clone_url: String) -> Result<Vec<GitHubRepoLabelDto>, ProjectPullRequestMergeError>;
pub async fn add_github_issue_labels(clone_url: String, number: u64, name: String) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>;
pub async fn remove_github_issue_label(clone_url: String, number: u64, name: String) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>;
pub async fn list_github_repo_assignees(clone_url: String) -> Result<Vec<GitHubIssueUserDto>, ProjectPullRequestMergeError>;
pub async fn add_github_issue_assignees(clone_url: String, number: u64, login: String) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>;
pub async fn remove_github_issue_assignee(clone_url: String, number: u64, login: String) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>;
pub async fn get_github_authenticated_user() -> Result<GitHubIssueUserDto, ProjectPullRequestMergeError>;
```

Add `pub use project_github_issue_writes::*;` in `commands/mod.rs`.

Create `desktop/src-tauri/src/invoke.rs`.
Move the current `tauri::generate_handler![...]` list from `lib.rs:602-928` into `desktop_invoke_handler()`.
Insert the nine new command names immediately after `list_github_issue_comments`.
Copy every `use` from `lib.rs` that those command identifiers need.
Add `mod invoke;` next to `mod commands;` in `lib.rs`.
Replace `.invoke_handler(tauri::generate_handler![...])` with `.invoke_handler(invoke::desktop_invoke_handler())`.

If the `Invoke` type does not compile, use `impl Fn(tauri::ipc::Invoke<tauri::Wry>) + Send + Sync + 'static`.

Create `desktop/src/shared/api/projectGithubIssues.ts` and move the I1+I2 issue DTOs plus `listGithubIssues`, `createGithubIssue`, and `listGithubIssueComments` into it.
Add `html_url: string` to `GithubIssueCommentDto`.
Add wrappers `updateGithubIssueState`, `createGithubIssueComment`, `listGithubRepoLabels`, `addGithubIssueLabels`, `removeGithubIssueLabel`, `listGithubRepoAssignees`, `addGithubIssueAssignees`, `removeGithubIssueAssignee`, and `getGithubAuthenticatedUser`.
Each wrapper calls `invokeTauri` and rethrows `parseProjectPullRequestMergeError`.

In `projectGit.ts`, delete the moved issue block and re-export the I1+I2 symbols from `./projectGithubIssues`.

- [ ] **Step 5: Run Rust tests, typecheck, and the line-count ratchet**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issue_writes
. ./bin/activate-hermit && cd desktop && pnpm typecheck && pnpm check:file-sizes
```

Expected: tests pass and every edited desktop file is at or below 1,000 lines.
`lib.rs` must drop well below 1,000 after the handler move.

- [ ] **Step 6: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
cd desktop && pnpm exec biome check --write src/shared/api/projectGit.ts src/shared/api/projectGithubIssues.ts
git add desktop/src-tauri/src/commands/project_github_issue_writes.rs \
  desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/invoke.rs \
  desktop/src-tauri/src/lib.rs \
  desktop/src/shared/api/projectGit.ts \
  desktop/src/shared/api/projectGithubIssues.ts
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): expose GitHub issue write commands"
```

---

### Task 5: Query keys, write routing, and selection policy

**Files:**

- Modify: `desktop/src/features/projects/lib/projectGithubIssues.ts`
- Modify: `desktop/src/features/projects/lib/projectGithubIssues.test.mjs`
- Create: `desktop/src/features/projects/lib/projectGithubIssueWrites.ts`
- Create: `desktop/src/features/projects/lib/projectGithubIssueWrites.test.mjs`
- Modify: `desktop/src/features/projects/hooks.ts`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx`

**Interfaces:**

- Consumes: `listGithubIssues`, write wrappers, `parseGithubIssueNumber`, and `isGitHubCloneUrl`.
- Produces: `fetchProjectIssuesWith(..., state)`, `useProjectIssuesQuery(project, listState)`, `githubIssueWriteTarget`, `nextGithubIssueListState`, `selectedGithubIssueAfterListLoad`, `githubIssueWriteInvalidationKey`, and host-routed write helpers.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `fetchProjectIssuesWith`, `useProjectIssuesQuery`, `createProjectIssueComment`, and `ProjectDetailScreen`.

- [ ] **Step 2: Write failing routing and policy tests**

Add a closed-state case to `projectGithubIssues.test.mjs` that asserts `loadGithub` received `{ cloneUrl, state: "closed" }`.

Create `projectGithubIssueWrites.test.mjs` covering:

- `githubIssueWriteTarget` accepts only GitHub URLs and positive issue numbers.
- `nextGithubIssueListState("close") === "closed"` and reopen/create return `"open"`.
- `selectedGithubIssueAfterListLoad` keeps `#N` while `isSuccess` is false and clears it only when the successful page lacks `#N`.
- `createGithubIssueCommentWith` on a GitHub URL never calls `publishBuzz` and sends the trimmed body.
- `createGithubIssueCommentWith` on a Buzz URL never calls `createGithub`.
- Empty comment bodies throw `Comment cannot be empty.` before either backend.
- `updateGithubIssueStateWith` on GitHub never calls a Buzz updater.
- `githubIssueWriteInvalidationKey("p1")` is `["project", "p1", "issues"]`.

- [ ] **Step 3: Run and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectGithubIssueWrites.test.mjs
```

Expected: `fetchProjectIssuesWith` does not accept `"closed"` and `projectGithubIssueWrites.ts` is missing.

- [ ] **Step 4: Implement routing and policy**

Change `fetchProjectIssuesWith` so GitHub calls `loadGithub({ cloneUrl, state })` and Buzz ignores `state`.
Default `state` is `"open"`.

Change `useProjectIssuesQuery(project, listState = "open")` so GitHub keys are `["project", id, "issues", listState]` and Buzz keys stay `["project", id, "issues"]`.

Create `projectGithubIssueWrites.ts` with the helpers the tests import, plus:

```ts
useGithubRepoLabelsQuery -> ["project", id, "github-labels"], staleTime 60_000
useGithubRepoAssigneesQuery -> ["project", id, "github-assignees"], staleTime 60_000
useGithubAuthenticatedUserQuery -> ["github", "authenticated-user"], staleTime Infinity
```

In `ProjectDetailScreen.tsx` add `githubIssueListState` defaulting to `"open"`.
Reset it to `"open"` when `repository.id` changes.
Pass that state into `useProjectIssuesQuery`.
After create success, `setGithubIssueListState("open")` and `setSelectedIssueId(issueId)`.
Do not call `issuesQuery.refetch()` after create.

Pass `githubIssueListState` and `onGithubIssueListStateChange` through `WorkspaceTabs` into `ProjectIssuesPanel`.
The panel must call `useProjectIssuesQuery(project, githubIssueListState)`.
After a successful GitHub list fetch, clear a missing selection with `selectedGithubIssueAfterListLoad`.

- [ ] **Step 5: Run tests and typecheck and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectGithubIssueWrites.test.mjs src/features/projects/issueMutations.test.mjs && pnpm typecheck && pnpm check:file-sizes
```

Expected: routing, policy, and create-invalidation tests pass.
`ProjectDetailScreen.tsx` stays at or below 1,000 lines.

- [ ] **Step 6: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/features/projects/lib/projectGithubIssues.ts \
  src/features/projects/lib/projectGithubIssues.test.mjs \
  src/features/projects/lib/projectGithubIssueWrites.ts \
  src/features/projects/lib/projectGithubIssueWrites.test.mjs \
  src/features/projects/hooks.ts \
  src/features/projects/ui/ProjectDetailScreen.tsx \
  src/features/projects/ui/ProjectWorkspaceTabs.tsx \
  src/features/projects/ui/ProjectIssuesPanel.tsx
git add desktop/src/features/projects/lib/projectGithubIssues.ts \
  desktop/src/features/projects/lib/projectGithubIssues.test.mjs \
  desktop/src/features/projects/lib/projectGithubIssueWrites.ts \
  desktop/src/features/projects/lib/projectGithubIssueWrites.test.mjs \
  desktop/src/features/projects/hooks.ts \
  desktop/src/features/projects/ui/ProjectDetailScreen.tsx \
  desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx \
  desktop/src/features/projects/ui/ProjectIssuesPanel.tsx
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): route GitHub issue writes and list state"
```

---

### Task 6: GitHub-only write UI

**Files:**

- Create: `desktop/src/features/projects/ui/GitHubIssueIdentity.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueStateFilter.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueStateButton.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueCommentComposer.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueLabels.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueAssignees.tsx`
- Modify: `desktop/src/features/projects/ui/GitHubProjectIssues.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx`
- Modify: `desktop/tests/e2e/github-issues.spec.ts`

**Interfaces:**

- Consumes: write helpers and catalog hooks from Task 5, `mapGithubIssueToProjectIssue`, and `GitHubRepoStateRecovery`.
- Produces: GitHub filter, Close/Reopen, textarea composer, label chips, and assignee picker that never mount Buzz identity components.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `ProjectIssuesPanel`, `GitHubIssueDetail`, `GitHubIssueRow`, and `WorkspaceTabs`.

- [ ] **Step 2: Change the I1+I2 smoke composer assertion so it fails until the textarea exists**

In `github-issues.spec.ts`, replace the hidden-composer assertion with:

```ts
  const composer = page.getByTestId("project-issue-comment-composer");
  await expect(composer).toBeVisible();
  await expect(page.getByTestId("message-insert-mention")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Attach file" })).toHaveCount(0);
  await expect(page.getByTestId("issue-discussed-in")).toHaveCount(0);
  await expect(page.getByTestId("project-issue-assign")).toHaveCount(0);
```

Keep the comment-load failure spec.
The composer may be visible while comments retry.

- [ ] **Step 3: Run the existing smoke spec and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts
```

Expected: the GitHub detail still hides the composer, so `toBeVisible()` fails.

- [ ] **Step 4: Implement the sibling UI files**

Move `GitHubLoginIdentity` and `GitHubAssigneeFacepile` into `GitHubIssueIdentity.tsx`.

`GitHubIssueStateFilter` is a tablist with Open and Closed buttons.
Test ids: `project-github-issue-state-filter`, `project-github-issue-filter-open`, `project-github-issue-filter-closed`.

`GitHubIssueStateButton` calls `updateGithubIssueStateWith`.
On success, call `onListStateChange(nextGithubIssueListState(action))`, keep `issue.id` selected, and invalidate `githubIssueWriteInvalidationKey(project.id)`.
On `github_issue_unavailable`, toast `Issue not found.` and clear selection.
On any other error, toast the parsed `gh` message and do not change the filter.
Close when `issue.status === "Open"`.
Reopen when `issue.status === "Closed"`.
Test ids: `project-github-issue-close`, `project-github-issue-reopen`.

`GitHubIssueCommentComposer` is a textarea plus a Comment button inside `data-testid="project-issue-comment-composer"`.
Input test id is `project-github-issue-comment-input`.
Submit test id is `project-github-issue-comment-submit`.
Trim before invoke.
On success, toast `Comment posted.`, clear the textarea, and invalidate the issues prefix.
On failure, toast the parsed message and keep the text.
Do not import `ForumComposer`.

`GitHubIssueLabels` renders chips from `issue.labels`.
Each chip removes that name.
The add control lists catalog names that are not already on the issue.
Hide add when the catalog is empty or failed.
On label write `github_issues_failed`, invalidate `["project", id, "github-labels"]`.
Chip test id is `project-github-issue-label` with `data-label-name={name}`.
Option test id is `project-github-issue-label-option-${name}`.

`GitHubIssueAssignees` renders login identities from `issue.assignees`.
Assign me / Unassign me uses `useGithubAuthenticatedUserQuery(true)`.
If that login is already an assignee, show Unassign me.
If `GET /user` failed, disable Assign me and render `GitHubRepoStateRecovery` with `unavailableTitle="Could not load GitHub user"`.
Never read the Buzz pubkey.
Do not render `data-testid="project-issue-assign"`.
Use `project-github-issue-assign-me`, `project-github-issue-unassign-me`, and `project-github-issue-unassign-${login}`.

Update `GitHubIssueRow` so status comes from `issue.status`.
Closed rows use `CircleX` and `text-destructive`.

Update `GitHubIssueDetail` to accept `onListStateChange` and `onSelectedIssueIdChange`.
Put Close / Reopen in the detail header.
Mount the textarea composer under the comment timeline.
Mount labels and assignees in the meta rail even when the current sets are empty.

Update `ProjectIssuesPanel` so GitHub loading, empty, and list states keep the filter visible.
Auth/CLI recovery stays full-page and does not show the empty copy.
Empty strings are `No open issues.` and `No closed issues.`
`hasMore` copy is `More open issues exist on GitHub.` or `More closed issues exist on GitHub.`

- [ ] **Step 5: Run the updated I1+I2 smoke spec and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts
. ./bin/activate-hermit && cd desktop && pnpm typecheck && pnpm check:file-sizes && pnpm check:px-text
```

Expected: `#42` still lists, create still works, the body composer is visible without mention or attach controls, and auth recovery still wins over empty copy.

- [ ] **Step 6: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/features/projects/ui/GitHubIssueIdentity.tsx \
  src/features/projects/ui/GitHubIssueStateFilter.tsx \
  src/features/projects/ui/GitHubIssueStateButton.tsx \
  src/features/projects/ui/GitHubIssueCommentComposer.tsx \
  src/features/projects/ui/GitHubIssueLabels.tsx \
  src/features/projects/ui/GitHubIssueAssignees.tsx \
  src/features/projects/ui/GitHubProjectIssues.tsx \
  src/features/projects/ui/ProjectIssuesPanel.tsx \
  tests/e2e/github-issues.spec.ts
git add desktop/src/features/projects/ui/GitHubIssueIdentity.tsx \
  desktop/src/features/projects/ui/GitHubIssueStateFilter.tsx \
  desktop/src/features/projects/ui/GitHubIssueStateButton.tsx \
  desktop/src/features/projects/ui/GitHubIssueCommentComposer.tsx \
  desktop/src/features/projects/ui/GitHubIssueLabels.tsx \
  desktop/src/features/projects/ui/GitHubIssueAssignees.tsx \
  desktop/src/features/projects/ui/GitHubProjectIssues.tsx \
  desktop/src/features/projects/ui/ProjectIssuesPanel.tsx \
  desktop/tests/e2e/github-issues.spec.ts
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): add GitHub issue write chrome"
```

---

### Task 7: Mutable mock store and write smoke coverage

**Files:**

- Modify: `desktop/src/testing/e2eBridge.ts`
- Create: `desktop/tests/e2e/github-issue-writes.spec.ts`
- Modify: `desktop/playwright.config.ts`

**Interfaces:**

- Consumes: the nine new commands plus I1+I2 list/create/comments.
- Produces: an in-memory GitHub issue store the UI can close, comment, label, assign, and reopen.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for the E2E invoke switch and Playwright smoke `testMatch`.

- [ ] **Step 2: Add Window flags and a mutable store, then write the failing write spec**

Add `__BUZZ_E2E_GITHUB_ISSUE_WRITE_ERROR__` and `__BUZZ_E2E_GITHUB_ISSUE_STORE__`.
Initialize the store in `maybeInstallE2eTauriMocks` if it is missing.
Seed issue `#42` as open with label `bug` and assignee `linus`.
Seed catalog labels `bug` and `docs`.
Seed catalog assignees `linus` and `ada`.
Seed authenticated user `ada`.
Seed the two existing comments with valid `html_url` values.

Change `list_github_issues` so it filters `store.issues` by `input.state`.
Do not throw when `state === "closed"`.
Keep `__BUZZ_E2E_GITHUB_ISSUES_ERROR__` as a list-level failure.

Add switch cases for `update_github_issue_state`, `create_github_issue_comment`, `list_github_repo_labels`, `add_github_issue_labels`, `remove_github_issue_label`, `list_github_repo_assignees`, `add_github_issue_assignees`, `remove_github_issue_assignee`, and `get_github_authenticated_user`.
`list_github_issue_comments` must read `store.commentsByNumber[number]`.

Create `github-issue-writes.spec.ts` with three tests:

1. Close `#42`, assert Closed filter and kept selection, post `Looks good`, add `docs`, remove `bug`, unassign `linus`, Assign me as `ada`, reopen to Open, and assert the write commands ran and `sign_event` did not.
2. Stub `github_issues_failed` on writes, click Close, assert toast `Close failed.` and Open remains selected.
3. Stub `github_auth_required` on list and assert recovery, not `No open issues.` or `No closed issues.`

Add `"**/github-issue-writes.spec.ts"` next to `"**/github-issues.spec.ts"` in smoke `testMatch`.

- [ ] **Step 3: Run the write spec and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issue-writes.spec.ts
```

Expected: the mock still rejects `state=closed` or the write commands are missing, so close does not move `#42`.

- [ ] **Step 4: Finish the store and any remaining UI test-id wiring, then verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts github-issue-writes.spec.ts project-issue-comments.spec.ts
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issue_writes
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectGithubIssueWrites.test.mjs src/features/projects/issueMutations.test.mjs && pnpm typecheck && pnpm check:file-sizes && pnpm check:px-text
```

Expected: GitHub write smoke passes, I1+I2 smoke still passes, Buzz comment and assign specs stay green, and no file-size or px-text failures.

- [ ] **Step 5: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write src/testing/e2eBridge.ts tests/e2e/github-issue-writes.spec.ts playwright.config.ts
git add desktop/src/testing/e2eBridge.ts \
  desktop/tests/e2e/github-issue-writes.spec.ts \
  desktop/playwright.config.ts
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "test(projects): cover GitHub issue writes in smoke e2e"
```

If GitNexus is available, run `detect_changes({ scope: "compare", base_ref: "main" })` before handoff.

---

## Acceptance Criteria

On a GitHub-hosted project (`harness-service` or equivalent), with `gh` installed and authenticated:

- Open | Closed lists GitHub issues as `#N` for that state.
- Close moves `#N` to the Closed filter and keeps it selected.
- Reopen moves `#N` back to Open.
- A comment created in Buzz appears on the GitHub issue.
- No kind:1 is published for that comment.
- Adding or removing a label or assignee changes the GitHub issue.
- Assignees are logins, not pubkeys.
- Assign me uses the `gh` authenticated user.
- Without `gh` or without auth, the tab shows merge-style recovery, not an empty Buzz list.

Buzz-hosted repositories still list, create, comment, and assign through Nostr only.

## Validation Commands

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issue_writes
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectGithubIssueWrites.test.mjs src/features/projects/issueMutations.test.mjs src/features/projects/projectIssues.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm typecheck && pnpm check:file-sizes && pnpm check:px-text
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts github-issue-writes.spec.ts project-issue-comments.spec.ts
```

Manual check on `harness-service` after `gh auth status --hostname github.com` succeeds: close `#N`, comment, change one label and one assignee, Assign me, then reopen.
Confirm the GitHub issue on github.com matches and no kind:1 appears in the Buzz issue timeline.

## Spec Coverage

| Spec requirement | Task |
|---|---|
| `list_github_issues` sends `state=closed` | Task 1 |
| Comment `html_url` validation | Task 1 |
| `github_issue_unavailable` versus repo 404 | Task 1 |
| Close / reopen PATCH `{ "state" }` | Task 2 |
| Comment POST `{ "body" }` via tempfile | Task 2 |
| Empty comment / number 0 / Buzz URL fail before `gh` | Task 2 |
| Label DELETE percent-encoding | Task 3 |
| Assignee JSON body, not path | Task 3 |
| `GET /user` login + avatar | Task 3 |
| Label color 6-hex without `#` | Task 3 |
| Nine Tauri commands registered | Task 4 |
| TypeScript invoke wrappers | Task 4 |
| Query key includes state | Task 5 |
| GitHub writes do not sign or publish Nostr | Task 5 |
| After close/reopen/create filter policy | Task 5 |
| Failed write does not change filter | Task 5 / Task 7 |
| Open \| Closed chrome, empty copy, hasMore copy | Task 6 |
| Close / Reopen in detail header | Task 6 |
| Body-only composer, no mentions or media | Task 6 |
| Label chips + catalog | Task 6 |
| Assignee facepile + Assign me | Task 6 |
| Auth recovery before empty list | Task 6 / Task 7 |
| Write smoke: close, comment, label, assignee, reopen | Task 7 |
| Buzz comment and assign specs stay green | Task 7 |
