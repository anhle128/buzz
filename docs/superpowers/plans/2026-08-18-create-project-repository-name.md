# Optional repository name on Create project Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an optional **Repository name** field to Desktop **Create a new project** so the first `kind:30617` can use a different display `name` and `d` tag than the `kind:30621` project.

**Architecture:** Keep the current two-event create path (`kind:30617` then `kind:30621`).
`buildInitialProjectEventTemplates` still keys project identity on trimmed **Name**.
When `repositoryName` is present after trim, the repository `name` tag keeps that string (including `/`) and the repository `d` tag is `repositoryDtagFromName(trimmed)`.
`useCreateProject` queries `kinds: [30617]` only when `repositoryDtag !== dtag` and refuses to publish if a head already exists.

**Tech Stack:** TypeScript, React 19, TanStack Query, Node `node:test` via `desktop` `pnpm test`, Playwright mock-bridge smoke.

**Spec:** [2026-08-18-create-project-repository-name-design.md](../specs/2026-08-18-create-project-repository-name-design.md)

**Product contract:** [VISION.md](../../../VISION.md) and [VISION_PROJECTS.md](../../../VISION_PROJECTS.md) already model one project with many repos (project `platform`, members `buzz` and `buzz-infra`).
This change removes the leftover Create-project coupling that forced the first repo to inherit the project slug.
It does not change NIP-MP, NIP-34, relay `validate_repo_id`, CLI `buzz projects create`, mobile, web, or **Add repository**.

**GitNexus blast radius (upstream, 2026-08-18):** `buildInitialProjectEventTemplates`, `createProject`, `repositoryDtagFromName`, and `CreateProjectDialog` are all **LOW** risk.
Direct callers stay inside Desktop Projects create/add-repository.
No HIGH or CRITICAL warning.

## Global Constraints

- Desktop **Create a new project** only.
- Do not change CLI, mobile, web, relay ingest, or **Add repository**.
- Empty **Repository name** keeps today’s events: first repo copies the project display name and project slug.
- Field presentation starts empty with helper `Defaults to the project name.`
- `anhle128/buzz` stores `name=anhle128/buzz` and `d=anhle128-buzz`.
- Do not put `/` in the repo `d` tag.
- Do not autofill clone/web URLs from `owner/repo`.
- Do not create a project with zero repositories.
- Do not make **Repository name** required.
- Do not prefill the field from **Name**.
- Do not regroup the dialog into Project vs Initial repository sections.
- Do not change the **Name** placeholder (`bee-garden-game`).
- `InitialProjectEventTemplates.dtag` remains the **project** slug.
- Add `repositoryDtag: string` on that same type.
- When the field is empty, `repositoryDtag === dtag`.
- Move `repositoryDtagFromName` into `projectCreation.ts` and re-export it from `projectRepositoryCreation.ts`.
- Do not add a second slugger.
- `projectDtagFromName` must call `repositoryDtagFromName`.
- Project identity, resume, and `You already have a project named "…".` stay keyed on `templates.dtag`.
- Clobber fetch runs only when `repositoryDtag !== dtag`.
- Do not trust `fetchProjects()` for the clobber check; hidden cards are omitted there.
- A filled **Repository name** never falls back to the project name, even if invalid.
- No live relay is required for unit tests.
- No screenshot set.
- Activate Hermit in every shell: `. ./bin/activate-hermit && …`.
- Shell CWD does not persist across commands; `cd` in the same command.
- Commits use `git commit -s`.
- Before each commit run `node .gitnexus/run.cjs detect` if that runner exists.
- Do not edit NIP-MP, NIP-34, or `validate_repo_id`.

---

## File map

| File | Role |
|------|------|
| `desktop/src/features/projects/projectCreation.ts` | Shared `repositoryDtagFromName`, `repositoryName` on the template builder, `repositoryDtag` on the result |
| `desktop/src/features/projects/projectRepositoryCreation.ts` | Re-export `repositoryDtagFromName` only; keep add-repository templates unchanged |
| `desktop/src/features/projects/projectCreation.test.mjs` | Empty-field bit-compatibility plus override, `///`, 64-char, and 256-byte cases |
| `desktop/src/features/projects/projectRepositoryCreation.test.mjs` | One re-export slug assertion so add-repository still sees the same function |
| `desktop/src/features/projects/useCreateProject.ts` | `CreateProjectInput.repositoryName?`, export `createProject`, inject I/O seams, clobber fetch |
| Create: `desktop/src/features/projects/useCreateProject.test.mjs` | Clobber throws and publishes nothing; same-slug path does not query `30617` |
| `desktop/src/features/projects/ui/CreateProjectDialog.tsx` | Optional field after Description, before clone URL |
| `desktop/tests/e2e/project-commit-detail.spec.ts` | Field visible, Optional marker, helper, submit enabled without it, reset on reopen |

Do not create other files.

---

### Task 1: Shared slugger and first-repo override in the template builder

**Files:**
- Modify: `desktop/src/features/projects/projectCreation.ts`
- Modify: `desktop/src/features/projects/projectRepositoryCreation.ts:14-21` and the `repositoryDtagFromName` import/call at the add-repository builder
- Modify: `desktop/src/features/projects/projectCreation.test.mjs`
- Modify: `desktop/src/features/projects/projectRepositoryCreation.test.mjs`
- Test: `desktop/src/features/projects/projectCreation.test.mjs`
- Test: `desktop/src/features/projects/projectRepositoryCreation.test.mjs`

