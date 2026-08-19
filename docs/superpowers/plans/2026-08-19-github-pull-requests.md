# GitHub Pull Requests (list + create) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a Projects repository uses a plain `github.com` clone URL, its per-repository Pull Request tab lists open GitHub pull requests, creates GitHub pull requests, loads read-only conversation comments, and never reads or publishes NIP-34 `kind:1618` for that repository.

**Architecture:** Add three Tauri commands in a new `project_github_pulls.rs` module backed by the existing bounded `GhRunner` and `GitHubRepoRef` abstractions.
Route the existing pull-request query and create mutation by the selected repository's first clone URL, map bounded GitHub DTOs into `ProjectPullRequest`, and keep the Buzz-hosted kind:1618 path unchanged.
Put GitHub-only list and detail chrome in a focused component so GitHub logins never reach pubkey normalization, profile lookup, merge, review writes, or Files changed.

**Tech Stack:** Rust, Tauri 2, `gh api`, React 19, TanStack Query, Node `node:test`, and Playwright with the E2E mock bridge.

**Spec:** [2026-08-18-github-pull-requests-design.md](../specs/2026-08-18-github-pull-requests-design.md)

**Product contract:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) make GitHub the native git, issue, and pull-request backend for GitHub-hosted repositories while Buzz remains the collaboration layer.
This slice advances that host split.
It does not yet create branch channels or reuse `merge_github_pull_request` for listed `#N`; those belong to later P3/P4 work.

**Phase doc:** [phase-03-github-pull-requests.md](../../../plans/20260818-1211-github-native-host/phase-03-github-pull-requests.md), slice P1 + P2 only.

## Global Constraints

- Implement P1 + P2 only: list open pull requests, create a ready same-repo pull request with title, body, head, and base, and load existing issue comments read-only when detail opens.
- Do not implement review or required-check UI (P3), merge-by-number (P4 adapt), review comments or posting comments (P5), close, reopen, convert to draft, or mark ready.
- Do not change `merge_github_pull_request` or add list/create into `project_github_pull_request.rs`.
- Do not change the global Projects PR list, project card or activity counts, CLI, mobile, or the `buzz://pr` scheme.
- Do not union GitHub pull requests with kind `1618`, import GitHub PRs into Nostr, or dual-write.
- Do not call `gh pr`, add a provider trait, or store a GitHub token.
- Authenticate only through installed `gh` and `gh auth status --hostname github.com`.
- Use `gh api` with one page, `per_page=100`, `state=open`, `sort=updated`, `direction=desc`, and no `--paginate`.
- Include inbound fork PRs and drafts on that open page.
- Compute `has_more` from the projected raw page length before dropping invalid URLs.
- Accept only positive `u64` pull-request numbers in Rust and positive safe decimal integers in TypeScript.
- Treat GitHub authors as logins plus avatar URLs, never as Nostr pubkeys.
- Never pass a GitHub login to `ProfileIdentityButton`, `ProfileAuthorName`, `normalizePubkey`, `useUsersBatchQuery`, `MergePullRequestButton`, `PullRequestReviewCard`, `PullRequestReviewersRow`, `ForumComposer`, or Nostr review/merge mutations.
- Map GitHub `draft: true` to `"Draft"` and otherwise `"Open"`.
- Never map GitHub open or draft rows to `"Merged"` or `"Closed"` in this slice.
- Copy only a validated GitHub pull-request URL, and keep the existing hex-event `buzz://pr` fallback for Buzz pull requests.
- Create with exactly `{ "title", "body", "head", "base" }` through a tempfile passed to `gh api --input`.
- Trim the title, reject an empty title, and reject more than 256 Unicode scalar values before running `gh`.
- Trim `head` and `base`, reject either when empty, and reject `head == base` after trim before running `gh`.
- Preserve the body supplied to the native command, including an empty body.
- Create is same-repo only: `head` is a branch name, never `owner:branch` and never `head_repo`.
- Reject a create `head` containing `:` before running `gh`; Git ref names cannot contain `:`, and this prevents the GitHub `owner:branch` cross-repository form at the native boundary.
- Do not send reviewers or `draft` on create.
- On GitHub create success, invalidate only `["project", project.id, "pull-requests"]`.
- On Buzz create success, preserve the current pull-requests, work-items, and activity-summaries invalidations.
- Keep the list query key `["project", project.id, "pull-requests"]` in this slice.
- Change `useProjectPullRequestsQuery` data to `{ pullRequests, hasMore }` and update every caller that treated `data` as an array.
- Conversation comments use `GET /repos/{owner}/{repo}/issues/{number}/comments?per_page=100`.
- Do not call `GET /repos/{owner}/{repo}/pulls/{number}/comments`.
- Use only `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, `github_pr_unavailable`, and `github_pulls_failed` from the new native module.
- Do not leak `github_merge_failed`, `github_state_failed`, or `github_issues_failed` from any new command.
- Comment-load HTTP 404 is `github_pr_unavailable` and clears selection while keeping the list.
- Other comment-load failures keep the PR body and retry only the comment query.
- Check GitHub errors before empty-success rendering.
- GitHub empty-success copy is `No open pull requests.` and the existing Buzz empty/error copy stays unchanged.
- Hide Merge, review writes, reviewers, Request changes, the composer, Discussed in channels, origin reference, and Files changed on GitHub rows.
- Hide the Nostr-only Update PR action and disable remote and local pull-request diff queries on GitHub rows.
- If `selectedTab === "pr-files"` on a GitHub-hosted repo, snap to `pr-conversation`.
- Commits tab shows one `head.sha` row and a badge of `1`.
- Checks tab keeps `No checks have been reported for this pull request yet.`
- `desktop/src/features/projects/ui/ProjectPullRequestsPanel.tsx` is 993 lines.
- Do not add GitHub chrome to that file; put it in `GitHubProjectPullRequests.tsx`.
- `desktop/src/features/projects/hooks.ts` is 947 lines and `ProjectDetailScreen.tsx` is 962 lines.
- Keep both under the 1,000-line ratchet by extracting routing into `projectGithubPulls.ts` and replacing `pullRequestsQuery.data` with a local `pullRequests` array.
- `shouldPublishPullRequestUpdateAfterPush` already returns false for GitHub clone URLs; keep that behavior and do not publish kind:1613 against `"42"`.
- Do not run live GitHub requests in automated tests.
- Do not add production `unsafe`, `unwrap()`, or `expect()` calls.
- Add doc comments to every new public Rust or TypeScript API.
- Use named rem-based text tokens and do not add arbitrary text sizes.
- Activate Hermit in every shell command with `. ./bin/activate-hermit && ...` or as the first line of the same shell block.
- Run GitNexus `impact({ target, direction: "upstream" })` before editing each existing symbol when the MCP tools are available.
- Warn before proceeding if GitNexus reports HIGH or CRITICAL risk.
- Run GitNexus `detect_changes({ scope: "staged" })` before every commit and `detect_changes({ scope: "compare", base_ref: "main" })` before final handoff when the MCP tools are available.
- If GitNexus tools are unavailable, record that fact in the implementation handoff and use `git diff --stat`, `git diff --name-only`, and `git diff --check` as the fallback scope evidence.
- Sign every commit with `git commit -s`.

## Resolved Implementation Decisions

- Use `GH_PULL_STREAM_LIMIT = 32 * 1024 * 1024` for list, create, and comment JSON.
- The 32 MiB cap accommodates one page of 100 large UTF-8 bodies plus projected metadata while remaining bounded.
- A response that exceeds the cap becomes invalid or truncated JSON and returns `github_pulls_failed`; never substitute an empty list.
- A created pull request whose `html_url` fails repository-bound validation returns `github_pulls_failed`.
- List recovery title is `Could not load GitHub pull requests`.
- Comment recovery title is `Could not load GitHub comments` and retries only the comment query.
- Command registration lives in `desktop/src-tauri/src/invoke.rs`, not `lib.rs`.
- `GitHubRepoRef.owner` and `.repo` are already `pub(crate)`; do not change `project_github_pull_request.rs` in this slice.
- Do not reuse `list_github_issue_comments` or `github_api_json` from `project_github_issues.rs`.
- Those helpers remap to issue codes and accept only `/issues/{n}#issuecomment-{id}` URLs.
- This module must accept both `/issues/{n}#issuecomment-{id}` and `/pull/{n}#issuecomment-{id}` and map comment 404 to `github_pr_unavailable`.
- Flattened jq fields are remapped into a nested public DTO: `head.ref`, `head.sha`, `head.repo.full_name`, `base.ref`, `base.repo.full_name`.
- When `head.repo.full_name` is empty (deleted fork), treat the head as same-repo for the branch label and do not render `:branch`.
- Missing `draft` in the projection is `false`.
- Empty `head.ref`, `base.ref`, or `head.sha` drops a list item; the same failure on create is `github_pulls_failed`.
- `head == base` comparison is case-sensitive after trim.
- The create dialog already trims title and body; the native command still trims title and preserves the body it receives.
- Register the three commands immediately after `list_github_issue_comments` in `invoke.rs`.
- A deleted-fork PR with empty `head.repo.full_name` shows `head → base`; only prefix `owner:` when the full name is non-empty and differs from the target repo case-insensitively.

## File Map

| File | Responsibility |
|------|----------------|
| Create `desktop/src-tauri/src/commands/project_github_pulls.rs` | Bounded list/create/comments commands, DTO mapping, URL validation, and pull-specific error remapping |
| `desktop/src-tauri/src/commands/mod.rs` | Declare and re-export the new command module |
| `desktop/src-tauri/src/invoke.rs` | Register the three Tauri commands |
| `desktop/src/shared/api/projectGit.ts` | Native DTO types and three Tauri invoke wrappers |
| `desktop/src/features/projects/projectPullRequests.mjs` | Populate neutral GitHub-extension fields on Nostr pull requests |
| `desktop/src/features/projects/projectPullRequests.d.mts` | Keep `ProjectPullRequest` and comment declarations aligned with the runtime object |
| `desktop/src/features/projects/projectPullRequests.test.mjs` | Protect the unchanged Nostr mapping defaults |
| Create `desktop/src/features/projects/lib/projectGithubPulls.ts` | Host routing, DTO mapping, display helpers, identity filtering, and comment query |
| Create `desktop/src/features/projects/lib/projectGithubPulls.test.mjs` | Protect routing, mapping, identity, number, branch-label, and comment-query contracts |
| `desktop/src/features/projects/lib/projectShareLinks.ts` | Prefer a strictly validated GitHub pull URL before the existing Buzz fallback |
| `desktop/src/features/projects/lib/projectShareLinks.test.mjs` | Protect GitHub URL validation and the existing Buzz deep link |
| `desktop/src/features/projects/hooks.ts` | Route `useProjectPullRequestsQuery` and retain the current Nostr loader |
| `desktop/src/features/projects/pullRequestMutations.ts` | Route create, select the correct invalidation set, and refuse non-hex ids on Nostr writes |
| `desktop/src/features/projects/pullRequestReviews.ts` | Refuse decimal GitHub ids on lifecycle, review-request, approval, and request-changes fallbacks |
| Create `desktop/src/features/projects/pullRequestMutations.github.test.mjs` | Prove GitHub never calls the Buzz publisher and invalidates only pull-requests |
| `desktop/src/features/projects/ui/CreatePullRequestDialog.tsx` | Read `data.pullRequests` for the duplicate-open check |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Read the new query shape and filter profile lookup to 64-hex identities |
| `desktop/src/features/projects/ui/ProjectWorkspaceTabList.tsx` | Hide Files changed and use `commentCount` for the conversation badge |
| `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx` | Branch GitHub list/detail chrome, snap `pr-files` to conversation, and skip Files changed |
| Create `desktop/src/features/projects/ui/GitHubProjectPullRequests.tsx` | Render GitHub list rows, read-only detail, login identities, and comment states |
| Create `desktop/src/features/projects/ui/GitHubProjectPullRequests.test.mjs` | Render the GitHub row/detail/tab contract before implementing the components |
| `desktop/src/testing/e2eBridge.ts` | Stub list/create/comments and structured pull errors |
| Create `desktop/tests/e2e/github-pull-requests.spec.ts` | Exercise list, detail comments, create, hidden write controls, and auth recovery |
| `desktop/tests/e2e/project-pr-review.spec.ts` | Remove obsolete GitHub-hosted kind:1618 merge UI scenarios while retaining Buzz-hosted review/merge coverage and unrelated GitHub clone coverage |
| `desktop/playwright.config.ts` | Add the new spec to the smoke project |

---

### Task 1: Add the bounded Rust pull-request list core

**Files:**

- Create: `desktop/src-tauri/src/commands/project_github_pulls.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs` to add only `mod project_github_pulls;` in this task

**Interfaces:**

- Consumes: `GitHubRepoRef::{parse, slug, owner, repo}`, `GhRunner::{ensure_auth, from_resolved, run_with_limit}`, `combined_cli_diagnostic`, `redact_diagnostic`, and `ProjectPullRequestMergeError`
- Produces: public DTOs `GitHubPullRequestUserDto`, `GitHubPullRequestRepoDto`, `GitHubPullRequestHeadDto`, `GitHubPullRequestBaseDto`, `GitHubPullRequestDto`, and `GitHubPullRequestListDto`, plus module-private `list_github_pull_requests_with`, `is_pull_html_url`, and `remap_pulls_error`

- [ ] **Step 1: Run impact checks before touching existing symbols**

Run GitNexus upstream impact for `GitHubRepoRef`, `GhRunner::run_with_limit`, `combined_cli_diagnostic`, and `redact_diagnostic`.
Report direct callers, affected processes, and risk level.
Stop and warn before editing if any result is HIGH or CRITICAL.

- [ ] **Step 2: Add the module declaration and write failing tests first**

