# GitHub pull request checks and reviews

**Status:** Draft — awaiting user review  
**Scope:** Buzz Desktop Projects — review and required-check chrome on a listed GitHub `#N`. Depends on [list + create](./2026-08-18-github-pull-requests-design.md).  
**Plan:** [phase-03-github-pull-requests.md](../../../plans/20260818-1211-github-native-host/phase-03-github-pull-requests.md) review / checks slice.

## Summary

List + create makes GitHub the only PR backend for `github.com` clone URLs and hides Merge, review writes, and Files changed. Merge already reads `reviewDecision` and required checks, but only when the user merges a Nostr kind:1618 row. The Checks tab is a static empty placeholder.

This slice loads those gates for a listed `#N` and submits Approve / Request changes as GitHub pull-request reviews. The Buzz npub is not the GitHub actor. Conversation issue comments stay read-only. Merge, close, draft, reopen, request-review, and Files changed stay hidden.

## Problem

`gh pr view` and `gh pr checks --required` already run inside `project_github_pull_request.rs` for merge. The PR tab never calls them. `PullRequestReviewCard` and `ForumComposer` publish kind:1 review notes against a 64-hex event id. A GitHub row id is `"42"`; a GitHub login is not a pubkey. Remounting that card would dual-write Nostr and call merge find-or-create.

## Goals

- On a listed GitHub `#N`, the Checks tab shows `reviewDecision` and every **required** check (name, pass/fail/pending, link).
- Approve and Request changes post `POST /repos/{owner}/{repo}/pulls/{number}/reviews` as the `gh` user. Do not publish kind:1.
- Pin the review to gates `head_sha` (`headRefOid`). If GitHub’s head moved, refuse the write.
- Reuse merge’s `gh pr view` / `gh pr checks --required` readers. Do not call `merge_github_pull_request`.
- Missing `gh` or auth uses the same recovery as merge / list. Do not show the empty-checks placeholder on auth failure.

## Non-goals

- Merge-by-number (later). Hide `MergePullRequestButton`.
- Close, reopen, convert to draft, mark ready.
- Request review of other people. Hide `PullRequestReviewersRow`.
- Posting issue comments or review comments / inline comments (later). `ForumComposer` stays hidden.
- Files changed. Stay hidden per list + create.
- All check runs (only `--required`). No Actions jobs, annotations, or logs.
- Conversation-header readiness summary. No list-row check badges.
- `gh pr review`. GraphQL. GitHub Enterprise. OAuth token in Buzz.
- Dual-write to Nostr. Global Projects PR list. CLI. Mobile.

## Product decisions

- **Host split:** `isGitHubCloneUrl` / `GitHubRepoRef::parse`. Buzz-hosted tabs never call these commands.
- **Auth:** installed `gh`, `gh auth status --hostname github.com`. Buzz never stores a GitHub token.
- **Who may review:** GitHub decides. Show Approve / Request changes on Open and Draft when gates loaded a non-empty `head_sha`. Do not use `canReviewProjectPullRequest` (npub owner/reviewer). Do not hide buttons after a 422.
- **Approve:** existing dialog; body optional after trim. Event `APPROVE`.
- **Request changes:** a dialog on the same actions row (composer stays hidden). Body required after trim. Event `REQUEST_CHANGES`.
- **Commit pin:** `commit_id` is gates `head_sha`. Before POST, re-read the PR. If `headRefOid` differs (case-insensitive hex), return `github_pr_head_changed` and do not POST.
- **Checks:** one page from `gh pr checks <n> --required --json name,state,bucket,link`. Exit code `8` is pending, not failure. “No required checks reported” is an empty list, not an error.
- **`reviewDecision` display (desktop):** `APPROVED` → “Approved”; `CHANGES_REQUESTED` → “Changes requested”; `REVIEW_REQUIRED` → “Review required”; empty/missing → “No review required”; any other token → “Review state is {token}”. Rust returns the raw GitHub string.
- **Check links:** keep only `https` URLs with no userinfo. Drop others; the row still shows. Open the link in the system browser.
- **After a successful review:** invalidate gates only. The review body is not an issue comment and must not appear in the conversation list.
- **Card:** new thin GitHub actions row. Do not remount `PullRequestReviewCard` (it owns Merge and Nostr status).

