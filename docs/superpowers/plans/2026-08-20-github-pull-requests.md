# GitHub Pull Requests (list + create) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a Projects repository uses a plain `github.com` clone URL, its per-repository Pull Request tab lists open GitHub pull requests, creates ready GitHub pull requests, and loads existing conversation comments read-only without reading or publishing NIP-34 pull-request events for that repository.

**Architecture:** Add a focused Tauri command module backed by the existing bounded `GhRunner` and `GitHubRepoRef` abstractions.
Route the existing repository pull-request query and create mutation by the selected repository's first clone URL, map bounded GitHub DTOs into the extended `ProjectPullRequest` model, and keep the Buzz-hosted Nostr path unchanged.
Render GitHub rows and detail through a separate component so GitHub logins never reach pubkey normalization, profile lookup, review mutations, merge, or Nostr comment/update publishers.

**Tech Stack:** Rust, Tauri 2, `gh api`, React 19, TanStack Query, Node `node:test`, and Playwright with the E2E mock bridge.

**Spec:** [2026-08-18-github-pull-requests-design.md](../specs/2026-08-18-github-pull-requests-design.md)

**Product contract:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) make GitHub the sole issue and pull-request backend for GitHub-hosted repositories and prohibit dual-write.

**Testing contract:** [TESTING.md](../../../TESTING.md) and the desktop E2E rules in [AGENTS.md](../../../AGENTS.md) require focused unit coverage, an E2E-mode build, the mock Tauri bridge, and the full repository gate before handoff.

**Phase doc:** [phase-03-github-pull-requests.md](../../../plans/20260818-1211-github-native-host/phase-03-github-pull-requests.md), slice P1 + P2 only.

## Global Constraints

- Implement P1 + P2 only: list the first page of open GitHub pull requests, create a ready same-repository pull request, and load the first page of existing issue conversation comments read-only.
- Do not implement required checks, reviews, review comments, posting comments, approve, request changes, reviewer assignment, close, reopen, draft conversion, ready conversion, merge-by-number, merge queue, auto-merge, or GitHub files.
- Do not change `merge_github_pull_request`, its find-or-create behavior, or kind `1631` publication in this slice.
- Do not change the global Projects pull-request list, project card/activity counts, CLI, mobile, or the `buzz://pr` scheme.
- Do not union GitHub pull requests with kind `1618`, import GitHub pull requests into Nostr, or dual-write.
- Do not call `gh pr list` or `gh pr create`, add a provider trait, or store a GitHub token.
- Authenticate only through installed `gh` and `gh auth status --hostname github.com`.
- Use one `gh api` page with `state=open`, `per_page=100`, `sort=updated`, `direction=desc`, and no `--paginate`.
- Include inbound fork pull requests in the list.
- Compute `has_more` from the raw projected page length before dropping malformed rows.
- Accept only positive `u64` pull-request numbers in Rust and positive safe decimal integers in TypeScript.
- Treat GitHub authors as logins plus avatar URLs, never as Nostr pubkeys.
- Never pass a GitHub login to `ProfileIdentityButton`, `ProfileAuthorName`, `normalizePubkey`, `useUsersBatchQuery`, review mutations, merge mutations, or Nostr comment/update publishers.
- Map `draft: true` to `Draft` and every other listed pull request to `Open`.
- Render `head.ref → base.ref`, or `owner:head.ref → base.ref` when `head.repo.full_name` differs from the target repository case-insensitively.
- Copy only a validated `https://github.com/{owner}/{repo}/pull/{number}` URL.
- Keep the existing hex-event `buzz://pr` fallback for Buzz pull requests.
- Create with exactly `{ "title", "body", "head", "base" }` through a tempfile passed to `gh api --input`.
- Trim the title, reject an empty title, reject more than 256 Unicode scalar values, reject an empty head or base, and reject `head == base` before running `gh`.
- Preserve the supplied body, including an empty body.
- Do not send `draft`, `head_repo`, reviewers, a commit SHA, or a merge base to the GitHub create endpoint.
- On GitHub create success, invalidate only `["project", project.id, "pull-requests"]`.
- On Buzz create success, preserve the current pull-request, work-items, and activity-summaries invalidations.
- Keep the pull-request list query key `["project", project.id, "pull-requests"]` and stale time of 30 seconds.
- Use comment query key `["project", project.id, "pull-requests", number, "comments"]` and load comments only after a numeric GitHub row is selected.
- Keep list comments empty and show the list-provided `commentCount` until the detail query resolves.
- Display the conversation count as `max(commentCount, comments.length)` after comments resolve.
- Show exactly one commit row for `head.sha` and a commit badge of `1` for GitHub pull requests.
- Keep the existing checks placeholder text exactly `No checks have been reported for this pull request yet.`.
- Hide Merge, reviewers, Request review, Approve, Request changes, the comment composer, Discussed in channels, the origin reference, and Files changed on GitHub rows.
- If a stale GitHub detail opens on `pr-files`, move it to `pr-conversation` and do not mount `ProjectPullRequestFilesChangedPanel`.
- Do not run GitHub PR diff queries through the Nostr `refs/nostr/{id}` path.
- Do not show the Nostr `Update PR` action for a GitHub pull request.
- Preserve the existing `shouldPublishPullRequestUpdateAfterPush` GitHub skip in `projectGithubSync.ts` and keep its regression test green.
- Refuse any non-64-hex pull-request id before a leftover Nostr status, review, comment, update, or merge call can invoke Tauri or sign an event.
- Use only `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, `github_pr_unavailable`, and `github_pulls_failed` on the new native surface.
- Do not leak `github_merge_failed`, `github_state_failed`, or `github_issues_failed` from a new command.
- Check GitHub list errors before empty-success rendering.
- GitHub empty-success copy is `No open pull requests.` and the existing Buzz empty/error copy stays unchanged.
- GitHub truncation copy is `More open pull requests exist on GitHub.` and it is a muted note, not a button.
- A comment 404 shows `Pull request not found.`, clears selection, and keeps the list.
- A non-404 comment failure keeps the PR body visible and retries only the comment query.
- Create failures stay in the dialog and do not clear title or body.
- Do not run live GitHub requests in automated tests.
- Do not add production `unsafe`, `unwrap()`, or `expect()` calls.
- Add doc comments to every new exported TypeScript declaration and every new public Rust API.
- Use named rem-based text tokens and do not add arbitrary text sizes.
- Keep every edited file covered by `pnpm check:file-sizes` at or below 1,000 lines.
- `desktop/src/testing/e2eBridge.ts` is intentionally outside that ratchet and must be changed only in its GitHub PR declarations, store initialization, and invoke cases.
- Activate Hermit in every shell command with `. ./bin/activate-hermit && ...` or as the first line of the same shell block.
- Run commands from the repository root because shell working directories do not persist between tool calls.
- Run GitNexus `impact({ target, direction: "upstream" })` before editing each existing symbol when the MCP tools are available.
- Warn before proceeding if GitNexus reports HIGH or CRITICAL risk.
- Run GitNexus `detect_changes({ scope: "staged" })` before every commit and `detect_changes({ scope: "compare", base_ref: "main" })` before final handoff when the MCP tools are available.
- If GitNexus tools are unavailable, record that fact and use `git diff --stat`, `git diff --name-only`, and `git diff --check` as fallback scope evidence.
- Sign every commit with `git commit -s`.

---

## Resolved Implementation Decisions

- Do not add code to `desktop/src-tauri/src/commands/project_github_pull_request.rs`; it is already 998 lines and already exposes every reusable runner, repository parser, and redaction interface needed by the new module.
- Put list, create, and comment-list commands in the new `desktop/src-tauri/src/commands/project_github_pulls.rs` module.
- Use a 32 MiB stdout cap for one projected page of pull-request or comment JSON so response handling stays bounded while accommodating 100 large UTF-8 bodies.
- A response that exceeds the cap becomes invalid/truncated JSON and returns `github_pulls_failed`; never substitute an empty list.
- Drop malformed list rows and malformed listed comments.
- Reject a malformed created pull-request response with `github_pulls_failed` because create cannot safely report success without a valid returned number and URL.
- Require the projected base repository to match the parsed target repository case-insensitively before mapping a PR.
- Preserve GitHub's comment order without local sorting.
- Accept comment URLs in exactly the GitHub issue-comment forms named by the spec, with either `/issues/{number}` or `/pull/{number}` before `#issuecomment-{id}`.
- Put frontend DTOs and Tauri wrappers in the new `desktop/src/shared/api/projectGithubPulls.ts`; adding them to the existing 882-line `projectGit.ts` would push that file toward the ratchet and mix a new bounded API with the merge API.
- Set `initialCommit` and `commit` to `head.sha` for a mapped GitHub PR so existing commit-selection code sees one stable head row while `updates` remains empty.
- Reuse `GitHubLoginIdentity` and `ProjectIssueCommentTimeline` in GitHub mode because both already render login/avatar identities without pubkey helpers.
- Put all GitHub-only PR list/detail chrome in the new `desktop/src/features/projects/ui/GitHubProjectPullRequests.tsx` component.
- Keep the existing Buzz-only `ProjectPullRequestsPanel.tsx` implementation intact except for shared model compatibility.
- Use `GitHubRepoStateRecovery` with `unavailableTitle="Could not load GitHub pull requests"` for list errors and `unavailableTitle="Could not load GitHub pull request comments"` for comment errors.
- Use a toast with exact copy `Pull request not found.` before clearing selection on `github_pr_unavailable`.
- Do not modify `repoSyncHooks.ts` or `projectGithubSync.ts`; the existing `shouldPublishPullRequestUpdateAfterPush` test already proves GitHub pushes skip kind `1613` publication.

## File Map

| File | Responsibility |
|---|---|
| Create `desktop/src-tauri/src/commands/project_github_pulls.rs` | Bounded list/create/comments commands, DTO mapping, URL validation, and PR-specific error remapping |
| `desktop/src-tauri/src/commands/mod.rs` | Declare and re-export the new command module |
| `desktop/src-tauri/src/invoke.rs` | Register the three new Tauri commands |
| Create `desktop/src/shared/api/projectGithubPulls.ts` | GitHub PR DTO types and three typed Tauri invoke wrappers |
| `desktop/src/features/projects/projectPullRequests.mjs` | Populate neutral GitHub-extension defaults on Nostr pull requests |
| `desktop/src/features/projects/projectPullRequests.d.mts` | Extend `ProjectPullRequest` and comment declarations with avatar, URL, head-repository, and count fields |
| `desktop/src/features/projects/projectPullRequests.test.mjs` | Protect unchanged Nostr mapping defaults |
| Create `desktop/src/features/projects/lib/projectGithubPullRequests.ts` | Host routing, DTO mapping, number/identity/branch/tab helpers, settled-selection helper, comment query, and Nostr-id guard |
| Create `desktop/src/features/projects/lib/projectGithubPullRequests.test.mjs` | Protect routing, mapping, identity, number, branch, comment-query, and guard contracts |
| `desktop/src/features/projects/lib/projectShareLinks.ts` | Prefer a strictly validated GitHub PR URL before the existing Buzz fallback |
| `desktop/src/features/projects/lib/projectShareLinks.test.mjs` | Protect GitHub PR URL validation and the existing Buzz deep link |
| `desktop/src/features/projects/hooks.ts` | Route `useProjectPullRequestsQuery` and guard the existing Nostr comment publisher |
| `desktop/src/features/projects/pullRequestMutations.ts` | Route create, select host-specific invalidations, skip GitHub update publication, and guard Nostr update/merge paths |
| `desktop/src/features/projects/pullRequestMutations.test.mjs` | Prove host-exclusive create, invalidations, and non-hex Nostr refusal |
| `desktop/src/features/projects/pullRequestReviews.ts` | Guard every Nostr status/review publisher before normalization, signing, or invoke |
| `desktop/src/features/projects/ui/CreatePullRequestDialog.tsx` | Read the new query result shape while preserving current branch and duplicate-open validation |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Read the new query shape without unstable fallback arrays, filter profile lookup, disable GitHub diff/update behavior, and pass list status, `hasMore`, and retry |
| `desktop/src/features/projects/ui/ProjectWorkspaceTabList.tsx` | Use `commentCount`, force one GitHub commit, and hide Files changed |
| `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx` | Branch list/detail chrome by host, recover missing selections, snap stale Files tabs, and avoid mounting GitHub file/review UI |
| Create `desktop/src/features/projects/ui/GitHubProjectPullRequests.tsx` | Render GitHub list, detail, login identities, branches, read-only comments, commit, checks, errors, and truncation note |
| `desktop/src/testing/e2eBridge.ts` | Add a mutable GitHub PR store plus list/create/comments command stubs and structured errors |
| Create `desktop/tests/e2e/github-pull-requests.spec.ts` | Exercise list, detail, comments, hidden writes/files, create, auth recovery, and comment recovery |
| `desktop/playwright.config.ts` | Add the new spec to the smoke project |
| Verification only: `desktop/src/features/projects/lib/projectGithubSync.test.mjs` | Keep the existing GitHub push/no-Nostr-update regression green |
| Verification only: `desktop/tests/e2e/project-pr-review.spec.ts` | Keep the existing Buzz-hosted review/merge surface green |

