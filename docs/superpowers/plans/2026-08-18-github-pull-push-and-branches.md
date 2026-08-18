# GitHub pull, push, and remote branches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** For `github.com` clone URLs, reuse the existing Buzz pull / push / sync-status / create-delete-branch git commands with a wider URL gate and host-aware `gh auth git-credential` auth, so the project header matches Buzz (Pull when behind, Push when ahead, otherwise Fetch) and Create / Delete work without a false “create the first commit” reason.

**Architecture:** Do not add new pull, push, create, or delete Tauri commands.
Add `validate_git_operation_url` (Buzz workspace URL **or** `GitHubRepoRef::parse`) and one `build_git_operation_auth_config` used by clone, sync, pull, push, and remote branch commands.
GitHub HTTPS uses `!<gh-path> auth git-credential` after `GhRunner::discover` + `ensure_auth`.
GitHub SSH uses the agent and never calls `ensure_auth`.
The project page stops calling G4; when a checkout exists, `get_project_repo_sync_status` is the only ahead/behind source and Fetch refetches that query (which `git fetch`es).

**Tech Stack:** Tauri 2 desktop crate, system `git`, installed `gh` CLI, React Query, existing `RepoSyncActionButton` / `GitHubRepoStateRecovery` / `ProjectPullRequestMergeError`.

**Spec:** [2026-08-18-github-pull-push-and-branches-design.md](../specs/2026-08-18-github-pull-push-and-branches-design.md)

**Vision:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) treat GitHub as a first-class Projects host when the clone URL is `github.com`.


## Global Constraints

- G5 + G6 + G7 only: no new `gh api` ref mutations, no GitHub Issues (M2), no GitHub PR list/create (M3), no branch-to-channel creation, no kind:30617 `default-branch` persist, no GitHub tags, no GitHub Enterprise / GitLab / stored OAuth tokens.
- Do not open `get_project_repo_snapshot` to GitHub.
- Do not call `get_github_ahead_behind` (G4) from the project page.
- When a checkout exists, sync status is the only ahead/behind source.
- Do not add new pull / push / create / delete Tauri commands.
- Auth: never store a GitHub token.
- GitHub processes must not receive `NOSTR_PRIVATE_KEY`.
- HTTPS GitHub `discover` / `ensure_auth` failures return `serde_json::to_string` of `ProjectPullRequestMergeError` on today’s `Result<T, String>` commands.
- Remap every `github_merge_failed` leaving this path.
- Allowed structured codes: only `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, `github_state_failed`.
- Git stderr (protected branch, non-fast-forward, SSH denied) stays a plain string.
- Never invent a successful sync (`0 / 0`, `can_pull`, `can_push`) on failure.
- If G1 failed, do not start sync / create / delete.
- Gate GitHub recovery copy with `isGitHubCloneUrl`.
- Buzz `/git/<64-hex>/<id>` sync, pull, push, and branch commands stay on `git-credential-nostr`.
- Fake-`gh` must not treat credential-helper invocations (`auth git-credential`) as API paths.
- Reuse the existing `githubHosted` flag on `RepoSourceHeaderControls` as the spec’s `isGithubRemote` flag.
- Do not delete the G4 command or `useGithubAheadBehindQuery`; only stop calling them from the project page.
- Activate Hermit in **every** shell: `. ./bin/activate-hermit && …`.
- CWD does not persist across tool calls.
- Commits use `git commit -s`.
- Before each commit run `if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi`.
- Before editing a symbol, run GitNexus `impact({target, direction: "upstream"})` when the MCP tools are available.
- No `unsafe`.
- No new `unwrap()` / `expect()` on production paths.
- New public Rust/TS APIs get doc comments.
- Desktop text uses rem tokens, never `text-[Npx]`.

## Required Impact Checks

Run these before the task’s first edit when the GitNexus MCP tools are available.
Report direct callers, affected processes, and risk level before editing.

- Task 1: `validate_workspace_clone_url`, `validate_local_clone_url`, and `GitHubRepoRef::parse`.
- Task 2: `GitAuthConfig`, `configure_git_auth`, `build_git_clone_auth_config`, `build_git_auth_config_for_keys`, and `GhRunner::ensure_auth`.
- Task 3: `get_project_repo_sync_status`, `pull_project_local_repository`, `push_project_local_repository`, `create_project_remote_branch`, and `delete_project_remote_branch`.
- Task 4: `useProjectRepoSyncStatusQuery`, `usePushProjectLocalRepositoryMutation`, `projectCloneErrorPresentation`, and `projectBranchErrorMessage`.
- Task 5: `RepoSyncActionButton`, `useProjectRepositorySourceControls`, `useGithubAheadBehindQuery`, and `githubAheadBehindCounts`.
- Task 6: `maybeInstallE2eTauriMocks` in `desktop/src/testing/e2eBridge.ts` and the Playwright smoke `testMatch` in `desktop/playwright.config.ts`.

## Open Questions

1. **G4 on the project page.**
   The G3+G4 slice used `get_github_ahead_behind` for counts and Fetch-only chrome.
   This spec forbids calling G4 from the project page once a checkout exists, and there is no ahead/behind without a checkout.
   **Provisional default:** leave the G4 command and `useGithubAheadBehindQuery` in the tree.
   Remove the hook call from `useProjectRepositorySourceControls`.
   Update `desktop/tests/e2e/github-snapshot-and-fetch.spec.ts` so Fetch with a checkout calls `get_project_repo_sync_status` and does not call `get_github_ahead_behind`.

2. **Ahead/behind without a checkout.**
   Sync status already returns `ahead_count: 0`, `behind_count: 0`, and `can_*` false when there is no local path.
   Showing `0 / 0` there would invent a comparison.
   **Provisional default:** render `data-testid="repo-ahead-behind"` only when `githubHosted` and `localPath` is set and both counts are numbers.

3. **Structured auth errors on clone / toasts / create-branch dialog.**
   After the shared builder is used, HTTPS clone, sync, pull, push, and branch commands can fail with a JSON `ProjectPullRequestMergeError` string.
   Raw JSON in a toast is not recovery copy.
   **Provisional default:** parse that payload in the existing TypeScript wrappers and in `projectCloneErrorPresentation` / `projectBranchErrorMessage`.

4. **`isGithubRemote` vs `githubHosted`.**
   The spec names a new `isGithubRemote` field.
   `RepoSourceHeaderControls.githubHosted` already exists and already gates “Open on GitHub”.
   **Provisional default:** reuse `githubHosted`.
   Do not add a second boolean.

---

## File map

| File | Role |
|------|------|
| `desktop/src-tauri/src/commands/project_git_exec.rs` | `validate_git_operation_url`, host-aware `GitAuthConfig` helper string, `build_git_operation_auth_config`, `inspect_git_auth` |
| `desktop/src-tauri/src/commands/project_github_pull_request.rs` | `GhRunner::binary_path()` so the helper can be `!<gh> auth git-credential` |
| `desktop/src-tauri/src/commands/project_git.rs` | Sync / pull / push use the new URL gate + auth builder |
| `desktop/src-tauri/src/commands/project_git_branches.rs` | Create / delete use the new URL gate + auth builder |
| `desktop/src/shared/api/projectGit.ts` | Parse structured GitHub errors on sync / pull / push / clone / create / delete |
| Create: `desktop/src/features/projects/lib/projectGithubSync.ts` | Enable helper, count visibility, push side-effect gate, branch-disabled reason, primary-action helper |
| Create: `desktop/src/features/projects/lib/projectGithubSync.test.mjs` | Routing / header / push / G7 reason tests |
| `desktop/src/features/projects/repoSyncHooks.ts` | Enable GitHub sync after G1; skip `publishProjectPullRequestUpdate` on GitHub |
| `desktop/src/features/projects/lib/projectGitError.ts` | Clone recovery for structured GitHub codes |
| `desktop/src/features/projects/lib/projectGitError.test.mjs` | Clone presentation tests |
| `desktop/src/features/projects/lib/projectBranchErrors.ts` | Parse structured GitHub errors in the create/delete dialog |
| `desktop/src/features/projects/lib/projectBranchErrors.test.mjs` | Dialog copy tests |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Pass `githubStateReady` into the sync query |
| `desktop/src/features/projects/ui/CreatePullRequestDialog.tsx` | Same `githubStateReady` argument |
| `desktop/src/features/projects/ui/useProjectRepositorySourceControls.ts` | Pull/Push/Create/Delete on GitHub; Fetch runs sync when a checkout exists; no G4 |
| `desktop/src/features/projects/ui/ProjectRepositorySource.tsx` | GitHub uses the Buzz Pull / Push / Fetch control plus optional `ahead / behind` |
| `desktop/tests/e2e/github-snapshot-and-fetch.spec.ts` | Create/Delete visible; Fetch with checkout calls sync, not G4 |
| Create: `desktop/tests/e2e/github-pull-push-and-branches.spec.ts` | Pull, Push, Fetch-only, Create enabled, G1 auth disables Create |
| `desktop/playwright.config.ts` | Register the new spec in the smoke `testMatch` |

Do not modify `get_project_repo_snapshot` beyond leaving it on `validate_workspace_clone_url`.
Do not modify `get_github_ahead_behind` or `get_github_repository_snapshot` command bodies.

---

### Task 1: Accept GitHub clone URLs for git operations

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_git_exec.rs`