Add `mod project_github_pulls;` beside the other `project_github_*` modules so its unit tests compile as part of the desktop library.
Create the new file with test-only fixtures and tests that reference the not-yet-defined production functions.
The fake `gh` output must already match the declared `--jq` projection.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
    use crate::commands::project_github_pull_request::{GhRunner, GitHubRepoRef};
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
            "body": "PR body",
            "html_url": format!("https://github.com/acme/app/pull/{number}"),
            "draft": draft,
            "comments": 3,
            "created_at": "2026-01-02T03:04:05Z",
            "updated_at": "2026-01-03T03:04:05Z",
            "user": { "login": "ada", "avatar_url": "https://avatars.githubusercontent.com/u/1" },
            "head": {
                "ref": "feature",
                "sha": "dddddddddddddddddddddddddddddddddddddddd",
                "repo": { "full_name": "acme/app" }
            },
            "base": {
                "ref": "develop",
                "repo": { "full_name": "acme/app" }
            }
        })
    }

    #[cfg(unix)]
    fn fake_gh_raw(output: &str, status: i32, stderr: &str) -> (tempfile::TempDir, PathBuf) {
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
    fn fake_gh(output: &serde_json::Value, status: i32, stderr: &str) -> (tempfile::TempDir, PathBuf) {
        fake_gh_raw(&output.to_string(), status, stderr)
    }

    #[cfg(unix)]
    #[test]
    fn maps_projected_open_and_draft_pull_fields() {
        let output = json!([projected_pull(42, false), projected_pull(7, true)]);
        let (dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect("list");
        assert_eq!(page.pulls.len(), 2);
        assert_eq!(page.pulls[0].number, 42);
        assert!(!page.pulls[0].draft);
        assert_eq!(page.pulls[0].user.login, "ada");
        assert_eq!(page.pulls[0].head.git_ref, "feature");
        assert_eq!(page.pulls[0].head.sha, "dddddddddddddddddddddddddddddddddddddddd");
        assert_eq!(page.pulls[0].head.repo.full_name, "acme/app");
        assert_eq!(page.pulls[0].base.git_ref, "develop");
        assert!(page.pulls[1].draft);
        assert!(!page.has_more);
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
        assert!(calls.contains("/repos/acme/app/pulls?state=open&per_page=100&sort=updated&direction=desc"));
        assert!(calls.contains("--jq"));
        assert!(!calls.contains("--paginate"));
    }

    #[cfg(unix)]
    #[test]
    fn inbound_fork_keeps_head_repo_full_name() {
        let mut item = projected_pull(42, false);
        item["head"]["repo"]["full_name"] = json!("other/app");
        item["head"]["ref"] = json!("fork-branch");
        let (_dir, path) = fake_gh(&json!([item]), 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect("list");
        assert_eq!(page.pulls[0].head.repo.full_name, "other/app");
        assert_eq!(page.pulls[0].head.git_ref, "fork-branch");
    }

    #[cfg(unix)]
    #[test]
    fn raw_projected_page_of_100_sets_has_more_before_filtering() {
        let output = serde_json::Value::Array(
            (1..=100)
                .map(|number| {
                    let mut item = projected_pull(number, false);
                    item["html_url"] = json!("https://evil.example/pull/1");
                    item
                })
                .collect(),
        );
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect("list");
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
            ("https://github.com/acme/app/issues/42", 42, false),
            ("https://evil.example/acme/app/pull/42", 42, false),
            ("https://user@github.com/acme/app/pull/42", 42, false),
            ("https://github.com/acme/app/pull/42?x=1", 42, false),
            ("https://github.com/acme/app/pull/42#x", 42, false),
            ("https://github.com/acme/app/pull/42/", 42, false),
        ] {
            assert_eq!(is_pull_html_url(&repo, raw, number), expected, "{raw}");
        }
    }

    #[test]
    fn rejects_non_github_clone_url_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = list_github_pull_requests_with(
            &gh,
            &format!("https://relay.example/git/{}/app", "ab".repeat(32)),
        )
        .expect_err("reject Buzz URL");
        assert_eq!(error_code(&error), "github_pulls_failed");
    }

    #[cfg(unix)]
    #[test]
    fn malformed_or_truncated_json_is_an_error_not_an_empty_list() {
        let (_dir, path) = fake_gh_raw("[{", 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let error = list_github_pull_requests_with(&gh, "https://github.com/acme/app")
            .expect_err("malformed response");
        assert_eq!(error_code(&error), "github_pulls_failed");
    }

    #[cfg(unix)]
    #[test]
    fn failed_auth_maps_to_auth_required() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("gh");
        std::fs::write(
            &path,
            "#!/bin/sh\ncase \"$*\" in *auth*status*) exit 1 ;; *) exit 1 ;; esac\n",
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
    fn remaps_repository_and_rate_failures_to_pull_codes() {
        let not_found = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 404");
        assert_eq!(
            error_code(&remap_pulls_error(not_found, "Not Found", PullsNotFound::Repo)),
            "github_repo_unavailable"
        );
        let forbidden = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
        assert_eq!(
            error_code(&remap_pulls_error(forbidden, "Forbidden", PullsNotFound::Repo)),
            "github_repo_unavailable"
        );
        let limited = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
        assert_eq!(
            error_code(&remap_pulls_error(limited, "API rate limit exceeded", PullsNotFound::Repo)),
            "github_pulls_failed"
        );
        let missing_pr = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 404");
        assert_eq!(
            error_code(&remap_pulls_error(missing_pr, "Not Found", PullsNotFound::PullRequest)),
            "github_pr_unavailable"
        );
    }
}
```

- [ ] **Step 3: Run the focused test and verify RED**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib maps_projected_open_and_draft_pull_fields
```

Expected: compilation fails because `list_github_pull_requests_with` and the DTOs do not exist.
Do not change production code until the failure is observed for that reason.

- [ ] **Step 4: Implement the minimal list core**

Add the following shapes and constants in `project_github_pulls.rs`.

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
const PULL_LIST_JQ: &str = "[.[] | {number, title, body: (.body // \"\"), html_url, draft: (.draft // false), comments, created_at, updated_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), head: {ref: .head.ref, sha: .head.sha, repo: {full_name: (.head.repo.full_name // \"\")}}, base: {ref: .base.ref, repo: {full_name: (.base.repo.full_name // \"\")}}}]";

/// GitHub login identity returned to the desktop pull-request UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestUserDto {
    pub login: String,
    pub avatar_url: String,
}

/// Nested repository identity on a GitHub pull-request head or base.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestRepoDto {
    pub full_name: String,
}

/// Head ref returned to the desktop pull-request UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestHeadDto {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub sha: String,
    pub repo: GitHubPullRequestRepoDto,
}

/// Base ref returned to the desktop pull-request UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestBaseDto {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub repo: GitHubPullRequestRepoDto,
}

/// Bounded GitHub pull request returned to the desktop UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestDto {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub html_url: String,
    pub draft: bool,
    pub comments: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub user: GitHubPullRequestUserDto,
    pub head: GitHubPullRequestHeadDto,
    pub base: GitHubPullRequestBaseDto,
}

/// One bounded GitHub pull-request page plus its first-page truncation signal.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestListDto {
    pub pulls: Vec<GitHubPullRequestDto>,
    pub has_more: bool,
}

#[derive(Clone, Copy)]
enum PullsNotFound {
    Repo,
    PullRequest,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestUserWire {
    login: String,
    avatar_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestRepoWire {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestHeadWire {
    #[serde(rename = "ref")]
    git_ref: String,
    sha: String,
    repo: GitHubPullRequestRepoWire,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestBaseWire {
    #[serde(rename = "ref")]
    git_ref: String,
    repo: GitHubPullRequestRepoWire,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestWire {
    number: u64,
    title: String,
    body: String,
    html_url: String,
    draft: bool,
    comments: u64,
    created_at: String,
    updated_at: String,
    user: Option<GitHubPullRequestUserWire>,
    head: GitHubPullRequestHeadWire,
    base: GitHubPullRequestBaseWire,
}
```

Implement the request boundary once so list, create, and comments share the same cap.

```rust
fn github_api_json<T: DeserializeOwned>(
    gh: &GhRunner,
    method: &str,
    path: &str,
    jq: &str,
    input: Option<&Path>,
    not_found: PullsNotFound,
) -> Result<T, ProjectPullRequestMergeError> {
    let mut args = vec![
        OsString::from("api"),
        OsString::from("--hostname"),
        OsString::from("github.com"),
        OsString::from("--method"),
        OsString::from(method),
        OsString::from(path),
        OsString::from("--jq"),
        OsString::from(jq),
    ];
    if let Some(input) = input {
        args.push(OsString::from("--input"));
        args.push(input.as_os_str().to_os_string());
    }
    let output = gh
        .run_with_limit(&args, GH_PULL_STREAM_LIMIT)
        .map_err(|error| remap_pulls_error(error, "", not_found))?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        return Err(remap_pulls_error(
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                redact_diagnostic(&diagnostic),
            ),
            &diagnostic,
            not_found,
        ));
    }
    serde_json::from_str(&output.stdout).map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "GitHub CLI returned an unexpected or truncated pull request response. Update gh, then retry.",
        )
    })
}
```

Implement URL validation with `url::Url`.
Require HTTPS, host `github.com`, no username/password/query/fragment, no non-default port, exactly four path segments, case-insensitive owner/repo equality, literal `pull`, and a matching positive number.

```rust
fn is_pull_html_url(repo: &GitHubRepoRef, raw: &str, number: u64) -> bool {
    if number == 0 || raw != raw.trim() || raw.contains('\\') || raw.contains('%') {
        return false;
    }
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    let Some(segments) = url.path_segments().map(|segments| segments.collect::<Vec<_>>()) else {
        return false;
    };
    segments.len() == 4
        && segments[0].eq_ignore_ascii_case(&repo.owner)
        && segments[1].eq_ignore_ascii_case(&repo.repo)
        && segments[2] == "pull"
        && segments[3].parse::<u64>().ok() == Some(number)
}
```

Implement `map_pull` as a filter boundary.
Return `None` for number zero, empty login, empty head/base ref, empty head sha, invalid timestamp, or invalid `html_url`.

```rust
fn map_pull(repo: &GitHubRepoRef, item: GitHubPullRequestWire) -> Option<GitHubPullRequestDto> {
    let user = item.user?;
    if item.number == 0
        || user.login.trim().is_empty()
        || item.head.git_ref.trim().is_empty()
        || item.base.git_ref.trim().is_empty()
        || item.head.sha.trim().is_empty()
        || !is_pull_html_url(repo, &item.html_url, item.number)
    {
        return None;
    }
    let created_at = DateTime::parse_from_rfc3339(&item.created_at).ok()?.timestamp();
    let updated_at = DateTime::parse_from_rfc3339(&item.updated_at).ok()?.timestamp();
    Some(GitHubPullRequestDto {
        number: item.number,
        title: item.title,
        body: item.body,
        html_url: item.html_url,
        draft: item.draft,
        comments: item.comments,
        created_at,
        updated_at,
        user: GitHubPullRequestUserDto {
            login: user.login,
            avatar_url: user.avatar_url,
        },
        head: GitHubPullRequestHeadDto {
            git_ref: item.head.git_ref,
            sha: item.head.sha,
            repo: GitHubPullRequestRepoDto {
                full_name: item.head.repo.full_name,
            },
        },
        base: GitHubPullRequestBaseDto {
            git_ref: item.base.git_ref,
            repo: GitHubPullRequestRepoDto {
                full_name: item.base.repo.full_name,
            },
        },
    })
}

fn remap_pulls_error(
    error: ProjectPullRequestMergeError,
    diagnostic: &str,
    not_found: PullsNotFound,
) -> ProjectPullRequestMergeError {
    let value = serde_json::to_value(&error).unwrap_or_default();
    let code = value.get("code").and_then(|value| value.as_str()).unwrap_or("");
    if matches!(code, "github_cli_missing" | "github_auth_required") {
        return error;
    }
    let original = value.get("message").and_then(|value| value.as_str()).unwrap_or("");
    let combined = format!("{diagnostic} {original}");
    let lower = combined.to_ascii_lowercase();
    let message = if diagnostic.trim().is_empty() {
        if original.is_empty() {
            "GitHub pull request request failed.".to_string()
        } else {
            original.to_string()
        }
    } else {
        redact_diagnostic(diagnostic)
    };
    if lower.contains("rate limit") || lower.contains("abuse") {
        return ProjectPullRequestMergeError::new("github_pulls_failed", message);
    }
    if lower.contains("404")
        || lower.contains("not found")
        || (lower.contains("403") && !lower.contains("rate"))
    {
        let code = match not_found {
            PullsNotFound::Repo => "github_repo_unavailable",
            PullsNotFound::PullRequest => "github_pr_unavailable",
        };
        return ProjectPullRequestMergeError::new(code, message);
    }
    ProjectPullRequestMergeError::new("github_pulls_failed", message)
}

fn list_github_pull_requests_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<GitHubPullRequestListDto, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_pulls_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_pulls_error(error, "", PullsNotFound::Repo))?;
    let path = format!(
        "/repos/{}/pulls?state=open&per_page=100&sort=updated&direction=desc",
        repo.slug(),
    );
    let raw: Vec<GitHubPullRequestWire> =
        github_api_json(gh, "GET", &path, PULL_LIST_JQ, None, PullsNotFound::Repo)?;
    let has_more = raw.len() == 100;
    let pulls = raw.into_iter().filter_map(|item| map_pull(&repo, item)).collect();
    Ok(GitHubPullRequestListDto { pulls, has_more })
}
```

- [ ] **Step 5: Run all pull-module tests and verify GREEN**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_pulls
```

Expected: all list, URL, validation, and error tests pass on Unix; runner-backed tests are skipped on non-Unix platforms.

- [ ] **Step 6: Format, inspect scope, and commit**

Run GitNexus `detect_changes({ scope: "staged" })` after staging and verify only the new pull module and its module declaration are affected.

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_pulls.rs \
  desktop/src-tauri/src/commands/mod.rs
git diff --check
git commit -s -m "feat(projects): map GitHub pull requests from gh api"
```

---

### Task 2: Add GitHub pull-request creation and read-only comment loading in Rust

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_pulls.rs`

**Interfaces:**

- Consumes: `github_api_json`, `map_pull`, `remap_pulls_error`, `GitHubRepoRef`, and `GhRunner` from Task 1
- Produces: public DTO `GitHubPullRequestCommentDto` plus module-private `is_pull_comment_html_url`, `create_github_pull_request_with`, and `list_github_pull_request_comments_with`

- [ ] **Step 1: Write failing create and comment tests**

Keep each boundary in its own test.
The create fake must copy the `--input` tempfile before the process exits so the test can assert the real JSON body.

```rust
#[test]
fn create_rejects_blank_title_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let error = create_github_pull_request_with(&gh, "https://github.com/acme/app", "   ", "body", "feature", "develop")
        .expect_err("blank");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn create_rejects_more_than_256_unicode_scalars_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let title = "é".repeat(257);
    let error = create_github_pull_request_with(&gh, "https://github.com/acme/app", &title, "", "feature", "develop")
        .expect_err("long");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn create_rejects_empty_or_equal_branches_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let empty_head = create_github_pull_request_with(&gh, "https://github.com/acme/app", "Title", "", "  ", "develop")
        .expect_err("empty head");
    assert_eq!(error_code(&empty_head), "github_pulls_failed");
    let empty_base = create_github_pull_request_with(&gh, "https://github.com/acme/app", "Title", "", "feature", "  ")
        .expect_err("empty base");
    assert_eq!(error_code(&empty_base), "github_pulls_failed");
    let same = create_github_pull_request_with(&gh, "https://github.com/acme/app", "Title", "", "develop", "develop")
        .expect_err("same");
    assert_eq!(error_code(&same), "github_pulls_failed");
}

#[test]
fn create_rejects_cross_repository_head_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let error = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "Title",
        "",
        "fork-owner:feature",
        "develop",
    )
    .expect_err("cross-repository head");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[cfg(unix)]
#[test]
fn create_posts_trimmed_title_and_exact_body() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("gh");
    let output = projected_pull(43, false).to_string();
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
        "  Pull 43  ",
        " body with surrounding space ",
        "  feature  ",
        "  develop  ",
    )
    .expect("create");
    assert_eq!(pull.number, 43);
    let input: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
    )
    .expect("json");
    assert_eq!(input, serde_json::json!({
        "title": "Pull 43",
        "body": " body with surrounding space ",
        "head": "feature",
        "base": "develop",
    }));
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("--method POST"));
    assert!(calls.contains("/repos/acme/app/pulls"));
    assert!(calls.contains("--input"));
    assert!(!calls.contains("/pulls/"));
}

#[cfg(unix)]
#[test]
fn create_rejects_response_with_foreign_html_url() {
    let mut output = projected_pull(43, false);
    output["html_url"] = serde_json::Value::String("https://evil.example/pull/43".to_string());
    let (_dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = create_github_pull_request_with(&gh, "https://github.com/acme/app", "Pull 43", "", "feature", "develop")
        .expect_err("foreign URL");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn comments_reject_number_zero_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let error = list_github_pull_request_comments_with(&gh, "https://github.com/acme/app", 0)
        .expect_err("zero");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn accepts_issue_and_pull_comment_urls_for_the_same_number() {
    let repo = GitHubRepoRef::parse("https://github.com/acme/app").expect("repo");
    assert!(is_pull_comment_html_url(
        &repo,
        "https://github.com/acme/app/issues/42#issuecomment-9",
        42,
        9,
    ));
    assert!(is_pull_comment_html_url(
        &repo,
        "https://github.com/acme/app/pull/42#issuecomment-9",
        42,
        9,
    ));
    assert!(!is_pull_comment_html_url(
        &repo,
        "https://github.com/acme/app/pull/42#discussion_r9",
        42,
        9,
    ));
    assert!(!is_pull_comment_html_url(
        &repo,
        "https://github.com/acme/other/issues/42#issuecomment-9",
        42,
        9,
    ));
}

#[cfg(unix)]
#[test]
fn comments_keep_projected_github_order_and_drop_invalid_urls() {
    let output = serde_json::json!([
        { "id": 1, "body": "first", "html_url": "https://github.com/acme/app/issues/42#issuecomment-1", "created_at": "2026-01-02T03:04:05Z", "user": { "login": "ada", "avatar_url": "https://example.com/a" } },
        { "id": 2, "body": "bad", "html_url": "https://evil.example/comment/2", "created_at": "2026-01-02T04:04:05Z", "user": { "login": "eve", "avatar_url": "https://example.com/e" } },
        { "id": 3, "body": "second", "html_url": "https://github.com/acme/app/pull/42#issuecomment-3", "created_at": "2026-01-02T05:04:05Z", "user": { "login": "linus", "avatar_url": "https://example.com/b" } }
    ]);
    let (dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let comments = list_github_pull_request_comments_with(&gh, "https://github.com/acme/app", 42)
        .expect("comments");
    assert_eq!(
        comments.iter().map(|comment| comment.body.as_str()).collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(comments[1].user.login, "linus");
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("/repos/acme/app/issues/42/comments?per_page=100"));
    assert!(!calls.contains("/pulls/42/comments"));
}

#[cfg(unix)]
#[test]
fn comments_map_http_404_to_pr_unavailable() {
    let (_dir, path) = fake_gh(&serde_json::json!({}), 1, "gh: HTTP 404 Not Found");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = list_github_pull_request_comments_with(&gh, "https://github.com/acme/app", 42)
        .expect_err("missing");
    assert_eq!(error_code(&error), "github_pr_unavailable");
}
```

