# Optional repository name on Create project

**Status:** Draft — awaiting review  
**Scope:** Buzz Desktop **Create a new project** only. One optional field so the first `kind:30617` can have a different display name and `d` tag than the `kind:30621` project.  
**Surfaces:** Desktop. Not CLI, mobile, web, relay ingest, or **Add repository**.

## Summary

Create project today slugifies **Name** and writes that slug and display string onto both the project and the initial repository. A project that will hold several repositories therefore inherits the first repo’s identity.

This design adds an optional **Repository name** field. Empty keeps today’s events. Filled names only the first repository. A value such as `anhle128/buzz` is stored as the repo `name` tag; the repo `d` tag is the dashed slug `anhle128-buzz`. The project keeps the identity from **Name**.

## Problem

`buildInitialProjectEventTemplates` in `desktop/src/features/projects/projectCreation.ts` uses one `dtag` for:

- project `d` and `name`
- repository `d` and `name`
- the project member coordinate `30617:<owner>:<dtag>`

VISION_PROJECTS.md and NIP-MP already model one project, many repos (project `platform`, members `buzz` and `buzz-infra`). **Add repository** already takes its own name. Create project is the leftover coupling.

## Goals

- Let the author set the first repository’s display name independently of the project name.
- Accept `/` in that display name (`anhle128/buzz`).
- Leave the empty-field path bit-compatible with current events and tests.
- Refuse to overwrite an existing `kind:30617` when the repo slug is not the project slug.

## Non-goals

- Changing NIP-MP, NIP-34, or relay `validate_repo_id`.
- Putting `/` in the repo `d` tag (relay and `/git/<pubkey>/<repo>` require one `[a-zA-Z0-9._-]{1,64}` segment).
- Autofilling clone/web URLs from `owner/repo`.
- Creating a project with zero repositories.
- Making **Repository name** required.
- Prefilling the field from **Name**.
- Redesigning the dialog into Project vs Initial repository groups.
- Changing **Add repository**, CLI `buzz projects create`, or mobile (no create-project form).
- Changing the **Name** placeholder.

## Product decisions

Locked in brainstorming:

| Decision | Choice |
|----------|--------|
| Empty **Repository name** | First repo copies project display name and project slug |
| Field presentation | Starts empty; helper `Defaults to the project name.` |
| Approach | Optional override on the current create path; no dialog regroup |
| `anhle128/buzz` | `name` = `anhle128/buzz`; `d` = `anhle128-buzz` |
| GitHub URL fill | No |

## Event mapping

Create still publishes two events in today’s order: `kind:30617` then `kind:30621`.

| Input | Project `30621` | First repo `30617` |
|-------|-----------------|--------------------|
| **Name** only | `d` + `name` from **Name** | Same as project |
| **Name** + **Repository name** | Still only from **Name** | `d` + `name` from **Repository name** |

Slug algorithm is `repositoryDtagFromName`: lowercase, each run of `[^a-z0-9]+` → `-`, trim leading/trailing `-`.

Example: **Name** `Bee Garden`, **Repository name** `anhle128/buzz`

- Project: `d=bee-garden`, `name=Bee Garden`
- Repo: `d=anhle128-buzz`, `name=anhle128/buzz`
- Project `a` tag: `30617:<owner>:anhle128-buzz`

Description, `buzz-channel`, clone URL, and web URL stay as they are: channel and description on both events; URLs only on the repo.

## UI

`desktop/src/features/projects/ui/CreateProjectDialog.tsx`

Field order:

1. **Name** (required) — unchanged
2. **Repository access channel** — unchanged
3. **Description** — unchanged
4. **Repository name** — new, optional
5. **Initial repository clone URL** — unchanged
6. **Initial repository web URL** — unchanged

**Repository name**

- Label `Repository name` plus the existing `Optional` marker
- Helper under the input: `Defaults to the project name.`
- Placeholder: `bee-garden-ios`
- Same single-line chrome as the other text fields
- `id="create-project-repository-name"`
- `data-testid="create-project-repository-name"`
- Reset to `""` whenever the dialog opens
- Submit does not require it
- Disabled while `isCreating`

The dialog passes `repositoryName` into `onCreate` only when `trim()` is non-empty. Otherwise the property is omitted.

## Write path

### Input

`CreateProjectInput` and `buildInitialProjectEventTemplates` gain:

```ts
repositoryName?: string;
```

Omitted, `undefined`, and whitespace-only are the same: first repo copies the project.

### Template builder

`desktop/src/features/projects/projectCreation.ts`