**Interfaces:**
- Consumes: `GitHubRepoRef::parse`, `validate_workspace_clone_url`, `validate_clone_url_against_relay`
- Produces:
  - `pub(crate) fn validate_git_operation_url(clone_url: &str, state: &AppState) -> Result<(), String>`
  - `fn validate_git_operation_url_against_relay(clone_url: &str, relay_base: &str) -> Result<(), String>`

- [ ] **Step 1: Write failing tests**

Add these tests next to `workspace_clone_url_requires_exact_relay_origin_and_prefix` in the existing `tests` module.
Import `validate_git_operation_url_against_relay`.

```rust
#[test]
fn git_operation_url_accepts_github_https_and_ssh() {
    let relay = "https://relay.example/prefix";
    assert!(validate_git_operation_url_against_relay(
        "https://github.com/acme/app",
        relay
    )
    .is_ok());
    assert!(validate_git_operation_url_against_relay(
        "https://github.com/acme/app.git",
        relay
    )
    .is_ok());
    assert!(validate_git_operation_url_against_relay(
        "git@github.com:acme/app.git",
        relay
    )
    .is_ok());
    assert!(validate_git_operation_url_against_relay(
        "ssh://git@github.com/acme/app.git",
        relay
    )
    .is_ok());
}

#[test]
fn git_operation_url_rejects_gitlab_and_still_requires_relay_for_buzz() {
    let owner = "a".repeat(64);
    let relay = "https://relay.example/prefix";
    assert!(
        validate_git_operation_url_against_relay("https://gitlab.com/acme/app", relay).is_err()
    );
    assert!(validate_git_operation_url_against_relay(
        &format!("https://relay.example/prefix/git/{owner}/repo"),
        relay
    )
    .is_ok());
    assert!(validate_git_operation_url_against_relay(
        &format!("https://evil.example/prefix/git/{owner}/repo"),
        relay
    )
    .is_err());
}
```

- [ ] **Step 2: Run the tests — expect compile fail**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib git_operation_url_accepts_github_https_and_ssh
```

Expected: FAIL with `cannot find function validate_git_operation_url_against_relay`.

- [ ] **Step 3: Implement the gate**

Keep `validate_workspace_clone_url` unchanged.
Add the new functions next to it.

```rust
/// Accept a Buzz workspace clone URL or a strict github.com HTTPS/SSH URL.
pub(crate) fn validate_git_operation_url(
    clone_url: &str,
    state: &AppState,
) -> Result<(), String> {
    let relay_base = crate::relay::relay_api_base_url_with_override(state);
    validate_git_operation_url_against_relay(clone_url, &relay_base)
}

fn validate_git_operation_url_against_relay(
    clone_url: &str,
    relay_base: &str,
) -> Result<(), String> {
    if GitHubRepoRef::parse(clone_url).is_ok() {
        return Ok(());
    }
    validate_clone_url_against_relay(clone_url, relay_base)
}
```

Do not call this from `get_project_repo_snapshot`.
Clone stays on `validate_local_clone_url` / `validate_local_clone_url_for_workspace`.

- [ ] **Step 4: Run tests**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib git_operation_url
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git add desktop/src-tauri/src/commands/project_git_exec.rs
git commit -s -m "feat(projects): accept GitHub URLs for git operations"
```

---

### Task 2: Host-aware Git credential helper

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_github_pull_request.rs`
- Modify: `desktop/src-tauri/src/commands/project_git_exec.rs`

**Interfaces:**
- Consumes: `GhRunner::{discover, from_resolved, ensure_auth}`, `remap_state_error`, `credential_helper_config_value`, `GitHubRepoRef::parse`
- Produces:
  - `impl GhRunner { pub(crate) fn binary_path(&self) -> &std::path::Path }`
  - `GitAuthConfig.credential_helper` changes from `Option<PathBuf>` to `Option<String>` (full `credential.helper` value)
  - `pub(crate) struct GitAuthInspection { pub credential_helper: Option<String>, pub injects_nostr_private_key: bool }`
  - `pub(crate) fn inspect_git_auth(auth: &GitAuthConfig, needs_credentials: bool) -> GitAuthInspection`
  - `pub(crate) fn github_command_error_string(error: ProjectPullRequestMergeError) -> String`
  - `pub(crate) fn build_github_git_auth_config_with<F>(clone_url: &str, discover: F) -> Result<GitAuthConfig, String> where F: FnOnce() -> Result<GhRunner, ProjectPullRequestMergeError>`
  - `pub(crate) fn build_git_operation_auth_config(clone_url: &str, state: &AppState) -> Result<GitAuthConfig, String>`
  - `build_git_clone_auth_config` becomes a one-line call to `build_git_operation_auth_config`

- [ ] **Step 1: Write failing builder tests**

Add a unix `fake_gh` helper copied from `project_github_repository_state.rs` into the `project_git_exec` tests module.
Match `*auth*status*` and `*auth*git-credential*` before any API path.
A `*) exit 1` arm must fail the test if git or the builder accidentally treats the helper as `gh api`.

```rust
fn error_json_code(error: &str) -> String {
    serde_json::from_str::<serde_json::Value>(error)
        .ok()
        .and_then(|value| value.get("code")?.as_str().map(ToString::to_string))
        .unwrap_or_default()
}

