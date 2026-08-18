# GitHub issue writes (close, comment, labels, assignees)

**Status:** Draft — awaiting user review  
**Scope:** Buzz Desktop Projects — I3 + I4 + I5 + I6. When a repository clone URL is `github.com`, the per-repo Issues tab closes and reopens GitHub issues, posts comments, and adds or removes labels and assignees via `gh api`.  
**Plan:** [phase-02-github-issues.md](../../../plans/20260818-1211-github-native-host/phase-02-github-issues.md) slice I3–I6.  
**Depends on:** [I1+I2 list + create](./2026-08-18-github-issues-design.md). This spec amends that document’s open-only list, hidden composer, and read-only labels/assignees.

## Summary

I1+I2 lists and creates GitHub issues and shows status, labels, assignees, and comments as read-only. Close, comment, and metadata writes still no-op or still sign Nostr events if the host gate is missing.

This design extends `project_github_issues.rs` with focused write commands. The Issues tab gains an Open | Closed filter so reopen has a target. Write chrome is GitHub-native and thin: Close / Reopen, a body-only composer, label chips from the repo catalog, and a login assignee picker. Buzz-hosted repos keep kind:1632 / kind:1 / assignment notes.

## Problem and root cause

After I1+I2, `fetchProjectIssues` can return GitHub rows, but:

- The list is `state=open` only, so a closed issue has no row to reopen.
- `ProjectIssueDetail` hides `ForumComposer` and `IssueAssigneesRow` on GitHub.
- Status and labels are badges. There is no close control even on Buzz issues.
- `useCreateProjectIssueCommentMutation` and `writeProjectIssueAssignment` always sign kind:1. `IssueAssigneesRow` searches community pubkeys.

A GitHub login is not a pubkey. Passing it to `ProfileIdentityButton`, `normalizePubkey`, or assignment mutations is wrong.

## Goals

- Close and reopen a GitHub issue (`state` only) from the per-repo Issues tab.
- List closed issues through an Open | Closed filter so reopen has a target.
- Post a GitHub issue comment as a body string. Do not publish kind:1.
- Add and remove labels from the repo label catalog.
- Add and remove assignees from the repo assignee catalog. Assign me uses `GET /user`.
- Surface the same `gh` recovery codes as I1+I2 / G1/G2 / merge.
- Leave Buzz-hosted close, comment, label, and assign paths unchanged.

## Non-goals

- Comment edit or delete.
- Close `state_reason` (`completed`, `not_planned`).
- Title or body edit after create.
- `state=all`, a second page, or Load more.
- Global Projects Issues list and card / activity counts (X2 / M4).
- New `buzz://issue` scheme for GitHub numbers.
- CLI (`buzz issues`) and mobile.
- GitHub Enterprise, GitLab, OAuth tokens stored in Buzz.
- Importing GitHub issues into Nostr. No dual-write.
- Buzz Triage / Backlog / In Progress / In Review / Done on GitHub rows.
- Milestones, issue types, projects, reactions, timeline events.

## Product decisions

- **Host split:** `isGitHubCloneUrl` / `GitHubRepoRef::parse`. GitHub is the only write backend for that host.
- **Auth:** installed `gh`, `gh auth status --hostname github.com`. Buzz never reads or stores a GitHub token. The Buzz npub is not a GitHub actor.
- **Filter:** Open | Closed on GitHub tabs only. Local tab state. Default Open. Not a URL param. Not persisted. Buzz tabs have no filter.
- **List amendment:** `list_github_issues` requires `state: "open" | "closed"`. Same page size (100), `sort=updated`, `direction=desc`, drop `pull_request`, `has_more` from raw page length. I1+I2 should accept `state` even if its first UI only sends `open`.
- **After close:** set filter to Closed and keep `#N` selected. After reopen: set filter to Open and keep `#N`. After I1+I2 create: set filter to Open and select the new `#N` (a new issue is open). If the new page does not contain `#N` (full page, sort dropped it), clear selection and stay on that filter.
- **Close / Reopen:** `PATCH` `{ "state": "closed" | "open" }` only. No Buzz status vocabulary.
- **Comment:** trimmed body, required. No npub mentions. No Blossom media. No reply threading.
- **Labels:** add/remove one name per action from `GET /repos/{owner}/{repo}/labels`. Do not replace the whole set.
- **Assignees:** add/remove one login per action from `GET /repos/{owner}/{repo}/assignees`. Assign me uses `GET /user`. Do not replace the whole set.
- **Identity:** authors and assignees stay GitHub logins + avatar URLs. Never pass a login to `ProfileIdentityButton`, `normalizePubkey`, or Nostr assignment mutations.
- **Invalidation:** GitHub writes invalidate the prefix `["project", id, "issues"]`, which covers both list states and `["project", id, "issues", number, "comments"]`. Do not invalidate `work-items` or `activity-summaries`.
- **Surface:** per-repo Issues tab only.

