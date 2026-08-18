# GitHub remote snapshot and Fetch (ahead/behind)

**Status:** Approved 2026-08-18  
**Scope:** Buzz Desktop Projects — G3 + G4 only. When a repository clone URL is `github.com`, the project-detail remote source loads README, file tree, and recent commits via `gh api`, and Fetch refreshes that data. Ahead/behind is shown only when a local checkout exists.  
**Plan:** [phase-01-git-native.md](../../../plans/20260818-1211-github-native-host/phase-01-git-native.md) slice G3 + G4.

## Summary

Desktop git remote snapshot and sync status are Buzz-only. `useProjectRepoSnapshotQuery` is enabled only when `host.kind === "buzz"`. `useProjectRepoSyncStatusQuery` is the same. `RepoSyncActionButton` treats every external host as **Open**, so GitHub never Fetches.

This design adds two GitHub-only Tauri commands that reuse `GhRunner` and the existing `ProjectRepoSnapshot` type. Buzz `get_project_repo_snapshot` and `get_project_repo_sync_status` stay Buzz-only. Fetch does not run `git fetch`. Pull/Push stay hidden (G5).

## Problem and root cause

`get_project_repo_snapshot` and `get_project_repo_sync_status` call `validate_workspace_clone_url`, which accepts only the active Buzz relay git path.

`ProjectDetailScreen` passes `repoRemote.host.kind === "buzz"` into `useProjectRepoSnapshotQuery`, so GitHub never calls the snapshot command.

On failure the README panel shows “Code hosted on github.com” and tells the user to clone. `RepoSyncActionButton` returns an Open link whenever `remoteKind === "external"`.

Local checkout already works for GitHub (`get_project_local_repo_snapshot` and `clone_project_repository` use `validate_local_clone_url`). Remote browse and Fetch do not.

## Goals

- On the project page, a `github.com` remote source shows README, file tree, and recent commits for the selected branch.
- Use `gh api` only for that remote snapshot. Do not blobless-clone GitHub into a temp dir.
- Fetch is the primary header action for GitHub. It refetches GitHub state, snapshot, and compare. It does not update local remotes.
- Show ahead/behind only when a local HEAD exists. Do not show Pull or Push.
- Keep Open as a secondary “Open on GitHub” item on the source dropdown.
- Reuse G1/merge recovery codes and copy. Do not invent a successful snapshot or `0 / 0` on failure.
- Leave Buzz-hosted snapshot and sync on the existing git commands.

## Non-goals

- Pull / push in the app (G5).
- Create or delete branches (G6).
- Fix Create-branch disabled copy (G7).
- Community overview / card snapshots (`useProjectsRepoSnapshots`).
- Per-file previews other than README. Opening a non-README file does not fetch contents in this slice.
- Contributors on the GitHub snapshot (`contributors` is always `[]`).
- `git fetch`, `git ls-remote`, or `gh auth git-credential`.
- GitHub tags, `refs/nostr/…` PR snapshots, or GitHub PR files (M3).
- Persisting `default-branch` on kind:30617 (X1 / M4).
- GitHub Enterprise, GitLab, OAuth tokens stored in Buzz.

G1 + G2 (branch list + default branch) are already specified and implemented. This spec does not change that command.

## Product decisions

- **Host split:** `isGitHubCloneUrl` / `GitHubRepoRef::parse`. GitHub is the only remote snapshot source for that host. Do not union with a Buzz clone snapshot.
- **Auth:** installed `gh`, `gh auth status --hostname github.com`. Buzz never reads or stores a GitHub token.
- **Snapshot fidelity:** README `previewContent` + recursive tree (cap 250) + last 50 commits on the selected ref. No other file bodies.
- **Fetch:** refresh GitHub queries only. “Fetch updates local remotes” waits for G5.
- **Compare:** `GET /repos/{slug}/compare/{remoteSha}...{localSha}`. If GitHub does not know `local_sha`, counts are omitted — not `0 / 0`.
- **Surfaces:** project detail only.

## Architecture

```
github.com clone URL
  G1/G2  get_github_repository_state(clone_url)           → RepoState
  G3     get_github_repository_snapshot(clone_url, ref)   → ProjectRepoSnapshot
  G4     get_github_ahead_behind(clone_url, branch, local_sha, remote_sha)?
         → { status, ahead?, behind? }

Fetch = refetch those queries (G4 skipped when there is no local HEAD)

Buzz /git/<pubkey>/<id>
  kind:30618 + get_project_repo_snapshot + get_project_repo_sync_status
  unchanged
```

