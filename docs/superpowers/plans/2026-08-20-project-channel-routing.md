# Project Channel Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route the Desktop Projects Open Discussion control and Channels tab to the resolvable `buzz-channel` bound to the selected repository, falling back to the project container binding.

**Architecture:** Keep `buzz-channel` as NIP-MP metadata and resolve it only against the viewer-readable channel list returned by `useChannelsQuery`.
The pure resolver applies repository-first precedence, rejects malformed, unavailable, archived, and DM channels, and supplies one resolved channel to both project surfaces.
The project screen passes that resolved value through the existing chrome `actions` slot and the workspace Channels tab without changing relay routing, event writers, or share-link vocabulary.

**Tech Stack:** Desktop TypeScript, React 19, TanStack Query, TanStack Router, Node `node:test`, Playwright mock bridge, Biome, Hermit, and `just`.

**Spec:** [docs/nips/NIP-MP.md](../../nips/NIP-MP.md), especially Metadata interpretation, Authority, and Routing.

**Approved brainstorm input:** `2026-08-20-buzz-project-channel-routing-design`.

**Design provenance:** The workflow record contains the approved brainstorm name but no standalone design file, so the approved implementation decisions are repeated explicitly in this plan.

## Global Constraints

- Desktop is the only product surface in scope.
- `buzz-channel` is the only canonical project and repository discussion-binding tag.
- Do not add `h` or `project-channel` tags to kind:30617 or kind:30621 events.
- Do not change `desktop/src/features/projects/projectCreation.ts` or `desktop/src/features/projects/projectRepositoryCreation.ts`; both current writers already emit `buzz-channel`.
- Do not change `desktop/src/features/projects/projectModels.ts`; current readers already parse valid `buzz-channel` values into `repository.channelId` and `project.projectChannelId`.
- Do not change relay ingest, git transport, git push policy, CLI commands, mobile, or the web client.
- Do not change the meaning of `buzz://project?...&tab=channels` or `buzz://repo?...&tab=channels`; those links continue to open the Projects workspace Channels tab.
- The selected repository's resolvable `channelId` wins over `project.projectChannelId`.
- If the selected repository binding is absent or unusable, fall back to the project binding.
- A binding is usable only when it is a valid UUID, appears in `useChannelsQuery` data, has `archivedAt === null`, and has `channelType !== "dm"`.
- Stream and forum channels are both valid because the existing project creation and repository management UIs allow every non-DM access channel.
- An open channel the viewer has not joined is valid when it appears in `useChannelsQuery` data; the resolver must not inspect `isMember`.
- When NIP-50 finds no repository mentions, the bound channel still appears first in the Channels tab with the secondary label `Linked discussion channel`.
- When NIP-50 already found the bound channel, move that existing row first and preserve its hit count, activity time, and participants.
- When no binding resolves, render no Open Discussion control and preserve the current FTS-only Channels tab behavior.
- The Open Discussion control uses a Hash icon, `aria-label="Open Discussion"`, and `data-testid="project-open-discussion"`.
- Use the existing `ProjectDetailChrome.actions` slot so the control renders immediately before `ShareLinkButton` without adding a new chrome API.
- `ProjectDetailScreen.tsx` and `ProjectWorkspaceTabs.tsx` must remain at or below the repository's 1000-line ceiling.
- Every new public TypeScript export needs a doc comment.
- Activate Hermit before every Git or validation command with `. ./bin/activate-hermit && ...`.
- Create every commit with `git commit -s`.
- Follow TDD for each behavior: add the named failing test, run it and confirm the expected failure, implement only that task, and run the same test green.
- Do not add production code for a later task to make an earlier task pass.

## Verified Repository Facts

- `desktop/src/features/projects/projectCreation.ts` writes the same access-channel UUID to both the initial kind:30617 repository and kind:30621 project as `buzz-channel`.
- `desktop/src/features/projects/projectRepositoryCreation.ts` preserves and updates repository `buzz-channel` metadata.
- `desktop/src/features/projects/projectModels.ts` reads only `buzz-channel`, validates it with `isValidProjectChannelId`, and exposes it as `Repository.channelId` and `Project.projectChannelId`.
- `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` selects the active repository with `selectProjectRepository` and already owns `goChannel`.
- `desktop/src/features/projects/ui/DiscussionChannels.tsx` currently derives the Channels tab only from NIP-50 search hits.
- `desktop/src/features/channels/hooks.ts` exposes the viewer-readable channel list through `useChannelsQuery`.
- `desktop/src/features/projects/ui/ProjectDetailChrome.tsx` already renders its optional `actions` node immediately before `ShareLinkButton`.
- `desktop/src/testing/e2eBridge.ts` binds the mock `buzz` repository to `9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50` (`#general`) while the mock kind:30621 project has no `buzz-channel`, which proves repository-first routing without changing fixtures.
- `desktop/tests/e2e/projects-v3-screenshots.spec.ts` currently asserts that Open Discussion is absent, so that assertion must become positive in the chrome task.

