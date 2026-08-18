# GitHub remote snapshot and Fetch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** For `github.com` clone URLs, load README, file tree, and recent commits via `gh api`, and make Fetch the primary header action with ahead/behind only when a local HEAD exists.

**Architecture:** Add two GitHub-only Tauri commands that reuse `GhRunner` and return the existing `ProjectRepoSnapshot` / a small compare DTO.
`get_project_repo_snapshot` and `get_project_repo_sync_status` stay Buzz-only.
The project-detail query layer routes GitHub remotes by `isGitHubCloneUrl`, not `host.kind`.
Fetch refetches GitHub queries only and does not run `git fetch`.

**Tech Stack:** Tauri 2 desktop crate, `gh api`, React Query, existing `ProjectRepoSnapshot` and `GitHubRepoStateRecovery`.

**Spec:** [2026-08-18-github-snapshot-and-fetch-design.md](../specs/2026-08-18-github-snapshot-and-fetch-design.md)

**Vision:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) treat GitHub as a first-class Projects host when the clone URL is `github.com`.

## Global Constraints

- G3 + G4 only: no Pull/Push (G5), no create/delete branch (G6), no G7 copy fix, no overview snapshots, no file-body fetch except README, no contributors, no `git fetch` / `ls-remote`, no GitHub tags or `refs/nostr/…`, no kind:30617 `default-branch` persist, no GitHub Enterprise / GitLab / stored OAuth tokens.
- Auth: `GhRunner::discover` then `ensure_auth`.
- Never store a GitHub token.
- Error codes from the two new commands: only `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, `github_state_failed`.
- Remap every `github_merge_failed` before returning.
- Never invent a successful snapshot or `0 / 0` on failure.
- `contributors` is always `[]` on the GitHub snapshot.
- Do not blobless-clone GitHub into a temp dir.
- Fake-`gh` match `/commits`, `/git/trees`, `/readme`, `/compare` **before** the repo root path.
- Do not send full GitHub tree or README JSON through the default 64 KiB `GH_STREAM_LIMIT` without `--jq` or a 256 KiB README cap.
- If G1 (`get_github_repository_state`) failed, do not start G3 or G4.
- Gate recovery copy with `isGitHubCloneUrl`.
- Buzz `/git/<64-hex>/<id>` snapshot and `git fetch` stay unchanged.
- Activate Hermit in **every** shell: `. ./bin/activate-hermit && …`.
- CWD does not persist across tool calls.
- Commits use `git commit -s`.
- Before each commit run `node .gitnexus/run.cjs detect` if that file exists.
- Before editing a symbol, run GitNexus `impact({target, direction: "upstream"})` when the MCP tools are available.
- No `unsafe`.
- No new `unwrap()` / `expect()` on production paths.
- New public Rust/TS APIs get doc comments.
- Desktop text uses rem tokens (`text-2xs` for the ahead/behind label), never `text-[Npx]`.

## Open Questions

1. **Ahead/behind chrome.**
   The spec requires showing `0 / 0` when local HEAD equals the GitHub tip, and hiding counts on `unpushed`.
   There is no existing `0 / 0` widget.
   **Provisional default:** render a mono `text-2xs` `{ahead} / {behind}` label with `data-testid="repo-ahead-behind"` immediately left of Fetch when `status === "compared"` and both counts are numbers.

2. **GitHub-host empty card.**
   The spec forbids that card when G3 succeeds or when G3 fails after G1.
   **Provisional default:** never pass `externalHost === "github.com"` into the README splash or the Files “Not mirrored on Buzz” message.
   Other external hosts keep the splash.

3. **Compare HTTP 404.**
   **Provisional default:** every compare 404 is `{ status: "unpushed" }` with no counts, even if the repository later vanished.
   That is not `github_repo_unavailable`.

---

## File map

| File | Role |
|------|------|
| `desktop/src-tauri/src/commands/project_github_pull_request.rs` | Add `GhRunner::run_with_limit`; `run` delegates with `GH_STREAM_LIMIT` |
| `desktop/src-tauri/src/commands/project_github_repository_state.rs` | `pub(crate)` `remap_state_error` and `combined_cli_diagnostic` |
| Create: `desktop/src-tauri/src/commands/project_github_repository_snapshot.rs` | G3: commits + tree + README → `ProjectRepoSnapshotInfo` |
| Create: `desktop/src-tauri/src/commands/project_github_ahead_behind.rs` | G4: equal SHAs or compare → `{ status, ahead?, behind? }` |
| `desktop/src-tauri/src/commands/mod.rs` | `mod` + `pub use` both new modules |
| `desktop/src-tauri/src/lib.rs` | Register `get_github_repository_snapshot` and `get_github_ahead_behind` next to `get_github_repository_state` |
| `desktop/src/shared/api/projectGit.ts` | `getGithubRepositorySnapshot`, `getGithubAheadBehind` |
| Create: `desktop/src/features/projects/lib/projectGithubSnapshot.ts` | Host-aware snapshot fetch + enable helper |
| Create: `desktop/src/features/projects/lib/projectGithubSnapshot.test.mjs` | Routing tests |
| Create: `desktop/src/features/projects/lib/projectGithubAheadBehind.ts` | Compare DTO + query enable helper |
| Create: `desktop/src/features/projects/lib/projectGithubAheadBehind.test.mjs` | Enable / count-visibility tests |
| Create: `desktop/src/features/projects/lib/projectGithubRemoteView.ts` | Splash-host helper |
| Create: `desktop/src/features/projects/lib/projectGithubRemoteView.test.mjs` | `github.com` never splashes |
| `desktop/src/features/projects/hooks.ts` | Use GitHub snapshot fetch; query key includes clone URL |
| `desktop/src/features/projects/repoSyncHooks.ts` | `useGithubAheadBehindQuery`; leave Buzz sync query Buzz-only |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Enable G3 after G1 success; wire Fetch; hide Pull/Push; pass GitHub counts |
| `desktop/src/features/projects/ui/ProjectRepositorySource.tsx` | GitHub Fetch + ahead/behind; Open moves to the source dropdown |
| `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx` | Stop treating GitHub as the host-empty splash |
| `desktop/src/features/projects/ui/ProjectReadmePanel.tsx` | README fallback to the file that carries `previewContent` |
| `desktop/src/testing/e2eBridge.ts` | Stub the two new commands and optional error flags |
| Create: `desktop/tests/e2e/github-snapshot-and-fetch.spec.ts` | Smoke: README, auth recovery, ahead/behind, Fetch invokes GitHub commands |
| `desktop/playwright.config.ts` | Register the new spec in the smoke `testMatch` |

Do not modify `useProjectsRepoSnapshots.ts`.
Do not modify `get_project_repo_snapshot` or `get_project_repo_sync_status` beyond leaving them Buzz-only.

---

### Task 1: Bounded `gh` stdout + shared remap

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_github_pull_request.rs`
- Modify: `desktop/src-tauri/src/commands/project_github_pull_request/runner_tests.rs`
- Modify: `desktop/src-tauri/src/commands/project_github_repository_state.rs`

**Interfaces:**
- Consumes: existing `GhRunner::run`, `GH_STREAM_LIMIT`, `remap_state_error`, `combined_cli_diagnostic`
- Produces:
  - `pub(crate) fn GhRunner::run_with_limit(&self, args: &[OsString], stdout_limit: usize) -> Result<GhOutput, ProjectPullRequestMergeError>`
  - `pub(crate) fn GhRunner::run(&self, args: &[OsString])` calls `run_with_limit(args, GH_STREAM_LIMIT)`
  - `pub(crate) fn remap_state_error(...)` and `pub(crate) fn combined_cli_diagnostic(...)` in `project_github_repository_state.rs`

- [ ] **Step 1: Write the failing larger-limit test**

Add to `runner_tests.rs` next to `gh_runner_drains_both_streams_and_bounds_retention`:

```rust
#[cfg(unix)]
#[test]
fn gh_runner_run_with_limit_keeps_more_than_default_stream_cap() {
    let (_dir, binary) = fake_gh(
        r#"
head -c 200000 /dev/zero | tr '\0' r
"#,
    );
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(5),
    };
    let output = runner
        .run_with_limit(&[], 256 * 1024)
        .expect("drain fake gh output");
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 200_000);
    assert!(output.stdout.bytes().all(|byte| byte == b'r'));
}
```

