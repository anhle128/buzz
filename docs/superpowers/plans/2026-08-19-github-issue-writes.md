# GitHub issue writes (close, comment, labels, assignees) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a Projects repository uses a plain `github.com` clone URL, its per-repository Issues tab can list Open or Closed GitHub issues, close and reopen an issue, post a body-only comment, and add or remove one label or assignee at a time through `gh api`, without publishing Nostr events.

**Architecture:** Extend the existing I1+I2 native issue boundary with a focused Rust write module and a focused TypeScript write API.
Route the existing project-issue comment mutation by repository host as a defense-in-depth guarantee, while dedicated GitHub state, label, and assignee hooks reject non-GitHub targets before invoking Tauri.
Keep Open or Closed state in `ProjectDetailScreen`, and keep GitHub-only identity and write chrome separate from every Nostr pubkey component.

**Tech Stack:** Rust, Tauri 2, `gh api`, React 19, TanStack Query, Node `node:test`, and Playwright with the E2E mock bridge.

**Spec:** [2026-08-18-github-issue-writes-design.md](../specs/2026-08-18-github-issue-writes-design.md)

**Depends on:** [2026-08-18-github-issues-design.md](../specs/2026-08-18-github-issues-design.md) and the existing I1+I2 implementation in `desktop/src-tauri/src/commands/project_github_issues.rs`.

**Product contract:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) make GitHub the native issue backend for GitHub-hosted repositories while Buzz remains the collaboration layer.

**Testing contract:** [TESTING.md](../../../TESTING.md) and the desktop E2E rules in [AGENTS.md](../../../AGENTS.md) require focused unit coverage, an E2E-mode build, the mock Tauri bridge, and real-workflow evidence when a designated test repository is available.

**Phase doc:** [phase-02-github-issues.md](../../../plans/20260818-1211-github-native-host/phase-02-github-issues.md), slice I3-I6.

## Global Constraints

- Implement I3-I6 only: Open | Closed filter, close/reopen, body-only comments, add/remove one label, add/remove one assignee, and Assign me via `GET /user`.
- Do not implement comment edit or delete, close `state_reason`, title or body edit after create, `state=all`, a second page, or Load more.
- Do not change the global Projects Issues list, project card/activity counts, CLI, mobile, or the `buzz://issue` scheme.
- Do not union GitHub issues with kind `1621`, import GitHub issues into Nostr, or dual-write.
- Do not call `gh issue`, add a provider trait, or store a GitHub token.
- Authenticate only through installed `gh` and `gh auth status --hostname github.com`.
- Use `gh api` with one page, `per_page=100`, and no `--paginate`.
- Require `list_github_issues.state` to be exactly `open` or `closed`.
- Accept only positive `u64` issue numbers in Rust and positive safe decimal integers in TypeScript.
- Treat GitHub authors and assignees as logins plus avatar URLs, never as Nostr pubkeys.
- Never pass a GitHub login to `ProfileIdentityButton`, `ProfileAuthorName`, `normalizePubkey`, `useUsersBatchQuery`, `IssueAssigneesRow`, or Nostr assignment/comment mutations.
- Do not mount `IssueAssigneesRow` or `ForumComposer` on GitHub-hosted issue detail.
- Do not add Close or Reopen to Buzz-hosted issues.
- Close and reopen send only `{ "state": "closed" | "open" }`.
- Comments send only a trimmed non-empty `{ "body" }` through a tempfile passed to `gh api --input`.
- Labels and assignees add or remove one name or login per action and never replace the whole set.
- Assign me uses `GET /user` and never the Buzz pubkey.
- Percent-encode label names on DELETE.
- Send assignee logins only in JSON bodies, never as path segments.
- On GitHub writes, invalidate only the prefix `["project", project.id, "issues"]`.
- Do not invalidate `work-items` or `activity-summaries` for GitHub writes.
- GitHub list query keys are `["project", id, "issues", "open"]` and `["project", id, "issues", "closed"]`.
- Buzz list query keys stay `["project", id, "issues"]`.
- Open | Closed is GitHub-only, local tab state, default Open, not a URL parameter, and not persisted.
- After close, set the filter to Closed and keep `#N` selected.
- After reopen or create, set the filter to Open and keep or select `#N`.
- Clear `#N` only after the destination list query has finished fetching successfully and its loaded page does not contain `#N`.
- Failed writes do not change the Open | Closed filter.
- Failed comments do not clear composer text.
- Check GitHub list errors before empty-success rendering.
- Empty Open copy is `No open issues.`.
- Empty Closed copy is `No closed issues.`.
- Auth or CLI failure must not show either empty-state string.
- Use only `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, `github_issue_unavailable`, and `github_issues_failed` on this surface.
- Do not leak `github_merge_failed` or `github_state_failed` across the Tauri boundary.
- Do not run live GitHub requests in automated tests.
- Do not add production `unsafe`, `unwrap()`, or `expect()` calls.
- Add doc comments to every new exported TypeScript declaration and every new public Rust API.
- Use named rem-based text tokens and do not add arbitrary text sizes.
- Keep every edited file covered by `pnpm check:file-sizes` at or below 1,000 lines.
- `desktop/src/testing/e2eBridge.ts` is intentionally outside that ratchet and is already larger than 1,000 lines, so modify only its GitHub issue declarations, initialization, and invoke cases.
- Activate Hermit in every shell command with `. ./bin/activate-hermit && ...` or as the first line of the same shell block.
- Run commands from the repository root because shell working directories do not persist between tool calls.
- Run GitNexus `impact({ target, direction: "upstream" })` before editing each existing symbol when the MCP tools are available.
- Warn before proceeding if GitNexus reports HIGH or CRITICAL risk.
- Run GitNexus `detect_changes({ scope: "staged" })` before every commit and `detect_changes({ scope: "compare", base_ref: "main" })` before final handoff when the MCP tools are available.
- If GitNexus tools are unavailable, record that fact and use `git diff --stat`, `git diff --name-only`, and `git diff --check` as fallback scope evidence.
- Sign every commit with `git commit -s`.

## Resolved Implementation Decisions

- Keep list, create, and comment-list commands in `desktop/src-tauri/src/commands/project_github_issues.rs`.
- Put write and catalog commands in `desktop/src-tauri/src/commands/project_github_issue_writes.rs` because the existing module is 766 lines before this slice.
- Generalize `issue_json_input(title, body)` into `github_json_input(&serde_json::Value)` so every JSON write uses the same flushed `buzz-gh-` tempfile path.
- Add `html_url` to the comment wire and DTO, and map comments through one shared repo-bound validator on both list and create.
- Accept a comment URL only when it is exactly `https://github.com/{owner}/{repo}/issues/{number}#issuecomment-{id}` for the parsed repo, number, and comment id.
- Drop a listed comment whose URL is invalid, and reject a newly created comment whose URL is invalid with `github_issues_failed`.
- GitHub label POST and DELETE return a label array rather than an issue object, so a successful label mutation performs one follow-up GET for `/repos/{owner}/{repo}/issues/{number}` and returns that mapped issue DTO.
- Validate catalog colors as exactly six ASCII hexadecimal characters without `#`, and drop empty-name or invalid-color entries.
- Use a body-only textarea because `ForumComposer` always initializes mentions and media and always renders the corresponding controls.
- Keep label and assignee catalog queries stale for 60 seconds.
- Keep `["github", "authenticated-user"]` stale for the session with `staleTime: Number.POSITIVE_INFINITY` because the `gh` actor is machine-global rather than community-scoped.
- Do not add the authenticated-user query to `resetCommunityState()`.
- Lift Open | Closed state to `ProjectDetailScreen` because both its profile-prefetch query and `ProjectIssuesPanel` subscribe to the issues query.
- Reset the filter to Open when `repository.id` changes.
- Do not move the existing I1+I2 DTOs out of `desktop/src/shared/api/projectGit.ts`.
- Put only the new write DTOs and invoke wrappers in `desktop/src/shared/api/projectGithubIssueWrites.ts`, which avoids a circular re-export through `projectGit.ts`.
- `desktop/src-tauri/src/lib.rs` is 999 lines before this slice, so move its existing `tauri::generate_handler![...]` expression into `desktop/src-tauri/src/invoke.rs` before registering nine commands.
- The extracted handler has one exact generic signature and no fallback placeholder.