#[cfg(unix)]
fn fake_gh(script: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("create fake gh directory");
    let path = dir.path().join("gh");
    std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{script}\n")).expect("write fake gh");
    let mut permissions = std::fs::metadata(&path).expect("stat fake gh").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod fake gh");
    (dir, path)
}

#[cfg(unix)]
#[test]
fn https_github_auth_uses_gh_credential_helper_without_nsec() {
    let script = r#"
printf '%s\n' "$*" >> "${0%/gh}/calls"
case "$*" in
  *auth*status*) exit 0 ;;
  *auth*git-credential*) exit 0 ;;
  *) exit 1 ;;
esac
"#;
    let (dir, path) = fake_gh(script);
    let auth = build_github_git_auth_config_with("https://github.com/acme/app", || {
        GhRunner::from_resolved(Some(path.clone()))
    })
    .expect("https auth");
    let inspection = inspect_git_auth(&auth, true);
    let helper = inspection.credential_helper.expect("helper");
    assert!(helper.starts_with('!'), "{helper}");
    assert!(
        helper.ends_with(" auth git-credential"),
        "{helper}"
    );
    assert!(!inspection.injects_nostr_private_key);
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.lines().any(|line| line.contains("auth") && line.contains("status")));
    assert!(!calls.contains("/repos/"));
}

#[test]
fn ssh_github_auth_skips_helper_and_ensure_auth() {
    let auth = build_github_git_auth_config_with("git@github.com:acme/app.git", || {
        panic!("SSH GitHub must not discover gh");
    })
    .expect("ssh auth");
    let inspection = inspect_git_auth(&auth, true);
    assert_eq!(inspection.credential_helper, None);
    assert!(!inspection.injects_nostr_private_key);
}

#[test]
fn https_github_missing_gh_is_cli_missing_json() {
    let err = build_github_git_auth_config_with("https://github.com/acme/app", || {
        GhRunner::from_resolved(None)
    })
    .expect_err("missing gh");
    assert_eq!(error_json_code(&err), "github_cli_missing");
}

#[cfg(unix)]
#[test]
fn https_github_failed_ensure_auth_is_auth_required_json() {
    let (_dir, path) = fake_gh(
        r#"
case "$*" in
  *auth*status*) exit 1 ;;
  *) exit 1 ;;
esac
"#,
    );
    let err = build_github_git_auth_config_with("https://github.com/acme/app", || {
        GhRunner::from_resolved(Some(path))
    })
    .expect_err("auth");
    assert_eq!(error_json_code(&err), "github_auth_required");
}

#[test]
fn github_command_error_string_remaps_merge_failed() {
    let json = github_command_error_string(ProjectPullRequestMergeError::new(
        "github_merge_failed",
        "boom",
    ));
    assert_eq!(error_json_code(&json), "github_state_failed");
}

#[test]
fn buzz_auth_still_injects_nostr_helper_when_present() {
    let auth = build_git_auth_config_for_keys(&nostr::Keys::generate()).expect("buzz auth");
    assert!(!auth_nsec_for_test(&auth).is_empty());
    if let Some(helper) = inspect_git_auth(&auth, true).credential_helper {
        assert!(helper.contains("git-credential-nostr"), "{helper}");
        assert!(inspect_git_auth(&auth, true).injects_nostr_private_key);
    }
}
```

Add `#[cfg(test)] pub(crate) fn auth_nsec_for_test(auth: &GitAuthConfig) -> &str` next to the builder if the field stays private.

- [ ] **Step 2: Run the tests — expect compile fail**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib https_github_auth_uses_gh_credential_helper_without_nsec
```

Expected: FAIL because `build_github_git_auth_config_with` is missing.

- [ ] **Step 3: Implement the builder**

In `project_github_pull_request.rs`, add:

```rust
impl GhRunner {
    pub(crate) fn binary_path(&self) -> &std::path::Path {
        &self.binary
    }
}
```

In `project_git_exec.rs`, change `GitAuthConfig` to store the helper as a finished config value:

```rust
pub(crate) struct GitAuthConfig {
    git_path: std::path::PathBuf,
    credential_helper: Option<String>,
    nsec: String,
    allow_file_transport: bool,
}
```

Update `configure_git_auth` so a non-empty helper is applied as-is, and `NOSTR_PRIVATE_KEY` is set only when `needs_credentials && !auth.nsec.is_empty() && auth.credential_helper.is_some()`.

Update `build_git_auth_config_for_keys` to store `resolve_command("git-credential-nostr").map(|path| credential_helper_config_value(&path))`.

Add:

```rust
pub(crate) fn github_command_error_string(error: ProjectPullRequestMergeError) -> String {
    let remapped = remap_state_error(error, "");
    serde_json::to_string(&remapped).unwrap_or_else(|_| {
        "{\"code\":\"github_state_failed\",\"message\":\"GitHub authentication failed.\",\"recovery\":null}"
            .to_string()
    })
}

pub(crate) fn inspect_git_auth(auth: &GitAuthConfig, needs_credentials: bool) -> GitAuthInspection {
    let helper = if needs_credentials {
        auth.credential_helper.clone()
    } else {
        None
    };
    GitAuthInspection {
        injects_nostr_private_key: helper.is_some() && needs_credentials && !auth.nsec.is_empty(),
        credential_helper: helper,
    }
}

pub(crate) fn build_github_git_auth_config_with<F>(
    clone_url: &str,
    discover: F,
) -> Result<GitAuthConfig, String>
where
    F: FnOnce() -> Result<GhRunner, ProjectPullRequestMergeError>,
{
    let git_path =
        resolve_command("git").ok_or_else(|| "git was not found on PATH".to_string())?;
    if !clone_url.starts_with("https://github.com/") {
        return Ok(GitAuthConfig {
            git_path,
            credential_helper: None,
            nsec: String::new(),
            allow_file_transport: false,
        });
    }
    let gh = discover().map_err(github_command_error_string)?;
    gh.ensure_auth()
        .map_err(github_command_error_string)?;
    Ok(GitAuthConfig {
        git_path,
        credential_helper: Some(format!(
            "!{} auth git-credential",
            credential_helper_config_value(gh.binary_path())
        )),
        nsec: String::new(),
        allow_file_transport: false,
    })
}