## GitNexus Gates

- Before editing an existing function, class, method, or exported type, run `impact({ target: "<symbol>", direction: "upstream" })` and report direct callers, affected processes, and risk.
- Stop and warn the user before editing when GitNexus reports HIGH or CRITICAL risk.
- If the index is stale and `.gitnexus/run.cjs` exists, run `. ./bin/activate-hermit && node .gitnexus/run.cjs analyze` from the repository root.
- If no `.gitnexus/run.cjs` exists, run `. ./bin/activate-hermit && npx gitnexus analyze` from the repository root before retrying the MCP call.
- Before every commit, stage only that task's files and run `detect_changes({ scope: "staged" })`.
- Before final handoff, run `detect_changes({ scope: "compare", base_ref: "main" })`.
- If a required GitNexus MCP call remains unavailable after refreshing the index, stop before editing or committing rather than substituting `rg` for the repository gate.

## File Map

| File | Responsibility |
|---|---|
| Create: `desktop/src/features/projects/lib/projectDiscussionChannel.ts` | Pure repository-first channel resolver and bound-row merge |
| Create: `desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs` | Resolver and merge unit coverage |
| Create: `desktop/src/features/projects/useProjectDiscussionChannel.ts` | React hook over `useChannelsQuery` |
| Modify: `desktop/src/features/projects/ui/DiscussionChannels.tsx` | Merge and render the bound channel in the Channels tab |
| Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx` | Carry the resolved channel into `DiscussionChannelsPanel` |
| Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Resolve once and feed both routing surfaces |
| Create: `desktop/src/features/projects/ui/OpenDiscussionButton.tsx` | Icon control for opening the resolved channel |
| Create: `desktop/tests/e2e/project-channel-routing.spec.ts` | Mock-bridge routing tests for the Channels row and chrome control |
| Modify: `desktop/tests/e2e/projects-v3-screenshots.spec.ts` | Update the screenshot contract to require Open Discussion |
| Modify: `desktop/playwright.config.ts` | Register the new smoke spec |

## Out of Scope

- Legacy model changes such as copying `Repository.channelId` into `Project.projectChannelId` for implicit projects are unnecessary because the resolver receives the selected repository directly.
- `desktop/src/features/projects/lib/projectLabels.ts` is unused and is not part of routing.
- Branch-channel creation, archive automation, and git-event fan-in to Stream remain future work from `VISION_PROJECTS.md`.
- Changing the relay's global-only handling of kind:30621 is forbidden.
- Auto-creating channels or publishing chat messages is forbidden.

---

### Task 1: Resolve and merge the discussion channel

**Files:**

- Create: `desktop/src/features/projects/lib/projectDiscussionChannel.ts`
- Create: `desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs`

**Interfaces:**

- Consumes: `isValidProjectChannelId` from `desktop/src/features/projects/projectModels.ts`.
- Consumes: `DiscussionChannel` from `desktop/src/features/projects/lib/discussionChannels.ts`.
- Consumes: `Channel` from `desktop/src/shared/api/types.ts`.
- Produces: `export type ProjectDiscussionChannel = { id: string; name: string }`.
- Produces: `resolveProjectDiscussionChannel(input): ProjectDiscussionChannel | null` with the exact signature below.
- Produces: `mergeBoundDiscussionChannel(bound, discovered): DiscussionChannel[]` with the exact signature below.

- [ ] **Step 1: Write the failing resolver and merge tests**

Create `desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs` with this complete content:

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  mergeBoundDiscussionChannel,
  resolveProjectDiscussionChannel,
} from "./projectDiscussionChannel.ts";

const GENERAL = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50";
const DESIGN = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb51";
const MISSING = "11111111-1111-4111-8111-111111111111";

function channel(id, overrides = {}) {
  return {
    id,
    name: id === GENERAL ? "general" : "design",
    archivedAt: null,
    channelType: "stream",
    ...overrides,
  };
}

test("repository binding wins over the project binding", () => {
  assert.deepEqual(
    resolveProjectDiscussionChannel({
      repositoryChannelId: GENERAL,
      projectChannelId: DESIGN,
      channels: [channel(GENERAL), channel(DESIGN)],
    }),
    { id: GENERAL, name: "general" },
  );
});

test("project binding is the fallback when the repository has no binding", () => {
  assert.deepEqual(
    resolveProjectDiscussionChannel({
      repositoryChannelId: null,
      projectChannelId: DESIGN,
      channels: [channel(DESIGN)],
    }),
    { id: DESIGN, name: "design" },
  );
});

test("an unusable repository binding falls through to the project binding", () => {
  assert.deepEqual(
    resolveProjectDiscussionChannel({
      repositoryChannelId: DESIGN,
      projectChannelId: GENERAL,
      channels: [
        channel(GENERAL),
        channel(DESIGN, { archivedAt: "2026-08-20T00:00:00Z" }),
      ],
    }),
    { id: GENERAL, name: "general" },
  );
});

test("malformed and viewer-unresolved bindings do not resolve", () => {
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: "general",
      projectChannelId: null,
      channels: [channel(GENERAL)],
    }),
    null,
  );
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: MISSING,
      projectChannelId: null,
      channels: [channel(GENERAL)],
    }),
    null,
  );
});

test("archived and DM bindings do not resolve", () => {
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: GENERAL,
      projectChannelId: null,
      channels: [channel(GENERAL, { archivedAt: "2026-08-20T00:00:00Z" })],
    }),
    null,
  );
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: GENERAL,
      projectChannelId: null,
      channels: [channel(GENERAL, { channelType: "dm" })],
    }),
    null,
  );
});

test("non-DM channels resolve without a membership check", () => {
  assert.deepEqual(
    resolveProjectDiscussionChannel({
      repositoryChannelId: GENERAL,
      projectChannelId: null,
      channels: [channel(GENERAL, { channelType: "forum", isMember: false })],
    }),
    { id: GENERAL, name: "general" },
  );
});

test("a zero-hit bound channel is inserted first", () => {
  const discovered = [
    {
      id: DESIGN,
      name: "design",
      messageCount: 4,
      lastActivityAt: 200,
      participants: ["aa"],
    },
  ];
  const merged = mergeBoundDiscussionChannel(
    { id: GENERAL, name: "general" },
    discovered,
  );
  assert.deepEqual(merged[0], {
    id: GENERAL,
    name: "general",
    messageCount: 0,
    lastActivityAt: 0,
    participants: [],
  });
  assert.equal(merged[1], discovered[0]);
});

test("an already-discovered bound channel moves first without losing hit data", () => {
  const general = {
    id: GENERAL,
    name: "general",
    messageCount: 1,
    lastActivityAt: 1,
    participants: [],
  };
  const design = {
    id: DESIGN,
    name: "design",
    messageCount: 9,
    lastActivityAt: 9,
    participants: ["aa"],
  };
  const merged = mergeBoundDiscussionChannel({ id: DESIGN, name: "design" }, [
    general,
    design,
  ]);
  assert.equal(merged.length, 2);
  assert.equal(merged[0], design);
  assert.equal(merged[1], general);
});

test("no bound channel preserves the discovered order", () => {
  const discovered = [
    {
      id: DESIGN,
      name: "design",
      messageCount: 2,
      lastActivityAt: 2,
      participants: [],
    },
  ];
  assert.deepEqual(mergeBoundDiscussionChannel(null, discovered), discovered);
});
```

