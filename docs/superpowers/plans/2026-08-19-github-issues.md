# GitHub Issues (list + create) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a Projects repository uses a plain `github.com` clone URL, its per-repository Issues tab lists open GitHub Issues, creates GitHub Issues, and never reads or publishes NIP-34 `kind:1621` for that repository.

**Architecture:** Add three Tauri commands backed by the existing bounded `GhRunner` and `GitHubRepoRef` abstractions.
Route the existing issue query and create mutation by the selected repository's first clone URL, map bounded GitHub DTOs into `ProjectIssue`, and keep the Buzz-hosted issue path unchanged.
Put GitHub-only list/detail chrome in a focused component so GitHub logins never reach pubkey normalization, profile lookup, or Nostr assignment/comment mutations.

**Tech Stack:** Rust, Tauri 2, `gh api`, React 19, TanStack Query, Node `node:test`, and Playwright with the E2E mock bridge.

**Spec:** [2026-08-18-github-issues-design.md](../specs/2026-08-18-github-issues-design.md)

**Product contract:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) make GitHub the native issue backend for GitHub-hosted repositories while Buzz remains the collaboration layer.

**Phase doc:** [phase-02-github-issues.md](../../../plans/20260818-1211-github-native-host/phase-02-github-issues.md), slice I1 + I2 only.

## Global Constraints

- Implement I1 + I2 only: list open issues, create an issue with title and body, and load existing comments read-only when detail opens.
- Do not implement close/reopen, posting comments, label writes, assignee writes, an Open/Closed filter, `state=all`, a second page, or Load more.
- Do not change the global Projects Issues list, project card/activity counts, CLI, mobile, or the `buzz://issue` scheme.
- Do not union GitHub Issues with kind `1621`, import GitHub issues into Nostr, or dual-write.
- Do not call `gh issue`, add a provider trait, or store a GitHub token.
- Authenticate only through installed `gh` and `gh auth status --hostname github.com`.
- Use `gh api` with one page, `per_page=100`, `sort=updated`, `direction=desc`, and no `--paginate`.
- Require `list_github_issues.state` to be exactly `open` or `closed`; this slice's UI always sends `open`.
- Drop items whose projected `has_pull_request` is true.
- Compute `has_more` from the projected raw page length before filtering pull requests or invalid URLs.
- Accept only positive `u64` issue numbers in Rust and positive safe decimal integers in TypeScript.
- Treat GitHub authors and assignees as logins plus avatar URLs, never as Nostr pubkeys.
- Never pass a GitHub login to `ProfileIdentityButton`, `ProfileAuthorName`, `normalizePubkey`, `useUsersBatchQuery`, `IssueAssigneesRow`, or Nostr assignment/comment mutations.
- Add `Open` to both the runtime and declaration-file `ProjectIssueStatus` definitions.
- Map GitHub `open` to `Open` and GitHub `closed` to `Closed`; never map GitHub `open` to `Backlog`.
- Copy only a validated GitHub issue URL, and keep the existing hex-event `buzz://issue` fallback for Buzz issues.
- Create with exactly `{ "title", "body" }` through a tempfile passed to `gh api --input`.
- Trim the title, reject an empty title, and reject more than 256 Unicode scalar values before running `gh`.
- Preserve the body supplied to the native command, including an empty body.
- On GitHub create success, invalidate only `["project", project.id, "issues"]`.
- On Buzz create success, preserve the current issues, work-items, and activity-summaries invalidations.
- Keep the issue list query key `["project", project.id, "issues"]` in this slice.
- Use only `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, and `github_issues_failed` from the new native module.
- Do not leak `github_merge_failed` or `github_state_failed` from any new command.
- Check GitHub errors before empty-success rendering.
- GitHub empty-success copy is `No open issues.` and the existing Buzz empty/error copy stays unchanged.
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

- Use `GH_ISSUE_STREAM_LIMIT = 32 * 1024 * 1024` for issue-list, create, and comment JSON.
- The 32 MiB cap accommodates one page of 100 large UTF-8 bodies plus projected metadata while remaining bounded.
- A response that exceeds the cap becomes invalid/truncated JSON and returns `github_issues_failed`; never substitute an empty list.
- A created issue whose `html_url` fails repository-bound validation returns `github_issues_failed`.
- List recovery title is `Could not load GitHub issues`.
- Comment recovery title is `Could not load GitHub comments` and retries only the comment query.
- `desktop/src-tauri/src/lib.rs` is currently 999 lines.
- Registering three commands adds three lines, so Task 3 removes the two existing blank lines between `ClipboardState::release()`, the macOS restart block, and the native-destructor comment to keep the formatted file at exactly 1,000 lines.

## File Map

| File | Responsibility |
|------|----------------|
| `desktop/src-tauri/src/commands/project_github_pull_request.rs` | Expose only `GitHubRepoRef.owner` and `.repo` as `pub(crate)` |
| Create `desktop/src-tauri/src/commands/project_github_issues.rs` | Bounded list/create/comments commands, DTO mapping, URL validation, and issue-specific error remapping |
| `desktop/src-tauri/src/commands/mod.rs` | Declare and re-export the new command module |
| `desktop/src-tauri/src/lib.rs` | Register three commands without crossing the 1,000-line ratchet |
| `desktop/src/shared/api/projectGit.ts` | Native DTO types and three Tauri invoke wrappers |
| `desktop/src/features/projects/projectIssues.mjs` | Add `Open` and populate neutral GitHub-extension fields on Nostr issues |
| `desktop/src/features/projects/projectIssues.d.mts` | Keep `ProjectIssue`, comments, and status declarations aligned with the runtime object |
| `desktop/src/features/projects/projectIssues.test.mjs` | Protect the unchanged Nostr mapping defaults |
| Create `desktop/src/features/projects/lib/projectGithubIssues.ts` | Host routing, DTO mapping, display helpers, identity filtering, and comment query |
| Create `desktop/src/features/projects/lib/projectGithubIssues.test.mjs` | Protect routing, mapping, identity, number, and comment-query contracts |
| `desktop/src/features/projects/lib/projectShareLinks.ts` | Prefer a strictly validated GitHub issue URL before the existing Buzz fallback |
| `desktop/src/features/projects/lib/projectShareLinks.test.mjs` | Protect GitHub URL validation and the existing Buzz deep link |
| `desktop/src/features/projects/hooks.ts` | Route `useProjectIssuesQuery` and retain the current Nostr loader |
| `desktop/src/features/projects/issueMutations.ts` | Route create and select the correct invalidation set |
| Create `desktop/src/features/projects/issueMutations.test.mjs` | Prove GitHub never calls the Buzz publisher and invalidates only issues |
| `desktop/src/features/projects/ui/GitHubRepoStateRecovery.tsx` | Accept issue-specific titles and unique heading IDs |
| Create `desktop/src/features/projects/ui/GitHubProjectIssues.tsx` | Render GitHub list rows, read-only detail, login identities, assignee facepile, and comment states |
| `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx` | Branch once by host and preserve the existing Buzz issue components |
| `desktop/src/features/projects/ui/ProjectIssueCommentTimeline.tsx` | Render GitHub login/avatar comments without pubkey helpers |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Read the new query shape and filter profile lookup to 64-hex identities |
| `desktop/src/testing/e2eBridge.ts` | Stub list/create/comments and a structured issues error |
| Create `desktop/tests/e2e/github-issues.spec.ts` | Exercise list, detail comments, create, hidden write controls, and auth recovery |
| `desktop/playwright.config.ts` | Add the new spec to the smoke project |

---

### Task 1: Add the bounded Rust issue-list core

**Files:**

- Modify `desktop/src-tauri/src/commands/project_github_pull_request.rs:21-24`.
- Create `desktop/src-tauri/src/commands/project_github_issues.rs`.
- Modify `desktop/src-tauri/src/commands/mod.rs` to add only `mod project_github_issues;` in this task.

**Interfaces:**

- Consumes `GitHubRepoRef::{parse, slug, owner, repo}`, `GhRunner::{ensure_auth, from_resolved, run_with_limit}`, `combined_cli_diagnostic`, `redact_diagnostic`, and `ProjectPullRequestMergeError`.
- Produces `GitHubIssueUserDto`, `GitHubIssueDto`, `GitHubIssueListDto`, `list_github_issues_with`, `is_issue_html_url`, and `remap_issues_error`.

- [ ] **Step 1: Run impact checks before touching existing symbols**

Run GitNexus upstream impact for `GitHubRepoRef`, `GhRunner::run_with_limit`, `combined_cli_diagnostic`, and `remap_state_error`.
Report direct callers, affected processes, and risk level.
Stop and warn before editing if any result is HIGH or CRITICAL.

- [ ] **Step 2: Add the module declaration and write failing tests first**

Add `mod project_github_issues;` beside the other `project_github_*` modules so its unit tests compile as part of the desktop library.
Create the new file with test-only fixtures and tests that reference the not-yet-defined production functions.
The fake `gh` output must already match the declared `--jq` projection: labels are strings and every issue has `has_pull_request`.

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

    fn projected_issue(number: u64, has_pull_request: bool) -> serde_json::Value {
        json!({
            "number": number,
            "title": format!("Issue {number}"),
            "body": "Steps",
            "state": "open",
            "html_url": format!("https://github.com/acme/app/issues/{number}"),
            "comments": 3,
            "created_at": "2026-01-02T03:04:05Z",
            "updated_at": "2026-01-03T03:04:05Z",
            "user": { "login": "ada", "avatar_url": "https://avatars.githubusercontent.com/u/1" },
            "labels": ["bug"],
            "assignees": [{ "login": "linus", "avatar_url": "https://avatars.githubusercontent.com/u/2" }],
            "has_pull_request": has_pull_request,
        })
    }

    #[cfg(unix)]
    fn fake_gh(output: &serde_json::Value, status: i32, stderr: &str) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("create fake gh directory");
        let path = dir.path().join("gh");
        let script = format!(
            "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\n  *) printf '%s' '{}' ; printf '%s' '{}' >&2; exit {} ;;\nesac\n",
            output.to_string(),
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
    fn maps_projected_open_issue_fields() {
        let output = json!([projected_issue(42, false)]);
        let (dir, path) = fake_gh(&output, 0, "");
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
        assert!(calls.contains("/repos/acme/app/issues?state=open&per_page=100&sort=updated&direction=desc"));
        assert!(calls.contains("--jq"));
        assert!(calls.contains("has_pull_request"));
    }

    #[cfg(unix)]
    #[test]
    fn drops_projected_pull_request_items() {
        let output = json!([projected_issue(7, true), projected_issue(42, false)]);
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_issues_with(&gh, "https://github.com/acme/app", "open").expect("list");
        assert_eq!(page.issues.iter().map(|issue| issue.number).collect::<Vec<_>>(), vec![42]);
    }

    #[cfg(unix)]
    #[test]
    fn raw_projected_page_of_100_sets_has_more_before_filtering() {
        let output = serde_json::Value::Array(
            (1..=100).map(|number| projected_issue(number, true)).collect(),
        );
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_issues_with(&gh, "https://github.com/acme/app", "open").expect("list");
        assert!(page.issues.is_empty());
        assert!(page.has_more);
    }

    #[test]
    fn accepts_only_repo_bound_issue_urls() {
        let repo = GitHubRepoRef::parse("https://github.com/Acme/App").expect("repo");
        for (raw, number, expected) in [
            ("https://github.com/acme/app/issues/42", 42, true),
            ("https://github.com/ACME/APP/issues/42", 42, true),
            ("https://github.com/acme/app/issues/43", 42, false),
            ("https://github.com/acme/other/issues/42", 42, false),
            ("https://evil.example/acme/app/issues/42", 42, false),
            ("https://user@github.com/acme/app/issues/42", 42, false),
            ("https://github.com/acme/app/issues/42?x=1", 42, false),
            ("https://github.com/acme/app/issues/42#x", 42, false),
            ("https://github.com/acme/app/issues/42/", 42, false),
        ] {
            assert_eq!(is_issue_html_url(&repo, raw, number), expected, "{raw}");
        }
    }

    #[test]
    fn rejects_non_github_clone_url_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = list_github_issues_with(
            &gh,
            &format!("https://relay.example/git/{}/app", "ab".repeat(32)),
            "open",
        )
        .expect_err("reject Buzz URL");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[test]
    fn rejects_state_all_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = list_github_issues_with(&gh, "https://github.com/acme/app", "all")
            .expect_err("reject all");
        assert_eq!(error_code(&error), "github_issues_failed");
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
        let error = list_github_issues_with(&gh, "https://github.com/acme/app", "open")
            .expect_err("auth");
        assert_eq!(error_code(&error), "github_auth_required");
    }

    #[test]
    fn remaps_repository_and_rate_failures_to_issue_codes() {
        let not_found = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 404");
        assert_eq!(error_code(&remap_issues_error(not_found, "Not Found")), "github_repo_unavailable");
        let forbidden = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
        assert_eq!(error_code(&remap_issues_error(forbidden, "Forbidden")), "github_repo_unavailable");
        let limited = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
        assert_eq!(error_code(&remap_issues_error(limited, "API rate limit exceeded")), "github_issues_failed");
    }
}
```

