# Project Channel Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route Desktop Projects discussion navigation to the bound `buzz-channel` (selected repository first, then the project container) so Open Discussion and the Channels tab open a real Stream channel instead of a dead label or FTS-only empty state.

**Architecture:** Keep `buzz-channel` as NIP-MP metadata, not an `h` routing tag.
Add a pure resolver that maps repository/project bindings onto a channel the viewer can actually see, then wire that result into the project chrome button and the Channels tab.
Do not post git events into chat, do not auto-create branch channels, and do not change relay ingest.

**Tech Stack:** Desktop TypeScript, React 19, TanStack Router `goChannel`, Node `node:test` (`*.test.mjs`), Playwright mock-bridge smoke, Biome, Hermit, `just`.

**Spec:** The named design file `docs/superpowers/specs/2026-08-20-buzz-project-channel-routing-design.md` is not in this tree.
This plan is the executable contract.
Requirements come from [VISION_PROJECTS.md](../../../VISION_PROJECTS.md), [NIP-MP](../../nips/NIP-MP.md) (`buzz-channel` is metadata; unresolvable values must not become broken links), GitHub issue [#3611](https://github.com/block/buzz/issues/3611), and the current Desktop writers/readers in `projectCreation.ts`, `projectRepositoryCreation.ts`, and `projectModels.ts`.

**Product contract:** [VISION.md](../../../VISION.md) says the app is the channels where the work happens.
[VISION_PROJECTS.md](../../../VISION_PROJECTS.md) documents `buzz-channel` as the project/repo discussion binding.
[NIP-MP](../../nips/NIP-MP.md) forbids treating that tag as channel scope and forbids dropping a project or showing a broken link when the UUID cannot be resolved.

## Global Constraints

- Desktop only.
- Canonical tag is `buzz-channel`.
- Do not write `h` or `project-channel` on kind:30617 or kind:30621.
- Do not change relay ingest, git push policy, or `crates/buzz-relay/src/api/git/binding.rs`.
- Do not change CLI `buzz repos bind` / `buzz repos create --channel` except if a test fixture needs a UUID.
- Do not auto-create a channel per branch.
- Do not publish kind:9/11 messages for pushes, PRs, or issues.
- Do not change `buzz://` share-link vocabulary: `tab=channels` still means the Projects workspace Channels tab.
- Do not add mobile routing.
- Do not add web client routing.
- Selected repository `channelId` wins over `project.projectChannelId`.
- Hide Open Discussion when the bound UUID is missing, malformed, archived, a DM, or absent from the viewer's channel list.
- Open channels the viewer has not joined still count as resolvable if they appear in `useChannelsQuery` data.
- `ProjectDetailScreen.tsx` must stay at or under the 1000-line ratchet.
- `ProjectWorkspaceTabs.tsx` must stay at or under the 1000-line ratchet.
- New public TypeScript exports need a doc comment.
- Activate Hermit in every shell: `. ./bin/activate-hermit && …`.
- CWD does not persist across tool calls.
- Sign every commit with `git commit -s`.
- TDD: write the named failing test, run it red, write the minimum code, run it green, then refactor.
- Do not implement a later task's production code to make an earlier task pass.

## GitNexus Gates

GitNexus MCP tools and `.gitnexus/run.cjs` are absent in this planning session.
The implementation session must still follow the repository GitNexus policy in AGENTS.md.

- Before changing an existing function, class, method, or exported type, run `impact({ target: "<symbol>", direction: "upstream" })`.
- Report direct callers, affected execution flows, and the returned risk before editing.
- Stop before editing if risk is HIGH or CRITICAL.
- If the implementation session has no GitNexus index, run `npx gitnexus analyze`, then resume in a session where the GitNexus MCP tools are loaded.
- `rg` is useful corroborating evidence but is not a substitute for the required GitNexus impact call.
- Before each commit, stage the task files and run `detect_changes({ scope: "staged" })`.
- Before final handoff, run `detect_changes({ scope: "compare", base_ref: "main" })`.
- There is no GitNexus CLI `detect` command, so do not run `node .gitnexus/run.cjs detect`.
- If `detect_changes` is unavailable, stop before the commit, leave the changes uncommitted, and do not begin the next task until the repository gate is available.

## Resolved Implementation Decisions

- Resolver file is `desktop/src/features/projects/lib/projectDiscussionChannel.ts`.
- Hook file is `desktop/src/features/projects/useProjectDiscussionChannel.ts`.
- Button file is `desktop/src/features/projects/ui/OpenDiscussionButton.tsx`.
- Accessible name is exactly `Open Discussion` (aria-label), matching `projects-v3-screenshots.spec.ts` and issue #3611.
- Chrome control is an icon button with a Hash icon, height `h-7`, same chrome treatment as `ShareLinkButton`.
- Test id is `project-open-discussion`.
- Bound channel on the Channels tab is labeled `Linked discussion channel` when it has zero FTS hits.
- Legacy implicit projects copy `repository.channelId` into `project.projectChannelId`.
- `getDiscussionLabel` takes `ProjectDiscussionChannel | null`, not `Project`.
- `desktop/src/features/projects/lib/projectLabels.ts` re-exports `getDiscussionLabel` from the resolver module.
- Mock kind:30621 for `buzz` does not need a new tag for the happy-path e2e: the mock kind:30617 already binds `9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50` (`general`).
- `projects-v3-screenshots.spec.ts` currently asserts Open Discussion is absent; this plan reverses that assertion.

## File map

| File | Role |
|------|------|
| Create: `desktop/src/features/projects/lib/projectDiscussionChannel.ts` | Pure resolver + Channels-tab merge |
| Create: `desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs` | Unit tests for resolver, merge, labels |
| Modify: `desktop/src/features/projects/lib/projectLabels.ts` | Label helper consumes the resolver result |
| Modify: `desktop/src/features/projects/projectModels.ts` | Legacy implicit projects inherit repo `channelId` |
| Modify: `desktop/src/features/projects/projectModels.test.mjs` | Cover legacy `projectChannelId` inheritance |
| Create: `desktop/src/features/projects/useProjectDiscussionChannel.ts` | Hook over `useChannelsQuery` |
| Create: `desktop/src/features/projects/ui/OpenDiscussionButton.tsx` | Chrome control |
| Modify: `desktop/src/features/projects/ui/ProjectDetailChrome.tsx` | Render the control left of share |
| Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` | Resolve + `goChannel` + pass bound id into tabs |
| Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx` | Pass bound id into the Channels panel |
| Modify: `desktop/src/features/projects/ui/DiscussionChannels.tsx` | Pin bound channel; keep FTS rows |
| Modify: `desktop/tests/e2e/projects-v3-screenshots.spec.ts` | Expect Open Discussion to exist |
| Create: `desktop/tests/e2e/project-channel-routing.spec.ts` | Smoke: button opens `#general` |
| Modify: `desktop/playwright.config.ts` | Register the smoke spec |

## Out of scope

- Branch-as-channel creation from VISION_PROJECTS.
- Routing git ref updates, CI, or NIP-34 issues/PRs into Stream as messages.
- Changing `tab=channels` share links to call `goChannel`.
- Web, mobile, CLI bind UX, relay ACL, or NIP-MP envelope rules.
- Restoring Desktop readers for `h` / `project-channel` (writers already emit `buzz-channel`; `projectModels.ts` already reads only `buzz-channel`).

---

### Task 1: Resolve the bound discussion channel

**Files:**
- Create: `desktop/src/features/projects/lib/projectDiscussionChannel.ts`
- Create: `desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs`
- Modify: `desktop/src/features/projects/lib/projectLabels.ts`
- Modify: `desktop/src/features/projects/projectModels.ts` (`repositoryToLegacyProject`)
- Modify: `desktop/src/features/projects/projectModels.test.mjs`

**Interfaces:**
- Consumes: `isValidProjectChannelId` from `desktop/src/features/projects/projectModels.ts`, `DiscussionChannel` from `desktop/src/features/projects/lib/discussionChannels.ts`, `Channel` from `desktop/src/shared/api/types.ts`
- Produces:
  - `export type ProjectDiscussionChannel = { id: string; name: string; source: "repository" | "project" }`
  - `export function resolveProjectDiscussionChannel(input: { repositoryChannelId: string | null | undefined; projectChannelId: string | null | undefined; channels: readonly Pick<Channel, "id" | "name" | "archivedAt" | "channelType">[] }): ProjectDiscussionChannel | null`
  - `export function mergeBoundDiscussionChannel(bound: ProjectDiscussionChannel | null, discovered: readonly DiscussionChannel[]): DiscussionChannel[]`
  - `export function getDiscussionLabel(channel: ProjectDiscussionChannel | null): string`

- [ ] **Step 1: Write the failing tests**

Create `desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs` with this full content:

```js
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  getDiscussionLabel,
  mergeBoundDiscussionChannel,
  resolveProjectDiscussionChannel,
} from "./projectDiscussionChannel.ts";

const GENERAL = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50";
const DESIGN = "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb51";
const NOT_A_UUID = "general";

function channel(id, overrides = {}) {
  return {
    id,
    name: id === GENERAL ? "general" : "design",
    archivedAt: null,
    channelType: "stream",
    ...overrides,
  };
}

test("repository binding wins over project binding", () => {
  const resolved = resolveProjectDiscussionChannel({
    repositoryChannelId: GENERAL,
    projectChannelId: DESIGN,
    channels: [channel(GENERAL), channel(DESIGN)],
  });
  assert.deepEqual(resolved, {
    id: GENERAL,
    name: "general",
    source: "repository",
  });
});

test("falls back to the project binding when the repository has none", () => {
  const resolved = resolveProjectDiscussionChannel({
    repositoryChannelId: null,
    projectChannelId: DESIGN,
    channels: [channel(DESIGN)],
  });
  assert.equal(resolved?.source, "project");
  assert.equal(resolved?.id, DESIGN);
});

test("hides malformed, missing, archived, dm, and unresolved bindings", () => {
  const channels = [
    channel(GENERAL),
    channel(DESIGN, { archivedAt: "2026-01-01T00:00:00Z" }),
  ];
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: NOT_A_UUID,
      projectChannelId: null,
      channels,
    }),
    null,
  );
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: "11111111-1111-4111-8111-111111111111",
      projectChannelId: null,
      channels,
    }),
    null,
  );
  assert.equal(
    resolveProjectDiscussionChannel({
      repositoryChannelId: DESIGN,
      projectChannelId: null,
      channels,
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

test("skips an unusable repository binding and uses the project binding", () => {
  const resolved = resolveProjectDiscussionChannel({
    repositoryChannelId: DESIGN,
    projectChannelId: GENERAL,
    channels: [channel(GENERAL), channel(DESIGN, { archivedAt: "x" })],
  });
  assert.deepEqual(resolved, {
    id: GENERAL,
    name: "general",
    source: "project",
  });
});

test("mergeBoundDiscussionChannel pins a zero-hit bound channel first", () => {
  const merged = mergeBoundDiscussionChannel(
    { id: GENERAL, name: "general", source: "repository" },
    [
      {
        id: DESIGN,
        name: "design",
        messageCount: 4,
        lastActivityAt: 200,
        participants: ["aa"],
      },
    ],
  );
  assert.equal(merged[0].id, GENERAL);
  assert.equal(merged[0].messageCount, 0);
  assert.equal(merged[1].id, DESIGN);
});

test("mergeBoundDiscussionChannel moves an already-discovered bound channel first", () => {
  const merged = mergeBoundDiscussionChannel(
    { id: DESIGN, name: "design", source: "project" },
    [
      {
        id: GENERAL,
        name: "general",
        messageCount: 1,
        lastActivityAt: 1,
        participants: [],
      },
      {
        id: DESIGN,
        name: "design",
        messageCount: 9,
        lastActivityAt: 9,
        participants: ["aa"],
      },
    ],
  );
  assert.equal(merged[0].id, DESIGN);
  assert.equal(merged[0].messageCount, 9);
  assert.equal(merged.length, 2);
});

test("getDiscussionLabel reports linked vs none", () => {
  assert.equal(getDiscussionLabel(null), "No discussion");
  assert.equal(
    getDiscussionLabel({
      id: GENERAL,
      name: "general",
      source: "repository",
    }),
    "Discussion linked",
  );
});
```

Add this test to `desktop/src/features/projects/projectModels.test.mjs` after the existing implicit-project test (do not replace that test).
Update the local `repositoryEvent` helper so it can take extra tags without breaking other tests:

```js
function repositoryEvent(owner, id, createdAt = 100, extraTags = []) {
  return {
    id: `${id}-${createdAt}`,
    kind: 30617,
    pubkey: owner,
    created_at: createdAt,
    content: "",
    tags: [["d", id], ["name", id], ...extraTags],
  };
}

test("implicit projects inherit the repository buzz-channel", () => {
  const projects = buildProjectReadModels({
    projectEvents: [],
    repositoryEvents: [
      repositoryEvent(BACKEND_OWNER, "backend", 100, [
        ["buzz-channel", "11111111-1111-4111-8111-111111111111"],
      ]),
    ],
    relayOrigin: RELAY_ORIGIN,
  });
  assert.equal(projects.length, 1);
  assert.equal(projects[0].legacy, true);
  assert.equal(
    projects[0].projectChannelId,
    "11111111-1111-4111-8111-111111111111",
  );
  assert.equal(
    projects[0].repositories[0].channelId,
    "11111111-1111-4111-8111-111111111111",
  );
});
```

Leave `desktop/src/features/projects/lib/projectLabels.ts` unchanged until Step 3.

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs src/features/projects/projectModels.test.mjs
```

Expected: FAIL because `./projectDiscussionChannel.ts` cannot be resolved and the implicit project's `projectChannelId` is still `null`.

- [ ] **Step 3: Write the minimal implementation**

Create `desktop/src/features/projects/lib/projectDiscussionChannel.ts`:

```ts
import type { DiscussionChannel } from "@/features/projects/lib/discussionChannels";
import { isValidProjectChannelId } from "@/features/projects/projectModels";
import type { Channel } from "@/shared/api/types";

/** Bound Stream channel a Projects surface may open. */
export type ProjectDiscussionChannel = {
  id: string;
  name: string;
  source: "repository" | "project";
};

type ResolvableChannel = Pick<
  Channel,
  "id" | "name" | "archivedAt" | "channelType"
>;

function isUsableDiscussionChannel(channel: ResolvableChannel): boolean {
  return channel.channelType !== "dm" && channel.archivedAt == null;
}

/**
 * Pick the Stream channel Projects should open for discussion.
 * Repository `buzz-channel` wins because it is the git ACL binding.
 * Unresolvable, archived, or DM values are skipped rather than rendered
 * as broken links (NIP-MP client rule).
 */
export function resolveProjectDiscussionChannel(input: {
  repositoryChannelId: string | null | undefined;
  projectChannelId: string | null | undefined;
  channels: readonly ResolvableChannel[];
}): ProjectDiscussionChannel | null {
  const candidates: Array<{
    id: string;
    source: ProjectDiscussionChannel["source"];
  }> = [];
  if (input.repositoryChannelId) {
    candidates.push({
      id: input.repositoryChannelId,
      source: "repository",
    });
  }
  if (
    input.projectChannelId &&
    input.projectChannelId !== input.repositoryChannelId
  ) {
    candidates.push({
      id: input.projectChannelId,
      source: "project",
    });
  }

  for (const candidate of candidates) {
    if (!isValidProjectChannelId(candidate.id)) continue;
    const channel = input.channels.find((item) => item.id === candidate.id);
    if (!channel || !isUsableDiscussionChannel(channel)) continue;
    return {
      id: channel.id,
      name: channel.name,
      source: candidate.source,
    };
  }
  return null;
}

/**
 * Put the bound discussion channel first on the Channels tab.
 * Preserve FTS rows for other channels that mentioned the entity.
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

/** List/chrome copy for whether a resolvable discussion channel exists. */
export function getDiscussionLabel(
  channel: ProjectDiscussionChannel | null,
): string {
  return channel ? "Discussion linked" : "No discussion";
}
```

In `desktop/src/features/projects/projectModels.ts`, change `repositoryToLegacyProject` so `projectChannelId` is no longer hardcoded `null`:

```ts
projectChannelId: repository.channelId ?? null,
```

Keep every other field in that object unchanged.

Replace `desktop/src/features/projects/lib/projectLabels.ts` with:

```ts
export { getDiscussionLabel } from "./projectDiscussionChannel";
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs src/features/projects/projectModels.test.mjs
```

Expected: PASS.

- [ ] **Step 5: Refactor if needed, then commit**

If the helper re-export in `projectLabels.ts` is unused besides the new module, keep it: it is the existing public path and must not drift.

Run:

```bash
. ./bin/activate-hermit && git add desktop/src/features/projects/lib/projectDiscussionChannel.ts desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs desktop/src/features/projects/lib/projectLabels.ts desktop/src/features/projects/projectModels.ts desktop/src/features/projects/projectModels.test.mjs && git commit -s -m "$(cat <<'EOF'
feat(projects): resolve bound buzz-channel for discussion routing

EOF
)"
```

---

### Task 2: Pin the bound channel on the Channels tab

**Files:**
- Create: `desktop/src/features/projects/useProjectDiscussionChannel.ts`
- Modify: `desktop/src/features/projects/ui/DiscussionChannels.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx` (pass `boundChannel`; do not add the chrome button yet)

**Interfaces:**
- Consumes: `resolveProjectDiscussionChannel`, `mergeBoundDiscussionChannel`, `useChannelsQuery` from `desktop/src/features/channels/hooks.ts`
- Produces:
  - `export function shouldShowDiscussionEmptyState(bound: ProjectDiscussionChannel | null, discovered: readonly DiscussionChannel[]): boolean`
  - `export function useProjectDiscussionChannel(project: Project | null | undefined, repository: Repository | null | undefined): ProjectDiscussionChannel | null`
  - `DiscussionChannelsPanel` gains `boundChannel: ProjectDiscussionChannel | null`
  - `WorkspaceTabs` gains `boundChannel: ProjectDiscussionChannel | null`

- [ ] **Step 1: Write the failing panel test**

Add this test to `desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs` (same file as Task 1; do not create a React test for the panel):

```js
test("zero-hit bound channel still produces a Channels-tab row", () => {
  const merged = mergeBoundDiscussionChannel(
    {
      id: GENERAL,
      name: "general",
      source: "repository",
    },
    [],
  );
  assert.equal(merged.length, 1);
  assert.equal(merged[0].id, GENERAL);
  assert.equal(merged[0].name, "general");
});
```

Add `shouldShowDiscussionEmptyState` to the existing import from `./projectDiscussionChannel.ts` in that test file.
Then append:

```js
test("Channels tab is empty only when there is no bound channel and no hits", () => {
  assert.equal(shouldShowDiscussionEmptyState(null, []), true);
  assert.equal(
    shouldShowDiscussionEmptyState(
      { id: GENERAL, name: "general", source: "repository" },
      [],
    ),
    false,
  );
});
```

Do not add `shouldShowDiscussionEmptyState` to the implementation yet.

- [ ] **Step 2: Run the new empty-state test to verify it fails**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs
```