## Open Questions

### Label-write 404 ambiguity

GitHub uses HTTP 404 for both a missing issue and a missing label, and the label mutation response does not identify which resource was absent.

**Safe provisional default to implement:** Map a label POST or DELETE 404 to `github_issues_failed`, keep the issue selected, invalidate `["project", id, "github-labels"]`, and let the user retry from the refreshed catalog.
State, comment, and assignee write 404s map to `github_issue_unavailable`, toast `Issue not found.`, and clear the selection.
If product direction later requires distinguishing these cases, add an issue GET only on label-write 404 in a follow-up change.

## File Map

| File | Responsibility |
|---|---|
| `desktop/src-tauri/src/commands/project_github_issues.rs` | Shared `gh api`/JSON helpers, comment URL mapping, and closed-list regression coverage |
| Create `desktop/src-tauri/src/commands/project_github_issue_writes.rs` | Close/reopen, comment create, label and assignee catalogs/writes, and `GET /user` |
| `desktop/src-tauri/src/commands/mod.rs` | Declare and re-export the write command module |
| Create `desktop/src-tauri/src/invoke.rs` | Own the existing Tauri handler list plus the nine new commands |
| `desktop/src-tauri/src/lib.rs` | Declare `invoke` and install `desktop_invoke_handler()` |
| `desktop/src/shared/api/projectGit.ts` | Add `html_url` to the existing GitHub comment DTO only |
| Create `desktop/src/shared/api/projectGithubIssueWrites.ts` | New write/catalog DTOs and nine typed Tauri invoke wrappers |
| `desktop/src/features/projects/lib/projectGithubIssues.ts` | Accept Open or Closed state in the list loader and query inputs |
| `desktop/src/features/projects/lib/projectGithubIssues.test.mjs` | Protect state routing, numeric parsing, and comment DTO mapping |
| Create `desktop/src/features/projects/lib/projectGithubIssueWrites.ts` | Write targets, selection policy, host-routed comments, query keys, and GitHub query/mutation hooks |
| Create `desktop/src/features/projects/lib/projectGithubIssueWrites.test.mjs` | Protect host routing, no-Nostr behavior, invalidations, and selection timing |
| `desktop/src/features/projects/issueMutations.ts` | Reuse the shared host-aware issue invalidation-key helper for create |
| `desktop/src/features/projects/issueMutations.test.mjs` | Keep GitHub-only and Buzz-wide create invalidations green |
| `desktop/src/features/projects/hooks.ts` | Add list state to `useProjectIssuesQuery` and host-route the existing comment mutation |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Own/reset list state and set Open plus the returned number after create |
| `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx` | Pass list state and setter into the Issues panel |
| `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx` | Render filter/list/error states and clear selection only after a settled destination fetch |
| Create `desktop/src/features/projects/ui/GitHubIssueIdentity.tsx` | GitHub login identity and assignee facepile without pubkey helpers |
| Create `desktop/src/features/projects/ui/GitHubIssueStateFilter.tsx` | Accessible Open | Closed control |
| Create `desktop/src/features/projects/ui/GitHubIssueStateButton.tsx` | Close or Reopen with filter transition and issue-unavailable handling |
| Create `desktop/src/features/projects/ui/GitHubIssueCommentComposer.tsx` | Body-only textarea that preserves failed text |
| Create `desktop/src/features/projects/ui/GitHubIssueLabels.tsx` | Existing chips, removal buttons, and catalog add menu |
| Create `desktop/src/features/projects/ui/GitHubIssueAssignees.tsx` | Login identities, catalog picker, Assign me, and Unassign me |
| `desktop/src/features/projects/ui/GitHubProjectIssues.tsx` | Compose GitHub row/detail chrome and render Open or Closed status |
| `desktop/src/testing/e2eBridge.ts` | Mutable GitHub issue store plus read and write command stubs |
| `desktop/tests/e2e/github-issues.spec.ts` | Keep I1+I2 coverage while expecting the filter and body composer |
| Create `desktop/tests/e2e/github-issue-writes.spec.ts` | Full close/comment/label/assignee/reopen flow and failure behavior |
| `desktop/playwright.config.ts` | Register the new spec in the smoke project |

Do not modify `desktop/src/features/projects/issueAssignments.ts`, `desktop/src/features/projects/ui/IssueAssigneesRow.tsx`, or `desktop/src/features/forum/ui/ForumComposer.tsx`.
Keep `desktop/tests/e2e/project-issue-comments.spec.ts` green without changing its Buzz-hosted behavior.

## Required Impact Checks

GitNexus was unavailable during plan repair.
At execution time, try each named upstream impact check once before the first edit in that task.
If the tools remain unavailable, record that result and continue with direct source inspection and the documented diff checks.

- Task 1 targets `github_api_json`, `issue_json_input`, `list_github_issue_comments_with`, `GitHubIssueCommentDto`, and `list_github_issues_with`.
- Task 2 targets `map_issue`, `list_github_issue_comments_with`, `GhRunner::run_with_limit`, and `remap_issues_error`.
- Task 3 targets `GitHubRepoRef::slug`, `github_api_json`, and the `percent_encoding` call site.
- Task 4 targets `tauri::generate_handler!` in `lib.rs`, the `commands` re-exports, and `parseProjectPullRequestMergeError`.
- Task 5 targets `fetchProjectIssuesWith`, `useProjectIssuesQuery`, `ProjectDetailScreen`, `WorkspaceTabs`, and `ProjectIssuesPanel`.
- Task 6 targets `createProjectIssueComment`, `useCreateProjectIssueCommentMutation`, `useCreateProjectIssueMutation`, and `projectIssueInvalidationKeys`.
- Task 7 targets `GitHubIssueDetail`, `GitHubIssueRow`, and `GitHubRepoStateRecovery`.
- Task 8 targets the E2E invoke switch, the E2E `Window` declarations, and Playwright smoke `testMatch`.

---

### Task 1: Harden the shared native GitHub issue boundary

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_issues.rs`

**Interfaces:**

- Consumes: existing `GhRunner`, `GitHubRepoRef`, `ISSUE_ITEM_JQ`, `ISSUE_COMMENTS_JQ`, and `remap_issues_error`.
- Produces: `pub(crate) fn github_api_json<T>`, `pub(crate) fn github_json_input`, `pub(crate) fn map_issue`, `pub(crate) fn map_issue_comment`, `pub(crate) fn is_issue_comment_html_url`, `pub(crate) ISSUE_ITEM_JQ`, `pub(crate) GitHubIssueWire`, `pub(crate) GitHubIssueCommentWire`, and `html_url` on `GitHubIssueCommentDto`.

- [ ] **Step 1: Run the Task 1 impact checks.**

Report direct callers, affected processes, and risk before editing.
Stop and warn if a result is HIGH or CRITICAL.

- [ ] **Step 2: Write failing URL, mapping, and closed-list tests.**

Add these tests beside `comments_keep_projected_github_order`, and add valid `html_url` fields to every existing comment fixture.

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
        assert_eq!(is_issue_comment_html_url(&repo, raw, number, comment_id), expected, "{raw}");
    }
}

#[cfg(unix)]
#[test]
fn comments_drop_foreign_html_url() {
    let output = json!([{
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

#[cfg(unix)]
#[test]
fn list_sends_state_closed() {
    let output = json!([projected_issue(7, false)]);
    let (dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let page = list_github_issues_with(&gh, "https://github.com/acme/app", "closed")
        .expect("list");
    assert_eq!(page.issues[0].number, 7);
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains(
        "/repos/acme/app/issues?state=closed&per_page=100&sort=updated&direction=desc"
    ));
}
```

