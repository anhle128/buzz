# GitHub pull, push, and remote branches

**Status:** Draft 2026-08-18  
**Scope:** Buzz Desktop Projects — G5 + G6 + G7 only. When a repository clone URL is `github.com`, the project header uses the same Pull / Push / Fetch control as Buzz, Fetch updates local remotes when a checkout exists, and Create / Delete remote branch work through the existing git commands.  
**Plan:** [phase-01-git-native.md](../../../plans/20260818-1211-github-native-host/phase-01-git-native.md) slice G5 + G6 + G7.

## Summary

Desktop pull, push, sync-status, and remote create/delete still call `validate_workspace_clone_url`, which rejects GitHub. `RepoSyncActionButton` treats every external host as **Open**. Create branch is shown; the Tauri command rejects the URL. After G1 the picker has a real HEAD, so the button enables and then fails.

This design reuses the existing git commands. It widens the URL gate, makes `GitAuthConfig` host-aware, and enables `useProjectRepoSyncStatusQuery` for GitHub. It does not add `gh api` ref mutations and does not publish Nostr PR updates on GitHub push.

## Problem and root cause

`get_project_repo_sync_status`, `pull_project_local_repository`, `push_project_local_repository`, `create_project_remote_branch`, and `delete_project_remote_branch` all call `validate_workspace_clone_url`.

`build_git_auth_config` clears inherited credential helpers and injects `git-credential-nostr` plus `NOSTR_PRIVATE_KEY`. That cannot authenticate to GitHub. `build_git_clone_auth_config` already skips the nostr helper for GitHub but installs no replacement, so private HTTPS clone has no credentials.

`RepoSyncActionButton` returns Open whenever `remoteKind === "external"`.

`projectBranchCreationReason` treats a missing G1 commit as “create the first commit,” which is wrong when the real failure is missing `gh` / auth.

## Goals

- GitHub header matches Buzz: **Pull** when behind, **Push** when ahead, otherwise **Fetch**.
- Fetch with a local checkout runs `git fetch` (via sync status) and refetches G1 and G3 (if present).
- Fetch with no checkout stays query-only. Pull/Push are hidden.
- Create / Delete remote branch succeed for `github.com` when G1 has a source commit.
- HTTPS GitHub git uses `gh auth git-credential`. SSH uses the agent. Buzz never stores a GitHub token.
- Create / Delete stay visible; disable them with the real reason. Auth failures use merge/G1 recovery copy, not “create the first commit.”
- Leave Buzz-hosted sync, pull, push, and branch commands on the nostr helper.

## Non-goals

- Remote snapshot / README (G3). Do not open `get_project_repo_snapshot` to GitHub.
- `get_github_ahead_behind` (G4). When a checkout exists, sync status is the only ahead/behind source. Do not call G4 from the project page.
- GitHub Issues (M2) or GitHub PR list/create (M3).
- Branch → channel creation.
- Persisting `default-branch` on kind:30617 (X1 / M4).
- GitHub tags in the picker.
- GitHub Enterprise, GitLab, OAuth tokens stored in Buzz.

G1 + G2 are implemented. G3 + G4 are specified separately. This spec does not change those commands.

## Product decisions

- **Reuse:** No new pull/push/create/delete Tauri commands. Same bodies; new URL gate + auth builder.
- **Header:** Same contextual button as Buzz. Other non-GitHub external hosts stay Open-as-primary.
- **Open:** “Open on GitHub” stays on `RepoSourceDropdown` so removing primary Open is not a regression.
- **Auth:** Host-aware helper. `gh` + `ensure_auth` for HTTPS only.
- **Push side effects:** GitHub push does not call `publishProjectPullRequestUpdate`.
- **G7:** Hide Pull/Push unless `can_pull` / `can_push`. Keep Create/Delete in the menu.

## Architecture

