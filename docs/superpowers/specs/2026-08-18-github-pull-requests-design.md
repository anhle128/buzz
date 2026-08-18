# GitHub pull requests (list + create)

**Status:** Draft — awaiting user review  
**Scope:** Buzz Desktop Projects — P1 + P2 only. When a repository clone URL is `github.com`, the per-repo Pull Request tab lists and creates GitHub pull requests via `gh api`.  
**Plan:** [phase-03-github-pull-requests.md](../../../plans/20260818-1211-github-native-host/phase-03-github-pull-requests.md) slice P1 + P2.

## Summary

The Pull Request tab lists NIP-34 kind:1618 events keyed by repo address. Create always signs kind:1618. Merge for GitHub already goes through `gh` (`merge_github_pull_request`), but that path is Nostr-PR-first: it find-or-creates a GitHub PR from a 64-hex event, then publishes kind:1631.

GitHub-hosted repos do not publish 1618, so the tab is empty and Create would dual-write a Buzz PR that GitHub does not own.

This design makes `fetchProjectPullRequests` and `useCreateProjectPullRequestMutation` host-aware: `github.com` uses GitHub pull requests as the only backend. Buzz-hosted repos keep kind:1618. The existing tab and create dialog stay. Conversation comments are read-only. Merge, review writes, and Files changed stay hidden on GitHub rows.

## Problem and root cause

`fetchProjectPullRequests` loads kind:1618, kind:1613 updates, kind:1 comments, and status kinds (`desktop/src/features/projects/hooks.ts`).  
`publishProjectPullRequest` always signs kind:1618 (`desktop/src/features/projects/pullRequestMutations.ts`).  
`ProjectPullRequest.id` is a 64-hex event id; authors and reviewers are pubkeys; `#` in the row is `id.slice(0, 8)`.

A GitHub clone URL never yields 1618 events. Create would publish a Nostr PR that GitHub does not own. Merge would then treat `"42"` as an event id and a GitHub login as a pubkey — that is why this slice hides Merge instead of reusing it.

## Goals

- List open GitHub pull requests as `#N` on the per-repo Pull Request tab when the clone URL is `github.com`.
- Create a pull request on GitHub. Do not publish kind:1618 for that host.
- Show thin in-app detail: title, `#N`, Open/Draft, body, author login + avatar, branches, copy GitHub URL.
- Load conversation comments as **read-only** (issue comments on the PR).
- Surface `github_cli_missing` and `github_auth_required` with the same recovery as merge / G1/G2. Do not show “No pull requests yet” on auth failure.
- Leave Buzz-hosted pull requests on kind:1618.

## Non-goals

- Review / required-check UI (P3). Checks tab stays the current empty placeholder.
- Merge-by-number / skip kind:1631 (P4 adapt). Hide Merge on GitHub rows. Do not change `merge_github_pull_request`.
- Review comments, inline comments, or posting any comment (P5).
- Close / reopen / convert to draft / mark ready.
- Closed or Merged list. No Open | Closed | Merged filter. No `state=all`, second page, or Load more.
- Draft checkbox on create. Create is a ready (non-draft) PR.
- Cross-repository create (`head_repo`). Create is same-repo only.
- Files changed / GitHub files API. Hide the Files changed sub-tab.
- Global Projects PR list (`fetchProjectsWorkItems`) and card / activity counts (X2 / M4).
- New `buzz://pr` scheme for GitHub numbers.
- CLI (`buzz` PR commands) and mobile.
- GitHub Enterprise, GitLab, OAuth tokens stored in Buzz.
- Importing GitHub PRs into Nostr. No dual-write.
- Reviewer assignment, approve, request changes, merge queue, auto-merge.

## Product decisions