## Architecture

```
selected #N (github.com)
  → get_github_pull_request_gates     → Checks tab + head_sha
  → submit_github_pull_request_review → POST reviews (pinned commit)
```

```
Buzz /git/...  → kind:1618 / kind:1630 / kind:1  (unchanged)
```

Commands live in `desktop/src-tauri/src/commands/project_github_pulls.rs` (same module as list + create). Do not add them to `project_github_pull_request.rs`.

Extract `load_pull_view` and `load_required_checks` (and `GitHubPullView`, `GitHubCheck`, `is_no_required_checks_stderr`) as `pub(crate)` from the merge module. This module and merge both call them. Do not call `decide_open_pull`, `ensure_pull`, or `merge_github_pull_request`.

No provider trait. No `gh pr review`.

## Components

### `get_github_pull_request_gates`

Input: `clone_url`, `number` (`u64` > 0).  
Calls: `gh pr view` (same `--json` as merge) and `gh pr checks --required` (same fields as merge).  
Output:

| Field | Source |
|---|---|
| `number` | view |
| `head_sha` | `headRefOid` |
| `review_decision` | `reviewDecision` (string, may be empty) |
| `checks` | required-check rows |

Check row: `name`, `state`, `bucket`, `link` (`null` if dropped). Ignore unknown JSON fields.

Invalid clone URL or `number == 0` fails before `gh`.

### `submit_github_pull_request_review`

Input: `clone_url`, `number`, `event` (`"APPROVE"` \| `"REQUEST_CHANGES"`), `body`, `commit_id`.  
Fails before `gh` when:

- `event` is anything else
- `commit_id` is not 40-char hex
- `event == "REQUEST_CHANGES"` and `body.trim()` is empty

Then `ensure_auth`, re-read the PR, compare `headRefOid` to `commit_id`. On mismatch: `github_pr_head_changed`.

Then `POST /repos/{owner}/{repo}/pulls/{number}/reviews` via `gh api --input` tempfile:

```json
{ "commit_id": "<sha>", "body": "<trimmed or empty>", "event": "APPROVE" }
```

Return `{ submitted: true }`. Do not return GitHub’s review id to the UI.

### Desktop query / mutation

- Gates query enabled when the repo is GitHub and `selectedPullRequestId` matches `/^[1-9][0-9]*$/`. Key: `["project", id, "pull-requests", number, "gates"]`.
- Mutation calls `submit_github_pull_request_review` with `event`, trimmed `body`, and `commit_id` from the last successful gates payload. On success, invalidate that gates key only. Do not invalidate `work-items` or `activity-summaries`. Do not call `signRelayEvent`.

### `GitHubPullRequestReviewActions`

Approve (optional summary dialog) and Request changes (required-body dialog). No Merge, no overflow Close/Draft/Reopen, no Request review. Mount only on GitHub rows when gates succeeded with a non-empty `head_sha`.

### Checks tab

When `isGitHubCloneUrl`:

1. Review line from `review_decision`.
2. One row per required check: name, bucket/state, link control if `link` is present.
3. Zero checks after a successful load: “No required checks on this pull request.”

Do not show “No checks have been reported for this pull request yet.” on a GitHub-hosted repo.

Buzz-hosted Checks tab stays that placeholder.

## Data flow

1. User opens `#N` on a GitHub-hosted repo.
2. Comments query (list + create) and gates query run in parallel.
3. Checks tab reads gates only.
4. Approve / Request changes POST the review, then refetch gates. Conversation comments do not change.
5. Copy link, list, and create stay as in the list + create spec.

## UI

**Checks**

- Loading: “Loading checks…”.
- Auth/CLI/repo errors: same recovery card as list (`GitHubRepoStateRecovery` or the same codes). Retry refetches gates only. Keep the PR body and conversation.
- Other gates failures: retry on the Checks tab only; code `github_pulls_failed`.