- [ ] **Step 3: Run the focused test and verify RED**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib maps_projected_open_issue_fields
```

Expected: compilation fails because `list_github_issues_with` and the DTOs do not exist.
Do not change production code until the failure is observed for that reason.

- [ ] **Step 4: Implement the minimal list core and expose only the required repository fields**

Change only the two fields in the existing runner file.

```rust
pub(crate) struct GitHubRepoRef {
    pub(crate) owner: String,
    pub(crate) repo: String,
}
```

Add the following shapes and constants in `project_github_issues.rs`.

```rust
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{redact_diagnostic, GhRunner, GitHubRepoRef};
use crate::commands::project_github_repository_state::combined_cli_diagnostic;
use chrono::DateTime;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::ffi::OsString;
use std::path::Path;
use url::Url;

const GH_ISSUE_STREAM_LIMIT: usize = 32 * 1024 * 1024;
const ISSUE_LIST_JQ: &str = "[.[] | {number, title, body: (.body // \"\"), state, html_url, comments, created_at, updated_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), labels: [(.labels // [])[] | if type == \"string\" then . else .name end], assignees: [(.assignees // [])[] | {login, avatar_url: (.avatar_url // \"\")}], has_pull_request: has(\"pull_request\")}]";

/// GitHub login identity returned to the desktop issue UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubIssueUserDto {
    pub login: String,
    pub avatar_url: String,
}

/// Bounded GitHub issue returned to the desktop issue UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubIssueDto {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub html_url: String,
    pub comments: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub user: GitHubIssueUserDto,
    pub labels: Vec<String>,
    pub assignees: Vec<GitHubIssueUserDto>,
}

/// One bounded GitHub issue page plus its first-page truncation signal.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubIssueListDto {
    pub issues: Vec<GitHubIssueDto>,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubIssueUserWire {
    login: String,
    avatar_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubIssueWire {
    number: u64,
    title: String,
    body: String,
    state: String,
    html_url: String,
    comments: u64,
    created_at: String,
    updated_at: String,
    user: Option<GitHubIssueUserWire>,
    labels: Vec<String>,
    assignees: Vec<GitHubIssueUserWire>,
    has_pull_request: bool,
}
```

Implement the request boundary exactly once so list/create/comments all share the same cap and error conversion.

```rust
fn github_api_json<T: DeserializeOwned>(
    gh: &GhRunner,
    method: &str,
    path: &str,
    jq: &str,
    input: Option<&Path>,
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
        .run_with_limit(&args, GH_ISSUE_STREAM_LIMIT)
        .map_err(|error| remap_issues_error(error, ""))?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        return Err(remap_issues_error(
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                redact_diagnostic(&diagnostic),
            ),
            &diagnostic,
        ));
    }
    serde_json::from_str(&output.stdout).map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub CLI returned an unexpected or truncated issue response. Update gh, then retry.",
        )
    })
}
```

Implement URL validation with `url::Url`.
Require HTTPS, host `github.com`, no username/password/query/fragment, no non-default port, exactly four path segments, case-insensitive owner/repo equality, literal `issues`, and a matching positive number.

```rust
pub(crate) fn is_issue_html_url(repo: &GitHubRepoRef, raw: &str, number: u64) -> bool {
    if number == 0 || raw != raw.trim() || raw.contains('\\') || raw.contains('%') {
        return false;
    }
    let Ok(url) = Url::parse(raw) else { return false; };
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
        && segments[2] == "issues"
        && segments[3].parse::<u64>().ok() == Some(number)
}
```

Implement `map_issue` as a filter boundary.
Return `None` for a pull request, number zero, empty login, unsupported state, invalid timestamp, or invalid `html_url`.
Map RFC3339 timestamps with `DateTime::parse_from_rfc3339(...).ok()?.timestamp()`.

```rust
fn map_issue(repo: &GitHubRepoRef, item: GitHubIssueWire) -> Option<GitHubIssueDto> {
    if item.has_pull_request || item.number == 0 || !matches!(item.state.as_str(), "open" | "closed") {
        return None;
    }
    let user = item.user?;
    if user.login.trim().is_empty() || !is_issue_html_url(repo, &item.html_url, item.number) {
        return None;
    }
    let created_at = DateTime::parse_from_rfc3339(&item.created_at).ok()?.timestamp();
    let updated_at = DateTime::parse_from_rfc3339(&item.updated_at).ok()?.timestamp();
    let assignees = item
        .assignees
        .into_iter()
        .filter(|assignee| !assignee.login.trim().is_empty())
        .map(|assignee| GitHubIssueUserDto {
            login: assignee.login,
            avatar_url: assignee.avatar_url,
        })
        .collect();
    Some(GitHubIssueDto {
        number: item.number,
        title: item.title,
        body: item.body,
        state: item.state,
        html_url: item.html_url,
        comments: item.comments,
        created_at,
        updated_at,
        user: GitHubIssueUserDto { login: user.login, avatar_url: user.avatar_url },
        labels: item.labels,
        assignees,
    })
}
```

Implement `remap_issues_error` by serializing the existing structured error, preserving only CLI/auth codes, checking rate/abuse before generic 403, mapping repository absence to `github_repo_unavailable`, and mapping everything else to `github_issues_failed`.

```rust
pub(crate) fn remap_issues_error(
    error: ProjectPullRequestMergeError,
    diagnostic: &str,
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
        if original.is_empty() { "GitHub issue request failed.".to_string() } else { original.to_string() }
    } else {
        redact_diagnostic(diagnostic)
    };
    if lower.contains("rate limit") || lower.contains("abuse") {
        return ProjectPullRequestMergeError::new("github_issues_failed", message);
    }
    if lower.contains("404")
        || lower.contains("not found")
        || (lower.contains("403") && !lower.contains("rate"))
    {
        return ProjectPullRequestMergeError::new("github_repo_unavailable", message);
    }
    ProjectPullRequestMergeError::new("github_issues_failed", message)
}
```

Implement the list function with validation before any runner call.

```rust
pub(crate) fn list_github_issues_with(
    gh: &GhRunner,
    clone_url: &str,
    state: &str,
) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError> {
    if !matches!(state, "open" | "closed") {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub issue list state must be open or closed.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))?;
    gh.ensure_auth().map_err(|error| remap_issues_error(error, ""))?;
    let path = format!(
        "/repos/{}/issues?state={state}&per_page=100&sort=updated&direction=desc",
        repo.slug(),
    );
    let raw: Vec<GitHubIssueWire> = github_api_json(gh, "GET", &path, ISSUE_LIST_JQ, None)?;
    let has_more = raw.len() == 100;
    let issues = raw.into_iter().filter_map(|item| map_issue(&repo, item)).collect();
    Ok(GitHubIssueListDto { issues, has_more })
}
```

- [ ] **Step 5: Run all issue-module tests and verify GREEN**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
```

Expected: all list, URL, validation, and error tests pass on Unix; runner-backed tests are skipped on non-Unix platforms.

- [ ] **Step 6: Format, inspect scope, and commit**

Run GitNexus `detect_changes({ scope: "staged" })` after staging and verify only the new issue module, module declaration, and two field visibilities are affected.

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_pull_request.rs \
  desktop/src-tauri/src/commands/project_github_issues.rs \
  desktop/src-tauri/src/commands/mod.rs
git diff --check
git commit -s -m "feat(projects): map GitHub issues from gh api"
```

---

### Task 2: Add GitHub issue creation and read-only comment loading in Rust

**Files:**

- Modify `desktop/src-tauri/src/commands/project_github_issues.rs`.

**Interfaces:**

- Consumes `github_api_json`, `map_issue`, `remap_issues_error`, `GitHubRepoRef`, and `GhRunner` from Task 1.
- Produces `GitHubIssueCommentDto`, `create_github_issue_with`, and `list_github_issue_comments_with`.

- [ ] **Step 1: Write failing create and comment tests**

Keep each boundary in its own test.
The create fake must copy the `--input` tempfile before the process exits so the test can assert the real JSON body.

```rust
#[test]
fn create_rejects_blank_title_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let error = create_github_issue_with(&gh, "https://github.com/acme/app", "   ", "body")
        .expect_err("blank");
    assert_eq!(error_code(&error), "github_issues_failed");
}