- [ ] **Step 2: Run the test and verify the expected red state**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs
```

Expected: FAIL with `ERR_MODULE_NOT_FOUND` for `projectDiscussionChannel.ts`.

- [ ] **Step 3: Implement the pure resolver and merge**

Create `desktop/src/features/projects/lib/projectDiscussionChannel.ts` with this complete content:

```ts
import type { DiscussionChannel } from "@/features/projects/lib/discussionChannels";
import { isValidProjectChannelId } from "@/features/projects/projectModels";
import type { Channel } from "@/shared/api/types";

/** Viewer-resolvable channel used by project discussion routing. */
export type ProjectDiscussionChannel = {
  id: string;
  name: string;
};

type ResolvableChannel = Pick<
  Channel,
  "id" | "name" | "archivedAt" | "channelType"
>;

function isUsableDiscussionChannel(channel: ResolvableChannel): boolean {
  return channel.archivedAt === null && channel.channelType !== "dm";
}

/**
 * Resolves the selected repository binding first and the project binding second.
 * Invalid or unreadable metadata is omitted instead of becoming a broken link.
 */
export function resolveProjectDiscussionChannel(input: {
  repositoryChannelId: string | null | undefined;
  projectChannelId: string | null | undefined;
  channels: readonly ResolvableChannel[];
}): ProjectDiscussionChannel | null {
  const candidates = [input.repositoryChannelId, input.projectChannelId];

  for (const candidate of candidates) {
    if (!candidate || !isValidProjectChannelId(candidate)) continue;
    const channel = input.channels.find((item) => item.id === candidate);
    if (!channel || !isUsableDiscussionChannel(channel)) continue;
    return { id: channel.id, name: channel.name };
  }

  return null;
}