- [ ] **Step 2: Run and verify RED**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib create_posts_trimmed_title_and_exact_body
```

Expected: compilation fails because `create_github_pull_request_with` does not exist.

- [ ] **Step 3: Implement create and comments minimally**

```rust
const PULL_ITEM_JQ: &str = "{number, title, body: (.body // \"\"), html_url, draft: (.draft // false), comments, created_at, updated_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), head: {ref: .head.ref, sha: .head.sha, repo: {full_name: (.head.repo.full_name // \"\")}}, base: {ref: .base.ref, repo: {full_name: (.base.repo.full_name // \"\")}}}";
const PULL_COMMENTS_JQ: &str = "[.[] | {id, body: (.body // \"\"), html_url, created_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end)}]";

/// One read-only GitHub pull-request conversation comment.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestCommentDto {
    pub id: u64,
    pub body: String,
    pub html_url: String,
    pub created_at: i64,
    pub user: GitHubPullRequestUserDto,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestCommentWire {
    id: u64,
    body: String,
    html_url: String,
    created_at: String,
    user: Option<GitHubPullRequestUserWire>,
}

fn is_pull_comment_html_url(
    repo: &GitHubRepoRef,
    raw: &str,
    number: u64,
    comment_id: u64,
) -> bool {
    if number == 0 || comment_id == 0 || raw != raw.trim() || raw.contains('\\') || raw.contains('%')
    {
        return false;
    }
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    let expected_fragment = format!("issuecomment-{comment_id}");
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment() != Some(expected_fragment.as_str())
    {
        return false;
    }
    let Some(segments) = url.path_segments().map(|segments| segments.collect::<Vec<_>>()) else {
        return false;
    };
    segments.len() == 4
        && segments[0].eq_ignore_ascii_case(&repo.owner)
        && segments[1].eq_ignore_ascii_case(&repo.repo)
        && matches!(segments[2], "issues" | "pull")
        && segments[3].parse::<u64>().ok() == Some(number)
}

fn pull_json_input(
    title: &str,
    body: &str,
    head: &str,
    base: &str,
) -> Result<tempfile::NamedTempFile, ProjectPullRequestMergeError> {
    use std::io::Write;
    let mut file = tempfile::Builder::new()
        .prefix("buzz-gh-")
        .tempfile()
        .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({ "title": title, "body": body, "head": head, "base": base }),
    )
    .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?;
    file.flush()
        .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?;
    Ok(file)
}

fn create_github_pull_request_with(
    gh: &GhRunner,
    clone_url: &str,
    title: &str,
    body: &str,
    head: &str,
    base: &str,
) -> Result<GitHubPullRequestDto, ProjectPullRequestMergeError> {
    let title = title.trim();
    let head = head.trim();
    let base = base.trim();
    if title.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "Pull request title is required.",
        ));
    }
    if title.chars().count() > 256 {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "Pull request title must be 256 characters or fewer.",
        ));
    }
    if head.is_empty() || base.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "Pull request branches are required.",
        ));
    }
    if head.contains(':') {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "The compare branch must belong to this repository.",
        ));
    }
    if head == base {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "The base and compare branches must be different.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_pulls_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_pulls_error(error, "", PullsNotFound::Repo))?;
    let input = pull_json_input(title, body, head, base)?;
    let path = format!("/repos/{}/pulls", repo.slug());
    let raw: GitHubPullRequestWire = github_api_json(
        gh,
        "POST",
        &path,
        PULL_ITEM_JQ,
        Some(input.path()),
        PullsNotFound::Repo,
    )?;
    map_pull(&repo, raw).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "GitHub returned an invalid created pull request.",
        )
    })
}

fn list_github_pull_request_comments_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
) -> Result<Vec<GitHubPullRequestCommentDto>, ProjectPullRequestMergeError> {
    if number == 0 {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "GitHub pull request number must be greater than zero.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_pulls_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_pulls_error(error, "", PullsNotFound::PullRequest))?;
    let path = format!("/repos/{}/issues/{number}/comments?per_page=100", repo.slug());
    let raw: Vec<GitHubPullRequestCommentWire> = github_api_json(
        gh,
        "GET",
        &path,
        PULL_COMMENTS_JQ,
        None,
        PullsNotFound::PullRequest,
    )?;
    Ok(raw
        .into_iter()
        .filter_map(|comment| {
            let user = comment.user?;
            if comment.id == 0
                || user.login.trim().is_empty()
                || !is_pull_comment_html_url(&repo, &comment.html_url, number, comment.id)
            {
                return None;
            }
            let created_at = DateTime::parse_from_rfc3339(&comment.created_at).ok()?.timestamp();
            Some(GitHubPullRequestCommentDto {
                id: comment.id,
                body: comment.body,
                html_url: comment.html_url,
                created_at,
                user: GitHubPullRequestUserDto {
                    login: user.login,
                    avatar_url: user.avatar_url,
                },
            })
        })
        .collect())
}
```

- [ ] **Step 4: Run all Rust pull tests and verify GREEN**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_pulls
```

Expected: all list, create, comment, validation, and error tests pass.

- [ ] **Step 5: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_pulls.rs
git diff --check
git commit -s -m "feat(projects): create GitHub pull requests and list comments"
```

---

### Task 3: Expose and register the three Tauri commands

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_github_pulls.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs`
- Modify: `desktop/src-tauri/src/invoke.rs`

**Interfaces:**

- Consumes: the three injected-runner functions from Tasks 1 and 2 and `GhRunner::discover`
- Produces: public Tauri commands `list_github_pull_requests`, `create_github_pull_request`, and `list_github_pull_request_comments`, with module-private injected-runner wrappers

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `get_github_repository_state` and `desktop_invoke_handler`.
The module declaration and re-export are not indexed function symbols; verify their scope with the final `detect_changes` result.

- [ ] **Step 2: Write failing discovery-wrapper tests**

```rust
#[test]
fn list_wrapper_maps_missing_discovered_cli() {
    let error = list_github_pull_requests_with_runner(
        "https://github.com/acme/app".to_string(),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}

#[test]
fn create_wrapper_maps_missing_discovered_cli() {
    let error = create_github_pull_request_with_runner(
        "https://github.com/acme/app".to_string(),
        "title".to_string(),
        "body".to_string(),
        "feature".to_string(),
        "develop".to_string(),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}

#[test]
fn comments_wrapper_maps_missing_discovered_cli() {
    let error = list_github_pull_request_comments_with_runner(
        "https://github.com/acme/app".to_string(),
        42,
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}
```

- [ ] **Step 3: Run and verify RED**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib list_wrapper_maps_missing_discovered_cli
```

Expected: compilation fails because `list_github_pull_requests_with_runner` does not exist.

- [ ] **Step 4: Implement wrappers, commands, and registration**

```rust
fn list_github_pull_requests_with_runner(
    clone_url: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubPullRequestListDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_pulls_error(error, "", PullsNotFound::Repo))?;
    list_github_pull_requests_with(&gh, &clone_url)
}

fn create_github_pull_request_with_runner(
    clone_url: String,
    title: String,
    body: String,
    head: String,
    base: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubPullRequestDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_pulls_error(error, "", PullsNotFound::Repo))?;
    create_github_pull_request_with(&gh, &clone_url, &title, &body, &head, &base)
}

fn list_github_pull_request_comments_with_runner(
    clone_url: String,
    number: u64,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<Vec<GitHubPullRequestCommentDto>, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_pulls_error(error, "", PullsNotFound::PullRequest))?;
    list_github_pull_request_comments_with(&gh, &clone_url, number)
}

/// List one page of open GitHub pull requests for a github.com clone URL.
#[tauri::command]
pub async fn list_github_pull_requests(
    clone_url: String,
) -> Result<GitHubPullRequestListDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_pull_requests_with_runner(clone_url, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?
}

/// Create one ready GitHub pull request for a github.com clone URL.
#[tauri::command]
pub async fn create_github_pull_request(
    clone_url: String,
    title: String,
    body: String,
    head: String,
    base: String,
) -> Result<GitHubPullRequestDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        create_github_pull_request_with_runner(clone_url, title, body, head, base, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?
}

/// List the first page of conversation comments for one GitHub pull request.
#[tauri::command]
pub async fn list_github_pull_request_comments(
    clone_url: String,
    number: u64,
) -> Result<Vec<GitHubPullRequestCommentDto>, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_pull_request_comments_with_runner(clone_url, number, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?
}
```

Add `pub use project_github_pulls::*;` in `commands/mod.rs` next to `pub use project_github_issues::*;`.
Register the three names immediately after `list_github_issue_comments` in `invoke.rs`.

```rust
        list_github_issue_comments,
        list_github_pull_requests,
        create_github_pull_request,
        list_github_pull_request_comments,
        update_github_issue_state,
```

- [ ] **Step 5: Run Rust tests and the line-count ratchet**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_pulls
. ./bin/activate-hermit && cd desktop && pnpm check:file-sizes
```

Expected: tests pass and no file crosses 1,000 lines.

- [ ] **Step 6: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_pulls.rs \
  desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/invoke.rs