## Required Impact Checks

GitNexus was unavailable during plan repair.
At execution time, try each named upstream impact check once before the first edit in that task.
If the tools remain unavailable, record that result and continue with direct source inspection and the documented diff checks.

- Task 1 targets `GitHubRepoRef`, `GhRunner::ensure_auth`, `GhRunner::run_with_limit`, `combined_cli_diagnostic`, and `redact_diagnostic` as consumed interfaces.
- Task 2 targets the command-module re-exports and `desktop_invoke_handler` registration list.
- Task 3 targets `eventToProjectPullRequest`, `ProjectPullRequest`, `pullRequestShareLink`, and `useProjectPullRequestsQuery` consumers.
- Task 4 targets `publishProjectPullRequest`, `publishProjectPullRequestUpdate`, `useCreateProjectPullRequestMutation`, `useMergeProjectPullRequestMutation`, `createProjectPullRequestComment`, `updateProjectPullRequestStatus`, `requestProjectPullRequestReview`, and `submitProjectPullRequestReview`.
- Task 5 targets `ProjectDetailScreen`, `CreatePullRequestDialog`, `WorkspaceTabs`, `PullRequestTabsList`, and the `ProjectPullRequestsPanel` exports consumed by WorkspaceTabs.
- Task 6 targets the E2E invoke switch, E2E `Window` declarations/store initialization, and Playwright smoke `testMatch`.

---

### Task 1: Add the bounded native GitHub pull-request list core

**Files:**

- Create: `desktop/src-tauri/src/commands/project_github_pulls.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs:50-58`

**Interfaces:**

- Consumes: `GitHubRepoRef::{parse, slug, owner, repo}`, `GhRunner::{ensure_auth, from_resolved, run_with_limit}`, `combined_cli_diagnostic`, `redact_diagnostic`, and `ProjectPullRequestMergeError`.
- Produces: `GitHubPullRequestUserDto`, `GitHubPullRequestRepoDto`, `GitHubPullRequestHeadDto`, `GitHubPullRequestBaseDto`, `GitHubPullRequestDto`, `GitHubPullRequestListDto`, `list_github_pull_requests_with`, `is_pull_html_url`, and `remap_pulls_error`.

- [ ] **Step 1: Run the Task 1 impact checks.**

Report direct callers, affected processes, and risk before editing.
Stop and warn if a result is HIGH or CRITICAL.

- [ ] **Step 2: Declare the module and write failing list tests first.**