/**
 * Pins the bound channel before FTS-discovered rows while preserving hit data.
 */
export function mergeBoundDiscussionChannel(
  bound: ProjectDiscussionChannel | null,
  discovered: readonly DiscussionChannel[],
): DiscussionChannel[] {
  if (!bound) return [...discovered];

  const existing = discovered.find((channel) => channel.id === bound.id);
  const remainder = discovered.filter((channel) => channel.id !== bound.id);
  if (existing) return [existing, ...remainder];

  return [
    {
      id: bound.id,
      name: bound.name,
      messageCount: 0,
      lastActivityAt: 0,
      participants: [],
    },
    ...remainder,
  ];
}
```

- [ ] **Step 4: Run the focused test green and lint the new files**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs && pnpm exec biome check src/features/projects/lib/projectDiscussionChannel.ts src/features/projects/lib/projectDiscussionChannel.test.mjs
```

Expected: PASS with no Biome diagnostics.

- [ ] **Step 5: Stage, inspect the GitNexus change map, and commit**

Run:

```bash
. ./bin/activate-hermit && git add desktop/src/features/projects/lib/projectDiscussionChannel.ts desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs
```

Run `detect_changes({ scope: "staged" })` and confirm that only the new resolver symbols and their test are reported.

Run:

```bash
. ./bin/activate-hermit && git commit -s -m "feat(projects): resolve bound discussion channels"
```

---

### Task 2: Pin the bound channel in the Channels tab

**Files:**

- Create: `desktop/src/features/projects/useProjectDiscussionChannel.ts`
- Modify: `desktop/src/features/projects/ui/DiscussionChannels.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`
- Create: `desktop/tests/e2e/project-channel-routing.spec.ts`
- Modify: `desktop/playwright.config.ts`

**Interfaces:**

- Consumes: `resolveProjectDiscussionChannel` and `mergeBoundDiscussionChannel` from Task 1.
- Produces: `useProjectDiscussionChannel(project, repository): ProjectDiscussionChannel | null`.
- Changes: `DiscussionChannelsPanel` requires `boundChannel: ProjectDiscussionChannel | null`.
- Changes: `WorkspaceTabs` requires `boundChannel: ProjectDiscussionChannel | null`.

- [ ] **Step 1: Run required impact analysis before touching existing symbols**

Run `impact({ target: "DiscussionChannelsPanel", direction: "upstream" })`.

Run `impact({ target: "WorkspaceTabs", direction: "upstream" })`.

Run `impact({ target: "ProjectDetailScreen", direction: "upstream" })`.

Report each direct caller, affected process, and risk before editing.

Expected repository shape: `DiscussionChannelsPanel` is called by `WorkspaceTabs`, and `WorkspaceTabs` is called by `ProjectDetailScreen`.

- [ ] **Step 2: Write and register the failing Channels-tab smoke test**

Create `desktop/tests/e2e/project-channel-routing.spec.ts` with this complete content:

```ts
import { expect, test } from "@playwright/test";

import { installMockBridge } from "../helpers/bridge";

const GENERAL_CHANNEL_ID = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50";

async function enableProjectsFeature(page: import("@playwright/test").Page) {
  await page.addInitScript(() => {
    window.localStorage.setItem(
      "buzz-feature-overrides-v1",
      JSON.stringify({ projects: true }),
    );
  });
}

async function openBuzzProject(page: import("@playwright/test").Page) {
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await page.getByTestId("open-projects-view").click();
  await page.getByTestId("projects-section-projects").click();
  const projectEntry = page
    .locator(
      '[data-testid="project-card-buzz"], [data-testid="project-row-buzz"]',
    )
    .first();
  await expect(projectEntry).toBeVisible({ timeout: 10_000 });
  await projectEntry.click();
}

test("Channels tab pins and opens the zero-hit repository binding", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await installMockBridge(page);
  await openBuzzProject(page);

  await page.getByRole("tab", { name: "Channels" }).click();
  const list = page.getByTestId("discussion-channels");
  const firstRow = list.locator("li").first();
  const bound = firstRow.getByTestId("project-bound-discussion-channel");
  await expect(bound).toBeVisible({ timeout: 10_000 });
  await expect(bound).toContainText("#general");
  await expect(bound).toContainText("Linked discussion channel");

  await bound.click();
  await expect(page).toHaveURL(
    new RegExp(`/#/channels/${GENERAL_CHANNEL_ID}$`),
  );
  await expect(page.getByTestId("chat-title")).toHaveText("general");
});
```

Add `"**/project-channel-routing.spec.ts"` to the smoke `testMatch` array in `desktop/playwright.config.ts` immediately after `"**/projects-v3-screenshots.spec.ts"`.

- [ ] **Step 3: Run the smoke test and verify the expected red state**

Run:

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- project-channel-routing.spec.ts
```