git diff --check
git commit -s -m "feat(projects): expose GitHub pull request Tauri commands"
```

---

### Task 4: Add TypeScript DTOs, ProjectPullRequest mapping, and safe share links

**Files:**

- Modify: `desktop/src/shared/api/projectGit.ts`
- Modify: `desktop/src/features/projects/projectPullRequests.mjs`
- Modify: `desktop/src/features/projects/projectPullRequests.d.mts`
- Modify: `desktop/src/features/projects/projectPullRequests.test.mjs`
- Create: `desktop/src/features/projects/lib/projectGithubPulls.ts`
- Create: `desktop/src/features/projects/lib/projectGithubPulls.test.mjs`
- Modify: `desktop/src/features/projects/lib/projectShareLinks.ts`
- Modify: `desktop/src/features/projects/lib/projectShareLinks.test.mjs`

**Interfaces:**

- Consumes: `invokeTauri`, `parseProjectPullRequestMergeError`, `isGitHubCloneUrl`, `Repository`, and `ProjectPullRequest`
- Produces: native DTO wrappers, `{ pullRequests, hasMore }` host routing, GitHub pull/comment mappers, decimal-number display helpers, hex-only identity collection, branch labels, and safe GitHub pull sharing

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `eventToProjectPullRequest`, `pullRequestShareLink`, and `parseProjectPullRequestMergeError`.
Also run upstream impact for the `ProjectPullRequest` type before adding required fields to its declaration.

- [ ] **Step 2: Write mapper, routing, declaration-default, identity, number, comment, and share tests before production changes**

Add this Nostr regression to `projectPullRequests.test.mjs`.

```js
test("Nostr pull requests expose neutral GitHub extension fields", () => {
  const pullRequest = eventToProjectPullRequest(pullRequestEvent());
  assert.equal(pullRequest.authorAvatarUrl, null);
  assert.equal(pullRequest.headRepoFullName, null);
  assert.equal(pullRequest.htmlUrl, null);
  assert.equal(pullRequest.commentCount, 0);
  assert.equal(pullRequest.status, "Open");
});
```

Create `projectGithubPulls.test.mjs` with a complete DTO and independent literal expectations.

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchProjectPullRequestsWith,
  githubRepoFullNameFromCloneUrl,
  githubPullRequestBranchLabel,
  githubPullRequestCommentsRequest,
  githubPullRequestConversationCount,
  githubPullRequestId,
  mapGithubCommentToProjectPullRequestComment,
  mapGithubPullRequestToProjectPullRequest,
  parseGithubPullRequestNumber,
  pullRequestDisplayNumber,
  pullRequestIdentityPubkeys,
  requireBuzzPullRequestEventId,
  selectedGithubPullRequestAfterListLoad,
} from "./projectGithubPulls.ts";

const REPO_ADDRESS = `30617:${"a".repeat(64)}:app`;
const dto = {
  number: 42,
  title: "Fix login",
  body: "PR body",
  html_url: "https://github.com/acme/app/pull/42",
  draft: false,
  comments: 3,
  created_at: 1_704_166_645,
  updated_at: 1_704_253_045,
  user: { login: "ada", avatar_url: "https://avatars.githubusercontent.com/u/1" },
  head: {
    ref: "feature",
    sha: "d".repeat(40),
    repo: { full_name: "acme/app" },
  },
  base: {
    ref: "develop",
    repo: { full_name: "acme/app" },
  },
};

test("GitHub pull mapper fills the complete ProjectPullRequest contract", () => {
  const pullRequest = mapGithubPullRequestToProjectPullRequest(dto, {
    repoAddress: REPO_ADDRESS,
    cloneUrl: "https://github.com/acme/app",
  });
  assert.equal(pullRequest.id, "42");
  assert.equal(pullRequest.title, "Fix login");
  assert.equal(pullRequest.content, "PR body");
  assert.equal(pullRequest.author, "ada");
  assert.equal(pullRequest.authorAvatarUrl, "https://avatars.githubusercontent.com/u/1");
  assert.equal(pullRequest.status, "Open");
  assert.equal(pullRequest.branchName, "feature");
  assert.equal(pullRequest.targetBranch, "develop");
  assert.equal(pullRequest.commit, "d".repeat(40));
  assert.equal(pullRequest.headRepoFullName, "acme/app");
  assert.equal(pullRequest.htmlUrl, "https://github.com/acme/app/pull/42");
  assert.deepEqual(pullRequest.comments, []);
  assert.equal(pullRequest.commentCount, 3);
  assert.equal(pullRequest.updateCount, 0);
  assert.deepEqual(pullRequest.reviewers, []);
  assert.deepEqual(pullRequest.cloneUrls, ["https://github.com/acme/app"]);
});

test("GitHub draft maps to Draft and never to Open", () => {
  const pullRequest = mapGithubPullRequestToProjectPullRequest(
    { ...dto, draft: true },
    { repoAddress: REPO_ADDRESS, cloneUrl: "https://github.com/acme/app" },
  );
  assert.equal(pullRequest.status, "Draft");
});

test("GitHub pull ids reject non-positive, fractional, and unsafe numbers", () => {
  assert.equal(githubPullRequestId(42), "42");
  for (const number of [0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(
      () => githubPullRequestId(number),
      /GitHub returned an invalid pull request number/,
    );
    assert.throws(
      () =>
        mapGithubPullRequestToProjectPullRequest(
          { ...dto, number },
          { repoAddress: REPO_ADDRESS, cloneUrl: "https://github.com/acme/app" },
        ),
      /GitHub returned an invalid pull request number/,
    );
  }
});

test("GitHub comment mapper keeps login and avatar without pubkey conversion", () => {
  assert.deepEqual(
    mapGithubCommentToProjectPullRequestComment({
      id: 9,
      body: "Looks good.",
      html_url: "https://github.com/acme/app/issues/42#issuecomment-9",
      created_at: 1_704_253_100,
      user: { login: "grace", avatar_url: "https://avatars.githubusercontent.com/u/3" },
    }),
    {
      id: "9",
      content: "Looks good.",
      tags: [],
      author: "grace",
      authorAvatarUrl: "https://avatars.githubusercontent.com/u/3",
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
      loadGithub: async (input) => {
        calls.github += 1;
        assert.deepEqual(input, { cloneUrl: "https://github.com/acme/app" });
        return { pulls: [dto], has_more: true };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [];
      },
    },
  );
  assert.equal(calls.github, 1);
  assert.equal(calls.buzz, 0);
  assert.equal(result.pullRequests[0].id, "42");
  assert.equal(result.hasMore, true);
});

test("host routing invokes only Nostr for a Buzz clone URL", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectPullRequestsWith(
    {
      id: "p2",
      repoAddress: REPO_ADDRESS,
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
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
  assert.equal(calls.github, 0);
  assert.equal(calls.buzz, 1);
  assert.equal(result.pullRequests[0].id, "e".repeat(64));
  assert.equal(result.hasMore, false);
});

test("GitHub pull number parser accepts only positive safe decimal integers", () => {
  assert.equal(parseGithubPullRequestNumber("42"), 42);
  assert.equal(parseGithubPullRequestNumber("0"), null);
  assert.equal(parseGithubPullRequestNumber("01"), null);
  assert.equal(parseGithubPullRequestNumber("0x2"), null);
  assert.equal(parseGithubPullRequestNumber("9007199254740992"), null);
  assert.equal(parseGithubPullRequestNumber("e".repeat(64)), null);
  assert.equal(pullRequestDisplayNumber("42"), "42");
  assert.equal(pullRequestDisplayNumber("e".repeat(64)), "eeeeeeee");
  assert.equal(githubPullRequestId(42), "42");
});

test("Nostr-only writes reject GitHub decimal ids", () => {
  assert.equal(requireBuzzPullRequestEventId("e".repeat(64)), "e".repeat(64));
  assert.throws(
    () => requireBuzzPullRequestEventId("42"),
    /cannot be mutated through Nostr/,
  );
});

test("identity collection drops GitHub logins and keeps lowercase Nostr pubkeys", () => {
  const github = mapGithubPullRequestToProjectPullRequest(dto, {
    repoAddress: REPO_ADDRESS,
    cloneUrl: "https://github.com/acme/app",
  });
  const nostr = {
    ...github,
    id: "f".repeat(64),
    author: "A".repeat(64),
    reviewers: ["B".repeat(64)],
    comments: [
      {
        ...mapGithubCommentToProjectPullRequestComment({
          id: 1,
          body: "x",
          html_url: "https://github.com/acme/app/issues/1#issuecomment-1",
          created_at: 1,
          user: { login: "x", avatar_url: "" },
        }),
        author: "D".repeat(64),
      },
    ],
  };
  assert.deepEqual(pullRequestIdentityPubkeys([github]), []);
  assert.deepEqual(pullRequestIdentityPubkeys([nostr]), [
    "a".repeat(64),
    "b".repeat(64),
    "d".repeat(64),
  ]);
});

test("fork heads render owner:branch and empty head repos stay same-repo", () => {
  const same = mapGithubPullRequestToProjectPullRequest(dto, {
    repoAddress: REPO_ADDRESS,
    cloneUrl: "https://github.com/acme/app",
  });
  assert.equal(githubPullRequestBranchLabel(same), "feature → develop");
  const fork = mapGithubPullRequestToProjectPullRequest(
    {
      ...dto,
      head: { ...dto.head, repo: { full_name: "other/app" } },
    },
    { repoAddress: REPO_ADDRESS, cloneUrl: "https://github.com/acme/app" },
  );
  assert.equal(githubPullRequestBranchLabel(fork), "other:feature → develop");
  const deleted = mapGithubPullRequestToProjectPullRequest(
    {
      ...dto,
      head: { ...dto.head, repo: { full_name: "" } },
    },
    { repoAddress: REPO_ADDRESS, cloneUrl: "https://github.com/acme/app" },
  );
  assert.equal(githubPullRequestBranchLabel(deleted), "feature → develop");
  assert.equal(
    githubRepoFullNameFromCloneUrl("git@github.com:acme/app.git"),
    "acme/app",
  );
  assert.equal(
    githubRepoFullNameFromCloneUrl("https://github.com/acme/app/extra"),
    null,
  );
});

test("conversation badge uses the greater of list count and loaded comments", () => {
  assert.equal(githubPullRequestConversationCount({ commentCount: 3, commentsLength: 0 }), 3);
  assert.equal(githubPullRequestConversationCount({ commentCount: 3, commentsLength: 5 }), 5);
});
```

Add these share-link cases to `projectShareLinks.test.mjs`.

```js
test("pullRequestShareLink accepts only a canonical GitHub pull URL", () => {
  const base = {
    id: "42",
    repoAddress: REPO_ADDRESS,
    cloneUrls: ["https://github.com/acme/app"],
  };
  assert.equal(
    pullRequestShareLink({
      ...base,
      htmlUrl: "https://github.com/acme/app/pull/42",
    }),
    "https://github.com/acme/app/pull/42",
  );
  for (const htmlUrl of [
    "https://evil.example/acme/app/pull/42",
    "https://github.com/acme/app/pull/42?x=1",
    "https://github.com/acme/app/pull/42#x",
    "https://github.com/acme/app/pull/42/",
    "https://github.com/acme/app/issues/42",
    "https://github.com/acme/app/pull/43",
    "https://github.com/acme/other/pull/42",
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

- [ ] **Step 3: Run and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/projectPullRequests.test.mjs src/features/projects/lib/projectGithubPulls.test.mjs src/features/projects/lib/projectShareLinks.test.mjs
```

Expected: the new module import or `authorAvatarUrl` is missing.

- [ ] **Step 4: Add exact DTOs and invoke wrappers**

Add these types and documented functions to `projectGit.ts`.

```ts
/** GitHub login identity returned by the native pull-request commands. */
export type GithubPullRequestUserDto = { login: string; avatar_url: string };

/** Nested repository identity on a GitHub pull-request head or base. */
export type GithubPullRequestRepoDto = { full_name: string };

/** Bounded GitHub pull request returned by the native commands. */
export type GithubPullRequestDto = {
  number: number;
  title: string;
  body: string;
  html_url: string;
  draft: boolean;
  comments: number;
  created_at: number;
  updated_at: number;
  user: GithubPullRequestUserDto;
  head: { ref: string; sha: string; repo: GithubPullRequestRepoDto };
  base: { ref: string; repo: GithubPullRequestRepoDto };
};

/** One bounded GitHub pull-request page. */
export type GithubPullRequestListDto = {
  pulls: GithubPullRequestDto[];
  has_more: boolean;
};

/** One read-only GitHub pull-request conversation comment. */
export type GithubPullRequestCommentDto = {
  id: number;
  body: string;
  html_url: string;
  created_at: number;
  user: GithubPullRequestUserDto;
};

/** List the first open GitHub pull-request page for a github.com clone URL. */
export async function listGithubPullRequests(input: {
  cloneUrl: string;
}): Promise<GithubPullRequestListDto> {
  try {
    return await invokeTauri<GithubPullRequestListDto>(
      "list_github_pull_requests",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** Create one ready GitHub pull request for a github.com clone URL. */
export async function createGithubPullRequest(input: {
  cloneUrl: string;
  title: string;
  body: string;
  head: string;
  base: string;
}): Promise<GithubPullRequestDto> {
  try {
    return await invokeTauri<GithubPullRequestDto>(
      "create_github_pull_request",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** List the first read-only conversation comment page for one GitHub pull request. */
export async function listGithubPullRequestComments(input: {
  cloneUrl: string;
  number: number;
}): Promise<GithubPullRequestCommentDto[]> {
  try {
    return await invokeTauri<GithubPullRequestCommentDto[]>(
      "list_github_pull_request_comments",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}
```

- [ ] **Step 5: Align the runtime object and its declaration file**

Add these fields to `ProjectPullRequest` in `projectPullRequests.d.mts`.

```ts
/** GitHub author avatar; null for Nostr pull requests. */
authorAvatarUrl: string | null;
/** GitHub head repository full name; null for Nostr and deleted-fork heads. */
headRepoFullName: string | null;
/** Validated canonical GitHub URL; null for Nostr pull requests. */
htmlUrl: string | null;
/** Backend-reported conversation count before lazy comments load. */
commentCount: number;
```

Add documented field `authorAvatarUrl?: string | null` to `ProjectPullRequestComment` with the comment `/** GitHub comment avatar; null or absent for Nostr comments. */`.
Insert these four properties in the object returned by `eventToProjectPullRequest` without changing the existing property expressions.

```js
authorAvatarUrl: null,
headRepoFullName: null,
htmlUrl: null,
commentCount: comments.length,
```

Set `authorAvatarUrl: null` on each Nostr comment object.

- [ ] **Step 6: Implement host routing and DTO mapping**

Create `projectGithubPulls.ts` with these exact public functions.

```ts
import { useQuery } from "@tanstack/react-query";

import type { ProjectPullRequest } from "@/features/projects/projectPullRequests.mjs";
import type { Repository } from "@/features/projects/projectModels";
import {
  listGithubPullRequestComments,
  type GithubPullRequestCommentDto,
  type GithubPullRequestDto,
  type GithubPullRequestListDto,
} from "@/shared/api/projectGit";
import { isGitHubCloneUrl } from "./projectGitError";

const HEX64_RE = /^[a-fA-F0-9]{64}$/;

/** Host-routed pull-request list consumed by the repository Pull Request tab. */
export type ProjectPullRequestsResult = {
  pullRequests: ProjectPullRequest[];
  hasMore: boolean;
};

/** Convert a positive safe GitHub pull-request number into its decimal selection id. */
export function githubPullRequestId(number: number): string {
  if (!Number.isSafeInteger(number) || number <= 0) {
    throw new Error("GitHub returned an invalid pull request number.");
  }
  return String(number);
}

/** Parse a positive GitHub pull-request number that is safe in JavaScript. */
export function parseGithubPullRequestNumber(
  value: string | null | undefined,
): number | null {
  if (!value || !/^[1-9][0-9]*$/.test(value)) return null;
  const number = Number(value);
  return Number.isSafeInteger(number) ? number : null;
}

/** Display a full GitHub number or the existing eight-character Nostr prefix. */
export function pullRequestDisplayNumber(pullRequestId: string): string {
  return parseGithubPullRequestNumber(pullRequestId) === null
    ? pullRequestId.slice(0, 8)
    : pullRequestId;
}

/** Map a bounded native GitHub pull request onto the shared Projects model. */
export function mapGithubPullRequestToProjectPullRequest(
  dto: GithubPullRequestDto,
  input: { repoAddress: string; cloneUrl: string },
): ProjectPullRequest {
  return {
    id: githubPullRequestId(dto.number),
    title: dto.title,
    content: dto.body ?? "",
    tags: [],
    author: dto.user.login,
    authorAvatarUrl: dto.user.avatar_url,
    createdAt: dto.created_at,
    repoAddress: input.repoAddress,
    channelId: null,
    originAgentName: null,
    labels: [],
    recipients: [],
    reviewers: [],
    approvals: [],
    changeRequests: [],
    status: dto.draft ? "Draft" : "Open",
    statusEventId: null,
    statusCreatedAt: null,
    branchName: dto.head.ref,
    targetBranch: dto.base.ref,
    initialCommit: dto.head.sha,
    commit: dto.head.sha,
    cloneUrls: [input.cloneUrl],
    updateCount: 0,
    updatedAt: dto.updated_at,
    updates: [],
    comments: [],
    commentCount: dto.comments,
    headRepoFullName: dto.head.repo.full_name || null,
    htmlUrl: dto.html_url,
  };
}

/** Map a bounded native GitHub comment without interpreting its login as a pubkey. */
export function mapGithubCommentToProjectPullRequestComment(
  dto: GithubPullRequestCommentDto,
): ProjectPullRequest["comments"][number] {
  return {
    id: String(dto.id),
    content: dto.body ?? "",
    tags: [],
    author: dto.user.login,
    authorAvatarUrl: dto.user.avatar_url,
    createdAt: dto.created_at,
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
  };
}

/** Route one repository to exactly one pull-request backend. */
export async function fetchProjectPullRequestsWith(
  project: Pick<Repository, "id" | "repoAddress" | "cloneUrls">,
  loaders: {
    loadGithub: (input: { cloneUrl: string }) => Promise<GithubPullRequestListDto>;
    loadBuzz: () => Promise<ProjectPullRequest[]>;
  },
): Promise<ProjectPullRequestsResult> {
  const cloneUrl = project.cloneUrls[0] ?? "";
  if (isGitHubCloneUrl(cloneUrl)) {
    const page = await loaders.loadGithub({ cloneUrl });
    return {
      pullRequests: page.pulls.map((pull) =>
        mapGithubPullRequestToProjectPullRequest(pull, {
          repoAddress: project.repoAddress,
          cloneUrl,
        }),
      ),
      hasMore: page.has_more === true,
    };
  }
  return { pullRequests: await loaders.loadBuzz(), hasMore: false };
}

/** Collect only valid Nostr identities for profile batch lookup. */
export function pullRequestIdentityPubkeys(
  pullRequests: ProjectPullRequest[],
): string[] {
  const values = pullRequests.flatMap((pullRequest) => [
    pullRequest.author,
    ...pullRequest.recipients,
    ...pullRequest.reviewers,
    ...pullRequest.updates.map((update) => update.author),
    ...pullRequest.comments.map((comment) => comment.author),
    ...pullRequest.approvals.map((approval) => approval.author),
  ]);
  return [
    ...new Set(
      values
        .filter((value) => HEX64_RE.test(value))
        .map((value) => value.toLowerCase()),
    ),
  ];
}

/** Render `owner:branch → base` only when the head repo is a different non-empty name. */
export function githubPullRequestBranchLabel(
  pullRequest: Pick<
    ProjectPullRequest,
    "branchName" | "targetBranch" | "headRepoFullName" | "cloneUrls"
  >,
): string {
  const head = pullRequest.branchName ?? "";
  const base = pullRequest.targetBranch ?? "";
  const headRepo = pullRequest.headRepoFullName?.trim() ?? "";
  if (!headRepo) return `${head} → ${base}`;
  const targetRepo = githubRepoFullNameFromCloneUrl(pullRequest.cloneUrls[0] ?? "");
  if (!targetRepo || headRepo.toLowerCase() === targetRepo.toLowerCase()) {
    return `${head} → ${base}`;
  }
  const owner = headRepo.split("/")[0] ?? headRepo;
  return `${owner}:${head} → ${base}`;
}

/** Parse the target owner/repository from a supported github.com clone URL. */
export function githubRepoFullNameFromCloneUrl(cloneUrl: string): string | null {
  const ssh = cloneUrl.match(/^git@github\.com:([^/]+)\/(.+?)(?:\.git)?$/i);
  if (ssh) return `${ssh[1]}/${ssh[2]}`;
  try {
    const url = new URL(cloneUrl);
    if (
      url.protocol !== "https:" ||
      url.hostname.toLowerCase() !== "github.com" ||
      url.port !== "" ||
      url.username !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== ""
    ) {
      return null;
    }
    const segments = url.pathname.split("/").filter(Boolean);
    if (segments.length !== 2) return null;
    const [owner, rawRepo] = segments;
    const repo = rawRepo?.replace(/\.git$/, "");
    return owner && repo ? `${owner}/${repo}` : null;
  } catch {
    return null;
  }
}

/** Conversation badge: list count until comments load, then the greater value. */
export function githubPullRequestConversationCount(input: {
  commentCount: number;
  commentsLength: number;
}): number {
  return Math.max(input.commentCount, input.commentsLength);
}

/** Keep #N until the destination list fetch settles without it. */
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

/** Refuse a GitHub decimal id on a Nostr-only write path. */
export function requireBuzzPullRequestEventId(id: string): string {
  if (!HEX64_RE.test(id)) {
    throw new Error("This GitHub pull request cannot be mutated through Nostr.");
  }
  return id;
}

/** Resolve a valid GitHub comment request and its cache key. */
export function githubPullRequestCommentsRequest(
  project: Pick<Repository, "id" | "cloneUrls"> | null | undefined,
  selectedPullRequestId: string | null | undefined,
): {
  cloneUrl: string;
  number: number;
  queryKey: readonly ["project", string, "pull-requests", number, "comments"];
} | null {
  const cloneUrl = project?.cloneUrls[0] ?? "";
  const number = parseGithubPullRequestNumber(selectedPullRequestId);
  if (!project || !isGitHubCloneUrl(cloneUrl) || number === null) return null;
  return {
    cloneUrl,
    number,
    queryKey: ["project", project.id, "pull-requests", number, "comments"],
  };
}

/** Load the first read-only GitHub conversation comment page for the selected number. */
export function useGithubPullRequestCommentsQuery(
  project: Pick<Repository, "id" | "cloneUrls"> | null | undefined,
  selectedPullRequestId: string | null | undefined,
) {
  const request = githubPullRequestCommentsRequest(project, selectedPullRequestId);
  return useQuery({
    enabled: request !== null,
    queryKey: request?.queryKey ?? [
      "project",
      project?.id ?? "none",
      "pull-requests",
      "none",
      "comments",
    ],
    queryFn: async () => {
      if (!request) throw new Error("No GitHub pull request selected.");
      const comments = await listGithubPullRequestComments({
        cloneUrl: request.cloneUrl,
        number: request.number,
      });
      return comments.map(mapGithubCommentToProjectPullRequestComment);
    },
    staleTime: 30_000,
  });
}
```

