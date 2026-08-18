# GitHub-Backed Pull Request Merge Design

**Status:** Approved in the originating Buzz thread on 2026-08-05.
**Scope:** Buzz Desktop Projects pull-request merge flow for repositories hosted on `github.com`.

## Summary

Buzz Desktop currently presents a Merge action for a Projects pull request whose repository clone URL points to GitHub, but the Tauri command rejects that URL before attempting the merge.
This design adds a GitHub-specific path that finds or creates a real GitHub pull request, respects GitHub checks, reviews, branch rules, and merge permissions, and publishes the Buzz merged-status event only after GitHub confirms the merge.
The existing merge path for repositories hosted by a Buzz relay remains unchanged.

## Problem and Root Cause

`MergePullRequestButton` calls `useMergeProjectPullRequestMutation`, which sends both clone URLs, both branch names, the expected source commit, and Buzz event metadata to `merge_project_pull_request`.
The Tauri command currently passes both clone URLs through `validate_workspace_clone_url`, which only accepts the configured Buzz workspace git origin.
It then compares the owner embedded in the target clone URL with `target_owner`, even though `target_owner` is the Nostr pubkey authorized to sign the Buzz merged-status event rather than a GitHub account name.
The command therefore rejects a URL such as `https://github.com/anhle128/buzz` before its native clone, fetch, merge, and push flow can run.

The fix must separate repository-host routing from Buzz signing authorization.
The Nostr owner remains authoritative for permission to operate on the Buzz pull-request event, while GitHub remains authoritative for the repository merge.

## Goals

- Let the existing Merge action operate on pull requests whose target and source clone URLs are strict `https://github.com/<owner>/<repo>[.git]` URLs.
- Find an existing GitHub pull request for the exact target repository, base branch, source repository, and head branch, or create one when none exists.
- Refuse to merge when the GitHub head commit differs from the commit the user reviewed in Buzz.
- Respect required checks, reviews, repository rules, branch protection, merge permissions, and GitHub's final merge decision.
- Never enable GitHub auto-merge, enter a merge queue, use administrator bypass, or fall back to a direct git push.
- Publish Nostr kind `1631` only after GitHub reports the pull request as merged and supplies the merge commit.
- Make retries idempotent across PR creation, GitHub merge, and Nostr status publication.
- Preserve the current Buzz-hosted repository behavior and merge-conflict recovery.

## Non-Goals

- GitHub OAuth, device flow, token storage, or token refresh inside Buzz.
- GitHub Enterprise Server, GitLab, Bitbucket, or arbitrary git hosts.
- Selecting merge, squash, or rebase strategy in the UI.
- Enabling auto-merge or integrating with GitHub merge queues in this version.
- Replacing GitHub CLI with a direct HTTP client.
- Changing relay schemas or Nostr event kinds for Buzz-hosted repositories.

GitHub-hosted repositories are a native host. Listing GitHub pull requests, reviews, and comments in Projects is in scope for follow-up work; this document remains the merge path.

## Product Decisions

Buzz uses the installed GitHub CLI and the authentication that `gh` resolves for `github.com`.
Buzz never reads, persists, prints, or logs a GitHub token.
The user must install `gh` and authenticate it outside Buzz.

The merge method is GitHub's merge-commit method through the GitHub REST pull-request merge endpoint executed by `gh api`.
If the target repository disables merge commits, GitHub rejects the operation and Buzz links the user to the GitHub pull request instead of silently choosing another method.

Buzz does not use administrator bypass, auto-merge, merge-queue enrollment, or direct git push as a fallback.
Buzz intentionally does not call `gh pr merge` because current GitHub CLI behavior can automatically enable auto-merge or enqueue a pull request when a merge queue applies.
The REST merge request supplies the expected head SHA atomically and returns a rejection instead of opting into deferred merge behavior.
Buzz also detects an active `merge_queue` rule before the request so it can show a specific recovery message, and it fails closed when rule discovery is unavailable.

No feature flag is required.
Strict URL routing isolates the new behavior, and the existing path remains the fallback only when neither clone URL is a GitHub URL.

## Architecture

### Router

`merge_project_pull_request` remains the single Tauri entry point.
It first validates the Nostr owner, obtains the owner identity, normalizes both branches and the expected commit, and validates the Buzz pull-request metadata.
It then classifies both clone URLs before choosing a repository operation.

- When both URLs are strict GitHub URLs, the command calls the GitHub orchestration path.
- When neither URL is a GitHub URL, the command runs the existing Buzz workspace URL validation, clone-owner check, authenticated native git merge, and push path without behavioral changes.
- When only one URL is a GitHub URL, the command returns a structured unsupported-host-pair error.