- [ ] **Step 2: Run the test — expect compile fail**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib gh_runner_run_with_limit_keeps_more_than_default_stream_cap
```

Expected: FAIL with `no method named run_with_limit`.

- [ ] **Step 3: Implement `run_with_limit`**

In `impl GhRunner`, keep `run` as a one-line wrapper.

Move the current `run` body into `run_with_limit`.
The stdout reader uses `stdout_limit`.
The stderr reader stays on `GH_STREAM_LIMIT`.

```rust
pub(crate) fn run(&self, args: &[OsString]) -> Result<GhOutput, ProjectPullRequestMergeError> {
    self.run_with_limit(args, GH_STREAM_LIMIT)
}

pub(crate) fn run_with_limit(
    &self,
    args: &[OsString],
    stdout_limit: usize,
) -> Result<GhOutput, ProjectPullRequestMergeError> {
    // existing run() body, except:
    // let stdout_thread = std::thread::spawn(move || read_pipe_bounded(stdout, stdout_limit));
    // let stderr_thread = std::thread::spawn(move || read_pipe_bounded(stderr, GH_STREAM_LIMIT));
}
```

Change `fn remap_state_error` and `fn combined_cli_diagnostic` in `project_github_repository_state.rs` to `pub(crate)`.

Do not change remap behavior.

- [ ] **Step 4: Run runner + state tests**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib gh_runner_ && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_repository_state
```

Expected: PASS on unix.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_pull_request.rs \
  desktop/src-tauri/src/commands/project_github_pull_request/runner_tests.rs \
  desktop/src-tauri/src/commands/project_github_repository_state.rs
git commit -s -m "feat(projects): allow bounded gh stdout for GitHub snapshot"
```

---

### Task 2: Map `gh api` commits, tree, and README to `ProjectRepoSnapshot`

**Files:**
- Create: `desktop/src-tauri/src/commands/project_github_repository_snapshot.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs` (`mod project_github_repository_snapshot;`)

**Interfaces:**
- Consumes: `GhRunner::{from_resolved, ensure_auth, run, run_with_limit}`, `GitHubRepoRef::{parse, slug}`, `remap_state_error`, `combined_cli_diagnostic`, `clean_branch`, `ProjectRepoSnapshotInfo`, `ProjectRepoCommitInfo`, `ProjectRepoFileInfo`
- Produces:
  - `pub(crate) fn github_repository_snapshot_with(gh: &GhRunner, clone_url: &str, git_ref: &str) -> Result<ProjectRepoSnapshotInfo, ProjectPullRequestMergeError>`
  - `const GH_README_STREAM_LIMIT: usize = 256 * 1024`
  - `const MAX_PREVIEW_BYTES: usize = 64 * 1024`
  - `const MAX_TREE_ENTRIES: usize = 250`
  - `const MAX_COMMITS: usize = 50`

- [ ] **Step 1: Write failing mapping tests**

Put tests in the same file under `#[cfg(test)]`.
Copy the unix `fake_gh` helper from `project_github_repository_state.rs`.

```rust
#[cfg(unix)]
fn fake_gh(script: &str) -> (tempfile::TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("create fake gh directory");
    let path = dir.path().join("gh");
    std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{script}\n")).expect("write fake gh");
    let mut permissions = std::fs::metadata(&path).expect("stat fake gh").permissions();
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
fn maps_commits_tree_and_readme_preview() {
    let sha = "a".repeat(40);
    let tree = "b".repeat(40);
    let readme_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        b"# Hello develop\n",
    );
    let script = format!(
        r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[{{"sha":"{sha}","tree":"{tree}","name":"Ada","email":"ada@example.com","date":"2026-01-02T03:04:05Z","subject":"Seed readme"}}]'
    ;;
  *"/repos/acme/app/git/trees/"*)
    printf '%s' '{{"tree":[{{"path":"src/lib.rs","type":"blob","size":12}},{{"path":"README.md","type":"blob","size":16}}]}}'
    ;;
  *"/repos/acme/app/readme"*)
    printf '%s' '{{"path":"README.md","content":"{readme_b64}","encoding":"base64","size":16}}'
    ;;
  *) exit 1 ;;
esac
"#
    );
    let (_dir, path) = fake_gh(&script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let snapshot = github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
        .expect("snapshot");
    assert_eq!(snapshot.latest_commit.as_ref().map(|c| c.hash.as_str()), Some(sha.as_str()));
    assert_eq!(snapshot.commits.len(), 1);
    assert_eq!(snapshot.contributors.len(), 0);
    let readme = snapshot
        .files
        .iter()
        .find(|file| file.path == "README.md")
        .expect("readme");
    assert_eq!(readme.preview_content.as_deref(), Some("# Hello develop\n"));
    assert!(snapshot
        .files
        .iter()
        .find(|file| file.path == "src/lib.rs")
        .and_then(|file| file.preview_content.as_ref())
        .is_none());
}

#[cfg(unix)]
#[test]
fn readme_http_404_still_returns_tree() {
    let sha = "a".repeat(40);
    let tree = "b".repeat(40);
    let script = format!(
        r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[{{"sha":"{sha}","tree":"{tree}","name":"Ada","email":"ada@example.com","date":"2026-01-02T03:04:05Z","subject":"Seed"}}]'
    ;;
  *"/repos/acme/app/git/trees/"*)
    printf '%s' '{{"tree":[{{"path":"src/lib.rs","type":"blob","size":12}}]}}'
    ;;
  *"/repos/acme/app/readme"*)
    printf 'gh: HTTP 404\n' >&2
    printf '%s' '{{"message":"Not Found"}}'
    exit 1
    ;;
  *) exit 1 ;;
esac
"#
    );
    let (_dir, path) = fake_gh(&script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let snapshot = github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
        .expect("snapshot");
    assert_eq!(snapshot.files.len(), 1);
    assert!(snapshot.files[0].preview_content.is_none());
}

#[cfg(unix)]
#[test]
fn empty_commit_list_skips_tree_and_readme() {
    let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[]'
    ;;
  *) exit 1 ;;
esac
"#;
    let (_dir, path) = fake_gh(script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let snapshot = github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
        .expect("empty");
    assert!(snapshot.latest_commit.is_none());
    assert!(snapshot.commits.is_empty());
    assert!(snapshot.files.is_empty());
}

#[test]
fn rejects_non_github_clone_url_before_runner() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
        .expect("dummy runner unused");
    let err = github_repository_snapshot_with(&gh, "https://gitlab.com/acme/app", "main")
        .expect_err("gitlab");
    assert_eq!(error_code(&err), "github_state_failed");
}

#[test]
fn rejects_nostr_and_tag_refs() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
        .expect("dummy runner unused");
    let nostr = github_repository_snapshot_with(
        &gh,
        "https://github.com/acme/app",
        "refs/nostr/abc",
    )
    .expect_err("nostr");
    let tag = github_repository_snapshot_with(
        &gh,
        "https://github.com/acme/app",
        "refs/tags/v1",
    )
    .expect_err("tag");
    assert_eq!(error_code(&nostr), "github_state_failed");
    assert_eq!(error_code(&tag), "github_state_failed");
}

#[cfg(unix)]
#[test]
fn missing_gh_binary_is_cli_missing() {
    let err = GhRunner::from_resolved(None).expect_err("missing");
    assert_eq!(error_code(&err), "github_cli_missing");
}

#[cfg(unix)]
#[test]
fn inserts_readme_when_missing_from_tree_window() {
    let sha = "a".repeat(40);
    let tree = "b".repeat(40);
    let readme_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        b"# Deep\n",
    );
    let script = format!(
        r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[{{"sha":"{sha}","tree":"{tree}","name":"Ada","email":"ada@example.com","date":"2026-01-02T03:04:05Z","subject":"Seed"}}]'
    ;;
  *"/repos/acme/app/git/trees/"*)
    printf '%s' '{{"tree":[{{"path":"src/lib.rs","type":"blob","size":12}}]}}'
    ;;
  *"/repos/acme/app/readme"*)
    printf '%s' '{{"path":"docs/README.md","content":"{readme_b64}","encoding":"base64","size":7}}'
    ;;
  *) exit 1 ;;
esac
"#
    );
    let (_dir, path) = fake_gh(&script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let snapshot = github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
        .expect("snapshot");
    assert!(snapshot.files.iter().any(|file| file.path == "docs/README.md"
        && file.preview_content.as_deref() == Some("# Deep\n")));
}
```