#[test]
fn create_rejects_more_than_256_unicode_scalars_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let title = "é".repeat(257);
    let error = create_github_issue_with(&gh, "https://github.com/acme/app", &title, "")
        .expect_err("long");
    assert_eq!(error_code(&error), "github_issues_failed");
}

#[cfg(unix)]
#[test]
fn create_posts_trimmed_title_and_exact_body() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("gh");
    let output = projected_issue(43, false).to_string();
    let script = format!(
        "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\n  *--method*POST*)\n    previous=\"\"\n    for argument in \"$@\"; do\n      if [ \"$previous\" = \"--input\" ]; then cp \"$argument\" \"$root/input.json\"; fi\n      previous=\"$argument\"\n    done\n    printf '%s' '{}'\n    ;;\n  *) exit 1 ;;\nesac\n",
        output,
    );
    std::fs::write(&path, script).expect("write fake gh");
    let mut permissions = std::fs::metadata(&path).expect("stat").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let issue = create_github_issue_with(
        &gh,
        "https://github.com/acme/app",
        "  Issue 43  ",
        " body with surrounding space ",
    )
    .expect("create");
    assert_eq!(issue.number, 43);
    let input: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
    )
    .expect("json");
    assert_eq!(input, serde_json::json!({
        "title": "Issue 43",
        "body": " body with surrounding space ",
    }));
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("--method POST"));
    assert!(calls.contains("/repos/acme/app/issues"));
    assert!(calls.contains("--input"));
    assert!(calls.contains("--jq"));
}

#[cfg(unix)]
#[test]
fn create_rejects_response_with_foreign_html_url() {
    let mut output = projected_issue(43, false);
    output["html_url"] = serde_json::Value::String("https://evil.example/issues/43".to_string());
    let (_dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = create_github_issue_with(&gh, "https://github.com/acme/app", "Issue 43", "")
        .expect_err("foreign URL");
    assert_eq!(error_code(&error), "github_issues_failed");
}

#[test]
fn comments_reject_number_zero_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let error = list_github_issue_comments_with(&gh, "https://github.com/acme/app", 0)
        .expect_err("zero");
    assert_eq!(error_code(&error), "github_issues_failed");
}

#[cfg(unix)]
#[test]
fn comments_keep_projected_github_order() {
    let output = serde_json::json!([
        { "id": 1, "body": "first", "created_at": "2026-01-02T03:04:05Z", "user": { "login": "ada", "avatar_url": "https://example.com/a" } },
        { "id": 2, "body": "second", "created_at": "2026-01-02T04:04:05Z", "user": { "login": "linus", "avatar_url": "https://example.com/b" } }
    ]);
    let (dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let comments = list_github_issue_comments_with(&gh, "https://github.com/acme/app", 42)
        .expect("comments");
    assert_eq!(comments.iter().map(|comment| comment.body.as_str()).collect::<Vec<_>>(), vec!["first", "second"]);
    assert_eq!(comments[1].user.login, "linus");
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("/repos/acme/app/issues/42/comments?per_page=100"));
}
```

- [ ] **Step 2: Run and verify RED**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib create_posts_trimmed_title_and_exact_body
```

Expected: compilation fails because `create_github_issue_with` does not exist.

- [ ] **Step 3: Implement create and comments minimally**

Add the projected item/comment queries and comment wire/DTO types.

```rust
const ISSUE_ITEM_JQ: &str = "{number, title, body: (.body // \"\"), state, html_url, comments, created_at, updated_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), labels: [(.labels // [])[] | if type == \"string\" then . else .name end], assignees: [(.assignees // [])[] | {login, avatar_url: (.avatar_url // \"\")}], has_pull_request: has(\"pull_request\")}";
const ISSUE_COMMENTS_JQ: &str = "[.[] | {id, body: (.body // \"\"), created_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end)}]";

/// One read-only GitHub issue comment returned to the desktop UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubIssueCommentDto {
    pub id: u64,
    pub body: String,
    pub created_at: i64,
    pub user: GitHubIssueUserDto,
}

#[derive(Debug, Deserialize)]
struct GitHubIssueCommentWire {
    id: u64,
    body: String,
    created_at: String,
    user: Option<GitHubIssueUserWire>,
}
```

Use a local tempfile helper with no production `unwrap()` or `expect()`.

```rust
fn issue_json_input(
    title: &str,
    body: &str,
) -> Result<tempfile::NamedTempFile, ProjectPullRequestMergeError> {
    use std::io::Write;
    let mut file = tempfile::Builder::new()
        .prefix("buzz-gh-")
        .tempfile()
        .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?;
    serde_json::to_writer(&mut file, &serde_json::json!({ "title": title, "body": body }))
        .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?;
    file.flush()
        .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?;
    Ok(file)
}
```

Implement create with validation and repository parsing before authentication.

```rust
pub(crate) fn create_github_issue_with(
    gh: &GhRunner,
    clone_url: &str,
    title: &str,
    body: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ProjectPullRequestMergeError::new("github_issues_failed", "Issue title is required."));
    }
    if title.chars().count() > 256 {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "Issue title must be 256 characters or fewer.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))?;
    gh.ensure_auth().map_err(|error| remap_issues_error(error, ""))?;
    let input = issue_json_input(title, body)?;
    let path = format!("/repos/{}/issues", repo.slug());
    let raw: GitHubIssueWire = github_api_json(
        gh,
        "POST",
        &path,
        ISSUE_ITEM_JQ,
        Some(input.path()),
    )?;
    map_issue(&repo, raw).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub returned an invalid created issue.",
        )
    })
}
```

Implement comments with the same authentication/error boundary and no sorting.

```rust
pub(crate) fn list_github_issue_comments_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
) -> Result<Vec<GitHubIssueCommentDto>, ProjectPullRequestMergeError> {
    if number == 0 {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub issue number must be greater than zero.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))?;
    gh.ensure_auth().map_err(|error| remap_issues_error(error, ""))?;
    let path = format!("/repos/{}/issues/{number}/comments?per_page=100", repo.slug());
    let raw: Vec<GitHubIssueCommentWire> = github_api_json(
        gh,
        "GET",
        &path,
        ISSUE_COMMENTS_JQ,
        None,
    )?;
    Ok(raw
        .into_iter()
        .filter_map(|comment| {
            let user = comment.user?;
            if comment.id == 0 || user.login.trim().is_empty() {
                return None;
            }
            let created_at = DateTime::parse_from_rfc3339(&comment.created_at).ok()?.timestamp();
            Some(GitHubIssueCommentDto {
                id: comment.id,
                body: comment.body,
                created_at,
                user: GitHubIssueUserDto { login: user.login, avatar_url: user.avatar_url },
            })
        })
        .collect())
}
```

- [ ] **Step 4: Run all Rust issue tests and verify GREEN**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
```

Expected: all list, create, comment, validation, and error tests pass.

- [ ] **Step 5: Format, inspect scope, and commit**

Run GitNexus `detect_changes({ scope: "staged" })` and confirm only `project_github_issues.rs` changed in this task.

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_issues.rs
git diff --check
git commit -s -m "feat(projects): create GitHub issues and list comments"
```

---

### Task 3: Expose and register the three Tauri commands

**Files:**

- Modify `desktop/src-tauri/src/commands/project_github_issues.rs`.
- Modify `desktop/src-tauri/src/commands/mod.rs`.
- Modify `desktop/src-tauri/src/lib.rs:647-652` and remove the two ratchet-mitigation blank lines near current lines 983 and 988.

**Interfaces:**

- Consumes the three injected-runner functions from Tasks 1 and 2 and `GhRunner::discover`.
- Produces Tauri commands `list_github_issues`, `create_github_issue`, and `list_github_issue_comments` with the exact parameter names required by the TypeScript invoke layer.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `get_github_repository_state`, the `tauri::generate_handler!` registration in `lib.rs`, and `commands` module re-exports.
Report callers, affected processes, and risk before editing.

- [ ] **Step 2: Write failing discovery-wrapper tests**

```rust
#[test]
fn list_wrapper_maps_missing_discovered_cli() {
    let error = list_github_issues_with_runner(
        "https://github.com/acme/app".to_string(),
        "open".to_string(),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}

#[test]
fn create_wrapper_maps_missing_discovered_cli() {
    let error = create_github_issue_with_runner(
        "https://github.com/acme/app".to_string(),
        "title".to_string(),
        "body".to_string(),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}

#[test]
fn comments_wrapper_maps_missing_discovered_cli() {
    let error = list_github_issue_comments_with_runner(
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

Expected: compilation fails because `list_github_issues_with_runner` does not exist.

- [ ] **Step 4: Implement all three sync wrappers and async commands**

```rust
pub(crate) fn list_github_issues_with_runner(
    clone_url: String,
    state: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issues_error(error, ""))?;
    list_github_issues_with(&gh, &clone_url, &state)
}

pub(crate) fn create_github_issue_with_runner(
    clone_url: String,
    title: String,
    body: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issues_error(error, ""))?;
    create_github_issue_with(&gh, &clone_url, &title, &body)
}

pub(crate) fn list_github_issue_comments_with_runner(
    clone_url: String,
    number: u64,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<Vec<GitHubIssueCommentDto>, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issues_error(error, ""))?;
    list_github_issue_comments_with(&gh, &clone_url, number)
}

/// List one page of GitHub issues for a github.com clone URL.
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