**Interfaces:**
- Consumes: existing `isValidProjectChannelId`, `KIND_PROJECT_ANNOUNCEMENT` (`30621`), `KIND_REPO_ANNOUNCEMENT` (`30617`)
- Produces:
  - `export function repositoryDtagFromName(name: string): string`
  - `function projectDtagFromName(name: string): string` that only calls `repositoryDtagFromName(name)`
  - `export type InitialProjectEventTemplates = { dtag: string; repositoryDtag: string; project: ProjectEventTemplate; repository: ProjectEventTemplate; repositoryAddress: string }`
  - `buildInitialProjectEventTemplates({ accessChannelId: string; cloneUrl?: string; description?: string; name: string; ownerPubkey: string; repositoryName?: string; webUrl?: string }): InitialProjectEventTemplates`
  - `export { repositoryDtagFromName } from "./projectCreation"` in `projectRepositoryCreation.ts` (or equivalent import-then-export)
- `useAddProjectRepository.ts` keeps importing `repositoryDtagFromName` from `projectRepositoryCreation.ts`.

- [ ] **Step 1: Write the failing template tests**

In `desktop/src/features/projects/projectCreation.test.mjs`, add `templates.repositoryDtag` to the existing Sprout fixture and append the new cases.
Do not change production code yet.

```js
import assert from "node:assert/strict";
import test from "node:test";

import {
  buildInitialProjectEventTemplates,
  isUnsupportedProjectKindError,
  repositoryDtagFromName,
} from "./projectCreation.ts";

const OWNER = "a".repeat(64);
const CHANNEL = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50";

test("buildInitialProjectEventTemplates emits a NIP-MP project", () => {
  const templates = buildInitialProjectEventTemplates({
    accessChannelId: CHANNEL,
    cloneUrl: "https://relay.example/git/owner/sprout.git",
    description: "A multi-repository workspace",
    name: "Sprout",
    ownerPubkey: OWNER,
    webUrl: "https://example.com/sprout",
  });

  assert.equal(templates.dtag, "sprout");
  assert.equal(templates.repositoryDtag, "sprout");
  assert.equal(templates.project.kind, 30621);
  assert.equal(templates.repository.kind, 30617);
  assert.deepEqual(templates.project.tags, [
    ["d", "sprout"],
    ["name", "Sprout"],
    ["buzz-channel", CHANNEL],
    ["description", "A multi-repository workspace"],
    ["a", `30617:${OWNER}:sprout`],
  ]);
  assert.equal(templates.project.content, "");
  assert.deepEqual(templates.repository.tags, [
    ["d", "sprout"],
    ["name", "Sprout"],
    ["buzz-channel", CHANNEL],
    ["description", "A multi-repository workspace"],
    ["clone", "https://relay.example/git/owner/sprout.git"],
    ["web", "https://example.com/sprout"],
  ]);
});

test("omitted whitespace-only and undefined repositoryName copy the project identity", () => {
  const base = {
    accessChannelId: CHANNEL,
    name: "Sprout",
    ownerPubkey: OWNER,
  };
  const omitted = buildInitialProjectEventTemplates(base);
  const whitespace = buildInitialProjectEventTemplates({
    ...base,
    repositoryName: "   ",
  });
  const undef = buildInitialProjectEventTemplates({
    ...base,
    repositoryName: undefined,
  });
  assert.deepEqual(whitespace.project.tags, omitted.project.tags);
  assert.deepEqual(whitespace.repository.tags, omitted.repository.tags);
  assert.equal(whitespace.repositoryDtag, omitted.dtag);
  assert.deepEqual(undef.repository.tags, omitted.repository.tags);
});

test("repositoryName anhle128/buzz names only the first repository", () => {
  const templates = buildInitialProjectEventTemplates({
    accessChannelId: CHANNEL,
    name: "Bee Garden",
    ownerPubkey: OWNER,
    repositoryName: "  anhle128/buzz  ",
  });

  assert.equal(templates.dtag, "bee-garden");
  assert.equal(templates.repositoryDtag, "anhle128-buzz");
  assert.deepEqual(templates.project.tags, [
    ["d", "bee-garden"],
    ["name", "Bee Garden"],
    ["buzz-channel", CHANNEL],
    ["a", `30617:${OWNER}:anhle128-buzz`],
  ]);
  assert.deepEqual(templates.repository.tags, [
    ["d", "anhle128-buzz"],
    ["name", "anhle128/buzz"],
    ["buzz-channel", CHANNEL],
  ]);
  assert.match(templates.repositoryAddress, /:anhle128-buzz$/);
  assert.equal(templates.project.content, "");
});

test("filled repositoryName that slugs to the project dtag still uses the repo display string", () => {
  const templates = buildInitialProjectEventTemplates({
    accessChannelId: CHANNEL,
    name: "Bee Garden",
    ownerPubkey: OWNER,
    repositoryName: "bee-garden",
  });
  assert.equal(templates.dtag, "bee-garden");
  assert.equal(templates.repositoryDtag, "bee-garden");
  assert.deepEqual(templates.project.tags.slice(0, 2), [
    ["d", "bee-garden"],
    ["name", "Bee Garden"],
  ]);
  assert.deepEqual(templates.repository.tags.slice(0, 2), [
    ["d", "bee-garden"],
    ["name", "bee-garden"],
  ]);
});

test("buildInitialProjectEventTemplates rejects names without an identifier", () => {
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        name: "!!!",
        ownerPubkey: OWNER,
      }),
    /letters or numbers/,
  );
});

test("filled repositoryName /// does not fall back to the project name", () => {
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        ownerPubkey: OWNER,
        repositoryName: "///",
      }),
    /letters or numbers/,
  );
});

test("repository name slug must not exceed 64 characters", () => {
  assert.doesNotThrow(() =>
    buildInitialProjectEventTemplates({
      accessChannelId: CHANNEL,
      name: "Bee Garden",
      ownerPubkey: OWNER,
      repositoryName: "a".repeat(64),
    }),
  );
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        ownerPubkey: OWNER,
        repositoryName: "a".repeat(65),
      }),
    /64 characters/,
  );
});

test("project slug may exceed 64 characters when repositoryName is omitted", () => {
  const name = "a".repeat(65);
  const templates = buildInitialProjectEventTemplates({
    accessChannelId: CHANNEL,
    name,
    ownerPubkey: OWNER,
  });
  assert.equal(templates.dtag, name);
  assert.equal(templates.repositoryDtag, name);
});

test("repository name must not exceed 256 bytes", () => {
  assert.doesNotThrow(() =>
    buildInitialProjectEventTemplates({
      accessChannelId: CHANNEL,
      name: "Bee Garden",
      ownerPubkey: OWNER,
      repositoryName: "a".repeat(256),
    }),
  );
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        ownerPubkey: OWNER,
        repositoryName: "a".repeat(257),
      }),
    /256 bytes/,
  );
});

test("buildInitialProjectEventTemplates enforces the description tag byte limit", () => {
  assert.doesNotThrow(() =>
    buildInitialProjectEventTemplates({
      accessChannelId: CHANNEL,
      description: "🙂".repeat(512),
      name: "Sprout",
      ownerPubkey: OWNER,
    }),
  );
  assert.throws(
    () =>
      buildInitialProjectEventTemplates({
        accessChannelId: CHANNEL,
        description: "🙂".repeat(513),
        name: "Sprout",
        ownerPubkey: OWNER,
      }),
    /2,048 bytes/,
  );
});

test("repositoryDtagFromName replaces non-alphanumerics with dashes", () => {
  assert.equal(repositoryDtagFromName("anhle128/buzz"), "anhle128-buzz");
  assert.equal(repositoryDtagFromName("Bee Garden"), "bee-garden");
});

test("isUnsupportedProjectKindError recognizes relay kind compatibility failures", () => {
  assert.equal(
    isUnsupportedProjectKindError(
      new Error("restricted: unknown event kind 30621"),
    ),
    true,
  );
  assert.equal(
    isUnsupportedProjectKindError(new Error("mock project event rejection")),
    false,
  );
});
```