Add tests for `githubPullRequestCommentsRequest` and `selectedGithubPullRequestAfterListLoad` in the same test file.

```js
test("comment request is enabled only for GitHub numeric ids", () => {
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
      "e".repeat(64),
    ),
    null,
  );
});

test("selection survives refetch and clears when the number is absent", () => {
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
      pullRequestIds: ["42"],
      isSuccess: true,
      isFetching: false,
    }),
    null,
  );
});
```

- [ ] **Step 7: Implement strict GitHub URL sharing**

Import `githubRepoFullNameFromCloneUrl` from `./projectGithubPulls` in `projectShareLinks.ts` and use that one parser for both fork labels and share-link repository binding.

```ts
import { githubRepoFullNameFromCloneUrl } from "./projectGithubPulls";

function isSafeGitHubPullUrl(
  raw: string,
  pullRequest: Pick<ProjectPullRequest, "cloneUrls" | "id">,
): boolean {
  try {
    if (
      raw !== raw.trim() ||
      !raw.startsWith("https://github.com/") ||
      raw.endsWith("/") ||
      raw.includes("\\")
    ) {
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
      url.hash !== "" ||
      url.pathname.includes("%") ||
      url.pathname.includes("//")
    ) {
      return false;
    }
    const [owner, repo, segment, number, ...rest] = url.pathname
      .split("/")
      .filter(Boolean);
    const targetRepo = githubRepoFullNameFromCloneUrl(
      pullRequest.cloneUrls[0] ?? "",
    );
    return (
      rest.length === 0 &&
      segment === "pull" &&
      /^[A-Za-z0-9-]+$/.test(owner ?? "") &&
      /^[A-Za-z0-9._-]+$/.test(repo ?? "") &&
      /^[1-9][0-9]*$/.test(number ?? "") &&
      number === pullRequest.id &&
      targetRepo?.toLowerCase() === `${owner}/${repo}`.toLowerCase() &&
      raw === `https://github.com/${owner}/${repo}/pull/${number}`
    );
  } catch {
    return false;
  }
}

export function pullRequestShareLink(
  pullRequest: ProjectPullRequest,
): string | null {
  if (
    pullRequest.htmlUrl &&
    isSafeGitHubPullUrl(pullRequest.htmlUrl, pullRequest)
  ) {
    return pullRequest.htmlUrl;
  }
  const coordinate = repositoryCoordinate(pullRequest.repoAddress);
  return coordinate &&
    HEX64_RE.test(pullRequest.id) &&
    isLinkableCoordinate(coordinate.owner, coordinate.dtag)
    ? buildPullRequestLink({ ...coordinate, id: pullRequest.id })
    : null;
}
```

- [ ] **Step 8: Run tests, typecheck, and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/projectPullRequests.test.mjs src/features/projects/lib/projectGithubPulls.test.mjs src/features/projects/lib/projectShareLinks.test.mjs && pnpm typecheck
```

Expected: all tests pass and TypeScript accepts the `.mjs` declaration changes.

- [ ] **Step 9: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/shared/api/projectGit.ts \
  src/features/projects/projectPullRequests.mjs \
  src/features/projects/projectPullRequests.d.mts \
  src/features/projects/projectPullRequests.test.mjs \
  src/features/projects/lib/projectGithubPulls.ts \
  src/features/projects/lib/projectGithubPulls.test.mjs \
  src/features/projects/lib/projectShareLinks.ts \
  src/features/projects/lib/projectShareLinks.test.mjs
git add src/shared/api/projectGit.ts \
  src/features/projects/projectPullRequests.mjs \
  src/features/projects/projectPullRequests.d.mts \
  src/features/projects/projectPullRequests.test.mjs \
  src/features/projects/lib/projectGithubPulls.ts \
  src/features/projects/lib/projectGithubPulls.test.mjs \
  src/features/projects/lib/projectShareLinks.ts \
  src/features/projects/lib/projectShareLinks.test.mjs
git diff --check
git commit -s -m "feat(projects): map GitHub pull requests onto ProjectPullRequest"
```

---

### Task 5: Route the query, create mutation, invalidations, comments, and profile lookup

**Files:**

- Modify: `desktop/src/features/projects/hooks.ts`
- Modify: `desktop/src/features/projects/pullRequestMutations.ts`
- Modify: `desktop/src/features/projects/pullRequestReviews.ts`
- Create: `desktop/src/features/projects/pullRequestMutations.github.test.mjs`
- Modify: `desktop/src/features/projects/ui/CreatePullRequestDialog.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`

**Interfaces:**

- Consumes: `fetchProjectPullRequestsWith`, native list/create wrappers, number parsing, identity filtering, and `requireBuzzPullRequestEventId`
- Produces: `useProjectPullRequestsQuery` returning `{ pullRequests, hasMore }`, `createProjectPullRequestWith`, and `projectPullRequestInvalidationKeys`

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `fetchProjectPullRequests`, `useProjectPullRequestsQuery`, `createProjectPullRequestComment`, `publishProjectPullRequest`, `useCreateProjectPullRequestMutation`, `useMergeProjectPullRequestMutation`, `updateProjectPullRequestStatus`, `requestProjectPullRequestReview`, `submitProjectPullRequestReview`, `CreatePullRequestDialog`, and `ProjectDetailScreen`.

- [ ] **Step 2: Write failing create-routing and invalidation tests**

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  createProjectPullRequestWith,
  projectPullRequestInvalidationKeys,
  publishProjectPullRequestUpdate,
} from "./pullRequestMutations.ts";

const REPO_ADDRESS = `30617:${"a".repeat(64)}:app`;
const input = {
  title: "Fix login",
  body: "steps",
  branch: "feature",
  targetBranch: "develop",
  commit: "d".repeat(40),
  mergeBase: null,
  reviewers: [],
};

test("GitHub create never calls the Buzz pull-request publisher", async () => {
  const calls = { github: 0, buzz: 0 };
  const id = await createProjectPullRequestWith(
    {
      id: "p1",
      owner: "a".repeat(64),
      repoAddress: REPO_ADDRESS,
      cloneUrls: ["https://github.com/acme/app"],
    },
    input,
    {
      createGithub: async (payload) => {
        calls.github += 1;
        assert.deepEqual(payload, {
          cloneUrl: "https://github.com/acme/app",
          title: "Fix login",
          body: "steps",
          head: "feature",
          base: "develop",
        });
        return { number: 43 };
      },
      publishBuzz: async () => {
        calls.buzz += 1;
        return "e".repeat(64);
      },
    },
  );
  assert.equal(id, "43");
  assert.deepEqual(calls, { github: 1, buzz: 0 });
});

test("GitHub create rejects an unsafe native pull-request number", async () => {
  await assert.rejects(
    createProjectPullRequestWith(
      {
        id: "p1",
        owner: "a".repeat(64),
        repoAddress: REPO_ADDRESS,
        cloneUrls: ["https://github.com/acme/app"],
      },
      input,
      {
        createGithub: async () => ({ number: Number.MAX_SAFE_INTEGER + 1 }),
        publishBuzz: async () => "e".repeat(64),
      },
    ),
    /GitHub returned an invalid pull request number/,
  );
});

test("Buzz create never calls the GitHub creator", async () => {
  const calls = { github: 0, buzz: 0 };
  const id = await createProjectPullRequestWith(
    {
      id: "p2",
      owner: "a".repeat(64),
      repoAddress: REPO_ADDRESS,
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
    },
    input,
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
  assert.deepEqual(calls, { github: 0, buzz: 1 });
});

test("GitHub create invalidates only its repository pull-request query", () => {
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
    projectPullRequestInvalidationKeys({
      id: "p2",
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
    }),
    [
      ["project", "p2", "pull-requests"],
      ["projects", "work-items"],
      ["projects", "activity-summaries"],
    ],
  );
});

test("a decimal GitHub id is rejected before a Nostr update reaches identity or signing", async () => {
  await assert.rejects(
    publishProjectPullRequestUpdate({
      commit: "e".repeat(40),
      mergeBase: null,
      project: {
        owner: "a".repeat(64),
        repoAddress: REPO_ADDRESS,
        cloneUrls: ["https://github.com/acme/app"],
      },
      pullRequest: {
        id: "42",
        commit: "d".repeat(40),
      },
    }),
    /cannot be mutated through Nostr/,
  );
});
```

- [ ] **Step 3: Run and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/pullRequestMutations.github.test.mjs
```

Expected: `createProjectPullRequestWith` is missing.

- [ ] **Step 4: Implement routing**

Rename the existing `fetchProjectPullRequests` function in `hooks.ts` to `fetchBuzzProjectPullRequests` and keep its kind:1618 / 1613 / 163x / kind:1 fetches unchanged.

```ts
export function useProjectPullRequestsQuery(
  project: Repository | null | undefined,
) {
  return useQuery({
    enabled: Boolean(project),
    queryKey: ["project", project?.id ?? "none", "pull-requests"],
    queryFn: () => {
      if (!project) throw new Error("No project selected.");
      return fetchProjectPullRequestsWith(project, {
        loadGithub: listGithubPullRequests,
        loadBuzz: () => fetchBuzzProjectPullRequests(project),
      });
    },
    staleTime: 30_000,
  });
}
```

Add `listGithubPullRequests` to the `projectGit` import in `hooks.ts`.
Add `fetchProjectPullRequestsWith` to the `projectGithubPulls` import.
Do not add more than a few lines; `hooks.ts` is already 947 lines.

In `pullRequestMutations.ts`:

```ts
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createGithubPullRequest } from "@/shared/api/projectGit";
import { isGitHubCloneUrl } from "@/features/projects/lib/projectGitError";
import {
  githubPullRequestId,
  requireBuzzPullRequestEventId,
} from "@/features/projects/lib/projectGithubPulls";

/** Route pull-request creation to exactly one backend for the repository host. */
export async function createProjectPullRequestWith(
  project: Project,
  input: CreateProjectPullRequestInput,
  loaders: {
    createGithub: (input: {
      cloneUrl: string;
      title: string;
      body: string;
      head: string;
      base: string;
    }) => Promise<{ number: number }>;
    publishBuzz: typeof publishProjectPullRequest;
  },
): Promise<string> {
  const cloneUrl = project.cloneUrls[0] ?? "";
  if (isGitHubCloneUrl(cloneUrl)) {
    const pull = await loaders.createGithub({
      cloneUrl,
      title: input.title,
      body: input.body,
      head: input.branch,
      base: input.targetBranch,
    });
    return githubPullRequestId(pull.number);
  }
  return loaders.publishBuzz(project, input);
}

/** Query keys invalidated after a host-routed pull-request create. */
export function projectPullRequestInvalidationKeys(
  project: Pick<Project, "id" | "cloneUrls">,
): readonly unknown[][] {
  if (isGitHubCloneUrl(project.cloneUrls[0])) {
    return [["project", project.id, "pull-requests"]];
  }
  return [
    ["project", project.id, "pull-requests"],
    ["projects", "work-items"],
    ["projects", "activity-summaries"],
  ];
}