Expected: FAIL because the current Channels tab has no `project-bound-discussion-channel` row when the repository search has zero hits.

Do not proceed if the test fails earlier during app boot; fix only test setup until the failure reaches the missing row.

- [ ] **Step 4: Add the channel-resolution hook**

Create `desktop/src/features/projects/useProjectDiscussionChannel.ts` with this complete content:

```ts
import * as React from "react";

import { useChannelsQuery } from "@/features/channels/hooks";
import type { Project, Repository } from "@/features/projects/hooks";
import {
  type ProjectDiscussionChannel,
  resolveProjectDiscussionChannel,
} from "@/features/projects/lib/projectDiscussionChannel";

/** Resolves the viewer-readable discussion channel for the active project repository. */
export function useProjectDiscussionChannel(
  project: Project | null | undefined,
  repository: Repository | null | undefined,
): ProjectDiscussionChannel | null {
  const channelsQuery = useChannelsQuery();
  return React.useMemo(
    () =>
      resolveProjectDiscussionChannel({
        repositoryChannelId: repository?.channelId,
        projectChannelId: project?.projectChannelId,
        channels: channelsQuery.data ?? [],
      }),
    [channelsQuery.data, project?.projectChannelId, repository?.channelId],
  );
}
```

- [ ] **Step 5: Replace the Channels panel with bound-channel-aware rendering**

Add this import to `desktop/src/features/projects/ui/DiscussionChannels.tsx`:

```ts
import {
  mergeBoundDiscussionChannel,
  type ProjectDiscussionChannel,
} from "@/features/projects/lib/projectDiscussionChannel";
```

Replace only the existing `DiscussionChannelsPanel` function with this implementation and leave `DiscussedInChannels` unchanged:

```tsx
export function DiscussionChannelsPanel({
  boundChannel,
  query,
}: {
  boundChannel: ProjectDiscussionChannel | null;
  query: string;
}) {
  const {
    channels: discoveredChannels,
    isLoading,
    isTruncated,
  } = useDiscussionChannels(query);
  const { goChannel } = useAppNavigation();
  const channels = React.useMemo(
    () => mergeBoundDiscussionChannel(boundChannel, discoveredChannels),
    [boundChannel, discoveredChannels],
  );
  const channelName = useChannelNameLookup(channels.length > 0);
  const profilesQuery = useUsersBatchQuery(
    channels.flatMap((channel) => channel.participants),
    { enabled: channels.length > 0 },
  );
  const profiles = profilesQuery.data?.profiles;

  if (isLoading && boundChannel === null) {
    return (
      <p className="px-4 py-6 text-sm text-muted-foreground">
        Searching channel discussions…
      </p>
    );
  }
  if (channels.length === 0) {
    return (
      <p className="px-4 py-6 text-sm text-muted-foreground">
        No channels reference this repository yet. Paste its link (or a PR or
        issue link) in a channel and it will show up here.
      </p>
    );
  }

  return (
    <div>
      <ul
        className="divide-y divide-border/50"
        data-testid="discussion-channels"
      >
        {channels.map((channel) => {
          const name = channelName(channel.id, channel.name);
          const isBound = boundChannel?.id === channel.id;
          const speakers = channel.participants
            .slice(0, 2)
            .map((pubkey) => resolveUserLabel({ profiles, pubkey }));
          const others = channel.participants.length - speakers.length;
          return (
            <li className="relative" key={channel.id}>
              <button
                className="flex w-full min-w-0 items-center gap-2.5 px-4 py-3 text-left transition-colors hover:bg-muted/30"
                data-testid={
                  isBound ? "project-bound-discussion-channel" : undefined
                }
                onClick={() => void goChannel(channel.id)}
                type="button"
              >
                <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-muted/50">
                  <Hash className="h-4 w-4 text-muted-foreground" />
                </span>
                <span className="min-w-0 flex-1 space-y-1">
                  <span className="block truncate text-sm font-medium text-foreground">
                    #{name}
                  </span>
                  <span className="block truncate text-xs text-muted-foreground">
                    {isBound && channel.messageCount === 0
                      ? "Linked discussion channel"
                      : `${speakers.join(", ")}${
                          others > 0
                            ? ` and ${others} ${
                                others === 1 ? "other" : "others"
                              }`
                            : ""
                        } · ${channel.messageCount}${isTruncated ? "+" : ""} ${
                          channel.messageCount === 1 ? "message" : "messages"
                        }`}
                  </span>
                </span>
                <ParticipantFacepile
                  participants={channel.participants}
                  profiles={profiles}
                />
                <span
                  className="hidden w-20 shrink-0 text-right text-xs text-muted-foreground sm:block"
                  data-testid="project-channel-row-date"
                  title={
                    channel.lastActivityAt > 0
                      ? new Date(
                          channel.lastActivityAt * 1_000,
                        ).toLocaleString()
                      : undefined
                  }
                >
                  {channel.lastActivityAt > 0
                    ? relativeTime(channel.lastActivityAt)
                    : ""}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      {isTruncated ? (
        <p className="border-t border-border/50 px-4 py-2 text-xs text-muted-foreground">
          Showing the latest {DISCUSSION_SEARCH_LIMIT} mentions; totals may be
          higher.
        </p>
      ) : null}
    </div>
  );
}
```