`ProjectRepoSnapshot` stays `{ latestCommit, commits, files, contributors }`.

## Components

### `GitHubRepoRef` (existing)

Parses `https://github.com/<owner>/<repo>[.git]` or `git@github.com:`. Rejects other hosts. Invalid clone URL fails before `gh`.

### `get_github_repository_snapshot` (new Tauri command)

- **Input:** `clone_url: string`, `ref: string` (selected branch name, already cleaned by the same allowlist as Buzz `clean_branch`).
- **Auth:** `GhRunner::discover` then `ensure_auth`.
- **Calls:**
  1. `GET /repos/{slug}/commits?sha={ref}&per_page=50`  
     `--jq` each row to `{sha, tree, name, email, date, subject}` (`tree` = `commit.tree.sha`) so the payload stays under `GH_STREAM_LIMIT`. Map to `latest_commit` (first row) and `commits`.
  2. If that list is empty (empty repository), return an empty snapshot. Do not call tree or README.
  3. `GET /repos/{slug}/git/trees/{tree_sha}?recursive=1`  
     `tree_sha` is the first commit’s `tree`. `--jq` project `{path, type, size}` and take at most 250 entries. Do not deserialize the full GitHub tree object through `GH_STREAM_LIMIT`.
  4. `GET /repos/{slug}/readme?ref={ref}`  
     Decode base64 `content`. Cap preview at 64 KiB (same as Buzz `MAX_PREVIEW_BYTES`). That JSON can exceed `GH_STREAM_LIMIT`; this call may use a 256 KiB stdout cap. If stdout is still truncated, succeed with no preview (do not fail the snapshot). Attach `previewContent` on the README path only. If that path is missing from the 250-file window, insert that one file so the README panel still works.
- **Output:** `ProjectRepoSnapshot`. `contributors` is `[]`. Non-README files have `previewContent: null`, `latestCommit: null`, `lastChangedAt: null`.
- README HTTP 404 is success with no preview.
- A truncated GitHub tree is success; still return the first 250 projected entries.
- Do not pass `refs/nostr/…` or `refs/tags/…` as `ref`.

Register next to `get_github_repository_state`.

### `get_github_ahead_behind` (new Tauri command)

- **Input:** `clone_url: string`, `branch: string`, `local_sha: string`, `remote_sha: string` (`remote_sha` is the G1 branch tip already loaded; do not re-list branches).
- **Auth:** same `GhRunner`.
- If `local_sha` equals `remote_sha`, return `{ status: "compared", ahead: 0, behind: 0 }` without calling compare.
- Else `GET /repos/{slug}/compare/{remote_sha}...{local_sha}` → `ahead_by` / `behind_by` → `{ status: "compared", ahead, behind }`.
- If GitHub returns 404 because `local_sha` is not on the remote, return `{ status: "unpushed" }` with no counts. That is not `github_repo_unavailable` (the repository exists).
- Do not set `can_pull` or `can_push`. Do not call `git fetch`.

### Frontend routing

- `fetchProjectRepoSnapshot`: `isGitHubCloneUrl` → `get_github_repository_snapshot` with the selected branch. Otherwise existing Buzz clone snapshot. GitHub path ignores `targetRef` / `targetCommit` used for Buzz tags and `refs/nostr/…`.
- `useProjectRepoSnapshotQuery` enabled when the project has a clone URL **and** (Buzz host **or** `isGitHubCloneUrl`). Query key includes clone URL + branch (same fields as today, plus clone URL so community switch cannot reuse the wrong cache).
- `useProjectsRepoSnapshots` is unchanged (overview stays off GitHub API).
- `useProjectRepoSyncStatusQuery` stays `host.kind === "buzz"` only.
- New `useGithubAheadBehindQuery`: enabled when `isGitHubCloneUrl`, a local HEAD sha exists, and G1 has a tip sha for the selected branch. Pass that tip as `remote_sha`. Local sha comes from `get_project_local_repo_snapshot` / existing local snapshot query (already allowed for GitHub).
- `RepoSyncActionButton`: if the clone URL is GitHub, render Fetch (ahead/behind from the new query). Do not render Open as the primary control. Do not render Pull/Push even if counts are non-zero.
- `RepoSourceDropdown`: when `externalUrl` is set, add “Open on GitHub” (existing `externalUrl`, `isSafeUrl`).

### Unchanged