```
github.com + local checkout
  get_project_repo_sync_status            → git fetch + ahead/behind + can_pull/can_push
  pull_project_local_repository           → git pull --ff-only
  push_project_local_repository           → git push
  create/delete_project_remote_branch     → existing temp-bare + leased push

github.com + no checkout
  Fetch = refetch G1 (and G3 if present). No git fetch.
  Pull/Push hidden.
  Create/Delete allowed when G1 has a source commit.

Buzz /git/<pubkey>/<id>
  unchanged (nostr helper + existing validators)
```

## Components

### `validate_git_operation_url` (new)

Accept a Buzz workspace clone URL **or** `GitHubRepoRef::parse`. Call it from:

- `get_project_repo_sync_status`
- `pull_project_local_repository`
- `push_project_local_repository`
- `create_project_remote_branch`
- `delete_project_remote_branch`

Do not call it from `get_project_repo_snapshot`. Clone stays on `validate_local_clone_url`.

### Host-aware `GitAuthConfig`

Replace the GitHub branch of `build_git_clone_auth_config` with one builder used by clone, sync, pull, push, and remote branch commands.

| Clone URL | Helper | Extra |
|-----------|--------|--------|
| Buzz `/git/...` | `git-credential-nostr` + `NOSTR_PRIVATE_KEY` | Unchanged |
| `https://github.com/...` | `!<gh-path> auth git-credential` | `GhRunner::discover` then `ensure_auth`. No nsec. |
| `git@github.com:...` | None | SSH agent. No nsec. Do not call `ensure_auth` for git. |

Keep `GIT_TERMINAL_PROMPT=0`, `GIT_CONFIG_NOSYSTEM`, empty inherited `credential.helper`, and hooks disabled. GitHub processes must not receive `NOSTR_PRIVATE_KEY`.

HTTPS `discover` / `ensure_auth` failures return `serde_json::to_string` of `ProjectPullRequestMergeError` so `parseProjectPullRequestMergeError` works on today’s `Result<T, String>` commands. Remap `github_merge_failed` off this path. Codes: `github_cli_missing`, `github_auth_required`, `github_repo_unavailable`, `github_state_failed`. Git stderr (protected branch, non-fast-forward, SSH denied) stays a plain string.

### Frontend

- `useProjectRepoSyncStatusQuery` enabled when the project has a clone URL and (Buzz host **or** (`isGitHubCloneUrl` and G1 succeeded)). Do not poll GitHub sync while G1 is in error. Existing 60s focused poll applies when enabled (each poll `git fetch`es when a checkout exists).
- `RepoSourceHeaderControls` gets `isGithubRemote`. `RepoSyncActionButton` uses Pull / Push / Fetch when that flag is true. Other `remoteKind === "external"` stays Open.
- `RepoSourceDropdown`: when `isGithubRemote` and `externalUrl` is set, add “Open on GitHub” (`isSafeUrl`).
- `usePushProjectLocalRepositoryMutation`: skip `publishProjectPullRequestUpdate` when `isGitHubCloneUrl`.
- Create/Delete titles: if G1 failed on a GitHub clone URL, use that recovery reason. Do not call `projectBranchCreationReason` in that state.
- If G1 failed, do not start sync / create / delete. One header recovery is enough.

### Unchanged

`get_project_repo_snapshot`, G1 `get_github_repository_state`, G3 snapshot command, G4 compare command, Buzz nostr helper, lease checks, `--ff-only` pull, default-branch delete guard.

## Data flow

1. User opens a GitHub-hosted project. G1 loads `RepoState` and selects GitHub’s default (for example `develop`).
2. If a checkout exists, sync status fetches and computes counts / `can_*`. Header is Pull, Push, or Fetch.
3. No checkout: sync returns no path and `can_*` false. Header is Fetch. Fetch refetches G1 (and G3 if present) only.
4. Fetch with checkout: refetch sync status, G1, and G3. Toast on failure. Do not call G4.
5. Pull / Push use the existing commands and the same block reasons as Buzz, then invalidate project queries.
6. Create: temp-bare fetch of the source tip, lease-create, refetch G1, select the new branch.
7. Delete: refuse the GitHub default branch; lease-delete the observed commit; select the default; refetch G1.
8. Buzz-hosted projects never take the GitHub helper path.