- [ ] **Step 6: Thread the resolved value through the workspace**

Add this type import to `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx`:

```ts
import type { ProjectDiscussionChannel } from "@/features/projects/lib/projectDiscussionChannel";
```

Insert this field immediately after the opening line of the `WorkspaceTabs` destructuring, before `commitDiff`:

```ts
boundChannel,
```

Insert this declaration immediately after the opening line of the inline props type, before `commitDiff: ProjectRepoDiff | null | undefined;`:

```ts
boundChannel: ProjectDiscussionChannel | null;
```

Replace the existing Channels-tab call with:

```tsx
<DiscussionChannelsPanel
  boundChannel={boundChannel}
  query={repositoryDiscussionQuery(project)}
/>
```

Import `useProjectDiscussionChannel` in `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`:

```ts
import { useProjectDiscussionChannel } from "@/features/projects/useProjectDiscussionChannel";
```

Immediately after the existing `selectProjectRepository` call, resolve the channel once:

```ts
const repository = selectProjectRepository(project, routeRepositoryId);
const discussionChannel = useProjectDiscussionChannel(project, repository);
```

Insert this prop immediately after the opening `<WorkspaceTabs` line:

```tsx
boundChannel={discussionChannel}
```

- [ ] **Step 7: Run the unit, type, size, and smoke tests green**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs && pnpm typecheck && pnpm check:file-sizes && pnpm exec biome check src/features/projects/lib/projectDiscussionChannel.ts src/features/projects/lib/projectDiscussionChannel.test.mjs src/features/projects/useProjectDiscussionChannel.ts src/features/projects/ui/DiscussionChannels.tsx src/features/projects/ui/ProjectWorkspaceTabs.tsx src/features/projects/ui/ProjectDetailScreen.tsx tests/e2e/project-channel-routing.spec.ts playwright.config.ts
```

Expected: PASS, and both `ProjectDetailScreen.tsx` and `ProjectWorkspaceTabs.tsx` remain within the file-size gate.

Run:

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- project-channel-routing.spec.ts
```

Expected: PASS with `#general` first, `Linked discussion channel` visible, a `/#/channels/9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50` URL, and chat title `general`.

- [ ] **Step 8: Stage, inspect the GitNexus change map, and commit**

Run:

```bash
. ./bin/activate-hermit && git add desktop/src/features/projects/useProjectDiscussionChannel.ts desktop/src/features/projects/ui/DiscussionChannels.tsx desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx desktop/src/features/projects/ui/ProjectDetailScreen.tsx desktop/tests/e2e/project-channel-routing.spec.ts desktop/playwright.config.ts
```

Run `detect_changes({ scope: "staged" })` and confirm that the affected flow is limited to project detail rendering, project Channels-tab rendering, and channel navigation.

Run:

```bash
. ./bin/activate-hermit && git commit -s -m "feat(projects): pin the bound discussion channel"
```

---

### Task 3: Add the Open Discussion chrome control

**Files:**

- Create: `desktop/src/features/projects/ui/OpenDiscussionButton.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`
- Modify: `desktop/tests/e2e/project-channel-routing.spec.ts`
- Modify: `desktop/tests/e2e/projects-v3-screenshots.spec.ts`

**Interfaces:**