Add `mod project_github_pulls;` beside the other `project_github_*` declarations so unit tests compile before the command is re-exported.
Create the new file with the following exact projected fixture and tests.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn error_code(error: &ProjectPullRequestMergeError) -> String {
        serde_json::to_value(error).expect("serialize error")["code"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    fn projected_pull(number: u64, draft: bool) -> serde_json::Value {
        json!({
            "number": number,
            "title": format!("Pull {number}"),
            "body": "Details",
            "html_url": format!("https://github.com/acme/app/pull/{number}"),
            "draft": draft,
            "created_at": "2026-01-02T03:04:05Z",
            "updated_at": "2026-01-03T03:04:05Z",
            "comments": 3,
            "user": { "login": "ada", "avatar_url": "https://avatars.githubusercontent.com/u/1" },
            "head": {
                "ref": "feature/readme",
                "sha": "1111111111111111111111111111111111111111",
                "repo": { "full_name": "acme/app" }
            },
            "base": {
                "ref": "main",
                "repo": { "full_name": "acme/app" }
            }
        })
    }

    #[cfg(unix)]
    fn fake_gh(
        output: &serde_json::Value,
        status: i32,
        stderr: &str,
    ) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("create fake gh directory");
        let path = dir.path().join("gh");
        let script = format!(
            "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\n  *) printf '%s' '{}' ; printf '%s' '{}' >&2; exit {} ;;\nesac\n",
            output,
            stderr,
            status,
        );
        std::fs::write(&path, script).expect("write fake gh");
        let mut permissions = std::fs::metadata(&path).expect("stat fake gh").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("chmod fake gh");
        (dir, path)
    }

    #[cfg(unix)]
    #[test]
    fn list_maps_open_and_draft_pull_fields() {
        let output = json!([projected_pull(42, false), projected_pull(43, true)]);
        let (dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(
            &gh,
            "https://github.com/acme/app",
        )
        .expect("list");
        assert_eq!(page.pulls.len(), 2);
        assert_eq!(page.pulls[0].number, 42);
        assert!(!page.pulls[0].draft);
        assert!(page.pulls[1].draft);
        assert_eq!(page.pulls[0].user.login, "ada");
        assert_eq!(page.pulls[0].head.branch, "feature/readme");
        assert_eq!(page.pulls[0].head.repo.full_name, "acme/app");
        assert_eq!(page.pulls[0].base.branch, "main");
        assert_eq!(page.pulls[0].comments, 3);
        assert!(!page.has_more);
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
        assert!(calls.contains("/repos/acme/app/pulls?state=open&per_page=100&sort=updated&direction=desc"));
        assert!(calls.contains("--jq"));
        assert!(!calls.contains("--paginate"));
    }

    #[cfg(unix)]
    #[test]
    fn list_preserves_inbound_fork_metadata() {
        let mut item = projected_pull(42, false);
        item["head"]["repo"]["full_name"] = json!("fork-owner/app");
        let (_dir, path) = fake_gh(&json!([item]), 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "git@github.com:acme/app.git")
            .expect("list");
        assert_eq!(page.pulls[0].head.repo.full_name, "fork-owner/app");
        assert_eq!(page.pulls[0].head.sha, "1111111111111111111111111111111111111111");
    }

    #[cfg(unix)]
    #[test]
    fn raw_page_of_100_sets_has_more_before_invalid_rows_are_dropped() {
        let output = serde_json::Value::Array(
            (1..=100)
                .map(|number| {
                    let mut item = projected_pull(number, false);
                    item["html_url"] = json!(format!("https://evil.example/pull/{number}"));
                    item
                })
                .collect(),
        );
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app")
            .expect("list");
        assert!(page.pulls.is_empty());
        assert!(page.has_more);
    }

    #[test]
    fn accepts_only_repo_bound_pull_urls() {
        let repo = GitHubRepoRef::parse("https://github.com/Acme/App").expect("repo");
        for (raw, number, expected) in [
            ("https://github.com/acme/app/pull/42", 42, true),
            ("https://github.com/ACME/APP/pull/42", 42, true),
            ("https://github.com/acme/app/pull/43", 42, false),
            ("https://github.com/acme/other/pull/42", 42, false),
            ("https://evil.example/acme/app/pull/42", 42, false),
            ("https://user@github.com/acme/app/pull/42", 42, false),
            ("https://github.com/acme/app/pull/42?x=1", 42, false),
            ("https://github.com/acme/app/pull/42#x", 42, false),
            ("https://github.com/acme/app/pull/42/", 42, false),
        ] {
            assert_eq!(is_pull_html_url(&repo, raw, number), expected, "{raw}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn list_drops_a_pull_whose_base_repo_is_not_the_target() {
        let mut item = projected_pull(42, false);
        item["base"]["repo"]["full_name"] = json!("acme/other");
        let (_dir, path) = fake_gh(&json!([item]), 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app")
            .expect("list");
        assert!(page.pulls.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn list_drops_one_malformed_wire_row_without_losing_valid_rows() {
        let output = json!([
            { "number": "not-a-number", "title": 7 },
            projected_pull(42, false)
        ]);
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app")
            .expect("list");
        assert_eq!(page.pulls.len(), 1);
        assert_eq!(page.pulls[0].number, 42);
    }

    #[test]
    fn list_rejects_non_github_clone_url_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = list_github_pull_requests_with(
            &gh,
            &format!("https://relay.example/git/{}/app", "ab".repeat(32)),
        )
        .expect_err("reject Buzz URL");
        assert_eq!(error_code(&error), "github_pulls_failed");
    }
}
```

- [ ] **Step 3: Run the focused native test and verify RED.**

Run: `. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pulls -- --nocapture`

Expected: FAIL because `project_github_pulls` production types and functions do not exist.

- [ ] **Step 4: Implement the minimal bounded list core.**

Use these exact wire fields and public DTO names.
Use `#[serde(rename = "ref")]` for Rust branch fields and keep serialized DTO field names in snake case for Tauri.

```rust
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{redact_diagnostic, GhRunner, GitHubRepoRef};
use crate::commands::project_github_repository_state::combined_cli_diagnostic;
use chrono::DateTime;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::ffi::OsString;
use std::path::Path;
use url::Url;

const GH_PULL_STREAM_LIMIT: usize = 32 * 1024 * 1024;
const PULL_LIST_JQ: &str = "[.[] | {number, title, body: (.body // \"\"), html_url, draft, created_at, updated_at, comments, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), head: {ref: .head.ref, sha: .head.sha, repo: (if .head.repo == null then null else {full_name: .head.repo.full_name} end)}, base: {ref: .base.ref, repo: (if .base.repo == null then null else {full_name: .base.repo.full_name} end)}}]";

/// GitHub login identity returned to the desktop PR UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestUserDto {
    pub login: String,
    pub avatar_url: String,
}

/// GitHub repository identity returned to the desktop PR UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestRepoDto {
    pub full_name: String,
}

/// Bounded head metadata for one GitHub pull request.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestHeadDto {
    #[serde(rename = "ref")]
    pub branch: String,
    pub sha: String,
    pub repo: GitHubPullRequestRepoDto,
}

/// Bounded base metadata for one GitHub pull request.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestBaseDto {
    #[serde(rename = "ref")]
    pub branch: String,
    pub repo: GitHubPullRequestRepoDto,
}

/// One bounded GitHub pull request returned to the desktop UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestDto {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub html_url: String,
    pub draft: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub comments: u64,
    pub user: GitHubPullRequestUserDto,
    pub head: GitHubPullRequestHeadDto,
    pub base: GitHubPullRequestBaseDto,
}

/// One GitHub pull-request page plus its first-page truncation signal.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestListDto {
    pub pulls: Vec<GitHubPullRequestDto>,
    pub has_more: bool,
}
```

Implement `github_pull_api_json` with the same argument order used by `project_github_issues.rs`: `api`, `--hostname github.com`, `--method`, path, `--jq`, and optional `--input`.
Call `gh.run_with_limit(&args, GH_PULL_STREAM_LIMIT)`, classify from combined stdout and stderr, redact diagnostics, and deserialize only projected JSON.

Implement `is_pull_html_url` with strict HTTPS, host, credentials, port, query, fragment, percent, slash, target owner/repo, `pull`, and matching-number checks.
Deserialize the projected list as `Vec<serde_json::Value>` so one malformed object cannot fail the entire page.
Implement `map_pull(repo, value)` by first attempting `serde_json::from_value::<GitHubPullRequestWire>(value).ok()?`, then return `None` for zero numbers, blank login/title/head/base/SHA, missing user/repositories, invalid timestamps, a base repository other than the parsed target, or an invalid URL.

Implement the list operation exactly as follows.

```rust
/// List one bounded page of open GitHub pull requests with an injected runner.
pub(crate) fn list_github_pull_requests_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<GitHubPullRequestListDto, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_pulls_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_pulls_error(error, "", "github_repo_unavailable"))?;
    let path = format!(
        "/repos/{}/pulls?state=open&per_page=100&sort=updated&direction=desc",
        repo.slug(),
    );
    let raw: Vec<serde_json::Value> = github_pull_api_json(
        gh,
        "GET",
        &path,
        PULL_LIST_JQ,
        None,
        "github_repo_unavailable",
    )?;
    let has_more = raw.len() == 100;
    let pulls = raw
        .into_iter()
        .filter_map(|value| map_pull(&repo, value))
        .collect();
    Ok(GitHubPullRequestListDto { pulls, has_more })
}
```

Implement `remap_pulls_error(error, diagnostic, not_found_code)` with this exact precedence.

1. Preserve `github_cli_missing` and `github_auth_required` unchanged.
2. Map rate-limit or abuse diagnostics to `github_pulls_failed`.
3. Map a non-rate HTTP 403 to `github_repo_unavailable`.
4. Map HTTP 404 or `not found` to the caller-supplied `not_found_code`.
5. Map every other failure to `github_pulls_failed`.

- [ ] **Step 5: Run the focused native test and verify GREEN.**

Run: `. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pulls -- --nocapture`

Expected: PASS for the list, mapping, URL, host, fork, malformed-row, and raw-page tests.

- [ ] **Step 6: Commit Task 1.**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_pulls.rs desktop/src-tauri/src/commands/mod.rs
```

Run the staged GitNexus scope check when available.
If it is unavailable, run `git diff --cached --stat`, `git diff --cached --name-only`, and `git diff --cached --check` and confirm only the Task 1 files are staged.

```bash
. ./bin/activate-hermit
git commit -s -m "feat(desktop): add GitHub pull request list core"
```

---

### Task 2: Add native create, read-only comments, errors, and command registration

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_pulls.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs:100-110`
- Modify: `desktop/src-tauri/src/invoke.rs:45-62`

**Interfaces:**

- Consumes: Task 1's parser, runner, API helper, DTO mapper, and error remapper.
- Produces: `GitHubPullRequestCommentDto`, `create_github_pull_request_with`, `list_github_pull_request_comments_with`, runner wrappers, and three Tauri commands with the names required by the spec.

- [ ] **Step 1: Run the Task 2 impact checks.**

Report direct callers, affected processes, and risk before editing.
Stop and warn if a result is HIGH or CRITICAL.

- [ ] **Step 2: Write failing create and comment tests.**

Add tests with these exact assertions to the Task 1 test module.

```rust
#[test]
fn create_rejects_invalid_fields_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
    for (title, head, base, expected) in [
        ("   ", "feature", "main", "Pull request title is required."),
        ("title", "", "main", "Compare branch is required."),
        ("title", "feature", "", "Base branch is required."),
        ("title", "main", "main", "Base and compare branches must be different."),
    ] {
        let error = create_github_pull_request_with(
            &gh,
            "https://github.com/acme/app",
            title,
            "body",
            head,
            base,
        )
        .expect_err("invalid create");
        assert_eq!(error_code(&error), "github_pulls_failed");
        assert_eq!(
            serde_json::to_value(error).expect("serialize")["message"],
            expected,
        );
    }
}

#[test]
fn create_rejects_more_than_256_unicode_scalars_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
    let error = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        &"é".repeat(257),
        "",
        "feature",
        "main",
    )
    .expect_err("long title");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[cfg(unix)]
#[test]
fn create_posts_exact_json_and_returns_the_created_pull() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("gh");
    let output = projected_pull(44, false).to_string();
    let script = format!(
        "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\n  *--method*POST*)\n    previous=\"\"\n    for argument in \"$@\"; do\n      if [ \"$previous\" = \"--input\" ]; then cp \"$argument\" \"$root/input.json\"; fi\n      previous=\"$argument\"\n    done\n    printf '%s' '{}'\n    ;;\n  *) exit 1 ;;\nesac\n",
        output,
    );
    std::fs::write(&path, script).expect("write fake gh");
    let mut permissions = std::fs::metadata(&path).expect("stat").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let pull = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "  Add docs  ",
        " body with surrounding space ",
        "feature/readme",
        "main",
    )
    .expect("create");
    assert_eq!(pull.number, 44);
    let input: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
    )
    .expect("json");
    assert_eq!(input, json!({
        "title": "Add docs",
        "body": " body with surrounding space ",
        "head": "feature/readme",
        "base": "main"
    }));
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("--method POST"));
    assert!(calls.contains("/repos/acme/app/pulls"));
    assert!(calls.contains("--input"));
    assert!(!calls.contains("head_repo"));
    assert!(!calls.contains("draft"));
}

#[cfg(unix)]
#[test]
fn create_rejects_a_foreign_returned_url() {
    let mut item = projected_pull(44, false);
    item["html_url"] = json!("https://evil.example/pull/44");
    let (_dir, path) = fake_gh(&item, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "Add docs",
        "",
        "feature/readme",
        "main",
    )
    .expect_err("foreign URL");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn accepts_only_repo_bound_issue_comment_urls() {
    let repo = GitHubRepoRef::parse("https://github.com/Acme/App").expect("repo");
    for (raw, number, id, expected) in [
        ("https://github.com/acme/app/issues/42#issuecomment-9", 42, 9, true),
        ("https://github.com/acme/app/pull/42#issuecomment-9", 42, 9, true),
        ("https://github.com/ACME/APP/pull/42#issuecomment-9", 42, 9, true),
        ("https://github.com/acme/app/pulls/42#issuecomment-9", 42, 9, false),
        ("https://github.com/acme/app/pull/43#issuecomment-9", 42, 9, false),
        ("https://github.com/acme/app/pull/42#issuecomment-8", 42, 9, false),
        ("https://github.com/acme/app/pull/42?x=1#issuecomment-9", 42, 9, false),
    ] {
        assert_eq!(is_pull_comment_html_url(&repo, raw, number, id), expected, "{raw}");
    }
}

#[cfg(unix)]
#[test]
fn comments_keep_github_order_and_drop_foreign_urls() {
    let output = json!([
        { "id": "bad", "body": 7, "html_url": null, "created_at": false, "user": null },
        { "id": 2, "body": "first", "html_url": "https://github.com/acme/app/issues/42#issuecomment-2", "created_at": "2026-01-02T03:04:05Z", "user": { "login": "grace", "avatar_url": "https://example.com/a" } },
        { "id": 10, "body": "second", "html_url": "https://github.com/acme/app/pull/42#issuecomment-10", "created_at": "2026-01-02T04:04:05Z", "user": { "login": "linus", "avatar_url": "https://example.com/b" } },
        { "id": 11, "body": "foreign", "html_url": "https://evil.example/comment/11", "created_at": "2026-01-02T05:04:05Z", "user": { "login": "mallory", "avatar_url": "" } }
    ]);
    let (dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let comments = list_github_pull_request_comments_with(
        &gh,
        "https://github.com/acme/app",
        42,
    )
    .expect("comments");
    assert_eq!(comments.iter().map(|comment| comment.body.as_str()).collect::<Vec<_>>(), vec!["first", "second"]);
    assert_eq!(comments[1].user.login, "linus");
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("/repos/acme/app/issues/42/comments?per_page=100"));
}

#[test]
fn comments_reject_number_zero_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
    let error = list_github_pull_request_comments_with(
        &gh,
        "https://github.com/acme/app",
        0,
    )
    .expect_err("zero");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn error_remapping_preserves_recovery_codes() {
    let missing = GhRunner::from_resolved(None).expect_err("missing");
    assert_eq!(
        error_code(&remap_pulls_error(missing, "", "github_repo_unavailable")),
        "github_cli_missing",
    );
    let not_found = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 404");
    assert_eq!(
        error_code(&remap_pulls_error(not_found, "Not Found", "github_pr_unavailable")),
        "github_pr_unavailable",
    );
    let forbidden = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
    assert_eq!(
        error_code(&remap_pulls_error(forbidden, "Forbidden", "github_pr_unavailable")),
        "github_repo_unavailable",
    );
    let limited = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
    assert_eq!(
        error_code(&remap_pulls_error(limited, "API rate limit exceeded", "github_repo_unavailable")),
        "github_pulls_failed",
    );
}

#[cfg(unix)]
#[test]
fn failed_auth_maps_to_auth_required_before_api_access() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("gh");
    std::fs::write(
        &path,
        "#!/bin/sh\ncase \"$*\" in *auth*status*) exit 1 ;; *) exit 99 ;; esac\n",
    )
    .expect("write fake gh");
    let mut permissions = std::fs::metadata(&path).expect("stat").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = list_github_pull_requests_with(&gh, "https://github.com/acme/app")
        .expect_err("auth");
    assert_eq!(error_code(&error), "github_auth_required");
}

#[test]
fn runner_wrappers_preserve_missing_cli_recovery() {
    let list_error = list_github_pull_requests_with_runner(
        "https://github.com/acme/app".to_string(),
        GhRunner::from_resolved(None),
    )
    .expect_err("list missing");
    assert_eq!(error_code(&list_error), "github_cli_missing");

    let create_error = create_github_pull_request_with_runner(
        "https://github.com/acme/app".to_string(),
        "title".to_string(),
        "body".to_string(),
        "feature".to_string(),
        "main".to_string(),
        GhRunner::from_resolved(None),
    )
    .expect_err("create missing");
    assert_eq!(error_code(&create_error), "github_cli_missing");

    let comments_error = list_github_pull_request_comments_with_runner(
        "https://github.com/acme/app".to_string(),
        42,
        GhRunner::from_resolved(None),
    )
    .expect_err("comments missing");
    assert_eq!(error_code(&comments_error), "github_cli_missing");
}
```

- [ ] **Step 3: Run the focused native test and verify RED.**

Run: `. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pulls -- --nocapture`

Expected: FAIL because create, JSON input, comment mapping, and command wrappers do not exist.

- [ ] **Step 4: Implement create and read-only comments.**

Add a `PULL_ITEM_JQ` projection with the same fields as a single `PULL_LIST_JQ` item.
Add `PULL_COMMENTS_JQ` with `id`, body defaulting to `""`, `html_url`, `created_at`, and optional projected user.

```rust
/// One bounded read-only GitHub pull-request conversation comment.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestCommentDto {
    pub id: u64,
    pub body: String,
    pub html_url: String,
    pub created_at: i64,
    pub user: GitHubPullRequestUserDto,
}

fn pull_json_input(
    value: &serde_json::Value,
) -> Result<tempfile::NamedTempFile, ProjectPullRequestMergeError> {
    use std::io::Write;
    let mut file = tempfile::Builder::new()
        .prefix("buzz-gh-")
        .tempfile()
        .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?;
    serde_json::to_writer(&mut file, value)
        .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?;
    file.flush()
        .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?;
    Ok(file)
}
```

Implement `create_github_pull_request_with` so every validation precedes repository parsing, auth, tempfile creation, and `gh` execution.
Pass `title.trim()` and the original body/head/base strings in the exact JSON object.
Deserialize the projected create response as `serde_json::Value`, map it with `map_pull`, and return `github_pulls_failed` with `GitHub returned an invalid pull request response.` when mapping returns `None`.

Implement `list_github_pull_request_comments_with` using `/repos/{slug}/issues/{number}/comments?per_page=100`, `github_pr_unavailable` as its not-found code, and a `Vec<serde_json::Value>` response.
For each comment value, attempt wire deserialization inside `map_pull_comment` and use `filter_map` without sorting so malformed comments are dropped individually while valid API order is preserved.

Add runner wrappers that accept an injected `Result<GhRunner, ProjectPullRequestMergeError>` so missing-binary behavior is unit-testable before async dispatch.
Add these exact async command signatures.

```rust
/// List one bounded page of open GitHub pull requests.
#[tauri::command]
pub async fn list_github_pull_requests(
    clone_url: String,
) -> Result<GitHubPullRequestListDto, ProjectPullRequestMergeError>;

/// Create one ready same-repository GitHub pull request.
#[tauri::command]
pub async fn create_github_pull_request(
    clone_url: String,
    title: String,
    body: String,
    head: String,
    base: String,
) -> Result<GitHubPullRequestDto, ProjectPullRequestMergeError>;

/// List one bounded page of read-only GitHub PR conversation comments.
#[tauri::command]
pub async fn list_github_pull_request_comments(
    clone_url: String,
    number: u64,
) -> Result<Vec<GitHubPullRequestCommentDto>, ProjectPullRequestMergeError>;
```

Each command must use `tauri::async_runtime::spawn_blocking`, call its runner wrapper with `GhRunner::discover()`, and map join failure to `github_pulls_failed`.

- [ ] **Step 5: Re-export and register the commands.**

Add `pub use project_github_pulls::*;` beside the other GitHub command re-exports in `commands/mod.rs`.
Add these entries after `get_github_repository_state` and before the existing issue commands in `invoke.rs`.

```rust
list_github_pull_requests,
create_github_pull_request,
list_github_pull_request_comments,
```

- [ ] **Step 6: Run native tests, formatting, and the file-size gate.**

Run: `. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pulls -- --nocapture`

Expected: PASS.

Run: `. ./bin/activate-hermit && cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --all -- --check`

Expected: PASS.

Run: `. ./bin/activate-hermit && node desktop/scripts/check-file-sizes.mjs`

Expected: PASS with the new module below 1,000 lines and `project_github_pull_request.rs` unchanged at 998 lines.

- [ ] **Step 7: Commit Task 2.**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_pulls.rs desktop/src-tauri/src/commands/mod.rs desktop/src-tauri/src/invoke.rs
```

Run the staged GitNexus scope check when available.
If it is unavailable, run `git diff --cached --stat`, `git diff --cached --name-only`, and `git diff --cached --check` and confirm only the Task 2 files are staged.

```bash
. ./bin/activate-hermit
git commit -s -m "feat(desktop): create and read GitHub pull requests"
```

---

### Task 3: Add frontend DTOs, shared mapping, query routing, identity filtering, and share links

**Files:**

- Create: `desktop/src/shared/api/projectGithubPulls.ts`
- Modify: `desktop/src/features/projects/projectPullRequests.mjs:330-390`
- Modify: `desktop/src/features/projects/projectPullRequests.d.mts:1-120`
- Modify: `desktop/src/features/projects/projectPullRequests.test.mjs`
- Create: `desktop/src/features/projects/lib/projectGithubPullRequests.ts`
- Create: `desktop/src/features/projects/lib/projectGithubPullRequests.test.mjs`
- Modify: `desktop/src/features/projects/lib/projectShareLinks.ts:145-182`
- Modify: `desktop/src/features/projects/lib/projectShareLinks.test.mjs`
- Modify: `desktop/src/features/projects/hooks.ts:231-270,803-815`
- Modify: `desktop/src/features/projects/ui/CreatePullRequestDialog.tsx:35-125`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx:150-460,750-930`

**Interfaces:**

- Consumes: the three Task 2 commands, `isGitHubCloneUrl`, `projectPullRequestEventsToPullRequests`, and the existing PR query key.
- Produces: `GithubPullRequestDto`, `GithubPullRequestListDto`, `GithubPullRequestCommentDto`, `ProjectPullRequestsResult`, `fetchProjectPullRequestsWith`, `mapGithubPullRequestToProjectPullRequest`, `useGithubPullRequestCommentsQuery`, `pullRequestIdentityPubkeys`, `githubPullRequestBranchLabel`, `githubPullRequestDetailTab`, `selectedGithubPullRequestAfterListLoad`, `requireNostrPullRequestId`, stable array-shaped consumers, and GitHub-aware `pullRequestShareLink`.

- [ ] **Step 1: Run the Task 3 impact checks.**

Report direct callers, affected processes, and risk before editing.
Stop and warn if a result is HIGH or CRITICAL.

- [ ] **Step 2: Write failing model and routing tests.**

Create `projectGithubPullRequests.test.mjs` with one canonical DTO and these exact contracts.

```javascript
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchProjectPullRequestsWith,
  githubPullRequestBranchLabel,
  githubPullRequestCommentsRequest,
  githubPullRequestDetailTab,
  githubPullRequestId,
  mapGithubPullRequestComment,
  mapGithubPullRequestToProjectPullRequest,
  parseGithubPullRequestNumber,
  pullRequestIdentityPubkeys,
  requireNostrPullRequestId,
  selectedGithubPullRequestAfterListLoad,
} from "./projectGithubPullRequests.ts";

const OWNER = "a".repeat(64);
const REPO_ADDRESS = `30617:${OWNER}:app`;
const dto = {
  number: 42,
  title: "Add docs",
  body: "Details",
  html_url: "https://github.com/acme/app/pull/42",
  draft: false,
  created_at: 1_704_166_645,
  updated_at: 1_704_253_045,
  comments: 3,
  user: { login: "ada", avatar_url: "https://avatars.githubusercontent.com/u/1" },
  head: {
    ref: "feature/readme",
    sha: "1".repeat(40),
    repo: { full_name: "acme/app" },
  },
  base: { ref: "main", repo: { full_name: "acme/app" } },
};

test("GitHub pull mapper fills the complete shared contract", () => {
  assert.deepEqual(
    mapGithubPullRequestToProjectPullRequest(
      dto,
      REPO_ADDRESS,
      "https://github.com/acme/app",
    ),
    {
      id: "42",
      title: "Add docs",
      content: "Details",
      tags: [],
      author: "ada",
      authorAvatarUrl: "https://avatars.githubusercontent.com/u/1",
      createdAt: 1_704_166_645,
      repoAddress: REPO_ADDRESS,
      channelId: null,
      originAgentName: null,
      labels: [],
      recipients: [],
      reviewers: [],
      approvals: [],
      changeRequests: [],
      status: "Open",
      statusEventId: null,
      statusCreatedAt: null,
      branchName: "feature/readme",
      targetBranch: "main",
      headRepoFullName: "acme/app",
      initialCommit: "1".repeat(40),
      commit: "1".repeat(40),
      cloneUrls: ["https://github.com/acme/app"],
      updateCount: 0,
      updatedAt: 1_704_253_045,
      updates: [],
      comments: [],
      commentCount: 3,
      htmlUrl: "https://github.com/acme/app/pull/42",
    },
  );
  assert.equal(
    mapGithubPullRequestToProjectPullRequest(
      { ...dto, draft: true },
      REPO_ADDRESS,
      "https://github.com/acme/app",
    ).status,
    "Draft",
  );
});

test("GitHub pull ids accept only positive safe integers", () => {
  assert.equal(githubPullRequestId(42), "42");
  assert.equal(parseGithubPullRequestNumber("42"), 42);
  for (const value of ["0", "01", "0x2", "9007199254740992", "e".repeat(64)]) {
    assert.equal(parseGithubPullRequestNumber(value), null, value);
  }
  for (const number of [0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => githubPullRequestId(number), /invalid pull request number/);
  }
});

test("GitHub comment mapper keeps login and avatar without pubkey conversion", () => {
  assert.deepEqual(
    mapGithubPullRequestComment({
      id: 9,
      body: "Looks good.",
      html_url: "https://github.com/acme/app/pull/42#issuecomment-9",
      created_at: 1_704_253_100,
      user: { login: "grace", avatar_url: "https://avatars.githubusercontent.com/u/2" },
    }),
    {
      id: "9",
      content: "Looks good.",
      tags: [],
      author: "grace",
      authorAvatarUrl: "https://avatars.githubusercontent.com/u/2",
      createdAt: 1_704_253_100,
      commit: null,
      anchor: null,
      inlineCommentStatus: null,
      isInlineComment: false,
      isApproval: false,
      isChangeRequest: false,
      isReviewRequest: false,
      isTrustedReviewDecision: false,
      isTrustedReviewRequest: false,
      reviewDecision: null,
      reviewDecisionStatus: null,
      reviewerPubkeys: [],
    },
  );
});

test("host routing invokes only GitHub for github.com", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectPullRequestsWith(
    { id: "p1", repoAddress: REPO_ADDRESS, cloneUrls: ["https://github.com/acme/app"] },
    {
      loadGithub: async ({ cloneUrl }) => {
        calls.github += 1;
        assert.equal(cloneUrl, "https://github.com/acme/app");
        return { pulls: [dto], has_more: true };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [];
      },
    },
  );
  assert.deepEqual(calls, { github: 1, buzz: 0 });
  assert.equal(result.pullRequests[0].id, "42");
  assert.equal(result.hasMore, true);
});

test("host routing invokes only Nostr for a Buzz clone URL", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectPullRequestsWith(
    {
      id: "p2",
      repoAddress: REPO_ADDRESS,
      cloneUrls: [`https://relay.example/git/${OWNER}/app`],
    },
    {
      loadGithub: async () => {
        calls.github += 1;
        return { pulls: [dto], has_more: false };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [{ id: "e".repeat(64) }];
      },
    },
  );
  assert.deepEqual(calls, { github: 0, buzz: 1 });
  assert.equal(result.pullRequests[0].id, "e".repeat(64));
  assert.equal(result.hasMore, false);
});

test("branch labels prefix only an inbound fork owner", () => {
  const pull = mapGithubPullRequestToProjectPullRequest(
    { ...dto, head: { ...dto.head, repo: { full_name: "fork-owner/app" } } },
    REPO_ADDRESS,
    "https://github.com/acme/app",
  );
  assert.equal(githubPullRequestBranchLabel(pull, "https://github.com/acme/app"), "fork-owner:feature/readme");
  assert.equal(
    githubPullRequestBranchLabel(
      { ...pull, headRepoFullName: "ACME/APP" },
      "https://github.com/acme/app",
    ),
    "feature/readme",
  );
});

test("identity collection drops GitHub logins and keeps lowercase Nostr pubkeys", () => {
  const github = mapGithubPullRequestToProjectPullRequest(dto, REPO_ADDRESS, "https://github.com/acme/app");
  const nostr = {
    ...github,
    id: "f".repeat(64),
    author: "A".repeat(64),
    recipients: ["B".repeat(64)],
    reviewers: ["C".repeat(64)],
    comments: [{ ...mapGithubPullRequestComment({ id: 1, body: "x", html_url: "", created_at: 1, user: { login: "x", avatar_url: "" } }), author: "D".repeat(64) }],
  };
  assert.deepEqual(pullRequestIdentityPubkeys([github]), []);
  assert.deepEqual(pullRequestIdentityPubkeys([nostr]), [
    "a".repeat(64),
    "b".repeat(64),
    "c".repeat(64),
    "d".repeat(64),
  ]);
});

test("comment requests validate host, number, and exact query key", () => {
  assert.deepEqual(
    githubPullRequestCommentsRequest(
      { id: "p1", cloneUrls: ["https://github.com/acme/app"] },
      "42",
    ),
    {
      cloneUrl: "https://github.com/acme/app",
      number: 42,
      queryKey: ["project", "p1", "pull-requests", 42, "comments"],
    },
  );
  assert.equal(
    githubPullRequestCommentsRequest(
      { id: "p1", cloneUrls: ["https://github.com/acme/app"] },
      "0",
    ),
    null,
  );
});

test("Nostr write guard refuses GitHub numbers", () => {
  assert.equal(requireNostrPullRequestId("f".repeat(64)), "f".repeat(64));
  assert.throws(() => requireNostrPullRequestId("42"), /64-hex Nostr event id/);
});

test("created selection waits for a settled list before clearing", () => {
  assert.equal(
    selectedGithubPullRequestAfterListLoad({
      selectedPullRequestId: "43",
      pullRequestIds: ["42"],
      isSuccess: true,
      isFetching: true,
    }),
    "43",
  );
  assert.equal(
    selectedGithubPullRequestAfterListLoad({
      selectedPullRequestId: "43",
      pullRequestIds: ["42", "43"],
      isSuccess: true,
      isFetching: false,
    }),
    "43",
  );
  assert.equal(
    selectedGithubPullRequestAfterListLoad({
      selectedPullRequestId: "43",
      pullRequestIds: ["42"],
      isSuccess: true,
      isFetching: false,
    }),
    null,
  );
});

test("GitHub detail snaps stale Files changed state to Conversation", () => {
  assert.equal(
    githubPullRequestDetailTab("pr-files", true),
    "pr-conversation",
  );
  assert.equal(githubPullRequestDetailTab("pr-files", false), "pr-files");
  assert.equal(
    githubPullRequestDetailTab("pr-commits", true),
    "pr-commits",
  );
});
```

Append a Nostr mapper regression test in `projectPullRequests.test.mjs` that asserts `authorAvatarUrl`, `headRepoFullName`, and `htmlUrl` are `null` and `commentCount === comments.length`.
Append PR share-link tests alongside the existing issue URL tests.

```javascript
test("pullRequestShareLink accepts only a matching canonical GitHub PR URL", () => {
  const base = { id: "42", repoAddress: REPO_ADDRESS };
  assert.equal(
    pullRequestShareLink({
      ...base,
      htmlUrl: "https://github.com/acme/app/pull/42",
    }),
    "https://github.com/acme/app/pull/42",
  );
  for (const htmlUrl of [
    "https://evil.example/acme/app/pull/42",
    "https://github.com/acme/app/pull/43",
    "https://github.com/acme/app/pull/42?x=1",
    "https://github.com/acme/app/pull/42#x",
    "https://github.com/acme/app/pull/42/",
    "https://github.com/acme/app/issues/42",
  ]) {
    assert.equal(pullRequestShareLink({ ...base, htmlUrl }), null, htmlUrl);
  }
});

test("pullRequestShareLink preserves the existing Buzz deep link", () => {
  assert.equal(
    pullRequestShareLink({ id: EVENT_ID, repoAddress: REPO_ADDRESS, htmlUrl: null }),
    `buzz://pr?id=${EVENT_ID}&owner=${OWNER}&d=flappy-bee`,
  );
});
```

- [ ] **Step 3: Run the focused frontend tests and verify RED.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectGithubPullRequests.test.mjs src/features/projects/projectPullRequests.test.mjs src/features/projects/lib/projectShareLinks.test.mjs`

Expected: FAIL because the new API/model fields, mapper, router, query request, guard, and PR share URL support do not exist.

- [ ] **Step 4: Add the typed Tauri API.**

Create `projectGithubPulls.ts` with exported DTOs matching Task 2's snake-case serialization.
Use nested `{ ref, sha, repo: { full_name } }` and `{ ref, repo: { full_name } }` shapes exactly.

```typescript
import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import { invokeTauri } from "@/shared/api/tauri";

/** Bounded GitHub login identity returned by the native PR commands. */
export type GithubPullRequestUserDto = { login: string; avatar_url: string };
/** Bounded GitHub repository identity returned by the native PR commands. */
export type GithubPullRequestRepoDto = { full_name: string };
/** One bounded GitHub pull request returned by the native PR commands. */
export type GithubPullRequestDto = {
  number: number;
  title: string;
  body: string;
  html_url: string;
  draft: boolean;
  created_at: number;
  updated_at: number;
  comments: number;
  user: GithubPullRequestUserDto;
  head: { ref: string; sha: string; repo: GithubPullRequestRepoDto };
  base: { ref: string; repo: GithubPullRequestRepoDto };
};
/** One GitHub pull-request page and its first-page truncation signal. */
export type GithubPullRequestListDto = {
  pulls: GithubPullRequestDto[];
  has_more: boolean;
};
/** One bounded read-only GitHub pull-request conversation comment. */
export type GithubPullRequestCommentDto = {
  id: number;
  body: string;
  html_url: string;
  created_at: number;
  user: GithubPullRequestUserDto;
};
```

Export `listGithubPullRequests`, `createGithubPullRequest`, and `listGithubPullRequestComments` wrappers.
Each wrapper must pass camel-case Tauri input keys, invoke the exact snake-case command name, and rethrow `parseProjectPullRequestMergeError(error) ?? error`.

```typescript
async function invokeGithubPulls<T>(
  command: string,
  input: Record<string, unknown>,
): Promise<T> {
  try {
    return await invokeTauri<T>(command, input);
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** List the first bounded page of open GitHub pull requests. */
export function listGithubPullRequests(input: {
  cloneUrl: string;
}): Promise<GithubPullRequestListDto> {
  return invokeGithubPulls("list_github_pull_requests", input);
}

/** Create one ready same-repository GitHub pull request. */
export function createGithubPullRequest(input: {
  cloneUrl: string;
  title: string;
  body: string;
  head: string;
  base: string;
}): Promise<GithubPullRequestDto> {
  return invokeGithubPulls("create_github_pull_request", input);
}

/** List the first read-only issue-comment page for one GitHub pull request. */
export function listGithubPullRequestComments(input: {
  cloneUrl: string;
  number: number;
}): Promise<GithubPullRequestCommentDto[]> {
  return invokeGithubPulls("list_github_pull_request_comments", input);
}
```

- [ ] **Step 5: Extend the shared PR model without changing Buzz semantics.**

Add `authorAvatarUrl: string | null`, `headRepoFullName: string | null`, `htmlUrl: string | null`, and `commentCount: number` to `ProjectPullRequest`.
Add `authorAvatarUrl: string | null` to `ProjectPullRequestComment`.
In `eventToProjectPullRequest`, add neutral defaults and set `commentCount` after the parsed `comments` array exists.

```javascript
return {
  id: pullRequest.id,
  title,
  content: pullRequest.content,
  tags: getImetaTags(pullRequest),
  author: pullRequest.pubkey,
  authorAvatarUrl: null,
  createdAt: pullRequest.created_at,
  repoAddress: getTag(pullRequest, "a") ?? null,
  channelId: getTag(pullRequest, "h") ?? null,
  originAgentName: getTag(pullRequest, "buzz-origin-agent") ?? null,
  labels: getAllTags(pullRequest, "t"),
  recipients: getAllTags(pullRequest, "p"),
  reviewers,
  approvals: reviewDecisions.approvals,
  changeRequests: reviewDecisions.changeRequests,
  status: statusFromEvent(pullRequest, latestStatus),
  statusEventId: latestStatus?.id ?? null,
  statusCreatedAt: latestStatus?.created_at ?? null,
  branchName: getTag(pullRequest, "branch-name") ?? null,
  targetBranch: getTag(pullRequest, "target-branch") ?? null,
  headRepoFullName: null,
  initialCommit,
  commit: latestCommit,
  cloneUrls: getCloneUrls(latestUpdate ?? pullRequest),
  updateCount: updates.length,
  updatedAt,
  updates,
  comments,
  commentCount: comments.length,
  htmlUrl: null,
};
```

Add `authorAvatarUrl: null` in `eventToPullRequestComment`.
Preserve the existing `updatedAt` expression instead of duplicating or simplifying it.

- [ ] **Step 6: Implement the shared GitHub PR mapper and query.**

Create `projectGithubPullRequests.ts` with the interfaces pinned by the tests.
Use these exported signatures.

```typescript
/** Host-routed pull-request list consumed by one repository PR tab. */
export type ProjectPullRequestsResult = {
  pullRequests: ProjectPullRequest[];
  hasMore: boolean;
};

/** Convert a positive safe GitHub PR number into its decimal selection id. */
export function githubPullRequestId(number: number): string;

/** Parse a positive safe decimal GitHub PR selection id. */
export function parseGithubPullRequestNumber(
  value: string | null | undefined,
): number | null;

/** Map one bounded GitHub PR onto the shared Projects PR model. */
export function mapGithubPullRequestToProjectPullRequest(
  dto: GithubPullRequestDto,
  repoAddress: string,
  cloneUrl: string,
): ProjectPullRequest;

/** Map one bounded GitHub issue comment onto the shared PR comment model. */
export function mapGithubPullRequestComment(
  dto: GithubPullRequestCommentDto,
): ProjectPullRequestComment;

/** Route one repository to exactly one pull-request backend. */
export async function fetchProjectPullRequestsWith(
  project: Pick<Repository, "id" | "repoAddress" | "cloneUrls">,
  loaders: {
    loadGithub: (input: {
      cloneUrl: string;
    }) => Promise<GithubPullRequestListDto>;
    loadBuzz: () => Promise<ProjectPullRequest[]>;
  },
): Promise<ProjectPullRequestsResult>;

/** Display the fork owner prefix only when the PR head repository differs. */
export function githubPullRequestBranchLabel(
  pullRequest: Pick<ProjectPullRequest, "branchName" | "headRepoFullName">,
  targetCloneUrl: string,
): string;

/** Collect only valid Nostr identities for profile batch lookup. */
export function pullRequestIdentityPubkeys(
  pullRequests: ProjectPullRequest[],
): string[];

/** Resolve stale Files changed state away from a GitHub PR detail. */
export function githubPullRequestDetailTab(
  selectedTab: string,
  githubHosted: boolean,
): string;
```

Implement `fetchProjectPullRequestsWith` by choosing the first clone URL, invoking only the GitHub loader for `github.com`, invoking only the Buzz loader otherwise, mapping GitHub DTOs, and returning `{ pullRequests, hasMore }` with `hasMore: false` for Buzz.

Implement `useGithubPullRequestCommentsQuery` with `enabled: request !== null`, the exact request key, the Task 3 API wrapper, mapping through `mapGithubPullRequestComment`, and `staleTime: 30_000`.

Implement `pullRequestIdentityPubkeys` by collecting author, recipients, reviewers, update authors, comment authors, approval authors, and change-request authors, then retaining only 64-hex values and lowercasing/deduplicating them.

Implement the stale-tab resolver exactly as follows.

```typescript
export function githubPullRequestDetailTab(
  selectedTab: string,
  githubHosted: boolean,
): string {
  return githubHosted && selectedTab === "pr-files"
    ? "pr-conversation"
    : selectedTab;
}
```

Implement `selectedGithubPullRequestAfterListLoad` so an existing selected ID stays selected, a missing ID stays selected during initial load or refetch, and a missing ID clears only after a successful non-fetching result.

```typescript
export function selectedGithubPullRequestAfterListLoad(input: {
  selectedPullRequestId: string | null;
  pullRequestIds: readonly string[];
  isSuccess: boolean;
  isFetching: boolean;
}): string | null {
  if (input.selectedPullRequestId === null) return null;
  if (input.pullRequestIds.includes(input.selectedPullRequestId)) {
    return input.selectedPullRequestId;
  }
  if (input.isSuccess && !input.isFetching) return null;
  return input.selectedPullRequestId;
}
```

Implement `requireNostrPullRequestId` as follows.

```typescript
/** Require a canonical Nostr event id before entering a Nostr PR write. */
export function requireNostrPullRequestId(id: string): string {
  if (!/^[a-fA-F0-9]{64}$/.test(id)) {
    throw new Error("Pull request writes require a 64-hex Nostr event id.");
  }
  return id.toLowerCase();
}
```

- [ ] **Step 7: Route the existing query and update every array-shaped consumer in the same green change.**

Rename the current private loader to `fetchBuzzProjectPullRequests` and leave all four event queries unchanged.
Import `listGithubPullRequests` and `fetchProjectPullRequestsWith`.
Change `useProjectPullRequestsQuery` to return the result object.

```typescript
queryFn: () => {
  if (!project) throw new Error("No project selected.");
  return fetchProjectPullRequestsWith(project, {
    loadGithub: listGithubPullRequests,
    loadBuzz: () => fetchBuzzProjectPullRequests(project),
  });
},
```

In `ProjectDetailScreen`, derive one reference-stable array with `React.useMemo` and replace every direct `pullRequestsQuery.data` array operation with it.
Do not use `pullRequestsQuery.data?.pullRequests ?? []` directly because that allocates a fresh fallback array on every render and defeats memoization.

```typescript
const pullRequests = React.useMemo(
  () => pullRequestsQuery.data?.pullRequests ?? [],
  [pullRequestsQuery.data?.pullRequests],
);
```

Use `pullRequests` for referenced branches, branch matching, active selection, branch management, people lookup, breadcrumb selection, and the `WorkspaceTabs` prop.
Update every memo/effect dependency from `pullRequestsQuery.data` to `pullRequests` when that computation reads the derived array.

In `CreatePullRequestDialog`, derive the same reference-stable array and run duplicate-open detection against `pullRequests`.
Do not change repository selection, repo-state loading, branch selection, source-commit validation, form preservation, or toast behavior in this task.

- [ ] **Step 8: Add strict GitHub PR share validation.**

Add a private validator mirroring `isSafeGitHubIssueUrl`, but require the exact `/pull/{number}` path and require the URL number to equal `pullRequest.id` after positive-safe-decimal parsing.
Check `pullRequest.htmlUrl` first in `pullRequestShareLink`, then preserve the existing coordinate plus 64-hex `buzz://pr` branch unchanged.

```typescript
function isSafeGitHubPullRequestUrl(
  raw: string,
  pullRequestId: string,
): boolean {
  const expectedNumber = parseGithubPullRequestNumber(pullRequestId);
  if (
    expectedNumber === null ||
    raw !== raw.trim() ||
    !raw.startsWith("https://github.com/") ||
    raw.endsWith("/") ||
    raw.includes("\\")
  ) {
    return false;
  }
  try {
    const url = new URL(raw);
    if (
      url.protocol !== "https:" ||
      url.hostname !== "github.com" ||
      url.port !== "" ||
      url.username !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== "" ||
      url.pathname.includes("%") ||
      url.pathname.includes("//")
    ) {
      return false;
    }
    const [owner, repo, kind, number, ...rest] = url.pathname
      .split("/")
      .filter(Boolean);
    return (
      rest.length === 0 &&
      /^[A-Za-z0-9-]+$/.test(owner ?? "") &&
      /^[A-Za-z0-9._-]+$/.test(repo ?? "") &&
      kind === "pull" &&
      parseGithubPullRequestNumber(number) === expectedNumber &&
      `https://github.com/${owner}/${repo}/pull/${number}` === raw
    );
  } catch {
    return false;
  }
}
```

- [ ] **Step 9: Run the focused tests and typecheck.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectGithubPullRequests.test.mjs src/features/projects/projectPullRequests.test.mjs src/features/projects/lib/projectShareLinks.test.mjs`