export function useCreateProjectPullRequestMutation(
  project: Project | null | undefined,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateProjectPullRequestInput) => {
      if (!project) throw new Error("No project selected.");
      return createProjectPullRequestWith(project, input, {
        createGithub: createGithubPullRequest,
        publishBuzz: publishProjectPullRequest,
      });
    },
    onSuccess: async () => {
      if (!project) return;
      await Promise.all(
        projectPullRequestInvalidationKeys(project).map((queryKey) =>
          queryClient.invalidateQueries({ queryKey }),
        ),
      );
    },
  });
}
```

At the first executable line of `publishProjectPullRequestUpdate`, call `requireBuzzPullRequestEventId(pullRequest.id)` before the unchanged-commit early return or `getIdentity`.
In `useMergeProjectPullRequestMutation`, call `requireBuzzPullRequestEventId(pullRequest.id)` before branch validation and before `mergeProjectPullRequest`.
Import `requireBuzzPullRequestEventId` into `hooks.ts` and call it at the first executable line of `createProjectPullRequestComment`, before body validation, identity normalization, signing, or publishing.
Import the same helper into `pullRequestReviews.ts` and call it at the first executable line of `updateProjectPullRequestStatus`, `requestProjectPullRequestReview`, and `submitProjectPullRequestReview`.
These guards are defensive write isolation; the GitHub detail path still must not mount any of those mutations.

```ts
requireBuzzPullRequestEventId(pullRequest.id);
```

In `CreatePullRequestDialog.tsx` replace the array read.

```ts
const pullRequests = pullRequestsQuery.data?.pullRequests ?? [];
const hasOpenPullRequest = pullRequests.some(
  (pullRequest) =>
    (pullRequest.status === "Open" || pullRequest.status === "Draft") &&
    pullRequest.branchName === sourceBranch &&
    (pullRequest.targetBranch ?? repository?.defaultBranch) === targetBranch,
);
```

In `ProjectDetailScreen.tsx` introduce one stable local array immediately after the query and replace every `pullRequestsQuery.data` array use.

```ts
const pullRequests = React.useMemo(
  () => pullRequestsQuery.data?.pullRequests ?? [],
  [pullRequestsQuery.data?.pullRequests],
);
```

Use `pullRequests` for branch options, selected-branch matching, `hasOpenPullRequest`, `selectedPullRequest`, and the `pullRequests={pullRequests}` prop.
Replace the people-pubkey flatMap with `pullRequestIdentityPubkeys(pullRequests)`.
Replace memo dependency entries that used `pullRequestsQuery.data` with `pullRequests`.
In `handlePullRequestCreated`, keep calling `pullRequestsQuery.refetch()` but replace the whole `pullRequestsQuery` dependency with the stable `pullRequestsQuery.refetch` method.
Do not add any React Query result object to a dependency array.

Keep GitHub rows out of Nostr-only update and diff paths in `ProjectDetailScreen.tsx`.

```ts
const repoDiffQuery = useProjectRepoDiffQuery(
  repository,
  activeBranch,
  activeRepoPullRequest,
  repoSource === "remote" && !githubHosted,
);
const localRepoDiffQuery = useProjectLocalRepoDiffQuery(
  repository,
  activeCommunity?.reposDir,
  activeBranch,
  activeRepoPullRequest,
  repoSource === "local" && Boolean(activeRepoPullRequest) && !githubHosted,
);
```

Gate `updatePullRequestAction` with `!githubHosted` before `openBranchPullRequest` so GitHub branches never render the Nostr Update PR action.

```tsx
updatePullRequestAction={
  !githubHosted && openBranchPullRequest?.commit
    ? {
        onUpdate: () => {
          void handleUpdatePullRequest();
        },
        pending: updatePullRequestMutation.isPending,
      }
    : undefined
}
```

- [ ] **Step 5: Run tests and typecheck and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/pullRequestMutations.github.test.mjs src/features/projects/pullRequestMutations.test.mjs src/features/projects/lib/projectGithubPulls.test.mjs && pnpm typecheck && pnpm check:file-sizes
```

Expected: routing and decimal-id guard tests pass, existing tag tests stay green, TypeScript accepts the new query shape, no GitHub PR starts a `refs/nostr/{number}` diff, and no file exceeds 1,000 lines.

- [ ] **Step 6: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/features/projects/hooks.ts \
  src/features/projects/pullRequestMutations.ts \
  src/features/projects/pullRequestReviews.ts \
  src/features/projects/pullRequestMutations.github.test.mjs \
  src/features/projects/ui/CreatePullRequestDialog.tsx \
  src/features/projects/ui/ProjectDetailScreen.tsx
git add src/features/projects/hooks.ts \
  src/features/projects/pullRequestMutations.ts \
  src/features/projects/pullRequestReviews.ts \
  src/features/projects/pullRequestMutations.github.test.mjs \
  src/features/projects/ui/CreatePullRequestDialog.tsx \
  src/features/projects/ui/ProjectDetailScreen.tsx
git diff --check
git commit -s -m "feat(projects): route GitHub pull request list and create"
```

---

### Task 6: Render GitHub list and read-only detail chrome

**Files:**

- Create: `desktop/src/features/projects/ui/GitHubProjectPullRequests.tsx`
- Create: `desktop/src/features/projects/ui/GitHubProjectPullRequests.test.mjs`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabList.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` only to pass the exact query-state props described below
- Do not modify `desktop/src/features/projects/ui/ProjectPullRequestsPanel.tsx`.

**Interfaces:**

- Consumes: `GitHubLoginIdentity`, `GitHubRepoStateRecovery`, `useGithubPullRequestCommentsQuery`, `pullRequestShareLink`, `pullRequestDisplayNumber`, `githubPullRequestBranchLabel`, `githubPullRequestConversationCount`, and `selectedGithubPullRequestAfterListLoad`
- Produces: exported `GitHubPullRequestRow`, `GitHubPullRequestDetail`, `GitHubPullRequestDetailHeader`, `GitHubPullRequestMetaRail`, and `GitHubPullRequestsPanel`, plus module-private `GitHubPullRequestDetailShell`

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `WorkspaceTabs`, `PullRequestTabsList`, `PullRequestsPanel`, `PullRequestDetailHeader`, `PullRequestMetaRail`, and `ProjectDetailScreen`.

- [ ] **Step 2: Write the failing server-rendered component contract**

Create `GitHubProjectPullRequests.test.mjs` before creating the component file or changing `PullRequestTabsList`.

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import React from "react";
import { renderToStaticMarkup } from "react-dom/server";

import { Tabs } from "@/shared/ui/tabs";
import {
  GitHubPullRequestDetail,
  GitHubPullRequestDetailHeader,
  GitHubPullRequestMetaRail,
  GitHubPullRequestRow,
} from "./GitHubProjectPullRequests.tsx";
import { PullRequestTabsList } from "./ProjectWorkspaceTabList.tsx";

const pullRequest = {
  id: "42",
  title: "Fix login",
  content: "PR body from GitHub",
  tags: [],
  author: "ada",
  authorAvatarUrl: "https://avatars.githubusercontent.com/u/1",
  createdAt: 1_704_166_645,
  repoAddress: `30617:${"a".repeat(64)}:app`,
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
  branchName: "feature",
  targetBranch: "develop",
  initialCommit: "d".repeat(40),
  commit: "d".repeat(40),
  cloneUrls: ["https://github.com/acme/app"],
  updateCount: 0,
  updatedAt: 1_704_253_045,
  updates: [],
  comments: [],
  commentCount: 3,
  headRepoFullName: "acme/app",
  htmlUrl: "https://github.com/acme/app/pull/42",
};

const githubComment = {
  id: "9",
  content: "Looks good.",
  tags: [],
  author: "grace",
  authorAvatarUrl: "https://avatars.githubusercontent.com/u/3",
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
};

test("GitHub row renders #N, login, host-native branches, and status", () => {
  const html = renderToStaticMarkup(
    React.createElement(GitHubPullRequestRow, {
      onOpen() {},
      pullRequest,
    }),
  );
  assert.match(html, /#42/);
  assert.match(html, /ada/);
  assert.match(html, /feature/);
  assert.match(html, /develop/);
  assert.match(html, /Open/);
});

test("GitHub conversation renders body and login comments without write chrome", () => {
  const commentsQuery = {
    data: [githubComment],
    error: null,
    isError: false,
    isLoading: false,
    refetch: async () => {},
  };
  const html = renderToStaticMarkup(
    React.createElement(
      React.Fragment,
      null,
      React.createElement(GitHubPullRequestDetailHeader, { pullRequest }),
      React.createElement(GitHubPullRequestDetail, {
        commentsQuery,
        mode: "conversation",
        onSelectedPullRequestIdChange() {},
        pullRequest,
      }),
      React.createElement(GitHubPullRequestMetaRail, { pullRequest }),
    ),
  );
  assert.match(html, /PR body from GitHub/);
  assert.match(html, /Looks good\./);
  assert.match(html, /grace/);
  for (const forbidden of [
    "Merge",
    "Request changes",
    "Reviewers",
    "Discussed in channels",
    "Add a comment",
  ]) {
    assert.equal(html.includes(forbidden), false, forbidden);
  }
});

test("GitHub pull tabs hide Files changed and pin one commit", () => {
  const html = renderToStaticMarkup(
    React.createElement(
      Tabs,
      { defaultValue: "pr-conversation" },
      React.createElement(PullRequestTabsList, {
        conversationCount: 5,
        filesCount: 99,
        hideFiles: true,
        pullRequest,
      }),
    ),
  );
  assert.match(html, /Conversation/);
  assert.match(html, /Commits/);
  assert.equal(html.includes("Files changed"), false);
  assert.ok(html.includes(">5<"), "conversation badge uses the supplied count");
  assert.ok(html.includes(">1<"), "GitHub commit badge is exactly one");
});
```

- [ ] **Step 3: Run the component contract and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/ui/GitHubProjectPullRequests.test.mjs
```

Expected: FAIL because `GitHubProjectPullRequests.tsx` does not exist; after adding an empty module, the tabs assertion must still fail because `hideFiles` is not implemented.

- [ ] **Step 4: Extend `PullRequestTabsList` minimally**

Change the component to accept optional `hideFiles` and `conversationCount`.

```tsx
export function PullRequestTabsList({
  conversationCount,
  filesCount,
  hideFiles = false,
  pullRequest,
}: {
  conversationCount?: number;
  filesCount: number;
  hideFiles?: boolean;
  pullRequest: ProjectPullRequest;
}) {
  const commitCount = hideFiles ? 1 : Math.max(1, pullRequest.updateCount + 1);
  const comments =
    conversationCount ??
    pullRequest.commentCount ??
    pullRequest.comments.length;
  return (
    <TabsList className="h-9 w-fit justify-start gap-6 bg-transparent p-0">
      <TabsTrigger className={PR_TAB_TRIGGER_CLASS} value="pr-conversation">
        Conversation
        <span className="rounded-full bg-muted px-1.5 py-0.5 text-2xs">
          {comments}
        </span>
      </TabsTrigger>
      <TabsTrigger className={PR_TAB_TRIGGER_CLASS} value="pr-commits">
        Commits
        <span className="rounded-full bg-muted px-1.5 py-0.5 text-2xs">
          {commitCount}
        </span>
      </TabsTrigger>
      <TabsTrigger className={PR_TAB_TRIGGER_CLASS} value="pr-checks">
        Checks
        <span className="rounded-full bg-muted px-1.5 py-0.5 text-2xs">0</span>
      </TabsTrigger>
      {hideFiles ? null : (
        <TabsTrigger className={PR_TAB_TRIGGER_CLASS} value="pr-files">
          Files changed
          <span className="rounded-full bg-muted px-1.5 py-0.5 text-2xs">
            {filesCount}
          </span>
        </TabsTrigger>
      )}
    </TabsList>
  );
}
```

- [ ] **Step 5: Implement `GitHubProjectPullRequests.tsx`**

Use `text-sm`, `text-xs`, and `text-2xs` only.
Reuse `GitHubLoginIdentity` and `ProjectIssueCommentTimeline` with `githubMode`.
Do not import `ProfileIdentityButton`, `normalizePubkey`, `MergePullRequestButton`, `PullRequestReviewCard`, `PullRequestReviewersRow`, or `ForumComposer`.

```tsx
import { Check, GitBranch, GitCommitHorizontal, GitPullRequest, MessageSquare, X } from "lucide-react";
import * as React from "react";
import { toast } from "sonner";

import type { ProjectPullRequest, Repository } from "@/features/projects/hooks";
import {
  githubPullRequestBranchLabel,
  githubPullRequestConversationCount,
  pullRequestDisplayNumber,
  selectedGithubPullRequestAfterListLoad,
  useGithubPullRequestCommentsQuery,
} from "@/features/projects/lib/projectGithubPulls";
import { pullRequestShareLink } from "@/features/projects/lib/projectShareLinks";
import {
  formatExactTimestamp,
  relativeTime,
} from "@/features/projects/lib/projectsViewHelpers";
import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";
import { GitHubLoginIdentity } from "./GitHubIssueIdentity";
import { GitHubRepoStateRecovery } from "./GitHubRepoStateRecovery";
import {
  ProjectFeedRow,
  ProjectFeedRowCluster,
  ProjectFeedRowMonoCell,
} from "./ProjectFeedRow";
import { ProjectIssueCommentTimeline } from "./ProjectIssueCommentTimeline";
import { OverviewRailSection } from "./ProjectOverviewPanel";
import { ProjectRichContent } from "./ProjectRichContent";
import { ShareLinkButton } from "./ShareLinkButton";

function githubPullStatusClassName(status: ProjectPullRequest["status"]) {
  return status === "Draft" ? "text-muted-foreground" : "text-green-500";
}

/** One GitHub pull-request list row with numeric identity and login metadata. */
export function GitHubPullRequestRow({
  onOpen,
  pullRequest,
}: {
  onOpen: () => void;
  pullRequest: ProjectPullRequest;
}) {
  const number = pullRequestDisplayNumber(pullRequest.id);
  const StatusIcon = pullRequest.status === "Draft" ? X : Check;
  const statusClassName = githubPullStatusClassName(pullRequest.status);
  return (
    <ProjectFeedRow
      eventId={pullRequest.id}
      meta={
        <>
          <GitHubLoginIdentity
            avatarUrl={pullRequest.authorAvatarUrl}
            login={pullRequest.author}
          />
          <span className="inline-flex min-w-0 items-center gap-1 rounded-full border border-border/60 px-1.5 py-0.5 font-mono text-2xs">
            <GitBranch className="h-3 w-3 shrink-0" />
            <span className="truncate">{githubPullRequestBranchLabel(pullRequest)}</span>
          </span>
          <span className={`rounded-full border border-border/60 px-1.5 py-0.5 text-2xs font-medium ${statusClassName}`}>
            {pullRequest.status}
          </span>
        </>
      }
      onOpen={onOpen}
      statusIcon={<StatusIcon className={`h-3.5 w-3.5 shrink-0 ${statusClassName}`} />}
      testId="project-github-pull-request-row"
      title={pullRequest.title}
      trailing={
        <>
          {pullRequest.commentCount > 0 ? (
            <span className="flex items-center gap-1 text-xs text-muted-foreground">
              <MessageSquare className="h-3.5 w-3.5" />
              {pullRequest.commentCount}
            </span>
          ) : null}
          <ProjectFeedRowCluster>
            <ProjectFeedRowMonoCell
              label={`#${number}`}
              onClick={onOpen}
              title={`View GitHub pull request #${number}`}
            />
          </ProjectFeedRowCluster>
          <span
            className="hidden w-20 shrink-0 text-right text-xs text-muted-foreground sm:block"
            title={formatExactTimestamp(pullRequest.createdAt)}
          >
            {relativeTime(pullRequest.createdAt)}
          </span>
        </>
      }
    />
  );
}

/** GitHub pull-request conversation, one-commit list, or empty checks state. */
export function GitHubPullRequestDetail({
  commentsQuery,
  mode,
  onSelectedPullRequestIdChange,
  pullRequest,
}: {
  commentsQuery: ReturnType<typeof useGithubPullRequestCommentsQuery>;
  mode: "conversation" | "commits" | "checks";
  onSelectedPullRequestIdChange: (id: string | null) => void;
  pullRequest: ProjectPullRequest;
}) {
  const number = pullRequestDisplayNumber(pullRequest.id);
  const comments = commentsQuery.data ?? [];
  const parsed = parseProjectPullRequestMergeError(commentsQuery.error);
  React.useEffect(() => {
    if (parsed?.code === "github_pr_unavailable") {
      toast.error("Pull request not found.");
      onSelectedPullRequestIdChange(null);
    }
  }, [onSelectedPullRequestIdChange, parsed?.code]);

  if (mode === "commits") {
    const short = (pullRequest.commit ?? "").slice(0, 7);
    return (
      <section>
        <header className="flex min-h-10 items-center gap-2 border-b border-border/50 bg-muted/20 px-4">
          <GitCommitHorizontal className="h-4 w-4 text-muted-foreground" />
          <h4 className="text-sm font-medium text-foreground">Commits</h4>
          <span className="rounded-full bg-muted px-1.5 py-0.5 text-2xs text-muted-foreground">1</span>
        </header>
        <div className="divide-y divide-border/50">
          <ProjectFeedRow
            meta={
              <>
                <GitHubLoginIdentity
                  avatarUrl={pullRequest.authorAvatarUrl}
                  login={pullRequest.author}
                  showLabel={false}
                />
                <span className="truncate">{pullRequest.author} authored</span>
              </>
            }
            testId="project-github-pull-request-commit-row"
            title={pullRequest.title}
            trailing={
              <ProjectFeedRowCluster>
                <ProjectFeedRowMonoCell label={short || "unknown"} title={pullRequest.commit ?? ""} />
              </ProjectFeedRowCluster>
            }
          />
        </div>
      </section>
    );
  }

  if (mode === "checks") {
    return (
      <p className="p-4 text-sm text-muted-foreground">
        No checks have been reported for this pull request yet.
      </p>
    );
  }

  return (
    <section className="space-y-3 p-4">
      {commentsQuery.isLoading ? (
        <p className="text-sm text-muted-foreground">Loading comments…</p>
      ) : commentsQuery.isError && parsed?.code !== "github_pr_unavailable" ? (
        <GitHubRepoStateRecovery
          error={commentsQuery.error}
          onRetry={() => void commentsQuery.refetch()}
          titleId="github-pull-request-comments-recovery-title"
          unavailableTitle="Could not load GitHub comments"
        />
      ) : (
        <ProjectIssueCommentTimeline comments={comments} githubMode key={pullRequest.id} />
      )}
      <p className="sr-only">{`GitHub pull request #${number}`}</p>
    </section>
  );
}

