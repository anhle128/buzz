# Phase 01 — Git native (M1)

**Priority:** P0  
**Status:** Pending  
**Depends on:** M0  
**Brainstorm this first.**

## Overview

Desktop git today is Buzz-only. Most Tauri commands require `validate_workspace_clone_url`. Sync and remote snapshot are gated `host.kind === "buzz"`. GitHub remotes show Open, not Fetch; branch picker does not read GitHub refs. That is why `develop` is missing and Create branch is locked.

## Features

| ID | Feature | Why |
|----|---------|-----|
| G1 | List branches from GitHub | Picker does not read GitHub refs |
| G2 | Read GitHub default branch | Settings `develop` still shows `main` |
| G3 | Remote snapshot (README / files / commits) | Query enabled only for Buzz |
| G4 | Fetch + ahead/behind | Sync disabled when host is not Buzz |
| G5 | Pull / push in the app | Header is Open only |
| G6 | Create / delete branch on GitHub | UI shown; Tauri rejects GitHub URL |
| G7 | Hide actions that cannot succeed | Create branch disabled with a wrong message |

## Architecture

- `validate_git_operation_url` = Buzz workspace **or** `GitHubRepoRef::parse`
- Apply on snapshot, sync, fetch, pull, push, create/delete branch
- Default branch order: GitHub `default_branch` → announcement tag → `main`/`master`/first ref
- Auth: `gh` + OS git credentials. Do not store a GitHub token. Reuse merge recovery for missing/`gh auth`

## Related code

- `desktop/src-tauri/src/commands/project_git_exec.rs`
- `desktop/src-tauri/src/commands/project_git.rs`
- `desktop/src-tauri/src/commands/project_git_branches.rs`
- `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`
- `desktop/src/features/projects/repoSyncHooks.ts`
- `desktop/src/features/projects/projectModels.ts`
- `desktop/src/features/projects/lib/projectBranches.ts`

## Todo

- [ ] G1 List refs (`ls-remote` or `gh api`)
- [ ] G2 Resolve default branch from GitHub
- [ ] G3 Enable remote snapshot for `github.com`
- [ ] G4 Fetch + sync status
- [ ] G5 Pull / push
- [ ] G6 Create / delete remote branch
- [ ] G7 Disable/hide dead actions until backend exists

## Success

On a GitHub repo whose default is `develop`:

- Opening the project selects `develop`
- Dropdown lists GitHub remotes (`develop`, `main`, features)
- Remote source shows README/files
- Fetch updates local; Create branch works when HEAD is known
- Buzz-hosted repos unchanged

## Suggested Superpowers slice

Start **G1 + G2** only. Then G3+G4. Then G5–G7.

**G1+G2 implementation plan:** [docs/superpowers/plans/2026-08-18-github-repository-state.md](../../docs/superpowers/plans/2026-08-18-github-repository-state.md)  
**G1+G2 spec:** [docs/superpowers/specs/2026-08-18-github-repository-state-design.md](../../docs/superpowers/specs/2026-08-18-github-repository-state-design.md)

**G3+G4 spec:** [docs/superpowers/specs/2026-08-18-github-snapshot-and-fetch-design.md](../../docs/superpowers/specs/2026-08-18-github-snapshot-and-fetch-design.md)