`get_project_repo_snapshot`, `get_project_repo_sync_status`, kind:30618, local snapshot/clone, G1 `get_github_repository_state`, merge.

## Data flow

1. User opens a GitHub-hosted project.
2. G1 loads `RepoState`. Picker selects GitHub’s default (for example `develop`). If G1 fails, G3 and G4 do not run; one header recovery is enough.
3. G3 runs with the selected branch. Remote source shows README + files + recent commits. The “Code hosted on github.com / clone to explore” empty state is not used when the snapshot succeeds.
4. Changing the branch picker re-runs G3 for the new ref.
5. If a local checkout has `HEAD`, G4 compares that sha to the G1 tip for the selected branch. Equal → `0 / 0`. Compare success → numeric counts. `unpushed` → hide counts.
6. Fetch refetches G1, G3, and G4 (G4 omitted when there is no local HEAD).
7. Buzz-hosted projects never call the two new commands.

## Error handling

Do not invent a successful snapshot or `0 / 0` on failure.

| Situation | Code | UI |
|-----------|------|-----|
| `gh` not on PATH | `github_cli_missing` | Same title/body as G1/merge: install GitHub CLI |
| `gh auth status` fails | `github_auth_required` | `gh auth login --hostname github.com` plus copy + Retry |
| Repo HTTP 404, or 403 that is not a rate/abuse limit | `github_repo_unavailable` | `gh` message; remote source disabled |
| Rate/abuse 403, timeout, truncated JSON, other `gh` failure | `github_state_failed` | Retry; project page stays up |
| README 404 | *(not an error)* | Snapshot succeeds; no README preview |
| Tree truncated or longer than 250 | *(not an error)* | First 250 entries |
| Local SHA unknown to GitHub | `status: "unpushed"` | Ahead/behind omitted; Fetch still works |
| G1 already failed | — | Do not start G3/G4; one recovery on the header |

Reuse `GitHubRepoStateRecovery` and G1 remap rules. `github_merge_failed` must not leak. Gate recovery copy with `isGitHubCloneUrl`. Buzz snapshot/sync errors stay generic.

If G1 succeeded and G3 fails: picker stays on the GitHub default; README/files show recovery, not the GitHub-host empty card and not an empty tree that looks like success.

Retry calls `refetch` on the failed query. No automatic infinite retry.

## Testing

No live GitHub in unit tests. Inject `GhRunner` fixtures like G1/merge.

**Rust**

- Fixture commits + tree + README map to `latest_commit`, listed paths, README `preview_content` only, `contributors` empty.
- README 404 still returns a snapshot.
- Non-GitHub URL fails before `gh`.
- Missing binary → `github_cli_missing`; failed auth → `github_auth_required`.
- Compare: equal SHAs → `0/0`; fixture `ahead_by`/`behind_by`; unknown head SHA → `unpushed`, not `0/0`.
- Fake-`gh` matches `/commits`, `/git/trees`, `/readme`, `/compare` **before** the repo root path.

**TypeScript**

- GitHub clone URL uses the GitHub snapshot command and does not call the Buzz clone snapshot.
- Buzz `/git/...` does not call the GitHub snapshot or compare commands.
- Project-page snapshot query is enabled for GitHub; overview helper is unchanged.
- GitHub does not enable `useProjectRepoSyncStatusQuery`.
- Header: GitHub shows Fetch, not Open-as-primary; no Pull/Push; Open is on the source dropdown.

**e2e (mock bridge)**

- Stub snapshot with README + `develop` files: remote source shows README, not “Code hosted on github.com”.
- Stub `github_auth_required` on snapshot: recovery, not the empty GitHub-host card.
- Local HEAD + compare stub shows ahead/behind; Fetch invokes the GitHub commands.

Existing Buzz snapshot/sync tests stay green.

## Success criteria

On a GitHub-hosted project whose default branch is `develop`, with `gh` installed and authenticated:

- Opening the project on the remote source shows that branch’s README and file tree.
- Fetch refreshes GitHub state and snapshot. The primary header control is Fetch, not Open.
- With a local checkout whose `HEAD` matches the GitHub branch tip, ahead/behind is `0 / 0`. With an unpushed local SHA, counts are hidden rather than `0 / 0`.
- Pull and Push do not appear.
- Without `gh` or without auth, recovery matches G1/merge; the page does not pretend the remote tree is empty because the code is “on GitHub”.

Buzz-hosted repositories still clone-snapshot and `git fetch` as they do today.