- **Host split:** `isGitHubCloneUrl` / `GitHubRepoRef::parse`. GitHub is the only PR source for that host. Do not union with kind:1618.
- **Auth:** installed `gh`, `gh auth status --hostname github.com`. Buzz never reads or stores a GitHub token. The Buzz npub is not a GitHub actor.
- **List:** `state=open` only. First 100 items from one page, `sort=updated`, `direction=desc`. No `--paginate`. Include inbound fork PRs. Drafts stay in this list (`draft: true` → status `"Draft"`).
- **More:** `hasMore` is true when the **raw** GitHub page has 100 items. One muted note, not a button: “More open pull requests exist on GitHub.”
- **Identity:** authors are GitHub logins + avatar URLs, not pubkeys. Never pass a login to `ProfileIdentityButton`, `normalizePubkey`, review mutations, or merge.
- **Status:** GitHub `draft` → `"Draft"`, else `"Open"`. This slice never maps Merged or Closed.
- **Branches:** show `head.ref → base.ref`. When `head.repo.full_name` differs from the target (case-insensitive), show `owner:branch → base`.
- **Share:** Copy link uses validated `html_url`. No `buzz://` change.
- **Create:** title + body + compare (`head`) + base. Title required after trim, max 256 characters (same as Buzz). Body may be empty. No reviewers. No `draft`. No `head_repo`.
- **Surface:** per-repo Pull Request tab only. Global create dialog still routes by the **selected repository** host so a GitHub target never signs 1618.
- **Comments:** list carries `commentCount` from GitHub `comments`. Bodies load when a row is opened (first 100 issue comments). Composer is hidden.
- **Commits tab:** one row for `head.sha`. Badge is `1`. Do not invent update rows.
- **Checks tab:** keep “No checks have been reported for this pull request yet.”
- **Files changed:** hide the trigger and the `pr-files` panel. If the selected tab is `pr-files` on a GitHub-hosted repo, snap to `pr-conversation`.
- **Merge / reviews:** hide `MergePullRequestButton`, `PullRequestReviewCard`, `ForumComposer`, Request changes, and the reviewers row (including Request review).

## Architecture

```
github.com clone URL  → list / create / comments via gh api  → ProjectPullRequest
Buzz /git/...         → kind:1618 + updates + status         → ProjectPullRequest
```

Detail comments (GitHub only):

```
selected #N  → list_github_pull_request_comments → ProjectPullRequest.comments
```

New Tauri module: `desktop/src-tauri/src/commands/project_github_pulls.rs`.

Reuse `GhRunner`, `GitHubRepoRef`, `ensure_auth`, redaction, and `ProjectPullRequestMergeError` from `project_github_pull_request.rs`. Do **not** call `merge_github_pull_request` or find-or-create. Do not add list/create into the merge module.

No provider trait. No `gh pr list` / `gh pr create`.

## Components

### `GitHubRepoRef` (existing)

Parses a plain `https://github.com/<owner>/<repo>[.git]` or `git@github.com:` URL. Rejects other hosts. Invalid clone URL fails before `gh`.

### `project_github_pulls.rs` (new)

Register the three commands next to `get_github_repository_state` in `desktop/src-tauri/src/lib.rs`. `number` is a `u64` greater than 0, never a raw path segment from unparsed text.

| Command | Input | GitHub | Output |
|---|---|---|---|
| `list_github_pull_requests` | `clone_url` | `GET /repos/{owner}/{repo}/pulls?state=open&per_page=100&sort=updated&direction=desc` | `{ pulls, has_more }` |
| `create_github_pull_request` | `clone_url`, `title`, `body`, `head`, `base` | `POST /repos/{owner}/{repo}/pulls` via `--input` tempfile | one PR DTO |
| `list_github_pull_request_comments` | `clone_url`, `number` | `GET /repos/{owner}/{repo}/issues/{number}/comments?per_page=100` | comment list |

Conversation comments use the **issues** comments endpoint. `GET /repos/{owner}/{repo}/pulls/{number}/comments` is review comments (P5) — do not call it.

**PR DTO (bounded):** `number`, `title`, `body`, `html_url`, `draft`, `created_at`, `updated_at`, `comments` (count), `user.login`, `user.avatar_url`, `head.ref`, `head.sha`, `head.repo.full_name`, `base.ref`, `base.repo.full_name`. Ignore labels, milestone, requested reviewers, and other fields.