pub(crate) fn build_git_operation_auth_config(
    clone_url: &str,
    state: &AppState,
) -> Result<GitAuthConfig, String> {
    if GitHubRepoRef::parse(clone_url).is_ok() {
        return build_github_git_auth_config_with(clone_url, GhRunner::discover);
    }
    build_git_auth_config(state)
}

pub(crate) fn build_git_clone_auth_config(
    clone_url: &str,
    state: &AppState,
) -> Result<GitAuthConfig, String> {
    build_git_operation_auth_config(clone_url, state)
}
```

Import `ProjectPullRequestMergeError`, `GhRunner`, and `remap_state_error`.
`project_github_repository_state` does not import `project_git_exec` today, so this import is not a cycle.

Do not put `NOSTR_PRIVATE_KEY` on GitHub HTTPS or SSH configs.
Do not call `discover` / `ensure_auth` for `git@github.com:` or `ssh://git@github.com/`.

- [ ] **Step 4: Run auth + existing git exec tests**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_git_auth && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_command_error_string && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib ssh_github_auth && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib https_github_ && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib buzz_auth && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib credential_helper && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib local_clone_url
```

Expected: PASS on unix.
Windows: keep SSH / missing-gh / remap tests; keep fake-gh tests `#[cfg(unix)]`.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git add desktop/src-tauri/src/commands/project_git_exec.rs \
  desktop/src-tauri/src/commands/project_github_pull_request.rs
git commit -s -m "feat(projects): use gh git-credential for GitHub HTTPS"
```

---

### Task 3: Point sync, pull, push, and branch commands at the new gate

**Files:**
- Modify: `desktop/src-tauri/src/commands/project_git.rs`
- Modify: `desktop/src-tauri/src/commands/project_git_branches.rs`

**Interfaces:**
- Consumes: `validate_git_operation_url`, `build_git_operation_auth_config`
- Produces: the same five command signatures, now accepting `github.com` clone URLs

Replace only these call sites:

- `get_project_repo_sync_status`
- `pull_project_local_repository`
- `push_project_local_repository`
- `create_project_remote_branch`
- `delete_project_remote_branch`

Each currently does:

```rust
validate_workspace_clone_url(&clone_url, &state)?;
let auth = build_git_auth_config(&state)?;
```

Change each to:

```rust
validate_git_operation_url(&clone_url, &state)?;
let auth = build_git_operation_auth_config(&clone_url, &state)?;
```

Leave `get_project_repo_snapshot` on `validate_workspace_clone_url` + `build_git_auth_config`.
Leave merge / diff / identity commands on the Buzz helper.

- [ ] **Step 1: Write the stale-commit lease test**

Add next to `remote_branch_create_and_delete_round_trip` in `project_git_branches.rs`.
Reuse the same file-remote fixture setup, then pass a wrong expected commit.

```rust
#[test]
fn create_remote_branch_rejects_stale_expected_commit() {
    let auth = build_test_git_auth_config().expect("build test git config");
    let root = tempfile::tempdir().expect("create test directory");
    let remote = root.path().join("remote.git");
    let worktree = root.path().join("worktree");
    let remote_path = remote.to_str().expect("remote path");
    let worktree_path = worktree.to_str().expect("worktree path");

    run_git(&["init", "--bare", "--", remote_path], None, &auth).expect("init remote");
    run_git(&["init", "--", worktree_path], None, &auth).expect("init worktree");
    std::fs::write(worktree.join("README.md"), "branch test\n").expect("write fixture");
    run_git(&["add", "README.md"], Some(&worktree), &auth).expect("stage");
    run_git(
        &[
            "-c",
            "user.name=Buzz Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "Initial commit",
        ],
        Some(&worktree),
        &auth,
    )
    .expect("commit");
    run_git(&["branch", "-M", "main"], Some(&worktree), &auth).expect("rename");
    run_git(
        &["remote", "add", "origin", remote_path],
        Some(&worktree),
        &auth,
    )
    .expect("remote");
    run_git(&["push", "origin", "main"], Some(&worktree), &auth).expect("push");
    let stale = "a".repeat(40);
    let err = create_remote_branch_blocking(remote_path, "main", &stale, "feature/demo", &auth)
        .expect_err("stale");
    assert!(err.contains("source branch changed") || err.contains("Refresh the repository"));
}
```

The existing `delete_remote_branch_blocking(..., "main", ...)` assertion already refuses the default branch.

- [ ] **Step 2: Run the new test against current wrappers — expect PASS on the blocking function, then switch the five commands**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib create_remote_branch_rejects_stale_expected_commit
```

Expected: PASS after the test compiles (the lease lives in the blocking function, which is unchanged).

- [ ] **Step 3: Switch the five command wrappers**

Update imports in both files.
Do not change command argument names or result types.

- [ ] **Step 4: Run branch + git lib tests**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib remote_branch && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib git_operation_url && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_git_auth
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git add desktop/src-tauri/src/commands/project_git.rs \
  desktop/src-tauri/src/commands/project_git_branches.rs
git commit -s -m "feat(projects): enable GitHub pull push and branch git commands"
```

---

### Task 4: TypeScript enablement, push side effect, and structured errors

**Files:**
- Create: `desktop/src/features/projects/lib/projectGithubSync.ts`
- Create: `desktop/src/features/projects/lib/projectGithubSync.test.mjs`
- Modify: `desktop/src/features/projects/repoSyncHooks.ts`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`
- Modify: `desktop/src/features/projects/ui/CreatePullRequestDialog.tsx`
- Modify: `desktop/src/shared/api/projectGit.ts`
- Modify: `desktop/src/features/projects/lib/projectGitError.ts`
- Modify: `desktop/src/features/projects/lib/projectGitError.test.mjs`
- Modify: `desktop/src/features/projects/lib/projectBranchErrors.ts`
- Modify: `desktop/src/features/projects/lib/projectBranchErrors.test.mjs`

**Interfaces:**
- Consumes: `isGitHubCloneUrl`, `parseProjectPullRequestMergeError`, `useProjectRepoHost`
- Produces:
  - `export function projectRepoSyncStatusEnabled(input: { cloneUrl?: string | null; buzzHost: boolean; githubStateReady: boolean }): boolean`
  - `export function githubSyncCountDisplay(input: { githubHosted: boolean; localPath?: string | null; aheadCount?: number | null; behindCount?: number | null }): { ahead: number; behind: number } | null`
  - `export function shouldPublishPullRequestUpdateAfterPush(cloneUrl?: string | null): boolean`
  - `export function githubBranchActionReason(input: { githubHosted: boolean; githubStateError?: unknown }): string | null`
  - `export function repoSyncPrimaryAction(input: { githubHosted: boolean; remoteKind?: "buzz" | "external"; hasExternalUrl?: boolean; canPull?: boolean; canPush?: boolean; hasFetch?: boolean }): "pull" | "push" | "fetch" | "open" | null`
  - `useProjectRepoSyncStatusQuery(..., options?: { githubStateReady?: boolean })`

