# GitHub Issues (list + create)

**Status:** Draft — awaiting user review  
**Scope:** Buzz Desktop Projects — I1 + I2 only. When a repository clone URL is `github.com`, the per-repo Issues tab lists and creates GitHub Issues via `gh api`.  
**Plan:** [phase-02-github-issues.md](../../../plans/20260818-1211-github-native-host/phase-02-github-issues.md) slice I1 + I2.

## Summary

The Issues tab is NIP-34 `kind:1621` keyed by repo address. GitHub-hosted repos do not publish those events, so the tab is empty and Create still signs a Buzz issue.

This design makes `fetchProjectIssues` and `useCreateProjectIssueMutation` host-aware: `github.com` uses GitHub Issues as the only backend. Buzz-hosted repos keep kind:1621. The existing Issues tab and create dialog stay. Status, labels, assignees, and comments are read-only on GitHub rows.

## Problem and root cause

`fetchProjectIssues` loads kind:1621, status kinds, kind:1 comments, and assignment notes (`desktop/src/features/projects/hooks.ts`).  
`publishProjectIssue` always signs kind:1621 (`desktop/src/features/projects/issueMutations.ts`).  
`ProjectIssue.id` is a 64-hex event id; authors and assignees are pubkeys; `#` in the row is `id.slice(0, 8)`.

A GitHub clone URL never yields 1621 events. Create would dual-write a Nostr issue that GitHub does not own.

## Goals

- List open GitHub issues as `#N` on the per-repo Issues tab when the clone URL is `github.com`.
- Create an issue on GitHub. Do not publish kind:1621 for that host.
- Show GitHub open/closed, label names, assignee logins + avatars, and comments as **read-only**.
- Copy link copies the GitHub issue URL.
- Surface `github_cli_missing` and `github_auth_required` with the same recovery as merge / G1/G2. Do not show “No issues yet” on auth failure.
- Leave Buzz-hosted issues on kind:1621.

## Non-goals

- Close / reopen (I3).
- Posting comments (I4).
- Adding or removing labels or assignees (I5, I6).
- Closed issues, a second page, or Load more.
- Global Projects Issues list (`fetchProjectsWorkItems`) and card / activity counts (X2 / M4).
- New `buzz://issue` scheme for GitHub numbers.
- CLI (`buzz issues`) and mobile.
- GitHub Enterprise, GitLab, OAuth tokens stored in Buzz.
- Importing GitHub issues into Nostr. No dual-write.
- Milestones, issue types, projects, reactions, timeline events (labeled, referenced).

## Product decisions

- **Host split:** `isGitHubCloneUrl` / `GitHubRepoRef::parse`. GitHub is the only issue source for that host. Do not union with kind:1621.
- **Auth:** installed `gh`, `gh auth status --hostname github.com`. Buzz never reads or stores a GitHub token.
- **List:** `state=open`, first 100 items from one page, `sort=updated`, `direction=desc`. No `--paginate`. Drop pull requests (items with a `pull_request` field).
- **More:** `hasMore` is true when the **raw** GitHub page has 100 items. After PR filter the UI may show fewer. One muted note, not a button.
- **Identity:** authors and assignees are GitHub logins + avatar URLs, not pubkeys. Never pass a login to `ProfileIdentityButton`, `normalizePubkey`, or assignment mutations.
- **Status:** add `"Open"` to `ProjectIssueStatus`. GitHub `open` → `"Open"`, `closed` → `"Closed"`. Do not map open → Backlog. v1 list is open-only.
- **Share:** Copy link uses validated `html_url`. No `buzz://` change.
- **Create:** title + body only. Title required after trim, max 256 characters (same as Buzz). Body may be empty. No labels or assignees on create.
- **Surface:** per-repo Issues tab only. Global create dialog still routes by the **selected repository** host so a GitHub target never signs 1621.
- **Comments:** list carries `commentCount` from GitHub `comments`. Bodies load when a row is opened (first 100). Composer is hidden.

## Architecture

```
github.com clone URL  → list_github_issues / create_github_issue → ProjectIssue
Buzz /git/...         → kind:1621                                 → ProjectIssue
```

Detail comments (GitHub only):

```
selected #N  → list_github_issue_comments → ProjectIssue.comments
```

## Components

### `GitHubRepoRef` (existing)

Parses a plain `https://github.com/<owner>/<repo>[.git]` or `git@github.com:` URL. Rejects other hosts. Invalid clone URL fails before `gh`.

### `project_github_issues.rs` (new)

Reuses `GhRunner`, `ensure_auth`, redaction, and `ProjectPullRequestMergeError`. No provider trait. No `gh issue` subcommands.

