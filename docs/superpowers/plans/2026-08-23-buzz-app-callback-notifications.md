# Buzz App Callback Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a first-class community App that authenticates Archon callbacks, routes each repository to exactly one project channel, and publishes relay-signed kind `9` notifications under App attribution instead of a workflow owner.

**Architecture:** Store community Apps and hashed callback secrets in Postgres.
Authenticate `POST /hooks/apps/{app_id}` with `X-Webhook-Secret`, reuse the existing NIP-MP repository and project resolver behind a membership-free destination check, persist one delivery ledger row atomically with the kind `9` event, and publish relay-signed kind `39007` metadata so clients render `Archon · App`.

**Tech Stack:** Rust, Postgres, Axum, sqlx, sha2, rand, subtle, Nostr parameterized-replaceable events, buzz-cli clap, Desktop React 19, Mobile Flutter, node:test, Playwright mock bridge, Flutter widget tests, Hermit, and `just`.

**Spec:** [docs/superpowers/specs/2026-08-23-buzz-app-callback-notifications-design.md](docs/superpowers/specs/2026-08-23-buzz-app-callback-notifications-design.md)

**Approved brainstorm input:** `2026-08-23-buzz-app-callback-notifications-design`.

**Product contract:** [VISION.md](VISION.md) currently gives humans and agents one portable Nostr identity.
This change adds a deliberate community-local App actor attested by the community relay.
It does not create an App keypair, channel membership, or portable reputation.
[VISION_PROJECTS.md](VISION_PROJECTS.md) and [docs/nips/NIP-MP.md](docs/nips/NIP-MP.md) keep `buzz-channel` as destination metadata after claim-valid project resolution.

## Product tension

VISION identity is pubkey-portable.
An App is community-local and relay-attested.
That tension is intentional and in-scope.
Do not close it by minting an App keypair or converting the App into a workflow owner.

## Global Constraints

- Implement the approved App ingress path only.
- Do not convert, intercept, or remove `/hooks/{workflow_id}`.
- Do not give an App a Nostr keypair, agent identity, channel membership, read capability, mentions inbox, DMs, or presence.
- Do not accept callback-supplied `channel_id`, project coordinate, community identifier, App identifier, or App display name as routing authority.
- Do not add JSONata, Jolt, or a notification template engine.
- Do not add App deletion or Mobile App management UI.
- Do not implement Buzz reply delivery to GitHub or Archon.
- The destination is server-derived from repository identity plus exactly one claim-valid project's live `buzz-channel`.
- App names are not unique.
- The App UUID is the stable callback and attribution identifier.
- Buzz generates at least 256 bits of secret material and stores only SHA-256 of the encoded secret.
- The raw secret appears only in the create and rotate command results.
- Kind `39007` is relay-signed parameterized-replaceable App metadata with address `(relay_pubkey, 39007, app_id)`.
- Kind `9038` is the signed App admin command.
- Successful App messages are relay-signed kind `9` with `h`, `buzz:app`, repository `a`, project `a`, `buzz:app-delivery`, and `buzz:app-event`.
- App messages must not carry an author-attributing `p` tag for Kevin, the App creator, or a workflow owner.
- Mention `p` tags remain mention recipients only.
- Clients accept App attribution only for a valid relay-signed `buzz:app` UUID whose latest kind `39007` is signed by the same relay in the same community.
- If App attribution fails, render the actual relay signer and never the first mention `p` tag.
- Disabled App metadata remains queryable so historical messages stay resolvable.
- Authentication failure and a missing usable idempotency key create no delivery record.
- Identical retries and concurrent identical callbacks create one delivery and one message.
- Same key plus different payload returns `409` and changes nothing.
- Failures never post an error message to any channel.
- No production `unsafe`, `unwrap()`, or `expect()`.
- New public Rust and TypeScript APIs need doc comments.
- Keep every edited product file at or below the 1000-line ratchet.
- Activate Hermit before every shell command with `. ./bin/activate-hermit && ...`.
- Sign every commit with `git commit -s`.
- Follow TDD for each behavior: add the named failing test, run it and confirm the expected failure, implement only that task, run the same test green, then commit.
- Do not add production code for a later task to make an earlier task pass.

## Verified Repository Facts

- Workflow webhooks bind community from `Host`, authenticate with `X-Webhook-Secret`, and live at `POST /hooks/{id}` in `crates/buzz-relay/src/router.rs`.
- Dynamic workflow routing already lives in `crates/buzz-relay/src/workflow_route.rs` and `crates/buzz-workflow/src/routing.rs`.
- Workflow messages are relay-signed kind `9` with a first `p` tag for the workflow owner in `crates/buzz-relay/src/workflow_sink.rs`.
- Desktop `resolveEventAuthorPubkey` trusts delegated `p` / `actor` tags only on the active relay signer.
- Command kinds route through `is_command_kind` into `handle_command` and return secrets as `message: "response:{...}"`.
- Relay-only kinds are rejected from clients by `is_relay_only_kind`.
- Kind `39006` is taken.
- Kind `9037` is unused.
- Kind `9038` is unused.
- Latest SQL migration is `migrations/0032_workflow_run_route_idempotency.sql`.
- Owner/admin gating uses `get_relay_member` and Desktop `canManageCommunityMembers`.
- Settings section rendering is exhaustive in `renderSettingsSection`.
- Desktop search hits are built from full events in `desktop/src-tauri/src/nostr_convert.rs` and currently drop tags.
- Workflow triggering already skips relay-signed `buzz:workflow` messages in `crates/buzz-relay/src/handlers/event.rs`.

## GitNexus Gates

GitNexus MCP tools and `.gitnexus/run.cjs` may be absent in a planning session.
The implementation session must still follow the repository GitNexus policy.

- Before changing an existing function, class, method, or exported type, run `impact({ target: "<symbol>", direction: "upstream" })`.
- Expected high-touch symbols are `is_command_kind`, `is_relay_only_kind`, `handle_command`, `required_scope_for_kind`, `workflow_webhook`, `resolve_repository_identity`, `authorize_unique_project_route`, `resolveEventAuthorPubkey`, `formatTimelineMessages`, `renderSettingsSection`, and `canManageCommunityMembers`.
- Report direct callers, affected execution flows, and risk before editing.
- Stop and warn before editing if GitNexus reports HIGH or CRITICAL on an unexpected surface such as git push policy.
- If the index is stale and `.gitnexus/run.cjs` exists, run `. ./bin/activate-hermit && node .gitnexus/run.cjs analyze` from the repository root.
- If no `.gitnexus/run.cjs` exists, run `. ./bin/activate-hermit && npx gitnexus analyze` from the repository root before retrying the MCP call.
- Before every commit, stage only that task's files and run `detect_changes({ scope: "staged" })`.
- Before final handoff, run `detect_changes({ scope: "compare", base_ref: "main" })`.
- If GitNexus MCP tools remain unavailable after refreshing the index, record that in the handoff and use `git diff --stat`, `git diff --name-only`, and `git diff --check` as fallback scope evidence.

## Resolved Implementation Decisions

These are the safe defaults for product and layout choices the spec left to implementation.

- Secret material is 32 cryptographically random bytes encoded as 64 lowercase hex characters.
- `secret_hash` is SHA-256 of those UTF-8 hex bytes.
- Constant-time compare uses `subtle::ConstantTimeEq` on the 32-byte digests.
- Callback URL is `{relay_http_origin}/hooks/apps/{app_id}` with no query string.
- Kind `9038` is a command kind processed by `handle_command`.
- Kind `9038` is community-global and must not be channel-scoped even if a stray `h` tag is present.
- Kind `9038` uses `Scope::AdminUsers`.
- Kind `9038` is exempt from the timeout write-block and rejects banned actors inside the handler, matching kinds `9030`–`9033`.
- Kind `9038` content is JSON with `deny_unknown_fields` and one `action` field.
- Create and rotate return `message: "response:{...}"` containing `app_id`, `callback_url`, and `secret`.
- Duplicate `9038` events return `duplicate: already processed` and never reprint a secret.
- Kind `39007` is relay-only and parameterized-replaceable.
- Kind `39007` is stored with `channel_id = None` through `replace_addressable_event`.
- Kind `39007` tags are `d`, `name`, `status`, and optional `picture`.
- Kind `39007` content is the public description string, empty when absent.
- App admin authorization requires a current `relay_members` role of `owner` or `admin`.
- Open relays with no owner/admin roster reject App commands.
- Icon URLs reuse `validate_workspace_icon` rules from kind `9033`.
- App name is 1..=64 Unicode chars with no control characters and is not trimmed.
- Whitespace-only names fail validation.
- Description is optional, max 500 chars, no control characters except newline and tab.
- Register `POST /hooks/apps/{app_id}` in the API router before `POST /hooks/{id}`.
- App callbacks do not accept `?secret=`.
- `X-Webhook-Signature-V2` and `X-Webhook-Timestamp` are ignored.
- Allowed callback top-level keys are only `idempotency_key`, `repository_name`, `event_type`, `content`, and `metadata`.
- Reserved control keys are `channel_id`, `channel`, `project_id`, `project`, `project_coordinate`, `community_id`, `community`, `app_id`, `app`, and `app_name`.
- Reserved keys at the top level or anywhere inside `metadata` return `400` with code `reserved_control_field`.
- Unknown top-level keys return `400` with code `unknown_field`.
- `metadata` defaults to `{}`, max 8192 serialized bytes, max nesting depth 4, max 32 object keys at any object.
- `event_type` is 1..=64 chars in `[A-Za-z0-9._-]`.
- `content` uses the existing 65536-byte message content limit and rejects empty or whitespace-only bodies.
- Request bodies remain bounded by the existing 1 MiB `RequestBodyLimitLayer`.
- Canonical payload hash is `canonical_payload_hash` from `buzz-workflow::routing` after removing `idempotency_key`.
- App routing calls `resolve_repository_identity` with an empty alias map.
- App destination authorization requires the channel to exist, be undeleted, be unarchived, and belong to the host community.
- App destination authorization does not require channel membership.
- Rate limit is 60 requests per 60 seconds per `(community_id, app_id, source_ip)` using a process-local `DashMap` sliding window.
- Successful HTTP `202` body is `{ "delivery_id", "event_id", "status": "delivered" }`.
- Replay of an identical delivered key returns the same `202` body.
- Replay of a stored deterministic rejection returns the original redacted status and code.
- Delivery unique constraint is `(community_id, app_id, idempotency_key_hash)`.
- Admission uses a transaction-scoped advisory lock on `buzz_app_admission:{community}:{app}:{hex(key_hash)}`.
- Message insert and delivered-row insert share one Postgres transaction.
- Post-persist fan-out uses existing `dispatch_persistent_event`.
- App messages skip workflow triggering through a `buzz:app` marker, matching `buzz:workflow`.
- Mention `p` tags may still be appended after `@Name` resolution, but never as the author-attributing first `p`.
- Audit actions are `AppCreated`, `AppUpdated`, `AppSecretRotated`, `AppEnabled`, and `AppDisabled`.
- Audit `object_id` is the App UUID.
- Audit `detail` contains only `app_id` and `action`, never secrets or hashes.
- CLI compact listing omits `secret`, `secret_hash`, and callback credentials.
- Desktop Apps settings are visible only when `canManageCommunityMembers` is true.
- Closing the one-time credentials dialog drops the secret from React state.
- App authors do not open `UserProfilePopover`.
- `TimelineMessage.isApp` and `TimelineMessage.appId` carry App attribution.
- `TimelineMessage.pubkey` is omitted for accepted App actors so the avatar is not a user popover.
- `TimelineMessage.signerPubkey` remains the relay signer.
- Search hits include `tags` so Desktop can resolve `buzz:app` without a second event fetch.
- Search and feed hits skip signature re-verification and accept App attribution when `hit.pubkey` equals the active relay pubkey, `buzz:app` is a UUID, and matching kind `39007` metadata exists.
- Timeline and Mobile message surfaces still require `verifyEvent` for App attribution.
- App metadata is loaded with React Query / Riverpod from kind `39007` and needs no `resetCommunityState` module cache.
- Mobile renders App name, icon, and an `App` badge on message and thread surfaces only.