The existing equality check between the target clone URL owner and the Nostr `target_owner` remains only in the Buzz-hosted path.
The GitHub path does not compare those unrelated identity namespaces.

### GitHub Module

A focused `desktop/src-tauri/src/commands/project_github_pull_request.rs` module owns the GitHub behavior.
It contains the strict URL parser, the `gh` process runner, the minimal JSON response types, PR lookup and creation, gate evaluation, merge-queue detection, merge execution, and post-merge verification.
It does not introduce a provider interface, factory, or general remote-host abstraction because this version has one external host implementation.

The module reuses dependencies and utilities already present in the desktop crate:

- `url::Url` for URL parsing.
- `serde` and `serde_json` for bounded JSON contracts.
- `tempfile` for the PR-creation request body.
- `managed_agents::resolve_command` so GUI launches can locate `gh` through Buzz's existing command-resolution behavior.
- `std::process::Command` and the existing no-window helper for direct child-process execution without a shell.

### Frontend Contract

`ProjectPullRequestMergeInput` gains `title` and `body` fields.
`useMergeProjectPullRequestMutation` supplies `pullRequest.title` and `pullRequest.content` without modifying either value.
The current result shape remains unchanged so a confirmed GitHub merge can reuse the existing status-publication and retry behavior.

`ProjectPullRequestMergeRecovery` becomes a discriminated union with the existing `open_terminal` variant and a new `open_url` variant.
The `open_url` variant contains a validated GitHub pull-request URL and a bounded list of human-readable blocking reasons.
The frontend rejects any recovery URL that is not an exact `https://github.com/<owner>/<repo>/pull/<number>` or `https://github.com/<owner>/<repo>/pulls` URL before handing it to the installed Tauri opener plugin.
The repository pull-request list form is reserved for ambiguous lookup results where no single pull-request number is safe to select.

## Trust Boundaries and Validation

The GitHub parser accepts only HTTPS URLs on the exact `github.com` host with no explicit port.
It rejects embedded credentials, query strings, fragments, empty path segments, extra path segments, control characters, and whitespace.
The path must contain exactly an owner and repository, with one optional terminal `.git` suffix removed from the repository name.
Owner characters are limited to ASCII letters, digits, and hyphens, while repository characters are limited to ASCII letters, digits, periods, underscores, and hyphens.

Branch names continue through the existing branch normalizer.
The expected commit continues through the existing commit normalizer and must match GitHub's `headRefOid` or REST `head.sha` case-insensitively before any merge command runs.
The title must be non-empty after trimming because GitHub requires a title when creating a pull request.
The body remains optional and may be empty.

Every repository argument is pinned to `github.com/<owner>/<repo>`.
Every API path is assembled only from validated owner and repository components.
No user-controlled value is interpreted by a shell.

## GitHub CLI Process Contract

The runner resolves `gh` to an executable path once per merge attempt and starts it directly with an argument vector.
It closes stdin for ordinary commands, pipes stdout and stderr, disables an extra console window where the platform helper supports it, and applies a 60-second wall-clock timeout to each invocation.
GitHub API mutations write their JSON request to a permission-restricted temporary file and pass that path to `gh api --input`, which preserves the closed-stdin rule and avoids command-line length limits for the PR body.
Each temporary file is deleted when its request scope ends.

The runner drains both output pipes while the process runs so a chatty child cannot deadlock on a full pipe.
It retains at most 64 KiB from each stream and terminates the child on timeout.
User-facing diagnostics retain at most 8 KiB, remove URL userinfo and authorization-header values, and redact known GitHub token prefixes before crossing the Tauri boundary.
The runner never includes the child environment in an error or log message.

The preflight command is `gh auth status --hostname github.com`.
A missing executable maps to `github_cli_missing`.
An unsuccessful auth preflight maps to `github_auth_required` with the copyable command `gh auth login --hostname github.com`.
Unsupported flags or API behavior map to `github_merge_failed` with guidance to update GitHub CLI.

## Pull Request Lookup and Creation

Buzz lists matching pull requests with the GitHub REST pull-request endpoint through `gh api`.
The request pins the target owner and repository, asks for all states, filters by `head=<source-owner>:<source-branch>` and `base=<target-branch>`, and uses pagination.
Buzz then filters the response locally to exact case-insensitive repository full names and exact case-sensitive branch names.
This local check is required because branch names are case-sensitive and server-side filtering alone is not the identity boundary.

Selection follows these rules in order:

1. Exactly one open PR is reused.
2. More than one open PR is `github_pr_ambiguous`.
3. With no open PR, exactly one merged PR whose head SHA equals `expected_commit` is reused for recovery after a lost response or failed Nostr publication.
4. Multiple merged PRs matching the expected head are `github_pr_ambiguous`.
5. A closed, unmerged match stops with an `open_url` recovery instead of creating another PR implicitly.
6. With no match, Buzz creates one PR.

Creation uses `gh api --method POST --hostname github.com repos/<target-owner>/<target-repo>/pulls --input <temporary-json-file>`.
The JSON body always contains `title`, `body`, `head`, and `base`.
It also contains `head_repo` when the source repository differs from the target repository so cross-repository pull requests owned by the same organization are handled correctly.
After creation, Buzz performs the exact lookup again and proceeds only when it resolves one matching PR.
This second lookup makes a retry safe when GitHub created the PR but the original response was lost.

## Gate Evaluation and Merge State Machine

After lookup or creation, Buzz reads the current GitHub PR state and compares the head OID with `expected_commit`.
Any mismatch returns `github_branch_changed` and requires the user to refresh the Buzz pull request.

The states are:

### Already Merged

When GitHub reports `MERGED`, the head OID must still equal the expected commit and `mergeCommit.oid` must be present.
Buzz skips the merge command and uses that merge commit to build and publish the missing Nostr status.

### Closed Without Merge

When GitHub reports a closed but unmerged PR, Buzz stops and returns `github_pr_blocked` with an Open GitHub PR recovery.
Buzz does not reopen the PR or create a replacement.

### Open but Not Ready

Buzz reads `isDraft`, `mergeable`, `mergeStateStatus`, `reviewDecision`, `autoMergeRequest`, and required checks.
Required checks come from `gh pr checks <number> --required --json name,state,bucket,link`, where exit code `8` is treated as a valid pending result rather than a process failure.
GitHub CLI's no-required-checks result is normalized to an empty passing set.
Draft status, merge conflicts, a required review, requested changes, a required check outside the `pass` bucket, an existing auto-merge request, or an indeterminate merge state produces `github_pr_blocked`.
The recovery lists the exact known blockers and links to the GitHub PR.

Buzz queries `repos/<owner>/<repo>/rules/branches/<percent-encoded-target-branch>` through `gh api` before merging.
An active rule with type `merge_queue` produces `github_pr_blocked` because this version does not enqueue or enable auto-merge.
Failure to determine applicable rules also fails closed and links to GitHub.

### Open and Ready

Buzz writes `{ "sha": expected_commit, "merge_method": "merge" }` to a permission-restricted temporary JSON file.
It calls `gh api --method PUT --hostname github.com repos/<owner>/<repo>/pulls/<number>/merge --input <temporary-json-file>`.
The `sha` field is the atomic stale-head guard, and GitHub returns conflict status when the pull-request head no longer matches it.
That conflict maps to `github_branch_changed` rather than a generic merge failure.
This endpoint performs an immediate merge or returns a rejection; it does not opt Buzz into auto-merge or a merge queue.
GitHub remains the final authority, so any rejection becomes a structured error instead of a local-push fallback.

The merge response must contain `merged: true` and a non-empty `sha`.
Buzz then immediately re-queries the PR.
Success requires state `MERGED`, the same head OID, a non-null merged timestamp, and a non-null merge commit OID.
Any other result is treated as not merged, and Buzz does not publish kind `1631`.

## Nostr Status Publication

The existing owner identity signs the merged-status event only after the GitHub state machine returns a verified merge commit.
The event keeps the current repository address, pull-request event ID, pull-request author, timestamp monotonicity, and `merge-commit` tag behavior.

If relay publication succeeds, the command returns the normal merged result.
If relay publication fails, the command returns the signed status event and publication error through the existing partial-success result.
The UI changes the action to Publish merged status, and retrying that action publishes only the saved event.
It does not call GitHub again.

## Error Model and Recovery UX

The existing `code + message + recovery` envelope remains the Tauri error contract.
The GitHub path adds these stable codes:

- `github_cli_missing` means `gh` could not be resolved.
- `github_auth_required` means `gh` is present but not authenticated for `github.com`.
- `github_pr_blocked` means GitHub state, checks, reviews, rules, draft state, or an existing auto-merge request prevents the requested immediate merge.
- `github_branch_changed` means the GitHub head OID no longer matches the reviewed Buzz commit.
- `github_pr_ambiguous` means more than one GitHub PR could represent the Buzz pull request.
- `github_merge_failed` means GitHub CLI, GitHub API, permissions, repository settings, or final merge verification failed outside the more specific cases.

The confirmation dialog uses host-specific text.
For GitHub, it states that Buzz will find or create a GitHub PR, check its gates, and merge it only when immediately allowed.
It no longer claims that Buzz will perform a local merge and push.