/// Create one GitHub issue for a github.com clone URL.
#[tauri::command]
pub async fn create_github_issue(
    clone_url: String,
    title: String,
    body: String,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        create_github_issue_with_runner(clone_url, title, body, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// List the first page of comments for one GitHub issue.
#[tauri::command]
pub async fn list_github_issue_comments(
    clone_url: String,
    number: u64,
) -> Result<Vec<GitHubIssueCommentDto>, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_issue_comments_with_runner(clone_url, number, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}
```

Add `pub use project_github_issues::*;` in `commands/mod.rs`.
Register the three names immediately after `get_github_repository_state` in `lib.rs`.
Remove the two blank lines described in Resolved Implementation Decisions before formatting so the file remains at or below 1,000 lines.

```rust
get_github_repository_state,
list_github_issues,
create_github_issue,
list_github_issue_comments,
get_github_repository_snapshot,
```

- [ ] **Step 5: Run Rust tests and the line-count ratchet**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
. ./bin/activate-hermit && cd desktop && pnpm check:file-sizes
```

Expected: tests pass and `desktop/src-tauri/src/lib.rs` is no more than 1,000 lines.

- [ ] **Step 6: Format, inspect scope, and commit**

Run GitNexus `detect_changes({ scope: "staged" })` and confirm only the issue command wrappers, module re-export, invoke registrations, and two blank-line removals are present.

```bash
. ./bin/activate-hermit
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml
git add desktop/src-tauri/src/commands/project_github_issues.rs \
  desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/lib.rs
git diff --check
git commit -s -m "feat(projects): expose GitHub issue Tauri commands"
```

---

### Task 4: Add TypeScript DTOs, ProjectIssue mapping, and safe share links

**Files:**

- Modify `desktop/src/shared/api/projectGit.ts`.
- Modify `desktop/src/features/projects/projectIssues.mjs`.
- Modify `desktop/src/features/projects/projectIssues.d.mts`.
- Modify `desktop/src/features/projects/projectIssues.test.mjs`.
- Create `desktop/src/features/projects/lib/projectGithubIssues.ts`.
- Create `desktop/src/features/projects/lib/projectGithubIssues.test.mjs`.
- Modify `desktop/src/features/projects/lib/projectShareLinks.ts`.
- Modify `desktop/src/features/projects/lib/projectShareLinks.test.mjs`.

**Interfaces:**

- Consumes `invokeTauri`, `parseProjectPullRequestMergeError`, `isGitHubCloneUrl`, `Repository`, and `ProjectIssue`.
- Produces native DTO wrappers, `{ issues, hasMore }` host routing, GitHub issue/comment mappers, decimal-number display helpers, hex-only identity collection, and safe GitHub issue sharing.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `eventToProjectIssue`, `PROJECT_ISSUE_STATUS`, `issueShareLink`, and `parseProjectPullRequestMergeError`.
Report direct callers, processes, and risk before editing.

- [ ] **Step 2: Write all mapper, routing, declaration-default, identity, number, comment, and share tests before production changes**

Add this Nostr regression to `projectIssues.test.mjs`.

```js
test("Nostr issues expose neutral GitHub extension fields", () => {
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
  assert.equal(issue.status, PROJECT_ISSUE_STATUS.BACKLOG);
  assert.equal(PROJECT_ISSUE_STATUS.OPEN, "Open");
  assert.equal(issue.commentCount, 1);
  assert.equal(issue.htmlUrl, null);
  assert.equal(issue.authorAvatarUrl, null);
  assert.deepEqual(issue.assigneeAvatars, {});
});
```

Create `projectGithubIssues.test.mjs` with a complete DTO and independent literal expectations.

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchProjectIssuesWith,
  issueDisplayNumber,
  issueIdentityPubkeys,
  mapGithubCommentToProjectIssueComment,
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

test("GitHub issue mapper fills the complete ProjectIssue contract", () => {
  const issue = mapGithubIssueToProjectIssue(dto, REPO_ADDRESS);
  assert.deepEqual(issue, {
    id: "42",
    title: "Broken login",
    content: "Steps",
    tags: [],
    author: "ada",
    authorAvatarUrl: "https://avatars.githubusercontent.com/u/1",
    createdAt: 1_704_166_645,
    repoAddress: REPO_ADDRESS,
    channelId: null,
    originAgentName: null,
    labels: ["bug"],
    recipients: [],
    assignees: ["linus"],
    assigneeAvatars: { linus: "https://avatars.githubusercontent.com/u/2" },
    assigneeOperationHeads: {},
    status: "Open",
    statusEventId: null,
    updatedAt: 1_704_253_045,
    comments: [],
    commentCount: 3,
    htmlUrl: "https://github.com/acme/app/issues/42",
  });
});

test("GitHub comment mapper keeps login and avatar without pubkey conversion", () => {
  assert.deepEqual(
    mapGithubCommentToProjectIssueComment({
      id: 9,
      body: "I can reproduce this.",
      created_at: 1_704_253_100,
      user: { login: "grace", avatar_url: "https://avatars.githubusercontent.com/u/3" },
    }),
    {
      id: "9",
      content: "I can reproduce this.",
      tags: [],
      author: "grace",
      authorAvatarUrl: "https://avatars.githubusercontent.com/u/3",
      createdAt: 1_704_253_100,
    },
  );
});

test("host routing invokes only GitHub for github.com", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectIssuesWith(
    { id: "p1", repoAddress: REPO_ADDRESS, cloneUrls: ["https://github.com/acme/app"] },
    {
      loadGithub: async (input) => {
        calls.github += 1;
        assert.deepEqual(input, { cloneUrl: "https://github.com/acme/app", state: "open" });
        return { issues: [dto], has_more: true };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [];
      },
    },
  );
  assert.equal(calls.github, 1);
  assert.equal(calls.buzz, 0);
  assert.equal(result.issues[0].id, "42");
  assert.equal(result.hasMore, true);
});

test("host routing invokes only Nostr for a Buzz clone URL", async () => {
  const calls = { github: 0, buzz: 0 };
  const result = await fetchProjectIssuesWith(
    {
      id: "p2",
      repoAddress: REPO_ADDRESS,
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
    },
    {
      loadGithub: async () => {
        calls.github += 1;
        return { issues: [dto], has_more: false };
      },
      loadBuzz: async () => {
        calls.buzz += 1;
        return [{ id: "e".repeat(64) }];
      },
    },
  );
  assert.equal(calls.github, 0);
  assert.equal(calls.buzz, 1);
  assert.equal(result.issues[0].id, "e".repeat(64));
  assert.equal(result.hasMore, false);
});

test("GitHub issue number parser accepts only positive safe decimal integers", () => {
  assert.equal(parseGithubIssueNumber("42"), 42);
  assert.equal(parseGithubIssueNumber("0"), null);
  assert.equal(parseGithubIssueNumber("01"), null);
  assert.equal(parseGithubIssueNumber("0x2"), null);
  assert.equal(parseGithubIssueNumber("9007199254740992"), null);
  assert.equal(parseGithubIssueNumber("e".repeat(64)), null);
  assert.equal(issueDisplayNumber("42"), "42");
  assert.equal(issueDisplayNumber("e".repeat(64)), "eeeeeeee");
});

test("identity collection drops GitHub logins and keeps lowercase Nostr pubkeys", () => {
  const github = mapGithubIssueToProjectIssue(dto, REPO_ADDRESS);
  const nostr = {
    ...github,
    id: "f".repeat(64),
    author: "A".repeat(64),
    recipients: ["B".repeat(64)],
    assignees: ["C".repeat(64)],
    comments: [{ ...mapGithubCommentToProjectIssueComment({ id: 1, body: "x", created_at: 1, user: { login: "x", avatar_url: "" } }), author: "D".repeat(64) }],
  };
  assert.deepEqual(issueIdentityPubkeys([github]), []);
  assert.deepEqual(issueIdentityPubkeys([nostr]), ["a".repeat(64), "b".repeat(64), "c".repeat(64), "d".repeat(64)]);
});
```

Add these share-link cases to `projectShareLinks.test.mjs`.

```js
test("issueShareLink accepts only a canonical GitHub issue URL", () => {
  const base = { id: "42", repoAddress: REPO_ADDRESS };
  assert.equal(
    issueShareLink({ ...base, htmlUrl: "https://github.com/acme/app/issues/42" }),
    "https://github.com/acme/app/issues/42",
  );
  for (const htmlUrl of [
    "https://evil.example/acme/app/issues/42",
    "https://github.com/acme/app/issues/42?x=1",
    "https://github.com/acme/app/issues/42#x",
    "https://github.com/acme/app/issues/42/",
    "https://github.com/acme/app/pull/42",
  ]) {
    assert.equal(issueShareLink({ ...base, htmlUrl }), null, htmlUrl);
  }
});

test("issueShareLink preserves the existing Buzz deep link", () => {
  assert.equal(
    issueShareLink({ id: EVENT_ID, repoAddress: REPO_ADDRESS, htmlUrl: null }),
    `buzz://issue?id=${EVENT_ID}&owner=${OWNER}&d=flappy-bee`,
  );
});
```

- [ ] **Step 3: Run and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/projectIssues.test.mjs src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectShareLinks.test.mjs
```

Expected: the new module import or `PROJECT_ISSUE_STATUS.OPEN` is missing.

- [ ] **Step 4: Add exact DTOs and invoke wrappers**

Add these types and documented functions to `projectGit.ts`.

```ts
/** GitHub login identity returned by the native issue commands. */
export type GithubIssueUserDto = { login: string; avatar_url: string };

/** Bounded GitHub issue returned by the native issue commands. */
export type GithubIssueDto = {
  number: number;
  title: string;
  body: string;
  state: "open" | "closed";
  html_url: string;
  comments: number;
  created_at: number;
  updated_at: number;
  user: GithubIssueUserDto;
  labels: string[];
  assignees: GithubIssueUserDto[];
};

/** One bounded GitHub issue page. */
export type GithubIssueListDto = { issues: GithubIssueDto[]; has_more: boolean };

/** One read-only GitHub issue comment. */
export type GithubIssueCommentDto = {
  id: number;
  body: string;
  created_at: number;
  user: GithubIssueUserDto;
};

/** List the first GitHub issue page for a github.com clone URL. */
export async function listGithubIssues(input: {
  cloneUrl: string;
  state: "open" | "closed";
}): Promise<GithubIssueListDto> {
  try {
    return await invokeTauri<GithubIssueListDto>("list_github_issues", input);
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** Create one GitHub issue for a github.com clone URL. */
export async function createGithubIssue(input: {
  cloneUrl: string;
  title: string;
  body: string;
}): Promise<GithubIssueDto> {
  try {
    return await invokeTauri<GithubIssueDto>("create_github_issue", input);
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}

/** List the first read-only comment page for one GitHub issue. */
export async function listGithubIssueComments(input: {
  cloneUrl: string;
  number: number;
}): Promise<GithubIssueCommentDto[]> {
  try {
    return await invokeTauri<GithubIssueCommentDto[]>(
      "list_github_issue_comments",
      input,
    );
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}
```

- [ ] **Step 5: Align the runtime object and its declaration file**

Add `OPEN: "Open"` to `PROJECT_ISSUE_STATUS` in both `.mjs` and `.d.mts`.
Add `"Open"` to the `ProjectIssueStatus` union.
Add these fields to `ProjectIssue` in `projectIssues.d.mts`.

```ts
authorAvatarUrl: string | null;
assigneeAvatars: Record<string, string>;
commentCount: number;
htmlUrl: string | null;
```

Add `authorAvatarUrl?: string | null` to `ProjectIssueComment` because only GitHub comments carry it.
Populate neutral values in `eventToProjectIssue`.

```js
return {
  // existing fields stay unchanged
  authorAvatarUrl: null,
  assigneeAvatars: {},
  commentCount: comments.length,
  htmlUrl: null,
};
```

- [ ] **Step 6: Implement host routing and DTO mapping in the new helper**

Use these exact public functions and result shape.

```ts
import type { Repository } from "@/features/projects/projectModels";
import type { ProjectIssue } from "@/features/projects/projectIssues.mjs";
import {
  type GithubIssueCommentDto,
  type GithubIssueDto,
  type GithubIssueListDto,
} from "@/shared/api/projectGit";
import { isGitHubCloneUrl } from "./projectGitError";

/** Host-routed issue list consumed by the repository Issues tab. */
export type ProjectIssuesResult = { issues: ProjectIssue[]; hasMore: boolean };

/** Map a bounded native GitHub issue onto the shared Projects issue model. */
export function mapGithubIssueToProjectIssue(
  dto: GithubIssueDto,
  repoAddress: string,
): ProjectIssue {
  return {
    id: String(dto.number),
    title: dto.title,
    content: dto.body ?? "",
    tags: [],
    author: dto.user.login,
    authorAvatarUrl: dto.user.avatar_url,
    createdAt: dto.created_at,
    repoAddress,
    channelId: null,
    originAgentName: null,
    labels: [...dto.labels],
    recipients: [],
    assignees: dto.assignees.map((assignee) => assignee.login),
    assigneeAvatars: Object.fromEntries(
      dto.assignees.map((assignee) => [assignee.login, assignee.avatar_url]),
    ),
    assigneeOperationHeads: {},
    status: dto.state === "closed" ? "Closed" : "Open",
    statusEventId: null,
    updatedAt: dto.updated_at,
    comments: [],
    commentCount: dto.comments,
    htmlUrl: dto.html_url,
  };
}

/** Map a bounded native GitHub comment without interpreting its login as a pubkey. */
export function mapGithubCommentToProjectIssueComment(
  dto: GithubIssueCommentDto,
): ProjectIssue["comments"][number] {
  return {
    id: String(dto.id),
    content: dto.body ?? "",
    tags: [],
    author: dto.user.login,
    authorAvatarUrl: dto.user.avatar_url,
    createdAt: dto.created_at,
  };
}

/** Route one repository to exactly one issue backend. */
export async function fetchProjectIssuesWith(
  project: Pick<Repository, "id" | "repoAddress" | "cloneUrls">,
  loaders: {
    loadGithub: (input: {
      cloneUrl: string;
      state: "open";
    }) => Promise<GithubIssueListDto>;
    loadBuzz: () => Promise<ProjectIssue[]>;
  },
): Promise<ProjectIssuesResult> {
  const cloneUrl = project.cloneUrls[0] ?? "";
  if (isGitHubCloneUrl(cloneUrl)) {
    const page = await loaders.loadGithub({ cloneUrl, state: "open" });
    return {
      issues: page.issues.map((issue) =>
        mapGithubIssueToProjectIssue(issue, project.repoAddress),
      ),
      hasMore: page.has_more === true,
    };
  }
  return { issues: await loaders.loadBuzz(), hasMore: false };
}

/** Parse a positive GitHub issue number that is safe in JavaScript. */
export function parseGithubIssueNumber(value: string | null | undefined): number | null {
  if (!value || !/^[1-9][0-9]*$/.test(value)) return null;
  const number = Number(value);
  return Number.isSafeInteger(number) ? number : null;
}

/** Display a full GitHub number or the existing eight-character Nostr prefix. */
export function issueDisplayNumber(issueId: string): string {
  return parseGithubIssueNumber(issueId) === null ? issueId.slice(0, 8) : issueId;
}

/** Collect only valid Nostr identities for profile batch lookup. */
export function issueIdentityPubkeys(issues: ProjectIssue[]): string[] {
  const values = issues.flatMap((issue) => [
    issue.author,
    ...issue.recipients,
    ...issue.assignees,
    ...issue.comments.map((comment) => comment.author),
  ]);
  return [...new Set(values.filter((value) => /^[a-fA-F0-9]{64}$/.test(value)).map((value) => value.toLowerCase()))];
}
```

Task 5 adds the comment request helper and hook to this same file after their tests fail.

- [ ] **Step 7: Implement strict GitHub URL sharing without exporting the validator**

Add a private validator to `projectShareLinks.ts`.

```ts
function isSafeGitHubIssueUrl(raw: string): boolean {
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
    const [owner, repo, segment, number, ...rest] = url.pathname.split("/").filter(Boolean);
    return (
      rest.length === 0 &&
      segment === "issues" &&
      /^[A-Za-z0-9-]+$/.test(owner ?? "") &&
      /^[A-Za-z0-9._-]+$/.test(repo ?? "") &&
      /^[1-9][0-9]*$/.test(number ?? "") &&
      raw === `https://github.com/${owner}/${repo}/issues/${number}`
    );
  } catch {
    return false;
  }
}