Expected: PASS.

Run: `. ./bin/activate-hermit && cd desktop && pnpm typecheck`

Expected: PASS with no DTO, mapper, query-shape, or consumer errors.

- [ ] **Step 10: Commit Task 3.**

Do not commit while any focused test or typecheck error remains.

```bash
. ./bin/activate-hermit
git add desktop/src/shared/api/projectGithubPulls.ts desktop/src/features/projects/projectPullRequests.mjs desktop/src/features/projects/projectPullRequests.d.mts desktop/src/features/projects/projectPullRequests.test.mjs desktop/src/features/projects/lib/projectGithubPullRequests.ts desktop/src/features/projects/lib/projectGithubPullRequests.test.mjs desktop/src/features/projects/lib/projectShareLinks.ts desktop/src/features/projects/lib/projectShareLinks.test.mjs desktop/src/features/projects/hooks.ts desktop/src/features/projects/ui/CreatePullRequestDialog.tsx desktop/src/features/projects/ui/ProjectDetailScreen.tsx
```

Run the staged GitNexus scope check when available.
If it is unavailable, run `git diff --cached --stat`, `git diff --cached --name-only`, and `git diff --cached --check` and confirm only the Task 3 files are staged.

```bash
. ./bin/activate-hermit
git commit -s -m "feat(desktop): map GitHub pull request data"
```