| Command | Input | GitHub | Output |
|---|---|---|---|
| `list_github_issues` | `clone_url` | `GET /repos/{owner}/{repo}/issues?state=open&per_page=100&sort=updated&direction=desc` | `{ issues, has_more }` |
| `create_github_issue` | `clone_url`, `title`, `body` | `POST /repos/{owner}/{repo}/issues` via `--input` tempfile | one issue DTO |
| `list_github_issue_comments` | `clone_url`, `number` | `GET /repos/{owner}/{repo}/issues/{number}/comments?per_page=100` | comment list |

`number` is a `u64` greater than 0, never a raw path segment from unparsed text.

**Issue DTO (bounded):** `number`, `title`, `body`, `state`, `html_url`, `comments`, `created_at`, `updated_at`, `user.login`, `user.avatar_url`, `labels[].name`, `assignees[].login`, `assignees[].avatar_url`. Ignore milestone, reactions, issue type, and other fields.

**`html_url` check:** accept only `https://github.com/{owner}/{repo}/issues/{number}` for the parsed repo. Owner and repo compare case-insensitively. Number must match. Reject query, fragment, credentials, or a different host/path so Copy link cannot emit a foreign URL. Drop the issue if `html_url` fails this check.

**List rules:** one page. Drop any item that has a `pull_request` field. `has_more` is `raw_items.len() == 100`.

**Create:** JSON `{ "title", "body" }`. Empty title fails before `gh`. Return the created issue so the tab can select `#N`.

**Comments:** first 100, in GitHub’s order (oldest first). No reply threading. No prefetch for the whole list.

Register the three commands next to `get_github_repository_state` in `desktop/src-tauri/src/lib.rs`.

### `ProjectIssue` (extended)

Keep the type so the tab and detail shell stay. GitHub rows fill:

| Field | GitHub |
|---|---|
| `id` | `String(number)` — selection key (`"42"`) |
| `title` / `content` | title / body (`""` if body is null) |
| `author` / `assignees` | GitHub logins |
| `authorAvatarUrl` | `user.avatar_url` (new, optional; unused on Nostr) |
| `assigneeAvatars` | `Record<login, avatar_url>` (new, optional) |
| `labels` | label names |
| `status` | `"Open"` or `"Closed"` |
| `createdAt` / `updatedAt` | Unix seconds from RFC3339 |
| `comments` | `[]` on the list query |
| `commentCount` | GitHub `comments` (new). Nostr mapper sets `comments.length` |
| `htmlUrl` | validated GitHub issue URL (new). Nostr: `null` |
| `repoAddress` | Buzz announcement coordinate (React keys). Not a GitHub id |
| `channelId`, `originAgentName`, `recipients`, `assigneeOperationHeads`, `statusEventId` | empty / unused |

Add `"Open"` to `ProjectIssueStatus` / `PROJECT_ISSUE_STATUS`.

### `fetchProjectIssues` (existing, routed)

Inject loaders for tests, same idea as `fetchRepoStateWith`:

- `isGitHubCloneUrl(cloneUrl)` → `list_github_issues` → map DTOs
- otherwise → current kind:1621 fetch; `hasMore = false`

`useProjectIssuesQuery` keeps key `["project", id, "issues"]`. Its data shape becomes `{ issues: ProjectIssue[]; hasMore: boolean }`. Callers that treated `data` as an array (`ProjectIssuesPanel`) read `data.issues`. Stale time stays 30s.

### `useCreateProjectIssueMutation` (existing, routed)

- GitHub repo → `create_github_issue` → return `String(number)`
- Buzz repo → `publishProjectIssue` → event id

On GitHub success, invalidate only `["project", id, "issues"]`. Do **not** invalidate `work-items` or `activity-summaries` (those stay Nostr). Buzz create keeps today’s invalidation.

`CreateProjectIssueDialog` already passes the selected repository. `ProjectDetailScreen` `handleCreateIssue` already selects the returned id after refetch.

### Comments query (new)

Enabled when the repo is GitHub and `selectedIssueId` matches `/^[1-9][0-9]*$/`. Parse it as `u64` before invoke. Key: `["project", id, "issues", number, "comments"]`. Invoke `list_github_issue_comments`. Merge into the open issue for `ProjectIssueCommentTimeline`. List fetch does not wait on this query.

### `issueShareLink`

If `issue.htmlUrl` is a validated GitHub issue URL, return it. Otherwise keep the hex `buzz://issue` path. Hide Copy link when both fail (same as today).

## Data flow