export function issueShareLink(issue: ProjectIssue): string | null {
  if (issue.htmlUrl && isSafeGitHubIssueUrl(issue.htmlUrl)) {
    return issue.htmlUrl;
  }
  const coordinate = repositoryCoordinate(issue.repoAddress);
  return coordinate &&
    HEX64_RE.test(issue.id) &&
    isLinkableCoordinate(coordinate.owner, coordinate.dtag)
    ? buildIssueLink({ ...coordinate, id: issue.id })
    : null;
}
```

- [ ] **Step 8: Run tests, typecheck, and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/projectIssues.test.mjs src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectShareLinks.test.mjs && pnpm typecheck
```

Expected: all tests pass and TypeScript accepts the `.mjs` declaration changes.

- [ ] **Step 9: Format, inspect scope, and commit**

Run GitNexus `detect_changes({ scope: "staged" })` and confirm only DTO, model, helper, declaration, share-link, and test symbols are affected.

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/shared/api/projectGit.ts \
  src/features/projects/projectIssues.mjs \
  src/features/projects/projectIssues.d.mts \
  src/features/projects/projectIssues.test.mjs \
  src/features/projects/lib/projectGithubIssues.ts \
  src/features/projects/lib/projectGithubIssues.test.mjs \
  src/features/projects/lib/projectShareLinks.ts \
  src/features/projects/lib/projectShareLinks.test.mjs
git add src/shared/api/projectGit.ts \
  src/features/projects/projectIssues.mjs \
  src/features/projects/projectIssues.d.mts \
  src/features/projects/projectIssues.test.mjs \
  src/features/projects/lib/projectGithubIssues.ts \
  src/features/projects/lib/projectGithubIssues.test.mjs \
  src/features/projects/lib/projectShareLinks.ts \
  src/features/projects/lib/projectShareLinks.test.mjs
git diff --check
git commit -s -m "feat(projects): map GitHub issues onto ProjectIssue"
```

---

### Task 5: Route the query, create mutation, invalidations, comments, and profile lookup

**Files:**

- Modify `desktop/src/features/projects/hooks.ts`.
- Modify `desktop/src/features/projects/issueMutations.ts`.
- Create `desktop/src/features/projects/issueMutations.test.mjs`.
- Modify `desktop/src/features/projects/lib/projectGithubIssues.ts`.
- Modify `desktop/src/features/projects/lib/projectGithubIssues.test.mjs`.
- Modify `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`.
- Modify `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx` only for the new query result shape.

**Interfaces:**

- Consumes `fetchProjectIssuesWith`, native list/create/comments wrappers, number parsing, comment mapping, and identity filtering.
- Produces `useProjectIssuesQuery` returning `{ issues, hasMore }`, `createProjectIssueWith`, `projectIssueInvalidationKeys`, `githubIssueCommentsRequest`, and `useGithubIssueCommentsQuery`.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `fetchProjectIssues`, `useProjectIssuesQuery`, `publishProjectIssue`, `useCreateProjectIssueMutation`, `ProjectDetailScreen`, and `ProjectIssuesPanel`.
Report direct callers, processes, and risk before editing.

- [ ] **Step 2: Write failing create-routing and invalidation tests**

Create `issueMutations.test.mjs`.

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  createProjectIssueWith,
  projectIssueInvalidationKeys,
} from "./issueMutations.ts";

const REPO_ADDRESS = `30617:${"a".repeat(64)}:app`;

test("GitHub create never calls the Buzz issue publisher", async () => {
  const calls = { github: 0, buzz: 0 };
  const id = await createProjectIssueWith(
    {
      id: "p1",
      owner: "a".repeat(64),
      repoAddress: REPO_ADDRESS,
      cloneUrls: ["https://github.com/acme/app"],
    },
    { title: "Broken login", body: "steps" },
    {
      createGithub: async (input) => {
        calls.github += 1;
        assert.deepEqual(input, {
          cloneUrl: "https://github.com/acme/app",
          title: "Broken login",
          body: "steps",
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

test("Buzz create never calls the GitHub creator", async () => {
  const calls = { github: 0, buzz: 0 };
  const id = await createProjectIssueWith(
    {
      id: "p2",
      owner: "a".repeat(64),
      repoAddress: REPO_ADDRESS,
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
  assert.deepEqual(calls, { github: 0, buzz: 1 });
});

test("GitHub create invalidates only its repository issue query", () => {
  assert.deepEqual(
    projectIssueInvalidationKeys({
      id: "p1",
      cloneUrls: ["https://github.com/acme/app"],
    }),
    [["project", "p1", "issues"]],
  );
});

test("Buzz create preserves all existing invalidations", () => {
  assert.deepEqual(
    projectIssueInvalidationKeys({
      id: "p2",
      cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`],
    }),
    [
      ["project", "p2", "issues"],
      ["projects", "work-items"],
      ["projects", "activity-summaries"],
    ],
  );
});
```

- [ ] **Step 3: Add failing comment-request tests**

Append to `projectGithubIssues.test.mjs`.

```js
test("GitHub comment request validates host, number, and exact query key", () => {
  assert.deepEqual(
    githubIssueCommentsRequest(
      { id: "p1", cloneUrls: ["https://github.com/acme/app"] },
      "42",
    ),
    {
      cloneUrl: "https://github.com/acme/app",
      number: 42,
      queryKey: ["project", "p1", "issues", 42, "comments"],
    },
  );
  assert.equal(
    githubIssueCommentsRequest(
      { id: "p1", cloneUrls: ["https://github.com/acme/app"] },
      "0",
    ),
    null,
  );
  assert.equal(
    githubIssueCommentsRequest(
      { id: "p2", cloneUrls: [`https://relay.example/git/${"ab".repeat(32)}/app`] },
      "42",
    ),
    null,
  );
});
```

Import `githubIssueCommentsRequest` in that test file.

- [ ] **Step 4: Run and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/issueMutations.test.mjs src/features/projects/lib/projectGithubIssues.test.mjs
```