At the top of `desktop/src/features/projects/projectRepositoryCreation.test.mjs`, add this import and test (keep every existing test):

```js
import { repositoryDtagFromName } from "./projectRepositoryCreation.ts";

test("repositoryDtagFromName re-export slugs owner/repo display names", () => {
  assert.equal(repositoryDtagFromName("anhle128/buzz"), "anhle128-buzz");
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/projectCreation.test.mjs src/features/projects/projectRepositoryCreation.test.mjs
```

Expected: FAIL because `repositoryDtag` is missing, `repositoryName` is ignored, `repositoryDtagFromName` is not exported from `projectCreation.ts`, and `///` / 65 `a` / 257-byte names do not throw.

- [ ] **Step 3: Implement the shared slugger and override**

Replace the private slugger and builder in `desktop/src/features/projects/projectCreation.ts` with:

```ts
export function repositoryDtagFromName(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function projectDtagFromName(name: string): string {
  return repositoryDtagFromName(name);
}

export type InitialProjectEventTemplates = {
  dtag: string;
  repositoryDtag: string;
  project: ProjectEventTemplate;
  repository: ProjectEventTemplate;
  repositoryAddress: string;
};

export function buildInitialProjectEventTemplates({
  accessChannelId,
  cloneUrl,
  description,
  name,
  ownerPubkey,
  repositoryName,
  webUrl,
}: {
  accessChannelId: string;
  cloneUrl?: string;
  description?: string;
  name: string;
  ownerPubkey: string;
  repositoryName?: string;
  webUrl?: string;
}): InitialProjectEventTemplates {
  const normalizedName = name.trim();
  if (!normalizedName) {
    throw new Error("Project name is required.");
  }
  if (new TextEncoder().encode(normalizedName).byteLength > 256) {
    throw new Error("Project name must not exceed 256 bytes.");
  }
  const dtag = projectDtagFromName(normalizedName);
  if (!dtag) {
    throw new Error("Project name must include letters or numbers.");
  }

  const normalizedRepositoryName = repositoryName?.trim() ?? "";
  let repositoryDisplayName = normalizedName;
  let repositoryDtag = dtag;
  if (normalizedRepositoryName) {
    if (new TextEncoder().encode(normalizedRepositoryName).byteLength > 256) {
      throw new Error("Repository name must not exceed 256 bytes.");
    }
    repositoryDtag = repositoryDtagFromName(normalizedRepositoryName);
    if (!repositoryDtag) {
      throw new Error("Repository name must include letters or numbers.");
    }
    if (repositoryDtag.length > 64) {
      throw new Error("Repository name slug must not exceed 64 characters.");
    }
    repositoryDisplayName = normalizedRepositoryName;
  }

  const normalizedOwner = ownerPubkey.trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(normalizedOwner)) {
    throw new Error("Project owner public key is invalid.");
  }

  const normalizedDescription = description?.trim() ?? "";
  if (new TextEncoder().encode(normalizedDescription).byteLength > 2_048) {
    throw new Error("Project description must not exceed 2,048 bytes.");
  }
  const repositoryTags: string[][] = [
    ["d", repositoryDtag],
    ["name", repositoryDisplayName],
  ];
  const projectTags: string[][] = [
    ["d", dtag],
    ["name", normalizedName],
  ];
  const normalizedAccessChannelId = accessChannelId.trim();
  if (!isValidProjectChannelId(normalizedAccessChannelId)) {
    throw new Error("Repository access channel is invalid.");
  }
  repositoryTags.push(["buzz-channel", normalizedAccessChannelId]);
  projectTags.push(["buzz-channel", normalizedAccessChannelId]);
  if (normalizedDescription) {
    repositoryTags.push(["description", normalizedDescription]);
    projectTags.push(["description", normalizedDescription]);
  }
  const normalizedCloneUrl = cloneUrl?.trim();
  if (normalizedCloneUrl) {
    repositoryTags.push(["clone", normalizedCloneUrl]);
  }
  const normalizedWebUrl = webUrl?.trim();
  if (normalizedWebUrl) {
    repositoryTags.push(["web", normalizedWebUrl]);
  }

  const repositoryAddress = `${KIND_REPO_ANNOUNCEMENT}:${normalizedOwner}:${repositoryDtag}`;
  projectTags.push(["a", repositoryAddress]);

  return {
    dtag,
    repositoryDtag,
    project: {
      kind: KIND_PROJECT_ANNOUNCEMENT,
      content: "",
      tags: projectTags,
    },
    repository: {
      kind: KIND_REPO_ANNOUNCEMENT,
      content: normalizedDescription,
      tags: repositoryTags,
    },
    repositoryAddress,
  };
}
```