- [ ] **Step 2: Run tests — expect compile fail**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib maps_commits_tree_and_readme_preview
```

Expected: FAIL with `github_repository_snapshot_with` missing.

- [ ] **Step 3: Implement the mapper**

```rust
//! Load a GitHub remote snapshot through `gh api` without cloning.

use crate::commands::project_git::{
    ProjectRepoCommitInfo, ProjectRepoFileInfo, ProjectRepoSnapshotInfo,
};
use crate::commands::project_git_exec::clean_branch;
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{GhOutput, GhRunner, GitHubRepoRef};
use crate::commands::project_github_repository_state::{
    combined_cli_diagnostic, remap_state_error,
};
use base64::Engine as _;
use serde::Deserialize;
use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

const GH_README_STREAM_LIMIT: usize = 256 * 1024;
const MAX_PREVIEW_BYTES: usize = 64 * 1024;
const MAX_TREE_ENTRIES: usize = 250;
const COMMITS_JQ: &str = "[.[] | {sha, tree: .commit.tree.sha, name: .commit.author.name, email: .commit.author.email, date: .commit.author.date, subject: (.commit.message | split(\"\\n\")[0])}]";
const TREE_JQ: &str = "{tree: [.tree[:250][] | {path, type, size}]}";

#[derive(Debug, Deserialize)]
struct GithubCommitRow {
    sha: String,
    tree: String,
    name: Option<String>,
    email: Option<String>,
    date: Option<String>,
    subject: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubTreePayload {
    tree: Vec<GithubTreeEntry>,
}

#[derive(Debug, Deserialize)]
struct GithubTreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GithubReadmePayload {
    path: String,
    content: Option<String>,
    encoding: Option<String>,
    size: Option<u64>,
}

pub(crate) fn github_repository_snapshot_with(
    gh: &GhRunner,
    clone_url: &str,
    git_ref: &str,
) -> Result<ProjectRepoSnapshotInfo, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_state_failed", message))?;
    let branch = clean_github_branch(git_ref)?;
    gh.ensure_auth()
        .map_err(|error| remap_state_error(error, ""))?;

    let commits = list_commits(gh, &repo.slug(), &branch)?;
    let Some(head) = commits.first() else {
        return Ok(empty_snapshot());
    };
    let files = list_tree(gh, &repo.slug(), &head.tree)?;
    let readme = fetch_readme(gh, &repo.slug(), &branch)?;
    Ok(ProjectRepoSnapshotInfo {
        latest_commit: commits.first().cloned(),
        commits,
        files: attach_readme(files, readme),
        contributors: vec![],
    })
}

fn clean_github_branch(git_ref: &str) -> Result<String, ProjectPullRequestMergeError> {
    let trimmed = git_ref.trim();
    if trimmed.starts_with("refs/nostr/") || trimmed.starts_with("refs/tags/") {
        return Err(ProjectPullRequestMergeError::new(
            "github_state_failed",
            "GitHub snapshot accepts a branch name, not a tag or nostr ref.",
        ));
    }
    clean_branch(Some(trimmed.to_string())).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_state_failed",
            "Invalid GitHub branch name.",
        )
    })
}

fn list_commits(
    gh: &GhRunner,
    slug: &str,
    branch: &str,
) -> Result<Vec<ProjectRepoCommitInfo>, ProjectPullRequestMergeError> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("sha", branch)
        .append_pair("per_page", "50")
        .finish();
    let path = format!("/repos/{slug}/commits?{query}");
    let rows: Vec<GithubCommitRow> = github_api_json(gh, &path, Some(COMMITS_JQ), 64 * 1024)?;
    Ok(rows.into_iter().take(50).map(map_commit).collect())
}

fn list_tree(
    gh: &GhRunner,
    slug: &str,
    tree_sha: &str,
) -> Result<Vec<ProjectRepoFileInfo>, ProjectPullRequestMergeError> {
    let path = format!("/repos/{slug}/git/trees/{tree_sha}?recursive=1");
    let payload: GithubTreePayload = github_api_json(gh, &path, Some(TREE_JQ), 64 * 1024)?;
    Ok(payload
        .tree
        .into_iter()
        .take(MAX_TREE_ENTRIES)
        .map(|entry| ProjectRepoFileInfo {
            path: entry.path,
            kind: entry.kind,
            size: entry.size,
            preview_content: None,
            last_changed_at: None,
            latest_commit: None,
        })
        .collect())
}

fn fetch_readme(
    gh: &GhRunner,
    slug: &str,
    branch: &str,
) -> Result<Option<GithubReadmePayload>, ProjectPullRequestMergeError> {
    let query = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("ref", branch)
        .finish();
    let path = format!("/repos/{slug}/readme?{query}");
    let output = github_api_output(gh, &path, None, GH_README_STREAM_LIMIT)?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        let lower = diagnostic.to_ascii_lowercase();
        if lower.contains("404") || lower.contains("not found") {
            return Ok(None);
        }
        return Err(remap_state_error(
            ProjectPullRequestMergeError::new("github_merge_failed", diagnostic.clone()),
            &diagnostic,
        ));
    }
    if output.stdout.len() >= GH_README_STREAM_LIMIT {
        return Ok(None);
    }
    serde_json::from_str::<GithubReadmePayload>(&output.stdout)
        .map(Some)
        .or(Ok(None))
}

fn attach_readme(
    mut files: Vec<ProjectRepoFileInfo>,
    readme: Option<GithubReadmePayload>,
) -> Vec<ProjectRepoFileInfo> {
    let Some(readme) = readme else {
        return files;
    };
    let preview = decode_readme_preview(&readme);
    if let Some(existing) = files.iter_mut().find(|file| file.path == readme.path) {
        existing.preview_content = preview;
        return files;
    }
    files.insert(
        0,
        ProjectRepoFileInfo {
            path: readme.path,
            kind: "blob".to_string(),
            size: readme.size,
            preview_content: preview,
            last_changed_at: None,
            latest_commit: None,
        },
    );
    files
}

fn decode_readme_preview(readme: &GithubReadmePayload) -> Option<String> {
    if !readme
        .encoding
        .as_deref()
        .is_some_and(|encoding| encoding.eq_ignore_ascii_case("base64"))
    {
        return None;
    }
    let compact: String = readme
        .content
        .as_deref()?
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let mut bytes = base64::engine::general_purpose::STANDARD
        .decode(compact.as_bytes())
        .ok()?;
    if bytes.contains(&0) {
        return None;
    }
    if bytes.len() > MAX_PREVIEW_BYTES {
        bytes.truncate(MAX_PREVIEW_BYTES);
    }
    String::from_utf8(bytes).ok()
}

fn map_commit(row: GithubCommitRow) -> ProjectRepoCommitInfo {
    let timestamp = row
        .date
        .as_deref()
        .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
        .map(|date| date.timestamp())
        .unwrap_or(0);
    ProjectRepoCommitInfo {
        short_hash: row.sha.chars().take(7).collect(),
        hash: row.sha,
        author_name: row.name.unwrap_or_default(),
        author_email: row.email.unwrap_or_default(),
        timestamp,
        subject: row.subject.unwrap_or_default(),
    }
}

fn empty_snapshot() -> ProjectRepoSnapshotInfo {
    ProjectRepoSnapshotInfo {
        latest_commit: None,
        commits: vec![],
        files: vec![],
        contributors: vec![],
    }
}

fn github_api_json<T: serde::de::DeserializeOwned>(
    gh: &GhRunner,
    path: &str,
    jq: Option<&str>,
    stdout_limit: usize,
) -> Result<T, ProjectPullRequestMergeError> {
    let output = github_api_output(gh, path, jq, stdout_limit)?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        return Err(remap_state_error(
            ProjectPullRequestMergeError::new("github_merge_failed", diagnostic.clone()),
            &diagnostic,
        ));
    }
    serde_json::from_str(&output.stdout).map_err(|_| {
        remap_state_error(
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "GitHub CLI returned an unexpected JSON response. Update gh, then retry.",
            ),
            &output.stderr,
        )
    })
}