Expected: `createProjectIssueWith` or `githubIssueCommentsRequest` is missing.

- [ ] **Step 5: Implement host-aware query routing**

Rename the current private `fetchProjectIssues` to `fetchBuzzProjectIssues` without changing its Nostr filters or assignment-history behavior.
Keep it returning `Promise<ProjectIssue[]>`.
Change only the query function and return shape at the hook boundary.

```ts
export function useProjectIssuesQuery(project: Repository | null | undefined) {
  return useQuery({
    enabled: Boolean(project),
    queryKey: ["project", project?.id ?? "none", "issues"],
    queryFn: () => {
      if (!project) throw new Error("No project selected.");
      return fetchProjectIssuesWith(project, {
        loadGithub: listGithubIssues,
        loadBuzz: () => fetchBuzzProjectIssues(project),
      });
    },
    staleTime: 30_000,
  });
}
```

Add imports for `fetchProjectIssuesWith` and `listGithubIssues`.
Do not modify `fetchProjectsWorkItems`.

- [ ] **Step 6: Implement create routing and invalidation selection**

Keep `publishProjectIssue` unchanged.
Add documented helpers and make the hook use them.

```ts
/** Create an issue through exactly one repository-native backend. */
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
): Promise<string> {
  const cloneUrl = project.cloneUrls[0] ?? "";
  if (isGitHubCloneUrl(cloneUrl)) {
    const issue = await loaders.createGithub({
      cloneUrl,
      title: input.title,
      body: input.body,
    });
    return String(issue.number);
  }
  return loaders.publishBuzz(project, input);
}

/** Query keys invalidated after a repository-native issue create. */
export function projectIssueInvalidationKeys(
  project: Pick<Project, "id" | "cloneUrls">,
): unknown[][] {
  const keys: unknown[][] = [["project", project.id, "issues"]];
  if (!isGitHubCloneUrl(project.cloneUrls[0])) {
    keys.push(["projects", "work-items"], ["projects", "activity-summaries"]);
  }
  return keys;
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
      if (!project) return;
      await Promise.all(
        projectIssueInvalidationKeys(project).map((queryKey) =>
          queryClient.invalidateQueries({ queryKey }),
        ),
      );
    },
  });
}
```

- [ ] **Step 7: Implement validated comment request construction and the query hook**

Append this to `projectGithubIssues.ts`.
Add `useQuery` from `@tanstack/react-query` and `listGithubIssueComments` from `@/shared/api/projectGit` to the imports in the same edit.

```ts
/** Resolve a valid GitHub comment request and its cache key. */
export function githubIssueCommentsRequest(
  project: Pick<Repository, "id" | "cloneUrls"> | null | undefined,
  selectedIssueId: string | null | undefined,
): {
  cloneUrl: string;
  number: number;
  queryKey: readonly ["project", string, "issues", number, "comments"];
} | null {
  const cloneUrl = project?.cloneUrls[0] ?? "";
  const number = parseGithubIssueNumber(selectedIssueId);
  if (!project || !isGitHubCloneUrl(cloneUrl) || number === null) return null;
  return {
    cloneUrl,
    number,
    queryKey: ["project", project.id, "issues", number, "comments"],
  };
}

/** Load the first read-only GitHub comment page for the selected numeric issue. */
export function useGithubIssueCommentsQuery(
  project: Pick<Repository, "id" | "cloneUrls"> | null | undefined,
  selectedIssueId: string | null | undefined,
) {
  const request = githubIssueCommentsRequest(project, selectedIssueId);
  return useQuery({
    enabled: request !== null,
    queryKey:
      request?.queryKey ??
      ["project", project?.id ?? "none", "issues", "none", "comments"],
    queryFn: async () => {
      if (!request) throw new Error("No GitHub issue selected.");
      const comments = await listGithubIssueComments({
        cloneUrl: request.cloneUrl,
        number: request.number,
      });
      return comments.map(mapGithubCommentToProjectIssueComment);
    },
    staleTime: 30_000,
  });
}
```

- [ ] **Step 8: Update the two query-data consumers and profile lookup**

In `ProjectDetailScreen.tsx`, replace both array reads.

```ts
const issuePubkeys = issueIdentityPubkeys(issuesQuery.data?.issues ?? []);

const selectedIssue =
  issuesQuery.data?.issues.find((item) => item.id === selectedIssueId) ?? null;
```

Keep the existing project and pull-request identity collection around `issuePubkeys` unchanged.
In `ProjectIssuesPanel.tsx`, change only the local data read in this task.

```ts
const issues = issuesQuery.data?.issues ?? [];
```

- [ ] **Step 9: Run tests and typecheck and verify GREEN**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/issueMutations.test.mjs src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/projectIssues.test.mjs && pnpm typecheck
```

Expected: host routing, invalidations, comment request construction, old Nostr mapping, and all TypeScript consumers pass.

- [ ] **Step 10: Format, inspect scope, and commit**

Run GitNexus `detect_changes({ scope: "staged" })` and confirm only repository issue query/create/comment flows and their two consumers changed.

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/features/projects/hooks.ts \
  src/features/projects/issueMutations.ts \
  src/features/projects/issueMutations.test.mjs \
  src/features/projects/lib/projectGithubIssues.ts \
  src/features/projects/lib/projectGithubIssues.test.mjs \
  src/features/projects/ui/ProjectDetailScreen.tsx \
  src/features/projects/ui/ProjectIssuesPanel.tsx
git add src/features/projects/hooks.ts \
  src/features/projects/issueMutations.ts \
  src/features/projects/issueMutations.test.mjs \
  src/features/projects/lib/projectGithubIssues.ts \
  src/features/projects/lib/projectGithubIssues.test.mjs \
  src/features/projects/ui/ProjectDetailScreen.tsx \
  src/features/projects/ui/ProjectIssuesPanel.tsx
git diff --check
git commit -s -m "feat(projects): route GitHub issue reads and creates"
```

---

### Task 6: Add GitHub-only issue UI and exercise it through the mock bridge

**Files:**

- Modify `desktop/src/testing/e2eBridge.ts`.
- Create `desktop/tests/e2e/github-issues.spec.ts`.
- Modify `desktop/playwright.config.ts`.
- Modify `desktop/src/features/projects/ui/GitHubRepoStateRecovery.tsx`.
- Create `desktop/src/features/projects/ui/GitHubProjectIssues.tsx`.
- Modify `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx`.
- Modify `desktop/src/features/projects/ui/ProjectIssueCommentTimeline.tsx`.

**Interfaces:**

- Consumes `{ issues, hasMore }`, `useGithubIssueCommentsQuery`, `issueDisplayNumber`, `issueShareLink`, `GitHubRepoStateRecovery`, `ProjectFeedRow`, and `ProjectRichContent`.
- Produces a host-gated GitHub list/detail flow that never executes Buzz pubkey, assignment, discussed-channel, or comment-composer code.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `ProjectIssuesPanel`, `ProjectIssueDetail`, `IssueRow`, `ProjectIssueCommentTimeline`, `GitHubRepoStateRecovery`, the E2E invoke switch, and the Playwright smoke `testMatch` list.
Report direct callers, processes, and risk before editing.

- [ ] **Step 2: Add E2E state and native-command stubs before changing the UI**

Add these `Window` fields beside the existing GitHub E2E flags.

```ts
/** Structured error returned by GitHub issue mock commands. */
__BUZZ_E2E_GITHUB_ISSUES_ERROR__?: { code: string; message: string };
/** Structured error returned only by the GitHub comment mock command. */
__BUZZ_E2E_GITHUB_ISSUE_COMMENTS_ERROR__?: { code: string; message: string };
/** Created GitHub issue DTOs retained for later list calls. */
__BUZZ_E2E_GITHUB_CREATED_ISSUES__?: Array<Record<string, unknown>>;
```

Set `window.__BUZZ_E2E_GITHUB_CREATED_ISSUES__ = [];` beside the existing mock command-log resets in `maybeInstallE2eTauriMocks` so repeated bridge installs start deterministically.

Add these switch cases beside `get_github_repository_state` and before the switch default.
The list stub rejects any state other than `open`, which protects this slice's fixed filter.

```ts
case "list_github_issues": {
  if (window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__) {
    throw window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__;
  }
  const input = payload as { state?: string };
  if (input.state !== "open") throw new Error("Expected GitHub state=open");
  return {
    issues: [
      {
        number: 42,
        title: "Broken login",
        body: "Repro steps",
        state: "open",
        html_url: "https://github.com/acme/app/issues/42",
        comments: 2,
        created_at: 1_704_166_645,
        updated_at: 1_704_253_045,
        user: { login: "ada", avatar_url: "" },
        labels: ["bug"],
        assignees: [{ login: "linus", avatar_url: "" }],
      },
      ...(window.__BUZZ_E2E_GITHUB_CREATED_ISSUES__ ?? []),
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
    created_at: 1_704_253_100,
    updated_at: 1_704_253_100,
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
  if (window.__BUZZ_E2E_GITHUB_ISSUE_COMMENTS_ERROR__) {
    throw window.__BUZZ_E2E_GITHUB_ISSUE_COMMENTS_ERROR__;
  }
  return [
    {
      id: 2,
      body: "API-order first comment.",
      created_at: 1_704_253_100,
      user: { login: "grace", avatar_url: "" },
    },
    {
      id: 10,
      body: "API-order second comment.",
      created_at: 1_704_253_100,
      user: { login: "linus", avatar_url: "" },
    },
  ];
}
```

- [ ] **Step 3: Write and register the failing Playwright contract**