- Project `d` / `name` always from trimmed **Name**.
- When `repositoryName` is present after trim:
  - repo `name` tag = that trimmed string (slash kept)
  - repo `d` tag = `repositoryDtagFromName(trimmed)`
- Otherwise repo `d` / `name` = project `d` / `name`.
- `repositoryAddress` uses the **repo** `d` tag.

`InitialProjectEventTemplates` keeps `dtag` as the **project** slug (resume and “already have this project” stay keyed on it). Add `repositoryDtag: string` so callers do not re-slug. When the field is empty, `repositoryDtag === dtag`.

**Slug DRY, no import cycle:** `projectRepositoryCreation.ts` already imports from `projectCreation.ts`. Move `repositoryDtagFromName` into `projectCreation.ts` and re-export it from `projectRepositoryCreation.ts`. Do not add a second slugger. `projectDtagFromName` becomes a call to that same function.

### Create mutation

`desktop/src/features/projects/useCreateProject.ts`

- Project identity, resume, and `You already have a project named "…".` stay keyed on `templates.dtag` (project slug).
- Publish order and unsupported-`30621` fallback stay as they are.

**Clobber guard (only when `repositoryDtag !== dtag`):**

After templates are built, fetch `kinds: [30617]`, `authors: [owner]`, `#d: [repositoryDtag]`, `limit: 1` (same query as **Add repository**). Do not trust `fetchProjects()` alone; hidden cards are omitted there.

If a head exists, throw:

`A repository named "<repositoryDtag>" already exists (as a standalone repository or in another project). Choose a different name to avoid overwriting it.`

When `repositoryDtag === dtag`, do **not** run this new query. Today’s project-collision, resume, and legacy implicit-project paths stay unchanged.

## Validation and errors

All new checks run in the template builder except the clobber fetch (mutation).

| Situation | Result |
|-----------|--------|
| **Name** empty | Submit stays disabled |
| **Name** slug empty | `Project name must include letters or numbers.` |
| **Name** > 256 bytes | existing error |
| **Repository name** filled, slug empty (`///`) | `Repository name must include letters or numbers.` No fallback to the project name |
| **Repository name** trimmed display > 256 bytes | `Repository name must not exceed 256 bytes.` |
| Repo slug length > 64 | `Repository name slug must not exceed 64 characters.` |
| Access channel / description | existing errors |
| Project slug already owned | existing `You already have a project named "…".` |
| Distinct repo slug, existing `30617` | clobber message above |
| Relay timeout / unsupported kind | existing handling |

The dialog stays open and shows the error in the existing red line.

A filled **Repository name** never falls back to the project name, even if invalid.

## Testing

No live relay required for unit tests.

Extend `desktop/src/features/projects/projectCreation.test.mjs`:

- Omit `repositoryName`: same tags as today’s `Sprout` fixture (`d`/`name`/`a` still `sprout`).
- `name: "Bee Garden"`, `repositoryName: "anhle128/buzz"` → project `d=bee-garden`, `name=Bee Garden`; repo `d=anhle128-buzz`, `name=anhle128/buzz`; `a` = `30617:<owner>:anhle128-buzz`; `templates.dtag === "bee-garden"`; `templates.repositoryDtag === "anhle128-buzz"`.
- `repositoryName: "///"` throws `/letters or numbers/`.
- `repositoryName` of 65 `a` characters after slugify throws `/64 characters/`.
- Existing description byte-limit and `!!!` project-name tests still pass.

Add `desktop/src/features/projects/useCreateProject.test.mjs`, following `useAddProjectRepository.test.mjs` (inject fetch/publish seams):

- when `repositoryDtag !== dtag` and the `30617` fetch returns a head, throw the clobber message and publish nothing;
- when `repositoryDtag === dtag`, do not issue that extra `30617` fetch.

Dialog / e2e:

- Existing create-project e2e that only fill **Name** stay green (empty field).
- One e2e addition (same create-project dialog coverage as `desktop/tests/e2e/project-commit-detail.spec.ts`): the field is visible with the Optional marker and helper `Defaults to the project name.`; submit stays enabled without it. Event-shape for `anhle128/buzz` is covered by the unit tests above, not by the mock bridge.

No screenshot set.

## Success criteria

- Creating **Bee Garden** with repository name `anhle128/buzz` yields a project card named Bee Garden whose first repo display is `anhle128/buzz` and whose git id is `anhle128-buzz`.
- Creating with only **Name** still produces one shared slug, same events as before this change.
- Typing `///` or a name that slugs to an existing other repo does not publish.
- Relay repo ids never contain `/`.
