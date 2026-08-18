# GitHub as a native repository host

**Status:** Phase 0 done. Phases 1–4 not started.  
**Contract:** GitHub-hosted repo → GitHub is source of truth for git, issues, PRs. Buzz-hosted repo (`/git/<pubkey>/<id>`) unchanged. No dual-write.

| Milestone | File | Status |
|-----------|------|--------|
| M0 Product contract | [phase-00-product-contract.md](phase-00-product-contract.md) | **Done** |
| M1 Git native | [phase-01-git-native.md](phase-01-git-native.md) | Pending |
| M2 GitHub Issues | [phase-02-github-issues.md](phase-02-github-issues.md) | Pending |
| M3 GitHub Pull Requests | [phase-03-github-pull-requests.md](phase-03-github-pull-requests.md) | Pending |
| M4 Polish | [phase-04-polish.md](phase-04-polish.md) | Pending |

## Host split

```
clone URL github.com     → GitHub API + git (`gh` + OS credentials)
clone URL Buzz /git/...  → NIP-34 + relay git (no change)
```

## Order

Brainstorm + implement **M1 first** (unblocks `develop` / create-branch). Then M2, M3, M4.

## Out of scope (v1)

GitHub Enterprise, GitLab, OAuth token in Buzz, GitHub as Buzz chat identity, import old GitHub issues into Nostr, channel ACL on GitHub git.