---

### Task 4: Route create, preserve invalidations, and close every Nostr write escape hatch

**Files:**

- Modify: `desktop/src/features/projects/pullRequestMutations.ts`
- Modify: `desktop/src/features/projects/pullRequestMutations.test.mjs`
- Modify: `desktop/src/features/projects/pullRequestReviews.ts`
- Modify: `desktop/src/features/projects/hooks.ts:270-360`

**Interfaces:**

- Consumes: `createGithubPullRequest`, `githubPullRequestId`, `isGitHubCloneUrl`, and `requireNostrPullRequestId`.
- Produces: `createProjectPullRequestWith`, `projectPullRequestInvalidationKeys`, a host-routed create hook, and defense-in-depth Nostr-id guards.

- [ ] **Step 1: Run the Task 4 impact checks.**

Report direct callers, affected processes, and risk before editing.
Stop and warn if a result is HIGH or CRITICAL.

- [ ] **Step 2: Write failing create-routing and guard tests.**

Extend `pullRequestMutations.test.mjs` with these tests.

```javascript
import {
  createProjectPullRequestWith,
  projectPullRequestInvalidationKeys,
} from "./pullRequestMutations.ts";

test("GitHub create never calls the Buzz pull request publisher", async () => {
  const calls = { github: 0, buzz: 0 };
  const project = {
    id: "p1",
    owner: OWNER,
    repoAddress: `30617:${OWNER}:app`,
    cloneUrls: ["https://github.com/acme/app"],
  };
  const id = await createProjectPullRequestWith(
    project,
    {
      title: "Add docs",
      body: "Details",
      branch: "feature/readme",
      targetBranch: "main",
      commit: COMMIT,
      mergeBase: MERGE_BASE,
      reviewers: [REVIEWER],
    },
    {
      createGithub: async (input) => {
        calls.github += 1;
        assert.deepEqual(input, {
          cloneUrl: "https://github.com/acme/app",
          title: "Add docs",
          body: "Details",
          head: "feature/readme",
          base: "main",
        });
        return { number: 44 };
      },
      publishBuzz: async () => {
        calls.buzz += 1;
        return "e".repeat(64);
      },
    },
  );
  assert.equal(id, "44");
  assert.deepEqual(calls, { github: 1, buzz: 0 });
});

test("Buzz create never calls the GitHub creator", async () => {
  const calls = { github: 0, buzz: 0 };
  const id = await createProjectPullRequestWith(
    project,
    {
      title: "Buzz PR",
      body: "",
      branch: "feature",
      targetBranch: "main",
      commit: COMMIT,
      mergeBase: null,
      reviewers: [],
    },
    {
      createGithub: async () => {
        calls.github += 1;
        return { number: 1 };
      },
      publishBuzz: async () => {
        calls.buzz += 1;
        return PR_ID;
      },
    },
  );
  assert.equal(id, PR_ID);
  assert.deepEqual(calls, { github: 0, buzz: 1 });
});

test("GitHub create invalidates only its repository pull request query", () => {
  assert.deepEqual(
    projectPullRequestInvalidationKeys({
      id: "p1",
      cloneUrls: ["https://github.com/acme/app"],
    }),
    [["project", "p1", "pull-requests"]],
  );
});

test("Buzz create preserves all existing invalidations", () => {
  assert.deepEqual(
    projectPullRequestInvalidationKeys({ id: "p2", cloneUrls: project.cloneUrls }),
    [
      ["project", "p2", "pull-requests"],
      ["projects", "work-items"],
      ["projects", "activity-summaries"],
    ],
  );
});

test("Nostr tag builders refuse a numeric GitHub pull request id", () => {
  assert.throws(
    () => projectPullRequestUpdateTags(project, { id: "42", author: AUTHOR, cloneUrls: project.cloneUrls }, COMMIT, null),
    /64-hex Nostr event id/,
  );
  assert.throws(
    () => projectPullRequestMergedTags(project, { id: "42", author: AUTHOR }, COMMIT),
    /64-hex Nostr event id/,
  );
});
```