**`html_url` check:** accept only `https://github.com/{owner}/{repo}/pull/{number}` for the parsed target repo. Owner and repo compare case-insensitively. Number must match. Reject query, fragment, credentials, or a different host/path so Copy link cannot emit a foreign URL. Drop the PR if `html_url` fails this check.

**List rules:** one page. `has_more` is `raw_items.len() == 100`.

**Create:** JSON `{ "title", "body", "head", "base" }`. `head` is the compare branch name (same repo). Empty title, empty `head`/`base`, or `head == base` fails before `gh`. Return the created PR so the tab can select `#N`.

**Comments:** first 100, in GitHub’s order (oldest first). No reply threading. No prefetch for the whole list.

**Comment DTO:** `id`, `body`, `html_url`, `created_at`, `user.login`, `user.avatar_url`. Accept `html_url` only when it is one of:

- `https://github.com/{owner}/{repo}/issues/{number}#issuecomment-{id}`
- `https://github.com/{owner}/{repo}/pull/{number}#issuecomment-{id}`

for that repo and number. Drop the comment if the URL fails this check.

### `ProjectPullRequest` (extended)

Keep the type so the tab and detail shell stay. GitHub rows fill:

| Field | GitHub |
|---|---|
| `id` | `String(number)` — selection key (`"42"`) |
| `title` / `content` | title / body (`""` if body is null) |
| `author` | GitHub login |
| `authorAvatarUrl` | `user.avatar_url` (new, optional; unused on Nostr) |
| `status` | `"Draft"` if `draft`, else `"Open"` |
| `branchName` / `targetBranch` / `commit` | `head.ref` / `base.ref` / `head.sha` |
| `headRepoFullName` | `head.repo.full_name` (new, optional) |
| `htmlUrl` | validated GitHub PR URL (new). Nostr: `null` |
| `comments` | `[]` on the list query |
| `commentCount` | GitHub `comments` (new). Nostr mapper sets `comments.length` |
| `repoAddress` | Buzz announcement coordinate (React keys). Not a GitHub id |
| `reviewers`, `approvals`, `changeRequests`, `recipients`, `updates` | empty |
| `updateCount` | `0` |
| `cloneUrls` | repository clone URL |
| `channelId`, `originAgentName`, `statusEventId` | unused |

### `fetchProjectPullRequests` (existing, routed)

Inject loaders for tests, same idea as `fetchRepoStateWith`:

- `isGitHubCloneUrl(cloneUrl)` → `list_github_pull_requests` → map DTOs
- otherwise → current kind:1618 fetch; `hasMore = false`

`useProjectPullRequestsQuery` data shape becomes `{ pullRequests: ProjectPullRequest[]; hasMore: boolean }`. Callers that treated `data` as an array (`ProjectDetailScreen`, `CreatePullRequestDialog`, `PullRequestsPanel`) read `data.pullRequests`. Query key stays `["project", id, "pull-requests"]`. Stale time stays 30s.

### `useCreateProjectPullRequestMutation` (existing, routed)

- GitHub repo → `create_github_pull_request` → return `String(number)`
- Buzz repo → `publishProjectPullRequest` → event id

On GitHub success, invalidate only `["project", id, "pull-requests"]`. Do **not** invalidate `work-items` or `activity-summaries` (those stay Nostr). Buzz create keeps today’s invalidation.

`CreatePullRequestDialog` already passes the selected repository, branches, and source commit. Duplicate-open check uses the GitHub-backed list. Do not send `reviewers`.

### Comments query (new)

Enabled when the repo is GitHub and `selectedPullRequestId` matches `/^[1-9][0-9]*$/`. Parse it as `u64` before invoke. Key: `["project", id, "pull-requests", number, "comments"]`. Invoke `list_github_pull_request_comments`. Merge into the open PR for the conversation list. List fetch does not wait on this query.