## Architecture

```
Open|Closed  → list_github_issues(state)     → ProjectIssue[]
Close/Reopen → update_github_issue_state     → issue DTO
Comment      → create_github_issue_comment   → comment DTO
Labels       → list + add + remove
Assignees    → list + add + remove
Assign me    → GET /user login
```

```
github.com clone URL  → gh api issue writes     → ProjectIssue
Buzz /git/...         → kind:1632 / kind:1      → ProjectIssue
```

No provider trait. No `gh issue` subcommands.

## Components

### `project_github_issues.rs` (I1+I2 module, extended)

Reuses `GhRunner`, `ensure_auth`, redaction, and `ProjectPullRequestMergeError`. `number` is a `u64` greater than 0.

| Command | Input | GitHub | Output |
|---|---|---|---|
| `list_github_issues` | `clone_url`, `state` | `GET /repos/{owner}/{repo}/issues?state={open\|closed}&per_page=100&sort=updated&direction=desc` | `{ issues, has_more }` |
| `update_github_issue_state` | `clone_url`, `number`, `state` | `PATCH /repos/{owner}/{repo}/issues/{number}` `{ "state" }` | issue DTO |
| `create_github_issue_comment` | `clone_url`, `number`, `body` | `POST /repos/{owner}/{repo}/issues/{number}/comments` `{ "body" }` via `--input` tempfile | comment DTO |
| `list_github_repo_labels` | `clone_url` | `GET /repos/{owner}/{repo}/labels?per_page=100` | `{ name, color }[]` |
| `add_github_issue_labels` | `clone_url`, `number`, `name` | `POST /repos/{owner}/{repo}/issues/{number}/labels` `{ "labels": [name] }` | issue DTO |
| `remove_github_issue_label` | `clone_url`, `number`, `name` | `DELETE /repos/{owner}/{repo}/issues/{number}/labels/{name}` | issue DTO |
| `list_github_repo_assignees` | `clone_url` | `GET /repos/{owner}/{repo}/assignees?per_page=100` | `{ login, avatar_url }[]` |
| `add_github_issue_assignees` | `clone_url`, `number`, `login` | `POST /repos/{owner}/{repo}/issues/{number}/assignees` `{ "assignees": [login] }` | issue DTO |
| `remove_github_issue_assignee` | `clone_url`, `number`, `login` | `DELETE /repos/{owner}/{repo}/issues/{number}/assignees` `{ "assignees": [login] }` | issue DTO |
| `get_github_authenticated_user` | (none) | `GET /user` | `{ login, avatar_url }` |

Unknown `state` fails before `gh`. Empty comment body, empty label name, or empty login fails before `gh`.

**Path safety:** do not interpolate unparsed text into URLs. Percent-encode label `name` on DELETE (spaces, `#`, unicode). Send logins only in JSON bodies.

**Issue DTO:** same bounded shape as I1+I2, including validated `html_url`.

**Comment DTO:** `id`, `body`, `html_url`, `created_at`, `user.login`, `user.avatar_url`. Accept `html_url` only when it is `https://github.com/{owner}/{repo}/issues/{number}#issuecomment-{id}` for that repo and number. Drop the comment if the URL fails this check.

