# GitHub pull, push, and remote branches Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let strict `github.com` HTTPS and SSH repository URLs use the existing Desktop clone, sync-status, pull, push, create-branch, and delete-branch workflows with host-correct authentication and the same contextual Pull, Push, or Fetch header control used by Buzz-hosted repositories.

**Architecture:** Keep the existing Tauri git commands and widen only their URL/authentication policy through `validate_git_operation_url` and `build_git_operation_auth_config`.
GitHub HTTPS uses an explicitly quoted `!<gh-path> auth git-credential` helper after `GhRunner::discover` and `ensure_auth`, GitHub SSH uses the user's SSH agent without `gh` git authentication, and Buzz URLs keep `git-credential-nostr` plus the signing key.
The project page enables git sync only after GitHub repository state (G1) succeeds, uses sync status as the sole checkout ahead/behind source, and keeps branch actions visible with accurate recovery reasons.

**Tech Stack:** Tauri 2, Rust, system `git`, GitHub CLI `gh`, React 19, TypeScript, TanStack React Query, Node test runner, and Playwright's E2E mock bridge.

**Spec:** [2026-08-18-github-pull-push-and-branches-design.md](../specs/2026-08-18-github-pull-push-and-branches-design.md)

**Product direction:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) make `github.com` a first-class Projects git host while Buzz remains the collaboration layer.

## Global Constraints

- Implement G5, G6, and G7 only.
- Do not add a pull, push, create-branch, or delete-branch Tauri command.
- Do not add `gh api` ref mutations.
- Do not implement GitHub Issues, GitHub pull-request list/create, branch-to-channel creation, `default-branch` persistence, GitHub tags, GitHub Enterprise, GitLab, or stored OAuth tokens.
- Do not open `get_project_repo_snapshot` to GitHub.
- Do not modify the bodies of `get_github_repository_snapshot` or `get_github_ahead_behind`.
- Leave `get_github_ahead_behind` and `useGithubAheadBehindQuery` in the tree, but remove the project-page hook call so no project-page workflow invokes G4.
- When a checkout exists, `get_project_repo_sync_status` is the only ahead/behind source and each refetch performs the existing `git fetch`.
- When no checkout exists, Fetch refetches G1 and G3 only and must not refetch sync status.
- Do not invent `0 / 0`, `canPull`, or `canPush` after a failed sync or when no checkout exists.
- Do not start sync, create, or delete after G1 has failed.
- Keep Create and Delete reachable in the branch menu even while G1 is failed, but disable both with the GitHub recovery reason.
- Keep Pull and Push hidden unless `can_pull` or `can_push` is true.
- Reuse `RepoSourceHeaderControls.githubHosted`; do not add a duplicate `isGithubRemote` flag.
- Keep “Open on GitHub” in `RepoSourceDropdown`; only the primary header action changes from Open to Pull, Push, or Fetch.
- Skip `publishProjectPullRequestUpdate` after a GitHub push.
- Never persist a GitHub token.
- Explicitly remove inherited `NOSTR_PRIVATE_KEY` from every spawned `git` and `gh` process, then add it back only for a Buzz credential-helper invocation.
- Keep Buzz `/git/<64-hex>/<id>` clone, sync, pull, push, create, and delete on `git-credential-nostr` and the active-relay URL gate.
- Return HTTPS GitHub `GhRunner::discover` and `ensure_auth` failures from string-returning commands as serialized `ProjectPullRequestMergeError` JSON.
- Remap `github_merge_failed` to `github_state_failed` before serializing it from the git-auth path.
- The only structured codes emitted by the git-auth path are `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, and `github_state_failed`.
- Keep protected-branch, non-fast-forward, SSH permission, and other `git` stderr failures as plain strings.
- Ensure the fake `gh` distinguishes `auth git-credential` from `api` calls and fails on every unexpected argument list.
- Do not add `unsafe`, production `unwrap()`, or production `expect()`.
- Add doc comments to new exported TypeScript APIs and new Rust APIs visible outside their module.
- Use existing rem-based text tokens only.
- Activate Hermit in every shell with `. ./bin/activate-hermit && ...`.
- Run commands from the repository root because shell working directories do not persist between tool calls.
- Before each task's first symbol edit, run the Required Impact Checks when GitNexus MCP tools are available and warn before editing on HIGH or CRITICAL risk.
- Before each commit, run GitNexus change detection when available, inspect `git diff --check`, and commit with `git commit -s`.

---

## Resolved Decisions

- G4 remains implemented for its own slice, but the project page stops calling it.
- Ahead/behind text renders for GitHub only when sync status has a non-empty `localPath` and numeric counts.
- The existing TypeScript command wrappers parse serialized GitHub errors so queries, toasts, clone recovery, and branch dialogs receive `ProjectPullRequestMergeError` objects.
- The existing `githubHosted` flag is the single host flag passed to repository header controls.
- A failed G1 still renders an interactive branch-picker trigger labelled `—`; its menu contains disabled Create and Delete items plus the recovery reason.
- The `gh` helper path is single-quoted for Git's shell helper syntax so installed paths containing spaces or apostrophes are safe.
- No live GitHub repository is used by automated tests; fake `gh`, local file remotes, pure policy tests, and the E2E mock bridge provide deterministic coverage.

## File Map

| File | Responsibility in this change |
|------|--------------------------------|
| `desktop/src-tauri/src/commands/project_git_exec.rs` | GitHub-or-Buzz URL gate, host-aware auth builder, helper quoting, inherited-secret removal, and Rust unit tests |
| `desktop/src-tauri/src/commands/project_github_pull_request.rs` | Expose the resolved `gh` path and remove inherited `NOSTR_PRIVATE_KEY` from every `gh` process |
| `desktop/src-tauri/src/commands/project_git.rs` | Route sync, pull, and push through the new URL gate and auth builder while leaving remote snapshot unchanged |
| `desktop/src-tauri/src/commands/project_git_branches.rs` | Route create and delete through the new URL gate and auth builder while retaining lease/default-branch guards |
| `desktop/src/shared/api/projectGit.ts` | Parse structured GitHub errors at the six affected TypeScript command boundaries |
| `desktop/src/features/projects/lib/projectGitError.ts` | Present structured GitHub clone failures before generic git-stderr classification |
| `desktop/src/features/projects/lib/projectGitError.test.mjs` | Clone recovery regression tests |
| `desktop/src/features/projects/lib/projectBranchErrors.ts` | Present structured command errors and derive the disabled GitHub branch-action reason |
| `desktop/src/features/projects/lib/projectBranchErrors.test.mjs` | Branch dialog and disabled-reason tests |
| Create `desktop/src/features/projects/lib/projectGithubSync.ts` | Pure host-routing policies for sync enablement, counts, primary action, and push side effects |
| Create `desktop/src/features/projects/lib/projectGithubSync.test.mjs` | Pure policy tests for GitHub and non-GitHub repositories |
| `desktop/src/features/projects/repoSyncHooks.ts` | Enable sync after G1 and skip the Nostr PR update for GitHub push |
| `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Pass G1 readiness to the main sync query |
| `desktop/src/features/projects/ui/CreatePullRequestDialog.tsx` | Pass G1 readiness to both dialog sync queries |
| `desktop/src/features/projects/ui/ProjectRepositorySource.tsx` | Render contextual Pull/Push/Fetch and keep the failed-G1 branch menu reachable |
| `desktop/src/features/projects/ui/useProjectRepositorySourceControls.ts` | Remove G4, coordinate Fetch, enable GitHub branch/pull/push handlers, counts, and recovery |
| `desktop/tests/e2e/github-snapshot-and-fetch.spec.ts` | Update no-checkout and checkout Fetch expectations and branch-menu visibility |
| Create `desktop/tests/e2e/github-pull-push-and-branches.spec.ts` | Cover Pull, Push, create/delete, and G1 recovery through the mock bridge |
| `desktop/playwright.config.ts` | Register the new smoke spec |