/** GitHub pull-request list, empty, and error states for the repository tab. */
export function GitHubPullRequestsPanel({
  error,
  hasMore,
  isFetching,
  isLoading,
  isSuccess,
  onRetry,
  onSelectedPullRequestIdChange,
  pullRequests,
  selectedPullRequestId,
}: {
  error: unknown;
  hasMore: boolean;
  isFetching: boolean;
  isLoading: boolean;
  isSuccess: boolean;
  onRetry: () => void | Promise<unknown>;
  onSelectedPullRequestIdChange: (id: string | null) => void;
  pullRequests: ProjectPullRequest[];
  selectedPullRequestId: string | null;
}) {
  React.useEffect(() => {
    const nextSelected = selectedGithubPullRequestAfterListLoad({
      selectedPullRequestId,
      pullRequestIds: pullRequests.map((pullRequest) => pullRequest.id),
      isSuccess,
      isFetching,
    });
    if (nextSelected !== selectedPullRequestId) {
      onSelectedPullRequestIdChange(nextSelected);
    }
  }, [
    isFetching,
    isSuccess,
    onSelectedPullRequestIdChange,
    pullRequests,
    selectedPullRequestId,
  ]);

  if (isLoading) {
    return <p className="p-4 text-sm text-muted-foreground">Loading pull requests…</p>;
  }
  if (error) {
    return (
      <div className="p-4">
        <GitHubRepoStateRecovery
          error={error}
          onRetry={onRetry}
          titleId="github-pull-requests-recovery-title"
          unavailableTitle="Could not load GitHub pull requests"
        />
      </div>
    );
  }
  if (pullRequests.length === 0) {
    return (
      <div className="space-y-2 p-4 text-sm text-muted-foreground">
        <p>No open pull requests.</p>
        {hasMore ? <p>More open pull requests exist on GitHub.</p> : null}
      </div>
    );
  }
  return (
    <div>
      <div className="divide-y divide-border/50">
        {pullRequests.map((pullRequest) => (
          <GitHubPullRequestRow
            key={pullRequest.id}
            onOpen={() => onSelectedPullRequestIdChange(pullRequest.id)}
            pullRequest={pullRequest}
          />
        ))}
      </div>
      {hasMore ? (
        <p className="border-t border-border/50 p-4 text-xs text-muted-foreground">
          More open pull requests exist on GitHub.
        </p>
      ) : null}
    </div>
  );
}

/** GitHub detail header: title, #N, login author, no origin or channel chrome. */
export function GitHubPullRequestDetailHeader({
  pullRequest,
}: {
  pullRequest: ProjectPullRequest;
}) {
  const number = pullRequestDisplayNumber(pullRequest.id);
  return (
    <header className="min-w-0 space-y-1 p-4 pb-4">
      <h3 className="line-clamp-2 min-w-0 text-xl font-semibold text-foreground">
        {pullRequest.title}{" "}
        <span className="font-normal text-muted-foreground">#{number}</span>
        <ShareLinkButton
          className="ml-1 inline-flex h-7 w-7 align-text-bottom"
          label="Copy pull request link"
          link={pullRequestShareLink(pullRequest)}
          testId="project-pull-request-copy-link"
        />
      </h3>
      <p className="flex flex-wrap items-center gap-x-1 gap-y-1 text-xs font-medium text-muted-foreground">
        <GitPullRequest className="h-3.5 w-3.5 shrink-0" />
        <GitHubLoginIdentity
          avatarUrl={pullRequest.authorAvatarUrl}
          login={pullRequest.author}
        />
        <span title={formatExactTimestamp(pullRequest.createdAt)}>
          created {relativeTime(pullRequest.createdAt)}
        </span>
      </p>
      {pullRequest.content ? (
        <ProjectRichContent className="pt-3" content={pullRequest.content} tags={[]} />
      ) : null}
    </header>
  );
}

/** GitHub meta rail: status, login author, branches. No reviewers. */
export function GitHubPullRequestMetaRail({
  pullRequest,
}: {
  pullRequest: ProjectPullRequest;
}) {
  return (
    <aside className="min-w-0 space-y-6 border-t border-border/60 p-4 xl:border-l xl:border-t-0">
      <OverviewRailSection title="Status">
        <span
          className={`inline-flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium text-white ${
            pullRequest.status === "Draft" ? "bg-muted-foreground/80" : "bg-green-600"
          }`}
        >
          <GitPullRequest className="h-3.5 w-3.5" />
          {pullRequest.status}
        </span>
      </OverviewRailSection>
      <OverviewRailSection title="Author">
        <GitHubLoginIdentity
          avatarUrl={pullRequest.authorAvatarUrl}
          login={pullRequest.author}
        />
      </OverviewRailSection>
      <OverviewRailSection title="Branches">
        <p className="font-mono text-xs text-muted-foreground">
          {githubPullRequestBranchLabel(pullRequest)}
        </p>
      </OverviewRailSection>
    </aside>
  );
}

```

- [ ] **Step 6: Host-split `WorkspaceTabs` with exact query-state props**

Add these required props to the `WorkspaceTabs` destructuring and parameter type.

```ts
pullRequestsHasMore: boolean;
pullRequestsFetching: boolean;
pullRequestsSuccess: boolean;
onRetryPullRequests: () => void | Promise<unknown>;
```

In `ProjectDetailScreen.tsx`, pass the exact values below.

```tsx
pullRequestsHasMore={pullRequestsQuery.data?.hasMore ?? false}
pullRequestsFetching={pullRequestsQuery.isFetching}
pullRequestsSuccess={pullRequestsQuery.isSuccess}
onRetryPullRequests={pullRequestsQuery.refetch}
```

Do not add defaults or optional query-state props.
`ProjectDetailScreen` is the only `WorkspaceTabs` caller, and required props ensure GitHub selection reconciliation cannot accidentally treat a failed or in-flight list as an empty successful list.

Add these imports in `ProjectWorkspaceTabs.tsx`.

```tsx
import {
  githubPullRequestConversationCount,
  useGithubPullRequestCommentsQuery,
} from "@/features/projects/lib/projectGithubPulls";
import {
  GitHubPullRequestDetail,
  GitHubPullRequestDetailHeader,
  GitHubPullRequestMetaRail,
  GitHubPullRequestsPanel,
} from "./GitHubProjectPullRequests";
```

Add this effect after the existing PR-tab effect:

```ts
React.useEffect(() => {
  if (githubHosted && selectedTab === "pr-files") {
    setSelectedTab("pr-conversation");
  }
}, [githubHosted, selectedTab]);
```

Replace the existing `commitAuthorPubkeys` declaration with these two declarations so GitHub logins stay out of the signed-event commit-author profile map.

```tsx
const nostrIdentityPullRequests = React.useMemo(
  () => (githubHosted ? [] : pullRequests),
  [githubHosted, pullRequests],
);
const commitAuthorPubkeys = React.useMemo(
  () => commitAuthorPubkeysFromPullRequests(nostrIdentityPullRequests),
  [nostrIdentityPullRequests],
);
```

Pass `pullRequests={nostrIdentityPullRequests}` to `ActivityPanel` so its internal commit-author map also receives only Nostr identities.
Keep the full `pullRequests` array for counts, PR selection, and the host-routed PR panels.

Add `GitHubPullRequestDetailShell` in `ProjectWorkspaceTabs.tsx`.
It owns `useGithubPullRequestCommentsQuery` so the conversation badge can use `max(commentCount, comments.length)` after comments load.

```tsx
/** GitHub-only detail shell that never mounts Buzz review, merge, or diff UI. */
function GitHubPullRequestDetailShell({
  onSelectedPullRequestIdChange,
  project,
  pullRequest,
}: {
  onSelectedPullRequestIdChange: (id: string | null) => void;
  project: Repository;
  pullRequest: ProjectPullRequest;
}) {
  const commentsQuery = useGithubPullRequestCommentsQuery(project, pullRequest.id);
  const conversationCount = githubPullRequestConversationCount({
    commentCount: pullRequest.commentCount,
    commentsLength: commentsQuery.data?.length ?? 0,
  });
  return (
    <div className={PROJECT_DETAIL_PANEL_CLASS} data-project-detail-panel>
      <div className="grid xl:grid-cols-[minmax(0,1fr)_18rem]">
        <div className="min-w-0">
          <GitHubPullRequestDetailHeader pullRequest={pullRequest} />
          <div className="border-b border-border/60 px-4">
            <PullRequestTabsList
              conversationCount={conversationCount}
              filesCount={0}
              hideFiles
              pullRequest={pullRequest}
            />
          </div>
          {(["conversation", "commits", "checks"] as const).map((mode) => (
            <TabsContent className="m-0" key={mode} value={`pr-${mode}`}>
              <GitHubPullRequestDetail
                commentsQuery={commentsQuery}
                mode={mode}
                onSelectedPullRequestIdChange={onSelectedPullRequestIdChange}
                pullRequest={pullRequest}
              />
            </TabsContent>
          ))}
        </div>
        <GitHubPullRequestMetaRail pullRequest={pullRequest} />
      </div>
    </div>
  );
}
```

Keep `GitHubPullRequestDetail` on the shared `commentsQuery` prop shown in Step 5; do not call the comment hook inside it.
Replace the existing unconditional `selectedPullRequest` block with this exact host branch.

```tsx
{selectedPullRequest ? (
  githubHosted ? (
    <GitHubPullRequestDetailShell
      onSelectedPullRequestIdChange={onSelectedPullRequestIdChange}
      project={project}
      pullRequest={selectedPullRequest}
    />
  ) : (
    <div className={PROJECT_DETAIL_PANEL_CLASS} data-project-detail-panel>
      <div className="grid xl:grid-cols-[minmax(0,1fr)_18rem]">
        <div className="min-w-0">
          <PullRequestDetailHeader
            profiles={profiles}
            pullRequest={selectedPullRequest}
          />
          <div className="border-b border-border/60 px-4">
            <PullRequestTabsList
              filesCount={repoDiff?.files.length ?? files.length}
              pullRequest={selectedPullRequest}
            />
          </div>
          {(["conversation", "commits", "checks"] as const).map((mode) => (
            <TabsContent className="m-0" key={mode} value={`pr-${mode}`}>
              <PullRequestsPanel
                error={pullRequestsError}
                isLoading={pullRequestsLoading}
                mode={mode}
                onOpenInlineComment={handleOpenPullRequestComment}
                onOpenCommit={onSelectedCommitHashChange}
                onOpenTerminal={onOpenMergeRecoveryTerminal}
                onSelectedPullRequestIdChange={onSelectedPullRequestIdChange}
                profiles={profiles}
                project={project}
                pullRequests={pullRequests}
                selectedPullRequestId={selectedPullRequestId}
              />
            </TabsContent>
          ))}
          <TabsContent className="m-0" value="pr-files">
            <ProjectPullRequestFilesChangedPanel
              diff={repoDiff}
              error={repoDiffError}
              focusedAnchor={
                pullRequestCommentTarget?.pullRequestId === selectedPullRequestId
                  ? pullRequestCommentTarget.anchor
                  : null
              }
              isLoading={repoDiffLoading}
              profiles={profiles}
              project={project}
              pullRequest={selectedPullRequest}
            />
          </TabsContent>
        </div>
        <PullRequestMetaRail
          profiles={profiles}
          project={project}
          pullRequest={selectedPullRequest}
        />
      </div>
    </div>
  )
) : null}
```

Do not mount `TabsContent value="pr-files"` or `ProjectPullRequestFilesChangedPanel` when `githubHosted`.
Do not mount `MergePullRequestButton`, `PullRequestReviewCard`, `ForumComposer`, or `PullRequestReviewersRow` on this path.

In the `TabsContent value="prs"` list block, replace only the panel child with this explicit host branch.

```tsx
{githubHosted ? (
  <GitHubPullRequestsPanel
    error={pullRequestsError}
    hasMore={pullRequestsHasMore}
    isFetching={pullRequestsFetching}
    isLoading={pullRequestsLoading}
    isSuccess={pullRequestsSuccess}
    onRetry={onRetryPullRequests}
    onSelectedPullRequestIdChange={onSelectedPullRequestIdChange}
    pullRequests={pullRequests}
    selectedPullRequestId={selectedPullRequestId}
  />
) : (
  <PullRequestsPanel
    error={pullRequestsError}
    isLoading={pullRequestsLoading}
    onOpenCommit={onSelectedCommitHashChange}
    onOpenTerminal={onOpenMergeRecoveryTerminal}
    onSelectedPullRequestIdChange={onSelectedPullRequestIdChange}
    profiles={profiles}
    project={project}
    pullRequests={pullRequests}
    selectedPullRequestId={selectedPullRequestId}
  />
)}
```

`GitHubPullRequestsPanel` must check `error` before the empty-success branch and wire `onRetry` directly to `GitHubRepoStateRecovery.onRetry`.

- [ ] **Step 7: Run focused tests, typecheck, and both UI ratchets**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/ui/GitHubProjectPullRequests.test.mjs src/features/projects/lib/projectGithubPulls.test.mjs && pnpm typecheck && pnpm check:file-sizes && pnpm check:px-text
```

Expected: the component and helper contracts pass, TypeScript passes, `ProjectPullRequestsPanel.tsx` remains unchanged and under 1,000 lines, `GitHubProjectPullRequests.tsx` stays under 1,000 lines, and no new arbitrary text sizes are introduced.

- [ ] **Step 8: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/features/projects/ui/GitHubProjectPullRequests.tsx \
  src/features/projects/ui/GitHubProjectPullRequests.test.mjs \
  src/features/projects/ui/ProjectWorkspaceTabList.tsx \
  src/features/projects/ui/ProjectWorkspaceTabs.tsx \
  src/features/projects/ui/ProjectDetailScreen.tsx
git add src/features/projects/ui/GitHubProjectPullRequests.tsx \
  src/features/projects/ui/GitHubProjectPullRequests.test.mjs \
  src/features/projects/ui/ProjectWorkspaceTabList.tsx \
  src/features/projects/ui/ProjectWorkspaceTabs.tsx \
  src/features/projects/ui/ProjectDetailScreen.tsx