- [ ] **Step 3: Run the focused test and verify RED.**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib accepts_only_repo_bound_comment_urls
```

Expected: compilation fails because `is_issue_comment_html_url` does not exist.

- [ ] **Step 4: Implement the minimal shared boundary changes.**

Change `ISSUE_COMMENTS_JQ` so every projected comment includes `html_url`.
Add `html_url: String` to `GitHubIssueCommentDto` and `GitHubIssueCommentWire`.
Replace `issue_json_input(title, body)` with the following general helper and update `create_github_issue_with` to pass `{ "title", "body" }`.

```rust
pub(crate) fn github_json_input(
    value: &serde_json::Value,
) -> Result<tempfile::NamedTempFile, ProjectPullRequestMergeError>
```

Make `github_api_json`, `map_issue`, `ISSUE_ITEM_JQ`, `GitHubIssueWire`, and `GitHubIssueCommentWire` visible to the sibling module with `pub(crate)`.
Do not expose wire fields because sibling code consumes them only through `map_issue` and `map_issue_comment`.
Implement the shared comment mapper with this signature.

```rust
pub(crate) fn map_issue_comment(
    repo: &GitHubRepoRef,
    number: u64,
    comment: GitHubIssueCommentWire,
) -> Option<GitHubIssueCommentDto>
```

Implement `is_issue_comment_html_url` with `url::Url`.
Require HTTPS, host `github.com`, no credentials/query/port, no `%` or backslash, case-insensitive owner and repo, literal `issues`, matching number, and fragment exactly `issuecomment-{id}`.
Call `map_issue_comment` from `list_github_issue_comments_with` so invalid comments are dropped.
The closed-state implementation already exists, so the new test is regression coverage rather than a production behavior change.

- [ ] **Step 5: Run the issue-module tests and verify GREEN.**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
```

Expected: all existing issue tests plus the new URL, mapping, and closed-state tests pass.

- [ ] **Step 6: Format, inspect scope, and commit.**

Run staged GitNexus change detection when available before committing.

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_issues.rs
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): validate GitHub issue comment URLs"
```

---

### Task 2: Close, reopen, and create comments in Rust

**Files:**

- Create: `desktop/src-tauri/src/commands/project_github_issue_writes.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs`

**Interfaces:**

- Consumes: Task 1 helpers, `GitHubRepoRef`, `GhRunner`, and `ProjectPullRequestMergeError`.
- Produces: `update_github_issue_state_with`, `create_github_issue_comment_with`, and their runner-injected wrappers.

- [ ] **Step 1: Run the Task 2 impact checks.**

- [ ] **Step 2: Declare the module and write failing tests before production functions.**

Add `mod project_github_issue_writes;` beside `mod project_github_issues;` without re-exporting it yet.
Create a Unix `fake_gh_input` fixture that logs `$*`, copies the file following `--input` into `input.json`, and returns the supplied projected JSON.
Add these concrete tests.

```rust
#[cfg(unix)]
#[test]
fn state_rejects_unknown_and_zero_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
    assert_eq!(
        error_code(&update_github_issue_state_with(
            &gh,
            "https://github.com/acme/app",
            42,
            "done",
        ).expect_err("state")),
        "github_issues_failed",
    );
    assert_eq!(
        error_code(&update_github_issue_state_with(
            &gh,
            "https://github.com/acme/app",
            0,
            "closed",
        ).expect_err("number")),
        "github_issues_failed",
    );
}

#[cfg(unix)]
#[test]
fn close_patches_state_only() {
    let output = projected_issue(42, "closed");
    let (dir, path) = fake_gh_input(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let issue = update_github_issue_state_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "closed",
    ).expect("close");
    assert_eq!(issue.state, "closed");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
        ).expect("json"),
        json!({ "state": "closed" }),
    );
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("api --hostname github.com --method PATCH /repos/acme/app/issues/42"));
}

#[cfg(unix)]
#[test]
fn comment_posts_trimmed_body_and_validates_html_url() {
    let output = json!({
        "id": 9,
        "body": "Looks good",
        "html_url": "https://github.com/acme/app/issues/42#issuecomment-9",
        "created_at": "2026-01-02T03:04:05Z",
        "user": { "login": "ada", "avatar_url": "" }
    });
    let (dir, path) = fake_gh_input(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let comment = create_github_issue_comment_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "  Looks good  ",
    ).expect("comment");
    assert_eq!(comment.id, 9);
    let input: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
    ).expect("json");
    assert_eq!(input, json!({ "body": "Looks good" }));
}
```

Add the remaining boundary tests with these exact assertions.

```rust
#[cfg(unix)]
#[test]
fn comment_rejects_blank_body_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
    let error = create_github_issue_comment_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "   ",
    ).expect_err("blank");
    assert_eq!(error_code(&error), "github_issues_failed");
}

#[cfg(unix)]
#[test]
fn comment_rejects_foreign_html_url() {
    let output = json!({
        "id": 9,
        "body": "Looks good",
        "html_url": "https://evil.example/acme/app/issues/42#issuecomment-9",
        "created_at": "2026-01-02T03:04:05Z",
        "user": { "login": "ada", "avatar_url": "" }
    });
    let (_dir, path) = fake_gh_input(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = create_github_issue_comment_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "Looks good",
    ).expect_err("foreign URL");
    assert_eq!(error_code(&error), "github_issues_failed");
}

#[cfg(unix)]
#[test]
fn writes_reject_buzz_url_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
    let error = update_github_issue_state_with(
        &gh,
        "https://relay.example/git/owner/app",
        42,
        "closed",
    ).expect_err("host");
    assert_eq!(error_code(&error), "github_issues_failed");
}

#[test]
fn write_remap_turns_issue_404_into_issue_unavailable() {
    let error = ProjectPullRequestMergeError::new("github_repo_unavailable", "gh: HTTP 404");
    assert_eq!(
        error_code(&remap_issue_write_error(error, "Not Found")),
        "github_issue_unavailable",
    );
}
```

Define local `error_code`, `projected_issue`, `fake_gh_input`, and path imports inside this new module's `#[cfg(test)]` block because sibling-module test helpers are private.

- [ ] **Step 3: Run one test and verify RED.**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib close_patches_state_only
```

Expected: compilation fails because `update_github_issue_state_with` does not exist.

- [ ] **Step 4: Implement state and comment writes.**

Use these exact production signatures.

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

Validate unknown state or blank body first, then `number == 0`, then `GitHubRepoRef::parse`, then auth, and only then run `gh api`.
PATCH state with `ISSUE_ITEM_JQ`, map the returned wire through `map_issue`, and reject an invalid returned issue as `github_issues_failed`.
POST comments with a dedicated comment-item jq projection that includes `html_url`, then map through `map_issue_comment`.
Implement a private `remap_issue_write_error` that preserves CLI/auth codes, turns an issue-targeted 404 or Not Found diagnostic into `github_issue_unavailable`, preserves non-rate 403 as `github_repo_unavailable`, and maps every other write failure to `github_issues_failed`.
Add one `*_with_runner` wrapper per function so Task 4 can test missing discovery without invoking async Tauri code.

- [ ] **Step 5: Run the write-module tests and verify GREEN.**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issue_writes
```

- [ ] **Step 6: Format, inspect scope, and commit.**

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_issue_writes.rs \
  desktop/src-tauri/src/commands/mod.rs
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): close and comment on GitHub issues"
```

---

### Task 3: Add label, assignee, and authenticated-user native operations

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_issue_writes.rs`

**Interfaces:**

- Consumes: Tasks 1 and 2 helpers plus `percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC}`.
- Produces: label and assignee DTOs, catalog/write functions, and `get_github_authenticated_user_with`.

- [ ] **Step 1: Run the Task 3 impact checks.**

- [ ] **Step 2: Write failing catalog and mutation tests.**

Add the following tests with a path-dispatching fake `gh` fixture so the label mutation and follow-up issue GET can return different JSON values.