Do not modify `desktop/src-tauri/src/commands/project_git_workflow.rs`, because `clone_project_repository` already calls `build_git_clone_auth_config` after `validate_local_clone_url_for_workspace`.
Do not modify `desktop/src-tauri/src/commands/project_terminal.rs`; its existing clone path also receives the common builder through `build_git_clone_auth_config`, and an existing checkout ignores an auth-builder error before opening the terminal.
Do not modify `desktop/src/testing/e2eBridge.ts`; the required clone URL, GitHub state error, sync status, command log, pull, push, create, and delete mocks already exist.

## Required Impact Checks

Run these before the first edit in each task when GitNexus MCP tools are available.
Report direct callers, affected processes, and the risk level before editing.
If GitNexus is unavailable or this worktree has no `.gitnexus/run.cjs`, record that fact and continue with direct source inspection.

- Task 1: `validate_workspace_clone_url`, `GitHubRepoRef::parse`, `configure_git_auth`, `build_git_clone_auth_config`, `GhRunner::ensure_auth`, `get_project_repo_sync_status`, `pull_project_local_repository`, `push_project_local_repository`, `create_project_remote_branch`, and `delete_project_remote_branch`.
- Task 2: `parseProjectPullRequestMergeError`, `projectCloneErrorPresentation`, and `projectBranchErrorMessage`.
- Task 3: `useProjectRepoSyncStatusQuery`, `usePushProjectLocalRepositoryMutation`, `ProjectDetailScreen`, and `CreatePullRequestDialog`.
- Task 4: `RepoSyncActionButton`, `RepositoryBranchDropdown`, `useProjectRepositorySourceControls`, `useGithubAheadBehindQuery`, and the Playwright smoke `testMatch`.

---

### Task 1: Add the strict URL gate and host-aware git authentication

**Files:**

- Modify: `desktop/src-tauri/src/commands/project_git_exec.rs:1-235,283-373`
- Modify: `desktop/src-tauri/src/commands/project_github_pull_request.rs:276-448`
- Modify: `desktop/src-tauri/src/commands/project_git.rs:865-976`
- Modify: `desktop/src-tauri/src/commands/project_git_branches.rs:1-217`
- Test: `desktop/src-tauri/src/commands/project_git_exec.rs:369-503`
- Regression test: `desktop/src-tauri/src/commands/project_git_branches.rs:218-346`

**Interfaces:**

- Consumes: `GitHubRepoRef::parse`, `validate_clone_url_against_relay`, `GhRunner::{discover, from_resolved, ensure_auth}`, `remap_state_error`, and `ProjectPullRequestMergeError`.
- Produces: `pub(crate) fn validate_git_operation_url(clone_url: &str, state: &AppState) -> Result<(), String>`.
- Produces: private `fn validate_git_operation_url_against_relay(clone_url: &str, relay_base: &str) -> Result<(), String>` for deterministic tests.
- Produces: `pub(crate) fn build_git_operation_auth_config(clone_url: &str, state: &AppState) -> Result<GitAuthConfig, String>`.
- Produces: test seam `pub(crate) fn build_github_git_auth_config_with<F>(clone_url: &str, discover: F) -> Result<GitAuthConfig, String> where F: FnOnce() -> Result<GhRunner, ProjectPullRequestMergeError>`.
- Preserves: the five Tauri command signatures and all blocking git operation signatures.

- [ ] **Step 1: Add failing URL-gate tests**

Add `validate_git_operation_url_against_relay` to the existing test-module import list in `project_git_exec.rs`.
Add these tests next to `workspace_clone_url_requires_exact_relay_origin_and_prefix`.

```rust
#[test]
fn git_operation_url_accepts_strict_github_https_and_ssh() {
    let relay = "https://relay.example/prefix";
    for clone_url in [
        "https://github.com/acme/app",
        "https://github.com/acme/app.git",
        "git@github.com:acme/app.git",
        "ssh://git@github.com/acme/app.git",
    ] {
        assert!(
            validate_git_operation_url_against_relay(clone_url, relay).is_ok(),
            "{clone_url}",
        );
    }
}

#[test]
fn git_operation_url_rejects_other_hosts_and_keeps_buzz_relay_scoped() {
    let owner = "a".repeat(64);
    let relay = "https://relay.example/prefix";
    assert!(
        validate_git_operation_url_against_relay("https://gitlab.com/acme/app", relay).is_err()
    );
    assert!(
        validate_git_operation_url_against_relay(
            "https://github.com/acme/app/issues",
            relay,
        )
        .is_err()
    );
    assert!(
        validate_git_operation_url_against_relay(
            &format!("https://relay.example/prefix/git/{owner}/repo"),
            relay,
        )
        .is_ok()
    );
    assert!(
        validate_git_operation_url_against_relay(
            &format!("https://evil.example/prefix/git/{owner}/repo"),
            relay,
        )
        .is_err()
    );
}
```

- [ ] **Step 2: Add failing auth-builder and secret-isolation tests**

Extend the test-module imports with `build_github_git_auth_config_with`, `configure_git_auth`, `github_command_error_string`, `github_credential_helper_config_value`, `GitAuthConfig`, `GhRunner`, and `ProjectPullRequestMergeError`.
Add the following helpers and tests.
The fake binary deliberately lives under a directory containing a space so the test exercises shell quoting.

