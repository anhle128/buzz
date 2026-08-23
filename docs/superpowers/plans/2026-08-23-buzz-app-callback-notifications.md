# Buzz App Callback Notifications Implementation Plan

> **For Grok:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add community-scoped Apps whose authenticated HTTP callbacks become exactly-once, relay-signed project-channel messages with trustworthy App attribution in the CLI, Desktop, and Mobile clients.

**Architecture:** Owners and admins manage Apps by publishing signed kind `9038` commands, while all clients discover relay-signed parameterized-replaceable kind `39007` App metadata through normal Nostr queries.
The only new HTTP surface is `POST /hooks/apps/{app_id}`, because third-party callback delivery genuinely requires HTTP.
The callback path authenticates before exposing payload diagnostics, resolves the existing repository-to-project-to-channel route, serializes idempotency in PostgreSQL, and commits the message event and final delivery record in one transaction.
Clients attribute a kind `9` message to an App only after verifying both the message signer and the latest same-relay App metadata, and every failure falls back to the relay signer rather than a `p` tag.

**Tech Stack:** Rust, Axum, SQLx/PostgreSQL, Redis, Nostr, Clap, React 19, TanStack Query, Tauri 2, Playwright, Flutter, Riverpod, and `package:nostr`.

---

## Product contract

This plan implements the approved design in `docs/superpowers/specs/2026-08-23-buzz-app-callback-notifications-design.md`.
It advances `VISION.md` by making external automation legible as a first-class participant without inventing a second client protocol.
It advances `VISION_PROJECTS.md` by reusing the canonical repository-to-project-to-channel relationship instead of creating App-specific destinations.
The approved design intentionally lets an App reach a live project channel without becoming a channel member.
That tension with the normal membership boundary is contained by owner/admin lifecycle control, a per-App secret, tenant binding, deterministic project routing, relay signing, strict client attribution, and exactly-once admission.
No App-list HTTP endpoint, App-specific channel membership, provider-selected channel, or provider-signed Nostr event is in scope.

## Mandatory execution protocol

Run every shell command from the repository root after activating Hermit in that same command.
Use `. ./bin/activate-hermit && <command>` because shell activation does not persist between commands.
Read the complete repository `AGENTS.md`, `VISION.md`, `VISION_PROJECTS.md`, `TESTING.md`, `crates/buzz-cli/TESTING.md`, and this plan before editing.
Use the `superpowers:test-driven-development` skill for every behavior change.
Run GitNexus upstream impact analysis before editing each existing function, method, class, or exported component named in a task.
Report any HIGH or CRITICAL GitNexus result before making that edit.
If the checked-in GitNexus runner and MCP tools are both unavailable, record that fact and use `rg` plus direct caller inspection as the fallback without inventing impact output.
Write one failing behavioral test first, run the exact test, and confirm that it fails for the expected product reason before adding production behavior.
An unmatched test filter, zero tests, an unwired module, or an unrelated compiler failure is not a valid RED result.
When a new test first fails to compile because its planned public symbol or module does not exist, add only the smallest typed stub that returns an explicit `not implemented` error, rerun the test, and require at least one discovered behavioral assertion to fail before implementing real behavior.
For a pure move or rename, establish a passing characterization test before the move and rerun the same behavior after the move instead of manufacturing a RED failure.
Keep production code free of new `unwrap()`, `expect()`, and `unsafe`.
Add doc comments to every new public Rust API.
Keep every Desktop, Web, and Mobile source file at or below 1,000 lines by splitting files instead of changing the size gate.
Use rem-based named text tokens in Desktop UI and never add arbitrary text-size literals.
Use `ConsumerWidget` or `HookConsumerWidget` for Mobile UI and never add a `StatefulWidget`.
Before each Desktop Playwright run, stop any listener left on port 4173 with `. ./bin/activate-hermit && listener_pids=$(lsof -tiTCP:4173 -sTCP:LISTEN); if [[ -n "$listener_pids" ]]; then kill $listener_pids; fi`, then use `pnpm test:e2e:smoke` so `pnpm build:e2e` always rebuilds the mock bridge.
Before every commit, run GitNexus `detect_changes` against `main`, inspect `git diff --check`, inspect `git status --short`, and confirm that only the task's intended files and flows changed.
Create every commit with `git commit -s`.
Do not upload screenshots through Buzz or a third-party host.

## Resolved behavior

### App lifecycle and metadata

- An App is keyed by `(community_id, app_id)` where `app_id` is a canonical lowercase hyphenated UUID.
- `create` generates the App UUID and a 32-byte cryptographically random secret only after the kind `9038` command event has been transactionally claimed.
- The wire secret is base64url without padding, and PostgreSQL stores only `SHA-256(secret_bytes)`.
- The App row stores the creating command signer's 32-byte pubkey as immutable `created_by`, and App names are not unique.
- The raw secret appears only in the successful create or rotate response and is never persisted, audited, logged, cached, or redisplayed.
- A lost create or rotate response is recovered by issuing a new rotate command.
- The lifecycle status is exactly `active` or `disabled`.
- Disabling an App keeps its latest kind `39007` metadata queryable so historical messages remain attributable.
- Kind `39007` is relay-only and parameterized-replaceable by `(kind, relay_pubkey, d)`.
- Each metadata event contains exactly one `d`, `name`, and `status` tag, at most one `picture` tag, the description as content, and no secret or secret hash.
- The relay publishes a metadata replacement after create, public-metadata update, enable, or disable, but secret rotation changes only the private App row and `updated_at`.
- Metadata replacement uses the `d` tag and never `replace_addressable_event`, whose channel-derived key would collapse relay-signed Apps.
- Metadata timestamps are monotonic per App as `max(now_seconds, current_head.created_at + 1)` so rapid same-second lifecycle updates cannot lose NIP-33 ordering.
- If metadata events tie on `created_at`, the lexicographically lower event ID wins, matching the repository's existing NIP-33 rule.

### Kind `9038` command schema

The command content is a Serde internally tagged object with `deny_unknown_fields` and one of these exact shapes.

```json
{"action":"create","name":"Buildkite","description":"Build notifications","icon_url":"https://example.test/icon.png"}
{"action":"update","app_id":"6eb31227-8ed2-42ec-9024-863497cbeed2","name":"CI","description":"","icon_url":""}
{"action":"rotate_secret","app_id":"6eb31227-8ed2-42ec-9024-863497cbeed2"}
{"action":"enable","app_id":"6eb31227-8ed2-42ec-9024-863497cbeed2"}
{"action":"disable","app_id":"6eb31227-8ed2-42ec-9024-863497cbeed2"}
```

- `name` is trimmed, non-empty, and at most 128 Unicode scalar values.
- `description` is at most 2,048 Unicode scalar values.
- `icon_url` is at most 4,096 UTF-8 bytes and accepts only the repository's safe HTTP(S) or `data:image/` forms.
- Empty `description` and `icon_url` values clear those optional fields on update.
- Create normalizes empty optional values to absent.
- Update must contain at least one mutable field.
- Only active community owners and admins may submit kind `9038`.
- A duplicate command event performs no second mutation and returns accepted duplicate status without a secret.
- The command event, App row mutation, and new kind `39007` head commit in one PostgreSQL transaction.
- Audit enqueue and Nostr fan-out happen only after commit.

### Callback request and response contract

- The route is `POST /hooks/apps/{app_id}` under the host-derived community.
- `app_id` parsing failure returns `400` with code `invalid_app_id`.
- Missing, cross-community, or disabled Apps return the same `404` response with code `app_not_found`.
- Authentication reads only `X-Webhook-Secret`, base64url-decodes it without padding to exactly 32 bytes, and compares its SHA-256 digest to the stored digest in constant time.
- A missing or wrong secret returns `401` with code `unauthorized` before content-type, body, JSON, or control-field diagnostics.
- A secret in the query string never authenticates the request.
- The request body limit is 64 KiB and an oversized body returns `400` with code `invalid_callback`.
- After authentication, the handler requires `Content-Type: application/json`, valid JSON, and a top-level object.
- The only top-level keys are `idempotency_key`, `repository_name`, `event_type`, `content`, and optional `metadata`.
- Unknown or authority-like top-level fields such as `channel_id`, `project_id`, `community_id`, `app_id`, `pubkey`, `author`, `tags`, and `kind` return `400` with code `invalid_callback`.
- `idempotency_key` is a non-empty string of at most 512 UTF-8 bytes and is hashed without trimming.
- `repository_name` is a non-empty string of at most 512 UTF-8 bytes and is passed unchanged to the shared exact-tier route resolver.
- `event_type` is lowercase ASCII matching `[a-z0-9][a-z0-9._:-]{0,63}`.
- `content` is a non-empty UTF-8 string subject to the existing 256 KiB message-content limit, while the 64 KiB callback-body limit is the effective tighter bound.
- `metadata`, when present, must be a JSON object and is otherwise opaque to routing and attribution in version one.
- Serialized `metadata` is at most 32 KiB, JSON nesting is at most 16 levels, and the recursive total of object members plus array elements is at most 1,024.
- Missing or invalid required control values return `422` with code `invalid_control_fields` before a ledger row is created.
- JSON object key order does not affect the payload hash.
- Array order and field presence do affect the payload hash, so omitted `metadata` and explicit empty `metadata` are distinct payloads.
- The canonical payload hash covers the accepted request object after removing only `idempotency_key`.
- The handler drops the raw idempotency key after hashing and never logs it.
- Success and same-payload replay return `202` with `delivery_id`, `event_id`, `status: "delivered"`, and a boolean `replayed`.
- A reused key with a different payload returns `409` with code `idempotency_conflict`.
- A deterministic route rejection commits a final rejected delivery and returns or replays the same `422` code and redacted message.
- A transient database, pre-admission Redis, signer, route-read, or mention-read failure returns `503` and commits no new delivery record.
- A post-commit fan-out or audit-queue failure cannot roll back the delivered event and ledger row and is handled by the existing dispatch observability path.
- Rate-limit denial returns `429`, code `rate_limited`, and a `Retry-After` header.
- All JSON errors use the existing `api_error_with_code` envelope with only safe fields.

### Routing and exactly-once delivery