- [ ] **Step 1: Write failing helper tests**

```js
import assert from "node:assert/strict";
import { test } from "node:test";
import { ProjectPullRequestMergeError } from "../../../shared/api/projectGit.ts";
import {
  githubBranchActionReason,
  githubSyncCountDisplay,
  projectRepoSyncStatusEnabled,
  repoSyncPrimaryAction,
  shouldPublishPullRequestUpdateAfterPush,
} from "./projectGithubSync.ts";

test("GitHub clone URL enables sync only after G1 succeeds", () => {
  assert.equal(
    projectRepoSyncStatusEnabled({
      cloneUrl: "https://github.com/acme/app",
      buzzHost: false,
      githubStateReady: false,
    }),
    false,
  );
  assert.equal(
    projectRepoSyncStatusEnabled({
      cloneUrl: "https://github.com/acme/app",
      buzzHost: false,
      githubStateReady: true,
    }),
    true,
  );
  assert.equal(
    projectRepoSyncStatusEnabled({
      cloneUrl: "https://gitlab.com/acme/app",
      buzzHost: false,
      githubStateReady: true,
    }),
    false,
  );
  assert.equal(
    projectRepoSyncStatusEnabled({
      cloneUrl: `https://relay.example/git/${"ab".repeat(32)}/app`,
      buzzHost: true,
      githubStateReady: false,
    }),
    true,
  );
});

test("GitHub header uses Pull Push Fetch and not Open", () => {
  assert.equal(
    repoSyncPrimaryAction({
      githubHosted: true,
      remoteKind: "external",
      hasExternalUrl: true,
      canPull: true,
      canPush: false,
      hasFetch: true,
    }),
    "pull",
  );
  assert.equal(
    repoSyncPrimaryAction({
      githubHosted: true,
      remoteKind: "external",
      hasExternalUrl: true,
      canPull: false,
      canPush: true,
      hasFetch: true,
    }),
    "push",
  );
  assert.equal(
    repoSyncPrimaryAction({
      githubHosted: true,
      remoteKind: "external",
      hasExternalUrl: true,
      canPull: false,
      canPush: false,
      hasFetch: true,
    }),
    "fetch",
  );
  assert.equal(
    repoSyncPrimaryAction({
      githubHosted: false,
      remoteKind: "external",
      hasExternalUrl: true,
      canPull: true,
      hasFetch: true,
    }),
    "open",
  );
});

test("GitHub counts require a local checkout", () => {
  assert.equal(
    githubSyncCountDisplay({
      githubHosted: true,
      localPath: null,
      aheadCount: 0,
      behindCount: 0,
    }),
    null,
  );
  assert.deepEqual(
    githubSyncCountDisplay({
      githubHosted: true,
      localPath: "/tmp/repo",
      aheadCount: 0,
      behindCount: 0,
    }),
    { ahead: 0, behind: 0 },
  );
});

test("GitHub push does not publish a Nostr pull request update", () => {
  assert.equal(
    shouldPublishPullRequestUpdateAfterPush("https://github.com/acme/app"),
    false,
  );
  assert.equal(
    shouldPublishPullRequestUpdateAfterPush(
      `https://relay.example/git/${"ab".repeat(32)}/app`,
    ),
    true,
  );
});

test("G1 errors disable Create with recovery copy instead of first-commit copy", () => {
  assert.equal(
    githubBranchActionReason({
      githubHosted: true,
      githubStateError: new ProjectPullRequestMergeError(
        "github_auth_required",
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        null,
      ),
    }),
    "Authenticate GitHub CLI with: gh auth login --hostname github.com",
  );
  assert.equal(
    githubBranchActionReason({
      githubHosted: true,
      githubStateError: new ProjectPullRequestMergeError(
        "github_cli_missing",
        "Install the GitHub CLI to continue.",
        null,
      ),
    }),
    "Install GitHub CLI, then retry.",
  );
  assert.equal(
    githubBranchActionReason({
      githubHosted: false,
      githubStateError: new ProjectPullRequestMergeError(
        "github_auth_required",
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        null,
      ),
    }),
    null,
  );
});
```

Add clone and branch-error tests:

```js
// projectGitError.test.mjs
test("presents structured GitHub CLI recovery on clone", () => {
  assert.deepEqual(
    projectCloneErrorPresentation(
      new ProjectPullRequestMergeError(
        "github_cli_missing",
        "Install the GitHub CLI to continue.",
        null,
      ),
      "https://github.com/acme/app",
    ),
    {
      title: "GitHub CLI is required",
      description: "Install GitHub CLI, then retry.",
    },
  );
});

// projectBranchErrors.test.mjs
test("maps structured GitHub auth errors for the branch dialog", () => {
  assert.equal(
    projectBranchErrorMessage(
      new ProjectPullRequestMergeError(
        "github_auth_required",
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        null,
      ),
      "Failed to create branch.",
    ),
    "Authenticate GitHub CLI with: gh auth login --hostname github.com",
  );
});
```

Import `ProjectPullRequestMergeError` in those test files.

- [ ] **Step 2: Run tests — expect fail**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test -- src/features/projects/lib/projectGithubSync.test.mjs
```

Expected: FAIL because `projectGithubSync.ts` is missing.

- [ ] **Step 3: Implement helpers and wire callers**

`projectGithubSync.ts`:

```ts
import { isGitHubCloneUrl } from "@/features/projects/lib/projectGitError";
import { parseProjectPullRequestMergeError } from "@/shared/api/projectGit";

/** Enable Buzz sync always; enable GitHub sync only after G1 succeeds. */
export function projectRepoSyncStatusEnabled(input: {
  cloneUrl?: string | null;
  buzzHost: boolean;
  githubStateReady: boolean;
}) {
  if (!input.cloneUrl) return false;
  if (input.buzzHost) return true;
  return isGitHubCloneUrl(input.cloneUrl) && input.githubStateReady;
}

/** Visible ahead/behind only when a GitHub checkout exists. */
export function githubSyncCountDisplay(input: {
  githubHosted: boolean;
  localPath?: string | null;
  aheadCount?: number | null;
  behindCount?: number | null;
}) {
  if (!input.githubHosted || !input.localPath) return null;
  if (typeof input.aheadCount !== "number" || typeof input.behindCount !== "number") {
    return null;
  }
  return { ahead: input.aheadCount, behind: input.behindCount };
}

/** GitHub push must not publish a Nostr pull-request update. */
export function shouldPublishPullRequestUpdateAfterPush(
  cloneUrl?: string | null,
) {
  return !isGitHubCloneUrl(cloneUrl);
}

/** G1 recovery reason for visible-but-disabled Create/Delete. */
export function githubBranchActionReason(input: {
  githubHosted: boolean;
  githubStateError?: unknown;
}) {
  if (!input.githubHosted || input.githubStateError == null) return null;
  const parsed = parseProjectPullRequestMergeError(input.githubStateError);
  if (parsed?.code === "github_cli_missing") {
    return "Install GitHub CLI, then retry.";
  }
  if (parsed?.code === "github_auth_required") {
    return parsed.message;
  }
  if (parsed) return parsed.message;
  return input.githubStateError instanceof Error
    ? input.githubStateError.message
    : "Could not load GitHub branches.";
}

/** Header primary control. GitHub uses Buzz actions; other externals stay Open. */
export function repoSyncPrimaryAction(input: {
  githubHosted: boolean;
  remoteKind?: "buzz" | "external";
  hasExternalUrl?: boolean;
  canPull?: boolean;
  canPush?: boolean;
  hasFetch?: boolean;
}) {
  if (!input.githubHosted && input.remoteKind === "external") {
    return input.hasExternalUrl ? "open" : null;
  }
  if (input.canPull) return "pull";
  if (input.canPush) return "push";
  return input.hasFetch ? "fetch" : null;
}
```