```rust
#[cfg(unix)]
#[test]
fn label_delete_percent_encodes_path_and_refetches_issue() {
    let (dir, path) = fake_gh_dispatch();
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let issue = remove_github_issue_label_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "good first #issue",
    ).expect("remove label");
    assert_eq!(issue.number, 42);
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains(
        "/repos/acme/app/issues/42/labels/good%20first%20%23issue"
    ));
    assert!(calls.contains("--method GET /repos/acme/app/issues/42"));
    assert!(!calls.contains("labels/good first"));
}

#[cfg(unix)]
#[test]
fn assignee_add_sends_login_in_json_body() {
    let (dir, path) = fake_gh_dispatch();
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    add_github_issue_assignees_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "linus",
    ).expect("assign");
    let input: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
    ).expect("json");
    assert_eq!(input, json!({ "assignees": ["linus"] }));
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("/repos/acme/app/issues/42/assignees"));
    assert!(!calls.contains("/assignees/linus"));
}

#[test]
fn label_catalog_accepts_only_nonempty_names_and_six_hex_colors() {
    let labels = map_repo_labels(vec![
        label_wire("bug", "d73a4a"),
        label_wire("docs", "0075ca"),
        label_wire("bad", "red"),
        label_wire("hash", "#d73a4a"),
        label_wire("   ", "ffffff"),
    ]);
    assert_eq!(
        labels.into_iter().map(|label| (label.name, label.color)).collect::<Vec<_>>(),
        vec![("bug".into(), "d73a4a".into()), ("docs".into(), "0075ca".into())],
    );
}
```

Add the remaining validation and error tests with these exact results.

```rust
#[cfg(unix)]
#[test]
fn label_rejects_blank_name_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
    let error = add_github_issue_labels_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "   ",
    ).expect_err("blank");
    assert_eq!(error_code(&error), "github_issues_failed");
}

#[cfg(unix)]
#[test]
fn assignee_rejects_blank_login_and_zero_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
    let blank = add_github_issue_assignees_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "   ",
    ).expect_err("blank");
    let zero = remove_github_issue_assignee_with(
        &gh,
        "https://github.com/acme/app",
        0,
        "linus",
    ).expect_err("zero");
    assert_eq!(error_code(&blank), "github_issues_failed");
    assert_eq!(error_code(&zero), "github_issues_failed");
}

#[cfg(unix)]
#[test]
fn user_maps_login_and_avatar() {
    let output = json!({ "login": "ada", "avatar_url": "https://example.com/ada" });
    let (_dir, path) = fake_gh_input(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let user = get_github_authenticated_user_with(&gh).expect("user");
    assert_eq!(user.login, "ada");
    assert_eq!(user.avatar_url, "https://example.com/ada");
}

#[cfg(unix)]
#[test]
fn catalog_404_is_repo_unavailable() {
    let (_dir, path) = fake_gh_input(&json!([]), 1, "gh: HTTP 404");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = list_github_repo_labels_with(&gh, "https://github.com/acme/app")
        .expect_err("missing repo");
    assert_eq!(error_code(&error), "github_repo_unavailable");
}

#[cfg(unix)]
#[test]
fn label_write_404_is_issues_failed() {
    let (_dir, path) = fake_gh_input(&json!([]), 1, "gh: HTTP 404");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = remove_github_issue_label_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "missing",
    ).expect_err("missing label");
    assert_eq!(error_code(&error), "github_issues_failed");
}
```

Cover DELETE bodies separately so neither assignee path can regress to `/assignees/{login}`.

```rust
#[cfg(unix)]
#[test]
fn assignee_remove_sends_login_in_json_body() {
    let output = projected_issue(42, "open");
    let (dir, path) = fake_gh_input(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    remove_github_issue_assignee_with(
        &gh,
        "https://github.com/acme/app",
        42,
        "linus",
    ).expect("unassign");
    let input: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
    ).expect("json");
    assert_eq!(input, json!({ "assignees": ["linus"] }));
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("--method DELETE /repos/acme/app/issues/42/assignees"));
    assert!(!calls.contains("/assignees/linus"));
}
```

- [ ] **Step 3: Run one test and verify RED.**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib label_delete_percent_encodes_path_and_refetches_issue
```

Expected: compilation fails because `remove_github_issue_label_with` does not exist.

- [ ] **Step 4: Implement catalogs and mutations.**

Use these exact signatures.

```rust
pub(crate) fn list_github_repo_labels_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<Vec<GitHubRepoLabelDto>, ProjectPullRequestMergeError>

pub(crate) fn add_github_issue_labels_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    name: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>

pub(crate) fn remove_github_issue_label_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    name: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>

pub(crate) fn list_github_repo_assignees_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<Vec<GitHubIssueUserDto>, ProjectPullRequestMergeError>

pub(crate) fn add_github_issue_assignees_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    login: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>

pub(crate) fn remove_github_issue_assignee_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    login: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError>

pub(crate) fn get_github_authenticated_user_with(
    gh: &GhRunner,
) -> Result<GitHubIssueUserDto, ProjectPullRequestMergeError>
```

List labels with `/repos/{slug}/labels?per_page=100` and project only `{name, color}`.
List assignees with `/repos/{slug}/assignees?per_page=100` and project only `{login, avatar_url}`.
Reject blank names/logins and zero issue numbers before auth or `gh`.
Encode the DELETE label path segment with `utf8_percent_encode(name, NON_ALPHANUMERIC)`.
POST label JSON as `{ "labels": [name] }`, ignore the returned label array after verifying valid JSON, then GET the issue with `ISSUE_ITEM_JQ` and `map_issue`.
DELETE the encoded label path without a body, then perform the same issue GET.
Use the provisional label-specific 404 remapper from Open Questions on the label mutation call only.
Use the issue-write remapper on the follow-up issue GET and on both assignee mutations.
POST or DELETE assignees at `/issues/{number}/assignees` with `{ "assignees": [login] }` through `github_json_input`.
GET `/user`, reject an empty returned login, and preserve the returned avatar URL.
Add one runner-injected wrapper per function for Task 4.

- [ ] **Step 5: Run all write-module tests and verify GREEN.**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issue_writes
```

- [ ] **Step 6: Format, inspect scope, and commit.**

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_issue_writes.rs
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): write GitHub issue labels and assignees"
```

---

### Task 4: Register Tauri commands and add typed write wrappers

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_issue_writes.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs`
- Create: `desktop/src-tauri/src/invoke.rs`
- Modify: `desktop/src-tauri/src/lib.rs`
- Modify: `desktop/src/shared/api/projectGit.ts`
- Create: `desktop/src/shared/api/projectGithubIssueWrites.ts`

**Interfaces:**

- Consumes: every runner-injected function from Tasks 2 and 3 plus `GhRunner::discover`.
- Produces: nine Tauri commands and nine camelCase TypeScript invoke wrappers.

- [ ] **Step 1: Run the Task 4 impact checks.**

- [ ] **Step 2: Write failing discovery-wrapper tests.**

```rust
#[test]
fn state_wrapper_maps_missing_discovered_cli() {
    let error = update_github_issue_state_with_runner(
        "https://github.com/acme/app".to_string(),
        42,
        "closed".to_string(),
        GhRunner::from_resolved(None),
    ).expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}

#[test]
fn user_wrapper_maps_missing_discovered_cli() {
    let error = get_github_authenticated_user_with_runner(GhRunner::from_resolved(None))
        .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}
```

- [ ] **Step 3: Run one wrapper test and verify RED.**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib state_wrapper_maps_missing_discovered_cli
```

Expected: compilation fails because the wrapper does not exist yet.

- [ ] **Step 4: Add runner wrappers and async Tauri commands.**

Use the same `spawn_blocking` plus `GhRunner::discover()` pattern as `list_github_issues`.
Add doc comments and these exact async signatures.

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

Add `pub use project_github_issue_writes::*;` in `desktop/src-tauri/src/commands/mod.rs` only after the commands exist.

- [ ] **Step 5: Extract and extend the Tauri handler with one exact signature.**

Move the entire existing `tauri::generate_handler![...]` expression from `desktop/src-tauri/src/lib.rs` into a function with this exact signature in `desktop/src-tauri/src/invoke.rs`.

```rust
pub(crate) fn desktop_invoke_handler<R: tauri::Runtime>()
    -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static