- Rename `crates/buzz-relay/src/workflow_route.rs` to `crates/buzz-relay/src/project_route.rs` with `git mv` and update its two current importers.
- Preserve the existing repository identity tiers, claim validation, single-project rule, and `buzz-channel` parsing without a compatibility copy or re-export.
- Workflow callers continue to pass configured aliases and validate workflow-owner channel membership.
- App callers pass an empty alias map and require only that the destination channel exists in the same community and is neither deleted nor archived.
- The App itself does not need channel membership.
- Admission serializes on a PostgreSQL advisory transaction lock derived from `(community_id, app_id, SHA-256(idempotency_key))` before checking for an existing delivery.
- A same-hash existing row is replayed, while a different payload hash is a conflict.
- A vacant admission guard owns a pre-generated delivery UUID that is used both in the event tag and ledger row.
- Repository heads, project heads, and the destination channel are read on the locked admission transaction.
- A deterministic route failure inserts one immutable rejected row and commits it.
- Dropping a vacant guard rolls the transaction back and inserts nothing.
- Successful delivery inserts the kind `9` event, any event mentions, and the final delivered row in the same transaction.
- The successful event is a root message, so no reply or descendant counter changes are required.
- Final delivery identity, payload, route, outcome, and event fields are immutable at the database layer.

### Relay-signed App messages

- The relay creates a kind `9` event whose author is the active relay key.
- The event content is exactly the validated callback `content`.
- The event contains one `h` tag with the routed channel UUID.
- The event contains one `buzz:app` tag with the canonical App UUID.
- The event contains one repository `a` tag with marker `repository`.
- The event contains one project `a` tag with marker `project`.
- The event contains one `buzz:app-delivery` tag with the delivery UUID.
- The event contains one `buzz:app-event` tag with the validated event type.
- The relay reuses the existing workflow `@Name` resolver against current destination-channel members and adds one deduplicated `p` tag per resolved mention.
- Mention resolution never adds a creator, admin, workflow owner, or App-author `p` tag.
- Resolved mention `p` tags are never treated as the App author.
- A relay-signed event with `buzz:app` is excluded from workflow triggering just like a relay-signed event with `buzz:workflow`.
- Persisted dispatch uses the normal event-created audit path and Redis fan-out after the database transaction commits.

### Client attribution

- A client recognizes an App only on kind `9` with exactly one canonical `buzz:app` UUID tag.
- A live signed message must have a valid event ID and signature from the active relay key.
- Search and feed projections that do not carry signatures may skip only the message-signature check because their data comes from the local Tauri bridge, but they must still require kind `9`, the active relay pubkey, a canonical App UUID, and verified metadata.
- Kind `39007` metadata must have a valid event ID and signature, the active relay author, exactly one valid `d`, `name`, and `status`, and at most one `picture`.
- Clients fold only the latest metadata head per App using `created_at` descending and event ID ascending.
- Disabled metadata remains valid for historical attribution.
- Any malformed UUID, wrong signer, invalid signature, missing metadata, duplicate required tag, or metadata mismatch falls back to the relay signer.
- Fallback uses the existing signer-aware user resolver and never the first `p` tag.
- The visible App identity uses the metadata name and optional picture, shows an `App` badge, keeps the relay pubkey as the cryptographic signer, and does not open a user profile popover.
- Grouping keys include `appId` so two Apps signed by one relay never group as the same author.
- App messages cannot expose user-only message management actions.

### Audit and observability

- Add lifecycle audit actions `AppCreated`, `AppUpdated`, `AppSecretRotated`, `AppEnabled`, and `AppDisabled`.
- Lifecycle audit details contain only `app_id`, action, and resulting status.
- Callback metrics and logs may contain community ID, App ID, delivery ID, outcome, stable error code, and duration.
- Logs and audit data must never contain a raw or hashed secret, callback headers, raw idempotency key, payload hash, repository payload, message content, description, icon URL, or App name.
- A delivered kind `9` continues to emit the standard `EventCreated` audit record instead of a duplicate App-delivery action.

## Open questions with binding provisional defaults

Implementation proceeds with these defaults unless the product owner records a different decision before the relevant task begins.

1. The production callback quota and trusted-proxy source-IP policy are not specified by the approved design.
The provisional default is a Redis fixed window of 60 requests per 60 seconds after resolving an active App for `(community, app, transport peer IP)`, including failed secret attempts, and the handler does not trust `X-Forwarded-For` or similar headers.

2. The approved design names link previews, but the current message-link pill carries only channel ID, event ID, and excerpt and has no author projection.
The provisional default is that previews backed by a real `TimelineMessage` inherit App attribution, while the channel-and-ID-only `MessageLinkPill` remains authorless.

3. A deployed community, owner credential, and callback-observation channel may not be available to the implementer.
The provisional default is to make the hermetic relay-backed E2E suite mandatory and to record deployed live acceptance as operator-gated rather than silently claiming it passed.

## File map

### Backend protocol, persistence, and relay

- Create `crates/buzz-core/src/app.rs` for the command schema, App status, validation limits, and safe icon validation.
- Modify `crates/buzz-core/src/lib.rs` to export the App protocol module.
- Modify `crates/buzz-core/src/kind.rs` for kinds `9038` and `39007` plus command, relay-admin, and relay-only classification.
- Modify `crates/buzz-audit/src/action.rs` for lifecycle actions and round-trip coverage.
- Create `migrations/0033_app_callback_notifications.sql` for `apps`, `app_callback_deliveries`, constraints, indexes, and immutability enforcement.
- Create `crates/buzz-db/src/app.rs` for tenant-scoped lifecycle rows and transaction helpers.
- Create `crates/buzz-db/src/app_admission.rs` for serialized delivery admission and atomic finalize operations.
- Modify `crates/buzz-db/src/lib.rs` to expose the two modules and reuse transaction-local event and mention insertion.
- Create `crates/buzz-db/tests/app_storage.rs` for lifecycle, command claim, metadata replacement, and tenant tests.
- Create `crates/buzz-db/tests/app_admission.rs` for concurrency, replay, conflict, rejection, rollback, and atomic delivery tests.
- Rename `crates/buzz-relay/src/workflow_route.rs` to `crates/buzz-relay/src/project_route.rs`.
- Modify `crates/buzz-relay/src/workflow_admission.rs` and `crates/buzz-relay/src/lib.rs` for the route-module rename and App modules.
- Create `crates/buzz-relay/src/handlers/app_admin.rs` for kind `9038` execution.
- Modify `crates/buzz-relay/src/handlers/mod.rs` and `crates/buzz-relay/src/handlers/command_executor.rs` to dispatch App commands.
- Modify `crates/buzz-relay/src/handlers/ingest.rs` for global App-command scoping and owner/admin authorization.
- Modify `crates/buzz-relay/src/handlers/event.rs` to suppress workflow recursion for relay-generated App messages.
- Create `crates/buzz-relay/src/message_mentions.rs` by extracting the existing pure `@Name` resolver from `crates/buzz-relay/src/workflow_sink.rs`.
- Modify `crates/buzz-relay/src/workflow_sink.rs` to use the shared mention resolver without changing workflow behavior.
- Create `crates/buzz-relay/src/app_sink.rs` for callback parsing, validation, admission, route resolution, event construction, safe responses, and observations.
- Modify `crates/buzz-relay/src/api/bridge.rs` to expose the tenant-bound callback wrapper.
- Modify `crates/buzz-relay/src/router.rs` for the `/hooks/apps/{app_id}` route.
- Modify `crates/buzz-pubsub/src/rate_limiter.rs` for a documented named-key rate-limit method.
- Create `crates/buzz-test-client/tests/e2e_app_callback_notifications.rs` for relay-backed acceptance.

### CLI

- Create `crates/buzz-cli/src/commands/apps.rs` for list and lifecycle subcommands.
- Modify `crates/buzz-cli/src/commands/mod.rs` and `crates/buzz-cli/src/lib.rs` for module registration, Clap definitions, dispatch, and tests.
- Modify `crates/buzz-cli/README.md` and `crates/buzz-cli/TESTING.md` for exact command and live-test examples.

### Desktop data and attribution

- Create `desktop/src/features/apps/types.ts` for verified App metadata and App actor types.
- Create `desktop/src/features/apps/lib/appMetadata.ts` and `desktop/src/features/apps/lib/appMetadata.test.mjs` for metadata validation and latest-head folding.
- Create `desktop/src/features/apps/lib/appActor.ts` and `desktop/src/features/apps/lib/appActor.test.mjs` for fail-closed actor resolution.
- Create `desktop/src/features/apps/hooks/useAppsQuery.ts` for the same-relay kind `39007` query.
- Modify `desktop/src/shared/constants/kinds.ts` to add kind `39007` in sync with Rust and Mobile.
- Create `desktop/src/shared/api/searchWire.ts` and `desktop/src/shared/api/searchWire.test.mjs` by moving search wire conversion out of the already oversized `desktop/src/shared/api/tauri.ts` and adding tags.
- Modify `desktop/src/shared/api/tauri.ts`, `desktop/src/shared/api/types.ts`, and `desktop/src/testing/e2eBridge.ts` to preserve search-hit tags.
- Modify `desktop/src-tauri/src/models.rs`, `desktop/src-tauri/src/nostr_convert.rs`, and `desktop/src-tauri/src/nostr_convert/tests.rs` to preserve search-hit tags across the native bridge.
- Modify `desktop/src/features/messages/types.ts` and `desktop/src/features/messages/lib/formatTimelineMessages.ts` for signer and App identity fields.
- Modify `desktop/src/features/messages/lib/formatTimelineMessages.test.mjs` for attribution and fallback cases.
- Create `desktop/src/features/messages/ui/MessageAuthorIdentity.tsx` and move the current author/avatar/popover block out of `desktop/src/features/messages/ui/MessageRow.tsx` before adding App rendering.
- Modify `desktop/src/features/messages/ui/MessageRow.tsx` and `desktop/src/features/messages/lib/messageGrouping.ts` for App badge, no profile popover, memo stability, and grouping identity.
- Modify `desktop/src/features/messages/lib/messageGrouping.test.mjs` for two Apps under one relay signer.
- Create `desktop/src/features/channels/ui/ChannelScreen.helpers.ts` and move `latestActiveMessage` plus related constants out of `desktop/src/features/channels/ui/ChannelScreen.tsx` before adding App-query wiring.
- Modify `desktop/src/features/channels/ui/ChannelScreen.tsx`, `desktop/src/features/messages/useIndependentThreadPanel.ts`, `desktop/src/features/messages/lib/independentThreadPanel.ts`, `desktop/src/features/projects/ui/ProjectConversationPanel.tsx`, `desktop/src/features/projects/ui/ProjectsAgentPromptPage.tsx`, `desktop/src/features/home/useHomeInboxContextMessages.ts`, and `desktop/src/features/home/ui/HomeView.tsx` to pass verified App metadata into every production `TimelineMessage` projection.
- Create `desktop/src/features/home/ui/HomeView.types.ts` and move `HomeViewProps` out of the already 999-line `HomeView.tsx` before adding new props.
- Create `desktop/src/features/search/lib/searchHitActor.ts` and `desktop/src/features/search/lib/searchHitActor.test.mjs` for tag-aware search attribution.
- Create `desktop/src/features/search/ui/MessageSearchResultRow.tsx` and move the message result row out of the already 999-line `desktop/src/features/search/ui/TopbarSearch.tsx` before adding App UI.
- Modify `desktop/src/features/search/ui/TopbarSearch.tsx` to provide verified Apps and relay identity to the extracted row.
- Create `desktop/src/features/notifications/lib/feedActor.ts` and `desktop/src/features/notifications/lib/feedActor.test.mjs` for feed attribution without a message signature.
- Create `desktop/src/app/useAppShellChannelCreation.ts` by moving the existing channel/forum create, join, and browse callbacks out of the 986-line `desktop/src/app/AppShell.tsx` before adding App notification wiring.
- Modify `desktop/src/features/notifications/use-feed-desktop-notifications.ts`, `desktop/src/features/notifications/hooks.ts`, and `desktop/src/app/AppShell.tsx` to use the App name in desktop-notification titles while keeping the relay signer as the event pubkey.

