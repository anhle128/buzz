# Agent permission alerts (notify + open transcript)

**Status:** Draft — awaiting user review
**Scope:** Buzz Desktop only. When an owned agent emits an actionable `session/request_permission`, tell the owner even if they are in another thread, and open the existing process transcript on click.
**Vision:** [VISION.md](../../../VISION.md) — workflow approvals already notify; Stream stays zero-notification for chat. Agent permission is an owner approval, not a channel message.

## Summary

Actionable permission cards already render in the agent process transcript (`LifecycleActivity`) with the agent's full option list (`allow_once`, `allow_always`, `reject_once`, `reject_always`, plus any other admitted `name`/`optionId`). Observer frames already arrive app-wide via `useAgentObserverIngestion`.

The miss: the grant UI is only visible when that transcript pane is open. The owner chatting in another thread never sees the request. The 300s harness timeout then fail-closes.

This design adds an in-app toast (app focused) or OS notification (app background). Click opens the existing session pane through `useOpenAgentActivity`. Grant stays on the transcript card. No new event kind. No Home inbox. No grant buttons on the notification.

## Problem

`handle_permission_request` in `crates/buzz-acp` emits an enveloped `acp_read` (`authorization.actionable: true` under policy `ask`). Desktop folds that into `pendingPermissions` on the transcript. `PermissionDecisionButtons` lives only in `LifecycleActivity`.

`useOpenAgentActivity(pubkey, { channelId })` already navigates to that pane (`goChannel` + `agentSession` search param). Nothing calls it when a request arrives.

## Goals

- Notify the owner when a new actionable permission request arrives and they are not already looking at that agent's session pane for that channel.
- In-app toast when Buzz is focused; OS notification when it is not.
- Click either surface opens the process transcript for that agent and channel.
- Leave grant/deny on the existing card, using the agent's option `name`s and `optionId`s.
- Do not re-notify replayed frames after reconnect or community remount snapshot.

## Non-goals

- Granting from the notification or OS action buttons.
- New Nostr kind, Home `needs_action` row, or relay persistence.
- Withdrawing OS notifications after a decision (no withdraw API today).
- Mobile.
- New notification settings slot.
- Changing ACP admission, nonce binding, or `permission_decision` control frames.
- Notifying anyone except the owner (frames are already `#p`-addressed).

## Product decisions

- **Trigger:** `ObserverEvent.kind === "acp_read"` and payload method `session/request_permission` and `authorization.actionable === true`. Ignore auto-allow, auto-reject, fail-closed, and missing envelope.
- **Identity:** one alert per `authorization.requestNonce`. Concurrent requests (different nonces) each get an alert.
- **Copy:** title `{agentName} needs permission` with `#channel` when the channel name is known. Body is a short prompt to open the process log. Do not list options.
- **Suppress:** only when the open session pane is **that agent and that channel**. Being in the same channel on a different thread still alerts. Matching rule:
  - Route search `agentSession` equals the requesting agent (normalized pubkey).
  - Request `channelId` is null **or** equals the current channel id **or** equals `agentSessionChannel`.
- **Focus split:** `isAppFocused()` → sonner toast, `id = requestNonce`. Else → existing `sendDesktopNotification` + dock bounce. Respect `desktopEnabled` for the OS path only. In-app toast still fires when desktop notifications are off (the original miss is in-app).
- **Mute:** channel mute and `notifyWhileViewing` do not apply. This is an owner approval, not a channel message.
- **Sound:** `needs_action` slot when that slot is enabled. Play for both toast and OS paths.
- **Click:** `revealDesktopAppWindow()` then `openAgentActivity(agentPubkey, { channelId })`. Missing `channelId` uses the existing no-channel fallback (working / member channel). Inaccessible channel uses the existing warning toast.
- **Dismiss:** on `acp_write` / timeout / cancel for that nonce, `toast.dismiss(requestNonce)`. OS notification stays until the user or OS clears it. A late click still opens the pane; the card may already show an outcome.
- **Replay:** on hook mount, seed a seen-nonce set from current transcripts. Only later new nonces alert. Seen set is React state so community remount (`AppReady` key) clears it. Do not add a `resetCommunityState` singleton.

## Architecture

```
acp_read (actionable permission)
        │
        ▼
observer store (already app-wide)
        │
        ▼
usePendingPermissionAlerts   ── focused ──► sonner toast (id = nonce)
        │                    └── background ► sendDesktopNotification
        │                                              │
        └── click ──► openAgentActivity(agent, { channelId })
                            │
                            ▼
                   existing transcript card
                   (full option list, permission_decision)
```