Create `github-issues.spec.ts`.
Call `page.addInitScript` before `installMockBridge(page)`.
Do not call `waitForAnimations` because this spec does not capture screenshots.

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
    .locator('[data-testid="project-card-buzz"], [data-testid="project-row-buzz"]')
    .first();
  await expect(projectEntry).toBeVisible({ timeout: 10_000 });
  await projectEntry.click();
}

async function openGithubIssues(page: import("@playwright/test").Page) {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ = "https://github.com/acme/app";
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
}

test("GitHub Issues lists metadata, loads read-only detail, and creates #N", async ({ page }) => {
  await openGithubIssues(page);
  const row = page.getByTestId("project-github-issue-row").first();
  await expect(row).toContainText("#42");
  await expect(row).toContainText("Open");
  await expect(row).toContainText("ada");
  await expect(row).toContainText("bug");
  await expect(row.getByLabel("Assigned to linus")).toBeVisible();

  await row.getByRole("button", { name: "#42", exact: true }).click();
  await expect(page.getByText("Repro steps", { exact: true })).toBeVisible();
  const comments = page.getByTestId("project-issue-comment-timeline-row");
  await expect(comments).toHaveCount(2);
  await expect(comments.nth(0)).toContainText("API-order first comment.");
  await expect(comments.nth(1)).toContainText("API-order second comment.");
  await expect(page.getByText("grace", { exact: true })).toBeVisible();
  await expect(page.getByTestId("project-issue-comment-composer")).toHaveCount(0);
  await expect(page.getByTestId("issue-discussed-in")).toHaveCount(0);
  await expect(page.getByTestId("project-issue-assign")).toHaveCount(0);

  await page.getByRole("button", { name: "New issue" }).click();
  await page.getByTestId("create-issue-title").fill("New GitHub bug");
  await page.getByTestId("create-issue-body").fill("Created from Buzz");
  await page.getByTestId("create-issue-submit").click();
  await expect(page.getByText("New GitHub bug", { exact: true })).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText("#43", { exact: true })).toBeVisible();

  const commands = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(commands).toContain("list_github_issues");
  expect(commands).toContain("list_github_issue_comments");
  expect(commands).toContain("create_github_issue");
});

test("GitHub issue auth failure renders recovery before empty state", async ({ page }) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ = "https://github.com/acme/app";
    window.__BUZZ_E2E_GITHUB_ISSUES_ERROR__ = {
      code: "github_auth_required",
      message: "Authenticate GitHub CLI with: gh auth login --hostname github.com",
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  await expect(page.getByText("GitHub authentication required")).toBeVisible({ timeout: 10_000 });
  await expect(page.getByText("No issues yet.")).toHaveCount(0);
  await expect(page.getByText("No open issues.")).toHaveCount(0);
  await expect(page.getByTestId("project-github-issue-row")).toHaveCount(0);
});

test("GitHub comment failure keeps the issue body and retries only comments", async ({ page }) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ = "https://github.com/acme/app";
    window.__BUZZ_E2E_GITHUB_ISSUE_COMMENTS_ERROR__ = {
      code: "github_issues_failed",
      message: "Comment request failed.",
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  const row = page.getByTestId("project-github-issue-row").first();
  await row.getByRole("button", { name: "#42", exact: true }).click();
  await expect(page.getByText("Repro steps", { exact: true })).toBeVisible();
  await expect(page.getByText("Could not load GitHub comments", { exact: true })).toBeVisible();
  await expect(page.getByText("Could not load GitHub issues", { exact: true })).toHaveCount(0);
  await expect(page.getByTestId("project-issue-comment-composer")).toHaveCount(0);
  const before = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  const listCallsBefore = before.filter((command) => command === "list_github_issues").length;
  const commentCallsBefore = before.filter(
    (command) => command === "list_github_issue_comments",
  ).length;
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(page.getByText("Could not load GitHub comments", { exact: true })).toBeVisible();
  const after = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(after.filter((command) => command === "list_github_issues").length).toBe(
    listCallsBefore,
  );
  expect(
    after.filter((command) => command === "list_github_issue_comments").length,
  ).toBe(commentCallsBefore + 1);
});
```

Add `"**/github-issues.spec.ts"` next to `github-repo-state.spec.ts` in the smoke `testMatch` list.

- [ ] **Step 4: Run the new smoke spec and verify RED**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts
```

Expected: the app still renders the Buzz issue path, so the GitHub row or recovery assertion fails.

- [ ] **Step 5: Generalize recovery titles without changing CLI/auth titles**

Implement optional `unavailableTitle` and `titleId` props with branch-compatible defaults.

```tsx
function githubStateErrorTitle(
  code: string | undefined,
  unavailableTitle: string,
): string {
  switch (code) {
    case "github_cli_missing":
      return "GitHub CLI is required";
    case "github_auth_required":
      return "GitHub authentication required";
    default:
      return unavailableTitle;
  }
}

export function GitHubRepoStateRecovery({
  error,
  onRetry,
  titleId = "github-repo-state-recovery-title",
  unavailableTitle = "Could not load GitHub branches",
}: {
  error?: unknown;
  onRetry?: () => void;
  titleId?: string;
  unavailableTitle?: string;
}) {
  const parsed = parseProjectPullRequestMergeError(error);
  const code = parsed?.code;
  const message =
    parsed?.message ??
    (error instanceof Error ? error.message : unavailableTitle);
  const title = githubStateErrorTitle(code, unavailableTitle);
  // Keep the existing install, auth-copy, and Retry JSX unchanged.
  // Replace both hard-coded heading IDs with titleId.
}
```

The implementation must treat `github_repo_unavailable`, `github_state_failed`, `github_issues_failed`, and unknown errors as `unavailableTitle`.

- [ ] **Step 6: Add the focused GitHub list/detail component**

Create `GitHubProjectIssues.tsx` and keep all login-only rendering in this file.
Use `UserAvatar` directly and never import a pubkey helper or Nostr write hook.

```tsx
import { CircleDot, MessageSquare } from "lucide-react";

import type { ProjectIssue, Repository } from "@/features/projects/hooks";
import {
  issueDisplayNumber,
  useGithubIssueCommentsQuery,
} from "@/features/projects/lib/projectGithubIssues";
import { issueShareLink } from "@/features/projects/lib/projectShareLinks";
import { relativeTime } from "@/features/projects/lib/projectsViewHelpers";
import { UserAvatar } from "@/shared/ui/UserAvatar";
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

function GitHubLoginIdentity({
  avatarUrl,
  login,
  showLabel = true,
}: {
  avatarUrl?: string | null;
  login: string;
  showLabel?: boolean;
}) {
  return (
    <span className="inline-flex min-w-0 items-center gap-1.5">
      <UserAvatar avatarUrl={avatarUrl || null} displayName={login} size="xs" />
      {showLabel ? <span className="truncate text-xs">{login}</span> : null}
    </span>
  );
}

function GitHubAssigneeFacepile({ issue }: { issue: ProjectIssue }) {
  if (issue.assignees.length === 0) return null;
  return (
    <div
      aria-label={`Assigned to ${issue.assignees.join(", ")}`}
      className="flex -space-x-1.5"
    >
      {issue.assignees.slice(0, 3).map((login) => (
        <UserAvatar
          avatarUrl={issue.assigneeAvatars[login] || null}
          className="ring-2 ring-background"
          displayName={login}
          key={login}
          size="xs"
        />
      ))}
    </div>
  );
}
```

Export a row whose observable contract matches the E2E selectors.

```tsx
/** One GitHub issue list row with numeric identity and login metadata. */
export function GitHubIssueRow({
  issue,
  onOpen,
}: {
  issue: ProjectIssue;
  onOpen: () => void;
}) {
  const number = issueDisplayNumber(issue.id);
  return (
    <ProjectFeedRow
      eventId={issue.id}
      meta={
        <>
          <GitHubLoginIdentity
            avatarUrl={issue.authorAvatarUrl}
            login={issue.author}
          />
          <span>·</span>
          <span>Open</span>
          {issue.labels.map((label) => (
            <span className="rounded-full border border-border/60 px-1.5 py-0.5 text-2xs" key={label}>
              {label}
            </span>
          ))}
        </>
      }
      onOpen={onOpen}
      statusIcon={<CircleDot className="h-3.5 w-3.5 shrink-0 text-green-500" />}
      testId="project-github-issue-row"
      title={issue.title}
      trailing={
        <>
          <GitHubAssigneeFacepile issue={issue} />
          {issue.commentCount > 0 ? (
            <span className="flex items-center gap-1 text-xs text-muted-foreground">
              <MessageSquare className="h-3.5 w-3.5" />
              {issue.commentCount}
            </span>
          ) : null}
          <ProjectFeedRowCluster>
            <ProjectFeedRowMonoCell
              label={`#${number}`}
              onClick={onOpen}
              title={`View GitHub issue #${number}`}
            />
          </ProjectFeedRowCluster>
          <span className="hidden w-20 shrink-0 text-right text-xs text-muted-foreground sm:block">
            {relativeTime(issue.createdAt)}
          </span>
        </>
      }
    />
  );
}
```

Export a detail component that calls only the GitHub comment query.
Render the existing body immediately, then render comment loading, comment recovery, or the mapped timeline beneath it.

```tsx
/** Read-only GitHub issue detail with an independently retryable comment query. */
export function GitHubIssueDetail({
  issue,
  project,
}: {
  issue: ProjectIssue;
  project: Repository;
}) {
  const commentsQuery = useGithubIssueCommentsQuery(project, issue.id);
  const number = issueDisplayNumber(issue.id);
  return (
    <div className="grid xl:grid-cols-[minmax(0,1fr)_18rem]">
      <div className="min-w-0">
        <header className="space-y-3 p-4">
          <p className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
            <CircleDot className="h-3.5 w-3.5 text-green-500" />
            Issue from {issue.author}
          </p>
          <h3 className="mt-1 line-clamp-2 text-base font-semibold text-foreground">
            {issue.title}{" "}
            <span className="font-normal text-muted-foreground">#{number}</span>
            <ShareLinkButton
              className="ml-1 inline-flex h-6 w-6 align-text-bottom"
              label="Copy issue link"
              link={issueShareLink(issue)}
              testId="project-issue-copy-link"
            />
          </h3>
          {issue.content ? <ProjectRichContent content={issue.content} tags={[]} /> : null}
        </header>
        <section className="space-y-3 p-4">
          {commentsQuery.isLoading ? (
            <p className="text-sm text-muted-foreground">Loading comments…</p>
          ) : commentsQuery.isError ? (
            <GitHubRepoStateRecovery
              error={commentsQuery.error}
              onRetry={() => void commentsQuery.refetch()}
              titleId="github-issue-comments-recovery-title"
              unavailableTitle="Could not load GitHub comments"
            />
          ) : (
            <ProjectIssueCommentTimeline
              comments={commentsQuery.data ?? []}
              githubMode
              key={issue.id}
            />
          )}
        </section>
      </div>
      <aside className="space-y-6 border-t border-border/60 p-4 xl:border-l xl:border-t-0">
        <OverviewRailSection title="Status">
          <span className="inline-flex items-center gap-1.5 text-xs font-medium text-green-500">
            <CircleDot className="h-3.5 w-3.5" /> Open
          </span>
        </OverviewRailSection>
        {issue.assignees.length > 0 ? (
          <OverviewRailSection title="Assignees">
            <div className="space-y-2">
              {issue.assignees.map((login) => (
                <GitHubLoginIdentity
                  avatarUrl={issue.assigneeAvatars[login]}
                  key={login}
                  login={login}
                />
              ))}
            </div>
          </OverviewRailSection>
        ) : null}
        <OverviewRailSection title="Author">
          <GitHubLoginIdentity avatarUrl={issue.authorAvatarUrl} login={issue.author} />
        </OverviewRailSection>
        {issue.labels.length > 0 ? (
          <OverviewRailSection title="Labels">
            <div className="flex flex-wrap gap-1.5">
              {issue.labels.map((label) => (
                <span className="rounded-full border border-border/60 px-1.5 py-0.5 text-2xs text-muted-foreground" key={label}>
                  {label}
                </span>
              ))}
            </div>
          </OverviewRailSection>
        ) : null}
        <OverviewRailSection title="Activity">
          <dl className="space-y-1.5 text-xs text-muted-foreground">
            <div className="flex items-center justify-between gap-3">
              <dt>Created</dt>
              <dd className="font-medium text-foreground">{relativeTime(issue.createdAt)}</dd>
            </div>
            <div className="flex items-center justify-between gap-3">
              <dt>Updated</dt>
              <dd className="font-medium text-foreground">{relativeTime(issue.updatedAt)}</dd>
            </div>
          </dl>
        </OverviewRailSection>
      </aside>
    </div>
  );
}
```

Do not import or render `DiscussedInChannels`, `ForumComposer`, `IssueAssigneesRow`, or `useCreateProjectIssueCommentMutation` in this file.

- [ ] **Step 7: Branch once in ProjectIssuesPanel before Buzz identity/detail code executes**

Import `isGitHubCloneUrl`, `GitHubRepoStateRecovery`, `GitHubIssueRow`, and `GitHubIssueDetail`.
Use this render order.

```tsx
const issuesQuery = useProjectIssuesQuery(project);
const issues = issuesQuery.data?.issues ?? [];
const githubHosted = isGitHubCloneUrl(project.cloneUrls[0]);
const selectedIssue = issues.find((issue) => issue.id === selectedIssueId) ?? null;