### Desktop App management

- Create `desktop/src/features/apps/lib/appCommands.ts` and `desktop/src/features/apps/lib/appCommands.test.mjs` for exact command templates and response parsing.
- Create `desktop/src/features/apps/hooks/useAppMutations.ts` for sign, publish, cache invalidation, and one-time credential state.
- Create `desktop/src/shared/api/relayPublishTracker.ts` and `desktop/src/shared/api/relayPublishTracker.test.mjs` for pending write timeouts, reconnect retry, rejection, and acknowledged `OK` messages.
- Create `desktop/src/shared/api/relayInboundFrame.ts` and `desktop/src/shared/api/relayInboundFrame.test.mjs` by extracting inbound frame parsing from the oversized relay session.
- Modify `desktop/src/shared/api/relayClientSession.ts` to delegate those concerns, expose the relay `OK` message, preserve the existing `publishEvent` return contract, and finish below 1,000 lines.
- Create `desktop/src/features/apps/ui/AppsSettingsPanel.tsx` for owner/admin list and actions.
- Create `desktop/src/features/apps/ui/AppFormDialog.tsx` for create and edit.
- Create `desktop/src/features/apps/ui/AppCredentialsDialog.tsx` for the one-time secret.
- Modify `desktop/src/features/settings/ui/SettingsView.tsx` for permission-gated navigation using `useMyRelayMembershipLookupQuery`.
- Modify `desktop/src/features/settings/ui/SettingsPanels.tsx` for the `apps` section and fail-closed deep-link behavior.
- Modify `desktop/src/testing/e2eBridge.ts` for App metadata queries and kind `9038` mock state transitions.
- Create `desktop/tests/e2e/apps-settings.spec.ts` and register it in `desktop/playwright.config.ts` under the smoke project.
- Create `desktop/tests/e2e/apps-attribution.spec.ts` and register it in `desktop/playwright.config.ts` under the smoke project.

### Mobile

- Modify `mobile/lib/shared/relay/nostr_models.dart` to add kind `39007` in sync with Desktop constants.
- Create `mobile/lib/shared/community/relay_information_provider.dart` for reusable NIP-11 fetch and validated relay `self`.
- Modify `mobile/lib/shared/community/community_icon_provider.dart` to reuse the new relay-information provider instead of retaining a second HTTP implementation.
- Create `mobile/lib/shared/relay/app_metadata.dart` for verified metadata parsing, latest-head folding, and actor resolution.
- Create `mobile/lib/shared/relay/app_metadata_provider.dart` for the active same-relay kind `39007` history query.
- Modify `mobile/lib/features/channels/timeline_message.dart` for signer and App identity fields and resolution.
- Modify `mobile/lib/features/channels/channel_detail_page.dart` and `mobile/lib/features/channels/thread_detail_page.dart` to watch the relay-self and App providers before formatting messages.
- Modify `mobile/lib/features/channels/channel_detail_page/message_bubble.dart` and `mobile/lib/features/channels/thread_detail_page/thread_message.dart` for App display and user-action suppression.
- Modify `mobile/lib/features/channels/channel_detail_page/message_list.dart` and `mobile/lib/features/channels/thread_detail_page/message_list.dart` so grouping includes `appId`.
- Create `mobile/lib/shared/widgets/app_badge.dart` and modify `mobile/lib/shared/widgets/message_author_meta.dart` for the named App badge slot.
- Create `mobile/test/shared/community/relay_information_provider_test.dart` for relay-self validation and failure behavior.
- Create `mobile/test/shared/relay/app_metadata_test.dart` for signature, schema, tie-break, and attribution cases.
- Create `mobile/test/shared/relay/app_metadata_provider_test.dart` for active-community query scoping and same-UUID isolation.
- Modify `mobile/test/features/channels/timeline_message_test.dart` for App formatting and fail-closed fallback.
- Modify `mobile/test/shared/widgets/message_author_meta_test.dart` for the App badge.
- Add App cases to `mobile/test/features/channels/channel_detail_page_test.dart` for main-timeline rendering.
- Create `mobile/test/features/channels/thread_detail_app_message_test.dart` for thread rendering and action suppression.

### Documentation and validation

- Create `docs/apps.md` for lifecycle, callback schema, delivery semantics, security, error codes, CLI examples, and secret rotation.
- Modify `README.md` to link the App integration guide.
- Modify `TESTING.md` to list the App callback E2E workflow and operator-gated live acceptance.

## Required interfaces

Implement these names and semantics.

```rust
pub const KIND_APP_ADMIN_COMMAND: u32 = 9038;
pub const KIND_APP_METADATA: u32 = 39007;
pub const APP_SECRET_BYTES: usize = 32;
pub const APP_CALLBACK_BODY_MAX_BYTES: usize = 65_536;
pub const APP_CALLBACK_METADATA_MAX_BYTES: usize = 32_768;
pub const APP_CALLBACK_METADATA_MAX_DEPTH: usize = 16;
pub const APP_CALLBACK_METADATA_MAX_NODES: usize = 1_024;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppStatus {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum AppAdminCommand {
    Create { name: String, description: Option<String>, icon_url: Option<String> },
    Update { app_id: Uuid, name: Option<String>, description: Option<String>, icon_url: Option<String> },
    RotateSecret { app_id: Uuid },
    Enable { app_id: Uuid },
    Disable { app_id: Uuid },
}
```

```rust
pub enum BeginAppAdmission {
    Existing(AppDeliveryRecord),
    PayloadConflict { existing: AppDeliveryRecord },
    Vacant(AppAdmissionGuard),
}

pub struct AppDeliveryCommit {
    pub record: AppDeliveryRecord,
    pub stored_event: StoredEvent,
}

impl AppAdmissionGuard {
    pub fn delivery_id(&self) -> Uuid;
    pub async fn list_latest_parameterized_heads(&mut self, kind: i32) -> Result<Vec<StoredEvent>>;
    pub async fn load_live_destination_channel(&mut self, channel_id: Uuid) -> Result<Option<AppAdmissionChannel>>;
    pub async fn list_named_destination_members(&mut self, channel_id: Uuid) -> Result<Vec<AppAdmissionNamedMember>>;
    pub async fn reject(self, failure: AppDeliveryFailure<'_>) -> Result<AppDeliveryRecord>;
    pub async fn deliver(self, event: &Event, route: &AppRouteSnapshot) -> Result<AppDeliveryCommit>;
}
```

```rust
impl RedisRateLimiter {
    pub async fn check_named_key(
        &self,
        key: &str,
        window_secs: u64,
        limit: u64,
    ) -> Result<RateLimitResult, AuthError>;
}
```

```typescript
export type AppMetadata = {
  appId: string;
  name: string;
  description?: string;
  picture?: string;
  status: "active" | "disabled";
  eventId: string;
  relayPubkey: string;
  updatedAt: number;
};

export type AppActor = {
  appId: string;
  name: string;
  picture?: string;
  signerPubkey: string;
};

export type RelayPublishAck = {
  event: RelayEvent;
  message: string;
};
```

```dart
final class AppMetadata {
  const AppMetadata({
    required this.appId,
    required this.name,
    required this.description,
    required this.picture,
    required this.status,
    required this.eventId,
    required this.relayPubkey,
    required this.updatedAt,
  });
}
```

## Task 1: Define protocol kinds, command validation, and audit vocabulary

**Files:**

- Create `crates/buzz-core/src/app.rs`.
- Create `crates/buzz-core/tests/app_protocol.rs`.
- Modify `crates/buzz-core/src/lib.rs`.
- Modify `crates/buzz-core/src/kind.rs`.
- Modify `crates/buzz-audit/src/action.rs`.

**Step 1: Record impact before editing existing symbols.**

Run upstream GitNexus impact for `is_command_kind`, `is_relay_admin_kind`, `is_relay_only_kind`, and `AuditAction`.
Inspect the current callers and serialization tests before proceeding.

**Step 2: Write failing protocol tests and wire them into a real test target.**

In `crates/buzz-core/tests/app_protocol.rs`, assert the numeric constants, all three kind classifications, every accepted command shape, every rejected unknown field, update-with-no-fields rejection, bounds, clearing semantics, canonical UUIDs, and safe icon validation.
In `crates/buzz-audit/src/action.rs`, add a test that names the five missing App action variants and expects their exact snake-case wire values.
The integration test must import the intended public API directly so Cargo cannot silently run zero tests.

**Step 3: Run the RED tests.**