```rust
fn error_json_code(error: &str) -> String {
    serde_json::from_str::<serde_json::Value>(error)
        .ok()
        .and_then(|value| value.get("code")?.as_str().map(ToString::to_string))
        .unwrap_or_default()
}

#[test]
fn github_git_auth_quotes_spaces_and_apostrophes_in_helper_path() {
    let path = std::path::Path::new("/tmp/Buzz's GitHub CLI/gh");
    assert_eq!(
        github_credential_helper_config_value(path),
        "!'/tmp/Buzz'\\''s GitHub CLI/gh' auth git-credential",
    );
}

#[cfg(unix)]
fn fake_gh(script: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("create fake gh directory");
    let bin_dir = dir.path().join("GitHub CLI");
    std::fs::create_dir(&bin_dir).expect("create spaced bin directory");
    let path = bin_dir.join("gh");
    std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{script}\n"))
        .expect("write fake gh");
    let mut permissions = std::fs::metadata(&path)
        .expect("stat fake gh")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod fake gh");
    (dir, path)
}

#[cfg(unix)]
fn invoke_credential_helper(auth: &GitAuthConfig) {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut command = Command::new(&auth.git_path);
    command
        .args(["credential", "fill"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env("NOSTR_PRIVATE_KEY", "must-not-leak");
    configure_git_auth(&mut command, auth, true);
    let mut child = command.spawn().expect("run git credential fill");
    let mut stdin = child.stdin.take().expect("credential stdin");
    stdin
        .write_all(b"protocol=https\nhost=github.com\n\n")
        .expect("write credential request");
    drop(stdin);
    let _ = child.wait().expect("wait for git credential fill");
}

#[cfg(unix)]
#[test]
fn github_git_auth_https_invokes_quoted_gh_helper_without_nsec() {
    let script = r#"
printf '%s\n' "$*" >> "${0%/GitHub CLI/gh}/calls"
case "$*" in
  *auth*status*) exit 0 ;;
  *auth*git-credential*)
    if [ "${NOSTR_PRIVATE_KEY+x}" = x ]; then exit 41; fi
    exit 0
    ;;
  *api*) exit 42 ;;
  *) exit 43 ;;
esac
"#;
    let (dir, path) = fake_gh(script);
    let auth = build_github_git_auth_config_with("https://github.com/acme/app", || {
        GhRunner::from_resolved(Some(path.clone()))
    })
    .expect("https GitHub auth");
    let helper = auth.credential_helper.as_deref().expect("credential helper");
    assert!(helper.starts_with("!'"), "{helper}");
    assert!(helper.ends_with("' auth git-credential"), "{helper}");
    assert!(auth.nsec.is_empty());
    invoke_credential_helper(&auth);
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("read calls");
    assert!(calls
        .lines()
        .any(|line| line == "auth status --hostname github.com"));
    assert!(calls
        .lines()
        .any(|line| line == "auth git-credential get"));
    assert!(!calls.lines().any(|line| line.starts_with("api ")));
}

#[test]
fn github_git_auth_ssh_skips_gh_helper_and_discovery() {
    let auth = build_github_git_auth_config_with("git@github.com:acme/app.git", || {
        panic!("SSH GitHub must not discover gh")
    })
    .expect("SSH GitHub auth");
    assert!(auth.credential_helper.is_none());
    assert!(auth.nsec.is_empty());
}

#[test]
fn github_git_auth_missing_cli_is_structured_json() {
    let error = build_github_git_auth_config_with("https://github.com/acme/app", || {
        GhRunner::from_resolved(None)
    })
    .expect_err("missing gh must fail");
    assert_eq!(error_json_code(&error), "github_cli_missing");
}

#[cfg(unix)]
#[test]
fn github_git_auth_failed_status_is_structured_json() {
    let (_dir, path) = fake_gh(
        r#"
case "$*" in
  *auth*status*) exit 1 ;;
  *) exit 43 ;;
esac
"#,
    );
    let error = build_github_git_auth_config_with("https://github.com/acme/app", || {
        GhRunner::from_resolved(Some(path))
    })
    .expect_err("failed auth status must fail");
    assert_eq!(error_json_code(&error), "github_auth_required");
}

#[test]
fn github_git_auth_remaps_merge_failed_before_serializing() {
    let json = github_command_error_string(ProjectPullRequestMergeError::new(
        "github_merge_failed",
        "boom",
    ));
    assert_eq!(error_json_code(&json), "github_state_failed");
}

#[test]
fn buzz_git_auth_adds_nsec_only_for_credential_operations() {
    use std::ffi::OsStr;
    use std::process::Command;
    let auth = GitAuthConfig {
        git_path: std::path::PathBuf::from("git"),
        credential_helper: Some("/tmp/git-credential-nostr".to_string()),
        nsec: "nsec1test".to_string(),
        allow_file_transport: false,
    };
    let mut local = Command::new("git");
    configure_git_auth(&mut local, &auth, false);
    let local_nsec = local
        .get_envs()
        .find(|(key, _)| *key == OsStr::new("NOSTR_PRIVATE_KEY"))
        .and_then(|(_, value)| value);
    assert_eq!(local_nsec, None);
    let mut remote = Command::new("git");
    configure_git_auth(&mut remote, &auth, true);
    let remote_nsec = remote
        .get_envs()
        .find(|(key, _)| *key == OsStr::new("NOSTR_PRIVATE_KEY"))
        .and_then(|(_, value)| value);
    assert_eq!(remote_nsec, Some(OsStr::new("nsec1test")));
}
```

- [ ] **Step 3: Run the new tests and verify the red state**

```bash
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib git_operation_url_
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib github_git_auth_
```

Expected: compilation fails because `validate_git_operation_url_against_relay`, `build_github_git_auth_config_with`, and `github_command_error_string` do not exist yet.

- [ ] **Step 4: Expose the `gh` path and scrub inherited secrets**

In `project_github_pull_request.rs`, add this method inside `impl GhRunner`.

```rust
/// Resolved GitHub CLI binary used by git's credential-helper command.
pub(crate) fn binary_path(&self) -> &std::path::Path {
    &self.binary
}
```

In `GhRunner::run_with_limit`, add `.env_remove("NOSTR_PRIVATE_KEY")` to the `Command` builder before spawning so G1, G3, G4, merge, and credential-auth checks cannot inherit a Nostr secret.

```rust
let mut command = Command::new(&self.binary);
command
    .args(args)
    .env_remove("NOSTR_PRIVATE_KEY")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
```

- [ ] **Step 5: Implement helper values, host-aware auth, and the URL gate**

Update the `project_git_exec.rs` imports to include `ProjectPullRequestMergeError`, `GhRunner`, and `remap_state_error`.
Change `credential_helper` from `Option<PathBuf>` to the complete `Option<String>` config value and derive `Debug` so failure assertions compile.

```rust
#[derive(Debug)]
pub(crate) struct GitAuthConfig {
    git_path: std::path::PathBuf,
    credential_helper: Option<String>,
    nsec: String,
    allow_file_transport: bool,
}
```

At the start of `configure_git_auth`, explicitly remove inherited `NOSTR_PRIVATE_KEY`.
When credentials are needed, set the secret only when `nsec` is non-empty, then append the already-finished helper value unchanged.

```rust
command.env("GIT_TERMINAL_PROMPT", "0");
command.env("GIT_CONFIG_NOSYSTEM", "1");
command.env_remove("NOSTR_PRIVATE_KEY");

if needs_credentials {
    let Some(credential_helper) = &auth.credential_helper else {
        return apply_git_config(command, &entries);
    };
    if !auth.nsec.is_empty() {
        command.env("NOSTR_PRIVATE_KEY", &auth.nsec);
    }
    entries.push(("credential.helper", credential_helper.clone()));
    entries.push(("credential.useHttpPath", "true".to_string()));
}
apply_git_config(command, &entries);
```

Keep `credential_helper_config_value` for slash normalization and add a Git shell command formatter that single-quotes the resolved `gh` path.

```rust
fn github_credential_helper_config_value(path: &std::path::Path) -> String {
    let path = credential_helper_config_value(path);
    format!(
        "!'{}' auth git-credential",
        path.replace('\'', "'\\''"),
    )
}
```

Update `build_git_auth_config_for_keys` so its helper is already a string.

```rust
let credential_helper = resolve_command("git-credential-nostr")
    .map(|path| credential_helper_config_value(&path));
```

Add the structured-error serializer and host-aware builders next to the current builders.

```rust
pub(crate) fn github_command_error_string(
    error: ProjectPullRequestMergeError,
) -> String {
    let remapped = remap_state_error(error, "");
    serde_json::to_string(&remapped).unwrap_or_else(|_| {
        "{\"code\":\"github_state_failed\",\"message\":\"GitHub authentication failed.\",\"recovery\":null}"
            .to_string()
    })
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
    gh.ensure_auth().map_err(github_command_error_string)?;
    Ok(GitAuthConfig {
        git_path,
        credential_helper: Some(github_credential_helper_config_value(
            gh.binary_path(),
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

Add the new URL gate beside `validate_workspace_clone_url` and leave the existing workspace gate unchanged.

```rust
/// Accept a Buzz URL on the active relay or a strict github.com URL.
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

- [ ] **Step 6: Route exactly five Tauri commands through the new policy**

In `project_git.rs`, update the imports and replace the validator and auth builder only in `get_project_repo_sync_status`, `push_project_local_repository`, and `pull_project_local_repository`.

```rust
validate_git_operation_url(&clone_url, &state)?;
let auth = build_git_operation_auth_config(&clone_url, &state)?;
```

In `project_git_branches.rs`, make the same replacement only in `create_project_remote_branch` and `delete_project_remote_branch`.
Leave `get_project_repo_snapshot`, git identity, diff, merge, and recovery commands on `validate_workspace_clone_url` and `build_git_auth_config`.

- [ ] **Step 7: Run Rust formatting and focused regression tests**

```bash
. ./bin/activate-hermit && cargo fmt --manifest-path desktop/src-tauri/Cargo.toml -- --check
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_git_exec::tests
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_git_branches::tests
```

Expected: all tests pass on Unix.
Expected on Windows: non-`cfg(unix)` URL, SSH, missing-CLI, remap, and auth-configuration tests pass; the shell-script invocation tests remain Unix-only.

- [ ] **Step 8: Commit the Rust slice**