- [ ] **Step 3: Run the mutation tests and verify RED.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/pullRequestMutations.test.mjs`

Expected: FAIL because create routing, host-specific invalidations, and Nostr-id guards are missing.

- [ ] **Step 4: Implement host-exclusive create and invalidations.**

Export the existing `publishProjectPullRequest` with a doc comment so it can be injected into the routing seam.
Add `createProjectPullRequestWith` and `projectPullRequestInvalidationKeys` with the following behavior.

```typescript
/** Create a pull request through exactly one repository-native backend. */
export async function createProjectPullRequestWith(
  project: Project,
  input: CreateProjectPullRequestInput,
  backends: {
    createGithub: typeof createGithubPullRequest;
    publishBuzz: typeof publishProjectPullRequest;
  },
): Promise<string> {
  const cloneUrl = project.cloneUrls[0] ?? "";
  if (isGitHubCloneUrl(cloneUrl)) {
    const pull = await backends.createGithub({
      cloneUrl,
      title: input.title,
      body: input.body,
      head: input.branch,
      base: input.targetBranch,
    });
    return githubPullRequestId(pull.number);
  }
  return backends.publishBuzz(project, input);
}

/** Host-routed query keys invalidated after pull-request creation. */
export function projectPullRequestInvalidationKeys(
  project: Pick<Project, "id" | "cloneUrls">,
): readonly unknown[][] {
  const pullRequestsKey = ["project", project.id, "pull-requests"];
  return isGitHubCloneUrl(project.cloneUrls[0])
    ? [pullRequestsKey]
    : [
        pullRequestsKey,
        ["projects", "work-items"],
        ["projects", "activity-summaries"],
      ];
}
```

Change `useCreateProjectPullRequestMutation` to use `useQueryClient`, call `createProjectPullRequestWith`, and await all keys from `projectPullRequestInvalidationKeys` on success.
Do not use the existing broad `useProjectPullRequestWriteInvalidation` for create because it would invalidate Nostr-only global data on GitHub success.

- [ ] **Step 5: Add defense-in-depth guards to every Nostr write.**

Call `requireNostrPullRequestId` before any author normalization, signing, relay publication, or Tauri invoke in these paths.

- `projectPullRequestUpdateTags`.
- `projectPullRequestMergedTags`.
- `publishProjectPullRequestUpdate` after the GitHub-host skip and before `getIdentity`.
- `useMergeProjectPullRequestMutation` before `mergeProjectPullRequest`.
- `createProjectPullRequestComment` before body/tag construction.
- `updateProjectPullRequestStatus`.
- `requestProjectPullRequestReview`.
- `submitProjectPullRequestReview`.

Capture the validated ID once at the top of each write and use that value in every `e`, `E`, and native `pullRequestId` field.

```typescript
const pullRequestId = requireNostrPullRequestId(pullRequest.id);
```

For `projectPullRequestUpdateTags` and `projectPullRequestMergedTags`, this line must precede `uniquePubkeys` or `normalizePubkey` calls.
For `createProjectPullRequestComment`, `updateProjectPullRequestStatus`, `requestProjectPullRequestReview`, and `submitProjectPullRequestReview`, this line must precede body/reviewer/recipient normalization and both managed-owner and local-signing branches.
For `useMergeProjectPullRequestMutation`, validate before `mergeProjectPullRequest` and pass the validated `pullRequestId` to the native API.

Make `publishProjectPullRequestUpdate` return `false` immediately when `isGitHubCloneUrl(project.cloneUrls[0])` so a direct caller cannot publish kind `1613` for `"42"` even if it bypasses `repoSyncHooks`.
For a Buzz-hosted repository, validate the ID immediately after the host skip and before `getIdentity`.
Keep the existing `shouldPublishPullRequestUpdateAfterPush` outer guard unchanged.

- [ ] **Step 6: Run mutation and sync regression tests.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm exec node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/pullRequestMutations.test.mjs src/features/projects/lib/projectGithubSync.test.mjs`

Expected: PASS and the existing `GitHub push skips a Nostr pull-request update` test remains green.

- [ ] **Step 7: Commit Task 4.**

```bash
. ./bin/activate-hermit
git add desktop/src/features/projects/pullRequestMutations.ts desktop/src/features/projects/pullRequestMutations.test.mjs desktop/src/features/projects/pullRequestReviews.ts desktop/src/features/projects/hooks.ts
```

Run the staged GitNexus scope check when available.
If it is unavailable, run `git diff --cached --stat`, `git diff --cached --name-only`, and `git diff --cached --check` and confirm only the Task 4 files are staged.

```bash
. ./bin/activate-hermit
git commit -s -m "feat(desktop): route GitHub pull request creation"
```

---

### Task 5: Add GitHub-only list and detail UI without pubkey or write chrome

**Files:**

- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabList.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx`
- Create: `desktop/src/features/projects/ui/GitHubProjectPullRequests.tsx`
- Create: `desktop/tests/e2e/github-pull-requests.spec.ts`

**Interfaces:**

- Consumes: `ProjectPullRequestsResult`, Task 3 helpers/query, `GitHubLoginIdentity`, `ProjectIssueCommentTimeline` in GitHub mode, `GitHubRepoStateRecovery`, `ProjectRichContent`, and the shared tabs/list-row components.
- Produces: `GitHubPullRequestRow`, `GitHubPullRequestsPanel`, and `GitHubPullRequestDetail` with no Nostr identity or write descendants.

- [ ] **Step 1: Run the Task 5 impact checks.**

Report direct callers, affected processes, and risk before editing.
Stop and warn if a result is HIGH or CRITICAL.

- [ ] **Step 2: Write the failing happy-path Playwright acceptance first.**

Create the spec with `enableProjectsFeature`, `openBuzzProject`, and a clone override matching the existing GitHub Issues spec.
The spec must assert the complete P1 + P2 happy path before the mock commands or UI exist.

```typescript
import { expect, test } from "@playwright/test";

import { installMockBridge } from "../helpers/bridge";

async function enableProjectsFeature(page: import("@playwright/test").Page) {
  await page.addInitScript(() => {
    window.localStorage.setItem(
      "buzz-feature-overrides-v1",
      JSON.stringify({ projects: true }),
    );
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
  });
}

async function openBuzzProject(page: import("@playwright/test").Page) {
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await page.getByTestId("open-projects-view").click();
  await page.getByTestId("projects-section-projects").click();
  const entry = page
    .locator('[data-testid="project-card-buzz"], [data-testid="project-row-buzz"]')
    .first();
  await expect(entry).toBeVisible({ timeout: 10_000 });
  await entry.click();
}

async function openGithubPullRequests(page: import("@playwright/test").Page) {
  await enableProjectsFeature(page);
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Pull Request", exact: true }).click();
}