git diff --check
git commit -s -m "feat(projects): render GitHub pull requests in the repository tab"
```

---

### Task 7: Add E2E mock coverage for list, detail, create, and auth recovery

**Files:**

- Modify: `desktop/src/testing/e2eBridge.ts`
- Create: `desktop/tests/e2e/github-pull-requests.spec.ts`
- Modify: `desktop/tests/e2e/project-pr-review.spec.ts`
- Modify: `desktop/playwright.config.ts`

**Interfaces:**

- Consumes: the three Tauri command names, `maybeInstallE2eTauriMocks`, the existing `__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__`, and the existing command/query/signed-event trackers
- Produces: an in-memory GitHub pull-request store, `github-pull-requests.spec.ts` on the smoke project, and a legacy review spec limited to Buzz-hosted Nostr PR behavior

- [ ] **Step 1: Run impact checks before editing the shared bridge and smoke registration**

Run GitNexus upstream impact for `maybeInstallE2eTauriMocks`, the mock Tauri invoke switch, and the smoke project in `desktop/playwright.config.ts`.
Report direct callers, affected processes, and risk level.
Stop and warn before editing if any result is HIGH or CRITICAL.

- [ ] **Step 2: Write the smoke spec**

```ts
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
  await expect(page.getByText("PR body from GitHub", { exact: true })).toBeVisible();
  const comments = page.getByTestId("project-issue-comment-timeline-row");
  await expect(comments).toHaveCount(2);
  await expect(comments.nth(0)).toContainText("API-order first comment.");
  await expect(comments.nth(1)).toContainText("API-order second comment.");
  await expect(page.getByText("grace", { exact: true })).toBeVisible();
  await expect(page.getByTestId("project-pull-request-comment-composer")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Merge" })).toHaveCount(0);
  await expect(page.getByRole("tab", { name: /Files changed/ })).toHaveCount(0);
  await expect(page.getByTestId("pull-request-discussed-in")).toHaveCount(0);

  await page.getByRole("tab", { name: /Commits/ }).click();
  await expect(page.getByTestId("project-github-pull-request-commit-row")).toBeVisible();
  await expect(page.getByTestId("project-github-pull-request-commit-row")).toContainText("ada");

  await page.getByRole("button", { name: "Pull Request", exact: true }).click();
  await page.getByRole("button", { name: "New pull request" }).click();
  await page.getByTestId("create-pull-request-compare-branch").selectOption("main");
  await page.getByTestId("create-pull-request-title").fill("New GitHub change");
  await page.getByTestId("create-pull-request-body").fill("Created from Buzz");
  await page.getByTestId("create-pull-request-submit").click();
  await expect(page.getByText("New GitHub change", { exact: true })).toBeVisible({
    timeout: 10_000,
  });
  await expect(page.getByText("#43", { exact: true })).toBeVisible();

  const commands = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
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
  expect(signedEvents).toHaveLength(0);
  const detailFilters = await page.evaluate(
    () => window.__BUZZ_E2E_PROJECT_QUERY_FILTERS__ ?? [],
  );
  expect(
    detailFilters.some((filter) => filter.kinds?.includes(1618)),
  ).toBe(false);
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
  await expect(page.getByTestId("project-github-pull-request-row")).toHaveCount(0);
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
  await expect(page.getByText("PR body from GitHub", { exact: true })).toBeVisible();
  await expect(
    page.getByText("Could not load GitHub comments", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText("Could not load GitHub pull requests", { exact: true }),
  ).toHaveCount(0);
  const before = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  const listCallsBefore = before.filter((command) => command === "list_github_pull_requests").length;
  const commentCallsBefore = before.filter((command) => command === "list_github_pull_request_comments").length;
  await page
    .locator('[aria-labelledby="github-pull-request-comments-recovery-title"]')
    .getByRole("button", { name: "Retry", exact: true })
    .click();
  await expect(
    page.getByText("Could not load GitHub comments", { exact: true }),
  ).toBeVisible();
  const after = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(after.filter((command) => command === "list_github_pull_requests").length).toBe(listCallsBefore);
  expect(after.filter((command) => command === "list_github_pull_request_comments").length).toBe(
    commentCallsBefore + 1,
  );
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
  await expect(page.getByText("Pull request not found.", { exact: true })).toBeVisible();
  await expect(page.getByTestId("project-github-pull-request-row")).toHaveCount(1);
  await expect(page.getByText("PR body from GitHub", { exact: true })).toHaveCount(0);
});
```

Add `"**/github-pull-requests.spec.ts"` to the smoke `testMatch` list in `playwright.config.ts` next to `"**/github-issues.spec.ts"`.

- [ ] **Step 3: Build the E2E app and verify RED before adding bridge behavior**

Kill any stale process on port 4173 before building because Playwright reuses the existing preview server.

```bash
. ./bin/activate-hermit
cd desktop
pnpm build:e2e
pnpm exec playwright test --project=smoke tests/e2e/github-pull-requests.spec.ts
```

Expected: FAIL because `list_github_pull_requests` is not handled by the mock bridge; do not weaken any UI assertion.

- [ ] **Step 4: Add the minimal mock store and command stubs**

Add these window fields beside the existing GitHub issue fields in `desktop/src/testing/e2eBridge.ts`.

```ts
__BUZZ_E2E_GITHUB_PULLS_ERROR__?: { code: string; message: string };
__BUZZ_E2E_GITHUB_PULL_COMMENTS_ERROR__?: { code: string; message: string };
__BUZZ_E2E_GITHUB_PULL_STORE__?: E2eGithubPullStore;
```

Add these mock DTOs beside the GitHub issue mock DTOs.

```ts
type E2eGithubPullUser = { login: string; avatar_url: string };
type E2eGithubPullDto = {
  number: number;
  title: string;
  body: string;
  html_url: string;
  draft: boolean;
  comments: number;
  created_at: number;
  updated_at: number;
  user: E2eGithubPullUser;
  head: { ref: string; sha: string; repo: { full_name: string } };
  base: { ref: string; repo: { full_name: string } };
};
type E2eGithubPullCommentDto = {
  id: number;
  body: string;
  html_url: string;
  created_at: number;
  user: E2eGithubPullUser;
};
type E2eGithubPullStore = {
  pulls: E2eGithubPullDto[];
  commentsByNumber: Record<number, E2eGithubPullCommentDto[]>;
};
```

Add this exact seed function.

```ts
function createDefaultE2eGithubPullStore(): E2eGithubPullStore {
  return {
    pulls: [
      {
        number: 42,
        title: "Fix login",
        body: "PR body from GitHub",
        html_url: "https://github.com/acme/app/pull/42",
        draft: false,
        comments: 2,
        created_at: 1_704_166_645,
        updated_at: 1_704_253_045,
        user: { login: "ada", avatar_url: "" },
        head: {
          ref: "feature",
          sha: "d".repeat(40),
          repo: { full_name: "acme/app" },
        },
        base: { ref: "develop", repo: { full_name: "acme/app" } },
      },
    ],
    commentsByNumber: {
      42: [
        {
          id: 2,
          body: "API-order first comment.",
          html_url: "https://github.com/acme/app/issues/42#issuecomment-2",
          created_at: 1_704_253_100,
          user: { login: "grace", avatar_url: "" },
        },
        {
          id: 10,
          body: "API-order second comment.",
          html_url: "https://github.com/acme/app/pull/42#issuecomment-10",
          created_at: 1_704_253_200,
          user: { login: "linus", avatar_url: "" },
        },
      ],
    },
  };
}
```

Initialize the store once per `installMockBridge` call with this exact expression before the invoke switch is installed.

```ts
window.__BUZZ_E2E_GITHUB_PULL_STORE__ ??=
  createDefaultE2eGithubPullStore();
```

Handle these cases beside the existing GitHub issue commands.

```ts
case "list_github_pull_requests": {
  if (window.__BUZZ_E2E_GITHUB_PULLS_ERROR__) {
    throw window.__BUZZ_E2E_GITHUB_PULLS_ERROR__;
  }
  const store = window.__BUZZ_E2E_GITHUB_PULL_STORE__;
  if (!store) throw new Error("GitHub pull store was not initialized.");
  return { pulls: store.pulls, has_more: false };
}
case "create_github_pull_request": {
  const store = window.__BUZZ_E2E_GITHUB_PULL_STORE__;
  if (!store) throw new Error("GitHub pull store was not initialized.");
  const input = payload as {
    cloneUrl: string;
    title: string;
    body: string;
    head: string;
    base: string;
  };
  const number = Math.max(0, ...store.pulls.map((pull) => pull.number)) + 1;
  const pull: E2eGithubPullDto = {
    number,
    title: input.title,
    body: input.body,
    html_url: `https://github.com/acme/app/pull/${number}`,
    draft: false,
    comments: 0,
    created_at: Math.floor(Date.now() / 1000),
    updated_at: Math.floor(Date.now() / 1000),
    user: { login: "mock-user", avatar_url: "" },
    head: {
      ref: input.head,
      sha: "e".repeat(40),
      repo: { full_name: "acme/app" },
    },
    base: { ref: input.base, repo: { full_name: "acme/app" } },
  };
  store.pulls.unshift(pull);
  store.commentsByNumber[number] = [];
  return pull;
}
case "list_github_pull_request_comments": {
  if (window.__BUZZ_E2E_GITHUB_PULL_COMMENTS_ERROR__) {
    throw window.__BUZZ_E2E_GITHUB_PULL_COMMENTS_ERROR__;
  }
  const store = window.__BUZZ_E2E_GITHUB_PULL_STORE__;
  if (!store) throw new Error("GitHub pull store was not initialized.");
  const number = (payload as { number: number }).number;
  return store.commentsByNumber[number] ?? [];
}
```

The wrapper in Task 4 passes `{ cloneUrl, number }` directly, so the mock reads `number` from the top-level payload exactly as shown.

- [ ] **Step 5: Remove obsolete GitHub-hosted Nostr merge UI scenarios**

The approved host split means a `github.com` repository no longer renders the kind:1618 merge UI, so the existing GitHub merge scenarios in `desktop/tests/e2e/project-pr-review.spec.ts` contradict the new product behavior.
Delete exactly these tests and the two-theme loop:

- `sends GitHub merge payload unchanged through native boundary`
- `GitHub merge success publishes one merged status`
- `GitHub merged-status retry skips the merge command`
- `GitHub CLI guidance persists with retry`
- `GitHub blocked recovery opens the exact pull request and retries`
- `GitHub branch changes require refresh without a stale merge action`
- `GitHub ambiguous recovery opens the exact pull-request list`
- `invalid GitHub recovery URLs never render an open action`
- `renders GitHub merge recovery in buzz`
- `renders GitHub merge recovery in buzz-dark`

Delete `openClonedGitHubAlicePullRequest`, `confirmMerge`, `openedExternalUrls`, and the now-unused `KIND_GIT_PULL_REQUEST` constant.
Keep `cloneMissingGitHubRepository`; the unrelated SCP-style clone coverage later in the file still uses it.
Keep all Buzz-hosted review, merge-conflict, inline-comment, create, and authorization scenarios unchanged.
Do not remove or modify the native merge implementation tests in `desktop/src-tauri/src/commands/project_github_pull_request/tests.rs`; P4 will adapt that tested capability to numeric GitHub PRs later.

- [ ] **Step 6: Run focused E2E, preserved native merge tests, and verify GREEN**

Kill any stale process on port 4173 before `pnpm build:e2e`.

```bash
. ./bin/activate-hermit
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_pull_request
cd desktop
pnpm build:e2e
pnpm exec playwright test --project=smoke tests/e2e/github-pull-requests.spec.ts tests/e2e/project-pr-review.spec.ts tests/e2e/github-issues.spec.ts
```

Expected: GitHub list, read-only detail, create, auth recovery, comment retry, and comment-404 recovery pass; Buzz-hosted PR review coverage and GitHub issue coverage remain green; and the existing native GitHub merge core remains tested without exposing it on this P1/P2 UI path.

- [ ] **Step 7: Format, inspect scope, and commit**

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/testing/e2eBridge.ts \
  tests/e2e/github-pull-requests.spec.ts \
  tests/e2e/project-pr-review.spec.ts \
  playwright.config.ts
git add src/testing/e2eBridge.ts \
  tests/e2e/github-pull-requests.spec.ts \
  tests/e2e/project-pr-review.spec.ts \
  playwright.config.ts
git diff --check
git commit -s -m "test(projects): cover GitHub pull request list and create"
```

---

## Acceptance Criteria

On a GitHub-hosted project (`harness-service` or the e2e `https://github.com/acme/app` override), with `gh` installed and authenticated or with the mock bridge:

- The Pull Request tab shows GitHub open PRs as `#N`, including drafts as Draft.
- A raw first page of 100 PRs shows the muted `More open pull requests exist on GitHub.` note and does not add pagination controls.
- Creating a pull request sends only title, exact body, same-repository head, and base to GitHub; a colon-qualified head is rejected before `gh` runs.
- Creating a pull request does not query, sign, or publish kind:1618 and invalidates only the per-repository pull-request query.
- Opening `#N` shows the body and read-only conversation comments in GitHub order.
- A non-404 comment failure keeps the body and retries only comments; a comment 404 shows `Pull request not found.`, clears selection, and keeps the list.
- Copy link copies `https://github.com/<owner>/<repo>/pull/<N>` when that URL is valid.
- Commits shows exactly one head-SHA row, Checks keeps the empty-check copy, and Files changed is absent.
- Merge, review writes, reviewers, the composer, Discussed in channels, origin reference, and the Nostr-only Update PR action are not offered.
- Opening a GitHub PR never starts `get_project_repo_diff` or `get_project_local_repo_diff` against `refs/nostr/<N>`.
- Without `gh` or without auth, the tab shows merge-style recovery, not `No pull requests yet.`
- Buzz-hosted repositories still list and create kind:1618 only.
- Existing native GitHub merge tests remain green, but the obsolete GitHub-hosted kind:1618 merge UI scenarios are removed from `project-pr-review.spec.ts` because P4 adaptation is out of scope.

## Validation Commands

Run from the worktree root after activating Hermit.

```bash
. ./bin/activate-hermit
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_pulls
cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_pull_request
cd desktop
pnpm test -- src/features/projects/projectPullRequests.test.mjs src/features/projects/lib/projectGithubPulls.test.mjs src/features/projects/lib/projectShareLinks.test.mjs src/features/projects/pullRequestMutations.test.mjs src/features/projects/pullRequestMutations.github.test.mjs src/features/projects/lib/projectGithubSync.test.mjs src/features/projects/ui/GitHubProjectPullRequests.test.mjs
pnpm typecheck
pnpm check:file-sizes
pnpm check:px-text
pnpm build:e2e
pnpm exec playwright test --project=smoke tests/e2e/github-pull-requests.spec.ts tests/e2e/project-pr-review.spec.ts tests/e2e/github-issues.spec.ts
cd ..
just ci
```

Before final handoff, run GitNexus `detect_changes({ scope: "compare", base_ref: "main" })` when available.
Confirm the diff is limited to the file map above and does not touch CLI, mobile, `merge_github_pull_request`, or the global work-items list.

## Self-Review

Spec coverage:

- Host split, auth, list, create, comments, share, identity, status, branches, errors, hidden chrome, query shape, invalidation, and tests each have a task.
- P3/P4/P5, CLI, mobile, global list, and merge-command changes are excluded by Global Constraints.
- `publishProjectPullRequestUpdate` skip for GitHub already exists and is covered by `projectGithubSync.test.mjs`.
- Legacy GitHub-hosted kind:1618 merge UI tests are removed because they contradict the approved host split, while the existing native merge-core test module remains in the validation gate.

Placeholder scan: no TBD, TODO, or "similar to Task N" steps remain.

Type consistency: Rust `GitHubPullRequestDto` / `GitHubPullRequestListDto.pulls` / `GitHubPullRequestCommentDto` match the TypeScript `GithubPullRequestDto` / `GithubPullRequestListDto` / `GithubPullRequestCommentDto` names used by later tasks.
Selection ids are decimal strings.
Query key remains `["project", id, "pull-requests"]`.
Comment query key is `["project", id, "pull-requests", number, "comments"]`.

Open questions: none.