```bash
. ./bin/activate-hermit && if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi && git diff --check && git add desktop/src-tauri/src/commands/project_git_exec.rs desktop/src-tauri/src/commands/project_github_pull_request.rs desktop/src-tauri/src/commands/project_git.rs desktop/src-tauri/src/commands/project_git_branches.rs && git commit -s -m "feat(projects): authenticate GitHub git operations"
```

---

### Task 2: Parse and present structured GitHub command errors

**Files:**

- Modify: `desktop/src/shared/api/projectGit.ts:300-470,637-683`
- Modify: `desktop/src/features/projects/lib/projectGitError.ts:1-70`
- Modify: `desktop/src/features/projects/lib/projectGitError.test.mjs:1-49`
- Modify: `desktop/src/features/projects/lib/projectBranchErrors.ts:1-36`
- Modify: `desktop/src/features/projects/lib/projectBranchErrors.test.mjs:1-43`

**Interfaces:**

- Consumes: `parseProjectPullRequestMergeError` and `ProjectPullRequestMergeError`.
- Produces: structured `Error` objects from sync, pull, push, clone, create, and delete TypeScript wrappers while preserving generic failures unchanged.
- Produces: `export function githubBranchActionReason(input: { githubHosted: boolean; error?: unknown }): string | null`.

- [ ] **Step 1: Add failing clone and branch error tests**

Import `ProjectPullRequestMergeError` from `../../../shared/api/projectGit.ts` in both test files.
Add these tests to `projectGitError.test.mjs`.

```js
test("presents structured GitHub CLI and auth failures for clone", () => {
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
  assert.deepEqual(
    projectCloneErrorPresentation(
      new ProjectPullRequestMergeError(
        "github_auth_required",
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        null,
      ),
      "https://github.com/acme/app",
    ),
    {
      title: "GitHub authentication required",
      description:
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
    },
  );
});

test("does not use GitHub recovery copy for a Buzz clone URL", () => {
  const owner = "ab".repeat(32);
  assert.deepEqual(
    projectCloneErrorPresentation(
      new ProjectPullRequestMergeError(
        "github_auth_required",
        "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        null,
      ),
      `https://relay.example/git/${owner}/app`,
    ),
    {
      title: "Repository access required",
      description:
        "Buzz could not authenticate with this repository. Check your access and try again.",
    },
  );
});
```

Add these tests to `projectBranchErrors.test.mjs` and add `githubBranchActionReason` to its import list.

```js
test("maps structured GitHub errors in branch dialogs", () => {
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

test("derives a GitHub branch-action recovery reason and gates it by host", () => {
  const error = new ProjectPullRequestMergeError(
    "github_cli_missing",
    "Install the GitHub CLI to continue.",
    null,
  );
  assert.equal(
    githubBranchActionReason({ githubHosted: true, error }),
    "Install GitHub CLI, then retry.",
  );
  assert.equal(
    githubBranchActionReason({ githubHosted: false, error }),
    null,
  );
});
```

- [ ] **Step 2: Run the focused tests and verify the red state**

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectGitError.test.mjs src/features/projects/lib/projectBranchErrors.test.mjs
```

Expected: the new structured recovery assertions fail and `githubBranchActionReason` is not exported.

- [ ] **Step 3: Centralize structured parsing at the affected API boundary**

Add this private helper in `projectGit.ts` after `parseProjectPullRequestMergeError`.

```ts
async function invokeProjectGitCommand<T>(
  command: string,
  args: Record<string, unknown>,
): Promise<T> {
  try {
    return await invokeTauri<T>(command, args);
  } catch (error) {
    throw parseProjectPullRequestMergeError(error) ?? error;
  }
}
```

Replace `invokeTauri` with `invokeProjectGitCommand` in exactly these wrappers while keeping their existing raw-to-camel-case mapping unchanged.

```ts
getProjectRepoSyncStatus       -> "get_project_repo_sync_status"
pushProjectLocalRepository     -> "push_project_local_repository"
pullProjectLocalRepository     -> "pull_project_local_repository"
cloneProjectRepository        -> "clone_project_repository"
createProjectRemoteBranch     -> "create_project_remote_branch"
deleteProjectRemoteBranch     -> "delete_project_remote_branch"
```

Do not use the helper for unrelated commands in this slice.
Because `parseProjectPullRequestMergeError` returns `null` for plain git stderr, generic git failures retain their existing type and text.

- [ ] **Step 4: Add structured clone and branch presentations**

Import `parseProjectPullRequestMergeError` into `projectGitError.ts`.
At the start of `projectCloneErrorPresentation`, after computing `github`, add this host-gated branch before generic stderr matching.

```ts
const structured = github ? parseProjectPullRequestMergeError(error) : null;
if (structured) {
  switch (structured.code) {
    case "github_cli_missing":
      return {
        title: "GitHub CLI is required",
        description: "Install GitHub CLI, then retry.",
      };
    case "github_auth_required":
      return {
        title: "GitHub authentication required",
        description: structured.message,
      };
    case "github_repo_unavailable":
    case "github_state_failed":
      return {
        title: "Could not load GitHub branches",
        description: structured.message,
      };
  }
}
```

Import `parseProjectPullRequestMergeError` into `projectBranchErrors.ts`.
Parse structured errors before the existing `instanceof Error` and no-channel-binding paths.
Add the branch-action reason beside `projectBranchErrorMessage`.

```ts
export function githubBranchActionReason(input: {
  githubHosted: boolean;
  error?: unknown;
}): string | null {
  if (!input.githubHosted || input.error == null) return null;
  const parsed = parseProjectPullRequestMergeError(input.error);
  if (!parsed) return null;
  return parsed.code === "github_cli_missing"
    ? "Install GitHub CLI, then retry."
    : parsed.message;
}

export function projectBranchErrorMessage(
  error: unknown,
  fallback: string,
): string {
  const structured = parseProjectPullRequestMergeError(error);
  if (structured) return structured.message;
  if (!(error instanceof Error)) return fallback;
  if (isNoChannelBindingError(error.message)) {
    return NO_CHANNEL_BINDING_COPY;
  }
  return error.message;
}
```

- [ ] **Step 5: Run focused tests, formatting, and typecheck**

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectGitError.test.mjs src/features/projects/lib/projectBranchErrors.test.mjs src/shared/api/projectGitMergeError.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm exec biome check src/shared/api/projectGit.ts src/features/projects/lib/projectGitError.ts src/features/projects/lib/projectGitError.test.mjs src/features/projects/lib/projectBranchErrors.ts src/features/projects/lib/projectBranchErrors.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm typecheck
```

Expected: all commands pass.

- [ ] **Step 6: Commit the structured error slice**

```bash
. ./bin/activate-hermit && if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi && git diff --check && git add desktop/src/shared/api/projectGit.ts desktop/src/features/projects/lib/projectGitError.ts desktop/src/features/projects/lib/projectGitError.test.mjs desktop/src/features/projects/lib/projectBranchErrors.ts desktop/src/features/projects/lib/projectBranchErrors.test.mjs && git commit -s -m "feat(projects): present GitHub git authentication failures"
```

---

### Task 3: Enable GitHub sync after G1 and suppress the Nostr push side effect

**Files:**

- Create: `desktop/src/features/projects/lib/projectGithubSync.ts`
- Create: `desktop/src/features/projects/lib/projectGithubSync.test.mjs`
- Modify: `desktop/src/features/projects/repoSyncHooks.ts:1-155`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx:300-315`
- Modify: `desktop/src/features/projects/ui/CreatePullRequestDialog.tsx:60-105`

**Interfaces:**

- Consumes: `isGitHubCloneUrl`, `useProjectRepoHost`, and `repoStateQuery.isSuccess`.
- Produces: `export function projectRepoSyncStatusEnabled(input: { cloneUrl?: string | null; buzzHost: boolean; githubStateReady: boolean }): boolean`.
- Produces: `export function githubSyncCountDisplay(input: { githubHosted: boolean; localPath?: string | null; aheadCount?: number | null; behindCount?: number | null }): { ahead: number; behind: number } | null`.
- Produces: `export function shouldPublishPullRequestUpdateAfterPush(cloneUrl?: string | null): boolean`.
- Produces: `export function repoSyncPrimaryAction(input: { githubHosted: boolean; remoteKind?: "buzz" | "external"; hasExternalUrl?: boolean; canPull?: boolean; canPush?: boolean; hasFetch?: boolean }): "pull" | "push" | "fetch" | "open" | null`.
- Extends: `useProjectRepoSyncStatusQuery(..., options?: { githubStateReady?: boolean })`.

- [ ] **Step 1: Create failing pure policy tests**

Create `projectGithubSync.test.mjs` with the following content.

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  githubSyncCountDisplay,
  projectRepoSyncStatusEnabled,
  repoSyncPrimaryAction,
  shouldPublishPullRequestUpdateAfterPush,
} from "./projectGithubSync.ts";

test("GitHub sync waits for G1 while Buzz sync stays enabled", () => {
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

test("GitHub counts require a checkout and numeric sync data", () => {
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
      localPath: "/tmp/acme-app",
      aheadCount: 1,
      behindCount: 0,
    }),
    { ahead: 1, behind: 0 },
  );
  assert.equal(
    githubSyncCountDisplay({
      githubHosted: true,
      localPath: "/tmp/acme-app",
      aheadCount: null,
      behindCount: 0,
    }),
    null,
  );
});

