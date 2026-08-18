# Phase 02 — GitHub Issues (M2)

**Priority:** P1  
**Status:** Pending  
**Depends on:** M1 (host detection + `gh` auth UX)

## Overview

The Issues tab is NIP-34 `kind:1621` on the relay, keyed by repo address. It is not GitHub Issues. There is no Issues API. For GitHub-hosted repos, GitHub Issues must be the backend.

## Features

| ID | Feature |
|----|---------|
| I1 | List issues `#N` |
| I2 | Create issue on GitHub (do not publish kind:1621) |
| I3 | Close / reopen |
| I4 | Comment |
| I5 | Labels |
| I6 | Assignees (GitHub login, not npub) |

## Architecture

- `host === github.com` → `gh api` issues list/create/update/comment
- Reuse `ProjectIssue` UI; show GitHub number `#N`
- Open/closed maps to GitHub `state`
- Assignees are GitHub logins + avatars
- Missing/`gh auth` uses the same recovery as merge
- Buzz-hosted repos keep kind:1621

## Related code

- `desktop/src/features/projects/issueMutations.ts`
- `desktop/src/features/projects/hooks.ts` (`fetchProjectIssues`)
- `desktop/src/features/projects/ui/ProjectIssuesPanel.tsx`
- `desktop/src/features/projects/ui/CreateProjectIssueDialog.tsx`
- `desktop/src-tauri/src/commands/project_github_pull_request.rs` (`gh` runner)

## Todo

- [ ] I1 List
- [ ] I2 Create
- [ ] I3 Close / reopen
- [ ] I4 Comment
- [ ] I5 Labels
- [ ] I6 Assignees

## Success

Issues tab on `harness-service` shows GitHub `#N`. Creating an issue creates a GitHub issue. Buzz-hosted issue flow unchanged.

## Suggested Superpowers slice

Start **I1 + I2** only (list + create). Status, labels, assignees, and comments are read-only. I3–I6 writes are later.

**I1+I2 spec:** [docs/superpowers/specs/2026-08-18-github-issues-design.md](../../docs/superpowers/specs/2026-08-18-github-issues-design.md)