## File Map

| File | Responsibility |
|------|----------------|
| Modify `crates/buzz-core/src/kind.rs` | Add `KIND_APP_METADATA = 39007` and `KIND_APP_ADMIN_COMMAND = 9038`, register in `ALL_KINDS`, `is_command_kind`, `is_relay_only_kind`, and compile-time asserts |
| Create `crates/buzz-core/src/app.rs` | App command JSON, callback JSON, secret generation/hashing, reserved fields, failure codes |
| Modify `crates/buzz-core/src/lib.rs` | Export `app` |
| Create `migrations/0033_apps_and_callback_deliveries.sql` | `apps` and `app_callback_deliveries` tables, constraints, indexes, immutability trigger |
| Modify `schema/schema.sql` | Keep the checked-in schema in sync |
| Create `crates/buzz-db/src/app.rs` | App CRUD |
| Create `crates/buzz-db/src/app_admission.rs` | Delivery admission guard, unique concurrent insert, atomic event+delivery persist |
| Modify `crates/buzz-db/src/lib.rs` | Module exports and `Db` wrappers |
| Create `crates/buzz-relay/src/project_route.rs` | Move current `workflow_route.rs` contents here without membership checks |
| Modify `crates/buzz-relay/src/workflow_route.rs` | Re-export `project_route` so existing workflow tests keep compiling during the move, then delete after workflow_admission imports are updated |
| Modify `crates/buzz-relay/src/workflow_admission.rs` | Import the shared resolver |
| Create `crates/buzz-relay/src/app_admin.rs` | Kind `9038` execution, kind `39007` publish, audit |
| Create `crates/buzz-relay/src/app_admission.rs` | Callback HTTP orchestration |
| Create `crates/buzz-relay/src/app_sink.rs` | Relay-signed App kind `9` builder without author `p` |
| Modify `crates/buzz-relay/src/handlers/command_executor.rs` | Dispatch `KIND_APP_ADMIN_COMMAND` |
| Modify `crates/buzz-relay/src/handlers/ingest.rs` | Scope, global-kind, timeout exemption, and command routing |
| Modify `crates/buzz-relay/src/handlers/event.rs` | Skip workflow triggering for `buzz:app` |
| Modify `crates/buzz-relay/src/router.rs` | Register `/hooks/apps/{app_id}` |
| Modify `crates/buzz-relay/src/lib.rs` | Declare new modules |
| Modify `crates/buzz-relay/src/state.rs` | App callback rate limiter |
| Modify `crates/buzz-audit/src/action.rs` | App audit actions |
| Create `crates/buzz-cli/src/commands/apps.rs` | `buzz apps` subcommands |
| Modify `crates/buzz-cli/src/commands/mod.rs` | Module |
| Modify `crates/buzz-cli/src/lib.rs` | Clap `Apps` command and dispatch |
| Create `desktop/src/shared/lib/appActor.ts` | Pure App actor resolution |
| Create `desktop/src/shared/lib/appActor.test.mjs` | Desktop App attribution unit tests |
| Modify `desktop/src/shared/lib/authors.ts` | Stop using first `p` when `buzz:app` is present |
| Modify `desktop/src/shared/lib/authors.test.mjs` | Pin the fail-closed `p` fallback |
| Modify `desktop/src/shared/constants/kinds.ts` | `KIND_APP_METADATA`, `KIND_APP_ADMIN_COMMAND` |
| Modify `desktop/src/features/messages/types.ts` | `isApp`, `appId` |
| Modify `desktop/src/features/messages/lib/formatTimelineMessages.ts` | Feed App name, icon, badge fields |
| Modify `desktop/src/features/messages/ui/MessageRow.tsx` | Render App badge and skip user popover |
| Create `desktop/src/features/messages/ui/MessageAppBadge.tsx` | `App` badge |
| Modify `desktop/src-tauri/src/models.rs` | Search hit tags |
| Modify `desktop/src-tauri/src/nostr_convert.rs` | Copy event tags onto search hits |
| Modify `desktop/src/shared/api/searchTypes.ts` | Optional tags |
| Modify `desktop/src/shared/api/tauri.ts` | Pass tags through `fromRawSearchHit` |
| Modify `desktop/src/features/search/ui/SearchResultItem.tsx` | App author rendering |
| Create `desktop/src/features/apps/appModels.ts` | Kind `39007` parse, used by actor resolution and settings |
| Create `desktop/src/features/apps/useAppsQuery.ts` | List live App metadata for timeline, search, and settings |
| Create `desktop/src/features/apps/appCommands.ts` | Sign and submit kind `9038` |
| Create `desktop/src/features/apps/ui/AppsSettingsCard.tsx` | Settings list |
| Create `desktop/src/features/apps/ui/CreateAppDialog.tsx` | Create form |
| Create `desktop/src/features/apps/ui/AppCredentialsDialog.tsx` | One-time URL and secret |
| Create `desktop/src/features/apps/ui/EditAppDialog.tsx` | Metadata edit |
| Modify `desktop/src/features/settings/ui/SettingsPanels.tsx` | Section type and render case |
| Modify `desktop/src/features/settings/ui/SettingsView.tsx` | Communities nav + owner/admin gate |
| Modify `desktop/tests/helpers/settings.ts` | `apps` section |
| Create `desktop/tests/e2e/apps-settings.spec.ts` | Management UI |
| Create `desktop/tests/e2e/apps-attribution.spec.ts` | Timeline App badge |
| Modify `desktop/playwright.config.ts` | Register smoke specs |
| Modify `desktop/src/testing/e2eBridge.ts` | Mock kind `9038` and `39007` |
| Modify `mobile/lib/shared/relay/nostr_models.dart` | Kind constants |
| Create `mobile/lib/shared/relay/app_actor.dart` | Mobile App actor resolution |
| Create `mobile/test/shared/relay/app_actor_test.dart` | Mobile attribution tests |
| Modify `mobile/lib/shared/widgets/message_author_meta.dart` | Optional App badge |
| Modify `NOSTR.md` | Document kinds `39007` and `9038` |
| Create `crates/buzz-test-client/tests/e2e_app_callback_notifications.rs` | Relay-backed App flow |

Do not create or modify any other product file unless a later task names it because a 1000-line split is required.

## Out of Scope

- App deletion.
- Mobile App management UI.
- Web client App management.
- HMAC callback authentication.
- Repository alias maps on Apps.
- Converting the old Archon workflow automatically.
- Branch-channel routing.
- Outbound GitHub replies.

## Required Implementation Order

Execute the tasks in dependency order `1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12`.
Task 5 must not register `/hooks/apps/{app_id}` until App message emission and admission are tested behind `handle_app_callback`.
Task 5's last implementation step is the router registration.

---

### Task 1: Kind registry, App protocol, and secret hashing

**Files:**

- Create: `crates/buzz-core/src/app.rs`
- Modify: `crates/buzz-core/src/kind.rs`
- Modify: `crates/buzz-core/src/lib.rs`

**Interfaces:**