### `pullRequestShareLink`

If `pullRequest.htmlUrl` is a validated GitHub PR URL, return it. Otherwise keep the hex `buzz://pr` path. Hide Copy link when both fail (same as today).

### Write isolation

- GitHub create / list / comments never `signRelayEvent` or `publishEvent`.
- `publishProjectPullRequestUpdate` (push hook in `repoSyncHooks.ts`) **skips** when `isGitHubCloneUrl`. GitHub already moved the head; do not publish kind:1613 against `"42"`.
- Reviewer, approve, request-changes, close/reopen, and merge mutations are not mounted on GitHub rows. If a leftover call fires, TypeScript must refuse a non-hex id before invoke.

## Data flow

1. User opens the Pull Request tab on a GitHub-hosted repo (for example `harness-service`).
2. Query calls `list_github_pull_requests`.
3. Rows show `#N`, Open or Draft, login + avatar, branches, `commentCount`.
4. Selecting `#N` shows body from the list payload and fetches conversation comments.
5. Create calls `create_github_pull_request`, refetches the list, selects the new number. If that number is not on the open page (full page, sort dropped it), clear selection and stay on the list.
6. Copy link writes `https://github.com/<owner>/<repo>/pull/<N>`.

Buzz-hosted tabs never call these commands.

## UI

Gate on `isGitHubCloneUrl` for the selected repository. Global `ProjectsPullRequestsList` is unchanged.

**List**

- Status: Open (green) or Draft (muted). `#` is the GitHub number, not eight hex chars.
- Author: login + `authorAvatarUrl`. Do not use `ProfileIdentityButton`.
- Branches: `owner:branch → base` when head repo differs; otherwise `branch → base`.
- Conversation count: `commentCount`.
- If `hasMore`, one muted line: more open pull requests exist on GitHub.

**Detail**

- Title + `#N`, body via `ProjectRichContent` with `tags = []`.
- **Hide** `DiscussedInChannels`, `ProjectOriginReference`, reviewers row, `MergePullRequestButton`, `PullRequestReviewCard`, `ForumComposer`, Request changes.
- Comment authors are logins + avatars.
- While comments load: “Loading comments…”.
- If comments fail: keep the body; retry on the comment section only.
- Conversation badge uses `commentCount` from the list until comments load, then `max(commentCount, comments.length)`.
- Commits: one head-SHA row. Author login, not a pubkey profile button.
- Checks: existing empty placeholder.
- **Hide Files changed** in `PullRequestTabsList`. Do not mount `ProjectPullRequestFilesChangedPanel`. If `selectedTab === "pr-files"`, set `pr-conversation`.

**Empty / error**

- Loading: “Loading pull requests…”.
- Empty success: “No open pull requests.”
- GitHub error: recovery UI with the same codes as G1/G2 (`GitHubRepoStateRecovery` or the same card with PR titles: “Could not load GitHub pull requests”). Check error **before** the empty-success branch. Do not show “No pull requests yet.” or “Could not load pull requests for this repository.” on a GitHub auth/CLI failure.
- Buzz errors stay on the current generic line. Do not use GitHub-only copy unless `isGitHubCloneUrl`.

**Create dialog**

- Same title + body + repository + base/compare fields. No reviewer or draft controls.
- Source commit still required from GitHub repo state (G1/G2). If GitHub state is unresolved, keep the dialog blocked the way it is today.
- Success toast: “Pull request created.”
- Failure: parsed GitHub error message. Do not clear title/body.

## Error handling

Do not invent an empty PR list or a fake create success on auth or CLI failure.

| Situation | Code | UI |
|---|---|---|
| `gh` not on PATH | `github_cli_missing` | Install GitHub CLI; Retry |
| `gh auth status` fails | `github_auth_required` | `gh auth login --hostname github.com` plus copy; Retry |
| Repo HTTP 404, or 403 that is not a rate/abuse limit | `github_repo_unavailable` | `gh` message; Retry |
| PR HTTP 404 on comments | `github_pr_unavailable` | “Pull request not found.” Clear selection. Keep the list. |
| Rate/abuse 403, timeout, truncated JSON, other `gh` failure | `github_pulls_failed` | Retry; project page stays up |