- Consumes: `ProjectDiscussionChannel` from Task 1.
- Consumes: the existing `ProjectDetailChrome.actions` node and `ProjectDetailScreen`'s existing `goChannel` callback.
- Produces: `OpenDiscussionButton({ channel, onOpen })`, which renders nothing for `null` and otherwise invokes `onOpen(channel.id)`.

- [ ] **Step 1: Run required impact analysis before editing the screen**

Run `impact({ target: "ProjectDetailScreen", direction: "upstream" })` again against the post-Task-2 index state.

Report direct callers, affected processes, and risk before editing.

- [ ] **Step 2: Add the failing Open Discussion smoke test and positive screenshot assertion**

Append this test to `desktop/tests/e2e/project-channel-routing.spec.ts`:

```ts
test("Open Discussion routes the repository binding to Stream", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await installMockBridge(page);
  await openBuzzProject(page);

  const openDiscussion = page.getByRole("button", {
    name: "Open Discussion",
  });
  await expect(openDiscussion).toBeVisible({ timeout: 10_000 });
  await openDiscussion.click();

  await expect(page).toHaveURL(
    new RegExp(`/#/channels/${GENERAL_CHANNEL_ID}$`),
  );
  await expect(page.getByTestId("chat-title")).toHaveText("general");
});
```

In `desktop/tests/e2e/projects-v3-screenshots.spec.ts`, replace:

```ts
await expect(
  page.getByRole("button", { name: "Open Discussion" }),
).toHaveCount(0);
```

with:

```ts
await expect(
  page.getByRole("button", { name: "Open Discussion" }),
).toBeVisible();
```

- [ ] **Step 3: Run the new test and verify the expected red state**

Run:

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- project-channel-routing.spec.ts --grep "Open Discussion"
```

Expected: FAIL because no button named Open Discussion exists.

Do not replace this with a source-text assertion; the Playwright test must prove the real control and router integration.

- [ ] **Step 4: Implement the icon control**

Create `desktop/src/features/projects/ui/OpenDiscussionButton.tsx` with this complete content:

```tsx
import { Hash } from "lucide-react";

import type { ProjectDiscussionChannel } from "@/features/projects/lib/projectDiscussionChannel";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";

/** Opens the resolved discussion channel from the project detail chrome. */
export function OpenDiscussionButton({
  channel,
  onOpen,
}: {
  channel: ProjectDiscussionChannel | null;
  onOpen: (channelId: string) => void;
}) {
  if (!channel) return null;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          aria-label="Open Discussion"
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted/40 hover:text-foreground focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring"
          data-testid="project-open-discussion"
          onClick={() => onOpen(channel.id)}
          type="button"
        >
          <Hash className="h-3.5 w-3.5" />
        </button>
      </TooltipTrigger>
      <TooltipContent>{`Open #${channel.name}`}</TooltipContent>
    </Tooltip>
  );
}
```

- [ ] **Step 5: Use the existing chrome actions slot**

Import the button in `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`:

```ts
import { OpenDiscussionButton } from "./OpenDiscussionButton";
```

Add this `actions` prop to the existing `ProjectDetailChrome` call before `activeTabCrumb`:

```tsx
<ProjectDetailChrome
  actions={
    <OpenDiscussionButton
      channel={discussionChannel}
      onOpen={(channelId) => {
        void goChannel(channelId);
      }}
    />
  }
  activeTabCrumb={activeTabCrumb}
```

Do not modify `ProjectDetailChrome.tsx`; its existing actions slot already places the control immediately before the share button.

- [ ] **Step 6: Run focused validation and both smoke specs**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs && pnpm typecheck && pnpm check:file-sizes && pnpm check:px-text && pnpm exec biome check src/features/projects/lib/projectDiscussionChannel.ts src/features/projects/lib/projectDiscussionChannel.test.mjs src/features/projects/useProjectDiscussionChannel.ts src/features/projects/ui/OpenDiscussionButton.tsx src/features/projects/ui/DiscussionChannels.tsx src/features/projects/ui/ProjectWorkspaceTabs.tsx src/features/projects/ui/ProjectDetailScreen.tsx tests/e2e/project-channel-routing.spec.ts tests/e2e/projects-v3-screenshots.spec.ts playwright.config.ts
```

Expected: PASS with no arbitrary text-size violations and no file-size violation.

Run:

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- project-channel-routing.spec.ts
```

Expected: both routing tests PASS.

Run:

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- projects-v3-screenshots.spec.ts
```

Expected: PASS, and `01-workspace-overview.png` now includes the Hash control next to the project share control.