Actionable GitHub errors render as a persistent card below the Merge action rather than only as a toast.
The missing-CLI card explains that GitHub CLI must be installed.
The auth card shows a copyable `gh auth login --hostname github.com` command and Retry.
The blocked card lists the known checks or reviews and provides Open GitHub PR and Retry actions.
The branch-changed card tells the user to refresh before retrying.
Closed or ambiguous cases link to GitHub for manual resolution.

The existing Buzz merge-conflict card and Resolve in Terminal flow remain unchanged and appear only for the Buzz-hosted native merge path.

## Testing Strategy

Implementation starts with a failing Playwright test that reproduces the reported end-user flow using a project clone URL of `https://github.com/anhle128/buzz`.
Before the fix, clicking Merge must reach the same workspace-URL rejection represented by the original report.
After the fix, the same flow must route to the mocked GitHub behavior and render the GitHub-specific result.

Rust unit tests cover strict URL acceptance and rejection, routing, exact PR selection, all state-machine branches, SHA mismatch, missing CLI, missing auth, timeout, output bounds, redaction, merge-queue refusal, and idempotent recovery after create or merge.
Command tests use a temporary fake `gh` executable that records arguments and returns scripted JSON.
They assert direct execution without a shell, closed stdin, exact repository pinning, an atomic merge request containing the expected `sha` and `merge_method: "merge"`, timeout cleanup, and no status-event construction before a confirmed merge.

TypeScript tests cover parsing both recovery variants and rejecting unsafe Open URL payloads.
Playwright covers the GitHub confirmation copy, installation and auth guidance, blocked-gate card, Open GitHub PR, Retry, branch-changed state, success, and Publish merged status partial recovery.
Existing Playwright coverage for Buzz-hosted merges, managed-agent owners, unauthorized viewers, conflicts, and Nostr publication must continue to pass.

The final verification sequence is the full desktop TypeScript unit suite, the full desktop Tauri Rust test suite, the full desktop Playwright suite, `just ci`, and `git diff --check`.
The actionable cards are captured and inspected in light and dark modes at the repository's standard desktop viewport.
A live smoke test, when authorized, uses only a disposable branch and pull request in a permitted GitHub repository and never an active user work item.

## Rollout and Rollback

The change requires no schema migration, new dependency, background service, or feature flag.
GitHub-backed projects gain the new route as soon as the desktop build ships.
Machines without a usable GitHub CLI fail closed with instructions, while Buzz-hosted repositories continue through the existing path.

Rollback is code-only because no persistent format changes.
Removing the GitHub router branch and frontend GitHub recovery card restores the previous behavior without data migration.

## Definition of Done

- The reported `https://github.com/anhle128/buzz` scenario finds or creates a real GitHub PR and merges it when GitHub permits an immediate merge.
- Pending or failed required checks, missing approvals, requested changes, conflicts, draft state, merge queues, and branch rules prevent both merge and Nostr merged status.
- A changed head commit cannot be merged through a stale Buzz view.
- Retrying after a lost create response does not create a duplicate PR.
- Retrying after GitHub merged does not merge twice.
- A failed Nostr publication can be retried without calling GitHub again.
- Missing CLI and missing auth states provide actionable persistent guidance.
- No GitHub token appears in UI text, logs, captured diagnostics, test snapshots, or temporary files.
- The existing Buzz-hosted merge, conflict recovery, ownership checks, and status-publication tests remain green.
- All repository quality gates listed in the testing strategy pass at the implementation commit.

## References

- Current Tauri merge command: `desktop/src-tauri/src/commands/project_git_workflow.rs`.
- Current structured merge errors: `desktop/src-tauri/src/commands/project_git_merge_error.rs`.
- Current frontend API boundary: `desktop/src/shared/api/projectGit.ts`.
- Current merge UI: `desktop/src/features/projects/ui/MergePullRequestButton.tsx`.
- Current desktop E2E coverage: `desktop/tests/e2e/project-pr-review.spec.ts`.
- GitHub CLI merge-queue and implicit auto-merge behavior that this design avoids: <https://cli.github.com/manual/gh_pr_merge>.
- GitHub CLI pull-request JSON fields: <https://cli.github.com/manual/gh_pr_view>.
- GitHub CLI required-check output: <https://cli.github.com/manual/gh_pr_checks>.
- GitHub REST pull-request lookup and creation: <https://docs.github.com/en/rest/pulls/pulls>.
- GitHub REST active branch rules and `merge_queue` rule type: <https://docs.github.com/en/rest/repos/rules>.