- Consumes: existing `is_command_kind`, `is_relay_only_kind`, `ALL_KINDS`, `sha2`, `rand`, `subtle`.
- Produces:
  - `pub const KIND_APP_METADATA: u32 = 39007;`
  - `pub const KIND_APP_ADMIN_COMMAND: u32 = 9038;`
  - `pub enum AppStatus { Active, Disabled }` serialized as `active` / `disabled`
  - `pub enum AppAdminAction { Create, Update, RotateSecret, Enable, Disable }` serialized as snake_case
  - `pub struct AppAdminCommand { pub action: AppAdminAction, pub app_id: Option<Uuid>, pub name: Option<String>, pub description: Option<String>, pub icon_url: Option<String> }`
  - `pub struct AppCallbackRequest { pub idempotency_key: String, pub repository_name: String, pub event_type: String, pub content: String, pub metadata: serde_json::Value }`
  - `pub enum AppFailure` with `code()`, `http_status()`, and `redacted_message()`
  - `pub fn parse_app_admin_command(content: &str) -> Result<AppAdminCommand, AppFailure>`
  - `pub fn parse_app_callback_body(body: &[u8]) -> Result<AppCallbackRequest, AppFailure>`
  - `pub fn generate_app_callback_secret() -> Result<String, AppFailure>`
  - `pub fn hash_app_callback_secret(secret: &str) -> [u8; 32]`
  - `pub fn app_secrets_equal(provided_hash: &[u8; 32], stored_hash: &[u8; 32]) -> bool`

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `is_command_kind`, `is_relay_only_kind`, and `ALL_KINDS`.
Report callers and risk.
The expected callers are ingest scope, command routing, and kind registry tests.
Stop only if impact shows git transport or NIP-29 group metadata writers.

- [ ] **Step 2: Write the failing protocol tests**