Comment-load HTTP 404 is `github_pr_unavailable` and clears selection. Other comment-load failures use `github_pulls_failed` (or the same auth codes if `gh` is gone mid-session) and do not clear the PR body. Create failures stay in the dialog; they never clear the list selection.

Invalid clone URL fails `GitHubRepoRef::parse` before `gh`. `number == 0` or a non-decimal selected id never calls `gh`. Rejected `html_url` drops that row or comment; it is not a tab-level error.

Buzz does not invent an ACL. A 403 that is not rate/abuse is `github_repo_unavailable`.

## Testing

No live GitHub in unit tests. Inject `GhRunner` fixtures like merge and G1/G2.

**Rust**

- Fixture list maps to `#42` / Open; `draft: true` maps Draft.
- Inbound fork keeps `head.repo.full_name`; maps `head.sha`, `head.ref`, `base.ref`.
- Raw page of 100 items → `has_more: true`; shorter page → `false`.
- `html_url` that is not `https://github.com/{owner}/{repo}/pull/{number}` for that repo is rejected.
- Create sends `{ title, body, head, base }`; empty title or `head == base` fails before `gh`; returns `number`.
- Comment `html_url` that is not the issues or pull `#issuecomment-{id}` form for that repo and number is dropped.
- Buzz `/git/...` URL fails `GitHubRepoRef::parse` before `gh`.
- Missing binary → `github_cli_missing`; failed auth → `github_auth_required`; repo 404 → `github_repo_unavailable`; PR 404 → `github_pr_unavailable`.

**Desktop JS**

- GitHub clone URL does not `fetchEvents` for kind:1618 / 1613 / 163x / kind:1 on the PR tab.
- Buzz clone URL does not invoke the three GitHub commands.
- Mapper: `#42`, Draft vs Open, login authors, `commentCount`, validated `htmlUrl`, `owner:branch` when head repo differs.
- `pullRequestShareLink` returns the GitHub URL when `htmlUrl` is set; hex Nostr PRs still build `buzz://pr`.
- Create on a GitHub repo does not call `signRelayEvent` / `publishEvent`.
- `publishProjectPullRequestUpdate` is skipped when the clone URL is GitHub.
- GitHub detail has no Merge, composer, reviewers, or Files changed trigger. Buzz PR chrome still has them.

**e2e**

- Mock bridge stubs `list_github_pull_requests`, `create_github_pull_request`, `list_github_pull_request_comments`.
- One smoke spec on a GitHub-hosted mock repo: tab shows `#N` and Open; open it → body + read-only comments; no Merge / composer / Files changed; create stub returns a number and the row appears.
- Auth stub `github_auth_required` shows recovery, not “No pull requests yet.”
- Existing Buzz-hosted PR specs (`project-pr-review.spec.ts` and merge coverage) stay green.

Existing kind:1618 and work-items tests stay green.

## Success criteria

On a GitHub-hosted project (`harness-service` or equivalent), with `gh` installed and authenticated:

- The Pull Request tab shows GitHub open PRs as `#N` (drafts as Draft).
- Creating a pull request creates a GitHub PR and does not publish kind:1618.
- Opening `#N` shows body and read-only conversation comments.
- Copy link copies `https://github.com/<owner>/<repo>/pull/<N>`.
- Merge, review writes, and Files changed are not offered.
- Without `gh` or without auth, the tab shows merge-style recovery, not an empty Buzz list.

Buzz-hosted repositories still list and create kind:1618 only.

## Follow-up slices (not this spec)

- **P3:** show GitHub `reviewDecision` and required checks in the Checks / review chrome.
- **P4 adapt:** merge the listed GitHub `#N` (no find-or-create, no kind:1631).
- **P5:** review comments and posting comments.