fn github_api_output(
    gh: &GhRunner,
    path: &str,
    jq: Option<&str>,
    stdout_limit: usize,
) -> Result<GhOutput, ProjectPullRequestMergeError> {
    let mut args = vec![
        OsString::from("api"),
        OsString::from("--hostname"),
        OsString::from("github.com"),
        OsString::from("--method"),
        OsString::from("GET"),
        OsString::from(path),
    ];
    if let Some(filter) = jq {
        args.push(OsString::from("--jq"));
        args.push(OsString::from(filter));
    }
    gh.run_with_limit(&args, stdout_limit)
        .map_err(|error| remap_state_error(error, ""))
}
```

Do not call tree or README when the commit list is empty.
Do not deserialize the raw GitHub tree object without `--jq`.
Every return path must be one of the four allowed codes.

- [ ] **Step 4: Run snapshot tests**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_repository_snapshot
```

Expected: PASS on unix.
Windows: keep the gitlab/ref tests; keep fake-gh tests `#[cfg(unix)]`.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_repository_snapshot.rs \
  desktop/src-tauri/src/commands/mod.rs
git commit -s -m "feat(projects): map GitHub snapshot from gh api"
```

---

### Task 3: Expose `get_github_repository_snapshot`

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_github_repository_snapshot.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs` (`pub use project_github_repository_snapshot::*;`)
- Modify: `desktop/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `github_repository_snapshot_with`, `GhRunner::discover`
- Produces: `#[tauri::command] pub async fn get_github_repository_snapshot(clone_url: String, git_ref: String) -> Result<ProjectRepoSnapshotInfo, ProjectPullRequestMergeError>`

Tauri invoke args from TypeScript: `{ cloneUrl, ref }`.
Name the Rust parameter `git_ref` and add `#[serde(rename = "ref")]` **or** name it `r#ref` if the crate already uses that style.
If the existing Tauri rename is camelCase-only, use `ref_name: String` in Rust and invoke as `{ cloneUrl, refName }`.
**Provisional default:** Rust parameter `git_ref: String`, TypeScript invoke key `ref` via `#[serde(rename = "ref")]` on a request struct if the command cannot take a raw `ref` identifier.

Prefer this command signature so the TS wrapper stays `{ cloneUrl, ref }`:

```rust
#[derive(Deserialize)]
pub struct GithubRepositorySnapshotInput {
    pub clone_url: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
}

pub(crate) fn get_github_repository_snapshot_with_runner(
    input: GithubRepositorySnapshotInput,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<ProjectRepoSnapshotInfo, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_state_error(error, ""))?;
    github_repository_snapshot_with(&gh, &input.clone_url, &input.git_ref)
}

#[tauri::command]
pub async fn get_github_repository_snapshot(
    clone_url: String,
    #[serde(rename = "ref")] git_ref: String,
) -> Result<ProjectRepoSnapshotInfo, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        get_github_repository_snapshot_with_runner(
            GithubRepositorySnapshotInput { clone_url, git_ref },
            GhRunner::discover(),
        )
    })
    .await
    .map_err(|error| {
        ProjectPullRequestMergeError::new("github_state_failed", error.to_string())
    })?
}
```

If `#[serde(rename = "ref")]` on a command argument fails to compile, switch the TypeScript invoke key to `gitRef` and keep the Rust name `git_ref`.
Document the chosen invoke key in the TS wrapper in Task 5 and do not mix both.

Register `get_github_repository_snapshot` in `lib.rs` immediately after `get_github_repository_state`.

- [ ] **Step 1: Write the wrapper test**

```rust
#[test]
fn wrapper_maps_discover_failure() {
    let err = get_github_repository_snapshot_with_runner(
        GithubRepositorySnapshotInput {
            clone_url: "https://github.com/acme/app".into(),
            git_ref: "develop".into(),
        },
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    let value = serde_json::to_value(err).expect("json");
    assert_eq!(value["code"], "github_cli_missing");
}
```

- [ ] **Step 2: Implement the command and register it**

- [ ] **Step 3: Run**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_repository_snapshot
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_repository_snapshot.rs \
  desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/lib.rs
git commit -s -m "feat(projects): expose get_github_repository_snapshot command"
```

---

### Task 4: Map GitHub compare to ahead/behind

**Files:**
- Create: `desktop/src-tauri/src/commands/project_github_ahead_behind.rs`
- Modify: `desktop/src-tauri/src/commands/mod.rs`
- Modify: `desktop/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `GhRunner`, `GitHubRepoRef`, `remap_state_error`, `combined_cli_diagnostic`, `clean_branch`
- Produces:
  - `pub struct GithubAheadBehind { pub status: String, pub ahead: Option<u32>, pub behind: Option<u32> }`
  - `pub(crate) fn github_ahead_behind_with(gh: &GhRunner, clone_url: &str, branch: &str, local_sha: &str, remote_sha: &str) -> Result<GithubAheadBehind, ProjectPullRequestMergeError>`
  - `#[tauri::command] pub async fn get_github_ahead_behind(clone_url: String, branch: String, local_sha: String, remote_sha: String)`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(unix)]
fn fake_gh(script: &str) -> (tempfile::TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("create fake gh directory");
    let path = dir.path().join("gh");
    std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{script}\n")).expect("write fake gh");
    let mut permissions = std::fs::metadata(&path).expect("stat fake gh").permissions();
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
fn equal_shas_skip_compare() {
    let sha = "d".repeat(40);
    let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/compare"*) exit 1 ;;
  *) exit 1 ;;
esac
"#;
    let (_dir, path) = fake_gh(script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let result = github_ahead_behind_with(
        &gh,
        "https://github.com/acme/app",
        "develop",
        &sha,
        &sha,
    )
    .expect("compared");
    assert_eq!(result.status, "compared");
    assert_eq!(result.ahead, Some(0));
    assert_eq!(result.behind, Some(0));
}

#[cfg(unix)]
#[test]
fn maps_ahead_by_and_behind_by() {
    let local = "e".repeat(40);
    let remote = "d".repeat(40);
    let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/compare"*)
    printf '%s' '{"ahead_by":2,"behind_by":1}'
    ;;
  *) exit 1 ;;
esac
"#;
    let (_dir, path) = fake_gh(script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let result = github_ahead_behind_with(
        &gh,
        "https://github.com/acme/app",
        "develop",
        &local,
        &remote,
    )
    .expect("compared");
    assert_eq!(result.status, "compared");
    assert_eq!(result.ahead, Some(2));
    assert_eq!(result.behind, Some(1));
}

#[cfg(unix)]
#[test]
fn unknown_local_sha_is_unpushed_not_zero() {
    let local = "f".repeat(40);
    let remote = "d".repeat(40);
    let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/compare"*)
    printf 'gh: HTTP 404\n' >&2
    printf '%s' '{"message":"Not Found"}'
    exit 1
    ;;
  *) exit 1 ;;
esac
"#;
    let (_dir, path) = fake_gh(script);
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let result = github_ahead_behind_with(
        &gh,
        "https://github.com/acme/app",
        "develop",
        &local,
        &remote,
    )
    .expect("unpushed");
    assert_eq!(result.status, "unpushed");
    assert_eq!(result.ahead, None);
    assert_eq!(result.behind, None);
}

#[test]
fn rejects_non_github_clone_url_before_runner() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
        .expect("dummy runner unused");
    let err = github_ahead_behind_with(
        &gh,
        "https://gitlab.com/acme/app",
        "main",
        &"a".repeat(40),
        &"b".repeat(40),
    )
    .expect_err("gitlab");
    assert_eq!(error_code(&err), "github_state_failed");
}

#[test]
fn wrapper_maps_discover_failure() {
    let err = get_github_ahead_behind_with_runner(
        "https://github.com/acme/app".into(),
        "develop".into(),
        "a".repeat(40),
        "b".repeat(40),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&err), "github_cli_missing");
}
```

- [ ] **Step 2: Run — expect compile fail**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib equal_shas_skip_compare
```

Expected: FAIL with `github_ahead_behind_with` missing.

- [ ] **Step 3: Implement compare**