```

Use the moved macro expression as the function body without deleting, renaming, or reordering any existing command identifier.
Insert the nine GitHub issue write command identifiers immediately after `list_github_issue_comments`.
Import the same crate-local command namespaces that currently make the identifiers resolve in `lib.rs`.
Add `mod invoke;` beside `mod commands;` in `lib.rs`.
Replace only `.invoke_handler(tauri::generate_handler![...])` with `.invoke_handler(invoke::desktop_invoke_handler())`.
Run `cargo check` in Step 7 to prove that no command was dropped and the generic runtime is inferred.

- [ ] **Step 6: Add the focused TypeScript write API.**

Add `html_url: string` to the existing `GithubIssueCommentDto` in `desktop/src/shared/api/projectGit.ts`.
Create `desktop/src/shared/api/projectGithubIssueWrites.ts` and import existing `GithubIssueDto`, `GithubIssueCommentDto`, `GithubIssueUserDto`, and `parseProjectPullRequestMergeError` from `./projectGit`.
Do not re-export this new module from `projectGit.ts`.
Define `GithubRepoLabelDto = { name: string; color: string }` and wrappers named `updateGithubIssueState`, `createGithubIssueComment`, `listGithubRepoLabels`, `addGithubIssueLabels`, `removeGithubIssueLabel`, `listGithubRepoAssignees`, `addGithubIssueAssignees`, `removeGithubIssueAssignee`, and `getGithubAuthenticatedUser`.
Each wrapper passes camelCase arguments to the snake_case Tauri command and rethrows `parseProjectPullRequestMergeError(error) ?? error`.
Use `state: "open" | "closed"`, positive `number: number`, `name: string`, `login: string`, and `body: string` without renaming payload fields.

- [ ] **Step 7: Run Rust tests, compile the handler, and typecheck.**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issue_writes
. ./bin/activate-hermit && cargo check --manifest-path desktop/src-tauri/Cargo.toml --lib
. ./bin/activate-hermit && cd desktop && pnpm typecheck && pnpm check:file-sizes
```

Expected: the extracted handler compiles, all nine commands resolve, `lib.rs` is well below 1,000 lines, and `projectGit.ts` remains below 1,000 lines.

- [ ] **Step 8: Format, inspect scope, and commit.**

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
cd desktop && pnpm exec biome check --write src/shared/api/projectGit.ts src/shared/api/projectGithubIssueWrites.ts
git add desktop/src-tauri/src/commands/project_github_issue_writes.rs \
  desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/invoke.rs \
  desktop/src-tauri/src/lib.rs \
  desktop/src/shared/api/projectGit.ts \
  desktop/src/shared/api/projectGithubIssueWrites.ts
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): expose GitHub issue write commands"
```

---

### Task 5: Add Open or Closed query state and settled selection policy

**Files:**

- Modify: `desktop/src/features/projects/lib/projectGithubIssues.ts`
- Modify: `desktop/src/features/projects/lib/projectGithubIssues.test.mjs`
- Create: `desktop/src/features/projects/lib/projectGithubIssueWrites.ts`
- Create: `desktop/src/features/projects/lib/projectGithubIssueWrites.test.mjs`
- Create: `desktop/src/features/projects/ui/GitHubIssueStateFilter.tsx`
- Modify: `desktop/src/features/projects/hooks.ts`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx`
- Modify: `desktop/src/testing/e2eBridge.ts`
- Modify: `desktop/tests/e2e/github-issues.spec.ts`

**Interfaces:**

- Consumes: existing `listGithubIssues`, `parseGithubIssueNumber`, and `isGitHubCloneUrl`.
- Produces: `GithubIssueListState`, state-aware `fetchProjectIssuesWith`, state-aware `useProjectIssuesQuery`, `nextGithubIssueListState`, `selectedGithubIssueAfterListLoad`, and a filter controlled by `ProjectDetailScreen`.

- [ ] **Step 1: Run the Task 5 impact checks.**

- [ ] **Step 2: Write failing state-routing and settled-selection tests.**

Change the loader type to accept both states in the test fixture, pass `"closed"` as the third argument to `fetchProjectIssuesWith`, and assert the loader receives it.

```js
test("host routing forwards closed state only to GitHub", async () => {
  let received = null;
  await fetchProjectIssuesWith(
    {
      id: "p1",
      repoAddress: REPO_ADDRESS,
      cloneUrls: ["https://github.com/acme/app"],
    },
    {
      loadGithub: async (input) => {
        received = input;
        return { issues: [], has_more: false };
      },
      loadBuzz: async () => {
        throw new Error("Buzz loader must not run");
      },
    },
    "closed",
  );
  assert.deepEqual(received, {
    cloneUrl: "https://github.com/acme/app",
    state: "closed",
  });
});
```

Create `projectGithubIssueWrites.test.mjs` and write the pure state/selection policy before UI wiring, using the exact expectations below.

```js
assert.equal(selectedGithubIssueAfterListLoad({
  selectedIssueId: "42",
  issueIds: [],
  isSuccess: true,
  isFetching: true,
}), "42");
assert.equal(selectedGithubIssueAfterListLoad({
  selectedIssueId: "42",
  issueIds: [],
  isSuccess: true,
  isFetching: false,
}), null);
assert.equal(nextGithubIssueListState("close"), "closed");
assert.equal(nextGithubIssueListState("reopen"), "open");
assert.equal(nextGithubIssueListState("create"), "open");
```

- [ ] **Step 3: Add a failing filter assertion to the existing smoke spec.**

Before production UI changes, update the E2E bridge list stub to accept `input.state === "closed"` and return an empty successful page for that state.
In `github-issues.spec.ts`, assert the Open button is selected, click Closed, expect `No closed issues.`, click Open, and continue the existing list/create/detail assertions.

```ts
await expect(page.getByTestId("project-github-issue-filter-open")).toHaveAttribute("aria-selected", "true");
await page.getByTestId("project-github-issue-filter-closed").click();
await expect(page.getByText("No closed issues.", { exact: true })).toBeVisible();
await page.getByTestId("project-github-issue-filter-open").click();
await expect(page.getByTestId("project-github-issue-row").first()).toContainText("#42");
```

Immediately before the existing New issue interaction, switch to Closed again.
After create succeeds, assert the Open tab has `aria-selected="true"` and the returned `#43` detail is selected.
This proves create changes the filter rather than depending on the user already being on Open.

- [ ] **Step 4: Run unit and smoke tests and verify RED.**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectGithubIssueWrites.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts
```

Expected: the unit test fails because the list function is still open-only, and the smoke test fails because the filter does not exist.

- [ ] **Step 5: Implement state routing and the controlled filter.**

Export `type GithubIssueListState = "open" | "closed"` from `projectGithubIssues.ts`.
Change the exact loader signature to `fetchProjectIssuesWith(project, loaders, state: GithubIssueListState = "open")`.
Pass `{ cloneUrl, state }` only to the GitHub loader, and leave Buzz loading unfiltered.
Change `useProjectIssuesQuery(project, listState: GithubIssueListState = "open")` so GitHub keys include state and Buzz keys do not.
Create `projectGithubIssueWrites.ts` with the following policy-only interface, leaving API calls and React Query hooks for Task 6.

```ts
export type GithubIssueWriteAction = "close" | "reopen" | "create";

export function nextGithubIssueListState(
  action: GithubIssueWriteAction,
): GithubIssueListState;

