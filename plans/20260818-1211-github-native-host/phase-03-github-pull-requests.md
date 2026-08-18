# Phase 03 — GitHub Pull Requests (M3)

**Priority:** P1  
**Status:** Pending  
**Depends on:** M1  

## Overview

Create/review of Buzz PRs is Nostr and host-agnostic. Merge for GitHub already goes through `gh` (`merge_github_pull_request`). The PR tab does not list or create GitHub PRs. For GitHub-hosted repos, GitHub PRs are the backend.

## Features

| ID | Feature | Notes |
|----|---------|--------|
| P1 | List GitHub PRs | PR tab |
| P2 | Create GitHub PR | `gh api` REST `POST /repos/.../pulls` |
| P3 | Show review / checks | Merge path already reads gates |
| P4 | Merge | **Exists** — keep |
| P5 | Review comments | Later |

## Architecture

- `host === github.com` → list/create via `gh`
- Merge: keep `desktop/src-tauri/src/commands/project_github_pull_request.rs`
- Buzz-hosted PRs stay Nostr + native git merge

## Related code

- `desktop/src/features/projects/ui/ProjectPullRequestsPanel.tsx`
- `desktop/src/features/projects/pullRequestMutations.ts`
- `desktop/src/features/projects/ui/MergePullRequestButton.tsx`
- `desktop/src-tauri/src/commands/project_github_pull_request.rs`

## Todo

- [ ] P1 List
- [ ] P2 Create
- [ ] P3 Review / checks in UI
- [x] P4 Merge (already ships)
- [ ] P5 Review comments (optional, after P1–P3)

## Success

PR tab on a GitHub-hosted repo shows GitHub PRs. Opening a PR creates a GitHub PR. Merge still uses `gh`.

## Suggested Superpowers slice

Start **P1 + P2** only (list + create). Hide Merge, review writes, and Files changed. Then **P3** (checks / review chrome). Then **P4 adapt** (merge the listed `#N`, no find-or-create, no kind:1631). **P5** later.

**P1+P2 spec:** [docs/superpowers/specs/2026-08-18-github-pull-requests-design.md](../../docs/superpowers/specs/2026-08-18-github-pull-requests-design.md)