```rust
//! Compare a local HEAD to a GitHub branch tip without git fetch.

#[derive(Clone, Debug, Serialize)]
pub struct GithubAheadBehind {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ahead: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GithubComparePayload {
    ahead_by: u32,
    behind_by: u32,
}

pub(crate) fn github_ahead_behind_with(
    gh: &GhRunner,
    clone_url: &str,
    branch: &str,
    local_sha: &str,
    remote_sha: &str,
) -> Result<GithubAheadBehind, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_state_failed", message))?;
    let branch = clean_branch(Some(branch.to_string())).ok_or_else(|| {
        ProjectPullRequestMergeError::new("github_state_failed", "Invalid GitHub branch name.")
    })?;
    let local_sha = parse_oid(local_sha)?;
    let remote_sha = parse_oid(remote_sha)?;
    if local_sha.eq_ignore_ascii_case(&remote_sha) {
        return Ok(GithubAheadBehind {
            status: "compared".into(),
            ahead: Some(0),
            behind: Some(0),
        });
    }
    gh.ensure_auth()
        .map_err(|error| remap_state_error(error, ""))?;
    let path = format!(
        "/repos/{}/compare/{}...{}",
        repo.slug(),
        remote_sha,
        local_sha
    );
    let output = github_api_output(gh, &path)?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        let lower = diagnostic.to_ascii_lowercase();
        if lower.contains("404") || lower.contains("not found") {
            return Ok(GithubAheadBehind {
                status: "unpushed".into(),
                ahead: None,
                behind: None,
            });
        }
        return Err(remap_state_error(
            ProjectPullRequestMergeError::new("github_merge_failed", diagnostic.clone()),
            &diagnostic,
        ));
    }
    let payload: GithubComparePayload = serde_json::from_str(&output.stdout).map_err(|_| {
        remap_state_error(
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "GitHub CLI returned an unexpected JSON response. Update gh, then retry.",
            ),
            &output.stderr,
        )
    })?;
    Ok(GithubAheadBehind {
        status: "compared".into(),
        ahead: Some(payload.ahead_by),
        behind: Some(payload.behind_by),
    })
}

fn parse_oid(value: &str) -> Result<String, ProjectPullRequestMergeError> {
    let value = value.trim().to_ascii_lowercase();
    if matches!(value.len(), 40 | 64) && value.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Ok(value);
    }
    Err(ProjectPullRequestMergeError::new(
        "github_state_failed",
        "GitHub compare requires a full commit SHA.",
    ))
}
```

Include this local helper in the same file:

```rust
fn github_api_output(
    gh: &GhRunner,
    path: &str,
) -> Result<GhOutput, ProjectPullRequestMergeError> {
    gh.run(&[
        OsString::from("api"),
        OsString::from("--hostname"),
        OsString::from("github.com"),
        OsString::from("--method"),
        OsString::from("GET"),
        OsString::from(path),
    ])
    .map_err(|error| remap_state_error(error, ""))
}
```

Do not set `can_pull` or `can_push`.
Do not call `git fetch`.

Register `get_github_ahead_behind` next to `get_github_repository_snapshot`.

```rust
pub(crate) fn get_github_ahead_behind_with_runner(
    clone_url: String,
    branch: String,
    local_sha: String,
    remote_sha: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GithubAheadBehind, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_state_error(error, ""))?;
    github_ahead_behind_with(&gh, &clone_url, &branch, &local_sha, &remote_sha)
}

#[tauri::command]
pub async fn get_github_ahead_behind(
    clone_url: String,
    branch: String,
    local_sha: String,
    remote_sha: String,
) -> Result<GithubAheadBehind, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        get_github_ahead_behind_with_runner(
            clone_url,
            branch,
            local_sha,
            remote_sha,
            GhRunner::discover(),
        )
    })
    .await
    .map_err(|error| {
        ProjectPullRequestMergeError::new("github_state_failed", error.to_string())
    })?
}
```

- [ ] **Step 4: Run**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_ahead_behind
```

Expected: PASS on unix.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src-tauri/src/commands/project_github_ahead_behind.rs \
  desktop/src-tauri/src/commands/mod.rs \
  desktop/src-tauri/src/lib.rs
git commit -s -m "feat(projects): compare GitHub ahead and behind via gh api"
```

---

### Task 5: TypeScript snapshot routing and query enablement

**Files:**
- Modify: `desktop/src/shared/api/projectGit.ts`
- Create: `desktop/src/features/projects/lib/projectGithubSnapshot.ts`
- Create: `desktop/src/features/projects/lib/projectGithubSnapshot.test.mjs`
- Modify: `desktop/src/features/projects/hooks.ts`

**Interfaces:**
- Consumes: `invokeTauri`, `parseProjectPullRequestMergeError`, `isGitHubCloneUrl`, `fromRawProjectRepoSnapshot` (keep private; call it only from `projectGit.ts`)
- Produces:
  - `export async function getGithubRepositorySnapshot(input: { cloneUrl: string; ref: string }): Promise<ProjectRepoSnapshot>`
  - `export function githubRemoteSnapshotEnabled(input: { cloneUrl?: string | null; buzzHost: boolean; githubStateReady: boolean }): boolean`
  - `export async function fetchProjectRepoSnapshotWith(project, branchName, pullRequest, tag, loaders)`
  - Query key includes `project.cloneUrls[0]`

- [ ] **Step 1: Write routing tests**

```js
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  fetchProjectRepoSnapshotWith,
  githubRemoteSnapshotEnabled,
} from "./projectGithubSnapshot.ts";

function repository(cloneUrl) {
  return {
    id: "owner:app",
    dtag: "app",
    name: "app",
    description: "",
    cloneUrls: [cloneUrl],
    webUrl: null,
    owner: "ab".repeat(32),
    contributors: [],
    createdAt: 0,
    status: "active",
    defaultBranch: "main",
    repoAddress: `30617:${"ab".repeat(32)}:app`,
  };
}

test("GitHub clone URL uses the GitHub snapshot command", async () => {
  let githubCalls = 0;
  let buzzCalls = 0;
  const snapshot = { latestCommit: null, commits: [], files: [], contributors: [] };
  const result = await fetchProjectRepoSnapshotWith(
    repository("https://github.com/acme/app"),
    "develop",
    null,
    null,
    {
      loadGithub: async ({ cloneUrl, ref }) => {
        githubCalls += 1;
        assert.equal(cloneUrl, "https://github.com/acme/app");
        assert.equal(ref, "develop");
        return snapshot;
      },
      loadBuzz: async () => {
        buzzCalls += 1;
        return snapshot;
      },
    },
  );
  assert.equal(githubCalls, 1);
  assert.equal(buzzCalls, 0);
  assert.equal(result, snapshot);
});

test("Buzz clone URL does not call the GitHub snapshot command", async () => {
  let githubCalls = 0;
  let buzzCalls = 0;
  const cloneUrl = `https://relay.example/git/${"ab".repeat(32)}/app`;
  await fetchProjectRepoSnapshotWith(repository(cloneUrl), "main", null, null, {
    loadGithub: async () => {
      githubCalls += 1;
      return { latestCommit: null, commits: [], files: [], contributors: [] };
    },
    loadBuzz: async () => {
      buzzCalls += 1;
      return { latestCommit: null, commits: [], files: [], contributors: [] };
    },
  });
  assert.equal(githubCalls, 0);
  assert.equal(buzzCalls, 1);
});

test("GitHub snapshot ignores Buzz tag and nostr target refs", async () => {
  let seenRef = null;
  await fetchProjectRepoSnapshotWith(
    repository("https://github.com/acme/app"),
    "develop",
    { id: "pr1", cloneUrls: ["https://github.com/acme/app"], commit: "c".repeat(40) },
    { name: "v1", commit: "d".repeat(40) },
    {
      loadGithub: async ({ ref }) => {
        seenRef = ref;
        return { latestCommit: null, commits: [], files: [], contributors: [] };
      },
      loadBuzz: async () => {
        throw new Error("buzz snapshot must not run");
      },
    },
  );
  assert.equal(seenRef, "develop");
});

test("snapshot query enablement requires G1 success on GitHub", () => {
  assert.equal(
    githubRemoteSnapshotEnabled({
      cloneUrl: "https://github.com/acme/app",
      buzzHost: false,
      githubStateReady: false,
    }),
    false,
  );
  assert.equal(
    githubRemoteSnapshotEnabled({
      cloneUrl: "https://github.com/acme/app",
      buzzHost: false,
      githubStateReady: true,
    }),
    true,
  );
  assert.equal(
    githubRemoteSnapshotEnabled({
      cloneUrl: `https://relay.example/git/${"ab".repeat(32)}/app`,
      buzzHost: true,
      githubStateReady: false,
    }),
    true,
  );
});
```

- [ ] **Step 2: Run tests — expect fail**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubSnapshot.test.mjs
```

