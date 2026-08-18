# Phase 04 — Polish (M4)

**Priority:** P2  
**Status:** Pending  
**Depends on:** M1, M2, M3

## Overview

Wire counts, announcement metadata, agent docs, and tests after git/issues/PRs work.

## Features

| ID | Feature |
|----|---------|
| X1 | Write `default-branch` on the 30617 announcement after a GitHub probe |
| X2 | Issue/PR counts on the project card from GitHub |
| X3 | Auth UX: `gh` missing / not logged in (reuse merge recovery) |
| X4 | Agent docs: GitHub remotes need `gh` on PATH |
| X5 | Tests: URL gate, default branch, issue/PR host split |

## Todo

- [ ] X1 Persist `default-branch`
- [ ] X2 Card counts
- [ ] X3 Auth recovery everywhere
- [ ] X4 ACP / CLI docs
- [ ] X5 Tests next to `project_github_pull_request/tests.rs`

## Success

Cards show live GitHub counts. Other clients can read `default-branch`. Failed auth is obvious. Buzz-hosted paths stay green.