export function selectedGithubIssueAfterListLoad(input: {
  selectedIssueId: string | null;
  issueIds: readonly string[];
  isSuccess: boolean;
  isFetching: boolean;
}): string | null;
```
In `ProjectDetailScreen`, add `githubIssueListState`, reset it to `"open"` when `repository?.id` changes, and pass it to the screen-level issues query.
After a successful create, set list state to Open and select the returned issue id without calling `issuesQuery.refetch()`.
Pass `githubIssueListState` and `onGithubIssueListStateChange` through `WorkspaceTabs` to `ProjectIssuesPanel`.
In the panel, subscribe with the passed state and use `selectedGithubIssueAfterListLoad` only when `githubHosted` is true.
The selection effect must include `issuesQuery.isFetching` so stale cached data cannot clear a new or moved issue before the destination fetch settles.
Create `GitHubIssueStateFilter` as a `role="tablist"` with Open and Closed `role="tab"` buttons, `aria-selected`, and these test ids: `project-github-issue-state-filter`, `project-github-issue-filter-open`, and `project-github-issue-filter-closed`.
Keep the filter visible for GitHub loading, empty, list, and selected-detail states.
Render list-level recovery before empty copy, and render `No open issues.` or `No closed issues.` only on successful empty pages.
Use `More open issues exist on GitHub.` or `More closed issues exist on GitHub.` for the bounded-page note.

- [ ] **Step 6: Run unit, smoke, type, size, and text checks and verify GREEN.**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectGithubIssueWrites.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts
. ./bin/activate-hermit && cd desktop && pnpm typecheck && pnpm check:file-sizes && pnpm check:px-text
```

- [ ] **Step 7: Format, inspect scope, and commit.**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/features/projects/lib/projectGithubIssues.ts \
  src/features/projects/lib/projectGithubIssues.test.mjs \
  src/features/projects/lib/projectGithubIssueWrites.ts \
  src/features/projects/lib/projectGithubIssueWrites.test.mjs \
  src/features/projects/ui/GitHubIssueStateFilter.tsx \
  src/features/projects/hooks.ts \
  src/features/projects/ui/ProjectDetailScreen.tsx \
  src/features/projects/ui/ProjectWorkspaceTabs.tsx \
  src/features/projects/ui/ProjectIssuesPanel.tsx \
  src/testing/e2eBridge.ts \
  tests/e2e/github-issues.spec.ts
git add desktop/src/features/projects/lib/projectGithubIssues.ts \
  desktop/src/features/projects/lib/projectGithubIssues.test.mjs \
  desktop/src/features/projects/lib/projectGithubIssueWrites.ts \
  desktop/src/features/projects/lib/projectGithubIssueWrites.test.mjs \
  desktop/src/features/projects/ui/GitHubIssueStateFilter.tsx \
  desktop/src/features/projects/hooks.ts \
  desktop/src/features/projects/ui/ProjectDetailScreen.tsx \
  desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx \
  desktop/src/features/projects/ui/ProjectIssuesPanel.tsx \
  desktop/src/testing/e2eBridge.ts \
  desktop/tests/e2e/github-issues.spec.ts
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): add GitHub issue list state"
```

---

### Task 6: Add host-routed write policy and React Query hooks

**Files:**

- Modify: `desktop/src/features/projects/lib/projectGithubIssueWrites.ts`
- Modify: `desktop/src/features/projects/lib/projectGithubIssueWrites.test.mjs`
- Modify: `desktop/src/features/projects/hooks.ts`
- Modify: `desktop/src/features/projects/issueMutations.ts`
- Modify: `desktop/src/features/projects/issueMutations.test.mjs`

**Interfaces:**

- Consumes: Task 4 invoke wrappers, `parseGithubIssueNumber`, `isGitHubCloneUrl`, existing Buzz comment publishing, and TanStack Query.
- Produces: target/policy helpers, catalog queries, authenticated-user query, state/label/assignee mutations, and a host-routed existing comment mutation.

- [ ] **Step 1: Run the Task 6 impact checks.**

- [ ] **Step 2: Write failing pure policy and host-routing tests.**

Create `projectGithubIssueWrites.test.mjs` with these concrete cases.

```js
test("write targets require GitHub host and a positive safe issue number", () => {
  assert.deepEqual(
    githubIssueWriteTarget(
      { cloneUrls: ["https://github.com/acme/app"] },
      "42",
    ),
    { cloneUrl: "https://github.com/acme/app", number: 42 },
  );
  assert.equal(githubIssueWriteTarget({ cloneUrls: [BUZZ_URL] }, "42"), null);
  assert.equal(githubIssueWriteTarget({ cloneUrls: ["https://github.com/acme/app"] }, "0"), null);
});

test("comment routing never publishes Nostr for GitHub", async () => {
  const calls = { github: 0, buzz: 0 };
  await createProjectIssueCommentWith(
    {
      project: { cloneUrls: ["https://github.com/acme/app"] },
      issue: { id: "42" },
      content: "  Looks good  ",
      mediaTags: [["imeta", "ignored"]],
      mentionPubkeys: ["a".repeat(64)],
    },
    {
      createGithub: async (input) => {
        calls.github += 1;
        assert.deepEqual(input, {
          cloneUrl: "https://github.com/acme/app",
          number: 42,
          body: "Looks good",
        });
      },
      publishBuzz: async () => {
        calls.buzz += 1;
      },
    },
  );
  assert.deepEqual(calls, { github: 1, buzz: 0 });
});

test("comment routing preserves the Buzz publisher input", async () => {
  let published = null;
  await createProjectIssueCommentWith(
    {
      project: { cloneUrls: [BUZZ_URL] },
      issue: { id: "e".repeat(64) },
      content: "Buzz comment",
      mediaTags: [["imeta", "x"]],
      mentionPubkeys: ["a".repeat(64)],
    },
    {
      createGithub: async () => {
        throw new Error("GitHub must not run");
      },
      publishBuzz: async (input) => {
        published = input;
      },
    },
  );
  assert.equal(published.content, "Buzz comment");
  assert.deepEqual(published.mediaTags, [["imeta", "x"]]);
  assert.deepEqual(published.mentionPubkeys, ["a".repeat(64)]);
});
```

Also assert that blank comments fail before either backend, close maps to `"closed"`, reopen/create map to `"open"`, GitHub invalidation is exactly one prefix, Buzz invalidation preserves all three existing keys, and selection clearing waits for `isSuccess && !isFetching`.

- [ ] **Step 3: Run the focused tests and verify RED.**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssueWrites.test.mjs src/features/projects/issueMutations.test.mjs
```

Expected: the new write-target, routing, invalidation, and hook exports are missing from the policy-only module created in Task 5.

- [ ] **Step 4: Implement pure policy and query-key helpers.**

Export these exact declarations from `projectGithubIssueWrites.ts`.

```ts
export function githubIssueWriteTarget(project, issueId): { cloneUrl: string; number: number } | null;
export function githubIssueWriteInvalidationKey(projectId: string): readonly ["project", string, "issues"];
export function projectIssueWriteInvalidationKeys(project): readonly unknown[][];
export async function createProjectIssueCommentWith(input, backends): Promise<void>;
```

Return `null` from `githubIssueWriteTarget` for a Buzz host or invalid number and throw a clear input error at every mutation call site before invoking Tauri.
Trim comment content once in `createProjectIssueCommentWith`.
Pass mentions and media only to `publishBuzz`, and pass only `{ cloneUrl, number, body }` to `createGithub`.
Update `issueMutations.ts` to use `projectIssueWriteInvalidationKeys` while preserving the exported `projectIssueInvalidationKeys` name for current callers and tests.

- [ ] **Step 5: Add exact query and mutation hooks.**

Add these hooks in the new file.

```ts
useGithubRepoLabelsQuery(project, enabled)
// key: ["project", project.id, "github-labels"], staleTime: 60_000

useGithubRepoAssigneesQuery(project, enabled)
// key: ["project", project.id, "github-assignees"], staleTime: 60_000

useGithubAuthenticatedUserQuery(enabled)
// key: ["github", "authenticated-user"], staleTime: Number.POSITIVE_INFINITY

useUpdateGithubIssueStateMutation(project, issueId)
useAddGithubIssueLabelMutation(project, issueId)
useRemoveGithubIssueLabelMutation(project, issueId)
useAddGithubIssueAssigneeMutation(project, issueId)
useRemoveGithubIssueAssigneeMutation(project, issueId)
```