Expected: FAIL because `projectGithubSnapshot.ts` is missing.

- [ ] **Step 3: Implement wrappers and hook wiring**

In `projectGit.ts` next to `getGithubRepositoryState`:

```ts
export async function getGithubRepositorySnapshot(input: {
  cloneUrl: string;
  ref: string;
}): Promise<ProjectRepoSnapshot> {
  try {
    const snapshot = await invokeTauri<RawProjectRepoSnapshot>(
      "get_github_repository_snapshot",
      { cloneUrl: input.cloneUrl, ref: input.ref },
    );
    return fromRawProjectRepoSnapshot(snapshot);
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}
```

If Task 3 settled on `gitRef`, pass `{ cloneUrl, gitRef: input.ref }` and keep the public TS field named `ref`.

In `projectGithubSnapshot.ts`:

```ts
import { isGitHubCloneUrl } from "@/features/projects/lib/projectGitError";
import type { Repository } from "@/features/projects/projectModels";
import type { ProjectPullRequest } from "@/features/projects/projectPullRequests.mjs";
import type { ProjectRepoSnapshot } from "@/shared/api/types";

export function githubRemoteSnapshotEnabled(input: {
  cloneUrl?: string | null;
  buzzHost: boolean;
  githubStateReady: boolean;
}) {
  if (input.buzzHost) return Boolean(input.cloneUrl);
  return Boolean(input.cloneUrl) && isGitHubCloneUrl(input.cloneUrl) && input.githubStateReady;
}

export async function fetchProjectRepoSnapshotWith(
  project: Repository,
  branchName: string | null | undefined,
  pullRequest: ProjectPullRequest | null | undefined,
  tag: { name: string; commit: string } | null | undefined,
  loaders: {
    loadGithub: (input: {
      cloneUrl: string;
      ref: string;
    }) => Promise<ProjectRepoSnapshot>;
    loadBuzz: () => Promise<ProjectRepoSnapshot | null>;
  },
): Promise<ProjectRepoSnapshot | null> {
  const cloneUrl = pullRequest?.cloneUrls[0] ?? project.cloneUrls[0];
  if (!cloneUrl) return null;
  if (isGitHubCloneUrl(cloneUrl)) {
    const ref = branchName ?? project.defaultBranch;
    if (!ref) return null;
    return loaders.loadGithub({ cloneUrl, ref });
  }
  return loaders.loadBuzz();
}
```

Replace the private `fetchProjectRepoSnapshot` in `hooks.ts` so it calls `fetchProjectRepoSnapshotWith` with `getGithubRepositorySnapshot` and the existing `getProjectRepoSnapshot` path (including `targetRef` / `targetCommit` for Buzz only).

Change `useProjectRepoSnapshotQuery`:

```ts
queryKey: [
  "project",
  project?.id ?? "none",
  "repo-snapshot",
  project?.cloneUrls[0] ?? "no-clone",
  selectedBranch ?? "default",
  pullRequest?.id ?? "none",
  pullRequest?.commit ?? "none",
  tag?.name ?? "no-tag",
  tag?.commit ?? "no-tag-commit",
],
```

Do not change `useProjectsRepoSnapshots`.

- [ ] **Step 4: Run unit tests and typecheck**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test -- src/features/projects/lib/projectGithubSnapshot.test.mjs src/features/projects/lib/projectRepoState.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/shared/api/projectGit.ts \
  desktop/src/features/projects/lib/projectGithubSnapshot.ts \
  desktop/src/features/projects/lib/projectGithubSnapshot.test.mjs \
  desktop/src/features/projects/hooks.ts
git commit -s -m "feat(projects): route GitHub remotes to the snapshot command"
```

---

### Task 6: Fetch, ahead/behind, Open, and no GitHub empty card

**Files:**
- Create: `desktop/src/features/projects/lib/projectGithubAheadBehind.ts`
- Create: `desktop/src/features/projects/lib/projectGithubAheadBehind.test.mjs`
- Create: `desktop/src/features/projects/lib/projectGithubRemoteView.ts`
- Create: `desktop/src/features/projects/lib/projectGithubRemoteView.test.mjs`
- Modify: `desktop/src/shared/api/projectGit.ts`
- Modify: `desktop/src/features/projects/repoSyncHooks.ts`
- Modify: `desktop/src/features/projects/ui/ProjectRepositorySource.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectReadmePanel.tsx`

**Interfaces:**
- Consumes: `getGithubAheadBehind`, local snapshot `latestCommit.hash`, G1 branch tip, `isGitHubCloneUrl`
- Produces:
  - `export type GithubAheadBehind = { status: "compared" | "unpushed"; ahead?: number; behind?: number }`
  - `export function githubAheadBehindCounts(result: GithubAheadBehind | null | undefined): { ahead: number; behind: number } | null`
  - `export function githubSplashHost(input: { repoSource: "remote" | "local"; hostKind?: string; host?: string; cloneUrl?: string | null }): string | undefined`
  - `useGithubAheadBehindQuery(...)`
  - GitHub header: Fetch + optional `N / M`; no Open-as-primary; no Pull/Push
  - Source dropdown: “Open on GitHub” when `externalUrl` is set

- [ ] **Step 1: Write failing helper tests**

```js
import assert from "node:assert/strict";
import { test } from "node:test";
import { githubAheadBehindCounts } from "./projectGithubAheadBehind.ts";
import { githubSplashHost } from "./projectGithubRemoteView.ts";

test("compared counts are visible and unpushed hides them", () => {
  assert.deepEqual(
    githubAheadBehindCounts({ status: "compared", ahead: 0, behind: 0 }),
    { ahead: 0, behind: 0 },
  );
  assert.equal(
    githubAheadBehindCounts({ status: "unpushed" }),
    null,
  );
  assert.equal(githubAheadBehindCounts(undefined), null);
});

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
```

- [ ] **Step 2: Run — expect fail**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubAheadBehind.test.mjs src/features/projects/lib/projectGithubRemoteView.test.mjs
```

Expected: FAIL because the modules are missing.

- [ ] **Step 3: Implement helpers, query, and UI**

`getGithubAheadBehind` in `projectGit.ts`:

```ts
export type GithubAheadBehind = {
  status: "compared" | "unpushed";
  ahead?: number;
  behind?: number;
};

export async function getGithubAheadBehind(input: {
  cloneUrl: string;
  branch: string;
  localSha: string;
  remoteSha: string;
}): Promise<GithubAheadBehind> {
  try {
    return await invokeTauri<GithubAheadBehind>("get_github_ahead_behind", {
      cloneUrl: input.cloneUrl,
      branch: input.branch,
      localSha: input.localSha,
      remoteSha: input.remoteSha,
    });
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}
```

```ts
export function githubAheadBehindCounts(
  result: { status: string; ahead?: number; behind?: number } | null | undefined,
) {
  if (!result || result.status !== "compared") return null;
  if (typeof result.ahead !== "number" || typeof result.behind !== "number") {
    return null;
  }
  return { ahead: result.ahead, behind: result.behind };
}
```

```ts
export function githubSplashHost(input: {
  repoSource: "remote" | "local";
  hostKind?: string;
  host?: string;
  cloneUrl?: string | null;
}) {
  if (input.repoSource !== "remote") return undefined;
  if (isGitHubCloneUrl(input.cloneUrl)) return undefined;
  if (input.hostKind === "external" && input.host) return input.host;
  return undefined;
}
```

Add `useGithubAheadBehindQuery` to `repoSyncHooks.ts`.
Leave `useProjectRepoSyncStatusQuery` enabled only when `host.kind === "buzz"`.