In `repoSyncHooks.ts`:

```ts
export function useProjectRepoSyncStatusQuery(
  project: Project | null | undefined,
  reposDir?: string | null,
  branchName?: string | null,
  baseBranch?: string | null,
  options?: { githubStateReady?: boolean },
) {
  const host = useProjectRepoHost(project);
  return useQuery({
    enabled: projectRepoSyncStatusEnabled({
      cloneUrl: project?.cloneUrls[0],
      buzzHost: host.kind === "buzz",
      githubStateReady: options?.githubStateReady ?? false,
    }),
    // existing queryKey / queryFn / staleTime / refetchInterval unchanged
  });
}
```

In `usePushProjectLocalRepositoryMutation`, wrap the `publishProjectPullRequestUpdate` block:

```ts
if (
  shouldPublishPullRequestUpdateAfterPush(project.cloneUrls[0]) &&
  pullRequest &&
  (pullRequest.status === "Open" || pullRequest.status === "Draft")
) {
  // existing publish try/catch
}
```

In `ProjectDetailScreen.tsx` and both `CreatePullRequestDialog.tsx` calls, pass `{ githubStateReady: repoStateQuery.isSuccess }`.

In `projectGit.ts`, wrap `getProjectRepoSyncStatus`, `pushProjectLocalRepository`, `pullProjectLocalRepository`, `cloneProjectRepository`, `createProjectRemoteBranch`, and `deleteProjectRemoteBranch` with:

```ts
try {
  // existing invoke
} catch (error) {
  throw parseProjectPullRequestMergeError(error) ?? error;
}
```

In `projectCloneErrorPresentation`, if `github` and `parseProjectPullRequestMergeError(error)` is set, return the G1/merge titles (`GitHub CLI is required` / `GitHub authentication required` / `Could not load GitHub branches`) instead of scanning git stderr.

In `projectBranchErrorMessage`, return `parseProjectPullRequestMergeError(error)?.message` when present, before the `instanceof Error` path.

- [ ] **Step 4: Run unit tests and typecheck**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test -- src/features/projects/lib/projectGithubSync.test.mjs src/features/projects/lib/projectGitError.test.mjs src/features/projects/lib/projectBranchErrors.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git add desktop/src/features/projects/lib/projectGithubSync.ts \
  desktop/src/features/projects/lib/projectGithubSync.test.mjs \
  desktop/src/features/projects/repoSyncHooks.ts \
  desktop/src/features/projects/ui/ProjectDetailScreen.tsx \
  desktop/src/features/projects/ui/CreatePullRequestDialog.tsx \
  desktop/src/shared/api/projectGit.ts \
  desktop/src/features/projects/lib/projectGitError.ts \
  desktop/src/features/projects/lib/projectGitError.test.mjs \
  desktop/src/features/projects/lib/projectBranchErrors.ts \
  desktop/src/features/projects/lib/projectBranchErrors.test.mjs
git commit -s -m "feat(projects): enable GitHub sync status after repository state"
```

---

### Task 5: Header Pull / Push / Fetch and visible Create / Delete

**Files:**
- Modify: `desktop/src/features/projects/ui/ProjectRepositorySource.tsx`
- Modify: `desktop/src/features/projects/ui/useProjectRepositorySourceControls.ts`
- Modify: `desktop/tests/e2e/github-snapshot-and-fetch.spec.ts`

**Interfaces:**
- Consumes: `repoSyncPrimaryAction`, `githubSyncCountDisplay`, `githubBranchActionReason`
- Produces: GitHub header matches Buzz; Create/Delete stay in the branch menu; Fetch with a checkout refetches sync status

- [ ] **Step 1: Rewrite `RepoSyncActionButton` to use the helper**

Replace the `if (controls.githubHosted) { always Fetch }` branch.
Keep non-GitHub `remoteKind === "external"` as Open.

```tsx
export function RepoSyncActionButton({
  controls,
}: {
  controls: RepoSourceHeaderControls;
}) {
  const action = repoSyncPrimaryAction({
    githubHosted: Boolean(controls.githubHosted),
    remoteKind: controls.remoteKind,
    hasExternalUrl: Boolean(controls.externalUrl),
    canPull: Boolean(controls.canPull && controls.onPull),
    canPush: Boolean(controls.canPush && controls.onPush),
    hasFetch: Boolean(controls.onFetch),
  });

  if (action === "open") {
    // existing Open <a> for non-GitHub externals
  }

  const counts =
    controls.githubHosted &&
    controls.aheadCount != null &&
    controls.behindCount != null ? (
      <span
        className="font-mono text-2xs text-muted-foreground"
        data-testid="repo-ahead-behind"
      >
        {controls.aheadCount} / {controls.behindCount}
      </span>
    ) : null;

  // existing Pull / Push / Fetch buttons, wrapped in:
  // <div className="flex items-center gap-2">{counts}{button}</div>
  // when counts is non-null; otherwise return the button alone.
}
```

Pull still appends ` ${count}` when `behindCount > 0`.
Hide Pull unless `canPull && onPull`.
Hide Push unless `canPush && onPush`.

- [ ] **Step 2: Wire source controls**

In `useProjectRepositorySourceControls.ts`:

- Delete the `useGithubAheadBehindQuery` call and the `githubAheadBehindCounts` import.
- Keep Create/Delete handlers for GitHub (remove the `githubHosted ? undefined : …` branches).
- If `githubBranchActionReason({ githubHosted, githubStateError: repoStateQuery.error })` is non-null, use that string as both titles and force both actions disabled.
- Do not call `projectBranchCreationReason` when that GitHub reason is set.
- `canPush` / `canPull` / `onPush` / `onPull` use the same rules as Buzz (`!selectedTag` and sync `can_*`).
- Counts:

```ts
const githubCounts = githubSyncCountDisplay({
  githubHosted,
  localPath: repoSyncStatusQuery.data?.localPath,
  aheadCount: repoSyncStatusQuery.data?.aheadCount,
  behindCount: repoSyncStatusQuery.data?.behindCount,
});
aheadCount: githubHosted
  ? (githubCounts?.ahead ?? null)
  : (repoSyncStatusQuery.data?.aheadCount ?? null),