**Conversation**

- GitHub actions row as above.
- Hide `MergePullRequestButton`, `PullRequestReviewCard`, `PullRequestReviewersRow`, `ForumComposer`, Request changes on the composer, Files changed.

**Toasts**

- Approve success: “Pull request approved.”
- Request-changes success: “Changes requested.”
- Failure: parsed GitHub / structured message. Do not clear the dialog body.

## Error handling

Do not invent passing checks or a successful review on auth or CLI failure.

| Situation | Code | UI |
|---|---|---|
| `gh` not on PATH | `github_cli_missing` | Install GitHub CLI; Retry |
| `gh auth status` fails | `github_auth_required` | `gh auth login --hostname github.com`; Retry |
| Repo HTTP 404, or 403 that is not rate/abuse | `github_repo_unavailable` | `gh` message; Retry |
| PR HTTP 404 | `github_pr_unavailable` | “Pull request not found.” Clear selection. Keep the list. |
| `headRefOid` ≠ `commit_id` | `github_pr_head_changed` | “This pull request’s head commit changed. Refresh and review again.” Refetch gates. Keep dialog text. |
| Review HTTP 422 (self-review, no permission, pending review) | `github_review_rejected` | Toast GitHub message. Keep buttons. |
| Rate/abuse 403, timeout, truncated JSON, other `gh` failure | `github_pulls_failed` | Retry; project page stays up |

Comment-load errors stay as in list + create. A gates 404 clears selection the same way as a comment 404.

## Testing

No live GitHub in unit tests. Inject `GhRunner` fixtures like merge.

**Rust**

- `APPROVED` + one required check with `bucket != pass` returns raw `review_decision: "APPROVED"` and that row.
- Empty `reviewDecision` returns `review_decision: ""`.
- Unknown token is returned unchanged.
- “No required checks reported” → `checks: []`.
- Exit code 8 with JSON → pending rows, not an error.
- Non-https or credentialed `link` is dropped (`link: null`).
- Empty `REQUEST_CHANGES` body fails before `gh`.
- `commit_id` not 40-hex fails before `gh`.
- Re-read head ≠ `commit_id` → `github_pr_head_changed`, no POST.
- POST body is `{ commit_id, body, event }`.
- 422 → `github_review_rejected`.
- Buzz `/git/...` URL fails `GitHubRepoRef::parse` before `gh`.

**Desktop JS**

- GitHub Approve / Request changes does not call `signRelayEvent` / `publishEvent`.
- Buzz-hosted PR does not invoke `get_github_pull_request_gates` or `submit_github_pull_request_review`.
- GitHub Checks tab maps `APPROVED` / empty / unknown tokens to the display lines above and shows required rows, not the empty placeholder.
- GitHub detail has no Merge, Close/Draft overflow, composer, or Files changed.
- Review buttons are absent until gates succeed with `head_sha`.
- Buzz review card and composer still mount on kind:1618 rows.

**e2e**

- Mock `get_github_pull_request_gates` and `submit_github_pull_request_review`.
- Open `#N` → Checks shows review line + a required check; Approve stub succeeds; no Merge / composer / Files changed.
- Auth stub `github_auth_required` on gates shows recovery, not the empty placeholder.
- Existing Buzz review specs stay green.

## Success criteria

On a GitHub-hosted project, with `gh` installed and authenticated:

- Checks tab shows GitHub `reviewDecision` and required checks for `#N`.
- Approve / Request changes create a GitHub review on the shown head SHA and do not publish Nostr events.
- A moved head refuses the write and refreshes gates.
- Merge, close/draft, request-review, composer, and Files changed are not offered.
- Without `gh` or without auth, Checks shows merge-style recovery.

Buzz-hosted repositories still review with kind:1 labeled notes only.

## Follow-up (not this spec)

- Merge the listed GitHub `#N` (no find-or-create, no kind:1631).
- Review comments, inline comments, and posting issue comments.
- Optional: conversation-header summary; list-row check badges; all-check view.