if (issuesQuery.isLoading) {
  return <p className="p-4 text-sm text-muted-foreground">Loading issues…</p>;
}

if (githubHosted && issuesQuery.isError) {
  return (
    <div className="p-4">
      <GitHubRepoStateRecovery
        error={issuesQuery.error}
        onRetry={() => void issuesQuery.refetch()}
        titleId="github-issues-recovery-title"
        unavailableTitle="Could not load GitHub issues"
      />
    </div>
  );
}

if (issuesQuery.isError) {
  return <p className="p-4 text-sm text-muted-foreground">Could not load issues for this repository.</p>;
}

if (issues.length === 0) {
  return (
    <div className="space-y-2 p-4 text-sm text-muted-foreground">
      <p>{githubHosted ? "No open issues." : "No issues yet."}</p>
      {githubHosted && issuesQuery.data?.hasMore ? <p>More open issues exist on GitHub.</p> : null}
    </div>
  );
}

if (selectedIssue) {
  return githubHosted ? (
    <GitHubIssueDetail issue={selectedIssue} project={project} />
  ) : (
    <ProjectIssueDetail issue={selectedIssue} profiles={profiles} project={project} />
  );
}

if (githubHosted) {
  return (
    <div>
      <div className="divide-y divide-border/50">
        {issues.map((issue) => (
          <GitHubIssueRow
            issue={issue}
            key={issue.id}
            onOpen={() => onSelectedIssueIdChange(issue.id)}
          />
        ))}
      </div>
      {issuesQuery.data?.hasMore ? (
        <p className="border-t border-border/50 p-4 text-xs text-muted-foreground">
          More open issues exist on GitHub.
        </p>
      ) : null}
    </div>
  );
}

// Keep the existing Buzz issue row mapping unchanged below this branch.
```

Because the existing `IssueRow`, `ProjectIssueDetail`, `IssueMetaRail`, and `issueMembers` now execute only for Buzz issues, their pubkey normalization and Nostr mutations remain unchanged and never see a login.

- [ ] **Step 8: Add login mode to the shared comment timeline**

Add `githubMode = false` to the props.
Preserve the supplied API order in GitHub mode so equal-second comments do not get reordered by their IDs.
Make both label and avatar selection lazy conditional expressions so the GitHub branch never calls `resolveUserLabel` or `normalizePubkey`.

```tsx
export function ProjectIssueCommentTimeline({
  comments,
  githubMode = false,
  profiles,
}: {
  comments: ProjectIssue["comments"];
  githubMode?: boolean;
  profiles?: UserProfileLookup;
}) {
  const orderedComments = React.useMemo(
    () =>
      githubMode
        ? [...comments]
        : [...comments].sort(
            (left, right) =>
              left.createdAt - right.createdAt || left.id.localeCompare(right.id),
          ),
    [comments, githubMode],
  );
  // Keep the existing collapse behavior below this order selection.
}

const authorLabel = githubMode
  ? comment.author
  : resolveUserLabel({ profiles, pubkey: comment.author });
const avatarUrl = githubMode
  ? comment.authorAvatarUrl ?? null
  : profiles?.[normalizePubkey(comment.author)]?.avatarUrl ?? null;
```

Use `avatarUrl` in `UserAvatar`.
Render the author with this branch.

```tsx
{githubMode ? (
  <span>{authorLabel}</span>
) : (
  <ProfileAuthorName pubkey={comment.author}>{authorLabel}</ProfileAuthorName>
)}
```

Keep collapsing, timestamps, and `ProjectRichContent` unchanged.

- [ ] **Step 9: Run the new smoke spec and existing Buzz issue spec and verify GREEN**

If a stale server is listening on port 4173, stop that specific process before rebuilding as directed by `AGENTS.md`.

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- project-issue-comments.spec.ts
```

Expected: the GitHub list/detail/create/auth cases pass and the existing Buzz comment/assignee cases remain green.

- [ ] **Step 10: Run focused unit/static checks**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/projectIssues.test.mjs src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectShareLinks.test.mjs src/features/projects/issueMutations.test.mjs && pnpm typecheck && pnpm check:px-text && pnpm check:file-sizes
```

Expected: all commands pass with no warnings or file-size violations.

- [ ] **Step 11: Format, inspect scope, and commit**

Run GitNexus `detect_changes({ scope: "staged" })` and confirm only the GitHub issue presentation, recovery generalization, mock commands, smoke registration, and comment identity branch changed.

```bash
. ./bin/activate-hermit
cd desktop && pnpm exec biome check --write \
  src/testing/e2eBridge.ts \
  tests/e2e/github-issues.spec.ts \
  playwright.config.ts \
  src/features/projects/ui/GitHubRepoStateRecovery.tsx \
  src/features/projects/ui/GitHubProjectIssues.tsx \
  src/features/projects/ui/ProjectIssuesPanel.tsx \
  src/features/projects/ui/ProjectIssueCommentTimeline.tsx
git add src/testing/e2eBridge.ts \
  tests/e2e/github-issues.spec.ts \
  playwright.config.ts \
  src/features/projects/ui/GitHubRepoStateRecovery.tsx \
  src/features/projects/ui/GitHubProjectIssues.tsx \
  src/features/projects/ui/ProjectIssuesPanel.tsx \
  src/features/projects/ui/ProjectIssueCommentTimeline.tsx
git diff --check
git commit -s -m "feat(projects): render GitHub issues as read-only native work items"
```

---

## Final Verification

- [ ] Run focused Rust tests.

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_github_issues
```

- [ ] Run all focused desktop unit and static checks.

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/projectIssues.test.mjs src/features/projects/lib/projectGithubIssues.test.mjs src/features/projects/lib/projectShareLinks.test.mjs src/features/projects/issueMutations.test.mjs && pnpm typecheck && pnpm check:px-text && pnpm check:file-sizes
```

- [ ] Rebuild and run both GitHub and Buzz issue smoke specs.

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-issues.spec.ts
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- project-issue-comments.spec.ts
```

- [ ] Run the repository-wide gate required before PR handoff.

```bash
. ./bin/activate-hermit && just ci
```

- [ ] Run final GitNexus scope validation.

Use `detect_changes({ scope: "compare", base_ref: "main" })` and verify the affected flows are limited to GitHub issue native commands, repository issue query/create, Projects issue presentation, and their tests.
If the tool is unavailable, capture `git diff --stat main...HEAD`, `git diff --name-only main...HEAD`, and `git diff --check` in the handoff.

## Acceptance Criteria

- A GitHub-hosted repository calls `list_github_issues` with `state=open` and never fetches kind `1621` for the per-repository tab.
- The list renders numeric `#N`, `Open`, GitHub author login/avatar, labels, read-only assignee avatars, comment count, and the first-page-more note when applicable.
- Opening a GitHub issue renders its list payload body immediately and then the first 100 comments in GitHub order.
- GitHub detail renders no discussed-channel block, comment composer, assignment mutation UI, pubkey profile button, or pubkey author component.
- Creating on a GitHub repository sends only title/body to `create_github_issue`, selects the returned numeric issue, and never signs or publishes kind `1621`.
- GitHub create invalidates only `["project", project.id, "issues"]`.
- Copy link returns only a canonical `https://github.com/<owner>/<repo>/issues/<N>` URL for GitHub rows.
- `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, and `github_issues_failed` render recovery before any empty state.
- A GitHub comment failure leaves the issue body visible and retries only the comment query.
- A successful empty GitHub page says `No open issues.`.
- Buzz-hosted repositories retain their existing kind `1621` list/create/comments/assignment behavior, deep links, invalidations, and copy.
- Rust tests, focused desktop tests, both smoke specs, file-size guards, typecheck, and `just ci` pass.