If a stale preview server is serving an old E2E build, identify the listener with `. ./bin/activate-hermit && lsof -nP -iTCP:4173 -sTCP:LISTEN`, terminate only that PID, and rerun the command so `pnpm build:e2e` serves the current code.

- [ ] **Step 7: Stage, inspect the GitNexus change map, and commit**

Run:

```bash
. ./bin/activate-hermit && git add desktop/src/features/projects/ui/OpenDiscussionButton.tsx desktop/src/features/projects/ui/ProjectDetailScreen.tsx desktop/tests/e2e/project-channel-routing.spec.ts desktop/tests/e2e/projects-v3-screenshots.spec.ts
```

Run `detect_changes({ scope: "staged" })` and confirm that the affected flow is limited to project detail chrome rendering and navigation to the resolved channel.

Run:

```bash
. ./bin/activate-hermit && git commit -s -m "feat(projects): open the bound discussion channel"
```

---

## Acceptance Criteria

- The selected repository's usable `buzz-channel` takes precedence over the project container's usable `buzz-channel`.
- The project container binding is used when the selected repository binding is absent, malformed, unresolved, archived, or a DM.
- A malformed, viewer-unresolved, archived, or DM-only binding produces no Open Discussion control and no synthetic Channels row.
- A viewer-readable stream or forum binding resolves even when `isMember` is false.
- The mock `buzz` project displays an Open Discussion Hash control with accessible name `Open Discussion`.
- Clicking Open Discussion navigates to `/#/channels/9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50` and renders chat title `general`.
- The Channels tab pins `#general` as its first row when NIP-50 has zero matching hits.
- The zero-hit row displays `Linked discussion channel` and no fabricated message count, participant, or activity date.
- Clicking the bound Channels row navigates to the same `#general` route.
- An already-discovered bound row keeps its real NIP-50 count, participants, and activity timestamp while moving first.
- With no resolved binding, the existing loading, empty, and FTS-discovered Channels behavior is unchanged.
- Project and repository share links keep `tab=channels` workspace semantics because no share-link or router vocabulary file changes.
- No relay, CLI, mobile, web, event-writer, or project-model file changes.
- `ProjectDetailScreen.tsx` and `ProjectWorkspaceTabs.tsx` remain within the 1000-line limit.

## Final Handoff Gates

- [ ] Run the repository-wide gate from the repository root:

```bash
. ./bin/activate-hermit && just ci
```

Expected: PASS.

- [ ] Confirm the implementation changed only the planned files:

```bash
. ./bin/activate-hermit && git diff --name-only main...HEAD
```

Expected files:

```text
desktop/playwright.config.ts
desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs
desktop/src/features/projects/lib/projectDiscussionChannel.ts
desktop/src/features/projects/ui/DiscussionChannels.tsx
desktop/src/features/projects/ui/OpenDiscussionButton.tsx
desktop/src/features/projects/ui/ProjectDetailScreen.tsx
desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx
desktop/src/features/projects/useProjectDiscussionChannel.ts
desktop/tests/e2e/project-channel-routing.spec.ts
desktop/tests/e2e/projects-v3-screenshots.spec.ts
docs/superpowers/plans/2026-08-20-project-channel-routing.md
```

The plan path appears because this branch already contains the reviewed plan commit on top of `main` before implementation begins.

- [ ] Run `detect_changes({ scope: "compare", base_ref: "main" })` and confirm that no relay ingest, git authorization, share-link parsing, mobile, or web execution flow is affected.

- [ ] Report the device and workflow exercised as: Desktop Playwright mock bridge, mock community, open `buzz` project, open Channels, route through bound `#general`, return to project, and route through Open Discussion.

## Open Questions

None.

The repository facts resolve channel type, precedence, membership, icon treatment, share-link behavior, and mock fixture choices without an implementation-time decision.

## Self-Review

- Spec coverage: NIP-MP metadata-only routing, broken-link omission, repository-first precedence, project fallback, Channels-tab pinning, Open Discussion routing, and viewer-readable resolution are each assigned to a test and implementation task.
- Scope coverage: existing canonical writers and readers remain untouched, and the file whitelist excludes relay, CLI, mobile, web, project model, project labels, and share-link code.
- TDD order: Task 1 starts with a missing-module unit failure, Task 2 starts with a missing-row Playwright failure, and Task 3 starts with a missing-control Playwright failure.
- Type consistency: `ProjectDiscussionChannel` has one definition and is consumed by the hook, panel, workspace props, and button.
- Placeholder scan: no unresolved marker, deferred implementation, generic error-handling instruction, or unnamed test remains.