**Catalogs:** one page of 100. No pagination. Label `color` is a 6-hex string without `#`. Ignore description, default, and other fields.

Register the new commands next to the I1+I2 commands in `desktop/src-tauri/src/lib.rs`.

### Desktop routing

`useProjectIssuesQuery` key becomes `["project", id, "issues", state]` on GitHub (`state` is `"open"` or `"closed"`). Buzz stays `["project", id, "issues"]` and unfiltered.

Host-route mutations on `isGitHubCloneUrl(cloneUrl)`:

| Action | GitHub | Buzz |
|---|---|---|
| Close / reopen | `update_github_issue_state` | unchanged (no control today; do not add one) |
| Comment | `create_github_issue_comment` | `createProjectIssueComment` (kind:1) |
| Labels | add/remove commands | unchanged (display-only) |
| Assignees | add/remove commands | `writeProjectIssueAssignment` |

Parse `selectedIssueId` with `/^[1-9][0-9]*$/` and `u64` before every GitHub write.

### UI files

`ProjectIssuesPanel.tsx` is already large. Put GitHub-only chrome in sibling files under `desktop/src/features/projects/ui/`:

- Open | Closed control
- Close / Reopen button
- GitHub label chips + catalog
- GitHub assignee facepile + catalog + Assign me

Do not mount `IssueAssigneesRow` or `ProfileIdentityButton` with a login.

**List header (GitHub):** Open | Closed segmented control. Empty Open: “No open issues.” Empty Closed: “No closed issues.” `hasMore` muted line names the current filter (“More open issues exist on GitHub.” / “More closed issues exist on GitHub.”).

**Detail header (GitHub):** Close when `status === "Open"`. Reopen when `status === "Closed"`.

**Composer (GitHub):** shown. Body only. If `ForumComposer` cannot hide the mention picker and media controls, use a textarea. Success toast: “Comment posted.”

**Labels (GitHub):** chips from `issue.labels`. Add from catalog (`["project", id, "github-labels"]`, stale 60s). Remove on chip. Hide the add control when the catalog is empty. Keep chips that are already on the issue.

**Assignees (GitHub):** facepile from `assigneeAvatars`. Picker from `["project", id, "github-assignees"]`. Assign me / Unassign me from `["github", "authenticated-user"]` (`get_github_authenticated_user`, session-stable). If that login is already an assignee, show Unassign me.

Buzz detail chrome is unchanged: `ForumComposer` keeps mentions and media; `IssueAssigneesRow` stays.

## Data flow

1. User opens the Issues tab on a GitHub-hosted repo. Filter is Open. Query `["project", id, "issues", "open"]` calls `list_github_issues` with `state=open`.
2. Switching to Closed uses `["project", id, "issues", "closed"]`. The other page may stay cached. Selection clears if `#N` is not in the new page.
3. Selecting `#N` shows the list body and loads `list_github_issue_comments` (I1+I2).
4. Close calls `update_github_issue_state(..., "closed")`. On success, set filter to Closed, keep `selectedIssueId`, invalidate the issues prefix.
5. Reopen does the same with `"open"`, then switches the filter to Open.
6. Comment calls `create_github_issue_comment`. Invalidate the issues prefix. Clear the composer. Do not send `mentionPubkeys` or `mediaTags`.
7. Create (I1+I2) on a GitHub tab sets the filter to Open, then selects the new `#N`.
8. Label add/remove invalidates the issues prefix. Do not refetch the catalog after success unless GitHub returned 404 for an unknown name.
9. Assignee add/remove / Assign me invalidates the issues prefix. Assign me never uses the Buzz pubkey.
10. Buzz-hosted tabs never call these commands and never mount the filter or GitHub pickers.

Copy link stays on validated `html_url` from I1+I2.

## Error handling

Do not invent an empty list or a fake success on auth or CLI failure. Failed writes do not switch the Open | Closed filter.