Run `. ./bin/activate-hermit && cargo test -p buzz-core --test app_protocol -- --nocapture`.
Expected result: the test target is discovered and fails because the App protocol behavior is not present.
Run `. ./bin/activate-hermit && cargo test -p buzz-audit app_lifecycle_actions_have_stable_wire_names -- --nocapture`.
Expected result: one discovered test fails because the App action variants are not present.

**Step 4: Implement the minimum protocol behavior.**

Add kinds `9038` and `39007` and classify `9038` as command plus relay-admin and `39007` as relay-only.
Implement the exact command enum and validation contract from this plan in `buzz_core::app`.
Add the five audit variants to `AuditAction::ALL` and preserve all existing string mappings.

**Step 5: Run the GREEN tests.**

Run `. ./bin/activate-hermit && cargo test -p buzz-core --test app_protocol`.
Run `. ./bin/activate-hermit && cargo test -p buzz-core kind::tests`.
Run `. ./bin/activate-hermit && cargo test -p buzz-audit`.
Expected result: all commands exit zero with non-zero test counts.

**Step 6: Commit the protocol slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(apps): define app protocol kinds"`.

## Task 2: Add App lifecycle storage and atomic command persistence

**Files:**

- Create `migrations/0033_app_callback_notifications.sql`.
- Create `crates/buzz-db/src/app.rs`.
- Create `crates/buzz-db/tests/app_storage.rs`.
- Modify `crates/buzz-db/src/lib.rs`.

**Step 1: Record impact before editing database entry points.**

Run upstream GitNexus impact for `Db`, `replace_parameterized_event`, `insert_event_with_thread_metadata_tx`, and `insert_mentions_in_transaction`.
Read `migrations/0032_workflow_run_route_idempotency.sql` and the parameterized-replacement tests around `crates/buzz-db/src/lib.rs:5638` before writing SQL.

**Step 2: Write the failing integration test.**

Create `crates/buzz-db/tests/app_storage.rs` as a real Cargo integration target using the repository's migrated PostgreSQL test setup.
Cover migration constraints, tenant isolation, create, update and clear, rotate, enable, disable, secret-hash-only storage, missing App behavior, duplicate command claim, rollback, and lifecycle timestamps.
Assert that `created_by` is the command signer's 32-byte pubkey and cannot be changed by later lifecycle actions.
Assert that two relay-signed kind `39007` events with different `d` tags retain two independent latest heads.
Assert that two same-second updates for one App produce a monotonic timestamp and one correct live head.
Assert that an App command event, App mutation, and metadata head all disappear if a metadata-changing transaction rolls back.
Assert that rotation updates the secret hash and private `updated_at` without creating a replacement kind `39007` event.

**Step 3: Run the RED test.**

Start the existing test PostgreSQL and Redis services with `. ./bin/activate-hermit && docker compose up -d postgres redis`.
Run `. ./bin/activate-hermit && cargo test -p buzz-db --test app_storage -- --test-threads=1 --nocapture`.
Expected result: the discovered tests fail because migration `0033` and the App transaction API do not exist.

**Step 4: Add the migration.**

Create `apps` with primary key `(community_id, id)`, bounded text checks, `active|disabled` status check, 32-byte `secret_hash` check, required 32-byte `created_by`, and created/updated timestamps.
Create `app_callback_deliveries` now so the schema lands atomically, with `community_id`, delivery `id`, `app_id`, two 32-byte hashes, sanitized `event_type`, optional route JSON, final `delivered|rejected` status, optional event ID, optional stable `failure_code`, non-null `created_at`, and non-null `completed_at`.
Use primary key `(community_id, id)`, foreign key `(community_id, app_id)`, and unique `(community_id, app_id, idempotency_key_hash)`.
Add check constraints that delivered rows have `event_id` and route data but no `failure_code`, while rejected rows have `failure_code` and no event ID.
Add a trigger that rejects updates to App-delivery identity, hashes, event type, route, outcome, event, failure code, or timestamps.
Do not create or update a `schema.sql` mirror because this repository has none.

**Step 5: Implement transaction-local lifecycle operations.**

Expose tenant-scoped App load functions on `Db` and transaction helpers for command claim, create, update, rotate, status change, and kind `39007` replacement.
The relay must begin the transaction, apply the existing `buzz_deletion` write fence, claim the command event with `ON CONFLICT DO NOTHING`, and only then generate create or rotate credentials.
Use the same transaction for the domain mutation and metadata replacement on create, update, enable, and disable.
On rotate, commit the command event and secret-hash update together without replacing public metadata.
Implement kind `39007` replacement by `(community, kind, relay pubkey, d)` with the existing NIP-33 timestamp and event-ID ordering rule.
Do not call `replace_addressable_event` for App metadata.
Keep raw secret generation out of `buzz-db`; the database API accepts only a 32-byte digest.

**Step 6: Run the GREEN tests.**

Run `. ./bin/activate-hermit && cargo test -p buzz-db --test app_storage -- --test-threads=1`.
Run `. ./bin/activate-hermit && cargo test -p buzz-db parameterized -- --test-threads=1`.
Expected result: both commands exit zero and the independent-App and rapid-update assertions pass.

**Step 7: Commit the storage slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(apps): persist app lifecycle atomically"`.

## Task 3: Generalize the project route resolver without changing behavior

**Files:**

- Rename `crates/buzz-relay/src/workflow_route.rs` to `crates/buzz-relay/src/project_route.rs`.
- Modify `crates/buzz-relay/src/workflow_admission.rs`.
- Modify `crates/buzz-relay/src/lib.rs`.

**Step 1: Record impact and establish the characterization baseline.**

Run upstream GitNexus impact for `resolve_repository_identity`, `authorize_unique_project_route`, `repository_head_from_event`, and `project_head_from_event`.
Run `. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_route::tests -- --nocapture`.
Expected result: the existing exact-tier, ambiguity, claim, and project-channel tests pass before the move.

**Step 2: Rename the module mechanically.**

Run `. ./bin/activate-hermit && git mv crates/buzz-relay/src/workflow_route.rs crates/buzz-relay/src/project_route.rs`.
Update only the module declaration and the two current import sites.
Do not leave a compatibility file, re-export, or duplicate resolver.

**Step 3: Re-run the characterization tests.**

Run `. ./bin/activate-hermit && cargo test -p buzz-relay --lib project_route::tests -- --nocapture`.
Run `. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_admission::tests`.
Expected result: the same routing cases pass with no semantic changes.

**Step 4: Commit the refactor.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "refactor(relay): share project route resolution"`.

## Task 4: Execute App lifecycle commands and publish metadata

**Files:**

- Create `crates/buzz-relay/src/handlers/app_admin.rs`.
- Modify `crates/buzz-relay/src/handlers/mod.rs`.
- Modify `crates/buzz-relay/src/handlers/command_executor.rs`.
- Modify `crates/buzz-relay/src/handlers/ingest.rs`.
- Modify `crates/buzz-relay/src/lib.rs`.

**Step 1: Record impact before touching authorization and command dispatch.**

Run upstream GitNexus impact for `handle_command`, `required_scope_for_kind`, `is_global_only_kind`, `dispatch_persistent_event`, and the membership-role helper used by relay-admin commands.
Inspect the existing relay-admin kind and global-scope branches in `handlers/ingest.rs` before adding the new kind.

**Step 2: Write failing handler tests.**

Wire `handlers::app_admin` into the relay test build before running filters.
Add tests named `app_admin_rejects_non_admin`, `app_admin_create_returns_secret_once`, `app_admin_duplicate_does_not_rotate`, `app_admin_update_clears_optional_fields`, `app_admin_rotate_replaces_hash_without_metadata`, `app_admin_disable_keeps_metadata`, and `app_metadata_contains_no_authority_or_secret_tags`.
Use real signed Nostr command events and the database transaction path instead of asserting mock calls.

**Step 3: Run the RED tests.**

Run `. ./bin/activate-hermit && cargo test -p buzz-relay --lib handlers::app_admin::tests -- --test-threads=1 --nocapture`.
Expected result: the discovered tests fail on missing App command behavior, not on an unmatched filter.

**Step 4: Implement command execution.**

Authorize kind `9038` with `Scope::AdminUsers`, global-only scoping, global-token enforcement through `is_relay_admin_kind`, and an active owner/admin database role check through the existing signed ingest path.
Validate the full command before opening the transaction.
Begin a transaction, apply the deletion fence, claim the command event, and return duplicate success immediately if it already exists.
Generate UUID and secret bytes only for a newly claimed create or rotate command.
Use `rand::rngs::OsRng`, base64url without padding, SHA-256, and `subtle::ConstantTimeEq` where comparison is needed.
For create, update, enable, and disable, build the relay-signed kind `39007` event with monotonic time and replace it in the same transaction as the App mutation.
For rotate, update only the secret hash and `updated_at` in the command transaction and do not publish metadata.
Commit before dispatching any metadata event and enqueueing the safe lifecycle audit action.
Return `response:{...}` JSON in the existing Nostr `OK` message format, including `app_id` and `webhook_secret` only for successful create or rotate.
Return no secret for duplicate commands.

**Step 5: Run the GREEN tests.**

Run `. ./bin/activate-hermit && cargo test -p buzz-relay --lib handlers::app_admin::tests -- --test-threads=1`.
Run `. ./bin/activate-hermit && cargo test -p buzz-relay --lib handlers::ingest::tests`.
Run `. ./bin/activate-hermit && cargo test -p buzz-relay --lib handlers::command_executor::tests`.
Expected result: all commands exit zero and duplicate create or rotate never produces a second credential.

**Step 6: Commit the command slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(relay): manage community apps"`.

## Task 5: Add serialized App admission and shared Redis rate limiting

**Files:**

- Create `crates/buzz-db/src/app_admission.rs`.
- Create `crates/buzz-db/tests/app_admission.rs`.
- Modify `crates/buzz-db/src/lib.rs`.
- Modify `crates/buzz-pubsub/src/rate_limiter.rs`.

**Step 1: Record impact and study the existing pattern.**

Run upstream GitNexus impact for `begin_workflow_admission`, `WorkflowAdmissionGuard`, `run_rate_limit`, and `RedisRateLimiter`.
Read `crates/buzz-db/src/workflow_admission.rs` completely and preserve its lock-first transaction pattern.

**Step 2: Write failing admission and rate-key tests.**