```ts
export function useGithubAheadBehindQuery(input: {
  projectId?: string;
  cloneUrl?: string | null;
  branch?: string | null;
  localSha?: string | null;
  remoteSha?: string | null;
  enabled: boolean;
}) {
  return useQuery({
    enabled: Boolean(
      input.enabled &&
        isGitHubCloneUrl(input.cloneUrl) &&
        input.branch &&
        input.localSha &&
        input.remoteSha,
    ),
    queryKey: [
      "project",
      input.projectId ?? "none",
      "github-ahead-behind",
      input.cloneUrl ?? "no-clone",
      input.branch ?? "default",
      input.localSha ?? "no-local",
      input.remoteSha ?? "no-remote",
    ],
    queryFn: () => {
      if (!input.cloneUrl || !input.branch || !input.localSha || !input.remoteSha) {
        throw new Error("GitHub compare is missing a SHA.");
      }
      return getGithubAheadBehind({
        cloneUrl: input.cloneUrl,
        branch: input.branch,
        localSha: input.localSha,
        remoteSha: input.remoteSha,
      });
    },
    staleTime: 10_000,
    retry: 1,
  });
}
```

In `ProjectDetailScreen.tsx`:

```ts
const githubHosted = isGitHubCloneUrl(repository?.cloneUrls[0]);
const snapshotEnabled = githubRemoteSnapshotEnabled({
  cloneUrl: repository?.cloneUrls[0],
  buzzHost: repoRemote.host.kind === "buzz",
  githubStateReady: repoStateQuery.isSuccess,
});
const repoSnapshotQuery = useProjectRepoSnapshotQuery(
  repository,
  activeBranch,
  selectedTag ? null : selectedBranchPullRequest,
  activeTag,
  snapshotEnabled,
);
const githubAheadBehindQuery = useGithubAheadBehindQuery({
  projectId: repository?.id,
  cloneUrl: repository?.cloneUrls[0],
  branch: activeBranch,
  localSha: localRepoSnapshotQuery.data?.snapshot.latestCommit?.hash ?? null,
  remoteSha:
    repoStateQuery.data?.branches.find((item) => item.name === activeBranch)
      ?.commit ?? null,
  enabled: githubHosted && repoStateQuery.isSuccess,
});
const githubCounts = githubAheadBehindCounts(githubAheadBehindQuery.data);
```

Change `handleFetchRepo`:

```ts
const handleFetchRepo = React.useCallback(async () => {
  const tasks = githubHosted
    ? [
        repoStateQuery.refetch(),
        ...(repoStateQuery.isError ? [] : [repoSnapshotQuery.refetch()]),
        ...(githubAheadBehindQuery.isFetched ||
        (localRepoSnapshotQuery.data?.snapshot.latestCommit?.hash &&
          repoStateQuery.data?.branches.find((item) => item.name === activeBranch)
            ?.commit)
          ? [githubAheadBehindQuery.refetch()]
          : []),
      ]
    : [
        repoSnapshotQuery.refetch(),
        repoStateQuery.refetch(),
        repoSyncStatusQuery.refetch(),
      ];
  const results = await Promise.all(tasks);
  const error = results.find((result) => result.error)?.error;
  if (error) {
    toast.error(
      githubHosted
        ? "Could not refresh GitHub repository."
        : "Could not fetch repository.",
      {
        description:
          error instanceof Error ? error.message : "The refresh failed.",
      },
    );
    return;
  }
  toast.success("Remote state refreshed.");
}, [/* include the queries used above */]);
```

On `filesSourceControls`:

- Set `githubHosted`.
- For GitHub, set `canPush: false`, `canPull: false`, `onPush: undefined`, `onPull: undefined`.
- Set `aheadCount` / `behindCount` from `githubCounts` on GitHub, otherwise keep Buzz sync counts.
- Keep `onFetch`.
- `fetchPending` includes `githubAheadBehindQuery.isFetching` on GitHub and does not wait on Buzz sync.
- `fetchTitle` is `"Refresh GitHub README, files, and compare"` on GitHub.
- Recovery: `showGithubStateRecovery` is true when GitHub and (`repoStateQuery.isError` or (`repoStateQuery.isSuccess` and `repoSnapshotQuery.isError`)).
- `stateError` is the G1 error if present, otherwise the G3 error.
- `onRetryState` refetches G1 when G1 failed, otherwise refetches G3.

In `RepoSourceHeaderControls` add `githubHosted?: boolean`.

In `RepoSyncActionButton`, handle GitHub **before** the generic `remoteKind === "external"` Open branch:

```tsx
if (controls.githubHosted) {
  if (!controls.onFetch) return null;
  return (
    <div className="flex items-center gap-2">
      {controls.aheadCount != null && controls.behindCount != null ? (
        <span
          className="font-mono text-2xs text-muted-foreground"
          data-testid="repo-ahead-behind"
        >
          {controls.aheadCount} / {controls.behindCount}
        </span>
      ) : null}
      <Button
        className={PROJECT_PANEL_ACTION_BUTTON_CLASS}
        disabled={controls.fetchPending}
        onClick={controls.onFetch}
        size="sm"
        title={controls.fetchTitle ?? "Refresh GitHub repository"}
        variant="ghost"
      >
        {controls.fetchPending ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <RefreshCw className="h-4 w-4" />
        )}
        Fetch
      </Button>
    </div>
  );
}
```

In `RepoSourceDropdown`, after the radio group / clone item, when `controls.githubHosted && controls.externalUrl`:

```tsx
<DropdownMenuSeparator />
<DropdownMenuItem asChild>
  <a href={controls.externalUrl} rel="noreferrer" target="_blank">
    <ExternalLink className="h-4 w-4" />
    Open on GitHub
  </a>
</DropdownMenuItem>
```

`externalUrl` is already `isSafeUrl`-filtered in `useProjectRepoPresentation`.

In `ProjectWorkspaceTabs.tsx` replace the `externalHost` assignment:

```ts
const splashHost = githubSplashHost({
  repoSource,
  hostKind: repoHost.kind,
  host: repoHost.kind === "external" ? repoHost.host : undefined,
  cloneUrl: project.cloneUrls[0],
});
```

Use `splashHost` everywhere `externalHost` currently drives the README splash or the Files “Not mirrored on Buzz” string.

Pass `snapshotLoading` from the screen as `repoSnapshotQuery.isLoading || (githubHosted && repoStateQuery.isPending)` so G1 pending shows “Loading repository…” instead of the splash.

In `ProjectReadmePanel.tsx` `findReadmeFile` call site (WorkspaceTabs already computes `readmeFile`), add:

```ts
const readmeFile =
  findReadmeFile(files) ??
  files.find((file) => Boolean(file.previewContent)) ??
  null;
```

- [ ] **Step 4: Typecheck and unit tests**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test -- src/features/projects/lib/projectGithubAheadBehind.test.mjs src/features/projects/lib/projectGithubRemoteView.test.mjs src/features/projects/lib/projectGithubSnapshot.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/shared/api/projectGit.ts \
  desktop/src/features/projects/lib/projectGithubAheadBehind.ts \
  desktop/src/features/projects/lib/projectGithubAheadBehind.test.mjs \
  desktop/src/features/projects/lib/projectGithubRemoteView.ts \
  desktop/src/features/projects/lib/projectGithubRemoteView.test.mjs \
  desktop/src/features/projects/repoSyncHooks.ts \
  desktop/src/features/projects/ui/ProjectRepositorySource.tsx \
  desktop/src/features/projects/ui/ProjectDetailScreen.tsx \
  desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx \
  desktop/src/features/projects/ui/ProjectReadmePanel.tsx