Expected: FAIL with `shouldShowDiscussionEmptyState` is not exported / not defined.

- [ ] **Step 3: Implement the helper, hook, and panel wiring**

Append to `projectDiscussionChannel.ts`:

```ts
/** True when the Channels tab has neither a bound channel nor FTS hits. */
export function shouldShowDiscussionEmptyState(
  bound: ProjectDiscussionChannel | null,
  discovered: readonly DiscussionChannel[],
): boolean {
  return bound == null && discovered.length === 0;
}
```

Create `desktop/src/features/projects/useProjectDiscussionChannel.ts`:

```ts
import * as React from "react";

import { useChannelsQuery } from "@/features/channels/hooks";
import type { Project, Repository } from "@/features/projects/hooks";
import {
  type ProjectDiscussionChannel,
  resolveProjectDiscussionChannel,
} from "@/features/projects/lib/projectDiscussionChannel";

/** Live bound discussion channel for the open project and selected repository. */
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

In `DiscussionChannels.tsx`, import `mergeBoundDiscussionChannel`, `shouldShowDiscussionEmptyState`, and `ProjectDiscussionChannel`.
Change `DiscussionChannelsPanel` to:

```ts
export function DiscussionChannelsPanel({
  boundChannel = null,
  query,
}: {
  boundChannel?: ProjectDiscussionChannel | null;
  query: string;
}) {
  const { channels, isLoading, isTruncated } = useDiscussionChannels(query);
  const { goChannel } = useAppNavigation();
  const merged = React.useMemo(
    () => mergeBoundDiscussionChannel(boundChannel, channels),
    [boundChannel, channels],
  );
  const channelName = useChannelNameLookup(merged.length > 0);
  const profilesQuery = useUsersBatchQuery(
    merged.flatMap((channel) => channel.participants),
    { enabled: merged.length > 0 },
  );
  const profiles = profilesQuery.data?.profiles;

  if (isLoading && shouldShowDiscussionEmptyState(boundChannel, channels)) {
    return (
      <p className="px-4 py-6 text-sm text-muted-foreground">
        Searching channel discussions…
      </p>
    );
  }
  if (shouldShowDiscussionEmptyState(boundChannel, merged)) {
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
        {merged.map((channel) => {
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
                      ? new Date(channel.lastActivityAt * 1_000).toLocaleString()
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

In `ProjectDetailScreen.tsx`, import `useProjectDiscussionChannel` and call it next to `selectProjectRepository`:

```ts
const discussionChannel = useProjectDiscussionChannel(project, repository);
```

Pass `boundChannel={discussionChannel}` into `WorkspaceTabs`.

In `ProjectWorkspaceTabs.tsx`, add `boundChannel` to the props object (type `ProjectDiscussionChannel | null`, default `null`) and pass it to `DiscussionChannelsPanel`.

Do not add the Open Discussion button in this task.

- [ ] **Step 4: Run unit tests**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs src/features/projects/projectModels.test.mjs
```

Expected: PASS.

Run typecheck:

```bash
. ./bin/activate-hermit && cd desktop && pnpm typecheck
```

Expected: PASS.
If `ProjectDetailScreen.tsx` exceeds 1000 lines, stop and move the hook call plus `WorkspaceTabs` prop wiring into `ProjectDetailChromeActions.tsx` instead of raising the ratchet.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/projects/lib/projectDiscussionChannel.ts desktop/src/features/projects/lib/projectDiscussionChannel.test.mjs desktop/src/features/projects/useProjectDiscussionChannel.ts desktop/src/features/projects/ui/DiscussionChannels.tsx desktop/src/features/projects/ui/ProjectWorkspaceTabs.tsx desktop/src/features/projects/ui/ProjectDetailScreen.tsx && git commit -s -m "$(cat <<'EOF'
feat(projects): pin bound buzz-channel on the Channels tab

EOF
)"
```

---

### Task 3: Open Discussion chrome control

**Files:**
- Create: `desktop/src/features/projects/ui/OpenDiscussionButton.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectDetailChrome.tsx`
- Modify: `desktop/src/features/projects/ui/ProjectDetailScreen.tsx`

**Interfaces:**
- Consumes: `useProjectDiscussionChannel`, `useAppNavigation().goChannel`, `getDiscussionLabel`
- Produces: chrome button with `aria-label="Open Discussion"` and `data-testid="project-open-discussion"` that calls `goChannel(discussionChannel.id)`

- [ ] **Step 1: Write the failing unit test for the button module**

Create `desktop/src/features/projects/ui/OpenDiscussionButton.test.mjs`:

```js
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const source = readFileSync(new URL("./OpenDiscussionButton.tsx", import.meta.url), "utf8");

test("Open Discussion control uses the issue 3611 accessible name", () => {
  assert.match(source, /aria-label="Open Discussion"/);
  assert.match(source, /data-testid="project-open-discussion"/);
  assert.match(source, /goChannel/);
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/ui/OpenDiscussionButton.test.mjs
```

Expected: FAIL because `OpenDiscussionButton.tsx` does not exist.

- [ ] **Step 3: Implement the button and chrome wiring**

Create `desktop/src/features/projects/ui/OpenDiscussionButton.tsx`:

```tsx
import { Hash } from "lucide-react";

import type { ProjectDiscussionChannel } from "@/features/projects/lib/projectDiscussionChannel";
import { cn } from "@/shared/lib/cn";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";

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
          className={cn(
            "flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted/40 hover:text-foreground focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring",
          )}
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

In `ProjectDetailChrome.tsx`, import `OpenDiscussionButton` and add props:

```ts
discussionChannel: ProjectDiscussionChannel | null;
onOpenDiscussion: (channelId: string) => void;
```

Render `<OpenDiscussionButton channel={discussionChannel} onOpen={onOpenDiscussion} />` immediately before `<ShareLinkButton … />`.

In `ProjectDetailScreen.tsx`, pass:

```ts
discussionChannel={discussionChannel}
onOpenDiscussion={(channelId) => {
  void goChannel(channelId);
}}
```

`discussionChannel` is the value from Task 2's `useProjectDiscussionChannel` call.
Do not call the hook twice.

Confirm `ProjectDetailScreen.tsx` line count stays ≤ 1000:

```bash
. ./bin/activate-hermit && wc -l desktop/src/features/projects/ui/ProjectDetailScreen.tsx
```

Expected: a number ≤ 1000.

- [ ] **Step 4: Run tests and typecheck**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/ui/OpenDiscussionButton.test.mjs src/features/projects/lib/projectDiscussionChannel.test.mjs && pnpm typecheck
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/projects/ui/OpenDiscussionButton.tsx desktop/src/features/projects/ui/OpenDiscussionButton.test.mjs desktop/src/features/projects/ui/ProjectDetailChrome.tsx desktop/src/features/projects/ui/ProjectDetailScreen.tsx && git commit -s -m "$(cat <<'EOF'
feat(projects): open the bound buzz-channel from project chrome

EOF
)"
```

---

### Task 4: Smoke e2e and screenshot contract

**Files:**
- Create: `desktop/tests/e2e/project-channel-routing.spec.ts`
- Modify: `desktop/tests/e2e/projects-v3-screenshots.spec.ts`
- Modify: `desktop/playwright.config.ts` (smoke `testMatch` array)

**Interfaces:**
- Consumes: mock repo `buzz-channel` `9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50` (`STARTER_GENERAL_CHANNEL_ID` / `#general`) already seeded in `desktop/src/testing/e2eBridge.ts`
- Produces: smoke coverage that Open Discussion navigates to `/channels/9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50`

- [ ] **Step 1: Write the failing smoke spec and update the screenshot assertion**

Create `desktop/tests/e2e/project-channel-routing.spec.ts`:

```ts
import { expect, test } from "@playwright/test";

import { waitForAnimations } from "../helpers/animations";
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

test("Open Discussion routes the bound buzz-channel to Stream", async ({
  page,
}) => {
  await enableProjectsFeature(page);
  await installMockBridge(page);
  await openBuzzProject(page);

  const openDiscussion = page.getByRole("button", { name: "Open Discussion" });
  await expect(openDiscussion).toBeVisible({ timeout: 10_000 });
  await waitForAnimations(page);
  await openDiscussion.click();

  await expect(page).toHaveURL(new RegExp(`/#/channels/${GENERAL_CHANNEL_ID}`));
  await expect(page.getByTestId("chat-title")).toHaveText("general");
});

test("Channels tab lists the bound channel first", async ({ page }) => {
  await enableProjectsFeature(page);
  await installMockBridge(page);
  await openBuzzProject(page);

  await page.getByRole("tab", { name: "Channels" }).click();
  const bound = page.getByTestId("project-bound-discussion-channel");
  await expect(bound).toBeVisible({ timeout: 10_000 });
  await expect(bound).toContainText("#general");
  await bound.click();
  await expect(page).toHaveURL(new RegExp(`/#/channels/${GENERAL_CHANNEL_ID}`));
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

Add `"**/project-channel-routing.spec.ts"` to the smoke `testMatch` array in `desktop/playwright.config.ts` immediately after `"**/projects-v3-screenshots.spec.ts"`.

- [ ] **Step 2: Run the smoke spec to verify the old screenshot assertion is gone and the new spec exists**

If Task 3 is not wired, this run is the red proof.
After Task 3 it should be green.

Run:

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- project-channel-routing.spec.ts
```

Expected after Task 3: PASS.
`addInitScript` must run before `installMockBridge` (Projects feature flag is localStorage, read on mount).

If the spec fails because Projects is gated, keep `enableProjectsFeature` before `installMockBridge` and do not invert that order.

- [ ] **Step 3: Run the screenshot spec smoke file too**

Run:

```bash
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- projects-v3-screenshots.spec.ts
```

Expected: PASS.
The overview screenshot will now include the Hash chrome button.
That pixel change is required, not a regression.

- [ ] **Step 4: Run desktop unit tests and typecheck once more**

Run:

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs src/features/projects/projectModels.test.mjs src/features/projects/ui/OpenDiscussionButton.test.mjs && pnpm typecheck && pnpm exec biome check src/features/projects/lib/projectDiscussionChannel.ts src/features/projects/useProjectDiscussionChannel.ts src/features/projects/ui/OpenDiscussionButton.tsx src/features/projects/ui/ProjectDetailChrome.tsx src/features/projects/ui/DiscussionChannels.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/tests/e2e/project-channel-routing.spec.ts desktop/tests/e2e/projects-v3-screenshots.spec.ts desktop/playwright.config.ts && git commit -s -m "$(cat <<'EOF'
test(projects): cover bound buzz-channel discussion routing

EOF
)"
```

---

## Acceptance criteria

- Opening the mock `buzz` project shows an `Open Discussion` control.
- Clicking it navigates to `/#/channels/9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50` and the chat title is `general`.
- The Channels tab shows `#general` as `project-bound-discussion-channel` even when NIP-50 FTS returns no hits.
- Clicking that row also opens `#general`.
- Implicit/legacy projects whose kind:30617 has a valid `buzz-channel` expose that UUID as `projectChannelId`.
- Malformed, archived, DM, or viewer-unresolved bindings render no Open Discussion button and no broken `#uuid` text.
- `buzz://project?tab=channels` and `buzz://repo?tab=channels` still open the Projects workspace Channels tab (existing `workspaceTabForShareTab` path).
- Relay ingest still treats kind:30621 as global-only.
- `ProjectDetailScreen.tsx` stays ≤ 1000 lines.

## Validation commands

```bash
. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/projects/lib/projectDiscussionChannel.test.mjs src/features/projects/projectModels.test.mjs src/features/projects/ui/OpenDiscussionButton.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm typecheck
. ./bin/activate-hermit && cd desktop && pnpm exec biome check src/features/projects/lib/projectDiscussionChannel.ts src/features/projects/useProjectDiscussionChannel.ts src/features/projects/ui/OpenDiscussionButton.tsx src/features/projects/ui/ProjectDetailChrome.tsx src/features/projects/ui/DiscussionChannels.tsx src/features/projects/ui/ProjectWorkspaceTabs.tsx src/features/projects/ui/ProjectDetailScreen.tsx
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- project-channel-routing.spec.ts
```

Always `pnpm build:e2e` (the `test:e2e:smoke` script already does) after UI changes.
Kill port 4173 if a stale Vite preview is serving an old bundle.

## Open Questions

1. Should `buzz://…&tab=channels` skip Projects and call `goChannel` when a bound channel exists?
   Provisional default: no.
   Share links already mean the workspace Channels tab, and changing that would break copied links.

2. Should Open Discussion appear for an open channel the viewer has not joined?
   Provisional default: yes, if the channel is in `useChannelsQuery` data.
   That matches `ProjectReadmePanel` access-restricted copy, which already links open unbound-membership channels.

3. Should the chrome control be labeled text instead of a Hash icon?
   Provisional default: Hash icon with accessible name `Open Discussion`.
   The detail chrome is icon-dense (`ShareLinkButton` is icon-only) and the screenshot/e2e contract keys off the accessible name, not visible text.

## Self-review

- Spec coverage: bound-tag spelling, writer/reader mismatch from #3611, NIP-MP no-broken-link rule, VISION discussion binding, Open Discussion affordance, Channels tab, legacy implicit projects, e2e on mock `general`.
- Non-coverage (intentional): branch channels, git-event fan-in to Stream, mobile, web, relay ingest, CLI bind.
- Placeholder scan: no TBD/TODO/implement later.
- Type consistency: `ProjectDiscussionChannel` is the only discussion-channel type used by the hook, chrome, panel, and tests.