Create a real `crates/buzz-db/tests/app_admission.rs` target that covers vacant admission, same-payload replay, different-payload conflict, two concurrent identical requests, deterministic rejection replay, dropped-guard rollback, cross-community isolation, live-channel lookup, atomic event plus delivery insert, event-mention insert, and rollback when final delivery insertion fails.
Add a rate-limiter test proving that community, App UUID, and transport IP each change the exact key `buzz:{community}:ratelimit:app_callback:{app_uuid}:{peer_ip}`.
The concurrency test must use two tasks synchronized before admission and assert one final row and one event.

**Step 3: Run the RED tests separately.**

Run `. ./bin/activate-hermit && cargo test -p buzz-db --test app_admission -- --test-threads=1 --nocapture`.
Expected result: the discovered integration tests fail because App admission is absent.
Run `. ./bin/activate-hermit && cargo test -p buzz-pubsub app_callback_rate_limit_key_is_fully_scoped -- --nocapture`.
Expected result: one discovered test fails because named App rate limiting is absent.

**Step 4: Implement the admission guard.**

Model `BeginAppAdmission` and `AppAdmissionGuard` on `WorkflowAdmissionGuard` but allocate the delivery UUID when the vacant guard is created.
Acquire the advisory transaction lock before selecting `app_callback_deliveries`.
Expose transaction-bound latest parameterized-head, live destination-channel, and current named-member reads.
Do not add an App membership query.
Make `reject` insert one final immutable rejected row and commit.
Make `deliver` insert the root event with `event::insert_event_with_thread_metadata_tx`, insert mentions with the standard transaction helper, insert the final delivered row using the same delivery UUID, commit once, and return both the final record and stored event for post-commit dispatch.
Return strongly typed records with non-optional completion time because only final rows are stored.

**Step 5: Implement named Redis limiting.**

Add a documented `check_named_key` method that delegates to the existing atomic Lua `run_rate_limit` helper.
Keep the App callback counter shared across relay processes and do not add a process-local map or a second limiter to `AppState`.
Use the provisional 60 requests per 60 seconds default in the callback caller.

**Step 6: Run the GREEN tests.**

Run `. ./bin/activate-hermit && cargo test -p buzz-db --test app_admission -- --test-threads=1`.
Run `. ./bin/activate-hermit && cargo test -p buzz-pubsub app_callback_rate_limit_key_is_fully_scoped`.
Expected result: the concurrent case creates one event and one delivery, and the dropped guard creates neither.

**Step 7: Commit the admission slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(apps): serialize callback admission"`.

## Task 6: Implement the authenticated callback and relay-signed message

**Files:**

- Create `crates/buzz-relay/src/app_sink.rs`.
- Modify `crates/buzz-relay/src/api/bridge.rs`.
- Modify `crates/buzz-relay/src/router.rs`.
- Modify `crates/buzz-relay/src/handlers/event.rs`.
- Create `crates/buzz-relay/src/message_mentions.rs`.
- Modify `crates/buzz-relay/src/workflow_sink.rs`.
- Modify `crates/buzz-relay/src/lib.rs`.
- Create `crates/buzz-test-client/tests/e2e_app_callback_notifications.rs`.

**Step 1: Record impact before editing route assembly and dispatch.**

Run upstream GitNexus impact for `build_router`, the existing bridge wrappers in `api/bridge.rs`, `dispatch_persistent_event`, `is_relay_workflow_msg`, `resolve_mention_pubkeys`, `canonical_payload_hash`, `hash_idempotency_key`, and `strip_idempotency_key`.
Read `docs/nips/NIP-MP.md` and preserve the host-derived `TenantContext` boundary.
Run `. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_sink::tests -- --nocapture` and confirm all existing `@Name` mention cases pass before extracting the helper.

**Step 2: Write the relay-backed test before adding the route.**

Create a real ignored integration target `crates/buzz-test-client/tests/e2e_app_callback_notifications.rs` that creates an owner, sends kind `9038`, publishes repository and project heads plus a live channel, and invokes the HTTP callback.
First add `callback_delivers_one_relay_signed_project_message` and assert the exact event kind, relay signature, content, six required tags, no workflow run, one delivery row, and a `202` response.

**Step 3: Run the first RED workflow.**

In terminal A, run `. ./bin/activate-hermit && cargo run -p buzz-relay` with the existing `.env` configured as documented in `TESTING.md`.
In terminal B, run `. ./bin/activate-hermit && cargo test -p buzz-test-client --test e2e_app_callback_notifications callback_delivers_one_relay_signed_project_message -- --ignored --exact --test-threads=1 --nocapture`.
Expected result: one discovered test fails because `/hooks/apps/{app_id}` is not registered.

**Step 4: Implement authentication and bounded parsing.**

Add an Axum wrapper that receives `TenantContext`, `ConnectInfo<SocketAddr>`, `Path<String>`, headers, and the raw request body.
Parse the App UUID, tenant-load the active App, base64url-decode exactly 32 secret bytes, hash and constant-time compare only `X-Webhook-Secret`, and return before body diagnostics on auth failure.
Use `axum::body::to_bytes(body, 65_536)` so an oversized body maps to the stable `400 invalid_callback` envelope rather than the router's unrelated global body limit.
Use the transport `SocketAddr::ip()` for rate identity, and use `IpAddr::UNSPECIFIED` only for tests or transports that provide no peer address.
After resolving an active App and before comparing the secret or reading the body, call the shared Redis limiter with the exact community, App, and transport-IP key so repeated authentication failures are bounded.
Call the existing shared canonical JSON and idempotency hashing helpers instead of creating another canonicalizer.
Reject unknown fields before converting the accepted fields, and retain the original accepted object minus only `idempotency_key` for payload hashing.

**Step 5: Implement route admission and event construction.**

Begin App admission only after authentication and control validation.
Resolve repository and project heads inside the admission transaction through `project_route` with an empty alias map.
Require a unique claim-valid project and its live same-community `buzz-channel`, but perform no App membership lookup.
Map every existing admission `RouteFailure` to its stable redacted `422` response and commit it through `guard.reject`.
Move the existing pure workflow `@Name` resolver to `message_mentions.rs`, keep its complete characterization tests, and use admission-transaction member data to construct deduplicated recipient `p` tags.
Construct the kind `9` event from server-resolved values only and include the six required provenance tags plus only resolved mention `p` tags.
Finalize the event and delivery through `guard.deliver`, then pass the returned `stored_event` to normal persisted-event dispatch without inserting again.
Rename `is_relay_workflow_msg` to `is_relay_generated_message` and suppress workflows when the relay signer has either `buzz:workflow` or `buzz:app`.
Emit safe metrics and structured logs with identifiers and codes only.

**Step 6: Expand the E2E matrix.**

Add tests for same-payload replay, changed-payload conflict, two concurrent requests, deterministic route rejection replay, transient rollback, wrong secret with malformed JSON, missing secret, query-only secret, ignored HMAC-only headers, malformed App UUID, disabled App, cross-community App, bad content type, malformed JSON, non-object JSON, oversized body, unknown and reserved keys, missing and invalid controls, metadata size, depth, and node bounds, canonical object-key ordering, significant array ordering and field presence, 60-per-minute rate limiting for both valid and invalid secrets, old-secret rejection immediately after rotation, metadata with two App heads, `@Name` recipient tags, and audit redaction.
Add a case where two repository announcements claimed by one project both route to the same project channel.
Add exact deterministic cases for missing and ambiguous repositories, missing and ambiguous projects, malformed or missing `buzz-channel`, deleted, archived, and cross-community channels, and an App with no channel membership that still reaches a valid route.
Add regression cases proving the existing static and dynamic workflow webhook endpoints and workflow message attribution are unchanged.
Assert that auth and validation failures create no delivery rows.
Assert that deterministic route rejection creates exactly one immutable rejected row and no event.
Assert that the event's `buzz:app-delivery` value equals the delivery primary key.
Assert that App events do not start workflows and standard event-created audit remains present.

**Step 7: Run unit and full GREEN workflows.**

Run `. ./bin/activate-hermit && cargo test -p buzz-relay --lib app_sink::tests -- --test-threads=1`.
Restart terminal A so the relay binary includes the new route.
Run `. ./bin/activate-hermit && cargo test -p buzz-test-client --test e2e_app_callback_notifications -- --ignored --test-threads=1 --nocapture`.
Expected result: every callback case passes and the concurrent test observes one event.

**Step 8: Commit the callback slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(relay): deliver app callback notifications"`.

## Task 7: Add complete App management to the CLI

**Files:**

- Create `crates/buzz-cli/src/commands/apps.rs`.
- Modify `crates/buzz-cli/src/commands/mod.rs`.
- Modify `crates/buzz-cli/src/lib.rs`.
- Modify `crates/buzz-cli/README.md`.
- Modify `crates/buzz-cli/TESTING.md`.

**Step 1: Record impact before editing Clap dispatch and client calls.**

Run upstream GitNexus impact for the top-level CLI command enum, its dispatch method, `Client::query`, `Client::submit_event`, `Client::get_public`, and `parse_write_response`.
Inspect the global `--format` handling and preserve its position before subcommands.

**Step 2: Write failing CLI tests.**

Add helper tests inside `crates/buzz-cli/src/commands/apps.rs` and Clap tests inside the existing `crates/buzz-cli/src/lib.rs` test module so the tests can exercise private command types without exposing new public API.
Prefix the Clap test function names with `apps_cli_` so the exact Cargo filter below discovers them.
Cover `apps list`, create, update, rotate-secret, enable, disable, update field conflicts, clear flags, exact kind `9038` JSON, callback URL derivation, NIP-11 `self` validation, metadata event ID and signature validation, latest-head folding, wrong-relay rejection, compact and full output, one-time secret output, and duplicate create or rotate failure when no secret is returned.

**Step 3: Run the RED test.**

Run `. ./bin/activate-hermit && cargo test -p buzz-cli commands::apps::tests -- --nocapture`.
Run `. ./bin/activate-hermit && cargo test -p buzz-cli apps_cli -- --nocapture`.
Expected result: both filters discover their intended tests and fail because the `apps` command behavior is absent.

**Step 4: Implement command parsing and mutation.**