No relay change. Native notification code already forwards the target JSON blob; adding `agentPubkey` on the JS target is enough for Linux/macOS round-trip.

## Components

### `pendingPermissionAlert.ts` (new, pure)

Testable helpers, no React:

- `extractActionablePermission(event)` → `{ requestNonce, channelId, title } | null`
- `shouldSuppressPermissionAlert({ agentPubkey, channelId, openAgentSession, openAgentSessionChannel, currentChannelId })` → boolean
- `permissionAlertCopy({ agentName, channelName })` → `{ title, body }`

### `usePendingPermissionAlerts.ts` (new)

Mounted next to `useAgentObserverIngestion` in `AppShell`.

- Subscribe `subscribeAgentObserverStore`.
- Seed seen nonces from `getAgentTranscript` for ingested agents (same owner-global list as observer ingestion).
- On store update, extract new actionable permissions; skip seen nonces; mark seen; suppress if pane matches; toast or OS notify.
- Subscribe resolution (`acp_write` / non-actionable follow-up for that nonce) only to dismiss the toast.
- Read current `agentSession` / `agentSessionChannel` / channel id from the same route search the channel panel uses.

### `DesktopNotificationTarget` (`desktop.ts`)

Add optional `agentPubkey`. Do **not** reuse `pubkey` — that field is the message author for `toSearchHit`.

Change `parseNotificationTarget` so a target is valid when `channelId`, `eventId`, **or** `agentPubkey` is present. Today `!channelId && !eventId` returns null, which would drop a no-channel permission click.

### `handleDesktopNotificationAction` (`useAppShellDesktopNotifications.ts`)

If `target.agentPubkey` is set, reveal the window and call `openAgentActivity(target.agentPubkey, { channelId: target.channelId })`. Do not call `openSearchHit`.

### Unchanged

- `LifecycleActivity` / `sendPermissionDecision`
- `buzz-acp` harness, nonce, 300s timeout
- Home feed, kinds, mobile

## Data flow

1. Agent emits `session/request_permission` with 1–16 options (`optionId`, `kind`, `name`).
2. Harness admits the request, emits enveloped `acp_read` (`actionable: true`, `requestNonce`).
3. Desktop observer store appends the frame and updates the transcript card (existing).
4. Alert hook sees a new nonce, not suppressed, and posts toast or OS notification.
5. Owner clicks → session pane for that agent/channel.
6. Owner picks an option on the card → `permission_decision` control frame (existing).
7. Harness writes ACP response, emits `acp_write`; hook dismisses the toast.

## Error handling

| Case | Behavior |
|------|----------|
| Replay / reconnect snapshot | Seeded seen set; no alert |
| Duplicate nonce | Ignored |
| `actionable: false` | No alert |
| App focused, desktop notifications off | In-app toast only |
| OS send fails | No retry; toast already shown if focused; if background, miss (same as other OS notifications) |
| Channel not openable | `openAgentActivity` warning; do not invent another destination |
| Late click after grant/timeout | Open pane; card shows outcome |
| Community switch | Hook remounts; new seen seed from new store |

## Testing

Unit only for this slice:

- Extract: actionable permission → payload; non-permission / non-actionable → null.
- Suppress: matching agent + channel → true; same channel, pane closed → false; other agent's pane open → false.
- `parseNotificationTarget`: `agentPubkey` alone is valid; `pubkey` without `agentPubkey` still routes as today.
- Click helper / action branch: `agentPubkey` present → open-activity args, not a search hit.
- Mount seed: existing transcript nonces do not alert.

No new Playwright spec required. Existing observer permission e2e stays the grant-on-card path.

## Files

| File | Change |
|------|--------|
| `desktop/src/features/agents/pendingPermissionAlert.ts` | New pure helpers |
| `desktop/src/features/agents/pendingPermissionAlert.test.mjs` | New unit tests |
| `desktop/src/features/agents/usePendingPermissionAlerts.ts` | New hook |
| `desktop/src/app/AppShell.tsx` | Mount hook |
| `desktop/src/features/notifications/lib/desktop.ts` | `agentPubkey` + parse guard |
| `desktop/src/features/notifications/lib/desktop.test.mjs` | Parse / target cases |
| `desktop/src/app/useAppShellDesktopNotifications.ts` | Click routing |

## Success

An owner in another thread sees a toast (or an OS notification if Buzz is in the background) when their agent asks for permission. Click opens the process log where the existing option buttons work. No alert if that pane is already open. No alert on snapshot replay. Grant UX is unchanged.