Keep `isUnsupportedProjectKindError` and the `ProjectEventTemplate` type unchanged.

In `desktop/src/features/projects/projectRepositoryCreation.ts`, delete the local `repositoryDtagFromName` function body.
Import it from `./projectCreation` next to `ProjectEventTemplate` and re-export it:

```ts
import {
  repositoryDtagFromName,
  type ProjectEventTemplate,
} from "./projectCreation";

export { repositoryDtagFromName };
```

Do not change `buildAddedRepositoryEventTemplatesFromHead`.
Do not add a 64-character check on **Add repository**.

- [ ] **Step 4: Run tests to verify they pass**

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/projectCreation.test.mjs src/features/projects/projectRepositoryCreation.test.mjs src/features/projects/useAddProjectRepository.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/projects/projectCreation.ts desktop/src/features/projects/projectCreation.test.mjs desktop/src/features/projects/projectRepositoryCreation.ts desktop/src/features/projects/projectRepositoryCreation.test.mjs && (test -f .gitnexus/run.cjs && node .gitnexus/run.cjs detect || true) && git commit -s -m "$(cat <<'EOF'
feat(projects): slug the first repository independently of the project name

Allow Create project templates to take an optional repositoryName so the
kind:30617 d/name tags can differ from the kind:30621 project identity.
EOF
)"
```

---

### Task 2: Clobber guard on distinct first-repo slugs

**Files:**
- Modify: `desktop/src/features/projects/useCreateProject.ts`
- Create: `desktop/src/features/projects/useCreateProject.test.mjs`
- Test: `desktop/src/features/projects/useCreateProject.test.mjs`

**Interfaces:**
- Consumes: `buildInitialProjectEventTemplates` from Task 1 (`templates.dtag`, `templates.repositoryDtag`, `templates.repositoryAddress`)
- Produces:
  - `export type CreateProjectInput = { accessChannelId: string; name: string; description?: string; cloneUrl?: string; webUrl?: string; repositoryName?: string }`
  - `export type CreateProjectDeps = { fetchEvents: (filter: RelaySubscriptionFilter) => Promise<RelayEvent[]>; fetchProjects: typeof fetchProjects; getIdentity: () => Promise<{ pubkey: string }>; publishEvent: (event: RelayEvent, timeoutMessage: string, sendErrorMessage: string) => Promise<unknown>; signRelayEvent: typeof signRelayEvent }`
  - `export async function createProject(input: CreateProjectInput, resumableProjectIds: Set<string>, deps?: Partial<CreateProjectDeps>): Promise<CreateProjectResult>`
- `useCreateProjectMutation` still calls `createProject(input, resumableProjectIdsRef.current)` with production defaults.

- [ ] **Step 1: Write the failing mutation tests**

Create `desktop/src/features/projects/useCreateProject.test.mjs`:

```js
import assert from "node:assert/strict";
import test from "node:test";

import { createProject } from "./useCreateProject.ts";

const OWNER = "a".repeat(64);
const CHANNEL = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50";

function makeRepoHead(dtag) {
  return {
    id: "b".repeat(64),
    kind: 30617,
    pubkey: OWNER,
    created_at: 1,
    content: "",
    tags: [
      ["d", dtag],
      ["name", dtag],
    ],
    sig: "0".repeat(128),
  };
}