Every mutation hook derives its target with `githubIssueWriteTarget` and throws before the API wrapper when the target is null.
Do not invalidate inside the state, label, or assignee hooks because Task 7 must order filter/selection changes before invalidation.
In `hooks.ts`, keep the existing Buzz comment function body unchanged and route `useCreateProjectIssueCommentMutation` through `createProjectIssueCommentWith` with `createGithubIssueComment` and that Buzz function as injected backends.
Change comment `onSuccess` to invalidate `projectIssueWriteInvalidationKeys(project)` so GitHub comments do not invalidate global Nostr work-item/activity queries.

- [ ] **Step 6: Run unit, type, and file-size checks and verify GREEN.**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssueWrites.test.mjs src/features/projects/issueMutations.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm typecheck && pnpm check:file-sizes
```

- [ ] **Step 7: Format, inspect scope, and commit.**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/features/projects/lib/projectGithubIssueWrites.ts \
  src/features/projects/lib/projectGithubIssueWrites.test.mjs \
  src/features/projects/hooks.ts \
  src/features/projects/issueMutations.ts \
  src/features/projects/issueMutations.test.mjs
git add desktop/src/features/projects/lib/projectGithubIssueWrites.ts \
  desktop/src/features/projects/lib/projectGithubIssueWrites.test.mjs \
  desktop/src/features/projects/hooks.ts \
  desktop/src/features/projects/issueMutations.ts \
  desktop/src/features/projects/issueMutations.test.mjs
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "feat(projects): route GitHub issue writes"
```

---

### Task 7: Build GitHub-only write chrome

**Files:**

- Create: `desktop/src/features/projects/ui/GitHubIssueIdentity.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueStateButton.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueCommentComposer.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueLabels.tsx`
- Create: `desktop/src/features/projects/ui/GitHubIssueAssignees.tsx`
- Modify: `desktop/src/features/projects/ui/GitHubProjectIssues.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx`
- Modify: `desktop/tests/e2e/github-issues.spec.ts`

**Interfaces:**

- Consumes: Task 6 hooks/helpers, `mapGithubIssueToProjectIssue`, `GitHubRepoStateRecovery`, `UserAvatar`, shared `Button`, and shared dropdown-menu primitives.
- Produces: GitHub-only Close/Reopen, textarea comment, label, and assignee controls that never import a pubkey helper.

- [ ] **Step 1: Run the Task 7 impact checks.**

- [ ] **Step 2: Change the existing smoke composer assertion and verify RED.**

Replace the hidden-composer assertion in `github-issues.spec.ts` with the exact body-only contract.

```ts
const composer = page.getByTestId("project-issue-comment-composer");
await expect(composer).toBeVisible();
await expect(page.getByTestId("message-insert-mention")).toHaveCount(0);
await expect(page.getByRole("button", { name: "Attach file" })).toHaveCount(0);
await expect(page.getByTestId("issue-discussed-in")).toHaveCount(0);
await expect(page.getByTestId("project-issue-assign")).toHaveCount(0);
```