Add `buzz apps list`, `buzz apps create`, `buzz apps update`, `buzz apps rotate-secret`, `buzz apps enable`, and `buzz apps disable`.
Use the exact `--app <uuid>` flag on update, rotate-secret, enable, and disable.
Use `--clear-description` and `--clear-icon` as conflicts for their value-taking update flags, and require at least one update mutation through Clap argument groups plus core validation.
Sign and publish kind `9038` through the existing client path.
Parse the Nostr `OK` message's `response:{...}` payload and print `app_id`, `callback_url`, and `webhook_secret` only for successful create or rotate.
Treat duplicate create or rotate without a returned secret as a write conflict with exit code `5` and instruct the user to rotate.

**Step 5: Implement verified list discovery.**

Fetch NIP-11 through `get_public("/")`, require a normalized lowercase 64-hex `self`, and query kind `39007` with `authors:[self]` and `limit:500`.
Verify every metadata event ID and signature, reject wrong authors, validate tag cardinality and UUIDs, and fold the latest head per `d`.
Compact output contains `app_id`, `name`, `status`, and `callback_url`.
Full output additionally contains `description`, `icon_url`, and `updated_at`.
Derive callback URLs from the relay HTTP base and never introduce an App-list HTTP request.

**Step 6: Run the GREEN tests and help snapshots.**

Run `. ./bin/activate-hermit && cargo test -p buzz-cli commands::apps::tests`.
Run `. ./bin/activate-hermit && cargo test -p buzz-cli apps_cli`.
Run `. ./bin/activate-hermit && cargo test -p buzz-cli`.
Run `. ./bin/activate-hermit && cargo run -p buzz-cli -- --help`.
Run `. ./bin/activate-hermit && cargo run -p buzz-cli -- apps --help`.
Expected result: tests pass and help lists every exact subcommand and clear flag.

**Step 7: Commit the CLI slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(cli): manage community apps"`.

## Task 8: Build verified Desktop App metadata and preserve search tags

**Files:**

- Create `desktop/src/features/apps/types.ts`.
- Create `desktop/src/features/apps/lib/appMetadata.ts`.
- Create `desktop/src/features/apps/lib/appMetadata.test.mjs`.
- Create `desktop/src/features/apps/lib/appActor.ts`.
- Create `desktop/src/features/apps/lib/appActor.test.mjs`.
- Create `desktop/src/features/apps/hooks/useAppsQuery.ts`.
- Modify `desktop/src/shared/constants/kinds.ts`.
- Create `desktop/src/shared/api/searchWire.ts`.
- Create `desktop/src/shared/api/searchWire.test.mjs`.
- Modify `desktop/src/shared/api/tauri.ts`.
- Modify `desktop/src/shared/api/types.ts`.
- Modify `desktop/src/testing/e2eBridge.ts`.
- Modify `desktop/src-tauri/src/models.rs`.
- Modify `desktop/src-tauri/src/nostr_convert.rs`.
- Modify `desktop/src-tauri/src/nostr_convert/tests.rs`.

**Step 1: Record impact before changing the Desktop and Tauri wire.**

Run upstream GitNexus impact for `fromRawSearchHit`, `SearchHitInfo`, the Nostr search converter, and `RelayClientSession.fetchEvents`.
Confirm that `desktop/src/shared/api/tauri.ts` is already above 1,000 lines and must shrink in this task.

**Step 2: Write failing parser and actor tests.**

In the Node test files, sign real fixtures with the existing test crypto helpers and cover valid active and disabled metadata, invalid event ID, invalid signature, wrong relay, malformed or duplicate tags, invalid UUID, latest tie-breaking, valid live App message, `p`-tag spoofing, wrong message signer, invalid message signature, duplicate `buzz:app`, missing metadata, and non-kind-`9` fallback.
Add a search-wire test that expects tags to survive snake-case-to-camel-case conversion.
Add a Tauri Rust test named `search_response_preserves_tags` that expects all Nostr tag arrays in `SearchHitInfo`.

**Step 3: Run the RED tests.**

Run `. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/apps/lib/appMetadata.test.mjs src/features/apps/lib/appActor.test.mjs src/shared/api/searchWire.test.mjs`.
Expected result: discovered tests fail because the App parsers and search tags do not exist.
Run `. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml search_response_preserves_tags -- --nocapture`.
Expected result: one discovered Rust test fails because `SearchHitInfo` omits tags.

**Step 4: Implement verified metadata and actor resolution.**

Parse and verify kind `39007` events against the active relay key and fold the latest valid head per App UUID.
Build `resolveAppActor` so live timelines require message signature verification while the explicit search/feed mode skips only that unavailable check.
Call the existing signer-aware user resolution on every failure and never inspect a `p` tag as fallback identity.
Create `useAppsQuery` on top of the existing `useRelaySelfQuery`, use query key `['apps', relaySelf]`, call the existing public `relayClient.fetchEvents` with explicit `kinds:[39007]`, `authors:[relaySelf]`, and `limit:500`, enable it only for a valid relay key, and return a stable `ReadonlyMap` keyed by App UUID.
The active community remount and relay-keyed query prevent cross-community cache reuse, so do not add a module-level singleton or `resetCommunityState` entry.

**Step 5: Split and extend the search wire.**

Move `RawSearchHit`, `RawSearchResponse`, and `fromRawSearchHit` from `tauri.ts` to `searchWire.ts` before adding tags.
Add `tags` with a Serde default to the Tauri model, map event tags in the native converter, expose them in Desktop `SearchHit`, and update the E2E bridge fixtures.
Do not grow the already oversized `tauri.ts`.

**Step 6: Run the GREEN tests and size gate.**

Run `. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/apps/lib/appMetadata.test.mjs src/features/apps/lib/appActor.test.mjs src/shared/api/searchWire.test.mjs`.
Run `. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml search_response_preserves_tags`.
Run `. ./bin/activate-hermit && just file-size-check`.
Expected result: parser, spoofing, search-wire, Tauri, and size checks pass.

**Step 7: Commit the Desktop data slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(desktop): verify app identities"`.

## Task 9: Render Desktop App attribution everywhere messages appear

**Files:**

- Modify `desktop/src/features/messages/types.ts`.
- Modify `desktop/src/features/messages/lib/formatTimelineMessages.ts`.
- Modify `desktop/src/features/messages/lib/formatTimelineMessages.test.mjs`.
- Create `desktop/src/features/messages/ui/MessageAuthorIdentity.tsx`.
- Modify `desktop/src/features/messages/ui/MessageRow.tsx`.
- Modify `desktop/src/features/messages/lib/messageGrouping.ts`.
- Modify `desktop/src/features/messages/lib/messageGrouping.test.mjs`.
- Create `desktop/src/features/channels/ui/ChannelScreen.helpers.ts`.
- Modify `desktop/src/features/channels/ui/ChannelScreen.tsx`.
- Modify `desktop/src/features/messages/useIndependentThreadPanel.ts`.
- Modify `desktop/src/features/messages/lib/independentThreadPanel.ts`.
- Modify `desktop/src/features/projects/ui/ProjectConversationPanel.tsx`.
- Modify `desktop/src/features/projects/ui/ProjectsAgentPromptPage.tsx`.
- Create `desktop/src/features/home/ui/HomeView.types.ts`.
- Modify `desktop/src/features/home/ui/HomeView.tsx`.
- Modify `desktop/src/features/home/useHomeInboxContextMessages.ts`.
- Create `desktop/src/features/search/lib/searchHitActor.ts`.
- Create `desktop/src/features/search/lib/searchHitActor.test.mjs`.
- Create `desktop/src/features/search/ui/MessageSearchResultRow.tsx`.
- Modify `desktop/src/features/search/ui/TopbarSearch.tsx`.
- Create `desktop/src/features/notifications/lib/feedActor.ts`.
- Create `desktop/src/features/notifications/lib/feedActor.test.mjs`.
- Create `desktop/src/app/useAppShellChannelCreation.ts`.
- Modify `desktop/src/features/notifications/use-feed-desktop-notifications.ts`.
- Modify `desktop/src/features/notifications/hooks.ts`.
- Modify `desktop/src/app/AppShell.tsx`.
- Create `desktop/tests/e2e/apps-attribution.spec.ts`.
- Modify `desktop/playwright.config.ts`.

**Step 1: Record impact and split near-limit components first.**

Run upstream GitNexus impact for `formatTimelineMessages`, `MessageRow`, `hasSameMessageAuthor`, `ChannelScreen`, `useIndependentThreadPanel`, `ProjectConversationPanel`, `useHomeInboxContextMessages`, `HomeView`, `TopbarSearch`, `AppShell`, `useFeedDesktopNotifications`, and `useHomeFeedNotificationState`.
Move the current author identity block to `MessageAuthorIdentity.tsx`, the ChannelScreen helper to `ChannelScreen.helpers.ts`, `HomeViewProps` to `HomeView.types.ts`, and the search message row to `MessageSearchResultRow.tsx` without changing behavior.
Move AppShell's existing channel/forum create, join, and browse callback block to `useAppShellChannelCreation.ts` without changing its returned values or dependencies, leaving `AppShell.tsx` safely below 1,000 lines before attribution edits.
Run `. ./bin/activate-hermit && cd desktop && pnpm test` and `. ./bin/activate-hermit && just file-size-check` to establish that the mechanical splits remain green.

**Step 2: Write failing unit and Playwright tests.**

Extend `formatTimelineMessages.test.mjs` with valid App identity, disabled historical App, two Apps under one relay, invalid message signature, wrong signer, malformed UUID, missing metadata, and `p`-tag spoof fallback.
Extend `messageGrouping.test.mjs` so matching relay pubkeys with different App UUIDs do not group and matching App UUIDs do group.
Add pure search-hit and feed-item actor tests that require active relay identity and verified metadata while operating in projection mode.
Create `apps-attribution.spec.ts` and seed relay `self`, signed App metadata, channel messages, a reply, a quoted message, a search hit, and a feed item through `installMockBridge`.
Assert App name, avatar fallback or picture, App badge, absent user profile popover, distinct grouping for two Apps, thread and quote attribution, search attribution, desktop-notification title, and fail-closed relay fallback for a forged case.
Assert existing human and managed-agent messages retain their current names, badges, grouping, and interaction behavior.
Switch between two mock communities that reuse one App UUID and assert the community-scoped query client never renders metadata from the previous community.
Register the spec in the smoke project before running it.