test("distinct repo slug with an existing 30617 head throws and publishes nothing", async () => {
  const published = [];
  let signed = 0;
  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        repositoryName: "anhle128/buzz",
      },
      new Set(),
      {
        getIdentity: async () => ({ pubkey: OWNER }),
        fetchProjects: async () => [],
        fetchEvents: async (filter) => {
          assert.deepEqual(filter.kinds, [30617]);
          assert.deepEqual(filter.authors, [OWNER]);
          assert.deepEqual(filter["#d"], ["anhle128-buzz"]);
          assert.equal(filter.limit, 1);
          return [makeRepoHead("anhle128-buzz")];
        },
        signRelayEvent: async () => {
          signed += 1;
          throw new Error("must not sign");
        },
        publishEvent: async (event) => {
          published.push(event);
          throw new Error("must not publish");
        },
      },
    ),
    /A repository named "anhle128-buzz" already exists \(as a standalone repository or in another project\)\. Choose a different name to avoid overwriting it\./,
  );
  assert.equal(published.length, 0);
  assert.equal(signed, 0);
});

test("same project and repo slugs do not query kind 30617", async () => {
  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Sprout",
      },
      new Set(),
      {
        getIdentity: async () => ({ pubkey: OWNER }),
        fetchProjects: async () => [],
        fetchEvents: async () => {
          throw new Error(
            "must not query kind 30617 when repositoryDtag === dtag",
          );
        },
        signRelayEvent: async () => {
          throw new Error("stop-after-clobber");
        },
        publishEvent: async () => {
          throw new Error("must not publish");
        },
      },
    ),
    /stop-after-clobber/,
  );
});

test("filled repositoryName that slugs to the project dtag does not query kind 30617", async () => {
  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        repositoryName: "bee-garden",
      },
      new Set(),
      {
        getIdentity: async () => ({ pubkey: OWNER }),
        fetchProjects: async () => [],
        fetchEvents: async () => {
          throw new Error(
            "must not query kind 30617 when repositoryDtag === dtag",
          );
        },
        signRelayEvent: async () => {
          throw new Error("stop-after-clobber");
        },
        publishEvent: async () => {
          throw new Error("must not publish");
        },
      },
    ),
    /stop-after-clobber/,
  );
});

test("distinct repo slug with no existing 30617 head proceeds to sign", async () => {
  const filters = [];
  await assert.rejects(
    createProject(
      {
        accessChannelId: CHANNEL,
        name: "Bee Garden",
        repositoryName: "anhle128/buzz",
      },
      new Set(),
      {
        getIdentity: async () => ({ pubkey: OWNER }),
        fetchProjects: async () => [],
        fetchEvents: async (filter) => {
          filters.push(filter);
          return [];
        },
        signRelayEvent: async () => {
          throw new Error("stop-after-clobber");
        },
        publishEvent: async () => {
          throw new Error("must not publish");
        },
      },
    ),
    /stop-after-clobber/,
  );
  assert.equal(filters.length, 1);
  assert.deepEqual(filters[0]["#d"], ["anhle128-buzz"]);
});
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/useCreateProject.test.mjs
```

Expected: FAIL because `createProject` is not exported and no `30617` clobber fetch exists.

- [ ] **Step 3: Export `createProject` and add the clobber fetch**

Update `desktop/src/features/projects/useCreateProject.ts` as follows.

Add imports:

```ts
import { KIND_REPO_ANNOUNCEMENT } from "@/shared/constants/kinds";
import type { RelayEvent } from "@/shared/api/types";
```

`relayClient` is already imported.

Replace the input type and the `createProject` function (keep `CreateProjectResult` and `useCreateProjectMutation` as they are, except the mutation still calls the now-exported function):

```ts
export type CreateProjectInput = {
  accessChannelId: string;
  name: string;
  description?: string;
  cloneUrl?: string;
  webUrl?: string;
  repositoryName?: string;
};

type FetchEventsInput = Parameters<(typeof relayClient)["fetchEvents"]>[0];

export type CreateProjectDeps = {
  fetchEvents: (filter: FetchEventsInput) => Promise<RelayEvent[]>;
  fetchProjects: typeof fetchProjects;
  getIdentity: () => Promise<{ pubkey: string }>;
  publishEvent: (
    event: RelayEvent,
    timeoutMessage: string,
    sendErrorMessage: string,
  ) => Promise<unknown>;
  signRelayEvent: typeof signRelayEvent;
};