git commit -s -m "feat(projects): fetch GitHub snapshot and show ahead behind"
```

---

### Task 7: e2e mock bridge and smoke

**Files:**
- Modify: `desktop/src/testing/e2eBridge.ts`
- Create: `desktop/tests/e2e/github-snapshot-and-fetch.spec.ts`
- Modify: `desktop/playwright.config.ts`

**Interfaces:**
- Stub `get_github_repository_snapshot` with README + `src/lib.rs` on `develop`
- Stub `get_github_ahead_behind` with compared `0 / 0` when SHAs match, otherwise optional override
- Optional `window.__BUZZ_E2E_GITHUB_SNAPSHOT_ERROR__`
- Extend local snapshot stub so a seeded `local_head` becomes `latest_commit.hash`

- [ ] **Step 1: Extend the mock bridge**

Add to the Window type next to `__BUZZ_E2E_GITHUB_REPO_STATE_ERROR__`:

```ts
__BUZZ_E2E_GITHUB_SNAPSHOT_ERROR__?: { code: string; message: string };
__BUZZ_E2E_GITHUB_AHEAD_BEHIND__?: {
  status: "compared" | "unpushed";
  ahead?: number;
  behind?: number;
};
```

In the invoke switch, after `get_github_repository_state`:

```ts
case "get_github_repository_snapshot": {
  if (window.__BUZZ_E2E_GITHUB_SNAPSHOT_ERROR__) {
    throw window.__BUZZ_E2E_GITHUB_SNAPSHOT_ERROR__;
  }
  return {
    latest_commit: {
      hash: "d".repeat(40),
      short_hash: "ddddddd",
      author_name: "Ada",
      author_email: "ada@example.com",
      timestamp: Math.floor(Date.now() / 1000) - 120,
      subject: "Document develop",
    },
    commits: [
      {
        hash: "d".repeat(40),
        short_hash: "ddddddd",
        author_name: "Ada",
        author_email: "ada@example.com",
        timestamp: Math.floor(Date.now() / 1000) - 120,
        subject: "Document develop",
      },
    ],
    contributors: [],
    files: [
      {
        path: "README.md",
        kind: "blob",
        size: 24,
        preview_content: "# Develop branch\n",
        last_changed_at: null,
        latest_commit: null,
      },
      {
        path: "src/lib.rs",
        kind: "blob",
        size: 12,
        preview_content: null,
        last_changed_at: null,
        latest_commit: null,
      },
    ],
  };
}
case "get_github_ahead_behind": {
  if (window.__BUZZ_E2E_GITHUB_AHEAD_BEHIND__) {
    return window.__BUZZ_E2E_GITHUB_AHEAD_BEHIND__;
  }
  const input = payload as { localSha?: string; remoteSha?: string };
  if (
    input.localSha &&
    input.remoteSha &&
    input.localSha.toLowerCase() === input.remoteSha.toLowerCase()
  ) {
    return { status: "compared", ahead: 0, behind: 0 };
  }
  return { status: "unpushed" };
}
```

Change `get_project_local_repo_snapshot` so a seeded `local_head` is returned as `latest_commit.hash`:

```ts
case "get_project_local_repo_snapshot": {
  const status = window.__BUZZ_E2E_PROJECT_REPO_SYNC_STATUS__;
  const path = status?.local_path;
  if (!path) return null;
  const hash = status.local_head;
  return {
    path,
    snapshot: {
      latest_commit: hash
        ? {
            hash,
            short_hash: hash.slice(0, 7),
            author_name: "Local",
            author_email: "local@example.com",
            timestamp: Math.floor(Date.now() / 1000),
            subject: "Local HEAD",
          }
        : null,
      commits: [],
      files: [],
      contributors: [],
    },
  };
}
```

- [ ] **Step 2: Write the smoke spec**

Follow `github-repo-state.spec.ts`: `addInitScript` before `installMockBridge`, `waitForAnimations` before screenshots or tight assertions, `pnpm build:e2e` only.

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

test("GitHub remote source shows README instead of the hosted-elsewhere card", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await expect(page.getByText("Develop branch")).toBeVisible({ timeout: 10_000 });
  await waitForAnimations(page);
  await expect(page.getByText("Code hosted on github.com")).toHaveCount(0);
  const header = page.getByTestId("project-repository-selection-row");
  await expect(header.getByRole("button", { name: /^Fetch$/ })).toBeVisible();
  await expect(header.getByRole("link", { name: /^Open$/ })).toHaveCount(0);
  await expect(header.getByRole("button", { name: /Pull/ })).toHaveCount(0);
  await expect(header.getByRole("button", { name: /Push/ })).toHaveCount(0);
});

test("GitHub snapshot auth recovery does not show the empty GitHub-host card", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
    window.__BUZZ_E2E_GITHUB_SNAPSHOT_ERROR__ = {
      code: "github_auth_required",
      message:
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await expect(page.getByText("GitHub authentication required")).toBeVisible({
    timeout: 10_000,
  });
  await waitForAnimations(page);
  await expect(page.getByText("Code hosted on github.com")).toHaveCount(0);
});

test("local HEAD matching the GitHub tip shows 0 / 0 and Fetch calls GitHub commands", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
    window.__BUZZ_E2E_PROJECT_REPO_SYNC_STATUS__ = {
      local_path: "/tmp/buzz/REPOS/acme-app",
      local_branch: "develop",
      local_branches: ["develop"],
      local_head: "d".repeat(40),
      local_short_head: "ddddddd",
      remote_branch: "develop",
      remote_head: "d".repeat(40),
      remote_short_head: "ddddddd",
      merge_base: "d".repeat(40),
      ahead_count: 0,
      behind_count: 0,
      has_uncommitted_changes: false,
      has_untracked_files: false,
      can_push: false,
      push_block_reason: null,
      can_pull: false,
      pull_block_reason: null,
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  await expect(page.getByTestId("repo-ahead-behind")).toHaveText("0 / 0", {
    timeout: 10_000,
  });
  await waitForAnimations(page);
  await page
    .getByTestId("project-repository-selection-row")
    .getByRole("button", { name: /^Fetch$/ })
    .click();
  const commands = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(commands).toContain("get_github_repository_state");
  expect(commands).toContain("get_github_repository_snapshot");
  expect(commands).toContain("get_github_ahead_behind");
  expect(commands).not.toContain("get_project_repo_sync_status");
});
```

Add `"**/github-snapshot-and-fetch.spec.ts"` to the smoke `testMatch` array in `playwright.config.ts` next to `github-repo-state.spec.ts`.

- [ ] **Step 3: Run the smoke spec**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test:e2e:smoke -- github-snapshot-and-fetch
```

Expected: PASS.
Kill anything on port 4173 first if a stale e2e preview is serving an old build.

- [ ] **Step 4: Commit**

```bash
. ./bin/activate-hermit
git add desktop/src/testing/e2eBridge.ts \
  desktop/tests/e2e/github-snapshot-and-fetch.spec.ts \
  desktop/playwright.config.ts
git commit -s -m "test(e2e): cover GitHub snapshot, fetch, and ahead behind"
```

---

## Spec coverage

| Spec requirement | Task |
|------------------|------|
| `get_github_repository_snapshot` via `gh api` | 2–3 |
| Commits `--jq`, tree cap 250, README 64 KiB / 256 KiB stdout | 2 |
| Empty repo skips tree/README | 2 |
| README 404 is success | 2 |
| Insert README if missing from the 250-file window | 2 |
| Reject non-GitHub URL and `refs/nostr` / `refs/tags` | 2 |
| Fake-gh match order | 2, 4 |
| Remap merge errors / missing CLI / auth | 1–4 |
| `get_github_ahead_behind` equal / compare / unpushed | 4 |
| No `can_pull` / `can_push` / `git fetch` | 4, 6 |
| GitHub snapshot routing; Buzz unchanged | 5 |
| Query key includes clone URL | 5 |
| `useProjectsRepoSnapshots` unchanged | 5 (do not edit) |
| `useProjectRepoSyncStatusQuery` stays Buzz-only | 6 |
| Fetch primary; Open on dropdown; no Pull/Push | 6 |
| Ahead/behind only when compared | 6 |
| No GitHub-host empty card | 6–7 |
| G1 failure blocks G3/G4 | 5–6 |
| e2e README, auth recovery, Fetch commands | 7 |
| Existing Buzz snapshot/sync tests stay green | 5–7 |

## Validation commands

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_repository_snapshot
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_ahead_behind
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_repository_state
. ./bin/activate-hermit && cd desktop && pnpm test -- src/features/projects/lib/projectGithubSnapshot.test.mjs src/features/projects/lib/projectGithubAheadBehind.test.mjs src/features/projects/lib/projectGithubRemoteView.test.mjs src/features/projects/lib/projectRepoState.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm typecheck
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-snapshot-and-fetch
```

Existing Buzz snapshot and sync unit tests must stay green.
Do not hit live GitHub in unit tests.

## Self-review

1. Spec coverage: every Goals / Product decisions / Error handling / Testing row maps to a task above.
2. Placeholder scan: no TBD, no “handle edge cases”, no “similar to Task N” without code.
3. Type consistency: `GithubAheadBehind.status` is `"compared" | "unpushed"` in Rust strings and TS; snapshot invoke key is `ref` unless Task 3 documents `gitRef`; query names stay `get_github_repository_snapshot` and `get_github_ahead_behind`.