behindCount: githubHosted
  ? (githubCounts?.behind ?? null)
  : (repoSyncStatusQuery.data?.behindCount ?? null),
```

- Fetch:

```ts
const hasCheckout = Boolean(
  repoSyncStatusQuery.data?.localPath || localRepoSnapshotQuery.data,
);
const handleFetchRepo = React.useCallback(async () => {
  const tasks = githubHosted
    ? [
        repoStateQuery.refetch(),
        ...(repoStateQuery.isError
          ? []
          : [
              repoSnapshotQuery.refetch(),
              ...(hasCheckout ? [repoSyncStatusQuery.refetch()] : []),
            ]),
      ]
    : [
        repoSnapshotQuery.refetch(),
        repoStateQuery.refetch(),
        repoSyncStatusQuery.refetch(),
      ];
  // existing toast handling
}, [/* include hasCheckout and repoSyncStatusQuery.refetch; omit G4 */]);
```

- `fetchPending` includes `repoSyncStatusQuery.isFetching` when `githubHosted && hasCheckout`.
- `fetchTitle` is `"Check for remote changes"` when `hasCheckout`, otherwise `"Refresh GitHub README and files"`.
- Do not refetch `get_github_ahead_behind`.

“Open on GitHub” on `RepoSourceDropdown` is already gated by `githubHosted && externalUrl`.
Leave that item in place.

- [ ] **Step 3: Update the G3+G4 smoke spec so it matches G5**

In `desktop/tests/e2e/github-snapshot-and-fetch.spec.ts`:

1. First test (no checkout): keep Fetch as primary, keep no Pull/Push, keep Open on the dropdown.
   Change Create/Delete from `toHaveCount(0)` to visible and enabled:

```ts
await header.getByRole("button").filter({ hasText: "develop" }).click();
await expect(page.getByTestId("project-create-branch")).toBeEnabled();
await expect(page.getByTestId("project-delete-branch")).toBeVisible();
```

2. Third test (local HEAD, `0 / 0`): after clicking Fetch, assert:

```ts
.toEqual(
  expect.arrayContaining([
    "get_github_repository_state",
    "get_github_repository_snapshot",
    "get_project_repo_sync_status",
  ]),
);
expect(commands).not.toContain("get_github_ahead_behind");
```

Keep the `0 / 0` assertion.

- [ ] **Step 4: Typecheck and run the focused unit tests**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test -- src/features/projects/lib/projectGithubSync.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git add desktop/src/features/projects/ui/ProjectRepositorySource.tsx \
  desktop/src/features/projects/ui/useProjectRepositorySourceControls.ts \
  desktop/tests/e2e/github-snapshot-and-fetch.spec.ts
git commit -s -m "feat(projects): show GitHub pull push and branch actions"
```

---

### Task 6: Mock-bridge smoke for Pull, Push, Fetch-only, and G7

**Files:**
- Create: `desktop/tests/e2e/github-pull-push-and-branches.spec.ts`
- Modify: `desktop/playwright.config.ts` (`smoke` `testMatch` adds `**/github-pull-push-and-branches.spec.ts`)

**Interfaces:**
- Reuse `__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__`, `__BUZZ_E2E_PROJECT_REPO_SYNC_STATUS__`, and `__BUZZ_E2E_GITHUB_REPO_STATE_ERROR__`
- Do not add live GitHub

Copy the `enableProjectsFeature` / `openBuzzProject` helpers from `github-snapshot-and-fetch.spec.ts` into the new file (do not import from that spec).

Shared checkout fixture fields (inline inside each `addInitScript`; do not close over Node constants):

```ts
local_path: "/tmp/buzz/REPOS/acme-app"
local_branch: "develop"
local_head / remote_head / merge_base: "d".repeat(40)
```

- [ ] **Step 1: Write the four smoke tests**

```ts
test("GitHub checkout that is behind shows Pull as the primary control", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
    const sha = "d".repeat(40);
    window.__BUZZ_E2E_PROJECT_REPO_SYNC_STATUS__ = {
      local_path: "/tmp/buzz/REPOS/acme-app",
      local_branch: "develop",
      local_branches: ["develop"],
      local_head: sha,
      local_short_head: sha.slice(0, 7),
      remote_branch: "develop",
      remote_head: sha,
      remote_short_head: sha.slice(0, 7),
      merge_base: sha,
      ahead_count: 0,
      behind_count: 1,
      has_uncommitted_changes: false,
      has_untracked_files: false,
      can_push: false,
      push_block_reason: null,
      can_pull: true,
      pull_block_reason: null,
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  const header = page.getByTestId("project-repository-selection-row");
  await expect(header.getByRole("button", { name: /Pull/ })).toBeVisible({
    timeout: 10_000,
  });
  await expect(header.getByRole("link", { name: /^Open$/ })).toHaveCount(0);
  await page.evaluate(() => {
    window.__BUZZ_E2E_COMMANDS__ = [];
  });
  await header.getByRole("button", { name: /Pull/ }).click();
  await expect
    .poll(() => page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []))
    .toContain("pull_project_local_repository");
});

test("GitHub checkout that is ahead shows Push and does not open a Nostr publish path", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
    const sha = "d".repeat(40);
    window.__BUZZ_E2E_PROJECT_REPO_SYNC_STATUS__ = {
      local_path: "/tmp/buzz/REPOS/acme-app",
      local_branch: "develop",
      local_branches: ["develop"],
      local_head: sha,
      local_short_head: sha.slice(0, 7),
      remote_branch: "develop",
      remote_head: sha,
      remote_short_head: sha.slice(0, 7),
      merge_base: sha,
      ahead_count: 1,
      behind_count: 0,
      has_uncommitted_changes: false,
      has_untracked_files: false,
      can_push: true,
      push_block_reason: null,
      can_pull: false,
      pull_block_reason: null,
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  const header = page.getByTestId("project-repository-selection-row");
  await expect(header.getByRole("button", { name: /^Push$/ })).toBeVisible({
    timeout: 10_000,
  });
  await page.evaluate(() => {
    window.__BUZZ_E2E_COMMANDS__ = [];
  });
  await header.getByRole("button", { name: /^Push$/ }).click();
  await expect
    .poll(() => page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []))
    .toContain("push_project_local_repository");
  const commands = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(commands.some((command) => command.includes("pull_request"))).toBe(
    false,
  );
});

test("GitHub without a checkout keeps Fetch and still allows Create", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  const header = page.getByTestId("project-repository-selection-row");
  await expect(header.getByRole("button", { name: /^Fetch$/ })).toBeVisible({
    timeout: 10_000,
  });
  await expect(header.getByRole("button", { name: /Pull/ })).toHaveCount(0);
  await expect(header.getByRole("button", { name: /Push/ })).toHaveCount(0);
  await header.getByRole("button").filter({ hasText: "develop" }).click();
  await expect(page.getByTestId("project-create-branch")).toBeEnabled();
  await page.getByTestId("project-create-branch").click();
  await waitForAnimations(page);
  await page.getByTestId("project-create-branch-name").fill("feature/demo");
  await page.evaluate(() => {
    window.__BUZZ_E2E_COMMANDS__ = [];
  });
  await page.getByTestId("project-create-branch-submit").click();
  await expect
    .poll(() => page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []))
    .toContain("create_project_remote_branch");
});

test("GitHub auth recovery disables Create with that reason and does not start sync", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await page.addInitScript(() => {
    window.__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__ =
      "https://github.com/acme/app";
    window.__BUZZ_E2E_GITHUB_REPO_STATE_ERROR__ = {
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
  const header = page.getByTestId("project-repository-selection-row");
  await expect(header.getByRole("button", { name: /Pull/ })).toHaveCount(0);
  await expect(header.getByRole("button", { name: /Push/ })).toHaveCount(0);
  await expect(header.getByText("create the first commit", { exact: false })).toHaveCount(0);
  const commands = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(commands).toContain("get_github_repository_state");
  expect(commands).not.toContain("get_project_repo_sync_status");
  expect(commands).not.toContain("create_project_remote_branch");
  expect(commands).not.toContain("get_github_ahead_behind");
});
```