/** Publishes a project announcement and its initial NIP-34 repository. */
export async function createProject(
  input: CreateProjectInput,
  resumableProjectIds: Set<string>,
  deps: Partial<CreateProjectDeps> = {},
): Promise<CreateProjectResult> {
  const {
    fetchEvents = relayClient.fetchEvents.bind(relayClient),
    fetchProjects: fetchProjectsFn = fetchProjects,
    getIdentity: getIdentityFn = getIdentity,
    publishEvent = relayClient.publishEvent.bind(relayClient),
    signRelayEvent: signRelayEventFn = signRelayEvent,
  } = deps;

  const identity = await getIdentityFn();
  const templates = buildInitialProjectEventTemplates({
    ...input,
    ownerPubkey: identity.pubkey,
  });
  const existing = await fetchProjectsFn();
  const ownerPubkey = identity.pubkey.toLowerCase();
  const existingProject = existing.find(
    (project) =>
      project.owner.toLowerCase() === ownerPubkey &&
      project.dtag === templates.dtag,
  );
  const projectId = `${ownerPubkey}:${templates.dtag}`;
  const canResume = resumableProjectIds.has(projectId);
  if (existingProject && !canResume) {
    throw new Error(`You already have a project named "${templates.dtag}".`);
  }
  if (existingProject && !existingProject.legacy) {
    if (
      existingProject.repositories.some(
        (repository) => repository.repoAddress === templates.repositoryAddress,
      )
    ) {
      resumableProjectIds.delete(projectId);
      return { project: existingProject };
    }
    throw new Error(`You already have a project named "${templates.dtag}".`);
  }

  if (!existingProject && templates.repositoryDtag !== templates.dtag) {
    const existingRepoHeads = await fetchEvents({
      kinds: [KIND_REPO_ANNOUNCEMENT],
      authors: [ownerPubkey],
      "#d": [templates.repositoryDtag],
      limit: 1,
    });
    if (existingRepoHeads.length > 0) {
      throw new Error(
        `A repository named "${templates.repositoryDtag}" already exists (as a standalone repository or in another project). Choose a different name to avoid overwriting it.`,
      );
    }
  }

  resumableProjectIds.add(projectId);
  const projectEvent = await signRelayEventFn(templates.project);

  let repositoryEvent = null;
  if (!existingProject) {
    repositoryEvent = await signRelayEventFn(templates.repository);
    await publishEvent(
      repositoryEvent,
      "Timed out creating the initial repository.",
      "Failed to create the initial repository.",
    );
  }

  try {
    await publishEvent(
      projectEvent,
      "Timed out creating project.",
      "Failed to create project.",
    );
  } catch (error) {
    if (!isUnsupportedProjectKindError(error)) throw error;

    const [legacyProject] = existingProject?.legacy
      ? [existingProject]
      : buildProjectReadModels({
          projectEvents: [],
          repositoryEvents: repositoryEvent ? [repositoryEvent] : [],
          relayOrigin: getCachedRelayOrigin(),
        });
    if (!legacyProject) throw error;

    resumableProjectIds.delete(projectId);
    return {
      project: legacyProject,
      compatibilityWarning:
        "The repository was created, but this relay does not support multi-repository projects yet. It will appear as a standalone project.",
    };
  }

  const [project] = repositoryEvent
    ? buildProjectReadModels({
        projectEvents: [projectEvent],
        repositoryEvents: [repositoryEvent],
        relayOrigin: getCachedRelayOrigin(),
      })
    : (await fetchProjectsFn()).filter(
        (candidate) =>
          candidate.owner.toLowerCase() === ownerPubkey &&
          candidate.dtag === templates.dtag &&
          !candidate.legacy,
      );
  if (!project) {
    throw new Error("The project was created but could not be read.");
  }
  resumableProjectIds.delete(projectId);
  return { project };
}
```

Place the new `30617` fetch **after** the existing project-collision / resume branches and **before** `signRelayEvent`.
Do not run it when `existingProject` is set.
Do not run it when `templates.repositoryDtag === templates.dtag`.
Do not change publish order (`30617` then `30621`) or the unsupported-`30621` fallback.

- [ ] **Step 4: Run tests to verify they pass**

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/useCreateProject.test.mjs src/features/projects/projectCreation.test.mjs
```

Expected: PASS.

Then typecheck the desktop package:

```bash
. ./bin/activate-hermit && cd desktop && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/projects/useCreateProject.ts desktop/src/features/projects/useCreateProject.test.mjs && (test -f .gitnexus/run.cjs && node .gitnexus/run.cjs detect || true) && git commit -s -m "$(cat <<'EOF'
feat(projects): refuse to clobber an existing first repository slug

When Create project uses a repository d-tag that differs from the project
slug, query kind:30617 directly and abort before sign or publish if a head
already exists.
EOF
)"
```

---

### Task 3: Optional Repository name field on the create dialog

**Files:**
- Modify: `desktop/src/features/projects/ui/CreateProjectDialog.tsx`
- Modify: `desktop/tests/e2e/project-commit-detail.spec.ts` (add one test after `project creation is idempotent after a lost publish acknowledgement`)
- Test: `desktop/tests/e2e/project-commit-detail.spec.ts`

**Interfaces:**
- Consumes: `CreateProjectInput.repositoryName?: string` from Task 2
- Produces: dialog state `repositoryName: string`, `id="create-project-repository-name"`, `data-testid="create-project-repository-name"`
- `onCreate` receives `repositoryName` only when `repositoryName.trim()` is non-empty; otherwise the property is omitted.

- [ ] **Step 1: Write the failing Playwright test**

Add this test to `desktop/tests/e2e/project-commit-detail.spec.ts` immediately after the lost-ack create-project test.
Reuse the file’s existing `enableProjectsFeature` helper.
Do not add screenshots.