1. User opens the Issues tab on a GitHub-hosted repo (for example `harness-service`).
2. Query calls `list_github_issues`.
3. Rows show `#N`, Open, login + avatar, labels, read-only facepile, `commentCount`.
4. Selecting `#N` shows body from the list payload and fetches comments.
5. Create calls `create_github_issue`, refetches the list, selects the new number.
6. Copy link writes `https://github.com/<owner>/<repo>/issues/<N>`.

Buzz-hosted tabs never call these commands.

## UI

Gate on `isGitHubCloneUrl` for the selected repository. Global `ProjectsIssuesList` is unchanged.

**List**

- Status icon: green open dot. Label: **Open**, not Backlog.
- `#` is the GitHub number, not eight hex chars.
- Author: login + `authorAvatarUrl`. Do not use `ProfileIdentityButton`.
- Assignees: read-only facepile from `assigneeAvatars`. Do not reuse `IssueAssigneeFacepile` with logins as pubkeys.
- If `hasMore`, one muted line: more open issues exist on GitHub.

**Detail**

- Title + `#N`, body via `ProjectRichContent` with `tags = []` (GitHub-flavored extras such as issue autolinks are not required). Comment bodies are plain `content` strings.
- **Hide** `DiscussedInChannels`, **hide** `ForumComposer`, **hide** `IssueAssigneesRow` (no assign/unassign).
- Comment authors are logins + avatars.
- While comments load: “Loading comments…”.
- If comments fail: keep the body; retry on the comment section only.

**Empty / error**

- Loading: “Loading issues…”.
- Empty success: “No open issues.”
- GitHub error: recovery UI with the same codes as G1/G2 (`GitHubRepoStateRecovery` or the same codes with issue titles: “Could not load GitHub issues”). Check error **before** the empty-success branch. Do not show “No issues yet” or “Could not load issues for this repository.” on a GitHub auth/CLI failure.
- Buzz errors stay on the current generic line. Do not use GitHub-only copy unless `isGitHubCloneUrl`.

**Create dialog**

- Same title + body fields. No label/assignee controls.
- Success toast: “Issue created.”
- Failure: parsed GitHub error message.

## Error handling

Do not invent an empty issue list on auth or CLI failure.

| Situation | Code | UI |
|---|---|---|
| `gh` not on PATH | `github_cli_missing` | Install GitHub CLI; Retry |
| `gh auth status` fails | `github_auth_required` | `gh auth login --hostname github.com` plus copy; Retry |
| Repo HTTP 404, or 403 that is not a rate/abuse limit | `github_repo_unavailable` | `gh` message; Retry |
| Rate/abuse 403, timeout, truncated JSON, other `gh` failure | `github_issues_failed` | Retry; project page stays up |

Comment-load failures use `github_issues_failed` (or the same auth codes if `gh` is gone mid-session) and do not clear the issue body.

## Testing

No live GitHub in unit tests. Inject `GhRunner` fixtures like merge and G1/G2.

**Rust**

- Fixture list maps to `#42` / Open; items with `pull_request` are dropped.
- Raw page of 100 items → `has_more: true`; shorter page → `false`.
- `html_url` that is not `https://github.com/{owner}/{repo}/issues/{number}` for that repo is rejected.
- Create sends title + body and returns `number`; empty title fails before `gh`.
- Buzz `/git/...` URL fails `GitHubRepoRef::parse` before `gh`.
- Missing binary → `github_cli_missing`; failed auth → `github_auth_required`; 404 → `github_repo_unavailable`.

**Desktop JS**

- GitHub clone URL does not `fetchEvents` for kind:1621.
- Buzz clone URL does not invoke `list_github_issues`.
- Mapper: `#42`, status `"Open"`, login authors, `commentCount`, validated `htmlUrl`.
- `issueShareLink` returns the GitHub URL when `htmlUrl` is set; hex Nostr issues still build `buzz://issue`.
- Create on a GitHub repo does not call `signRelayEvent` / `publishEvent`.

**e2e**

- Mock bridge stubs `list_github_issues`, `create_github_issue`, `list_github_issue_comments`.
- One smoke spec: Issues tab on a GitHub-hosted mock repo shows `#N` and Open; create stub returns a number and the row appears; comment composer is absent.
- Auth stub `github_auth_required` shows recovery, not “No issues yet.”

Existing kind:1621 and work-items tests stay green.

## Success criteria

On a GitHub-hosted project (`harness-service` or equivalent), with `gh` installed and authenticated:

- The Issues tab shows GitHub open issues as `#N`.
- Creating an issue creates a GitHub issue and does not publish kind:1621.
- Copy link copies `https://github.com/<owner>/<repo>/issues/<N>`.
- Without `gh` or without auth, the tab shows merge-style recovery, not an empty Buzz list.

Buzz-hosted repositories still list and create kind:1621 only.