test("GitHub pull requests list, open read-only detail, and create #N", async ({ page }) => {
  await openGithubPullRequests(page);
  const row = page.getByTestId("project-github-pull-request-row").first();
  await expect(row).toContainText("#42");
  await expect(row).toContainText("Open");
  await expect(row).toContainText("ada");
  await row.getByRole("button", { name: "#42", exact: true }).click();

  await expect(page.getByText("GitHub pull request body", { exact: true })).toBeVisible();
  await expect(page.getByText("API-order first PR comment.", { exact: true })).toBeVisible();
  await expect(page.getByText("API-order second PR comment.", { exact: true })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Conversation/ })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Commits/ })).toContainText("1");
  await expect(page.getByRole("tab", { name: /Checks/ })).toBeVisible();
  await expect(page.getByRole("tab", { name: /Files changed/ })).toHaveCount(0);
  await expect(page.getByRole("button", { name: /Merge/ })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Approve", exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Request changes", exact: true })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Request review", exact: true })).toHaveCount(0);
  await expect(page.getByText("Reviewers", { exact: true })).toHaveCount(0);
  await expect(page.getByTestId("project-pull-request-comment-composer")).toHaveCount(0);
  await expect(page.getByTestId("pull-request-discussed-in")).toHaveCount(0);

  await page.getByRole("tab", { name: /Commits/ }).click();
  await expect(page.getByTestId("project-github-pull-request-commit-row")).toHaveCount(1);
  await expect(page.getByTestId("project-github-pull-request-commit-row")).toContainText("1111111");
  await page.getByRole("tab", { name: /Checks/ }).click();
  await expect(page.getByText("No checks have been reported for this pull request yet.", { exact: true })).toBeVisible();

  await page
    .getByRole("navigation", { name: "Project breadcrumb" })
    .getByRole("button", { name: "Pull Request", exact: true })
    .click();
  await page.getByRole("button", { name: "New pull request" }).click();
  await page.getByTestId("create-pull-request-title").fill("New GitHub PR");
  await page.getByTestId("create-pull-request-body").fill("Created from Buzz");
  await page.getByTestId("create-pull-request-base-branch").selectOption("develop");
  await page.getByTestId("create-pull-request-compare-branch").selectOption("main");
  await page.getByTestId("create-pull-request-submit").click();
  await expect(page.getByText("New GitHub PR", { exact: true })).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText("#43", { exact: true })).toBeVisible();

  const commands = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(commands).toContain("list_github_pull_requests");
  expect(commands).toContain("list_github_pull_request_comments");
  expect(commands).toContain("create_github_pull_request");
  const signed = await page.evaluate(() => window.__BUZZ_E2E_SIGNED_EVENTS__ ?? []);
  expect(signed.some((event) => event.kind === 1618 || event.kind === 1613)).toBe(false);
});
```

- [ ] **Step 3: Run the E2E spec and verify RED.**

Temporarily run the spec directly before adding it to Playwright `testMatch`.

Run: `. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test tests/e2e/github-pull-requests.spec.ts --project=smoke`

Expected: FAIL because the mock bridge does not handle `list_github_pull_requests` and no GitHub PR row renders.

- [ ] **Step 4: Isolate GitHub identities and disable Nostr-only diff/update behavior in `ProjectDetailScreen`.**

Use the reference-stable `pullRequests` array introduced in Task 3.
Use `pullRequestIdentityPubkeys(pullRequests)` instead of manually flattening every PR identity into `useUsersBatchQuery`.
Keep project and issue pubkeys in the same deduplicated array.

Disable both remote and local PR diff queries when `githubHosted` is true so `refs/nostr/42` is never requested.
Pass `undefined` for `updatePullRequestAction` when `githubHosted` is true.
Pass `pullRequestsHasMore={pullRequestsQuery.data?.hasMore ?? false}`, `pullRequestsFetching={pullRequestsQuery.isFetching}`, `pullRequestsSuccess={pullRequestsQuery.isSuccess}`, and `onRetryPullRequests={() => void pullRequestsQuery.refetch()}` to `WorkspaceTabs`.

- [ ] **Step 5: Make the shared PR tabs host-aware.**

Add a required boolean `githubHosted` prop at the only call site and use it to hide Files changed.
Use the shared model count and exact GitHub commit count.

```tsx
const conversationCount = Math.max(
  pullRequest.commentCount,
  pullRequest.comments.length,
);
const commitCount = githubHosted ? 1 : Math.max(1, pullRequest.updateCount + 1);
```

Render Conversation, Commits, and Checks for both hosts.
Render the Files changed trigger only when `githubHosted` is false.

- [ ] **Step 6: Implement the GitHub-only PR component.**

The file must not import any profile/pubkey component, `ForumComposer`, `PullRequestReviewCard`, `PullRequestReviewersRow`, `MergePullRequestButton`, `DiscussedInChannels`, or `ProjectOriginReference`.
Use `GitHubLoginIdentity` for every author.

Export these exact props.

```typescript
export function GitHubPullRequestsPanel(props: {
  error: unknown;
  hasMore: boolean;
  isLoading: boolean;
  onRetry: () => void;
  onSelectedPullRequestIdChange: (id: string | null) => void;
  pullRequests: ProjectPullRequest[];
}): React.ReactElement;

export function GitHubPullRequestDetail(props: {
  onOpenCommit?: (commitHash: string) => void;
  onSelectedPullRequestIdChange: (id: string | null) => void;
  project: Repository;
  pullRequest: ProjectPullRequest;
}): React.ReactElement;
```

`GitHubPullRequestsPanel` must render in this order.

1. `Loading pull requests…` while pending.
2. `GitHubRepoStateRecovery` when `error` is truthy, before inspecting list length.
3. `No open pull requests.` for an empty successful page.
4. A divided list of `GitHubPullRequestRow` components.
5. The muted `More open pull requests exist on GitHub.` note when `hasMore` is true.

Each row must use `pullRequest.id` as its full numeric display, `commentCount`, `GitHubLoginIdentity`, `relativeTime(pullRequest.updatedAt)`, status styling for Open/Draft, and `githubPullRequestBranchLabel`.
Use test id `project-github-pull-request-row` and an accessible number button named exactly `#${pullRequest.id}`.

`GitHubPullRequestDetail` must call `useGithubPullRequestCommentsQuery(project, pullRequest.id)` and derive a local hydrated object.

```typescript
const EMPTY_GITHUB_PULL_REQUEST_COMMENTS: ProjectPullRequestComment[] = [];

const comments =
  commentsQuery.data ?? EMPTY_GITHUB_PULL_REQUEST_COMMENTS;
const hydratedPullRequest = {
  ...pullRequest,
  comments,
  commentCount: Math.max(pullRequest.commentCount, comments.length),
};
```

Declare the empty comments array at module scope so loading-state renders do not allocate a new reference.

Parse structured comment errors with `parseProjectPullRequestMergeError`.
When the code is `github_pr_unavailable`, call `toast.error("Pull request not found.")` once and clear selection in an effect.
Use a ref keyed by `pullRequest.id` so React Strict Mode cannot emit the toast twice.
For every other error, keep the header/body and render `GitHubRepoStateRecovery` with a retry callback that calls only `commentsQuery.refetch()`.

Render the detail as a two-column grid matching the existing PR shell.
The left column owns title plus full `#N`, safe share link, login/avatar metadata, tabs, and tab content.
The right rail owns Status, Author, Branches, and Activity only.
Activity displays created and updated relative times from `createdAt` and `updatedAt`.

The Conversation content must render `ProjectRichContent` with `tags={[]}`, `Loading comments…`, and `ProjectIssueCommentTimeline` with `githubMode` after success.
The Commits content must render one row with `head.sha`, login/avatar, title, and branch.
The Checks content must use the exact existing placeholder.

- [ ] **Step 7: Branch `WorkspaceTabs` once by repository host.**

Add `pullRequestsHasMore`, `pullRequestsFetching`, `pullRequestsSuccess`, and `onRetryPullRequests` props.
Use the Task 3 settled-selection helper only for GitHub so a created ID is preserved through refetch and clears only if the settled first page omits it.

```typescript
React.useEffect(() => {
  if (!githubHosted) return;
  const nextSelectedPullRequestId = selectedGithubPullRequestAfterListLoad({
    selectedPullRequestId,
    pullRequestIds: pullRequests.map((item) => item.id),
    isSuccess: pullRequestsSuccess,
    isFetching: pullRequestsFetching,
  });
  if (nextSelectedPullRequestId !== selectedPullRequestId) {
    onSelectedPullRequestIdChange(nextSelectedPullRequestId);
  }
}, [
  githubHosted,
  onSelectedPullRequestIdChange,
  pullRequests,
  pullRequestsFetching,
  pullRequestsSuccess,
  selectedPullRequestId,
]);
```

Add an effect that applies `githubPullRequestDetailTab(selectedTab, githubHosted)` and updates state only when the helper returns a different tab.

When a PR is selected, render `GitHubPullRequestDetail` for GitHub and preserve the current Buzz header/tabs/panels/meta-rail tree otherwise.
Do not mount `ProjectPullRequestFilesChangedPanel` in the GitHub branch.

Inside the top-level `value="prs"` content, render `GitHubPullRequestsPanel` for GitHub and the existing `PullRequestsPanel` for Buzz.
Keep the current Buzz props unchanged.

Set `commitAuthorPubkeys` to a module-level empty map for GitHub before calling commit-detail components so a login is never treated as a signed Nostr commit publisher and the empty reference stays stable.
Also pass a module-level empty PR array to `ActivityPanel` on GitHub because that component independently builds the same signed-event commit-author map from its `pullRequests` prop.

```typescript
const EMPTY_COMMIT_AUTHOR_PUBKEYS = new Map<string, string>();
const EMPTY_SIGNED_PULL_REQUESTS: ProjectPullRequest[] = [];

const commitAuthorPubkeys = React.useMemo(
  () =>
    githubHosted
      ? EMPTY_COMMIT_AUTHOR_PUBKEYS
      : commitAuthorPubkeysFromPullRequests(pullRequests),
  [githubHosted, pullRequests],
);

const activityPullRequests = githubHosted
  ? EMPTY_SIGNED_PULL_REQUESTS
  : pullRequests;
```

Pass `activityPullRequests` only to `ActivityPanel` and keep the full list for the GitHub PR tab and the repository overview count.

- [ ] **Step 8: Add the minimal mock bridge PR store and commands needed for the happy path.**

Task 6 adds error controls, but this task must add the DTOs, default store, clone helpers, store initialization, and three successful command cases so the happy-path test can turn green.

Use default PR `#42`, title `Add GitHub pull request support`, body `GitHub pull request body`, author `ada`, head `feature/readme` at `"1".repeat(40)`, base `develop`, and two comments with bodies from the test.
Create must append the next number, copy `title`, `body`, `head`, and `base` from the command payload, use the matching mocked repository-state branch SHA when available, and return the cloned DTO.

Add the store next to the existing GitHub issue store with these exact shapes.

```typescript
type E2eGithubPullRequestDto = {
  number: number;
  title: string;
  body: string;
  html_url: string;
  draft: boolean;
  created_at: number;
  updated_at: number;
  comments: number;
  user: E2eGithubIssueUser;
  head: {
    ref: string;
    sha: string;
    repo: { full_name: string };
  };
  base: {
    ref: string;
    repo: { full_name: string };
  };
};

type E2eGithubPullRequestCommentDto = {
  id: number;
  body: string;
  html_url: string;
  created_at: number;
  user: E2eGithubIssueUser;
};

type E2eGithubPullRequestStore = {
  pulls: E2eGithubPullRequestDto[];
  commentsByNumber: Record<number, E2eGithubPullRequestCommentDto[]>;
};

function createDefaultE2eGithubPullRequestStore(): E2eGithubPullRequestStore {
  return {
    pulls: [
      {
        number: 42,
        title: "Add GitHub pull request support",
        body: "GitHub pull request body",
        html_url: "https://github.com/acme/app/pull/42",
        draft: false,
        created_at: 1_704_166_645,
        updated_at: 1_704_253_045,
        comments: 2,
        user: { login: "ada", avatar_url: "" },
        head: {
          ref: "feature/readme",
          sha: "1".repeat(40),
          repo: { full_name: "acme/app" },
        },
        base: {
          ref: "develop",
          repo: { full_name: "acme/app" },
        },
      },
    ],
    commentsByNumber: {
      42: [
        {
          id: 2,
          body: "API-order first PR comment.",
          html_url: "https://github.com/acme/app/pull/42#issuecomment-2",
          created_at: 1_704_253_100,
          user: { login: "grace", avatar_url: "" },
        },
        {
          id: 10,
          body: "API-order second PR comment.",
          html_url: "https://github.com/acme/app/issues/42#issuecomment-10",
          created_at: 1_704_253_101,
          user: { login: "linus", avatar_url: "" },
        },
      ],
    },
  };
}
```

Initialize the store in `maybeInstallE2eTauriMocks` beside `__BUZZ_E2E_GITHUB_ISSUE_STORE__` so every page load starts from the deterministic defaults.
Clone nested `user`, `head.repo`, and `base.repo` objects on every command response so a UI test cannot mutate the backing store.

Implement these switch contracts.