```ts
test("create project dialog exposes an optional repository name", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await installMockBridge(page);
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await page.getByTestId("open-projects-view").click();
  await page.getByTestId("projects-create-menu").hover();
  await page.getByRole("menuitem", { name: "Project" }).click();

  const repositoryName = page.getByTestId("create-project-repository-name");
  await expect(page.getByTestId("create-project-dialog")).toBeVisible();
  await expect(repositoryName).toBeVisible();
  await expect(repositoryName).toHaveAttribute("placeholder", "bee-garden-ios");
  await expect(repositoryName).toHaveValue("");

  const label = page.locator("label[for='create-project-repository-name']");
  await expect(label).toContainText("Repository name");
  await expect(label).toContainText("Optional");
  await expect(page.getByText("Defaults to the project name.")).toBeVisible();

  const fieldOrder = await page
    .locator("#create-project-form [data-testid]")
    .evaluateAll((elements) =>
      elements
        .map((element) => element.getAttribute("data-testid"))
        .filter((id): id is string => Boolean(id)),
    );
  expect(fieldOrder.indexOf("create-project-name")).toBeLessThan(
    fieldOrder.indexOf("create-project-access-channel"),
  );
  expect(fieldOrder.indexOf("create-project-access-channel")).toBeLessThan(
    fieldOrder.indexOf("create-project-description"),
  );
  expect(fieldOrder.indexOf("create-project-description")).toBeLessThan(
    fieldOrder.indexOf("create-project-repository-name"),
  );
  expect(fieldOrder.indexOf("create-project-repository-name")).toBeLessThan(
    fieldOrder.indexOf("create-project-clone-url"),
  );
  expect(fieldOrder.indexOf("create-project-clone-url")).toBeLessThan(
    fieldOrder.indexOf("create-project-web-url"),
  );

  await page.getByTestId("create-project-name").fill("bee-garden");
  await expect(page.getByTestId("create-project-submit")).toBeEnabled();

  await repositoryName.fill("anhle128/buzz");
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("create-project-dialog")).toBeHidden();
  await page.getByTestId("projects-create-menu").hover();
  await page.getByRole("menuitem", { name: "Project" }).click();
  await expect(page.getByTestId("create-project-repository-name")).toHaveValue(
    "",
  );
});
```

Do not change the existing tests that only fill **Name**.
Those tests must keep proving the empty-field path.

- [ ] **Step 2: Run the new e2e test to verify it fails**

If a stale preview is listening on port 4173, stop it first so Playwright does not serve an old `dist`.

```bash
. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test --project=smoke tests/e2e/project-commit-detail.spec.ts -g "create project dialog exposes an optional repository name"
```

Expected: FAIL because `create-project-repository-name` is not in the dialog.

- [ ] **Step 3: Add the field to `CreateProjectDialog`**

In `desktop/src/features/projects/ui/CreateProjectDialog.tsx`:

1. Add `const [repositoryName, setRepositoryName] = React.useState("");` next to the other field state.
2. Inside the existing `useEffect` that runs when `open` becomes true, add `setRepositoryName("");` next to `setName("")`.
3. In `handleSubmit`, pass `repositoryName` only when trimmed:

```ts
      await onCreate({
        accessChannelId,
        name: trimmedName,
        description: description.trim() || undefined,
        repositoryName: repositoryName.trim() || undefined,
        cloneUrl: cloneUrl.trim() || undefined,
        webUrl: webUrl.trim() || undefined,
      });
```

4. Insert this block **after** the Description field and **before** the Initial repository clone URL field.
Use the same single-line chrome as the other `Input` fields (`CREATE_FIELD_SHELL_CLASS`, `CREATE_FIELD_CONTROL_CLASS`, `CREATE_LABEL_OPTIONAL_CLASS`).

```tsx
          <div className="space-y-1.5">
            <label
              className="text-sm font-medium text-foreground"
              htmlFor="create-project-repository-name"
            >
              Repository name
              <span className={CREATE_LABEL_OPTIONAL_CLASS}>Optional</span>
            </label>
            <div
              className={cn(
                "flex min-h-11 items-center px-3",
                CREATE_FIELD_SHELL_CLASS,
              )}
            >
              <Input
                autoCapitalize="none"
                autoComplete="off"
                autoCorrect="off"
                className={cn(
                  "h-8 px-0 py-0 leading-6",
                  CREATE_FIELD_CONTROL_CLASS,
                )}
                data-testid="create-project-repository-name"
                disabled={isCreating}
                id="create-project-repository-name"
                onChange={(event) => {
                  setRepositoryName(event.target.value);
                  setErrorMessage(null);
                }}
                placeholder="bee-garden-ios"
                spellCheck={false}
                value={repositoryName}
              />
            </div>
            <p className="text-xs text-muted-foreground">
              Defaults to the project name.
            </p>
          </div>
```

Do not require the field for submit.
Do not change the **Name** placeholder.
Do not regroup the dialog.
Keep the existing red `errorMessage` line; builder and clobber errors already surface there through `error.message`.

- [ ] **Step 4: Rebuild e2e assets and verify green**

Kill anything bound to 4173, then rebuild.
`reuseExistingServer: true` will otherwise serve the previous `dist`.

```bash
. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test --project=smoke tests/e2e/project-commit-detail.spec.ts
```

Expected: PASS, including the older create-project cases that only fill **Name**.