Create `crates/buzz-core/src/app.rs` with tests first in a `#[cfg(test)] mod tests` block that imports the public API.
Put this complete test module in `app.rs` and keep production items as `todo!()` until Step 4.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn kinds_are_stable() {
        assert_eq!(crate::kind::KIND_APP_METADATA, 39007);
        assert_eq!(crate::kind::KIND_APP_ADMIN_COMMAND, 9038);
        assert!(crate::kind::is_command_kind(crate::kind::KIND_APP_ADMIN_COMMAND));
        assert!(crate::kind::is_relay_only_kind(crate::kind::KIND_APP_METADATA));
        assert!(crate::kind::is_parameterized_replaceable(crate::kind::KIND_APP_METADATA));
        assert!(!crate::kind::is_relay_only_kind(crate::kind::KIND_APP_ADMIN_COMMAND));
    }

    #[test]
    fn generate_secret_is_256_bit_hex() {
        let a = generate_app_callback_secret().expect("secret a");
        let b = generate_app_callback_secret().expect("secret b");
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
        assert_ne!(a, b);
        assert_eq!(hash_app_callback_secret(&a).len(), 32);
        assert!(app_secrets_equal(
            &hash_app_callback_secret(&a),
            &hash_app_callback_secret(&a)
        ));
        assert!(!app_secrets_equal(
            &hash_app_callback_secret(&a),
            &hash_app_callback_secret(&b)
        ));
    }

    #[test]
    fn parse_create_command_accepts_optional_fields() {
        let cmd = parse_app_admin_command(
            r#"{"action":"create","name":"Archon","description":"CI","icon_url":"https://example.com/a.png"}"#,
        )
        .expect("create");
        assert_eq!(cmd.action, AppAdminAction::Create);
        assert_eq!(cmd.name.as_deref(), Some("Archon"));
    }

    #[test]
    fn parse_create_command_rejects_empty_name_and_unknown_fields() {
        assert_eq!(
            parse_app_admin_command(r#"{"action":"create","name":""}"#).unwrap_err(),
            AppFailure::InvalidCommand
        );
        assert_eq!(
            parse_app_admin_command(r#"{"action":"create","name":"Archon","extra":1}"#)
                .unwrap_err(),
            AppFailure::InvalidCommand
        );
    }

    #[test]
    fn callback_body_requires_control_fields_and_rejects_reserved_keys() {
        let ok = parse_app_callback_body(
            br#"{"idempotency_key":"k1","repository_name":"harness-service","event_type":"workflow.run.completed","content":"ok"}"#,
        )
        .expect("valid");
        assert_eq!(ok.repository_name, "harness-service");
        assert_eq!(ok.metadata, json!({}));
        assert_eq!(
            parse_app_callback_body(
                br#"{"idempotency_key":"k1","repository_name":"harness-service","event_type":"workflow.run.completed","content":"ok","channel_id":"x"}"#
            )
            .unwrap_err(),
            AppFailure::ReservedControlField
        );
        assert_eq!(
            parse_app_callback_body(
                br#"{"idempotency_key":"k1","repository_name":"harness-service","event_type":"workflow.run.completed","content":"ok","metadata":{"app_id":"x"}}"#
            )
            .unwrap_err(),
            AppFailure::ReservedControlField
        );
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && cargo test -p buzz-core --lib app::tests -- --nocapture`

Expected: FAIL because `KIND_APP_METADATA` and the `app` module do not exist.

- [ ] **Step 4: Write the minimal implementation**

Add to `kind.rs` after `KIND_WINDOW_BOUNDS`:

```rust
/// Relay-signed parameterized-replaceable App metadata (`d` = App UUID).
pub const KIND_APP_METADATA: u32 = 39007;
/// Community owner/admin command that creates or mutates a community App.
pub const KIND_APP_ADMIN_COMMAND: u32 = 9038;
```

Add both constants to `ALL_KINDS`.
Add `KIND_APP_ADMIN_COMMAND` to `is_command_kind`.
Add `KIND_APP_METADATA` to `is_relay_only_kind`.
Add `const _: () = assert!(is_parameterized_replaceable(KIND_APP_METADATA));`.
Export `pub mod app;` from `lib.rs`.

Implement `app.rs` with:

```rust
pub fn generate_app_callback_secret() -> Result<String, AppFailure> {
    let bytes: [u8; 32] = rand::random();
    Ok(hex::encode(bytes))
}

pub fn hash_app_callback_secret(secret: &str) -> [u8; 32] {
    Sha256::digest(secret.as_bytes()).into()
}

pub fn app_secrets_equal(provided_hash: &[u8; 32], stored_hash: &[u8; 32]) -> bool {
    bool::from(provided_hash.ct_eq(stored_hash))
}
```

`AppFailure` codes must be exactly:

| Variant | `code()` | `http_status()` | `redacted_message()` |
| --- | --- | --- | --- |
| `InvalidCommand` | `invalid_command` | 400 | `invalid app command` |
| `InvalidCallback` | `invalid_callback` | 400 | `invalid callback` |
| `ReservedControlField` | `reserved_control_field` | 400 | `reserved control field` |
| `UnknownField` | `unknown_field` | 400 | `unknown field` |
| `Unauthorized` | `unauthorized` | 401 | `authentication failed` |
| `NotFound` | `not_found` | 404 | `app not found` |
| `IdempotencyConflict` | `idempotency_conflict` | 409 | `idempotency key already used with a different payload` |
| `RateLimited` | `rate_limited` | 429 | `rate limit exceeded` |
| `Unavailable` | `unavailable` | 503 | `service unavailable` |

Reuse `buzz_workflow::RouteFailure` codes for repository, project, and channel routing by mapping those failures in Task 6 rather than duplicating them here.
`parse_app_callback_body` must reject non-object JSON, empty required strings, unknown top-level keys, reserved keys in the object or nested metadata, metadata deeper than 4, metadata objects with more than 32 keys, and metadata whose `serde_json::to_vec` length exceeds 8192.
`event_type` must match `^[A-Za-z0-9._-]{1,64}$`.
`content` must be non-empty after rejecting a whitespace-only string and must be `<= 65536` bytes.

- [ ] **Step 5: Run tests to verify they pass**

Run: `. ./bin/activate-hermit && cargo test -p buzz-core --lib app::tests -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-core/src/app.rs crates/buzz-core/src/kind.rs crates/buzz-core/src/lib.rs && git commit -s -m "feat(core): add App kinds, command schema, and hashed secrets"
```

---

### Task 2: App and delivery tables

**Files:**

- Create: `migrations/0033_apps_and_callback_deliveries.sql`
- Modify: `schema/schema.sql`
- Create: `crates/buzz-db/src/app.rs`
- Create: `crates/buzz-db/src/app_admission.rs`
- Modify: `crates/buzz-db/src/lib.rs`

**Interfaces:**

- Consumes: `CommunityId`, `hash_app_callback_secret`, existing `insert_event_with_thread_metadata_tx`.
- Produces:
  - `pub struct AppRecord { pub community_id: CommunityId, pub id: Uuid, pub name: String, pub description: Option<String>, pub icon_url: Option<String>, pub status: String, pub secret_hash: [u8; 32], pub created_by: Vec<u8>, pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc> }`
  - `pub struct AppRouteSnapshot { pub repository_coordinate: String, pub project_coordinate: String, pub channel_id: Uuid, pub matched_identity_tier: String }`
  - `pub struct AppDeliveryRecord { pub community_id: CommunityId, pub id: Uuid, pub app_id: Uuid, pub idempotency_key_hash: [u8; 32], pub payload_hash: [u8; 32], pub event_type: String, pub route_snapshot: Option<AppRouteSnapshot>, pub status: String, pub event_id: Option<Vec<u8>>, pub failure_code: Option<String>, pub created_at: DateTime<Utc>, pub completed_at: Option<DateTime<Utc>> }`
  - `pub enum BeginAppAdmission { Existing(AppDeliveryRecord), PayloadConflict { existing: AppDeliveryRecord }, Vacant(AppAdmissionGuard) }`
  - `impl AppAdmissionGuard { pub async fn list_latest_parameterized_heads(&mut self, kind: i32) -> Result<Vec<StoredEvent>>; pub async fn load_destination_channel(&mut self, channel_id: Uuid) -> Result<Option<WorkflowAdmissionChannel>>; pub async fn persist_delivered(self, event: &nostr::Event, channel_id: Uuid, thread_meta: ThreadMetadataParams<'_>, snapshot: &AppRouteSnapshot, event_type: &str) -> Result<(StoredEvent, AppDeliveryRecord)>; pub async fn persist_rejected(self, event_type: &str, failure_code: &str) -> Result<AppDeliveryRecord>; }`

- [ ] **Step 1: Write the failing isolation tests**

Add `#[cfg(test)]` tests in `crates/buzz-db/src/app.rs` using the same Postgres helper pattern as `crates/buzz-db/src/workflow.rs` tests.
Name them `app_rows_are_isolated_by_community`, `list_apps_does_not_return_secret_hash_to_callers_that_use_public_projection`, and `disabled_app_is_still_readable`.
Add `crates/buzz-db/src/app_admission.rs` tests named `duplicate_idempotency_key_with_same_hash_returns_existing`, `duplicate_idempotency_key_with_different_hash_conflicts`, and `persist_delivered_commits_event_and_row_together`.

The public list projection type is:

```rust
pub struct AppPublicRecord {
    pub community_id: CommunityId,
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub status: String,
    pub created_by: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

`list_apps` and `get_app_public` must not have a `secret_hash` field.
`get_app_auth` is the only reader that returns `secret_hash` and is documented as callback-auth only.

- [ ] **Step 2: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && cargo test -p buzz-db --lib app::tests app_admission::tests -- --ignored --nocapture`

Expected: FAIL because the modules and tables do not exist.

If the local suite requires Postgres, use `DATABASE_URL` / `BUZZ_TEST_DATABASE_URL` already used by `buzz-db` ignored tests.
Do not skip.

- [ ] **Step 3: Write the migration and repositories**

`migrations/0033_apps_and_callback_deliveries.sql` must contain:

```sql
SET LOCAL lock_timeout = '5s';

CREATE TABLE apps (
    community_id UUID NOT NULL REFERENCES communities(id),
    id           UUID NOT NULL,
    name         TEXT NOT NULL,
    description  TEXT,
    icon_url     TEXT,
    status       TEXT NOT NULL CHECK (status IN ('active', 'disabled')),
    secret_hash  BYTEA NOT NULL,
    created_by   BYTEA NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (community_id, id),
    CHECK (octet_length(secret_hash) = 32),
    CHECK (char_length(name) BETWEEN 1 AND 64)
);

CREATE INDEX idx_apps_community_status ON apps (community_id, status);

CREATE TABLE app_callback_deliveries (
    community_id          UUID NOT NULL REFERENCES communities(id),
    id                    UUID NOT NULL,
    app_id                UUID NOT NULL,
    idempotency_key_hash  BYTEA NOT NULL,
    payload_hash          BYTEA NOT NULL,
    event_type            TEXT NOT NULL,
    route_snapshot        JSONB,
    status                TEXT NOT NULL CHECK (status IN ('delivered', 'rejected')),
    event_id              BYTEA,
    failure_code          TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at          TIMESTAMPTZ,
    PRIMARY KEY (community_id, id),
    FOREIGN KEY (community_id, app_id) REFERENCES apps (community_id, id),
    CHECK (octet_length(idempotency_key_hash) = 32),
    CHECK (octet_length(payload_hash) = 32),
    CHECK ((status = 'delivered') = (event_id IS NOT NULL)),
    CHECK ((status = 'rejected') = (failure_code IS NOT NULL))
);

CREATE UNIQUE INDEX idx_app_callback_deliveries_idempotency
    ON app_callback_deliveries (community_id, app_id, idempotency_key_hash);

CREATE FUNCTION prevent_app_delivery_identity_update()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.idempotency_key_hash IS DISTINCT FROM OLD.idempotency_key_hash
       OR NEW.payload_hash IS DISTINCT FROM OLD.payload_hash
       OR NEW.route_snapshot IS DISTINCT FROM OLD.route_snapshot
       OR NEW.status IS DISTINCT FROM OLD.status
       OR NEW.event_id IS DISTINCT FROM OLD.event_id
       OR NEW.failure_code IS DISTINCT FROM OLD.failure_code
    THEN
        RAISE EXCEPTION 'app callback delivery identity is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER app_callback_delivery_identity_guard
BEFORE UPDATE ON app_callback_deliveries
FOR EACH ROW
EXECUTE FUNCTION prevent_app_delivery_identity_update();
```

Mirror the same tables into `schema/schema.sql`.
`persist_delivered` must call `insert_event_with_thread_metadata_tx` and insert the delivery row on the same transaction before commit.
Dropping `AppAdmissionGuard` without persist must roll back.

- [ ] **Step 4: Run tests to verify they pass**

Run: `. ./bin/activate-hermit && cargo test -p buzz-db --lib app::tests app_admission::tests -- --ignored --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add migrations/0033_apps_and_callback_deliveries.sql schema/schema.sql crates/buzz-db/src/app.rs crates/buzz-db/src/app_admission.rs crates/buzz-db/src/lib.rs && git commit -s -m "feat(db): add community Apps and callback delivery ledger"
```

---

### Task 3: Neutral project route module

**Files:**

- Create: `crates/buzz-relay/src/project_route.rs`
- Modify: `crates/buzz-relay/src/workflow_route.rs`
- Modify: `crates/buzz-relay/src/workflow_admission.rs`
- Modify: `crates/buzz-relay/src/lib.rs`

**Interfaces:**

- Consumes: current `workflow_route.rs` functions and tests.
- Produces: the same `RepositoryHead`, `ProjectHead`, `IdentityTier`, `resolve_repository_identity`, `authorize_unique_project_route`, `require_exact_stored_project_channel`, `clone_basename`, `repository_head_from_event`, and `project_head_from_event` in `project_route`.

- [ ] **Step 1: Write the failing destination test**

Move the existing `workflow_route` tests with the module.
Add this extra test in `project_route.rs`:

```rust
#[test]
fn empty_alias_map_uses_strict_identity_tiers() {
    let owner = owner();
    let heads = vec![repo("harness-service", &owner)];
    let (resolved, tier) =
        resolve_repository_identity("harness-service", &BTreeMap::new(), &heads)
            .expect("d tag");
    assert_eq!(resolved, coord(&owner, "harness-service"));
    assert_eq!(tier, IdentityTier::DTag);
}
```

- [ ] **Step 2: Run tests to verify the move fails before the file exists**

Run: `. ./bin/activate-hermit && cargo test -p buzz-relay --lib project_route -- --nocapture`

Expected: FAIL because `project_route` does not exist.

- [ ] **Step 3: Move the module**

Copy `workflow_route.rs` to `project_route.rs` unchanged except the module docs, which must say the resolver is shared by workflow webhooks and App callbacks and contains no App or workflow ownership checks.
Update `workflow_admission.rs` imports to `crate::project_route::{...}`.
Replace `workflow_route.rs` with:

```rust
//! Compatibility re-export. Prefer [`crate::project_route`].
pub use crate::project_route::*;
```

Keep that re-export only if any test module still imports `workflow_route`.
If no remaining imports exist after the admission update, delete `workflow_route.rs` in this task and drop it from `lib.rs`.
Do not change membership checks here; those stay in `workflow_admission.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `. ./bin/activate-hermit && cargo test -p buzz-relay --lib project_route -- --nocapture`

Expected: PASS, including the previous identity-tier tests.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-relay/src/project_route.rs crates/buzz-relay/src/workflow_route.rs crates/buzz-relay/src/workflow_admission.rs crates/buzz-relay/src/lib.rs && git commit -s -m "refactor(relay): share repository project routing across webhooks and Apps"
```

---

### Task 4: App admin command and metadata events

**Files:**

- Create: `crates/buzz-relay/src/app_admin.rs`
- Modify: `crates/buzz-relay/src/handlers/command_executor.rs`
- Modify: `crates/buzz-relay/src/handlers/ingest.rs`
- Modify: `crates/buzz-audit/src/action.rs`
- Modify: `crates/buzz-relay/src/lib.rs`

**Interfaces:**

- Consumes: `parse_app_admin_command`, `generate_app_callback_secret`, `hash_app_callback_secret`, `Db` App CRUD, `replace_addressable_event`, `AuditService::log`.
- Produces:
  - `pub async fn handle_app_admin_command(tenant: &TenantContext, state: &Arc<AppState>, event: &Event, auth: &IngestAuth) -> Result<IngestResult, IngestError>`
  - Command result JSON `{ "app_id": "<uuid>", "callback_url": "<url>", "secret": "<hex>" }` on create and rotate only
  - Kind `39007` replacement after create, update, enable, and disable

- [ ] **Step 1: Write the failing unit tests in `app_admin.rs`**

```rust
#[test]
fn create_result_includes_secret_once_shape() {
    let body = serde_json::json!({
        "app_id": "11111111-1111-4111-8111-111111111111",
        "callback_url": "http://localhost:3000/hooks/apps/11111111-1111-4111-8111-111111111111",
        "secret": "ab".repeat(32),
    });
    let message = format!("response:{body}");
    assert!(message.contains("secret"));
    assert!(message.contains("/hooks/apps/"));
}

#[test]
fn metadata_tags_exclude_secret_and_creator() {
    let tags = app_metadata_tags(
        uuid::Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap(),
        "Archon",
        "active",
        Some("https://example.com/a.png"),
    );
    let names: Vec<_> = tags.iter().map(|t| t[0].as_str()).collect();
    assert_eq!(names, vec!["d", "name", "status", "picture"]);
    assert!(!names.contains(&"secret"));
    assert!(!names.contains(&"created_by"));
}
```

Also add ingest unit coverage that `required_scope_for_kind` maps `9038` to `AdminUsers` and that `is_global_only_kind` includes `9038` and `39007`.
Follow the existing `ingest.rs` kind-table tests.

- [ ] **Step 2: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && cargo test -p buzz-relay --lib app_admin -- --nocapture`

Expected: FAIL because `app_admin` and `app_metadata_tags` do not exist.

- [ ] **Step 3: Implement the handler**

`handle_command` must add:

```rust
KIND_APP_ADMIN_COMMAND => handle_app_admin_command(tenant, state, &event, &auth).await,
```

Handler rules:

1. Reject banned actors with the same `blocked: you are banned` text used by ingest.
2. Reject timestamps outside ±120 seconds.
3. Require `get_relay_member` role `owner` or `admin`.
4. Parse JSON content with `parse_app_admin_command`.
5. `create` generates App UUID, secret, hash, inserts `apps` with `status=active`, publishes kind `39007`, audits `AppCreated`, and returns the secret.
6. `update` changes only provided public fields, sets `updated_at`, republishes `39007`, audits `AppUpdated`, and returns no secret.
7. `rotate_secret` stores a new hash immediately, audits `AppSecretRotated`, does not republish `39007`, and returns the new secret.
8. `enable` / `disable` flip `status`, republish `39007`, and audit the matching action.
9. Persist the kind `9038` event globally with `persist_command_event(..., None)` after mutations, matching workflow-def durability.
10. Build `callback_url` from `nip98_expected_url` origin logic: `{tenant host relay http origin}/hooks/apps/{app_id}`.
11. Kind `39007` content is `description.unwrap_or("")`.
12. Never put `secret` or `secret_hash` on the stored command event or metadata event.

Add `is_global_only_kind` arms for `KIND_APP_ADMIN_COMMAND` and `KIND_APP_METADATA`.
Add `KIND_APP_ADMIN_COMMAND` to the timeout-exemption predicate beside `is_relay_admin_kind` by introducing `fn is_timeout_exempt_admin_kind(kind: u32) -> bool` in `kind.rs` that is true for relay-admin kinds and `KIND_APP_ADMIN_COMMAND`.
Do not expand `is_relay_admin_kind` itself.

Add audit actions:

```rust
AppCreated,
AppUpdated,
AppSecretRotated,
AppEnabled,
AppDisabled,
```

with `as_str` values `app_created`, `app_updated`, `app_secret_rotated`, `app_enabled`, `app_disabled`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `. ./bin/activate-hermit && cargo test -p buzz-relay --lib app_admin -- --nocapture && cargo test -p buzz-audit --lib action -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-relay/src/app_admin.rs crates/buzz-relay/src/handlers/command_executor.rs crates/buzz-relay/src/handlers/ingest.rs crates/buzz-relay/src/lib.rs crates/buzz-core/src/kind.rs crates/buzz-audit/src/action.rs && git commit -s -m "feat(relay): add App admin commands and relay-signed App metadata"
```

---

### Task 5: App callback admission and message emission

**Files:**

- Create: `crates/buzz-relay/src/app_admission.rs`
- Create: `crates/buzz-relay/src/app_sink.rs`
- Modify: `crates/buzz-relay/src/router.rs`
- Modify: `crates/buzz-relay/src/api/bridge.rs` if a thin wrapper is required to match `workflow_webhook`
- Modify: `crates/buzz-relay/src/state.rs`
- Modify: `crates/buzz-relay/src/handlers/event.rs`
- Modify: `crates/buzz-relay/src/lib.rs`

**Interfaces:**

- Consumes: `parse_app_callback_body`, `hash_app_callback_secret`, `app_secrets_equal`, `BeginAppAdmission`, `project_route`, `dispatch_persistent_event`.
- Produces:
  - `pub async fn handle_app_callback(state: Arc<AppState>, app_id: String, headers: HeaderMap, body: Bytes) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)>`
  - `pub async fn emit_app_message(...) -> Result<nostr::Event, AppFailure>`

- [ ] **Step 1: Write the failing sink and skip-trigger tests**

In `app_sink.rs`:

```rust
#[test]
fn app_message_tags_do_not_include_author_p() {
    let tags = app_message_tags(
        "9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50",
        "11111111-1111-4111-8111-111111111111",
        "30617:aa...:harness-service",
        "30621:aa...:gigo-harness",
        "22222222-2222-4222-8222-222222222222",
        "workflow.run.completed",
        &[],
    );
    assert!(tags.iter().any(|t| t.as_slice()[0] == "h"));
    assert!(tags.iter().any(|t| t.as_slice() == ["buzz:app", "11111111-1111-4111-8111-111111111111"]));
    assert!(tags.iter().any(|t| t.as_slice()[0] == "a" && t.as_slice()[1].starts_with("30617:")));
    assert!(tags.iter().any(|t| t.as_slice()[0] == "a" && t.as_slice()[1].starts_with("30621:")));
    assert!(tags.iter().any(|t| t.as_slice()[0] == "buzz:app-delivery"));
    assert!(tags.iter().any(|t| t.as_slice() == ["buzz:app-event", "workflow.run.completed"]));
    assert!(!tags.iter().any(|t| t.as_slice()[0] == "p"));
    assert!(!tags.iter().any(|t| t.as_slice()[0] == "buzz:workflow"));
}
```

Use real 64-hex pubkeys in the coordinate strings in the actual test.
In `event.rs` tests, pin that a relay-signed kind `9` with `buzz:app` is treated as a workflow-trigger skip the same way `buzz:workflow` is.

- [ ] **Step 2: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && cargo test -p buzz-relay --lib app_sink -- --nocapture`

Expected: FAIL because `app_message_tags` does not exist.

- [ ] **Step 3: Implement admission and emission**

`handle_app_callback` order is mandatory:

1. Bind community from `Host`. Unknown host returns `404` `app not found`.
2. Parse `app_id` as UUID. Invalid UUID returns `400`.
3. Apply rate limit. Exceeded returns `429`.
4. Load `get_app_auth`. Missing or `disabled` returns `404` `app not found` with the same body.
5. Read `X-Webhook-Secret` only. Missing or mismatched hash returns `401` and creates no delivery row.
6. Require `Content-Type` starting with `application/json`.
7. Parse body with `parse_app_callback_body`.
8. Hash idempotency key and canonical payload.
9. `begin_app_admission`. Existing delivered same hash returns `202` with stored ids. Existing different hash returns `409`. Existing rejected returns the stored failure.
10. Resolve repository with empty aliases. Map `RouteFailure` through `api_error_with_code`.
11. Authorize unique claim-valid project. Read `buzz-channel` UUID. Load channel in the admission transaction. Missing, deleted, archived, or other-community channels fail as `project_channel_invalid`.
12. Build kind `9` with `emit_app_message`. Resolve `@Name` mentions into extra `p` tags only after the App tags, never as the first tag.
13. `persist_delivered` atomically. On success, `dispatch_persistent_event`, then return `202`.
14. Deterministic route failures call `persist_rejected` and return `422` with the stable code.
15. Transient DB errors drop the guard, insert nothing, and return `503`.

Router:

```rust
.route("/hooks/apps/{app_id}", post(api::bridge::app_webhook))
.route("/hooks/{id}", post(api::bridge::workflow_webhook))
```

`app_webhook` is a thin wrapper that does not read query secrets.

In `event.rs`, change the skip predicate to:

```rust
let skip_workflow = stored_event.event.pubkey == state.relay_keypair.public_key()
    && stored_event.event.tags.iter().any(|t| {
        matches!(
            t.as_slice().first().map(|s| s.as_str()),
            Some("buzz:workflow") | Some("buzz:app")
        )
    });
```

Rate limiter field on `AppState`:

```rust
pub app_callback_rate_limiter: Arc<DashMap<(Uuid, Uuid, String), (u32, Instant)>>,
```

Limit is 60 per 60 seconds.
Source address is the last hop in `X-Forwarded-For` when present, otherwise the socket IP if Axum connect info is available, otherwise `unknown`.
Logs use App UUID, delivery UUID, outcome, latency, and failure code only.

- [ ] **Step 4: Run tests to verify they pass**

Run: `. ./bin/activate-hermit && cargo test -p buzz-relay --lib app_sink app_admission -- --nocapture`

Expected: PASS for unit tests.
Admission tests that need Postgres stay `#[ignore]` and run in Task 10.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-relay/src/app_admission.rs crates/buzz-relay/src/app_sink.rs crates/buzz-relay/src/router.rs crates/buzz-relay/src/api/bridge.rs crates/buzz-relay/src/state.rs crates/buzz-relay/src/handlers/event.rs crates/buzz-relay/src/lib.rs && git commit -s -m "feat(relay): admit App callbacks and emit App-attributed messages"
```

---

### Task 6: Buzz CLI `apps` commands

**Files:**

- Create: `crates/buzz-cli/src/commands/apps.rs`
- Modify: `crates/buzz-cli/src/commands/mod.rs`
- Modify: `crates/buzz-cli/src/lib.rs`

**Interfaces:**

- Consumes: `BuzzClient::query`, `BuzzClient::submit_event`, `extract_relay_response_field`, `OutputFormat`.
- Produces: `buzz apps list|create|update|rotate-secret|enable|disable`.

- [ ] **Step 1: Write the failing CLI tests**

In `apps.rs`:

```rust
#[test]
fn compact_list_omits_secret_fields() {
    let event = serde_json::json!({
        "id": "ab".repeat(32),
        "pubkey": "cd".repeat(32),
        "kind": 39007,
        "created_at": 1,
        "content": "CI notifications",
        "tags": [
            ["d", "11111111-1111-4111-8111-111111111111"],
            ["name", "Archon"],
            ["status", "active"],
            ["picture", "https://example.com/a.png"]
        ]
    });
    let compact = compact_app_from_event(&event);
    assert_eq!(compact["app_id"], "11111111-1111-4111-8111-111111111111");
    assert_eq!(compact["name"], "Archon");
    assert_eq!(compact["status"], "active");
    assert!(compact.get("secret").is_none());
    assert!(compact.get("secret_hash").is_none());
}

#[test]
fn create_response_keeps_secret_only_for_mutating_commands() {
    let raw = r#"{"event_id":"e","accepted":true,"message":"response:{\"app_id\":\"11111111-1111-4111-8111-111111111111\",\"callback_url\":\"http://localhost:3000/hooks/apps/11111111-1111-4111-8111-111111111111\",\"secret\":\"deadbeef\"}"}"#;
    let printed = format_app_mutation_response(raw, true);
    assert!(printed.contains("deadbeef"));
    let listed = format_app_mutation_response(raw, false);
    assert!(!listed.contains("deadbeef"));
}
```

Add a clap parse test in `crates/buzz-cli/src/lib.rs` tests that `buzz apps create --name Archon` is a valid command.

- [ ] **Step 2: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && cargo test -p buzz-cli apps -- --nocapture`

Expected: FAIL because `commands/apps.rs` does not exist.

- [ ] **Step 3: Implement the commands**

`AppsCmd` in `lib.rs`:

```rust
/// Create and manage community Apps
#[command(subcommand)]
Apps(AppsCmd),
```

```rust
pub enum AppsCmd {
    List,
    Create { #[arg(long)] name: String, #[arg(long)] description: Option<String>, #[arg(long = "icon-url")] icon_url: Option<String> },
    Update { #[arg(long)] app: String, #[arg(long)] name: Option<String>, #[arg(long)] description: Option<String>, #[arg(long = "icon-url")] icon_url: Option<String> },
    RotateSecret { #[arg(long)] app: String },
    Enable { #[arg(long)] app: String },
    Disable { #[arg(long)] app: String },
}
```

`list` queries `{ "kinds": [39007] }`, keeps events whose `pubkey` will be displayed as-is, and prints JSON array of `{app_id,name,status,description,icon_url,created_at,pubkey}`.
Compact list prints `{app_id,name,status}` only.
Create/update/rotate/enable/disable sign kind `9038` with JSON content and no extra tags.
Create and rotate print the mutation JSON including `secret`.
Enable, disable, and update print `{event_id,accepted,app_id}` and must not print `secret` even if a confused relay echoed one.
Dispatch `Cmd::Apps` next to `Cmd::Workflows`.
Pass `&cli.format` into `apps::dispatch`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `. ./bin/activate-hermit && cargo test -p buzz-cli apps -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-cli/src/commands/apps.rs crates/buzz-cli/src/commands/mod.rs crates/buzz-cli/src/lib.rs && git commit -s -m "feat(cli): add buzz apps management commands"
```

---

### Task 7: Client App actor resolution

**Files:**

- Create: `desktop/src/shared/lib/appActor.ts`
- Create: `desktop/src/shared/lib/appActor.test.mjs`
- Modify: `desktop/src/shared/lib/authors.ts`
- Modify: `desktop/src/shared/lib/authors.test.mjs`
- Modify: `desktop/src/shared/constants/kinds.ts`
- Modify: `desktop/src/features/messages/types.ts`
- Modify: `desktop/src/features/messages/lib/formatTimelineMessages.ts`
- Create: `mobile/lib/shared/relay/app_actor.dart`
- Create: `mobile/test/shared/relay/app_actor_test.dart`
- Modify: `mobile/lib/shared/relay/nostr_models.dart`

**Interfaces:**

- Consumes: `verifyEvent`, relay pubkey, kind `39007` metadata map.
- Produces:

```ts
export type AppMetadata = {
  id: string;
  name: string;
  description: string;
  picture: string | null;
  status: "active" | "disabled";
  pubkey: string;
};

export type ResolvedEventActor =
  | { type: "user"; pubkey: string }
  | { type: "app"; appId: string };

export function resolveEventActor(input: {
  event: { id: string; pubkey: string; created_at: number; kind: number; tags: string[][]; content: string; sig: string };
  relaySelfPubkey?: string | null;
  appsById?: ReadonlyMap<string, AppMetadata>;
  verifySignature?: boolean;
}): ResolvedEventActor;
```

- [ ] **Step 1: Write the failing Desktop and Mobile tests**

`desktop/src/shared/lib/appActor.test.mjs` complete cases:

1. Relay-signed `buzz:app` plus matching kind `39007` from the same relay resolves `{ type: "app", appId }`.
2. User-signed `buzz:app` resolves `{ type: "user", pubkey: signer }`.
3. Relay-signed `buzz:app` with missing metadata resolves the relay signer as a user actor.
4. Relay-signed `buzz:app` with metadata signed by a different pubkey resolves the relay signer as a user actor.
5. Relay-signed `buzz:app` plus a mention `p` tag still resolves the App, never the mentioned user.
6. Invalid App UUID resolves the relay signer.
7. Invalid signature with `verifySignature: true` resolves the relay signer.
8. Search-mode `verifySignature: false` still requires relay pubkey plus live metadata.

Extend `authors.test.mjs` with:

```js
test("relay-signed first p tag is ignored when buzz:app is present", () => {
  assert.equal(
    resolveEventAuthorPubkey({
      event: appTaggedRelayEvent,
      relaySelfPubkey: RELAY,
      requireChannelTagForPTags: true,
    }),
    RELAY,
  );
});
```

`resolveEventAuthorPubkey` must return the user pubkey for user actors and the relay signer for App actors so leftover pubkey consumers cannot treat a mention as the author.

Mobile tests in `app_actor_test.dart` cover the same eight cases with `EventKind.appMetadata = 39007`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && cd desktop && node --test src/shared/lib/appActor.test.mjs src/shared/lib/authors.test.mjs`

Expected: FAIL because `appActor.ts` does not exist.

Run: `. ./bin/activate-hermit && cd mobile && flutter test test/shared/relay/app_actor_test.dart`

Expected: FAIL because `app_actor.dart` does not exist.

- [ ] **Step 3: Implement resolution and timeline mapping**

`resolveEventActor` algorithm:

1. Normalize signer.
2. Read first `buzz:app` tag value.
3. If absent, return `{ type: "user", pubkey: resolveEventAuthorPubkey(...) }`.
4. If signer is not the active relay, return `{ type: "user", pubkey: signer }`.
5. If UUID is invalid, return `{ type: "user", pubkey: signer }`.
6. If `verifySignature` is true (default) and `verifyEvent` fails, return `{ type: "user", pubkey: signer }`.
7. If `appsById` has no entry for that UUID whose `pubkey` equals the relay, return `{ type: "user", pubkey: signer }`.
8. Return `{ type: "app", appId }`.

In `formatTimelineMessages.ts`, add an optional last argument `appsById?: ReadonlyMap<string, AppMetadata>`.
Existing call sites keep compiling.
When the actor is an App, set `isApp: true`, `appId`, `author` to metadata.name, `avatarUrl` to metadata.picture, omit `pubkey`, and set `signerPubkey` to the relay.
Task 8 creates `parseAppMetadata` / `useAppsQuery` and updates ChannelScreen, independent thread panel, Home inbox, and Projects agent prompt to pass the live App map.

Mobile `MessageAuthorMeta` gains optional `isApp`.
Do not wire it into channel pages until Task 8 if that would require larger widget changes; the resolver must still be complete and tested here.

- [ ] **Step 4: Run tests to verify they pass**

Run: `. ./bin/activate-hermit && cd desktop && node --test src/shared/lib/appActor.test.mjs src/shared/lib/authors.test.mjs`

Run: `. ./bin/activate-hermit && cd mobile && flutter test test/shared/relay/app_actor_test.dart`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/shared/lib/appActor.ts desktop/src/shared/lib/appActor.test.mjs desktop/src/shared/lib/authors.ts desktop/src/shared/lib/authors.test.mjs desktop/src/shared/constants/kinds.ts desktop/src/features/messages/types.ts desktop/src/features/messages/lib/formatTimelineMessages.ts mobile/lib/shared/relay/app_actor.dart mobile/test/shared/relay/app_actor_test.dart mobile/lib/shared/relay/nostr_models.dart && git commit -s -m "feat(clients): resolve relay-attested App message actors"
```

---

### Task 8: Desktop and Mobile App rendering surfaces

**Files:**

- Create: `desktop/src/features/messages/ui/MessageAppBadge.tsx`
- Modify: `desktop/src/features/messages/ui/MessageRow.tsx`
- Modify: `desktop/src/features/search/ui/SearchResultItem.tsx`
- Modify: `desktop/src-tauri/src/models.rs`
- Modify: `desktop/src-tauri/src/nostr_convert.rs`
- Modify: `desktop/src/shared/api/searchTypes.ts`
- Modify: `desktop/src/shared/api/tauri.ts`
- Modify: `desktop/src/features/home/lib/inbox.ts` if inbox rows use `authorPubkey` from `resolveEventAuthorPubkey`
- Create: `desktop/src/features/apps/appModels.ts`
- Create: `desktop/src/features/apps/useAppsQuery.ts`
- Modify: `desktop/src/features/channels/ui/ChannelScreen.tsx`
- Modify: `desktop/src/features/messages/lib/independentThreadPanel.ts`
- Modify: `desktop/src/features/home/useHomeInboxContextMessages.ts`
- Modify: `desktop/src/features/projects/ui/ProjectsAgentPromptPage.tsx`
- Modify: `mobile/lib/shared/widgets/message_author_meta.dart`
- Modify: the Mobile channel message header that currently passes `displayName` into `MessageAuthorMeta`

**Interfaces:**

- Consumes: `ResolvedEventActor`, `AppMetadata`, `TimelineMessage.isApp`.
- Produces: visible `App` badge with `data-testid="message-app-badge"` on Desktop and `Key('message-app-badge')` on Mobile.

- [ ] **Step 1: Write the failing rendering tests**

Add `desktop/src/features/messages/ui/MessageAppBadge.test.mjs` if the repo tests UI copies with node:test; otherwise add a pure helper `appBadgeLabel()` in `MessageAppBadge.tsx` that returns `"App"` and test that.
Extend `formatTimelineMessages` tests if a `formatTimelineMessages.test.mjs` exists; if it does not, add `desktop/src/features/messages/lib/formatTimelineMessages.appActor.test.mjs` that formats one relay-signed App event and asserts `isApp === true`, `author === "Archon"`, and `pubkey` is omitted.

Add a Flutter widget test `mobile/test/shared/widgets/message_author_meta_test.dart` that shows `App` when `isApp: true` and hides it otherwise.

Search helper test: `desktop/src/features/search/lib/searchHitActor.test.mjs` resolving a hit with tags `[["buzz:app", APP_ID]]` and relay pubkey.

- [ ] **Step 2: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && cd desktop && node --test src/features/messages/lib/formatTimelineMessages.appActor.test.mjs src/features/search/lib/searchHitActor.test.mjs`

Expected: FAIL.

Run: `. ./bin/activate-hermit && cd mobile && flutter test test/shared/widgets/message_author_meta_test.dart`

Expected: FAIL.

- [ ] **Step 3: Implement rendering**

`MessageAppBadge` is a `span` with classes `text-2xs font-medium text-muted-foreground` and text `App`.
In `MessageRow`, when `message.isApp`, skip `UserProfilePopover`, render the avatar as a static image, place `MessageAppBadge` in `MessageMetaSegments` after the author name, and do not render `MessageAgentOwner`.
Add `tags` to `SearchHitInfo` as `pub tags: Vec<Vec<String>>` defaulting to empty for compatibility, copy `ev.tags` in `search_response_from_events`, and thread them to `SearchHit.tags`.
`SearchResultItem` uses `resolveEventActor` with `verifySignature: false`.
Inbox rows that currently call `resolveEventAuthorPubkey` must switch to `resolveEventActor` so Home does not show Kevin from a leftover `p` tag.
Quoted and thread-root rows share `MessageRow`, so they inherit the badge.
Desktop notifications already use feed item titles; if the feed item author label comes from `resolveEventAuthorPubkey`, switch that call site to `resolveEventActor` in this task.
Mobile channel and thread headers pass `isApp` from `app_actor.dart`.

If `MessageRow.tsx` would exceed 1000 lines, extract the author/avatar header into `MessageAuthorHeader.tsx` rather than raising the ratchet.

- [ ] **Step 4: Run tests to verify they pass**

Run the same Desktop and Mobile commands from Step 2.

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/messages/ui/MessageAppBadge.tsx desktop/src/features/messages/ui/MessageRow.tsx desktop/src/features/search/ui/SearchResultItem.tsx desktop/src-tauri/src/models.rs desktop/src-tauri/src/nostr_convert.rs desktop/src/shared/api/searchTypes.ts desktop/src/shared/api/tauri.ts desktop/src/features/home/lib/inbox.ts mobile/lib/shared/widgets/message_author_meta.dart mobile/test/shared/widgets/message_author_meta_test.dart && git commit -s -m "feat(ui): render App badge and App actors on message surfaces"
```

Include every additional file this task actually changed.

---

### Task 9: Desktop Apps settings UI

**Files:**

- Modify: `desktop/src/features/apps/appModels.ts`
- Create: `desktop/src/features/apps/appModels.test.mjs`
- Modify: `desktop/src/features/apps/useAppsQuery.ts`
- Create: `desktop/src/features/apps/appCommands.ts`
- Create: `desktop/src/features/apps/ui/AppsSettingsCard.tsx`
- Create: `desktop/src/features/apps/ui/CreateAppDialog.tsx`
- Create: `desktop/src/features/apps/ui/AppCredentialsDialog.tsx`
- Create: `desktop/src/features/apps/ui/EditAppDialog.tsx`
- Modify: `desktop/src/features/settings/ui/SettingsPanels.tsx`
- Modify: `desktop/src/features/settings/ui/SettingsView.tsx`
- Modify: `desktop/tests/helpers/settings.ts`

**Interfaces:**

- Consumes: `signRelayEvent`, `relayClient`, `canManageCommunityMembers`, existing media upload for icons.
- Produces: Settings section `"apps"` with test ids `settings-nav-apps`, `apps-empty-state`, `apps-create`, `app-row-{id}`, `app-credentials-dialog`.

- [ ] **Step 1: Write the failing model and command tests**

```js
test("parses kind 39007 without secret fields", () => {
  const app = parseAppMetadata({
    pubkey: RELAY,
    kind: 39007,
    content: "CI",
    tags: [
      ["d", APP_ID],
      ["name", "Archon"],
      ["status", "disabled"],
      ["picture", "https://example.com/a.png"],
    ],
  });
  assert.equal(app.id, APP_ID);
  assert.equal(app.name, "Archon");
  assert.equal(app.status, "disabled");
  assert.equal(app.secret, undefined);
});

test("create command content is create action JSON", () => {
  assert.deepEqual(JSON.parse(buildAppAdminContent({ action: "create", name: "Archon" })), {
    action: "create",
    name: "Archon",
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && cd desktop && node --test src/features/apps/appModels.test.mjs`

Expected: FAIL.

- [ ] **Step 3: Implement the settings section**

Add `"apps"` to `SettingsSection` and `SETTINGS_SECTION_VALUES`.
Add `{ value: "apps", label: "Apps", icon: Puzzle }` to `settingsSections`.
Put `"apps"` in the Communities nav group.
Hide it unless `canManageCommunityMembers(myMembershipQuery.data)` is true, same as `community-members`.
Empty copy is exactly `Apps receive external callbacks and post notifications into project channels.`
Primary button label is `Create App`.
Create dialog fields: name (required), description (optional), icon upload through the existing media path used by profile/workspace icons.
On accepted create, open `AppCredentialsDialog` with callback URL `{relayHttpUrl}/hooks/apps/{appId}` and the secret from `response:` JSON, each with a `CopyButton`.
`onOpenChange(false)` clears secret state.
Rotate requires a confirm dialog titled `Rotate secret?` whose body says `The previous secret stops working immediately.`
Disable requires a confirm dialog titled `Disable app?` whose body says `New callbacks will be rejected. Existing messages stay visible.`
There is no delete action.
List rows show icon, name, description, status badge, callback URL, and a menu with Edit, Rotate secret, Enable, Disable.
Do not optimistic-set `active` before the relay accepts the command.
Show explicit loading, empty, permission, relay-error, validation-error, and pending mutation states.
If `SettingsPanels.tsx` would exceed 1000 lines, extract `settingsSections` to `desktop/src/features/settings/ui/settingsSections.ts` in this task before adding Apps.

- [ ] **Step 4: Run tests to verify they pass**

Run: `. ./bin/activate-hermit && cd desktop && node --test src/features/apps/appModels.test.mjs`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/src/features/apps desktop/src/features/settings/ui/SettingsPanels.tsx desktop/src/features/settings/ui/SettingsView.tsx desktop/tests/helpers/settings.ts && git commit -s -m "feat(desktop): add owner-admin Apps settings"
```

---

### Task 10: Relay end-to-end tests

**Files:**

- Create: `crates/buzz-test-client/tests/e2e_app_callback_notifications.rs`
- Modify: `crates/buzz-test-client/Cargo.toml` only if a new dependency is required, which it should not be

**Interfaces:**

- Consumes: `BuzzTestClient`, existing workflow routing helpers, `CreateCommunityWithOwnerResult`.

- [ ] **Step 1: Write the failing ignored E2E tests**

Create the file with these tests, all `#[ignore]`:

1. `owner_creates_app_and_queries_metadata`
2. `member_cannot_create_app`
3. `callback_without_secret_is_401_and_creates_no_delivery`
4. `two_repositories_in_one_project_post_to_the_same_channel`
5. `missing_and_ambiguous_routes_create_no_message`
6. `identical_retries_and_concurrent_callbacks_create_one_message`
7. `idempotency_key_reuse_with_different_content_is_409`
8. `disabled_app_is_404_and_metadata_remains_queryable`
9. `message_has_app_tags_and_no_author_p`
10. `existing_static_and_dynamic_workflow_webhooks_still_work`

Use `harness-service` and `agentic-os-plan` as repository `d` tags, suffixing with `unique()` to avoid collisions.
Create one project with `buzz-channel` set to a live stream channel.
Create the App as the community owner.
POST JSON:

```json
{
  "idempotency_key": "stable-provider-event-id",
  "repository_name": "harness-service",
  "event_type": "workflow.run.completed",
  "content": "✅ Archon completed speckit-feature for harness-service",
  "metadata": {}
}
```

Assert HTTP `202`, one kind `9` in that channel, signer is the relay, tags include `buzz:app`, both `a` coordinates, `buzz:app-delivery`, `buzz:app-event`, and no author `p` for the owner.
Assert a second POST with the same key and body returns `202` and still one event.
Assert a POST with the same key and different `content` returns `409`.
Call the existing workflow webhook helper from `e2e_workflow_project_channel_routing.rs` patterns to prove `/hooks/{workflow_id}` is unchanged.
Do not copy that whole file; import shared helpers only if they are already public.
If they are private, duplicate the small HTTP helper locally rather than refactoring the workflow tests in this task.

- [ ] **Step 2: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && RELAY_URL=ws://localhost:3000 cargo test -p buzz-test-client --test e2e_app_callback_notifications -- --ignored --nocapture`

Expected: FAIL on create App or callback route until Tasks 4–5 are deployed on a running relay.
If the relay is not running, start it with `. ./bin/activate-hermit && cargo build --release -p buzz-relay && ./target/release/buzz-relay` in a separate terminal after `just setup`.

- [ ] **Step 3: Fix product code only if a test reveals a spec miss**

Do not weaken assertions.
If a helper is wrong, fix the helper.

- [ ] **Step 4: Re-run the ignored suite**

Run the same command as Step 2.

Expected: PASS against a local relay with Postgres and Redis.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-test-client/tests/e2e_app_callback_notifications.rs && git commit -s -m "test(relay): cover App callback admission, routing, and attribution"
```

---

### Task 11: Desktop E2E tests

**Files:**

- Create: `desktop/tests/e2e/apps-settings.spec.ts`
- Create: `desktop/tests/e2e/apps-attribution.spec.ts`
- Modify: `desktop/playwright.config.ts`
- Modify: `desktop/src/testing/e2eBridge.ts`
- Modify: `desktop/tests/helpers/settings.ts`

**Interfaces:**

- Consumes: `installMockBridge`, `waitForAnimations`, mock owner membership.

- [ ] **Step 1: Write the failing Playwright specs**

`apps-settings.spec.ts` must:

1. Log in as the mock owner and open Settings → Apps.
2. Assert the empty state and `Create App`.
3. Create `Archon`, assert the credentials dialog shows a callback URL containing `/hooks/apps/` and a secret, then close it and assert the secret node is gone.
4. Assert the list shows `Archon` as active.
5. Edit the description and assert the list updates after the mock relay accepts kind `9038`.
6. Rotate secret and require the confirm dialog.
7. Disable and enable the App.

Add a second test that a mock member session does not render `settings-nav-apps`.

`apps-attribution.spec.ts` must navigate to `#engineering`, emit a mock kind `9` signed by the mock relay with `buzz:app` plus a mention `p` for Kevin's pubkey, provide kind `39007` metadata named `Archon`, and assert `data-testid="message-author"` is `Archon` and `data-testid="message-app-badge"` is visible and the row does not contain Kevin's display name.

Register both specs in the Playwright `smoke` project `testMatch`.

Extend `e2eBridge.ts` so kind `9038` create returns `response:{"app_id":"...","callback_url":"...","secret":"..."}` and stores a kind `39007` event authored by the mock relay.
Closing credentials in the UI must not keep the secret in the DOM.

- [ ] **Step 2: Run tests to verify they fail**

Run: `. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- apps-settings.spec.ts apps-attribution.spec.ts`

Expected: FAIL before the bridge and UI wiring exist.

- [ ] **Step 3: Implement mock-bridge handling and fix UI gaps**

Handle kind `9038` next to the workflow save mock around the existing `webhook_secret` response.
Emit kind `39007` with tags `d`, `name`, `status`.
Do not change unrelated mock channels.

- [ ] **Step 4: Re-run smoke tests**

Run the same command as Step 2.

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add desktop/tests/e2e/apps-settings.spec.ts desktop/tests/e2e/apps-attribution.spec.ts desktop/playwright.config.ts desktop/src/testing/e2eBridge.ts desktop/tests/helpers/settings.ts && git commit -s -m "test(desktop): cover Apps settings and App message attribution"
```

---

### Task 12: Docs, regression, and live acceptance runbook

**Files:**

- Modify: `NOSTR.md`
- Modify: `crates/buzz-cli/TESTING.md` with a short Apps section copied from the commands in this plan
- Modify: `CONTRIBUTING.md` only if the new kinds need a mention beyond `kind.rs`

**Interfaces:**

- Consumes: none.

- [ ] **Step 1: Write the failing doc assertions as a checklist test in the commit message body, then update the docs**

There is no separate doc test runner.
Treat the following commands as the failing-then-passing validation for this task.

- [ ] **Step 2: Update NOSTR.md**

Add a table row for kind `39007` App metadata and kind `9038` App admin command.
State that `39007` is relay-signed, contains no secret, and is the App actor clients may render.
State that `9038` is owner/admin-only JSON with actions `create`, `update`, `rotate_secret`, `enable`, and `disable`.
State that App callbacks are `POST /hooks/apps/{app_id}` and are not workflow webhooks.

- [ ] **Step 3: Run the full local validation set**

```bash
. ./bin/activate-hermit && cargo test -p buzz-core --lib app::tests -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-cli apps -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib project_route app_admin app_sink -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib app::tests app_admission::tests -- --ignored --nocapture
. ./bin/activate-hermit && cd desktop && node --test src/shared/lib/appActor.test.mjs src/shared/lib/authors.test.mjs src/features/apps/appModels.test.mjs
. ./bin/activate-hermit && cd mobile && flutter test test/shared/relay/app_actor_test.dart test/shared/widgets/message_author_meta_test.dart
. ./bin/activate-hermit && just file-size-check
```

Expected: PASS.

If a live local relay is available, also run:

```bash
. ./bin/activate-hermit && RELAY_URL=ws://localhost:3000 cargo test -p buzz-test-client --test e2e_app_callback_notifications -- --ignored --nocapture
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- apps-settings.spec.ts apps-attribution.spec.ts
```

- [ ] **Step 4: Record the live gigo-harness acceptance procedure in `crates/buzz-cli/TESTING.md`**

Add a section titled `Apps` that contains these exact operator steps:

1. Sign in as a `gigo-harness` owner or admin.
2. Run `buzz apps create --name Archon --description "Archon repository notifications"`.
3. Copy `callback_url` and `secret` from the JSON once.
4. Configure both Archon provider bindings with that URL and secret.
5. Keep Archon JSONata responsible for the normalized callback object.
6. Invoke the Archon manual callback for `harness-service`.
7. Invoke the Archon manual callback for `agentic-os-plan`.
8. Confirm both messages appear in `gigo-harness` as `Archon · App`.
9. Retry one callback and confirm no second message.
10. Disable the old `Archon Notifications` workflow only after both callbacks pass.

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add NOSTR.md crates/buzz-cli/TESTING.md && git commit -s -m "docs: document App metadata, admin commands, and callback live test"
```

---

## Open Questions

1. **Reserved callback fields.**
   The spec allows treating extra fields as metadata or rejecting collisions.
   **Provisional default:** reject unknown top-level keys and reject reserved keys anywhere in `metadata`.

2. **Metadata size limits.**
   The spec requires configured size and nesting limits without numbers.
   **Provisional default:** 8192 bytes, depth 4, 32 keys per object.

3. **Secret encoding.**
   The spec requires at least 256 bits and does not name an encoding.
   **Provisional default:** 32 random bytes as lowercase hex.

4. **Rate limit.**
   The spec requires limits by community, App, and source address without numbers.
   **Provisional default:** 60 requests per 60 seconds.

5. **Open relays without an owner/admin roster.**
   The spec requires owner/admin authorization.
   **Provisional default:** reject App commands when the signer is not a current `relay_members` owner or admin.

6. **Workflow triggering.**
   App messages are kind `9` and would otherwise match `message_posted` workflows.
   **Provisional default:** skip workflow triggering for relay-signed `buzz:app` messages.

7. **Search signature re-verification.**
   Search hits do not include signatures.
   **Provisional default:** accept App attribution on search and feed hits when the stored signer is the active relay, `buzz:app` is a UUID, and kind `39007` exists.

8. **App author click target.**
   The spec does not define an App profile surface.
   **Provisional default:** do not open `UserProfilePopover` for App authors.

## Acceptance Criteria Mapping

| Criterion | Task |
| --- | --- |
| Owner/admin create and list in Desktop and CLI | 6, 9, 11 |
| Member cannot mutate Apps | 4, 10, 11 |
| Secret returned once and stored hashed | 1, 2, 4, 6 |
| Kind `39007` public metadata | 4, 10 |
| One App endpoint for multiple repositories | 5, 10 |
| Callback cannot select channel or project | 1, 5, 10 |
| Exactly one claim-valid project route | 3, 5, 10 |
| Missing or ambiguous routes create no message | 5, 10 |
| No App channel membership or read capability | 5 |
| Identical and concurrent retries are one delivery | 2, 5, 10 |
| Different payload with same key is `409` | 2, 5, 10 |
| Relay-signed `Archon · App` without Kevin | 5, 7, 8, 10, 11 |
| Spoofed App marker fails closed | 7, 8, 10 |
| Disabled Apps reject callbacks and keep metadata | 4, 5, 10 |
| Workflow webhooks and human/agent authors regress cleanly | 5, 10 |
| Live `harness-service` and `agentic-os-plan` in `gigo-harness` | 12 |

## Validation Commands

```bash
. ./bin/activate-hermit && cargo test -p buzz-core --lib app::tests -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib project_route app_admin app_sink -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib app::tests app_admission::tests -- --ignored --nocapture
. ./bin/activate-hermit && cargo test -p buzz-cli apps -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-audit --lib action -- --nocapture
. ./bin/activate-hermit && RELAY_URL=ws://localhost:3000 cargo test -p buzz-test-client --test e2e_app_callback_notifications -- --ignored --nocapture
. ./bin/activate-hermit && cd desktop && node --test src/shared/lib/appActor.test.mjs src/shared/lib/authors.test.mjs src/features/apps/appModels.test.mjs
. ./bin/activate-hermit && cd desktop && pnpm test:e2e:smoke -- apps-settings.spec.ts apps-attribution.spec.ts
. ./bin/activate-hermit && cd mobile && flutter test test/shared/relay/app_actor_test.dart test/shared/widgets/message_author_meta_test.dart
. ./bin/activate-hermit && just file-size-check
```

## Self-Review

1. **Spec coverage:** Every Goals, Non-goals, data-model, protocol, callback, routing, idempotency, attribution, error, audit, compatibility, testing, rollout, and acceptance item maps to a task above.
2. **Placeholder scan:** No TBD, TODO, or "similar to Task N" leftovers.
3. **Type consistency:** `AppAdminCommand`, `AppCallbackRequest`, `AppFailure`, `AppRecord`, `AppAdmissionGuard`, `ResolvedEventActor`, and kind numbers are reused with the same names in later tasks.