| Situation | Code | UI |
|---|---|---|
| `gh` not on PATH | `github_cli_missing` | Install GitHub CLI; Retry |
| `gh auth status` fails | `github_auth_required` | `gh auth login --hostname github.com`; Retry |
| Repo HTTP 404, or 403 that is not rate/abuse | `github_repo_unavailable` | `gh` message; Retry |
| Issue HTTP 404 | `github_issue_unavailable` | “Issue not found.” Clear selection. Keep the list. |
| Rate/abuse 403, timeout, truncated JSON, other `gh` failure | `github_issues_failed` | Retry; project page stays up |

**List / filter.** Check the error before the empty-success branch. Auth/CLI failure uses `GitHubRepoStateRecovery` (or the same codes with “Could not load GitHub issues”). Do not show “No open issues.” or “No closed issues.” on those codes.

**Close / Reopen / comment / label / assignee.** Stay on the open issue. Toast the parsed `gh` message. Do not clear composer text on comment failure.

**Assign me.** If `GET /user` fails, disable Assign me and show the same recovery codes. Do not guess a login from git or Buzz identity.

**Catalogs.** Picker failure disables add. Chips already on the issue stay. Remove from those chips still works.

**Empty input.** Empty comment, label, or login fails in TypeScript before `invoke`. Unknown `state` or `number == 0` fails in Rust before `gh`.

**Permissions.** A 403 that is not rate/abuse is `github_repo_unavailable`. Buzz does not invent an ACL.

**Buzz tabs.** Unchanged generic errors. No GitHub copy unless `isGitHubCloneUrl`.

## Testing

No live GitHub in unit tests. Inject `GhRunner` fixtures like I1+I2, merge, and G1/G2.

**Rust**

- `state=closed` is sent on the list query; unknown `state` fails before `gh`.
- Close / reopen PATCH body is `{ "state": "closed" | "open" }` and returns the mapped DTO.
- Empty comment body fails before `gh`; non-empty POST returns the comment DTO.
- Comment `html_url` that is not `https://github.com/{owner}/{repo}/issues/{number}#issuecomment-{id}` is rejected.
- Label DELETE percent-encodes names that contain spaces or `#`.
- Assignee add/remove send JSON logins, never a path segment.
- `GET /user` maps `login` + `avatar_url`.
- `number == 0` and a Buzz `/git/...` URL fail validation before `gh`.
- Missing binary → `github_cli_missing`; failed auth → `github_auth_required`; issue 404 → `github_issue_unavailable`; repo 404 → `github_repo_unavailable`.

**Desktop JS**

- GitHub clone URL does not `signRelayEvent` / `publishEvent` on close, comment, label, or assignee.
- Buzz clone URL does not invoke the GitHub write commands.
- Query key includes `state`; switching Open → Closed does not call `state=open`.
- After a successful close mock, filter becomes Closed and `#N` stays selected.
- Failed close does not change the filter.
- Assign me uses the mocked `/user` login, not the Buzz pubkey.
- GitHub composer has no mention picker and no media control. Buzz composer still has both.

**e2e**

- Mock bridge stubs the write commands plus I1+I2 list/create/comments.
- One smoke spec on a GitHub-hosted mock repo: Open list → close `#N` → Closed list shows it → comment posts → add/remove a label and an assignee → reopen returns it to Open. Composer has no mention picker.
- Auth stub `github_auth_required` shows recovery, not an empty list.
- Update the I1+I2 smoke assertion that the comment composer is absent; this slice shows the body composer.
- Buzz-hosted issue comment and assign specs stay green.

Existing kind:1621 and work-items tests stay green.

## Success criteria

On a GitHub-hosted project (`harness-service` or equivalent), with `gh` installed and authenticated:

- Open | Closed lists GitHub issues as `#N` for that state.
- Close moves `#N` to the Closed filter and keeps it selected.
- Reopen moves `#N` back to Open.
- A comment created in Buzz appears on the GitHub issue. No kind:1 is published.
- Adding or removing a label or assignee changes the GitHub issue. Assignees are logins, not pubkeys.
- Assign me uses the `gh` authenticated user.
- Without `gh` or without auth, the tab shows merge-style recovery, not an empty Buzz list.

Buzz-hosted repositories still list, create, comment, and assign through Nostr only.
