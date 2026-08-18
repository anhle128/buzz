# GitHub repository state (branch list + default branch)

**Status:** Approved 2026-08-18  
**Scope:** Buzz Desktop Projects — G1 + G2 only. When a repository clone URL is `github.com`, the branch picker and default branch come from GitHub via `gh api`.  
**Plan:** [phase-01-git-native.md](../../../plans/20260818-1211-github-native-host/phase-01-git-native.md) slice G1 + G2.

## Summary

For GitHub-hosted repositories, Desktop today treats kind:30618 and the announcement `default-branch` tag (fallback `"main"`) as the only branch source. GitHub pushes never publish 30618, so the picker shows only `main` after a GitHub default-branch change.

This design makes `useRepoStateQuery` host-aware: GitHub-hosted repos load `RepoState` from `gh api`. Buzz-hosted repos keep kind:30618. The existing picker and `resolveProjectDefaultBranch` stay the same.

## Problem and root cause

`eventToRepository` sets `defaultBranch` from tag `default-branch` or `"main"` (`desktop/src/features/projects/projectModels.ts`).  
`fetchRepoState` reads only kind:30618 (`desktop/src/features/projects/hooks.ts`).  
`resolveProjectDefaultBranch` then prefers 30618 `HEAD` if present.

A GitHub-linked announcement usually has no `default-branch` tag and no 30618 event. The UI therefore always lands on `main` and lists no GitHub remotes.

## Goals

- List GitHub branches in the project branch picker when the clone URL is `github.com`.
- Select GitHub’s `default_branch` on first open (for example `develop`).
- Use `gh api` only (same auth as merge). Do not use `git ls-remote` in this slice.
- Surface `github_cli_missing` and `github_auth_required` with the same recovery copy as merge. Do not silently fall back to `main`.
- Leave Buzz-hosted repository state on kind:30618.

## Non-goals

- Remote file / README / commit snapshot (G3).
- Fetch, pull, push, ahead/behind (G4–G5).
- Create or delete branches (G6).
- Hiding Create branch until G6 (G7) — out of this spec; current disabled copy may remain wrong until G6.
- Persisting `default-branch` on kind:30617 (X1 / M4).
- GitHub tags in the picker.
- GitHub Enterprise, GitLab, OAuth tokens stored in Buzz.

Opening `develop` in the picker does not load GitHub tree content until G3.

## Product decisions

- **Host split:** `projectRepoHostForRepository` already classifies Buzz vs `github.com`. GitHub is the only source of `RepoState` for that host. Do not union with kind:30618.
- **Auth:** installed `gh`, `gh auth status --hostname github.com`. Buzz never reads or stores a GitHub token.
- **Default branch:** GitHub `default_branch` maps to `RepoState.head`. `resolveProjectDefaultBranch(announced, repoState)` is unchanged.

## Architecture

```
github.com clone URL  → get_github_repository_state → RepoState
Buzz /git/...         → kind:30618                  → RepoState
```

`RepoState` stays `{ branches: [{ name, commit }], tags, head, updatedAt }`.  
For this slice GitHub `tags` is always `[]`.

## Components

### `GitHubRepoRef` (existing)

Parses a plain `https://github.com/<owner>/<repo>[.git]` or `git@github.com:` URL. Rejects other hosts.

### `get_github_repository_state` (new Tauri command)

- **Input:** `clone_url: string`
- **Auth:** `GhRunner::discover` then `ensure_auth` (same as merge)
- **Calls:**
  1. `GET /repos/{owner}/{repo}` → `default_branch`
  2. Branch pages via `gh api --paginate --slurp` **or** explicit `page=N` loops. Never deserialize `--paginate` without `--slurp` (`gh` emits one JSON array per page).
- **Output:** `{ head, branches: [{ name, commit }], tags: [], updated_at }`
  - `head` = `default_branch`
  - `commit` = branch `commit.sha`
  - `updated_at` = Unix seconds now (no GitHub timestamp required)

Invalid clone URL fails before `gh`.

### `fetchRepoState` (existing, routed)

- Host `external` + `github.com` and a clone URL → invoke `get_github_repository_state`
- Otherwise → current kind:30618 fetch

### `useRepoStateQuery`

Query key remains `["project", id, "repo-state"]`.  
`ProjectDetailScreen` keeps using `repoStateQuery.data` for `resolveProjectDefaultBranch` and `useOptimisticProjectBranches`.

## Data flow

1. User opens a GitHub-hosted repository (for example `harness-service`).
2. Host is `github.com`.
3. Query calls `get_github_repository_state`.
4. On success: `head` is `develop` if that is GitHub’s default; `branches` includes every GitHub branch.
5. Picker selects `develop` and lists those names.
6. Snapshot, local checkout, Fetch, and Create branch behave as today (G3–G6 not in this spec).

## Error handling

Do not invent a successful `RepoState` on failure.

| Situation | Code | UI |
|-----------|------|-----|
| `gh` not on PATH | `github_cli_missing` | Same title/body as merge: install GitHub CLI |
| `gh auth status` fails | `github_auth_required` | `gh auth login --hostname github.com` plus copy button |
| Repo HTTP 404, or 403 that is not a rate/abuse limit | `github_repo_unavailable` | `gh` message; picker disabled |
| Rate/abuse 403, timeout, truncated JSON, other `gh` failure | `github_state_failed` | Retry; project page stays up |

`useRepoStateQuery` is in error state (`data` is undefined).

For a **GitHub-hosted** repo in that state, do **not** call `resolveProjectDefaultBranch` with the announcement default. That tag is usually missing and falls back to `"main"`, so the optimistic picker would still select `main`. Pass `defaultBranch = null` and `observedBranches = []` until GitHub state succeeds. Show recovery next to the real header rows (`ProjectWorkspaceTabs`, `ProjectReadmePanel`, `ProjectRepositoryPanel`), not only in `ProjectRepositorySource.tsx`. Retry calls `refetch`. No automatic infinite retry.

Buzz kind:30618 errors must **not** use GitHub-only copy. Gate recovery on `isGitHubCloneUrl`.

## Testing

No live GitHub in unit tests. Inject `GhRunner` fixtures like merge tests.

- GitHub clone URL does not fetch kind:30618.
- Buzz `/git/...` URL does not call `gh`.
- Fixture JSON maps to `head: "develop"` and the listed branch names.
- `resolveProjectDefaultBranch` returns `develop` when `head` is `develop` and that name is in `branches`.
- Missing binary → `github_cli_missing`; failed auth → `github_auth_required`.
- e2e mock bridge stubs `get_github_repository_state`.
- One mock-bridge smoke spec asserts the picker selects `develop` when the stub returns `head: "develop"`, and shows recovery (not `main`) when the stub throws `github_auth_required`.

Existing 30618 tests stay green.

## Success criteria

On a GitHub-hosted project whose GitHub default branch is `develop`, with `gh` installed and authenticated:

- Opening the repository selects `develop`.
- The branch dropdown lists GitHub remotes (`develop`, `main`, feature branches).
- Without `gh` or without auth, the picker does not pretend the default is `main`; it shows the merge-style recovery.

Buzz-hosted repositories still read kind:30618 only.