Also run unit tests and desktop checks for the touched files:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/projectCreation.test.mjs src/features/projects/useCreateProject.test.mjs src/features/projects/projectRepositoryCreation.test.mjs src/features/projects/useAddProjectRepository.test.mjs && pnpm exec biome check src/features/projects/projectCreation.ts src/features/projects/projectCreation.test.mjs src/features/projects/projectRepositoryCreation.ts src/features/projects/projectRepositoryCreation.test.mjs src/features/projects/useCreateProject.ts src/features/projects/useCreateProject.test.mjs src/features/projects/ui/CreateProjectDialog.tsx && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/projects/ui/CreateProjectDialog.tsx desktop/tests/e2e/project-commit-detail.spec.ts && (test -f .gitnexus/run.cjs && node .gitnexus/run.cjs detect || true) && git commit -s -m "$(cat <<'EOF'
feat(projects): add optional repository name to create project

Surface an optional first-repository name on the Desktop create dialog so
authors can keep the project name independent of the initial git id.
EOF
)"
```

---

## Acceptance criteria

- Creating **Bee Garden** with repository name `anhle128/buzz` builds project `d=bee-garden` / `name=Bee Garden` and repo `d=anhle128-buzz` / `name=anhle128/buzz`, with project `a` = `30617:<owner>:anhle128-buzz`.
- Creating with only **Name** still produces one shared slug and the same Sprout fixture tags as before this change (`d` / `name` / `a` still `sprout`).
- Typing `///` throws `Repository name must include letters or numbers.` and does not fall back to the project name.
- A repository display that slugs to 65 `a` characters throws `Repository name slug must not exceed 64 characters.`
- A 257-byte repository display throws `Repository name must not exceed 256 bytes.`
- When `repositoryDtag !== dtag` and the `30617` fetch returns a head, create throws `A repository named "<repositoryDtag>" already exists (as a standalone repository or in another project). Choose a different name to avoid overwriting it.` and signs/publishes nothing.
- When `repositoryDtag === dtag`, create does not issue that extra `30617` fetch.
- The dialog shows **Repository name** with the Optional marker, helper `Defaults to the project name.`, placeholder `bee-garden-ios`, and `data-testid="create-project-repository-name"`.
- Submit stays enabled without filling **Repository name** once **Name** is filled and a channel is selected.
- The field resets to `""` when the dialog opens.
- Existing create-project e2e tests that only fill **Name** stay green.
- Relay repo ids produced by this path never contain `/`.

## Validation commands

Run from the worktree root after Hermit is active.

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/projectCreation.test.mjs src/features/projects/projectRepositoryCreation.test.mjs src/features/projects/useCreateProject.test.mjs src/features/projects/useAddProjectRepository.test.mjs
```

```bash
. ./bin/activate-hermit && cd desktop && pnpm typecheck && pnpm exec biome check src/features/projects/projectCreation.ts src/features/projects/projectCreation.test.mjs src/features/projects/projectRepositoryCreation.ts src/features/projects/projectRepositoryCreation.test.mjs src/features/projects/useCreateProject.ts src/features/projects/useCreateProject.test.mjs src/features/projects/ui/CreateProjectDialog.tsx
```

```bash
. ./bin/activate-hermit && cd desktop && pnpm build:e2e && pnpm exec playwright test --project=smoke tests/e2e/project-commit-detail.spec.ts
```

Optional package-wide unit sweep after the last task:

```bash
. ./bin/activate-hermit && cd desktop && pnpm test
```

## Spec coverage

| Spec requirement | Task |
|------------------|------|
| Empty field copies project `d` / `name` | Task 1 Sprout + whitespace tests |
| `anhle128/buzz` → `name` slash kept, `d=anhle128-buzz` | Task 1 Bee Garden test |
| Project identity stays on **Name** | Task 1 Bee Garden test |
| `repositoryAddress` / `a` tag use repo `d` | Task 1 Bee Garden test |
| `templates.dtag` is the project slug | Task 1 |
| `templates.repositoryDtag` added | Task 1 |
| Shared `repositoryDtagFromName`, no second slugger | Task 1 move + re-export test |
| `projectDtagFromName` calls the shared slugger | Task 1 |
| `///` throws letters-or-numbers, no fallback | Task 1 |
| Repo slug > 64 throws | Task 1 |
| Repo display > 256 bytes throws | Task 1 |
| Existing `!!!` and description byte tests still pass | Task 1 |
| Clobber fetch when slugs differ | Task 2 |
| No extra `30617` fetch when slugs match | Task 2 |
| Exact clobber copy; publish nothing | Task 2 |
| Field order, Optional, helper, placeholder, testid | Task 3 |
| Omit `repositoryName` when trim is empty | Task 3 submit payload |
| Reset to `""` on open | Task 3 e2e reopen |
| Submit enabled without the field | Task 3 e2e |
| Existing Name-only e2e stays green | Task 3 full spec run |
| No screenshots, no Add repository / CLI / mobile / NIP edits | All tasks omit those files |

## Open Questions

1. The design file header still says `Status: Draft — awaiting review`.
This plan treats [2026-08-18-create-project-repository-name-design.md](../specs/2026-08-18-create-project-repository-name-design.md) as the approved product contract because the user asked for an implementation plan from that brainstorm.
If review later forbids `/` in the display `name` tag, stop and revise Task 1 rather than silently changing the slugger.

2. Project slugs are still allowed to exceed 64 characters when **Repository name** is omitted (today’s builder has no project-slug length check).
Provisional default: do not add a project-slug 64-character check, and do not add that check to **Add repository**.
The new 64-character error applies only to a filled **Repository name**.
Verified by the Task 1 test `project slug may exceed 64 characters when repositoryName is omitted`.