```typescript
case "list_github_pull_requests": {
  return {
    pulls: e2eGithubPullRequestStore().pulls.map(cloneE2eGithubPullRequest),
    has_more: false,
  };
}
case "create_github_pull_request": {
  const input = payload as {
    title?: string;
    body?: string;
    head?: string;
    base?: string;
  };
  if (!input.title || !input.head || !input.base) {
    throw new Error("Missing GitHub pull request create field.");
  }
  const store = e2eGithubPullRequestStore();
  const number = Math.max(0, ...store.pulls.map((pull) => pull.number)) + 1;
  const branchCommits: Record<string, string> = {
    develop: "d".repeat(40),
    main: "m".repeat(40),
  };
  const created: E2eGithubPullRequestDto = {
    number,
    title: input.title,
    body: input.body ?? "",
    html_url: `https://github.com/acme/app/pull/${number}`,
    draft: false,
    created_at: 1_704_253_100,
    updated_at: 1_704_253_100,
    comments: 0,
    user: { login: "ada", avatar_url: "" },
    head: {
      ref: input.head,
      sha: branchCommits[input.head] ?? "1".repeat(40),
      repo: { full_name: "acme/app" },
    },
    base: {
      ref: input.base,
      repo: { full_name: "acme/app" },
    },
  };
  store.pulls.push(created);
  store.commentsByNumber[number] = [];
  return cloneE2eGithubPullRequest(created);
}
case "list_github_pull_request_comments": {
  const input = payload as { number?: number };
  return (
    e2eGithubPullRequestStore().commentsByNumber[input.number ?? 0] ?? []
  ).map(cloneE2eGithubPullRequestComment);
}
```

The create branch must throw if `title`, `head`, or `base` is missing, which makes a frontend payload regression fail the E2E test instead of silently creating fallback data.

- [ ] **Step 9: Run the happy-path E2E, unit suite, typecheck, and file-size gate.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test tests/e2e/github-pull-requests.spec.ts --project=smoke`

Expected: PASS.

Run: `. ./bin/activate-hermit && cd desktop && pnpm test`

Expected: PASS.

Run: `. ./bin/activate-hermit && cd desktop && pnpm typecheck`

Expected: PASS.

Run: `. ./bin/activate-hermit && cd desktop && pnpm check:file-sizes`

Expected: PASS, including `ProjectDetailScreen.tsx`, `hooks.ts`, and `ProjectPullRequestsPanel.tsx` below 1,000 lines.

- [ ] **Step 10: Commit Task 5.**

```bash
. ./bin/activate-hermit
git add desktop/src/features/projects/ui/ProjectDetailScreen.tsx desktop/src/features/projects/ui/ProjectWorkspaceTabList.tsx desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx desktop/src/features/projects/ui/GitHubProjectPullRequests.tsx desktop/src/testing/e2eBridge.ts desktop/tests/e2e/github-pull-requests.spec.ts
```

Run the staged GitNexus scope check when available.
If it is unavailable, run `git diff --cached --stat`, `git diff --cached --name-only`, and `git diff --cached --check` and confirm only the Task 5 files are staged.

```bash
. ./bin/activate-hermit
git commit -s -m "feat(desktop): render GitHub pull requests"
```

---

### Task 6: Add recovery E2E coverage and register the smoke spec

**Files:**

- Modify: `desktop/src/testing/e2eBridge.ts`
- Modify: `desktop/tests/e2e/github-pull-requests.spec.ts`
- Modify: `desktop/playwright.config.ts:120-140`

**Interfaces:**

- Consumes: Task 5's component and mock store.
- Produces: deterministic list/auth/comment/404 failures and smoke-project registration.

- [ ] **Step 1: Run the Task 6 impact checks.**

Report direct callers, affected processes, and risk before editing.
Stop and warn if a result is HIGH or CRITICAL.

- [ ] **Step 2: Write failing recovery E2E tests.**

Add exact tests for list auth, retryable comments, and comment 404.

```typescript
test("GitHub pull request auth failure renders recovery before empty state", async ({ page }) => {
  await page.addInitScript(() => {
    window.__BUZZ_E2E_GITHUB_PULLS_ERROR__ = {
      code: "github_auth_required",
      message: "Authenticate GitHub CLI with: gh auth login --hostname github.com",
    };
  });
  await openGithubPullRequests(page);
  await expect(page.getByText("GitHub authentication required")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText("No pull requests yet.")).toHaveCount(0);
  await expect(page.getByText("No open pull requests.")).toHaveCount(0);
});

test("GitHub comment failure keeps the PR body and retries only comments", async ({ page }) => {
  await page.addInitScript(() => {
    window.__BUZZ_E2E_GITHUB_PULL_COMMENTS_ERROR__ = {
      code: "github_pulls_failed",
      message: "Comment request failed.",
    };
  });
  await openGithubPullRequests(page);
  await page.getByTestId("project-github-pull-request-row").first().getByRole("button", { name: "#42" }).click();
  await expect(page.getByText("GitHub pull request body", { exact: true })).toBeVisible();
  await expect(page.getByText("Could not load GitHub pull request comments", { exact: true })).toBeVisible();
  const before = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  const listBefore = before.filter((command) => command === "list_github_pull_requests").length;
  const commentsBefore = before.filter((command) => command === "list_github_pull_request_comments").length;
  await page.locator('[aria-labelledby="github-pull-comments-recovery-title"]').getByRole("button", { name: "Retry" }).click();
  const after = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(after.filter((command) => command === "list_github_pull_requests").length).toBe(listBefore);
  expect(after.filter((command) => command === "list_github_pull_request_comments").length).toBe(commentsBefore + 1);
});

test("GitHub pull comment 404 reports not found and returns to the list", async ({ page }) => {
  await page.addInitScript(() => {
    window.__BUZZ_E2E_GITHUB_PULL_COMMENTS_ERROR__ = {
      code: "github_pr_unavailable",
      message: "Not Found",
    };
  });
  await openGithubPullRequests(page);
  await page.getByTestId("project-github-pull-request-row").first().getByRole("button", { name: "#42" }).click();
  await expect(page.getByText("Pull request not found.", { exact: true })).toBeVisible();
  await expect(page.getByTestId("project-github-pull-request-row").first()).toBeVisible();
});
```

- [ ] **Step 3: Run the recovery tests and verify RED.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test tests/e2e/github-pull-requests.spec.ts --project=smoke`

Expected: FAIL because the new mock error globals are not declared or thrown.

- [ ] **Step 4: Add structured mock errors.**

Declare these exact `Window` fields.

```typescript
__BUZZ_E2E_GITHUB_PULLS_ERROR__?: { code: string; message: string };
__BUZZ_E2E_GITHUB_PULL_COMMENTS_ERROR__?: { code: string; message: string };
```

Keep the `__BUZZ_E2E_GITHUB_PULL_STORE__?: E2eGithubPullRequestStore` declaration added in Task 5 unchanged.

Throw `__BUZZ_E2E_GITHUB_PULLS_ERROR__` from list and create.
Throw `__BUZZ_E2E_GITHUB_PULL_COMMENTS_ERROR__` only from the comment command.
Do not make a comment error affect the list or create command.

- [ ] **Step 5: Register the spec and run E2E with a fresh E2E build.**

Add `"**/github-pull-requests.spec.ts"` beside the other GitHub specs in the smoke `testMatch` array.

Run: `. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test tests/e2e/github-pull-requests.spec.ts --project=smoke`

Expected: PASS for happy path, auth-first recovery, comment-only retry, and 404 selection clearing.

- [ ] **Step 6: Run the existing Buzz-hosted PR E2E regression.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test tests/e2e/project-pr-review.spec.ts --project=smoke`

Expected: PASS with Merge, reviewers, review writes, composer, and Files changed still present on Buzz-hosted pull requests.

- [ ] **Step 7: Commit Task 6.**

```bash
. ./bin/activate-hermit
git add desktop/src/testing/e2eBridge.ts desktop/tests/e2e/github-pull-requests.spec.ts desktop/playwright.config.ts
```

Run the staged GitNexus scope check when available.
If it is unavailable, run `git diff --cached --stat`, `git diff --cached --name-only`, and `git diff --cached --check` and confirm only the Task 6 files are staged.

```bash
. ./bin/activate-hermit
git commit -s -m "test(desktop): cover GitHub pull request recovery"
```

---

### Task 7: Run final quality gates and verify the product contract

**Files:**

- Verify only: every file in the File Map.
- Do not add implementation during this task unless a failing gate first receives a focused regression test.

**Interfaces:**

- Consumes: all prior task outputs.
- Produces: final automated and diff evidence for handoff.

- [ ] **Step 1: Run the full focused native module tests.**

Run: `. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml project_github_pulls -- --nocapture`

Expected: PASS.

- [ ] **Step 2: Run all desktop frontend unit tests.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm test`

Expected: PASS.

- [ ] **Step 3: Run desktop static, text-size, and file-size checks.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm check && pnpm typecheck && pnpm check:file-sizes`

Expected: PASS with no arbitrary text-size violations and no edited file above the ratchet.

- [ ] **Step 4: Run Tauri formatting, compile, Clippy, and unit tests.**

Run: `. ./bin/activate-hermit && cargo fmt --manifest-path desktop/src-tauri/Cargo.toml --all -- --check`

Expected: PASS.

Run: `. ./bin/activate-hermit && just desktop-tauri-check`

Expected: PASS.

Run: `. ./bin/activate-hermit && just desktop-tauri-clippy`

Expected: PASS with warnings denied.

Run: `. ./bin/activate-hermit && just desktop-tauri-test`

Expected: PASS.

- [ ] **Step 5: Run the two PR E2E specs from a fresh E2E build.**

Run: `. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test tests/e2e/github-pull-requests.spec.ts tests/e2e/project-pr-review.spec.ts --project=smoke`

Expected: PASS.

- [ ] **Step 6: Run the repository-wide CI gate.**

Run: `. ./bin/activate-hermit && just ci`

Expected: PASS.

- [ ] **Step 7: Verify scope and whitespace.**

Run GitNexus `detect_changes({ scope: "compare", base_ref: "main" })` when available and confirm only the GitHub PR flows and explicitly listed shared consumers changed.

If GitNexus remains unavailable, run all fallback evidence.

```bash
. ./bin/activate-hermit
git diff --stat main...
git diff --name-only main...
git diff --check main...
```

Expected: only File Map implementation/test files are changed and `git diff --check` prints nothing.

- [ ] **Step 8: Perform the final manual source audit.**

Search the GitHub component and routing path for forbidden identity/write imports.

```bash
. ./bin/activate-hermit
rg -n "ProfileIdentityButton|ProfileAuthorName|normalizePubkey|ForumComposer|PullRequestReviewCard|PullRequestReviewersRow|MergePullRequestButton|DiscussedInChannels|ProjectOriginReference|signRelayEvent|publishEvent" desktop/src/features/projects/ui/GitHubProjectPullRequests.tsx desktop/src/features/projects/lib/projectGithubPullRequests.ts desktop/src/features/projects/pullRequestMutations.ts
```

Expected: no match in the GitHub UI or mapper.
Matches in `pullRequestMutations.ts` must be confined to the guarded Buzz backend.

Search for command policy.

```bash
. ./bin/activate-hermit
rg -n "gh pr (list|create)|--paginate|head_repo|\"draft\"" desktop/src-tauri/src/commands/project_github_pulls.rs
```

Expected: no matches.

## Acceptance Criteria

- A plain `https://github.com/<owner>/<repo>[.git]` or `git@github.com:<owner>/<repo>[.git]` repository uses only the three new GitHub PR commands on its per-repository Pull Request tab.
- A Buzz-hosted repository uses only the existing kind `1618`, kind `1613`, kind `1`, and status-event path.
- The GitHub list requests exactly one open page of 100 sorted by updated descending and includes inbound forks.
- Raw pages of 100 set `hasMore` before malformed rows are dropped.
- Rows display full `#N`, Open or Draft, login/avatar, correct same-repo or fork-prefixed branches, comment count, and updated time.
- Opening `#N` shows title, body, safe GitHub URL share, author, branches, one head commit, the checks placeholder, and first-page issue comments in GitHub order.
- GitHub detail never mounts Merge, reviewers, review writes, comment composer, channel/origin chrome, or Files changed.
- A stale `pr-files` selection on GitHub resolves to `pr-conversation` without mounting a diff panel.
- GitHub PR selection never builds `refs/nostr/{number}` or requests a PR diff.
- Creating sends only title, body, head, and base to GitHub, returns the created number, refetches the list, and never signs kind `1618`.
- A created PR missing from the one open page clears selection and stays on the PR list.
- GitHub create invalidates only the repository PR query.
- Buzz create retains all existing invalidations and behavior.
- A GitHub push or direct update call never publishes kind `1613`.
- Any leftover Nostr status, review, comment, update, or merge call rejects `"42"` before signing or invoking Tauri.
- Missing `gh` shows install guidance and Retry.
- Missing GitHub auth shows the login command, Copy, and Retry before any empty state.
- Repository 404/non-rate 403 shows GitHub recovery and never substitutes an empty list.
- Comment 404 reports `Pull request not found.`, clears selection, and preserves the loaded list.
- Other comment failures preserve the PR body and retry only comments.
- Malformed or foreign PR/comment URLs are never exposed or copied.
- Existing Buzz-hosted PR review and merge E2E coverage stays green.
- Focused tests, desktop tests, E2E tests, static checks, Tauri checks, file-size checks, and `just ci` all pass.