**Step 3: Run the RED tests.**

Run `. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/messages/lib/formatTimelineMessages.test.mjs src/features/messages/lib/messageGrouping.test.mjs src/features/search/lib/searchHitActor.test.mjs src/features/notifications/lib/feedActor.test.mjs`.
Expected result: the discovered tests fail because timeline, search, and feed projections lack App identity.
Run `. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- apps-attribution.spec.ts`.
Expected result: the built E2E app fails the App-attribution assertions.

**Step 4: Implement timeline and thread attribution.**

Add `signerPubkey`, `isApp`, `appId`, App display name, and App avatar fields to `TimelineMessage` without replacing the underlying relay pubkey.
Resolve App identity before existing `p`-aware author resolution in `formatTimelineMessages`.
Thread the stable App map through `ChannelScreen.tsx`, `useIndependentThreadPanel.ts`, `independentThreadPanel.ts`, `ProjectConversationPanel.tsx`, `ProjectsAgentPromptPage.tsx`, `HomeView.tsx`, and `useHomeInboxContextMessages.ts` so main timelines, independent threads, project conversations, Home context, roots, descendants, and quotes all share attribution.
Render an App badge and static App avatar/name in `MessageAuthorIdentity`, skip the profile popover and user-only controls, and include `appId` in grouping plus `React.memo` comparisons.

**Step 5: Implement search and notification attribution.**

Resolve tag-aware App actors in the extracted Topbar search row and keep ordinary result navigation unchanged.
Pass relay `self` and the stable App map from `AppShell` through `useHomeFeedNotificationState` to `useFeedDesktopNotifications`.
Use the App name in notification titles while retaining the relay event pubkey and existing channel-routing behavior.
Do not modify `desktop/src/shared/ui/markdown/MessageLinkPill.tsx` under the provisional Open Question 2 default.

**Step 6: Run the GREEN tests and interaction workflow.**

Run `. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/messages/lib/formatTimelineMessages.test.mjs src/features/messages/lib/messageGrouping.test.mjs src/features/search/lib/searchHitActor.test.mjs src/features/notifications/lib/feedActor.test.mjs`.
Run `. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- apps-attribution.spec.ts`.
Run `. ./bin/activate-hermit && just desktop-check`.
Run `. ./bin/activate-hermit && just file-size-check`.
Expected result: all views show one consistent App identity, forged cases show the relay, and every size limit passes.

**Step 7: Commit the Desktop attribution slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(desktop): show app message attribution"`.

## Task 10: Add owner/admin App management to Desktop settings

**Files:**

- Create `desktop/src/features/apps/lib/appCommands.ts`.
- Create `desktop/src/features/apps/lib/appCommands.test.mjs`.
- Create `desktop/src/features/apps/hooks/useAppMutations.ts`.
- Create `desktop/src/shared/api/relayPublishTracker.ts`.
- Create `desktop/src/shared/api/relayPublishTracker.test.mjs`.
- Create `desktop/src/shared/api/relayInboundFrame.ts`.
- Create `desktop/src/shared/api/relayInboundFrame.test.mjs`.
- Modify `desktop/src/shared/api/relayClientSession.ts`.
- Create `desktop/src/features/apps/ui/AppsSettingsPanel.tsx`.
- Create `desktop/src/features/apps/ui/AppFormDialog.tsx`.
- Create `desktop/src/features/apps/ui/AppCredentialsDialog.tsx`.
- Modify `desktop/src/features/settings/ui/SettingsView.tsx`.
- Modify `desktop/src/features/settings/ui/SettingsPanels.tsx`.
- Modify `desktop/src/testing/e2eBridge.ts`.
- Create `desktop/tests/e2e/apps-settings.spec.ts`.
- Modify `desktop/playwright.config.ts`.

**Step 1: Record impact before extending settings and bridge behavior.**

Run upstream GitNexus impact for `SettingsView`, `SettingsPanels`, `useMyRelayMembershipLookupQuery`, `signRelayEvent`, `PendingEvent`, `RelayClient.publishEvent`, `RelayClient.handleWsMessage`, `RelayClient.handleOk`, and the E2E bridge's `REQ` and `EVENT` handlers.
Keep `SettingsPanels.tsx` changes registration-only and put the complete panel implementation in `AppsSettingsPanel.tsx` so the settings registry remains below the size ceiling.
Confirm that `relayClientSession.ts` starts at 1,084 lines and must be split below 1,000 rather than grown.

**Step 2: Write failing command and E2E tests.**

Test exact kind `9038` templates, update clear behavior, response parsing, duplicate-without-secret error, and a credential-state helper that clears on close and unmount.
Test the extracted publish tracker for an accepted `OK` message, rejected `OK`, timeout, reconnect retry, and reject-all cleanup, and test inbound-frame parsing for valid and malformed `AUTH`, `EVENT`, `OK`, `EOSE`, `CLOSED`, and `NOTICE` frames.
Create `apps-settings.spec.ts` with `page.addInitScript` before `installMockBridge` and mock owner, admin, member, and unavailable-membership states.
Assert owner/admin navigation, member denial, fail-closed deep link, loading, empty, relay-error, validation-error, mutation-pending, and disabled states, verified list content, create, edit and clear, icon upload, rotate confirmation, disable confirmation, enable, one-time secret copy, secret disappearance after close, no secret after reload, and metadata persistence after disable.
Make the bridge return relay-signed kind `39007` fixtures for `REQ` and mutate App state only after valid kind `9038` `EVENT` messages.

**Step 3: Run the RED tests.**

Run `. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/apps/lib/appCommands.test.mjs src/shared/api/relayPublishTracker.test.mjs src/shared/api/relayInboundFrame.test.mjs`.
Expected result: discovered tests fail because command builders are absent.
Run `. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- apps-settings.spec.ts`.
Expected result: the built E2E app fails because the Apps settings surface is absent.

**Step 4: Implement mutations and one-time credential state.**

Move pending-write state and retry mechanics to `relayPublishTracker.ts`, add `RelayPublishAck { event, message }` and `publishEventWithAck`, resolve it from `handleOk`, and implement existing `publishEvent` as a compatibility wrapper that returns only `ack.event` so its current callers do not change.
Move raw inbound-frame parsing to `relayInboundFrame.ts`, keep connection-specific effects in the session switch, and leave `relayClientSession.ts` below 1,000 lines.
Build command templates through the same schema as the CLI and sign with `signRelayEvent` before `relayClient.publishEventWithAck`.
Parse create and rotate credentials only from the acknowledged relay `OK` message and reject accepted replies that omit the one-time secret.
Invalidate `useAppsQuery` after successful mutation and do not insert unverified optimistic metadata into the cache.
Keep the raw secret only in component state owned by `AppCredentialsDialog`, clear it on close and unmount, and never pass it to a toast, logger, query cache, URL, or storage.
Use `uploadMediaBytes` for validated image bytes and put the returned safe URL in `icon_url`.

**Step 5: Implement the permission-gated panel.**

Add an `apps` settings section only when `useMyRelayMembershipLookupQuery` reports active owner or admin.
If membership is pending, missing, or failed, hide the navigation item and render no management controls.
If a user deep-links to `apps` without permission, show the existing safe unavailable panel or redirect to the default section without issuing App mutation requests.
Show name, description, icon, status, updated time, and callback URL from verified metadata.
Do not show a Created row in version one because the approved public metadata event does not expose the private App row's `created_at`.
Expose create, edit, rotate secret, enable, and disable with explicit confirmation for rotate and disable.
Keep mutation controls pending until the relay accepts the signed command and never optimistically claim an App is active.

**Step 6: Run the GREEN tests and settings workflow.**

Run `. ./bin/activate-hermit && cd desktop && node --import ./test-loader.mjs --experimental-strip-types --test src/features/apps/lib/appCommands.test.mjs src/shared/api/relayPublishTracker.test.mjs src/shared/api/relayInboundFrame.test.mjs`.
Run `. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- apps-settings.spec.ts`.
Run `. ./bin/activate-hermit && just desktop-check`.
Run `. ./bin/activate-hermit && just file-size-check`.
Expected result: authorization, all lifecycle actions, and one-time secret behavior pass.

**Step 7: Commit the Desktop management slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(desktop): manage community apps"`.

## Task 11: Verify and render App attribution on Mobile

**Files:**

- Modify `mobile/lib/shared/relay/nostr_models.dart`.
- Create `mobile/lib/shared/community/relay_information_provider.dart`.
- Modify `mobile/lib/shared/community/community_icon_provider.dart`.
- Create `mobile/lib/shared/relay/app_metadata.dart`.
- Create `mobile/lib/shared/relay/app_metadata_provider.dart`.
- Modify `mobile/lib/features/channels/timeline_message.dart`.
- Modify `mobile/lib/features/channels/channel_detail_page.dart`.
- Modify `mobile/lib/features/channels/thread_detail_page.dart`.
- Modify `mobile/lib/features/channels/channel_detail_page/message_bubble.dart`.
- Modify `mobile/lib/features/channels/thread_detail_page/thread_message.dart`.
- Modify `mobile/lib/features/channels/channel_detail_page/message_list.dart`.
- Modify `mobile/lib/features/channels/thread_detail_page/message_list.dart`.
- Create `mobile/lib/shared/widgets/app_badge.dart`.
- Modify `mobile/lib/shared/widgets/message_author_meta.dart`.
- Create `mobile/test/shared/community/relay_information_provider_test.dart`.
- Create `mobile/test/shared/relay/app_metadata_test.dart`.
- Create `mobile/test/shared/relay/app_metadata_provider_test.dart`.
- Modify `mobile/test/features/channels/timeline_message_test.dart`.
- Modify `mobile/test/shared/widgets/message_author_meta_test.dart`.
- Modify `mobile/test/features/channels/channel_detail_page_test.dart`.
- Create `mobile/test/features/channels/thread_detail_app_message_test.dart`.

**Step 1: Record impact and inspect the actual Mobile render path.**

Run upstream GitNexus impact for `formatTimeline`, `TimelineMessage`, `ChannelDetailPage`, `ThreadDetailPage`, `MessageAuthorMeta`, and `communityIconProvider`.
Inspect both `message_list.dart` files so grouping changes cover main timelines and threads.