test("GitHub push skips a Nostr pull-request update", () => {
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

test("GitHub uses Pull Push Fetch while other external hosts use Open", () => {
  assert.equal(
    repoSyncPrimaryAction({
      githubHosted: true,
      remoteKind: "external",
      hasExternalUrl: true,
      canPull: true,
      hasFetch: true,
    }),
    "pull",
  );
  assert.equal(
    repoSyncPrimaryAction({
      githubHosted: true,
      remoteKind: "external",
      hasExternalUrl: true,
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
```

- [ ] **Step 2: Run the new test and verify the red state**

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectGithubSync.test.mjs
```

Expected: the test fails because `projectGithubSync.ts` does not exist.

- [ ] **Step 3: Implement the pure sync policies**

Create `projectGithubSync.ts` with this content.

```ts
import { isGitHubCloneUrl } from "@/features/projects/lib/projectGitError";

/** Decide whether local-vs-remote sync status may run for this repository. */
export function projectRepoSyncStatusEnabled(input: {
  cloneUrl?: string | null;
  buzzHost: boolean;
  githubStateReady: boolean;
}): boolean {
  if (!input.cloneUrl) return false;
  if (input.buzzHost) return true;
  return isGitHubCloneUrl(input.cloneUrl) && input.githubStateReady;
}

/** Return displayable GitHub counts only when sync examined a checkout. */
export function githubSyncCountDisplay(input: {
  githubHosted: boolean;
  localPath?: string | null;
  aheadCount?: number | null;
  behindCount?: number | null;
}): { ahead: number; behind: number } | null {
  if (!input.githubHosted || !input.localPath) return null;
  if (
    typeof input.aheadCount !== "number" ||
    typeof input.behindCount !== "number"
  ) {
    return null;
  }
  return { ahead: input.aheadCount, behind: input.behindCount };
}

/** Keep Nostr PR-update publication on Buzz pushes only. */
export function shouldPublishPullRequestUpdateAfterPush(
  cloneUrl?: string | null,
): boolean {
  return !isGitHubCloneUrl(cloneUrl);
}

/** Select the single primary repository sync action for the header. */
export function repoSyncPrimaryAction(input: {
  githubHosted: boolean;
  remoteKind?: "buzz" | "external";
  hasExternalUrl?: boolean;
  canPull?: boolean;
  canPush?: boolean;
  hasFetch?: boolean;
}): "pull" | "push" | "fetch" | "open" | null {
  if (!input.githubHosted && input.remoteKind === "external") {
    return input.hasExternalUrl ? "open" : null;
  }
  if (input.canPull) return "pull";
  if (input.canPush) return "push";
  return input.hasFetch ? "fetch" : null;
}
```

- [ ] **Step 4: Enable the query after G1 and guard the push side effect**

Import `projectRepoSyncStatusEnabled` and `shouldPublishPullRequestUpdateAfterPush` into `repoSyncHooks.ts`.
Keep the `isGitHubCloneUrl` import because the retained `useGithubAheadBehindQuery` implementation still uses it.
Replace the complete `useProjectRepoSyncStatusQuery` function with this implementation.

```ts
export function useProjectRepoSyncStatusQuery(
  project: Project | null | undefined,
  reposDir?: string | null,
  branchName?: string | null,
  baseBranch?: string | null,
  options?: { githubStateReady?: boolean },
) {
  const selectedBranch = branchName ?? project?.defaultBranch ?? null;
  const refetchInterval = useFocusedRefetchInterval(60_000);
  const selectedBaseBranch = baseBranch ?? project?.defaultBranch ?? null;
  const host = useProjectRepoHost(project);

  return useQuery({
    enabled: projectRepoSyncStatusEnabled({
      cloneUrl: project?.cloneUrls[0],
      buzzHost: host.kind === "buzz",
      githubStateReady: options?.githubStateReady ?? false,
    }),
    queryKey: [
      "project",
      project?.id ?? "none",
      "repo-sync-status",
      reposDir ?? "default",
      selectedBranch ?? "default",
      selectedBaseBranch ?? "default",
    ],
    queryFn: () => {
      if (!project?.cloneUrls[0]) throw new Error("No project selected.");
      return getProjectRepoSyncStatus({
        reposDir,
        projectDtag: project.dtag,
        cloneUrl: project.cloneUrls[0],
        branchName: selectedBranch,
        baseBranch: selectedBaseBranch,
      });
    },
    staleTime: 10_000,
    refetchInterval,
    refetchOnWindowFocus: false,
    retry: 1,
  });
}
```

In `usePushProjectLocalRepositoryMutation`, add the host guard to the existing pull-request update condition.

```ts
if (
  shouldPublishPullRequestUpdateAfterPush(project.cloneUrls[0]) &&
  pullRequest &&
  (pullRequest.status === "Open" || pullRequest.status === "Draft")
) {
  try {
    const updated = await publishProjectPullRequestUpdate({
      commit: result.commit,
      mergeBase: result.mergeBase,
      project,
      pullRequest,
    });
    pullRequestUpdate = {
      status: updated ? "updated" : "unchanged",
    };
  } catch (error) {
    pullRequestUpdate = {
      status: "failed",
      error:
        error instanceof Error
          ? error.message
          : "The pull request update could not be published.",
    };
  }
}
```

Replace the current condition and its try/catch with this single block; do not add a second publication path.

- [ ] **Step 5: Pass G1 readiness at every sync-query call site**

In `ProjectDetailScreen.tsx`, pass the options object as the fifth argument.

```ts
const repoSyncStatusQuery = useProjectRepoSyncStatusQuery(
  repository,
  activeCommunity?.reposDir,
  activeBranch,
  undefined,
  { githubStateReady: repoStateQuery.isSuccess },
);
```

In `CreatePullRequestDialog.tsx`, pass the same readiness option to both calls.

```ts
const initialSyncQuery = useProjectRepoSyncStatusQuery(
  repository,
  reposDir,
  defaultBranch || null,
  undefined,
  { githubStateReady: repoStateQuery.isSuccess },
);

const sourceSyncQuery = useProjectRepoSyncStatusQuery(
  repository,
  reposDir,
  sourceBranch || null,
  targetBranch || null,
  { githubStateReady: repoStateQuery.isSuccess },
);
```

- [ ] **Step 6: Run policy tests, formatting, and typecheck**

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectGithubSync.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm exec biome check src/features/projects/lib/projectGithubSync.ts src/features/projects/lib/projectGithubSync.test.mjs src/features/projects/repoSyncHooks.ts src/features/projects/ui/ProjectDetailScreen.tsx src/features/projects/ui/CreatePullRequestDialog.tsx
. ./bin/activate-hermit && cd desktop && pnpm typecheck
```

Expected: all commands pass.

- [ ] **Step 7: Commit the query and push-policy slice**

```bash
. ./bin/activate-hermit && if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi && git diff --check && git add desktop/src/features/projects/lib/projectGithubSync.ts desktop/src/features/projects/lib/projectGithubSync.test.mjs desktop/src/features/projects/repoSyncHooks.ts desktop/src/features/projects/ui/ProjectDetailScreen.tsx desktop/src/features/projects/ui/CreatePullRequestDialog.tsx && git commit -s -m "feat(projects): enable GitHub repository sync status"
```

---

### Task 4: Render GitHub Pull, Push, Fetch, Create, Delete, and recovery states

**Files:**

- Modify: `desktop/src/features/projects/ui/ProjectRepositorySource.tsx:1-405`
- Modify: `desktop/src/features/projects/ui/useProjectRepositorySourceControls.ts:1-226`
- Modify: `desktop/tests/e2e/github-snapshot-and-fetch.spec.ts:1-134`
- Create: `desktop/tests/e2e/github-pull-push-and-branches.spec.ts`
- Modify: `desktop/playwright.config.ts:110-120`
- Test: `desktop/src/features/projects/lib/projectGithubSync.test.mjs`
- Test: `desktop/src/features/projects/lib/projectBranchErrors.test.mjs`

**Interfaces:**

- Consumes: `repoSyncPrimaryAction`, `githubSyncCountDisplay`, `githubBranchActionReason`, and `parseProjectPullRequestMergeError`.
- Produces: one GitHub primary action, checkout-only counts, no project-page G4 request, and visible disabled branch actions on G1 recovery.
- Reuses: `RepoSourceHeaderControls.githubHosted`, `__BUZZ_E2E_PROJECT_CLONE_URL_OVERRIDE__`, `__BUZZ_E2E_PROJECT_REPO_SYNC_STATUS__`, `__BUZZ_E2E_GITHUB_REPO_STATE_ERROR__`, and `__BUZZ_E2E_COMMANDS__`.

- [ ] **Step 1: Update the existing Fetch smoke before production code**

In the first test of `github-snapshot-and-fetch.spec.ts`, replace the hidden Create/Delete assertions with visible branch-menu assertions.
Clear the command log before clicking Fetch so the assertion measures the manual action rather than initial query startup.

```ts
await expect
  .poll(() => page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []))
  .toContain("get_project_repo_sync_status");
await page.evaluate(() => {
  window.__BUZZ_E2E_COMMANDS__ = [];
});
await header.getByRole("button", { name: /^Fetch$/ }).click();
await expect
  .poll(() => page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []))
  .toEqual(
    expect.arrayContaining([
      "get_github_repository_state",
      "get_github_repository_snapshot",
    ]),
  );
const noCheckoutCommands = await page.evaluate(
  () => window.__BUZZ_E2E_COMMANDS__ ?? [],
);
expect(noCheckoutCommands).not.toContain("get_project_repo_sync_status");
expect(noCheckoutCommands).not.toContain("get_github_ahead_behind");
await header.getByRole("button").filter({ hasText: "develop" }).click();
await expect(page.getByTestId("project-create-branch")).toBeEnabled();
await expect(page.getByTestId("project-delete-branch")).toBeVisible();
await expect(page.getByTestId("project-delete-branch")).toBeDisabled();
```

In the checkout test, change the command assertions after Fetch to require sync status and forbid G4.

```ts
await expect
  .poll(() => page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []))
  .toEqual(
    expect.arrayContaining([
      "get_github_repository_state",
      "get_github_repository_snapshot",
      "get_project_repo_sync_status",
    ]),
  );
const checkoutCommands = await page.evaluate(
  () => window.__BUZZ_E2E_COMMANDS__ ?? [],
);
expect(checkoutCommands).not.toContain("get_github_ahead_behind");
```

- [ ] **Step 2: Create the failing Pull, Push, branch, and recovery smoke spec**

Create `github-pull-push-and-branches.spec.ts` with these imports and helpers, then append the tests below.

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
```

Use these three tests.

```ts
test("GitHub checkout behind shows Pull and invokes the existing command", async ({
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
      push_block_reason: "Local branch is not ahead.",
      can_pull: true,
      pull_block_reason: null,
    };
  });
  await installMockBridge(page);
  await openBuzzProject(page);
  const header = page.getByTestId("project-repository-selection-row");
  await expect(header.getByRole("button", { name: /^Pull 1$/ })).toBeVisible({
    timeout: 10_000,
  });
  await expect(header.getByRole("link", { name: /^Open$/ })).toHaveCount(0);
  await page.evaluate(() => {
    window.__BUZZ_E2E_COMMANDS__ = [];
  });
  await header.getByRole("button", { name: /^Pull 1$/ }).click();
  await expect
    .poll(() => page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []))
    .toContain("pull_project_local_repository");
});

test("GitHub checkout ahead shows Push and invokes the existing command", async ({
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
      remote_head: "c".repeat(40),
      remote_short_head: "ccccccc",
      merge_base: "c".repeat(40),
      ahead_count: 1,
      behind_count: 0,
      has_uncommitted_changes: false,
      has_untracked_files: false,
      can_push: true,
      push_block_reason: null,
      can_pull: false,
      pull_block_reason: "Local branch is not behind.",
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
});

test("G1 recovery keeps Create and Delete visible but disabled", async ({
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
  const header = page.getByTestId("project-repository-selection-row");
  await expect(header.getByRole("button", { name: /Pull/ })).toHaveCount(0);
  await expect(header.getByRole("button", { name: /Push/ })).toHaveCount(0);
  await header.getByTestId("project-branch-picker").click();
  await expect(page.getByTestId("project-create-branch")).toBeVisible();
  await expect(page.getByTestId("project-create-branch")).toBeDisabled();
  await expect(page.getByTestId("project-delete-branch")).toBeVisible();
  await expect(page.getByTestId("project-delete-branch")).toBeDisabled();
  await expect(
    page.getByRole("menu").getByText(
      "Authenticate GitHub CLI with: gh auth login --hostname github.com",
      { exact: true },
    ),
  ).toBeVisible();
  await expect(page.getByText("create the first commit", { exact: false })).toHaveCount(0);
  const commands = await page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []);
  expect(commands).toContain("get_github_repository_state");
  expect(commands).not.toContain("get_project_repo_sync_status");
  expect(commands).not.toContain("create_project_remote_branch");
  expect(commands).not.toContain("delete_project_remote_branch");
  expect(commands).not.toContain("get_github_ahead_behind");
});
```

Add this fourth test to the same file.

```ts
test("GitHub without a checkout creates and deletes through existing commands", async ({
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
  await header.getByTestId("project-branch-picker").click();
  await expect(page.getByTestId("project-create-branch")).toBeEnabled();
  await page.getByTestId("project-create-branch").click();
  await page.getByTestId("project-create-branch-name").fill("feature/demo");
  await page.evaluate(() => {
    window.__BUZZ_E2E_COMMANDS__ = [];
  });
  await page.getByTestId("project-create-branch-submit").click();
  await expect
    .poll(() => page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []))
    .toContain("create_project_remote_branch");
  await expect(header.getByTestId("project-branch-picker")).toContainText(
    "feature/demo",
  );
  await header.getByTestId("project-branch-picker").click();
  await expect(page.getByTestId("project-delete-branch")).toBeEnabled();
  await page.getByTestId("project-delete-branch").click();
  await page.getByTestId("project-delete-branch-submit").click();
  await expect
    .poll(() => page.evaluate(() => window.__BUZZ_E2E_COMMANDS__ ?? []))
    .toContain("delete_project_remote_branch");
});
```

Register `**/github-pull-push-and-branches.spec.ts` immediately after `**/github-snapshot-and-fetch.spec.ts` in the smoke `testMatch` list.

- [ ] **Step 3: Run the new and changed smoke specs and verify the red state**

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-pull-push-and-branches.spec.ts github-snapshot-and-fetch.spec.ts
```

Expected: Pull/Push stay Fetch, Create/Delete are absent on GitHub, the failed-G1 branch picker is not interactive, and checkout Fetch still calls G4.

- [ ] **Step 4: Make the empty branch picker interactive when actions exist**

In `RepositoryBranchDropdown`, compute an empty-safe branch list and only return the non-interactive dash when no actions can be shown.

```tsx
const selectableBranches =
  branchOptions.length > 0 ? branchOptions : branch ? [branch] : [];
const hasBranchActions = Boolean(onCreateBranch || onDeleteBranch);
const selectedValue = selectedTag ? `tag:${selectedTag}` : `branch:${branch}`;
const RefIcon = selectedTag ? Tag : GitBranch;
if (!branch && !hasBranchActions) {
  return (
    <span className="truncate font-mono text-sm font-semibold text-foreground">
      —
    </span>
  );
}
```

Add `data-testid="project-branch-picker"` to the dropdown trigger button.
Render its text as `{selectedTag || branch || "—"}`.
Keep the existing radio group; an empty `selectableBranches` array renders no fake empty branch.
Change the delete label to `{branch ? `Delete ${branch}` : "Delete branch"}` so recovery never renders a blank label.

- [ ] **Step 5: Replace the primary action routing without placeholders**

Import `type ReactNode` from `react` and import `repoSyncPrimaryAction` from `projectGithubSync.ts` into `ProjectRepositorySource.tsx`.
Use these exact imports.

```ts
import type { ReactNode } from "react";

import { repoSyncPrimaryAction } from "@/features/projects/lib/projectGithubSync";
```

Replace `RepoSyncActionButton` with this complete implementation.

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
    return controls.externalUrl ? (
      <Button
        asChild
        className={PROJECT_PANEL_ACTION_BUTTON_CLASS}
        size="sm"
        title={`Open repository on ${controls.remoteLabel}`}
        variant="ghost"
      >
        <a href={controls.externalUrl} rel="noreferrer" target="_blank">
          <ExternalLink className="h-4 w-4" />
          Open
        </a>
      </Button>
    ) : null;
  }

  let button: ReactNode;
  if (action === "pull" && controls.onPull) {
    const count = controls.behindCount ?? 0;
    button = (
      <Button
        className={PROJECT_PANEL_ACTION_BUTTON_CLASS}
        disabled={controls.pullDisabled}
        onClick={controls.onPull}
        size="sm"
        title={controls.pullTitle ?? "Pull remote commits"}
        variant="ghost"
      >
        {controls.pullPending ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <DownloadCloud className="h-4 w-4" />
        )}
        Pull{count > 0 ? ` ${count}` : ""}
      </Button>
    );
  } else if (action === "push" && controls.onPush) {
    button = (
      <Button
        className={PROJECT_PANEL_ACTION_BUTTON_CLASS}
        disabled={controls.pushDisabled}
        onClick={controls.onPush}
        size="sm"
        title={controls.pushTitle ?? "Push local commits"}
        variant="ghost"
      >
        {controls.pushPending ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <UploadCloud className="h-4 w-4" />
        )}
        Push
      </Button>
    );
  } else if (action === "fetch" && controls.onFetch) {
    button = (
      <Button
        className={PROJECT_PANEL_ACTION_BUTTON_CLASS}
        disabled={controls.fetchPending}
        onClick={controls.onFetch}
        size="sm"
        title={controls.fetchTitle ?? "Check for remote changes"}
        variant="ghost"
      >
        {controls.fetchPending ? (
          <Loader2 className="h-4 w-4 animate-spin" />
        ) : (
          <RefreshCw className="h-4 w-4" />
        )}
        Fetch
      </Button>
    );
  } else {
    return null;
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
  return counts ? (
    <div className="flex items-center gap-2">
      {counts}
      {button}
    </div>
  ) : (
    button
  );
}
```

- [ ] **Step 6: Wire controls, remove G4, and preserve recovery precedence**

In `useProjectRepositorySourceControls.ts`, remove the `useGithubAheadBehindQuery` and `githubAheadBehindCounts` imports, the hook call, its SHA-only variables, and every dependency on that query.
Import `githubBranchActionReason`, `githubSyncCountDisplay`, and `parseProjectPullRequestMergeError`.
After destructuring input, derive checkout state and structured sync recovery.

```ts
const hasCheckout = Boolean(
  repoSyncStatusQuery.data?.localPath || localRepoSnapshotQuery.data,
);
const githubSyncStateError =
  githubHosted &&
  parseProjectPullRequestMergeError(repoSyncStatusQuery.error)
    ? repoSyncStatusQuery.error
    : null;
const githubStateError = repoStateQuery.error ?? githubSyncStateError;
const githubBranchReason = githubBranchActionReason({
  githubHosted,
  error: githubStateError,
});
const createBranchReason =
  githubBranchReason ??
  projectBranchCreationReason({
    activeBranch,
    activeBranchCommit,
    localHead: repoSyncStatusQuery.data?.localHead,
  });
const githubCounts = githubSyncCountDisplay({
  githubHosted,
  localPath: repoSyncStatusQuery.data?.localPath,
  aheadCount: repoSyncStatusQuery.data?.aheadCount,
  behindCount: repoSyncStatusQuery.data?.behindCount,
});
```

Replace `handleFetchRepo` with this complete callback.

```ts
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
}, [
  githubHosted,
  hasCheckout,
  repoSnapshotQuery.refetch,
  repoStateQuery.isError,
  repoStateQuery.refetch,
  repoSyncStatusQuery.refetch,
]);
```

Return the following branch-action fields.

```ts
onCreateBranch: () => branchActions.setCreateOpen(true),
createBranchDisabled:
  branchActions.createPending || Boolean(createBranchReason),
createBranchTitle: createBranchReason ?? "Create a remote branch",
onDeleteBranch: () => branchActions.setDeleteOpen(true),
deleteBranchDisabled:
  branchActions.deletePending ||
  Boolean(githubBranchReason) ||
  Boolean(deleteBranchReason),
deleteBranchTitle:
  githubBranchReason ?? deleteBranchReason ?? "Delete this remote branch",
```

Return Pull, Push, counts, and Fetch fields using these expressions.

```ts
canPush: !selectedTag && (repoSyncStatusQuery.data?.canPush ?? false),
onPush: selectedTag
  ? undefined
  : () => {
      void input.onPush();
    },
canPull: !selectedTag && (repoSyncStatusQuery.data?.canPull ?? false),
onPull: selectedTag
  ? undefined
  : () => {
      void input.onPull();
    },
aheadCount: githubHosted
  ? (githubCounts?.ahead ?? null)
  : (repoSyncStatusQuery.data?.aheadCount ?? null),
behindCount: githubHosted
  ? (githubCounts?.behind ?? null)
  : (repoSyncStatusQuery.data?.behindCount ?? null),
fetchPending:
  repoSnapshotQuery.isFetching ||
  repoStateQuery.isFetching ||
  ((!githubHosted || hasCheckout) && repoSyncStatusQuery.isFetching),
fetchTitle: hasCheckout
  ? "Check for remote changes"
  : githubHosted
    ? "Refresh GitHub README and files"
    : "Check for remote changes",
```

Preserve existing push/pull disabled, pending, and title fields.
Update recovery so a structured sync-auth failure receives the same header component and retry behavior.

```ts
showGithubStateRecovery:
  githubHosted &&
  (repoStateQuery.isError ||
    githubSyncStateError != null ||
    (repoStateQuery.isSuccess && repoSnapshotQuery.isError)),
stateError:
  repoStateQuery.error ??
  githubSyncStateError ??
  (githubHosted ? repoSnapshotQuery.error : undefined),
onRetryState: () => {
  if (githubHosted && repoStateQuery.isError) {
    void repoStateQuery.refetch();
    return;
  }
  if (githubHosted && githubSyncStateError != null) {
    void repoSyncStatusQuery.refetch();
    return;
  }
  if (githubHosted) {
    void repoSnapshotQuery.refetch();
    return;
  }
  void repoStateQuery.refetch();
},
```

- [ ] **Step 7: Run unit tests, typecheck, and the three GitHub smoke specs**

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectGithubSync.test.mjs src/features/projects/lib/projectBranchErrors.test.mjs src/features/projects/lib/projectGitError.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm typecheck
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-pull-push-and-branches.spec.ts github-snapshot-and-fetch.spec.ts github-repo-state.spec.ts
```

Expected: all commands pass.
The E2E command uses `pnpm build:e2e` through the script and must not be replaced with `pnpm run build`.

- [ ] **Step 8: Commit the user-visible slice**

```bash
. ./bin/activate-hermit && if [ -f .gitnexus/run.cjs ]; then node .gitnexus/run.cjs detect; fi && git diff --check && git add desktop/src/features/projects/ui/ProjectRepositorySource.tsx desktop/src/features/projects/ui/useProjectRepositorySourceControls.ts desktop/tests/e2e/github-snapshot-and-fetch.spec.ts desktop/tests/e2e/github-pull-push-and-branches.spec.ts desktop/playwright.config.ts && git commit -s -m "feat(projects): show GitHub pull push and branch actions"
```

---

## Spec Coverage

| Requirement | Evidence |
|-------------|----------|
| Strict GitHub HTTPS/SSH accepted; GitLab and malformed GitHub paths rejected | Task 1 URL tests |
| Buzz URLs remain active-relay scoped | Task 1 URL tests and unchanged workspace validator |
| One host-aware builder serves clone, sync, pull, push, create, and delete | Task 1 builder plus five wrapper edits; existing clone callers already use `build_git_clone_auth_config` |
| HTTPS uses `gh auth git-credential` and handles spaces safely | Task 1 fake binary under `GitHub CLI/` and real `git credential fill` invocation |
| SSH does not discover or authenticate `gh` | Task 1 panic-on-discovery test |
| No GitHub process receives `NOSTR_PRIVATE_KEY` | Task 1 git-command sentinel test plus explicit removal in `configure_git_auth` and `GhRunner::run_with_limit` |
| Buzz credential operations still receive the nsec | Task 1 command-environment test |
| Missing CLI/auth are structured and `github_merge_failed` is remapped | Task 1 JSON-code tests |
| Generic git stderr remains generic | Task 2 boundary helper falls back to the original error |
| Sync starts for GitHub only after G1 | Task 3 pure policy test and all call-site options |
| G1 failure starts no sync/create/delete | Task 3 enable gate and Task 4 recovery smoke command log |
| Push skips Nostr PR update for GitHub | Task 3 pure policy test and hook guard |
| GitHub checkout behind/ahead/neutral maps to Pull/Push/Fetch | Task 3 pure action test and Task 4 smoke coverage |
| Counts appear only for a checkout | Task 3 count test and existing `0 / 0` checkout smoke |
| Fetch with checkout runs sync + G1 + G3 and never G4 | Task 4 updated checkout Fetch smoke |
| Fetch without checkout runs G1 + G3 and not sync/G4 | Task 4 updated no-checkout Fetch smoke |
| Other external hosts stay Open-as-primary | Task 3 pure action test |
| Open on GitHub remains in the source menu | Existing smoke assertion retained |
| Create/Delete remain visible and disabled with G1 recovery copy | Task 2 reason test and Task 4 failed-G1 smoke |
| Create and delete invoke existing commands on GitHub | Task 4 no-checkout branch smoke |
| Default branch remains undeletable | Existing Rust round-trip test and updated `develop` menu assertion |
| Remote create/delete leases remain intact | Unchanged blocking functions and existing Rust round-trip test |
| G4 implementation remains in the tree | Task 4 removes only imports and hook call from project-page controls |

## Acceptance Criteria

- A strict `https://github.com/owner/repo[.git]`, `git@github.com:owner/repo.git`, or `ssh://git@github.com/owner/repo[.git]` clone URL passes the operation gate.
- GitLab, GitHub issue paths, credential-bearing GitHub URLs, ports, query strings, fragments, and non-active Buzz relay URLs remain rejected by existing strict parsers.
- GitHub HTTPS runs `gh auth status --hostname github.com` before creating a config containing a safely quoted `!<resolved-gh> auth git-credential` helper.
- GitHub SSH creates no credential helper, carries no nsec, and does not call `GhRunner::discover` or `ensure_auth` for git operations.
- Buzz operations continue to clear inherited helpers, install `git-credential-nostr`, and expose `NOSTR_PRIVATE_KEY` only to credential-requiring git subprocesses.
- No `gh` subprocess or GitHub git subprocess inherits `NOSTR_PRIVATE_KEY`.
- Missing `gh` yields `github_cli_missing`; unauthenticated `gh` yields `github_auth_required`; runner failures on this path yield `github_state_failed` unless repository classification produces `github_repo_unavailable`.
- A GitHub checkout behind its selected remote branch shows Pull and invokes `pull_project_local_repository`, whose existing implementation uses `git pull --ff-only`.
- A GitHub checkout ahead shows Push and invokes `push_project_local_repository` without publishing kind 1617/PR-update state through `publishProjectPullRequestUpdate`.
- A GitHub checkout with neither action available shows Fetch and may show `0 / 0` only when a local path is present.
- Fetch with a checkout refetches `get_project_repo_sync_status`, `get_github_repository_state`, and `get_github_repository_snapshot`, and never calls `get_github_ahead_behind`.
- Fetch without a checkout refetches G1 and G3 only and never calls sync status or G4 during that click.
- Create from a known G1 branch commit invokes `create_project_remote_branch`, selects the new branch, and Delete invokes `delete_project_remote_branch` with the observed commit.
- Delete remains disabled on GitHub's default `develop` branch and the native default-branch guard remains unchanged.
- When G1 fails, the header shows one GitHub recovery component, the branch picker is still reachable, Create and Delete are visible and disabled with the same recovery reason, and no “create the first commit” copy appears.
- Clone, sync, pull, push, create, and delete surface structured GitHub authentication errors as readable recovery or toast/dialog copy rather than raw JSON.
- Other external hosts retain Open as the primary action, and Buzz-hosted projects retain existing clone, sync, pull, push, create, and delete behavior.

## Final Validation Commands

Run every command from the repository root.

```bash
. ./bin/activate-hermit && cargo fmt --manifest-path desktop/src-tauri/Cargo.toml -- --check
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_git_exec::tests
. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml --lib project_git_branches::tests
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectGithubSync.test.mjs src/features/projects/lib/projectGitError.test.mjs src/features/projects/lib/projectBranchErrors.test.mjs src/shared/api/projectGitMergeError.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm check
. ./bin/activate-hermit && cd desktop && pnpm typecheck
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- github-pull-push-and-branches.spec.ts github-snapshot-and-fetch.spec.ts github-repo-state.spec.ts
. ./bin/activate-hermit && just ci
```

Expected: every command passes.
If unrelated pre-existing failures occur in the repository-wide unit or `just ci` run, record the exact failing test and show that all focused commands above pass; do not silently waive a failure introduced by this slice.

## Implementation Order

1. Complete and commit Task 1 so every later UI path has a safe native operation and error contract.
2. Complete and commit Task 2 so all later queries and handlers receive readable structured errors.
3. Complete and commit Task 3 so GitHub sync is gated by G1 and push side effects are host-correct before controls are exposed.
4. Complete and commit Task 4 so the user-visible controls are implemented only after their mock-bridge tests are red.
5. Run Final Validation Commands, run GitNexus `detect_changes({scope: "compare", base_ref: "main"})` when available, and verify only the files in this plan and their expected execution flows changed.

## Open Questions

None.