Register the spec in `desktop/playwright.config.ts` immediately after `**/github-snapshot-and-fetch.spec.ts`.

Build with `pnpm build:e2e` (via `pnpm test:e2e:smoke`).
Never `pnpm run build`.
Call `addInitScript` before `installMockBridge`.
Call `waitForAnimations` before any screenshot; these tests assert roles, not screenshots.

- [ ] **Step 2: Run the new spec and the two existing GitHub smokes**

```bash
. ./bin/activate-hermit
cd desktop && pnpm test:e2e:smoke -- github-pull-push-and-branches
cd desktop && pnpm test:e2e:smoke -- github-snapshot-and-fetch
cd desktop && pnpm test:e2e:smoke -- github-repo-state
```

Expected: PASS.
If `test:e2e:smoke` does not accept a file filter, run `pnpm build:e2e && pnpm exec playwright test --project=smoke tests/e2e/github-pull-push-and-branches.spec.ts`.

- [ ] **Step 3: Commit**

```bash
. ./bin/activate-hermit
if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi
git add desktop/tests/e2e/github-pull-push-and-branches.spec.ts \
  desktop/playwright.config.ts
git commit -s -m "test(e2e): cover GitHub pull push and branch recovery"
```

---

## Spec coverage

| Spec / requirement | Task |
|--------------------|------|
| `validate_git_operation_url` accepts GitHub HTTPS/SSH; rejects GitLab; Buzz still needs the active relay | 1 |
| Do not call the new gate from `get_project_repo_snapshot` | 1 + 3 |
| One host-aware auth builder for clone, sync, pull, push, create, delete | 2 + 3 |
| HTTPS helper is `!<gh> auth git-credential`; no `NOSTR_PRIVATE_KEY` | 2 |
| SSH: no helper, no nsec, no `ensure_auth` | 2 |
| Missing `gh` / failed `ensure_auth` return structured JSON codes | 2 + 4 |
| Remap `github_merge_failed` | 2 |
| Buzz path still uses `git-credential-nostr` | 2 |
| Fake-`gh` must not treat credential-helper invocations as API paths | 2 |
| Five commands use the new gate + builder | 3 |
| Create/delete still lease-check the expected commit | 3 |
| Sync query enabled for GitHub only after G1; GitLab stays off | 4 |
| Push does not call `publishProjectPullRequestUpdate` | 4 |
| Structured errors parsed for clone / toasts / branch dialog | 4 |
| Header: GitHub + behind → Pull; ahead → Push; else Fetch; not Open-as-primary | 4 + 5 + 6 |
| Open stays on the source dropdown | 5 (already present) |
| Fetch with checkout refetches sync (git fetch) + G1 + G3; no G4 | 5 |
| Fetch without checkout is query-only | 5 + 6 |
| Create/Delete stay visible; G1 error uses recovery copy, not “create the first commit” | 4 + 5 + 6 |
| Hide Pull/Push unless `can_pull` / `can_push` | 5 + 6 |
| If G1 failed, do not start sync / create / delete | 4 + 5 + 6 |
| User-visible e2e | 5 (updated G3 spec) + 6 |
| Existing Buzz sync/branch tests stay green | 3 + 4 + 5 |

## Acceptance criteria

On a GitHub-hosted project whose default branch is `develop`, with `gh` installed and authenticated:

- A checkout that is behind shows Pull; Pull invokes `pull_project_local_repository` (`git pull --ff-only`).
- A checkout that is ahead shows Push; Push invokes `push_project_local_repository` and does not publish a Nostr PR update.
- Fetch with a checkout refetches `get_project_repo_sync_status`, `get_github_repository_state`, and `get_github_repository_snapshot`.
- Fetch without a checkout does not call sync and does not show Pull/Push.
- Create branch from a known G1 HEAD works.
- Delete remains visible and refuses the default branch (`develop`) through the existing guard.
- Without `gh` or without auth on HTTPS, G1 recovery stays on the header; Create/Delete are visible and disabled with that reason; they do not say “create the first commit”.
- Buzz-hosted repositories still sync, pull, push, and create/delete as they do today.

## Validation commands

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib git_operation_url
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_git_auth
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib https_github_
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib ssh_github_auth
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib remote_branch
. ./bin/activate-hermit
cd desktop && pnpm test -- src/features/projects/lib/projectGithubSync.test.mjs src/features/projects/lib/projectGitError.test.mjs src/features/projects/lib/projectBranchErrors.test.mjs && pnpm typecheck
. ./bin/activate-hermit
cd desktop && pnpm test:e2e:smoke -- github-pull-push-and-branches
cd desktop && pnpm test:e2e:smoke -- github-snapshot-and-fetch
cd desktop && pnpm test:e2e:smoke -- github-repo-state
```

## GSTACK REVIEW REPORT

| Review | Trigger | Why | Runs | Status | Findings |
|--------|---------|-----|------|--------|----------|
| CEO Review | `/plan-ceo-review` | Scope & strategy | 0 | skipped | — |
| Codex Review | `/codex review` | Independent 2nd opinion | 0 | skipped | — |
| Eng Review | `/plan-eng-review` | Architecture & tests (required) | 0 | skipped | — |
| Design Review | `/plan-design-review` | UI/UX gaps | 0 | skipped | — |
| DX Review | `/plan-devex-review` | Developer experience gaps | 0 | skipped | — |

- **VERDICT:** Plan written from the approved G5–G7 spec against the current G1–G4 tree.
- Ready to implement this revision.

NO UNRESOLVED DECISIONS