## Error handling

Do not invent a successful sync (`0 / 0`, `can_pull`, `can_push`) on failure.

| Situation | Code / result | UI |
|-----------|---------------|----|
| `gh` not on PATH (HTTPS GitHub) | `github_cli_missing` | Same title/body as merge/G1. Create/Delete visible, disabled, that reason. |
| `gh auth status` fails (HTTPS) | `github_auth_required` | `gh auth login --hostname github.com` plus copy + Retry. Same Create/Delete treatment. |
| SSH GitHub, no `gh` | Git uses the agent | G1 may still show CLI recovery (API). Git fails only if SSH fails. |
| Repo HTTP 404, or 403 that is not a rate/abuse limit | `github_repo_unavailable` | Recovery on the header. No Pull/Push. |
| Rate/abuse 403, timeout, helper/git crash | `github_state_failed` or git stderr | Retry. Page stays up. |
| Protected branch, non-fast-forward, SSH permission denied | git stderr (string) | Toast. Do not remap to `github_auth_required` unless `ensure_auth` failed. |
| No checkout | `can_*` false | Pull/Push hidden. Fetch stays. |
| G1 already failed | — | Do not start sync/create/delete. One recovery. Create/Delete disabled with that reason. |
| Buzz git / 30618 errors | Unchanged strings | No GitHub recovery copy. Gate on `isGitHubCloneUrl`. |

Retry calls `refetch` on the failed query. No automatic infinite retry.

## Testing

No live GitHub in unit tests. Inject `GhRunner` / fake `gh` like G1.

**Rust**

- `validate_git_operation_url` accepts `https://github.com/…` and `git@github.com:…`; rejects GitLab; still requires the active relay for Buzz `/git/…`.
- HTTPS GitHub auth: helper is `!<gh> auth git-credential`; environment has no `NOSTR_PRIVATE_KEY`.
- SSH GitHub auth: no helper, no nsec, no `ensure_auth`.
- Missing `gh` on HTTPS → `github_cli_missing` before git.
- Failed `ensure_auth` → `github_auth_required`.
- Buzz path still uses `git-credential-nostr`.
- Create/delete still lease-check the expected commit when the clone URL is GitHub.
- Fake-`gh` must not treat credential-helper invocations as API paths.

**TypeScript**

- GitHub clone URL enables `useProjectRepoSyncStatusQuery`; GitLab does not.
- Header: GitHub + checkout + behind → Pull; ahead → Push; else Fetch. Not Open-as-primary.
- Open stays on the source dropdown.
- Push does not call `publishProjectPullRequestUpdate`.
- G1 error: Create/Delete disabled with recovery copy, not “create the first commit.”
- Buzz `/git/…` never uses GitHub recovery copy.

**e2e (mock bridge)**

- Stub sync `behindCount: 1` / `canPull: true` → Pull is the primary control.
- Stub `canPush: true` → Push.
- Stub no `localPath` → Fetch only; Create still enabled when G1 has a commit.
- Stub `github_auth_required` on state → recovery; Create disabled with that reason.

Existing Buzz sync/branch tests stay green.

## Success criteria

On a GitHub-hosted project whose default branch is `develop`, with `gh` installed and authenticated:

- A checkout that is behind shows Pull; Pull fast-forwards the working tree.
- A checkout that is ahead shows Push; Push updates GitHub. Fetch updates local remotes.
- Create branch from a known HEAD works. Delete refuses `develop`.
- With no checkout, Fetch is query-only and Create still works.
- Without `gh` or without auth on HTTPS, recovery matches G1/merge; Create/Delete do not say “create the first commit.”

Buzz-hosted repositories still sync, pull, push, and create/delete as they do today.