Keep the comment-load recovery test, and expect the body-only composer to remain visible while listed comments are retrying.

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts
```

Expected: the existing read-only detail has no composer, so the visibility assertion fails.

- [ ] **Step 3: Extract GitHub identity and render state accurately.**

Move `GitHubLoginIdentity` and `GitHubAssigneeFacepile` from `GitHubProjectIssues.tsx` into `GitHubIssueIdentity.tsx` without importing profile/pubkey modules.
Change `GitHubIssueRow` and detail status to read `issue.status`.
Use `CircleDot` plus green for Open and `CircleX` plus `text-destructive` for Closed.

- [ ] **Step 4: Implement Close or Reopen and body-only comment controls.**

`GitHubIssueStateButton` accepts `issue`, `project`, `onListStateChange`, and `onSelectedIssueIdChange`.
It uses `useUpdateGithubIssueStateMutation`, shows Close for Open and Reopen for Closed, and uses test ids `project-github-issue-close` and `project-github-issue-reopen`.
On success, first set the destination list state, then keep `issue.id` selected, then invalidate `githubIssueWriteInvalidationKey(project.id)`.
On `github_issue_unavailable`, toast `Issue not found.` and clear selection.
On every other failure, toast the parsed GitHub message and leave the filter unchanged.

`GitHubIssueCommentComposer` uses one controlled `<textarea>`, a Comment button, and `useCreateProjectIssueCommentMutation(project)`.
Use test ids `project-issue-comment-composer`, `project-github-issue-comment-input`, and `project-github-issue-comment-submit`.
Submit only `{ content, issue }`, clear text only after success, and toast `Comment posted.`.
On `github_issue_unavailable`, toast `Issue not found.` and clear selection.
On every other failure, keep the exact textarea text and toast the parsed message.
Do not import `ForumComposer`, media hooks, mention hooks, or pubkey helpers.

- [ ] **Step 5: Implement label chips and catalog add.**

Render every existing `issue.labels` value as a chip even while the catalog query is loading or failed.
Give each removal button `aria-label="Remove label {name}"`, `data-testid="project-github-issue-label"`, and `data-label-name={name}`.
Use a shared dropdown menu for Add label and list only catalog names not already on the issue.
Give each option `data-testid={\`project-github-issue-label-option-${name}\`}` and an accessible label containing the exact name.
Hide Add label when the successful catalog is empty, and disable it with a concise unavailable title when the catalog query failed.
After add/remove success, invalidate the issues prefix and toast `Label added.` or `Label removed.`.
On provisional label `github_issues_failed`, also invalidate `["project", project.id, "github-labels"]` and keep the selection.
If a follow-up issue GET returns `github_issue_unavailable`, toast `Issue not found.` and clear selection.

- [ ] **Step 6: Implement assignee identities, catalog picker, and Assign me.**

Render current logins through `GitHubLoginIdentity` and remove buttons with `aria-label="Unassign {login}"` and `data-testid={\`project-github-issue-unassign-${login}\`}`.
Use a shared dropdown menu for Assign and list catalog logins not already assigned.
Disable the picker when its catalog query failed, but keep existing assignee removal controls working.
Use `useGithubAuthenticatedUserQuery(true)` for Assign me or Unassign me and never read the Buzz identity.
Show Assign me with `project-github-issue-assign-me` when the authenticated login is absent.
Show Unassign me with `project-github-issue-unassign-me` when the authenticated login is present.
If `GET /user` fails, disable self-assignment and render `GitHubRepoStateRecovery` with `unavailableTitle="Could not load GitHub user"` and its own title id.
After successful add/remove, invalidate the issues prefix and toast `Issue assigned.` or `Issue unassigned.`.
On `github_issue_unavailable`, toast `Issue not found.` and clear selection.
Never render `data-testid="project-issue-assign"` on a GitHub detail.

- [ ] **Step 7: Compose the detail and run the smoke contract.**

Extend `GitHubIssueDetail` props with `onListStateChange` and `onSelectedIssueIdChange`.
Put Close or Reopen in the detail header.
Keep comment loading/recovery local to the timeline, and mount the body composer below that timeline/recovery branch so comment-list failure does not remove the write control.
Mount Labels and Assignees rail sections even when the current issue sets are empty so add controls remain reachable.
Pass both callbacks from `ProjectIssuesPanel`.

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts
. ./bin/activate-hermit && cd desktop && pnpm typecheck && pnpm check:file-sizes && pnpm check:px-text
```

Expected: I1+I2 list/create still works, the textarea is visible without mention/media controls, and auth/comment-load recovery stays scoped correctly.

- [ ] **Step 8: Format, inspect scope, and commit.**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/features/projects/ui/GitHubIssueIdentity.tsx \
  src/features/projects/ui/GitHubIssueStateButton.tsx \
  src/features/projects/ui/GitHubIssueCommentComposer.tsx \
  src/features/projects/ui/GitHubIssueLabels.tsx \
  src/features/projects/ui/GitHubIssueAssignees.tsx \
  src/features/projects/ui/GitHubProjectIssues.tsx \
  src/features/projects/ui/ProjectIssuesPanel.tsx \
  tests/e2e/github-issues.spec.ts
git add desktop/src/features/projects/ui/GitHubIssueIdentity.tsx \
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

### Task 8: Add a mutable mock store and end-to-end write coverage

**Files:**

- Modify: `desktop/src/testing/e2eBridge.ts`
- Create: `desktop/tests/e2e/github-issue-writes.spec.ts`
- Modify: `desktop/playwright.config.ts`

**Interfaces:**

- Consumes: the nine new commands plus I1+I2 list/create/comments.
- Produces: one deterministic in-memory GitHub issue store for the complete UI workflow.

- [ ] **Step 1: Run the Task 8 impact checks.**

- [ ] **Step 2: Define the exact test store and write the failing E2E spec.**

Add `__BUZZ_E2E_GITHUB_ISSUE_WRITE_ERROR__` and a typed `__BUZZ_E2E_GITHUB_ISSUE_STORE__` to the E2E `Window` interface.
The store contains `issues`, `commentsByNumber`, `labels`, `assignees`, and `authenticatedUser`.
Initialize it only when absent with issue `#42` open, label `bug`, assignee `linus`, labels `bug` and `docs`, assignees `linus` and `ada`, authenticated user `ada`, and two comments with valid `html_url` values.
Replace `__BUZZ_E2E_GITHUB_CREATED_ISSUES__` with this store so I1+I2 create and all new writes share one source of truth.

Create `github-issue-writes.spec.ts` with the same init-script-before-bridge order used by `github-issues.spec.ts`.
The first test must execute this exact observable sequence.

```ts
await openGithubIssues(page);
await page.getByRole("button", { name: "#42", exact: true }).click();
await page.getByTestId("project-github-issue-close").click();
await expect(page.getByTestId("project-github-issue-reopen")).toBeVisible();
await page.getByTestId("project-github-issue-comment-input").fill("Looks good");
await page.getByTestId("project-github-issue-comment-submit").click();
await expect(page.getByText("Looks good", { exact: true })).toBeVisible();
await page.getByRole("button", { name: "Add label" }).click();
await page.getByTestId("project-github-issue-label-option-docs").click();
await page.getByRole("button", { name: "Remove label bug" }).click();
await page.getByRole("button", { name: "Unassign linus" }).click();
await page.getByTestId("project-github-issue-assign-me").click();
await expect(page.getByTestId("project-github-issue-unassign-me")).toBeVisible();
await page.getByTestId("project-github-issue-reopen").click();
await expect(page.getByTestId("project-github-issue-close")).toBeVisible();
```

After the sequence, inspect `__BUZZ_E2E_COMMAND_PAYLOADS__` and assert exact state/comment/label/assignee payloads.
Assert `__BUZZ_E2E_COMMANDS__` contains all exercised GitHub commands and contains no `sign_event` for the GitHub comment.
Add a failed-close test that sets `github_issues_failed`, expects toast `Close failed.`, keeps Open selected, and keeps the Close control visible.
Keep the list-level `github_auth_required` test in `github-issues.spec.ts`; do not duplicate it unless the new spec needs independent regression coverage.

- [ ] **Step 3: Run the new spec and verify RED.**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issue-writes.spec.ts
```

Expected: the spec is not registered or the write commands are absent from the mock bridge.

- [ ] **Step 4: Implement every mock command against the shared store.**

Make `list_github_issues` filter by `input.state` and preserve list-level `__BUZZ_E2E_GITHUB_ISSUES_ERROR__`.
Make `create_github_issue` append an open issue with the next number.
Make `list_github_issue_comments` read `commentsByNumber[number]` and preserve its dedicated error flag.
Add cases for `update_github_issue_state`, `create_github_issue_comment`, `list_github_repo_labels`, `add_github_issue_labels`, `remove_github_issue_label`, `list_github_repo_assignees`, `add_github_issue_assignees`, `remove_github_issue_assignee`, and `get_github_authenticated_user`.
Check `__BUZZ_E2E_GITHUB_ISSUE_WRITE_ERROR__` before mutating in every write case.
Return DTO shapes that match the real command, including valid comment `html_url` and updated issue `comments`, `labels`, and `assignees`.
Register `"**/github-issue-writes.spec.ts"` immediately after `"**/github-issues.spec.ts"` in the smoke `testMatch` array.

- [ ] **Step 5: Run focused E2E, unit, type, and ratchet checks and verify GREEN.**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts github-issue-writes.spec.ts project-issue-comments.spec.ts
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issue_writes
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectGithubIssueWrites.test.mjs src/features/projects/issueMutations.test.mjs src/features/projects/projectIssues.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm typecheck && pnpm check:file-sizes && pnpm check:px-text
```

Expected: the GitHub write flow passes, I1+I2 still passes, Buzz comments and assignments stay green, and every applicable ratchet passes.

- [ ] **Step 6: Format, inspect scope, and commit.**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write src/testing/e2eBridge.ts tests/e2e/github-issue-writes.spec.ts playwright.config.ts
git add desktop/src/testing/e2eBridge.ts \
  desktop/tests/e2e/github-issue-writes.spec.ts \
  desktop/playwright.config.ts
git diff --stat && git diff --name-only && git diff --check
git commit -s -m "test(projects): cover GitHub issue writes in smoke e2e"
```

---

## Final Validation and Acceptance

- [ ] Run repository-wide validation from the repository root.

```bash
. ./bin/activate-hermit && just ci
```

- [ ] Run GitNexus compare detection against `main` when available, or record its continued unavailability with final `git diff --stat`, `git diff --name-only`, and `git diff --check` output.

- [ ] On a designated disposable issue in `harness-service` or another explicitly authorized GitHub test repository, verify the real workflow after `gh auth status --hostname github.com` succeeds.

Create or choose the disposable issue, close it, comment, add and remove one label, add and remove one assignee, use Assign me, and reopen it.
Confirm the GitHub issue matches the UI and no kind:1 event appears in the Buzz issue timeline.
If no authorized repository or disposable issue is available, do not write to an arbitrary repository; report the manual check as not run and rely on the automated evidence.

Acceptance is complete when all of the following are true.

- Open | Closed lists GitHub issues as `#N` for the selected state.
- Close moves `#N` to Closed and keeps it selected.
- Reopen moves `#N` to Open and keeps it selected.
- Create switches to Open, selects the returned number, and clears it only if the settled page omits that number.
- A comment posted in Buzz appears on the GitHub issue and no kind:1 is published.
- Failed comment text remains in the textarea.
- Adding or removing one label changes the GitHub issue without replacing other labels.
- Adding or removing one assignee changes the GitHub issue without replacing other assignees.
- Assign me uses the authenticated `gh` login rather than a Buzz pubkey.
- GitHub identities never reach pubkey profile, assignment, or comment helpers.
- Missing `gh` or auth renders recovery before any empty state.
- Issue-targeted 404 behavior clears selection where specified, while provisional label 404 behavior keeps selection and refreshes the catalog.
- Buzz-hosted repositories still list, create, comment, and assign through their existing Nostr paths only.

## Spec Coverage

| Spec requirement | Task |
|---|---|
| Open or Closed list request and query key | Tasks 1 and 5 |
| Comment `html_url` validation | Tasks 1 and 2 |
| Close or reopen PATCH body | Task 2 |
| Comment POST body via tempfile | Task 2 |
| Label DELETE percent-encoding and response follow-up | Task 3 |
| Label catalog validation | Task 3 |
| Assignee JSON bodies and catalogs | Task 3 |
| Authenticated `gh` user | Task 3 |
| Stable native error codes | Tasks 2 and 3 |
| Nine Tauri commands and TypeScript wrappers | Task 4 |
| Local filter state and create/close/reopen selection policy | Task 5 |
| GitHub comment never publishes Nostr | Task 6 |
| GitHub-only filter, status, composer, labels, and assignees | Task 7 |
| Auth recovery before empty success | Tasks 5, 7, and 8 |
| Full write smoke and Buzz regression | Task 8 |
| Repository-wide and authorized real-workflow validation | Final Validation and Acceptance |