**Step 2: Write failing relay-info, parser, formatter, and widget tests.**

Test NIP-11 success, malformed or missing `self`, HTTP failure, and active-community changes.
Construct `nostr.Event(..., verify: true)` fixtures so `package:nostr` checks both canonical event ID and Schnorr signature.
Cover the same metadata schema, latest-head, disabled-history, valid App message, wrong signer, invalid signature, malformed UUID, missing metadata, duplicate tag, `p` spoof, and two-App grouping cases as Desktop.
Assert normal human and agent author formatting remains unchanged.
Override two relay sessions that reuse one App UUID and assert the provider discards the first community's map after the active community changes.
Add full widget cases for main-channel and thread App rendering, App badge, static avatar/name, no user sheet, and no user-only management actions.
Use `ProviderScope` overrides and the existing fake notifier patterns.

**Step 3: Run the RED tests.**

Run `. ./bin/activate-hermit && cd mobile && flutter test test/shared/community/relay_information_provider_test.dart test/shared/relay/app_metadata_test.dart test/shared/relay/app_metadata_provider_test.dart test/features/channels/timeline_message_test.dart test/shared/widgets/message_author_meta_test.dart test/features/channels/channel_detail_page_test.dart test/features/channels/thread_detail_app_message_test.dart`.
Expected result: discovered tests fail because relay-self, App metadata, and App UI behavior are absent.

**Step 4: Implement reusable relay information and verified App metadata.**

Move the reusable NIP-11 URI and HTTP behavior out of `community_icon_provider.dart` into `relay_information_provider.dart` and make the icon provider consume it.
Validate NIP-11 `self` as normalized lowercase 64-hex before exposing it.
Query `NostrFilter(kinds:[39007], authors:[relaySelf], limit:500)` through the active `RelaySession`.
Reconstruct each event with the validating `package:nostr` constructor, validate tag cardinality and UUIDs, and fold the latest valid head per App.
Keep the provider community-scoped through its watched relay configuration and session, with no module-level cache.

**Step 5: Implement timeline, thread, and author UI behavior.**

Add signer and App fields to `TimelineMessage` and pass relay self plus the App map into `formatTimeline` from both pages.
Resolve the App before existing profile or mention attribution and fall back to the relay signer on every validation failure.
Render the App metadata name and picture or deterministic fallback, add `AppBadge`, and disable profile-sheet and user-management interactions.
Include `appId` in both grouping algorithms.
Use only `context.colors`, `context.textTheme`, `Grid`, and `Radii` tokens for new UI.

**Step 6: Run the GREEN tests and Mobile gates.**

Run `. ./bin/activate-hermit && cd mobile && dart format --output=none --set-exit-if-changed .`.
Run `. ./bin/activate-hermit && cd mobile && flutter analyze`.
Run `. ./bin/activate-hermit && cd mobile && flutter test test/shared/community/relay_information_provider_test.dart test/shared/relay/app_metadata_test.dart test/shared/relay/app_metadata_provider_test.dart test/features/channels/timeline_message_test.dart test/shared/widgets/message_author_meta_test.dart test/features/channels/channel_detail_page_test.dart test/features/channels/thread_detail_app_message_test.dart`.
Run `. ./bin/activate-hermit && just file-size-check`.
Expected result: all targeted tests and static gates pass without a new `StatefulWidget`.

**Step 7: Exercise the real Mobile workflow when practical.**

Run `. ./bin/activate-hermit && just mobile-dev` against an already available simulator and configured community.
Open a project channel containing an App callback and its thread, then verify the App name, badge, avatar, grouping, and absent user actions.
Record the simulator, community, and observed workflow in the implementation handoff.
If no suitable deployed App message exists, report this runtime check as operator-gated and rely on the relay-backed plus widget tests without inventing evidence.

**Step 8: Commit the Mobile slice.**

Run the mandatory `detect_changes`, diff, and status checks.
Commit with `. ./bin/activate-hermit && git commit -s -m "feat(mobile): show app message attribution"`.

## Task 12: Document, validate, and perform end-to-end acceptance

**Files:**

- Create `docs/apps.md`.
- Modify `README.md`.
- Modify `TESTING.md`.

**Step 1: Preserve task ownership for any defect found by final validation.**

Do not make an untested production fix inside this documentation task.
If a final gate exposes a product defect, return to the task that owns the affected symbol, run its upstream impact analysis, add a reproducing RED test, implement the minimum fix, and rerun that task's GREEN commands before resuming here.
The repository has no dedicated App-documentation test harness, so do not invent a grep-only test that can pass while the documented workflow is wrong.
Use the real CLI help, callback E2E, and client workflows below as the documentation's executable evidence.

**Step 2: Write the operator guide.**

Document owner/admin prerequisites, CLI and Desktop lifecycle, the one-time credential warning, exact callback URL, exact headers and JSON schema, reserved keys, size limits, routing behavior, retry/idempotency behavior, every status and stable code, rotation and disable recovery, and the fact that clients trust only the active relay signer plus verified metadata.
Include a redacted `curl` example that takes the secret from an environment variable and never places it in a query string.
Link the guide from `README.md` and the relay-backed procedure from `TESTING.md`.

**Step 3: Run focused repository tests.**

Run `. ./bin/activate-hermit && cargo test -p buzz-core`.
Run `. ./bin/activate-hermit && cargo test -p buzz-audit`.
Run `. ./bin/activate-hermit && cargo test -p buzz-pubsub`.
Run `. ./bin/activate-hermit && cargo test -p buzz-db -- --test-threads=1`.
Run `. ./bin/activate-hermit && cargo test -p buzz-relay`.
Run `. ./bin/activate-hermit && cargo test -p buzz-cli`.
Run `. ./bin/activate-hermit && cargo test --manifest-path desktop/src-tauri/Cargo.toml`.
Run `. ./bin/activate-hermit && cd desktop && pnpm test`.
Run `. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- apps-attribution.spec.ts apps-settings.spec.ts`.
Run `. ./bin/activate-hermit && cd mobile && flutter test`.
Expected result: every command exits zero.

**Step 4: Run the relay-backed App acceptance suite.**

Start a freshly built relay against the configured PostgreSQL and Redis services.
Run `. ./bin/activate-hermit && cargo test -p buzz-test-client --test e2e_app_callback_notifications -- --ignored --test-threads=1 --nocapture`.
Expected result: all lifecycle, authentication, routing, idempotency, concurrency, redaction, and event assertions pass against the real HTTP, WebSocket, PostgreSQL, and Redis path.

**Step 5: Run repository-wide quality gates.**

Run `. ./bin/activate-hermit && just file-size-check`.
Run `. ./bin/activate-hermit && just ci`.
Run `. ./bin/activate-hermit && just test` because `buzz-relay` and `buzz-db` changed.
Expected result: formatting, lint, static checks, Rust tests, Desktop tests and builds, Tauri tests, Mobile checks and tests, web build, and integration tests all pass.

**Step 6: Perform deployed gigo acceptance when credentials exist.**

On the deployed Mac mini relay, run `buzz --format compact apps create --name Archon` in the `gigo-harness` community and configure the returned callback URL and secret in both current Archon provider bindings.
Invoke the existing Archon manual callback once for `harness-service` and once for `agentic-os-plan`, using distinct idempotency keys.
Observe both messages in the `gigo-harness` channel as `Archon · App` on Desktop and Mobile.
Replay one provider event, send a conflicting payload with the same key, disable the App, and retry the callback.
Observe no duplicate message, `409` for the conflict, and `404` after disable.
Rotate the secret, verify the old secret receives `401`, and verify the new secret delivers a new message.
Inspect database and audit records through approved operator access and confirm that no raw secret, raw idempotency key, headers, or callback content was retained outside the message event itself.
Disable the old `Archon Notifications` workflow only after both repository callbacks, attribution, replay, and audit checks pass.
Record the community, clients, and exact observed workflow without recording any secret.
If credentials are unavailable, mark only this deployed step as operator-gated and do not weaken the mandatory hermetic acceptance suite.

**Step 7: Complete the final change audit and commit documentation or test fixes.**

Run GitNexus `detect_changes({scope: "compare", base_ref: "main"})` and confirm only the expected App lifecycle, callback, route, client-attribution, and documentation flows changed.
Run `. ./bin/activate-hermit && git diff --check`.
Run `. ./bin/activate-hermit && git status --short`.
Commit remaining documentation and acceptance fixes with `. ./bin/activate-hermit && git commit -s -m "docs(apps): document callback integrations"`.

## Final acceptance criteria

- Owners and admins can list, create, edit, rotate, enable, and disable Apps through both CLI and Desktop without an App-list HTTP endpoint.
- Members and unauthenticated users cannot manage Apps, and Desktop fails closed while role data is pending or unavailable.
- Create and rotate expose a 256-bit raw secret once, and no persistence, logs, audits, caches, URLs, screenshots, or subsequent responses contain it.
- Multiple kind `39007` App heads coexist under one relay signer and rapid updates select the intended latest head.
- A correct authenticated callback resolves exactly one current project and one live same-community channel through existing project claims.
- A callback cannot choose its community, App identity, project, channel, author, kind, or raw tags.
- Identical sequential and concurrent callbacks produce one immutable delivery and one kind `9` event.
- Reusing a key for a different canonical payload returns `409`, including differences in arrays or field presence.
- Deterministic route failure is stored once and replayed, while authentication, missing-key, validation, rate-limit, and transient failures do not create a delivered event.
- The kind `9` event and delivered ledger row commit atomically and share the same delivery UUID.
- The event is relay-signed, contains exactly the required server-derived tags, preserves validated content, and cannot trigger a workflow loop.
- Desktop timeline, thread, quote, search, supported previews, and desktop notifications show the verified App identity.
- Mobile channel and thread messages show the same verified App identity.
- Invalid signatures, wrong relay keys, malformed App tags, missing metadata, and `p`-tag spoofing always fall back to the relay signer.
- Disabled metadata continues to attribute historical messages while new callbacks receive `404`.
- Lifecycle audits are present and safe, and ordinary delivered messages retain the standard event-created audit.
- All focused tests, the real relay-backed callback suite, `just file-size-check`, `just ci`, and `just test` pass.
- Deployed gigo acceptance is either recorded with environment details or explicitly marked operator-gated.
