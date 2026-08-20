# Webhook Project Channel Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let one opted-in webhook workflow resolve `repository_name` to the single authorized NIP-MP project channel, admit the callback idempotently, and post one relay-signed kind `9` root with repository, project, and run provenance.

**Architecture:** Resolve and authorize the route in Buzz webhook admission, persist an immutable server-only snapshot on the workflow run, and revalidate that exact snapshot immediately before each dynamic `send_message`.
The generic executor never reads callback identity fields to choose a channel.
Static workflows without `routing` keep today's stored-channel behavior.

**Tech Stack:** Rust, Postgres, Axum, `serde_yaml` / `serde_json`, `sha2`, `nostr`, `sqlx`, existing `buzz-workflow` / `buzz-db` / `buzz-relay` crates, and `buzz-test-client` ignored relay E2E tests.

**Spec:** [docs/superpowers/specs/2026-08-20-buzz-project-channel-routing-design.md](../specs/2026-08-20-buzz-project-channel-routing-design.md)

**Product contract:** [VISION.md](../../../VISION.md) treats Workflows as YAML-as-code automation and the request host as the community boundary.
[VISION_PROJECTS.md](../../../VISION_PROJECTS.md) and [NIP-MP.md](../../nips/NIP-MP.md) keep `buzz-channel` as metadata rather than git or ingest authority.
This plan uses that tag only as a destination reference after separate claim, channel-state, and workflow-owner membership checks.

**Naming note:** This plan implements the webhook design spec.
It is not the desktop Open Discussion plan at [2026-08-20-project-channel-routing.md](2026-08-20-project-channel-routing.md).
Do not edit that desktop plan or desktop UI files.

## Product tension

VISION_PROJECTS describes branch channels as the long-term home for CI conversation.
This inbound slice posts to the project's `buzz-channel`, not a branch channel, because the approved design is an inbound notification hub with no branch-channel creation.
That tension is intentional and in-scope.
Do not add branch-channel routing, outbound GitHub replies, or agent selection to close it.

## Global Constraints

- Implement the approved inbound webhook route only.
- Do not add a new HTTP endpoint, service, cache, projection, background reconciler, or provider adapter.
- Do not add a webhook-specific audit chain; successful messages continue through the existing `EventCreated` audit path.
- Do not change Archon, GitHub, the vault, desktop, mobile, web, or CLI.
- Do not implement Buzz-thread-to-GitHub-comment delivery or any GitHub token/outbound step.
- Do not bind routing to Archon, Hermes, Codex, Grok, or any other named provider or agent.
- Absence of `routing` is the compatibility switch for every existing workflow.
- Dynamic routing is valid only with `trigger.on: webhook` and `routing.mode: project_channel_by_repository`.
- For a dynamic workflow, every `send_message.channel` field is invalid.
- Duplicate YAML alias keys are invalid even when both entries name the same coordinate.
- The workflow home channel is the management and standing-authority boundary, never a fallback destination.
- Callback JSON is untrusted identity input.
- Aliases grant identity recognition only, never claim or channel authority.
- Repository identity uses the fixed tier order `d_tag`, then `alias`, then `clone_basename`, then `display_name`.
- Comparisons are case-sensitive and must not trim or normalize Unicode, spaces, hyphens, or underscores.
- The first identity tier with any match stops.
- One distinct coordinate at that tier succeeds.
- Two or more distinct coordinates at that tier fail closed.
- A weaker tier must never break a stronger-tier tie.
- Zero or multiple claim-valid projects fail closed.
- A listed project counts only when its latest live head is not `buzz-visibility: unlisted` and its signer is the repository owner or a current `maintainers` value.
- Desktop-local hidden state is not server authority.
- After the exactly-one project check, a bad channel fails that route rather than selecting another project.
- Dynamic destination membership is required even when the channel is open.
- The project signer does not also need destination-channel membership.
- There is no fallback to the home channel, another project, a weaker identity tier, or a provider-supplied channel.
- Failures never post an error message to any channel.
- `idempotency_key` is control-plane only: hash it, then drop it before trigger context, templates, traces, logs, errors, message content, and provenance.
- Same key plus same payload hash returns the existing run and must not start another resolution, execution, or message.
- Same key plus different payload hash returns `409` and must not change the existing run.
- A stored deterministic admission failure is replayed as the original redacted `422`.
- A retry of an admitted run must not restart it.
- No production `unsafe`, `unwrap()`, or `expect()`.
- New public Rust API needs doc comments.
- Activate Hermit before every shell command; every command block below includes the exact `. ./bin/activate-hermit &&` prefix.
- Sign every commit with `git commit -s`.
- Follow TDD for each behavior: add the named failing test, run it and confirm the expected failure, implement only that task, run the same test green, then commit.
- Do not add production code for a later task to make an earlier task pass.

## GitNexus Gates

- Before editing an existing function, class, method, or exported type, run `impact({ target: "<symbol>", direction: "upstream" })` and report direct callers, affected processes, and risk.
- Expected high-touch symbols are `workflow_webhook`, `WorkflowDef::validate`, `parse_yaml`, `create_workflow_run`, `resolve_send_message_channel`, `ActionSink::send_message`, `RelayActionSink::send_message`, and `handle_workflow_def`.
- If impact expands into git push policy, CLI, desktop, mobile, or a new HTTP route, stop and warn.
- If GitNexus reports HIGH or CRITICAL on those expected webhook/workflow symbols, record the blast radius and continue only when it matches this file map.
- If the index is stale and `.gitnexus/run.cjs` exists, run `. ./bin/activate-hermit && node .gitnexus/run.cjs analyze` from the repository root.
- Before every commit, stage only that task's files and run `detect_changes({ scope: "staged" })`.
- Before final handoff, run `detect_changes({ scope: "compare", base_ref: "main" })`.
- If GitNexus MCP tools are unavailable after refreshing the index, record that in the handoff and use `git diff --stat`, `git diff --name-only`, and `git diff --check` as fallback scope evidence.

## Resolved Implementation Decisions

These are the safe defaults for product and layout choices the spec left to implementation.

- Persist `idempotency_key_hash BYTEA`, `payload_hash BYTEA`, and `route_snapshot JSONB` on `workflow_runs`.
- Enforce uniqueness with a partial unique index on `(community_id, workflow_id, idempotency_key_hash) WHERE idempotency_key_hash IS NOT NULL`.
- Require both hashes together, and require each non-null hash to be exactly 32 bytes.
- Make the two hashes and `route_snapshot` immutable after insert with a database trigger.
- Store the route snapshot only after complete successful resolution and authorization.
- Serialize same-key admission with a transaction-scoped PostgreSQL advisory lock on the community id, workflow id, and hashed key before any repository, project, or channel resolution.
- A transaction that encounters a transient dependency failure inserts no row and releases the lock when its admission guard rolls back.
- The database uniqueness constraint remains the final correctness backstop even though the advisory lock prevents duplicate resolution work.
- Snapshot JSON fields are `community_id`, `repository_coordinate`, `project_coordinate`, `channel_id`, and `matched_identity_tier`.
- Repository coordinates are `30617:<64-lowercase-hex>:<d>` parsed by splitting on the first two colons.
- Project coordinates are `30621:<64-lowercase-hex>:<d>` with the same split.
- Identity tier strings are exactly `d_tag`, `alias`, `clone_basename`, and `display_name`.
- Canonical payload bytes are `serde_json::to_vec` of the parsed `serde_json::Value` after removing `idempotency_key`.
- Workspace `serde_json` does not enable `preserve_order`, so object keys serialize in sorted `BTreeMap` order and key order in the raw request is ignored.
- Array order, field presence, scalar type, and scalar value remain significant.
- Missing, non-string, or empty `repository_name` / `idempotency_key` uses failure code `invalid_control_fields`.
- Dynamic webhook error JSON is `{ "error": "<redacted>", "code": "<stable>" }`.
- Static webhook responses stay `{ "error": "<msg>" }` with no `code` field.
- Successful `202` body stays `{ "run_id", "workflow_id", "status" }` for both static and dynamic.
- `422` and `409` bodies do not include `run_id`.
- Exactly one terminal `.git` suffix is removed from a contributed clone segment.
- A missing, empty, or `.git`-only clone segment contributes no identity.
- A trailing-slash URL whose final segment is empty contributes no identity.
- Every `clone` tag contributes every value after the tag name.
- Every `maintainers` tag contributes every value after the tag name, compared exactly with no case folding.
- `buzz-visibility` is unlisted only when the first value is exactly `unlisted`.
- Absent or any other visibility token is listed.
- Deserialize aliases with a duplicate-detecting map visitor and reject the second occurrence of an exact alias key.
- Save-time alias validation requires every alias target to have a latest live repository head in the host community.
- Admission revalidates an alias target only when the alias tier is the matching tier.
- E2E tests may suffix `d` tags to avoid collisions on a shared local relay.
- Unit tests use the exact names `agentic-os-plan` and `harness-service`.

## File Map

| File | Responsibility |
|------|----------------|
| Create `crates/buzz-workflow/src/routing.rs` | Routing types, coordinate parse, control-field stripping, SHA-256 helpers, stable failure codes |
| Modify `crates/buzz-workflow/src/schema.rs` | Optional `routing` on `WorkflowDef`, validation of trigger/alias/`send_message.channel` |
| Modify `crates/buzz-workflow/src/error.rs` | `RouteStale` and routing failure mapping |
| Modify `crates/buzz-workflow/src/lib.rs` | Re-export routing types |
| Modify `crates/buzz-workflow/Cargo.toml` | Add `sha2` |
| Modify `crates/buzz-workflow/src/action_sink.rs` | Optional `WorkflowMessageRoute` on `send_message` |
| Modify `crates/buzz-workflow/src/executor.rs` | Dynamic runs use the snapshot channel and pass provenance |
| Create `migrations/0032_workflow_run_route_idempotency.sql` | Hash columns, snapshot column, length checks, partial unique index |
| Modify `schema/schema.sql` | Keep the checked-in schema in sync with the migration |
| Create `crates/buzz-db/src/workflow_admission.rs` | Advisory-lock admission guard, immutable snapshot type, existing/conflict decisions, and final run insert |
| Modify `crates/buzz-db/src/workflow.rs` | Run record fields and row decoding for hashes and snapshot |
| Create `crates/buzz-db/src/project_heads.rs` | Unlimited latest live `30617`/`30621` head queries |
| Modify `crates/buzz-db/src/lib.rs` | Module + `Db` wrappers |
| Create `crates/buzz-relay/src/workflow_route.rs` | Identity tiers, claim filter, destination authorization, revalidation |
| Create `crates/buzz-relay/src/workflow_admission.rs` | Dynamic webhook admission orchestration |
| Modify `crates/buzz-relay/src/lib.rs` | Declare the new modules |
| Modify `crates/buzz-relay/src/api/bridge.rs` | Keep `workflow_webhook` as a thin host-binding wrapper |
| Modify `crates/buzz-relay/src/api/mod.rs` | `api_error_with_code` |
| Modify `crates/buzz-relay/src/handlers/command_executor.rs` | Save-time live alias target check |
| Modify `crates/buzz-relay/src/workflow_sink.rs` | Revalidate dynamic routes and add provenance tags |
| Modify `Justfile` and `scripts/run-tests.sh` | Run `buzz-workflow --lib` in unit CI |
| Modify `crates/buzz-test-client/Cargo.toml` | Add test-only `buzz-db` access for scoped workflow-run and foreign-community fixtures |
| Create `crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs` | Relay-backed webhook routing flow |

## Out of Scope

- Desktop Open Discussion / Channels-tab routing.
- New kind integers.
- Materialized repository-to-channel projections.
- General exactly-once execution beyond webhook admission and run idempotency.
- Recursive workflow triggering changes beyond the existing `buzz:workflow` marker.
- Agent wake policy beyond ordinary `p` mentions.

## Required Implementation Order

Execute the tasks in dependency order `1 → 2 → 3 → 4 → 5 → 6 → 7 → 8`.
Task 6 builds and tests webhook admission behind an uncalled relay helper.
Task 7 wires snapshot-aware execution and fail-closed side effects, then activates the helper as its final implementation step, so no intermediate commit can admit a dynamic run that still posts through the static home-channel path.

---

### Task 1: Routing schema, control-plane hashing, and failure codes

**Files:**

- Create: `crates/buzz-workflow/src/routing.rs`
- Modify: `crates/buzz-workflow/src/schema.rs`
- Modify: `crates/buzz-workflow/src/lib.rs`
- Modify: `crates/buzz-workflow/Cargo.toml`

**Interfaces:**

- Consumes: existing `WorkflowDef`, `TriggerDef`, `ActionDef`, `WorkflowError`, `parse_yaml`.
- Produces:
  - `pub struct RoutingDef { pub mode: RoutingMode, pub aliases: BTreeMap<String, String> }`
  - `pub enum RoutingMode { ProjectChannelByRepository }` serialized as `project_channel_by_repository`
  - `pub enum RouteFailure` with `code()`, `http_status()`, and `redacted_message()`
  - `pub struct RepositoryCoordinate { owner_pubkey: [u8; 32], d_tag: String }` with `owner_pubkey()`, `owner_hex()`, `d_tag()`, and `as_coordinate()` accessors
  - `pub fn parse_repository_coordinate(coord: &str) -> Result<RepositoryCoordinate, WorkflowError>`
  - `pub fn parse_idempotency_key(body: &serde_json::Value) -> Result<String, RouteFailure>`
  - `pub fn parse_repository_name(body: &serde_json::Value) -> Result<String, RouteFailure>`
  - `pub fn strip_idempotency_key(body: serde_json::Value) -> serde_json::Value`
  - `pub fn hash_idempotency_key(raw: &str) -> [u8; 32]`
  - `pub fn canonical_payload_hash(body: &serde_json::Value) -> Result<[u8; 32], WorkflowError>`
  - `impl WorkflowDef { pub fn has_project_channel_routing(&self) -> bool }`

**Acceptance:** Static workflow JSON omits `routing` when it is absent, duplicate alias keys fail parsing, valid dynamic definitions round-trip, exact coordinate parsing preserves colon-bearing `d` tags, and the raw idempotency key is used only as hash input.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `WorkflowDef`, `WorkflowDef::validate`, and `parse_yaml`.
Report callers and risk.
The expected callers are YAML parse sites and webhook/event execution.
Stop only if impact shows git, CLI, or UI writers.

- [ ] **Step 2: Add `sha2` and the module skeleton, then write failing tests**

In `crates/buzz-workflow/Cargo.toml` add `sha2 = { workspace = true }` next to the other workspace crates.
In `crates/buzz-workflow/src/lib.rs` add `pub mod routing;` and re-export `RepositoryCoordinate`, `RouteFailure`, `RoutingDef`, and `RoutingMode`.
Create `routing.rs` with the public types first referenced from tests so the tests compile against named APIs.

Add these tests at the bottom of `schema.rs` in the existing `mod tests`:

```rust
    #[test]
    fn parse_project_channel_routing_with_empty_aliases() {
        let yaml = concat!(
            "name: Multi Repo Notify\n",
            "trigger:\n  on: webhook\n",
            "routing:\n  mode: project_channel_by_repository\n",
            "steps:\n  - id: notify\n    action: send_message\n    text: hi\n",
        );
        let (def, json) = parse_yaml(yaml).expect("dynamic workflow should parse");
        assert!(def.has_project_channel_routing());
        let reparsed: WorkflowDef = serde_json::from_str(&json).expect("json round-trip");
        assert!(reparsed.has_project_channel_routing());
        assert!(reparsed.routing.unwrap().aliases.is_empty());
    }

    #[test]
    fn routing_rejected_on_non_webhook_trigger() {
        let yaml = concat!(
            "name: Bad Routing\n",
            "trigger:\n  on: message_posted\n",
            "routing:\n  mode: project_channel_by_repository\n",
            "steps:\n  - id: notify\n    action: send_message\n    text: hi\n",
        );
        let err = parse_yaml(yaml).unwrap_err();
        match err {
            WorkflowError::InvalidDefinition(msg) => {
                assert!(msg.contains("webhook"), "expected webhook-only routing, got {msg}");
            }
            other => panic!("expected InvalidDefinition, got {other}"),
        }
    }

    #[test]
    fn routing_rejects_send_message_channel_override() {
        let yaml = concat!(
            "name: Routed Channel Override\n",
            "trigger:\n  on: webhook\n",
            "routing:\n  mode: project_channel_by_repository\n",
            "steps:\n  - id: notify\n    action: send_message\n    text: hi\n    channel: general\n",
        );
        let err = parse_yaml(yaml).unwrap_err();
        match err {
            WorkflowError::InvalidDefinition(msg) => {
                assert!(
                    msg.contains("send_message.channel"),
                    "expected channel prohibition, got {msg}"
                );
            }
            other => panic!("expected InvalidDefinition, got {other}"),
        }
    }

    #[test]
    fn static_webhook_without_routing_still_allows_channel_override_field() {
        let yaml = concat!(
            "name: Static Webhook\n",
            "trigger:\n  on: webhook\n",
            "steps:\n  - id: notify\n    action: send_message\n    text: hi\n    channel: general\n",
        );
        let (def, _) = parse_yaml(yaml).expect("static webhook remains valid");
        assert!(!def.has_project_channel_routing());
    }

    #[test]
    fn static_workflow_canonical_json_omits_routing() {
        let yaml = concat!(
            "name: Static\n",
            "trigger:\n  on: message_posted\n",
            "steps:\n  - id: notify\n    action: send_message\n    text: hi\n",
        );
        let (_, json) = parse_yaml(yaml).expect("static workflow should parse");
        assert!(!json.contains("\"routing\""));
    }

    #[test]
    fn duplicate_alias_keys_are_rejected() {
        let target = format!("30617:{}:harness-service", "ab".repeat(32));
        let yaml = format!(
            "name: Duplicate Alias\n\
             trigger:\n  on: webhook\n\
             routing:\n  mode: project_channel_by_repository\n\
             \x20 aliases:\n    hs: {target}\n    hs: {target}\n\
             steps:\n  - id: notify\n    action: send_message\n    text: hi\n"
        );
        assert!(parse_yaml(&yaml).is_err());
    }

    #[test]
    fn distinct_alias_keys_may_share_one_coordinate() {
        let target = format!("30617:{}:harness-service", "ab".repeat(32));
        let yaml = format!(
            "name: Shared Alias Target\n\
             trigger:\n  on: webhook\n\
             routing:\n  mode: project_channel_by_repository\n\
             \x20 aliases:\n    hs: {target}\n    harness: {target}\n\
             steps:\n  - id: notify\n    action: send_message\n    text: hi\n"
        );
        let (def, _) = parse_yaml(&yaml).expect("distinct aliases are valid");
        assert_eq!(def.routing.expect("routing").aliases.len(), 2);
    }
```

Add these tests in `routing.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn idempotency_key_requires_a_non_empty_string() {
        let err = parse_idempotency_key(&json!({"idempotency_key": ""})).unwrap_err();
        assert_eq!(err, RouteFailure::InvalidControlFields);
        assert_eq!(err.code(), "invalid_control_fields");
        assert_eq!(err.http_status(), 422);
        for invalid in [json!({}), json!({"idempotency_key": 7})] {
            assert_eq!(
                parse_idempotency_key(&invalid).unwrap_err(),
                RouteFailure::InvalidControlFields
            );
        }
        assert_eq!(
            parse_idempotency_key(&json!({"idempotency_key": " "})).unwrap(),
            " "
        );
    }

    #[test]
    fn repository_name_is_non_empty_but_is_not_trimmed() {
        assert_eq!(
            parse_repository_name(&json!({"repository_name": ""})).unwrap_err(),
            RouteFailure::InvalidControlFields
        );
        assert_eq!(
            parse_repository_name(&json!({"repository_name": " "})).unwrap(),
            " "
        );
    }

    #[test]
    fn strip_removes_only_idempotency_key() {
        let body = json!({
            "repository_name": "agentic-os-plan",
            "idempotency_key": "secret-key",
            "title": "hi"
        });
        let stripped = strip_idempotency_key(body);
        assert_eq!(stripped["repository_name"], "agentic-os-plan");
        assert_eq!(stripped["title"], "hi");
        assert!(stripped.get("idempotency_key").is_none());
    }

    #[test]
    fn canonical_payload_hash_ignores_object_key_order() {
        let a = strip_idempotency_key(json!({
            "b": 1,
            "a": "x",
            "idempotency_key": "k1"
        }));
        let b = strip_idempotency_key(json!({
            "idempotency_key": "k2",
            "a": "x",
            "b": 1
        }));
        assert_eq!(
            canonical_payload_hash(&a).expect("hash a"),
            canonical_payload_hash(&b).expect("hash b")
        );
    }

    #[test]
    fn canonical_payload_hash_keeps_array_order_significant() {
        let a = json!({"items": [1, 2]});
        let b = json!({"items": [2, 1]});
        assert_ne!(
            canonical_payload_hash(&a).expect("hash a"),
            canonical_payload_hash(&b).expect("hash b")
        );
    }

    #[test]
    fn idempotency_key_hash_is_sha256_of_raw_bytes() {
        let digest = hash_idempotency_key("abc");
        assert_eq!(digest.len(), 32);
        assert_ne!(digest, hash_idempotency_key("ABC"));
    }

    #[test]
    fn repository_coordinate_rejects_uppercase_owner_and_bare_d() {
        let upper = "30617:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:repo";
        assert!(parse_repository_coordinate(upper).is_err());
        assert!(parse_repository_coordinate("repo").is_err());
        let ok = format!("30617:{}:group:agentic-os-plan", "ab".repeat(32));
        let parsed = parse_repository_coordinate(&ok).expect("valid coordinate");
        assert_eq!(parsed.d_tag(), "group:agentic-os-plan");
        assert_eq!(parsed.as_coordinate(), ok);
    }

    #[test]
    fn alias_target_must_be_full_coordinate_not_clone_url() {
        let yaml = concat!(
            "name: Bad Alias\n",
            "trigger:\n  on: webhook\n",
            "routing:\n  mode: project_channel_by_repository\n",
            "  aliases:\n    plan: https://github.com/acme/agentic-os-plan.git\n",
            "steps:\n  - id: notify\n    action: send_message\n    text: hi\n",
        );
        let err = crate::schema::parse_yaml(yaml).unwrap_err();
        assert!(matches!(err, crate::WorkflowError::InvalidDefinition(_)));
    }
}
```

- [ ] **Step 3: Run the new tests and confirm they fail**

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib schema::tests::parse_project_channel_routing_with_empty_aliases -- --exact --nocapture
```

Expected: FAIL because `routing` / `has_project_channel_routing` do not exist.

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib routing::tests -- --nocapture
```

Expected: FAIL because `routing.rs` APIs are missing or unimplemented.

- [ ] **Step 4: Implement the minimal schema and hashing code**

Add to `WorkflowDef` after `trigger`:

```rust
    /// Optional inbound routing contract.
    ///
    /// Present only for webhook workflows that opt into
    /// `project_channel_by_repository`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<RoutingDef>,
```

Do not add `deny_unknown_fields` to `WorkflowDef`.
Stored definitions include `_webhook_secret`, which must continue to deserialize by ignoring unknown fields.

In `routing.rs` implement:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RoutingDef {
    /// Routing mode. The only supported value is `project_channel_by_repository`.
    pub mode: RoutingMode,
    /// Exact alias key to full `30617:<owner>:<d>` coordinate.
    #[serde(default, deserialize_with = "deserialize_aliases_no_duplicates")]
    pub aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    /// Resolve `repository_name` through NIP-MP to one project `buzz-channel`.
    ProjectChannelByRepository,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteFailure {
    InvalidControlFields,
    RepositoryMissing,
    RepositoryAmbiguous,
    AliasTargetUnavailable,
    ProjectMissing,
    ProjectAmbiguous,
    ProjectChannelInvalid,
    RouteUnauthorized,
    RouteStale,
    IdempotencyConflict,
}

impl RouteFailure {
    /// Stable machine-readable code. Never include candidate lists or raw keys.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidControlFields => "invalid_control_fields",
            Self::RepositoryMissing => "repository_missing",
            Self::RepositoryAmbiguous => "repository_ambiguous",
            Self::AliasTargetUnavailable => "alias_target_unavailable",
            Self::ProjectMissing => "project_missing",
            Self::ProjectAmbiguous => "project_ambiguous",
            Self::ProjectChannelInvalid => "project_channel_invalid",
            Self::RouteUnauthorized => "route_unauthorized",
            Self::RouteStale => "route_stale",
            Self::IdempotencyConflict => "idempotency_conflict",
        }
    }

    /// HTTP status for the webhook admission boundary.
    pub const fn http_status(self) -> u16 {
        match self {
            Self::IdempotencyConflict => 409,
            Self::RouteStale => 422,
            _ => 422,
        }
    }

    /// Provider-visible redacted text. No coordinates, keys, or bodies.
    pub const fn redacted_message(self) -> &'static str {
        match self {
            Self::InvalidControlFields => "invalid routing control fields",
            Self::RepositoryMissing => "repository could not be resolved",
            Self::RepositoryAmbiguous => "repository is ambiguous",
            Self::AliasTargetUnavailable => "alias target is unavailable",
            Self::ProjectMissing => "project could not be resolved",
            Self::ProjectAmbiguous => "project is ambiguous",
            Self::ProjectChannelInvalid => "project channel is invalid",
            Self::RouteUnauthorized => "route is unauthorized",
            Self::RouteStale => "route is no longer valid",
            Self::IdempotencyConflict => "idempotency key already used with a different payload",
        }
    }

    /// Parse a stored admission failure code for deterministic replay.
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "invalid_control_fields" => Some(Self::InvalidControlFields),
            "repository_missing" => Some(Self::RepositoryMissing),
            "repository_ambiguous" => Some(Self::RepositoryAmbiguous),
            "alias_target_unavailable" => Some(Self::AliasTargetUnavailable),
            "project_missing" => Some(Self::ProjectMissing),
            "project_ambiguous" => Some(Self::ProjectAmbiguous),
            "project_channel_invalid" => Some(Self::ProjectChannelInvalid),
            "route_unauthorized" => Some(Self::RouteUnauthorized),
            "route_stale" => Some(Self::RouteStale),
            "idempotency_conflict" => Some(Self::IdempotencyConflict),
            _ => None,
        }
    }

    /// True only for deterministic admission failures that replay as `422`.
    pub const fn is_admission_rejection(self) -> bool {
        !matches!(self, Self::RouteStale | Self::IdempotencyConflict)
    }
}
```

Implement `deserialize_aliases_no_duplicates` as a private Serde map visitor.
For every `(key, value)` from `MapAccess::next_entry`, insert into a `BTreeMap`; when insertion replaces an existing value, return `serde::de::Error::custom(format!("duplicate routing alias key: {key}"))`.
Do not attempt to detect duplicates after deserialization because the overwritten value is already lost at that point.

`parse_repository_coordinate` must split with `splitn(3, ':')`, require literal `30617`, require a 64-character lowercase-hex owner, decode it into `[u8; 32]` without production `unwrap()` or `expect()`, and preserve the non-empty remainder verbatim as the `d` tag.
Uppercase hex is rejected, never folded.

`parse_idempotency_key` and `parse_repository_name` accept only a JSON object field whose value is a non-empty string.
They must not trim or normalize the string.

In `WorkflowDef::validate`, after the existing step-id checks, add:

```rust
        if let Some(routing) = &self.routing {
            if !matches!(self.trigger, TriggerDef::Webhook) {
                return Err(WorkflowError::InvalidDefinition(
                    "routing is valid only with trigger.on: webhook".into(),
                ));
            }
            if routing.mode != RoutingMode::ProjectChannelByRepository {
                return Err(WorkflowError::InvalidDefinition(
                    "unsupported routing.mode".into(),
                ));
            }
            for (alias_key, target) in &routing.aliases {
                if alias_key.is_empty() {
                    return Err(WorkflowError::InvalidDefinition(
                        "routing alias keys must be non-empty".into(),
                    ));
                }
                parse_repository_coordinate(target)?;
            }
            for step in &self.steps {
                if let ActionDef::SendMessage {
                    channel: Some(_), ..
                } = &step.action
                {
                    return Err(WorkflowError::InvalidDefinition(
                        "send_message.channel is invalid when routing.mode is project_channel_by_repository"
                            .into(),
                    ));
                }
            }
        }
```

Add `has_project_channel_routing` returning `self.routing.as_ref().is_some_and(|r| r.mode == RoutingMode::ProjectChannelByRepository)`.

`strip_idempotency_key` removes only that object key and leaves every other field untouched.
`canonical_payload_hash` hashes `serde_json::to_vec(value)`.
`hash_idempotency_key` hashes the raw UTF-8 bytes with `Sha256`.

Do not add `RouteStale` to `WorkflowError` in this task.
That mapping is Task 7.

- [ ] **Step 5: Run the Task 1 tests green**

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib -- --nocapture
```

Expected: PASS, including the pre-existing static schema tests such as `parse_all_action_types` and `enabled_defaults_to_true`.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-workflow/src/routing.rs crates/buzz-workflow/src/schema.rs crates/buzz-workflow/src/lib.rs crates/buzz-workflow/Cargo.toml Cargo.lock && git commit -s -m "feat(workflow): add project-channel routing schema"
```

---

### Task 2: Serialized idempotent workflow admission and immutable run identity

**Files:**

- Create: `migrations/0032_workflow_run_route_idempotency.sql`
- Create: `crates/buzz-db/src/workflow_admission.rs`
- Modify: `schema/schema.sql`
- Modify: `crates/buzz-db/src/workflow.rs`
- Modify: `crates/buzz-db/src/lib.rs`

**Interfaces:**

- Consumes: existing `WorkflowRunRecord`, `WorkflowRunFailure`, `RunStatus`, `get_workflow_run`, and `create_workflow_run`.
- Produces:
  - `pub struct WorkflowRouteSnapshot { pub community_id: Uuid, pub repository_coordinate: String, pub project_coordinate: String, pub channel_id: Uuid, pub matched_identity_tier: String }`
  - `WorkflowRunRecord` fields `idempotency_key_hash: Option<Vec<u8>>`, `payload_hash: Option<Vec<u8>>`, and `route_snapshot: Option<WorkflowRouteSnapshot>`
  - `pub enum BeginWorkflowAdmission { Existing(WorkflowRunRecord), PayloadConflict { existing: WorkflowRunRecord }, Vacant(WorkflowAdmissionGuard) }`
  - `pub struct WorkflowAdmissionGuard` that owns a `sqlx::Transaction<'static, Postgres>` and releases its advisory lock on commit or rollback
  - `pub async fn begin_workflow_admission(pool, community_id, workflow_id, idempotency_key_hash: &[u8; 32], payload_hash: &[u8; 32]) -> Result<BeginWorkflowAdmission>`
  - `WorkflowAdmissionGuard::accept(self, trigger_context: &Value, route_snapshot: &WorkflowRouteSnapshot) -> Result<WorkflowRunRecord>`
  - `WorkflowAdmissionGuard::reject(self, trigger_context: &Value, failure: WorkflowRunFailure<'_>) -> Result<WorkflowRunRecord>`
- Keeps the signature and behavior of `create_workflow_run` unchanged for static webhook, event, cron, manual, and approval paths.

**Acceptance:** The first caller for a key holds an admission lock before route lookup, a same-payload duplicate waits and then receives the existing row, a different payload receives a conflict, a transient rollback leaves no row, and stored hashes and route snapshots cannot be updated.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `create_workflow_run`, `get_workflow_run`, `row_to_run_record`, and `WorkflowRunRecord`.
Report all direct callers and affected execution flows.
Do not change the existing `create_workflow_run` signature.

- [ ] **Step 2: Write the failing snapshot and admission-lock tests**

Create `workflow_admission.rs` with public type declarations only and add this unit test there:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_snapshot_round_trips_all_server_fields() {
        let community_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let snapshot = WorkflowRouteSnapshot {
            community_id,
            repository_coordinate: format!("30617:{}:group:agentic-os-plan", "ab".repeat(32)),
            project_coordinate: format!("30621:{}:gigo-harness", "cd".repeat(32)),
            channel_id,
            matched_identity_tier: "d_tag".into(),
        };
        let value = serde_json::to_value(&snapshot).expect("serialize snapshot");
        let parsed: WorkflowRouteSnapshot =
            serde_json::from_value(value).expect("deserialize snapshot");
        assert_eq!(parsed, snapshot);
    }
}
```

In the existing `workflow.rs` test module, reuse `setup_pool`, `make_community`, and `make_workflow_in` and add ignored tests named below.
Make the test module and those three helpers `pub(crate)` under `#[cfg(test)]` so Task 3 can reuse the same fixtures instead of copying database setup.

```rust
#[tokio::test]
#[ignore = "requires Postgres"]
async fn admission_same_key_same_payload_returns_existing_row() {
    let pool = setup_pool().await;
    let community = make_community(&pool).await;
    let (workflow_id, _) = make_workflow_in(&pool, community).await;
    let key = [1_u8; 32];
    let payload = [9_u8; 32];

    let first = begin_workflow_admission(&pool, community, workflow_id, &key, &payload)
        .await
        .expect("begin first admission");
    let BeginWorkflowAdmission::Vacant(guard) = first else {
        panic!("first admission must be vacant");
    };
    let created = guard
        .reject(
            &serde_json::json!({"webhook_fields": {}}),
            WorkflowRunFailure { code: "repository_missing", message: "repository could not be resolved" },
        )
        .await
        .expect("persist deterministic rejection");

    let second = begin_workflow_admission(&pool, community, workflow_id, &key, &payload)
        .await
        .expect("begin duplicate admission");
    let BeginWorkflowAdmission::Existing(existing) = second else {
        panic!("same payload must return existing");
    };
    assert_eq!(existing.id, created.id);
}

#[tokio::test]
#[ignore = "requires Postgres"]
async fn admission_same_key_different_payload_is_conflict() {
    let pool = setup_pool().await;
    let community = make_community(&pool).await;
    let (workflow_id, _) = make_workflow_in(&pool, community).await;
    let key = [2_u8; 32];
    let original_payload = [8_u8; 32];
    let first = begin_workflow_admission(
        &pool,
        community,
        workflow_id,
        &key,
        &original_payload,
    )
    .await
    .expect("begin first admission");
    let BeginWorkflowAdmission::Vacant(guard) = first else {
        panic!("first admission must be vacant");
    };
    let created = guard
        .reject(
            &serde_json::json!({"webhook_fields": {}}),
            WorkflowRunFailure {
                code: "repository_missing",
                message: "repository could not be resolved",
            },
        )
        .await
        .expect("persist first admission");

    let second = begin_workflow_admission(
        &pool,
        community,
        workflow_id,
        &key,
        &[7_u8; 32],
    )
    .await
    .expect("begin conflicting admission");
    let BeginWorkflowAdmission::PayloadConflict { existing } = second else {
        panic!("changed payload must conflict");
    };
    assert_eq!(existing.id, created.id);
    assert_eq!(existing.payload_hash.as_deref(), Some(original_payload.as_slice()));
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM workflow_runs WHERE community_id = $1 AND workflow_id = $2",
    )
    .bind(community.as_uuid())
    .bind(workflow_id)
    .fetch_one(&pool)
    .await
    .expect("count workflow runs");
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "requires Postgres"]
async fn admission_same_key_waits_for_first_guard_before_existing_decision() {
    let pool = setup_pool().await;
    let community = make_community(&pool).await;
    let (workflow_id, _) = make_workflow_in(&pool, community).await;
    let key = [3_u8; 32];
    let payload = [7_u8; 32];
    let first = begin_workflow_admission(&pool, community, workflow_id, &key, &payload)
        .await
        .expect("begin first admission");
    let BeginWorkflowAdmission::Vacant(guard) = first else {
        panic!("first admission must be vacant");
    };

    let second_pool = pool.clone();
    let mut second = tokio::spawn(async move {
        begin_workflow_admission(
            &second_pool,
            community,
            workflow_id,
            &key,
            &payload,
        )
        .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut second)
            .await
            .is_err(),
        "duplicate admission must wait for the first transaction"
    );

    let created = guard
        .reject(
            &serde_json::json!({"webhook_fields": {}}),
            WorkflowRunFailure {
                code: "project_missing",
                message: "project could not be resolved",
            },
        )
        .await
        .expect("finish first admission");
    let second = second
        .await
        .expect("join second admission")
        .expect("begin second admission");
    let BeginWorkflowAdmission::Existing(existing) = second else {
        panic!("second admission must observe the committed row");
    };
    assert_eq!(existing.id, created.id);
}

#[tokio::test]
#[ignore = "requires Postgres"]
async fn admission_guard_rollback_leaves_no_row_and_allows_retry() {
    let pool = setup_pool().await;
    let community = make_community(&pool).await;
    let (workflow_id, _) = make_workflow_in(&pool, community).await;
    let key = [4_u8; 32];
    let payload = [6_u8; 32];
    let first = begin_workflow_admission(&pool, community, workflow_id, &key, &payload)
        .await
        .expect("begin first admission");
    let BeginWorkflowAdmission::Vacant(guard) = first else {
        panic!("first admission must be vacant");
    };
    drop(guard);

    let retry = begin_workflow_admission(&pool, community, workflow_id, &key, &payload)
        .await
        .expect("retry admission after rollback");
    let BeginWorkflowAdmission::Vacant(retry_guard) = retry else {
        panic!("rollback must leave the key vacant");
    };
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM workflow_runs WHERE community_id = $1 AND workflow_id = $2",
    )
    .bind(community.as_uuid())
    .bind(workflow_id)
    .fetch_one(&pool)
    .await
    .expect("count workflow runs");
    assert_eq!(count, 0);
    drop(retry_guard);
}

#[tokio::test]
#[ignore = "requires Postgres"]
async fn admission_identity_columns_are_immutable_after_insert() {
    let pool = setup_pool().await;
    let community = make_community(&pool).await;
    let (workflow_id, _) = make_workflow_in(&pool, community).await;
    let first = begin_workflow_admission(
        &pool,
        community,
        workflow_id,
        &[5_u8; 32],
        &[5_u8; 32],
    )
    .await
    .expect("begin admission");
    let BeginWorkflowAdmission::Vacant(guard) = first else {
        panic!("first admission must be vacant");
    };
    let snapshot = WorkflowRouteSnapshot {
        community_id: *community.as_uuid(),
        repository_coordinate: format!("30617:{}:agentic-os-plan", "ab".repeat(32)),
        project_coordinate: format!("30621:{}:gigo-harness", "ab".repeat(32)),
        channel_id: Uuid::new_v4(),
        matched_identity_tier: "d_tag".into(),
    };
    let run = guard
        .accept(&serde_json::json!({"webhook_fields": {}}), &snapshot)
        .await
        .expect("persist accepted admission");

    let hash_error = sqlx::query(
        "UPDATE workflow_runs SET idempotency_key_hash = $1 WHERE community_id = $2 AND id = $3",
    )
    .bind([9_u8; 32].as_slice())
    .bind(community.as_uuid())
    .bind(run.id)
    .execute(&pool)
    .await
    .expect_err("idempotency hash update must fail");
    assert!(hash_error.to_string().contains("admission identity is immutable"));

    let payload_error = sqlx::query(
        "UPDATE workflow_runs SET payload_hash = $1 WHERE community_id = $2 AND id = $3",
    )
    .bind([8_u8; 32].as_slice())
    .bind(community.as_uuid())
    .bind(run.id)
    .execute(&pool)
    .await
    .expect_err("payload hash update must fail");
    assert!(payload_error.to_string().contains("admission identity is immutable"));

    let route_error = sqlx::query(
        "UPDATE workflow_runs SET route_snapshot = $1 WHERE community_id = $2 AND id = $3",
    )
    .bind(serde_json::json!({"tampered": true}))
    .bind(community.as_uuid())
    .bind(run.id)
    .execute(&pool)
    .await
    .expect_err("route snapshot update must fail");
    assert!(route_error.to_string().contains("admission identity is immutable"));
}
```

- [ ] **Step 3: Run the tests red**

```bash
. ./bin/activate-hermit && cargo test -p buzz-db --lib workflow_admission::tests::route_snapshot_round_trips_all_server_fields -- --exact --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib workflow::tests::admission_ -- --ignored --nocapture
```

Expected: FAIL because the snapshot and admission APIs do not exist.

- [ ] **Step 4: Add the migration and checked-in schema changes**

Create `migrations/0032_workflow_run_route_idempotency.sql` with exactly this structure:

```sql
-- Immutable webhook route snapshot and control-plane idempotency hashes.
-- Raw idempotency keys are never stored; NULL hashes keep non-dynamic runs unrestricted.
SET LOCAL lock_timeout = '5s';

ALTER TABLE workflow_runs
    ADD COLUMN idempotency_key_hash BYTEA,
    ADD COLUMN payload_hash BYTEA,
    ADD COLUMN route_snapshot JSONB;

ALTER TABLE workflow_runs
    ADD CONSTRAINT workflow_runs_idempotency_key_hash_len
        CHECK (idempotency_key_hash IS NULL OR octet_length(idempotency_key_hash) = 32),
    ADD CONSTRAINT workflow_runs_payload_hash_len
        CHECK (payload_hash IS NULL OR octet_length(payload_hash) = 32),
    ADD CONSTRAINT workflow_runs_hash_pair
        CHECK ((idempotency_key_hash IS NULL) = (payload_hash IS NULL)),
    ADD CONSTRAINT workflow_runs_route_requires_hashes
        CHECK (route_snapshot IS NULL OR idempotency_key_hash IS NOT NULL);

CREATE UNIQUE INDEX idx_workflow_runs_idempotency
    ON workflow_runs (community_id, workflow_id, idempotency_key_hash)
    WHERE idempotency_key_hash IS NOT NULL;

CREATE FUNCTION prevent_workflow_run_admission_identity_update()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.idempotency_key_hash IS DISTINCT FROM OLD.idempotency_key_hash
       OR NEW.payload_hash IS DISTINCT FROM OLD.payload_hash
       OR NEW.route_snapshot IS DISTINCT FROM OLD.route_snapshot
    THEN
        RAISE EXCEPTION 'workflow run admission identity is immutable'
            USING ERRCODE = 'integrity_constraint_violation';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER workflow_run_admission_identity_guard
BEFORE UPDATE OF idempotency_key_hash, payload_hash, route_snapshot ON workflow_runs
FOR EACH ROW
EXECUTE FUNCTION prevent_workflow_run_admission_identity_update();
```

Mirror the three columns, four checks, partial unique index, trigger function, and trigger in `schema/schema.sql`.

- [ ] **Step 5: Implement snapshot decoding and the serialized admission guard**

Derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `PartialEq`, and `Eq` on `WorkflowRouteSnapshot`.
Document every public field.
Validate `matched_identity_tier` against `d_tag`, `alias`, `clone_basename`, and `display_name` when decoding a row; map malformed stored JSON to `DbError::InvalidData`.

Extend `WorkflowRunRecord` with the three optional fields and update both `get_workflow_run` and `list_workflow_runs_page` to select them.
Make `row_to_run_record` `pub(crate)` and deserialize a non-null route snapshot through the validation above.
Update all five existing `WorkflowRunRecord` literals in `workflow.rs` tests with `idempotency_key_hash: None`, `payload_hash: None`, and `route_snapshot: None` so the static record contract remains explicit and the crate compiles.
Leave `create_workflow_run` unchanged so it inserts NULL for all three columns.

`begin_workflow_admission` must:

1. Begin a transaction.
2. Make its first SQL statement `SELECT pg_advisory_xact_lock(hashtextextended($1, 0))` with the lock text `buzz_workflow_admission:<community_uuid>:<workflow_uuid>:<lowercase_hex_key_hash>`.
3. Select an existing row by `(community_id, workflow_id, idempotency_key_hash)` inside that transaction.
4. Commit and return `Existing` when its payload hash matches.
5. Commit and return `PayloadConflict` when its payload hash differs.
6. Return `Vacant(WorkflowAdmissionGuard)` without committing when no row exists.

The guard's private finalizer must insert exactly one row with `trigger_event_id = NULL`, `started_at = NULL`, status `pending`, and a non-null route snapshot for `accept`, or status `failed`, NULL route snapshot, stable error code, redacted error message, and `completed_at = NOW()` for `reject`.
Both paths store sanitized trigger context, both hashes, and an empty execution trace.
The finalizer consumes the guard, returns the inserted `WorkflowRunRecord`, and commits only after the insert succeeds.
Dropping the guard before finalization must rely on SQLx transaction rollback and must insert nothing.

Declare `pub mod workflow_admission;` in `crates/buzz-db/src/lib.rs`.
Add `Db::begin_workflow_admission` with a datastore span of the same name.

- [ ] **Step 6: Apply migration and run the Task 2 tests green**

```bash
. ./bin/activate-hermit && just setup
. ./bin/activate-hermit && cargo test -p buzz-db --lib workflow_admission::tests::route_snapshot_round_trips_all_server_fields -- --exact --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib workflow::tests::admission_ -- --ignored --nocapture
```

Expected: PASS, including the lock wait, rollback, conflict, and immutability tests.

- [ ] **Step 7: Commit**

```bash
. ./bin/activate-hermit && git add migrations/0032_workflow_run_route_idempotency.sql schema/schema.sql crates/buzz-db/src/workflow_admission.rs crates/buzz-db/src/workflow.rs crates/buzz-db/src/lib.rs
```

Run staged `detect_changes` and confirm only workflow-run persistence and admission flows are reported.

```bash
. ./bin/activate-hermit && git commit -s -m "feat(db): serialize idempotent workflow admission"
```

---

### Task 3: Unlimited latest live repository and project heads

**Files:**

- Create: `crates/buzz-db/src/project_heads.rs`
- Modify: `crates/buzz-db/src/workflow_admission.rs`
- Modify: `crates/buzz-db/src/lib.rs`

**Interfaces:**

- Consumes: `CommunityId`, `StoredEvent`, `row_to_stored_event`, kinds `30617` and `30621`.
- Produces:
  - `pub async fn list_latest_parameterized_heads(pool, community_id, kind: i32) -> Result<Vec<StoredEvent>>`
  - `pub async fn get_latest_parameterized_head(pool, community_id, kind: i32, pubkey: &[u8], d_tag: &str) -> Result<Option<StoredEvent>>`
  - `pub struct WorkflowAdmissionChannel { pub id: Uuid, pub archived_at: Option<DateTime<Utc>> }`
  - `WorkflowAdmissionGuard::list_latest_parameterized_heads(&mut self, kind: i32)` using the guard's transaction
  - `WorkflowAdmissionGuard::load_destination_channel(&mut self, channel_id: Uuid)` scoped to the guard's community and excluding soft-deleted rows
  - `WorkflowAdmissionGuard::destination_member_role(&mut self, channel_id: Uuid, pubkey: &[u8])` scoped to active membership in the guard's community
- No `LIMIT` clause on the list query.
- Latest head ordering is `created_at DESC, id ASC` via `DISTINCT ON (pubkey, d_tag)`.
- Live means `deleted_at IS NULL`.
- Scope to `channel_id IS NULL` because these kinds are global-only.

**Acceptance:** Both ordinary and admission-guard queries return only latest live global heads in the requested community, enumerate more than 1,000 coordinates without truncation, preserve exact `d`-tag case, and cannot resolve a destination channel or membership that exists only in another community.

- [ ] **Step 1: Write failing ignored Postgres tests**

In `project_heads.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::insert_event;
    use nostr::{EventBuilder, Keys, Kind, Tag};
    use sqlx::PgPool;
    use uuid::Uuid;

    const TEST_DB_URL: &str = "postgres://buzz:buzz_dev@localhost:5432/buzz"; // sadscan:disable np.postgres.1

    async fn setup_pool() -> PgPool {
        let database_url = std::env::var("BUZZ_TEST_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| TEST_DB_URL.to_owned());
        PgPool::connect(&database_url)
            .await
            .expect("connect to test DB")
    }

    async fn make_community(pool: &PgPool) -> CommunityId {
        let id = Uuid::new_v4();
        let host = format!("heads-{}.example", id.simple());
        sqlx::query("INSERT INTO communities (id, host) VALUES ($1, $2)")
            .bind(id)
            .bind(&host)
            .execute(pool)
            .await
            .expect("insert community");
        CommunityId::from_uuid(id)
    }

    fn parameterized_event(
        kind: u16,
        keys: &Keys,
        d: &str,
        name: &str,
        created_at: u64,
    ) -> nostr::Event {
        EventBuilder::new(Kind::from(kind), "")
            .tags(vec![
                Tag::parse(["d", d]).unwrap(),
                Tag::parse(["name", name]).unwrap(),
            ])
            .custom_created_at(nostr::Timestamp::from(created_at))
            .sign_with_keys(keys)
            .unwrap()
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn list_heads_returns_latest_per_coordinate_without_limit() {
        let pool = setup_pool().await;
        let community = make_community(&pool).await;
        let keys_a = Keys::generate();
        let keys_b = Keys::generate();
        let keys_c = Keys::generate();
        let base = chrono::Utc::now().timestamp() as u64;
        let older = parameterized_event(30617, &keys_a, "agentic-os-plan", "old", base);
        let newer = parameterized_event(30617, &keys_a, "agentic-os-plan", "new", base + 5);
        let harness = parameterized_event(30617, &keys_b, "harness-service", "harness-service", base + 1);
        let extra = parameterized_event(30617, &keys_c, "extra-repo", "extra-repo", base + 2);
        insert_event(&pool, community, &older, None).await.expect("older");
        insert_event(&pool, community, &newer, None).await.expect("newer");
        insert_event(&pool, community, &harness, None).await.expect("harness");
        insert_event(&pool, community, &extra, None).await.expect("extra");

        let heads = list_latest_parameterized_heads(&pool, community, 30617)
            .await
            .expect("list heads");
        let named: Vec<String> = heads
            .iter()
            .filter_map(|h| {
                h.event.tags.iter().find_map(|tag| {
                    let parts = tag.as_slice();
                    (parts.first().map(String::as_str) == Some("name"))
                        .then(|| parts.get(1).cloned())
                        .flatten()
                })
            })
            .collect();
        assert_eq!(heads.len(), 3, "three live coordinates, no page limit");
        assert!(named.contains(&"new".to_string()));
        assert!(!named.contains(&"old".to_string()));
        assert!(named.contains(&"harness-service".to_string()));
        assert!(named.contains(&"extra-repo".to_string()));
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn list_heads_returns_more_than_default_query_page_limit() {
        let pool = setup_pool().await;
        for kind in [30617_u16, 30621_u16] {
            let community = make_community(&pool).await;
            let keys = Keys::generate();
            for index in 0..=crate::event::DEFAULT_MAX_PAGE_LIMIT {
                let d = format!("coordinate-{kind}-{index}");
                let event = parameterized_event(
                    kind,
                    &keys,
                    &d,
                    &d,
                    chrono::Utc::now().timestamp() as u64,
                );
                insert_event(&pool, community, &event, None)
                    .await
                    .expect("insert parameterized head");
            }
            let heads = list_latest_parameterized_heads(&pool, community, i32::from(kind))
                .await
                .expect("list all heads");
            assert_eq!(
                heads.len(),
                crate::event::DEFAULT_MAX_PAGE_LIMIT as usize + 1,
                "kind {kind} must not be truncated",
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn list_heads_excludes_deleted_rows() {
        let pool = setup_pool().await;
        let community = make_community(&pool).await;
        let keys = Keys::generate();
        let event = parameterized_event(30617, &keys, "agentic-os-plan", "agentic-os-plan", chrono::Utc::now().timestamp() as u64);
        insert_event(&pool, community, &event, None).await.expect("insert");
        sqlx::query("UPDATE events SET deleted_at = NOW() WHERE community_id = $1 AND id = $2")
            .bind(community.as_uuid())
            .bind(event.id.as_bytes().as_slice())
            .execute(&pool)
            .await
            .expect("soft-delete");
        let heads = list_latest_parameterized_heads(&pool, community, 30617)
            .await
            .expect("list heads");
        assert!(heads.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn get_head_is_case_sensitive_on_d_tag() {
        let pool = setup_pool().await;
        let community = make_community(&pool).await;
        let keys = Keys::generate();
        let event = parameterized_event(30617, &keys, "agentic-os-plan", "agentic-os-plan", chrono::Utc::now().timestamp() as u64);
        insert_event(&pool, community, &event, None).await.expect("insert");
        let pubkey = keys.public_key().to_bytes();
        let found = get_latest_parameterized_head(&pool, community, 30617, &pubkey, "agentic-os-plan")
            .await
            .expect("lookup");
        assert!(found.is_some());
        let missing = get_latest_parameterized_head(&pool, community, 30617, &pubkey, "Agentic-os-plan")
            .await
            .expect("case mismatch");
        assert!(missing.is_none());
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn admission_destination_reads_do_not_cross_community() {
        let pool = setup_pool().await;
        let db = crate::Db::from_pool(pool.clone());
        let community_a = make_community(&pool).await;
        let community_b = make_community(&pool).await;
        let (workflow_id, _) = crate::workflow::tests::make_workflow_in(&pool, community_a).await;
        let owner = Keys::generate();
        let owner_bytes = owner.public_key().to_bytes();
        db.ensure_user(community_b, &owner_bytes)
            .await
            .expect("ensure owner in community B");
        let shared_channel_id = Uuid::new_v4();
        db.create_channel_with_id(
            community_b,
            shared_channel_id,
            "only-in-b",
            crate::channel::ChannelType::Stream,
            crate::channel::ChannelVisibility::Open,
            None,
            &owner_bytes,
            None,
        )
        .await
        .expect("create channel in community B");

        let begin = crate::workflow_admission::begin_workflow_admission(
            &pool,
            community_a,
            workflow_id,
            &[1_u8; 32],
            &[2_u8; 32],
        )
        .await
        .expect("begin admission in community A");
        let crate::workflow_admission::BeginWorkflowAdmission::Vacant(mut guard) = begin else {
            panic!("fresh admission must be vacant");
        };
        assert!(
            guard
                .load_destination_channel(shared_channel_id)
                .await
                .expect("load destination")
                .is_none()
        );
        assert!(
            guard
                .destination_member_role(shared_channel_id, &owner_bytes)
                .await
                .expect("load destination role")
                .is_none()
        );
    }
}
```

- [ ] **Step 2: Run tests and confirm they fail**

```bash
. ./bin/activate-hermit && cargo test -p buzz-db --lib project_heads -- --ignored --nocapture
```

Expected: FAIL on missing module/function.

- [ ] **Step 3: Implement the queries**

```sql
SELECT DISTINCT ON (pubkey, d_tag)
    id, pubkey, created_at, kind, tags, content, sig, received_at, channel_id
FROM events
WHERE community_id = $1
  AND kind = $2
  AND deleted_at IS NULL
  AND channel_id IS NULL
  AND d_tag IS NOT NULL
ORDER BY pubkey, d_tag, created_at DESC, id ASC
```

```sql
SELECT id, pubkey, created_at, kind, tags, content, sig, received_at, channel_id
FROM events
WHERE community_id = $1
  AND kind = $2
  AND pubkey = $3
  AND d_tag = $4
  AND deleted_at IS NULL
  AND channel_id IS NULL
ORDER BY created_at DESC, id ASC
LIMIT 1
```

Reuse the existing `pub(crate) event::row_to_stored_event` and flatten its `Result<Option<StoredEvent>>` after `fetch_optional`.
Do not duplicate event reconstruction.

Declare `pub mod project_heads;` in `lib.rs` and wrap both functions on `Db` with datastore spans `list_latest_parameterized_heads` and `get_latest_parameterized_head`.

Add private `_on` variants that accept `&mut PgConnection` and use them from both the pool wrappers and `WorkflowAdmissionGuard`.
Implement the guard's destination-channel and member-role reads on its own transaction so admission does not need a second pool connection while it holds the advisory lock.
The destination-channel read returns only `id` and `archived_at` from `channels WHERE community_id = $1 AND id = $2 AND deleted_at IS NULL`.
The membership read uses the same active-row predicates as `channel::get_member_role`: matching community and channel, `removed_at IS NULL`, and a join to a non-deleted channel.

Do not call `query_events` here.
`EventQuery` applies `DEFAULT_MAX_PAGE_LIMIT` of 1000, which violates NIP-MP completeness.

- [ ] **Step 4: Run tests green and commit**

```bash
. ./bin/activate-hermit && cargo test -p buzz-db --lib project_heads -- --ignored --nocapture
```

```bash
. ./bin/activate-hermit && git add crates/buzz-db/src/project_heads.rs crates/buzz-db/src/workflow_admission.rs crates/buzz-db/src/lib.rs && git commit -s -m "feat(db): list latest live repository and project heads"
```

---

### Task 4: Identity resolver and claim authorization

**Files:**

- Create: `crates/buzz-relay/src/workflow_route.rs`
- Modify: `crates/buzz-relay/src/lib.rs`

**Interfaces:**

- Consumes: `RoutingDef` aliases, `StoredEvent` heads, `ProjectMemberCoord` parse rules, `RouteFailure`.
- Produces:
  - `pub struct RepositoryHead { pub coordinate: String, pub d_tag: String, pub name: Option<String>, pub clone_basenames: Vec<String>, pub owner_hex: String, pub maintainers: Vec<String> }`
  - `pub struct ProjectHead { pub coordinate: String, pub signer_hex: String, pub member_coordinates: Vec<String>, pub buzz_channel: Option<String>, pub listed: bool }`
  - `pub enum IdentityTier { DTag, Alias, CloneBasename, DisplayName }` with `as_str()` values `d_tag` / `alias` / `clone_basename` / `display_name`
  - `pub fn clone_basename(value: &str) -> Option<String>`
  - `pub fn repository_head_from_event(event: &nostr::Event) -> Option<RepositoryHead>`
  - `pub fn project_head_from_event(event: &nostr::Event) -> Option<ProjectHead>`
  - `pub fn resolve_repository_identity(repository_name: &str, aliases: &BTreeMap<String, String>, heads: &[RepositoryHead]) -> Result<(String, IdentityTier), RouteFailure>`
  - `pub fn claim_valid_projects<'a>(repository: &RepositoryHead, projects: &'a [ProjectHead]) -> Vec<&'a ProjectHead>`
  - `pub fn authorize_unique_project_route<'a>(repository: &RepositoryHead, projects: &'a [ProjectHead]) -> Result<&'a ProjectHead, RouteFailure>` counting claim-valid projects before inspecting channel quality

**Acceptance:** Resolution uses the approved exact tier order, collapses duplicate observations of one coordinate, stops on the first matching tier, preserves case and Unicode bytes, and counts only live listed owner-or-maintainer project claims before inspecting channel metadata.

- [ ] **Step 1: Write failing unit tests first**

These tests must not use Postgres.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> String { "ab".repeat(32) }
    fn other() -> String { "cd".repeat(32) }
    fn coord(owner: &str, d: &str) -> String { format!("30617:{owner}:{d}") }
    fn repo(d: &str, owner: &str) -> RepositoryHead {
        RepositoryHead {
            coordinate: coord(owner, d),
            d_tag: d.to_string(),
            name: Some(d.to_string()),
            clone_basenames: vec![d.to_string()],
            owner_hex: owner.to_string(),
            maintainers: vec![],
        }
    }

    #[test]
    fn d_tag_unique_stops_before_alias() {
        let owner = owner();
        let heads = vec![repo("agentic-os-plan", &owner)];
        let mut aliases = BTreeMap::new();
        aliases.insert(
            "agentic-os-plan".into(),
            coord(&other(), "other-repo"),
        );
        let (resolved, tier) = resolve_repository_identity("agentic-os-plan", &aliases, &heads)
            .expect("unique d tag");
        assert_eq!(resolved, coord(&owner, "agentic-os-plan"));
        assert_eq!(tier, IdentityTier::DTag);
    }

    #[test]
    fn d_tag_ambiguous_does_not_consult_weaker_tiers() {
        let heads = vec![
            repo("agentic-os-plan", &owner()),
            repo("agentic-os-plan", &other()),
        ];
        let err = resolve_repository_identity("agentic-os-plan", &BTreeMap::new(), &heads)
            .unwrap_err();
        assert_eq!(err, RouteFailure::RepositoryAmbiguous);
    }

    #[test]
    fn comparison_is_case_sensitive_and_untrimmed() {
        let heads = vec![repo("agentic-os-plan", &owner())];
        assert_eq!(
            resolve_repository_identity("Agentic-os-plan", &BTreeMap::new(), &heads).unwrap_err(),
            RouteFailure::RepositoryMissing
        );
        assert_eq!(
            resolve_repository_identity("agentic-os-plan ", &BTreeMap::new(), &heads).unwrap_err(),
            RouteFailure::RepositoryMissing
        );
    }

    #[test]
    fn clone_basename_strips_exactly_one_git_suffix() {
        assert_eq!(
            clone_basename("https://github.com/acme/agentic-os-plan.git").as_deref(),
            Some("agentic-os-plan")
        );
        assert_eq!(
            clone_basename("https://github.com/acme/agentic-os-plan.git.git").as_deref(),
            Some("agentic-os-plan.git")
        );
        assert_eq!(clone_basename("https://github.com/acme/agentic-os-plan.git/"), None);
        assert_eq!(clone_basename("https://github.com/"), None);
    }

    #[test]
    fn alias_unique_coordinate_does_not_grant_authority() {
        let target = coord(&owner(), "harness-service");
        let heads = vec![repo("harness-service", &owner())];
        let mut aliases = BTreeMap::new();
        aliases.insert("hs".into(), target.clone());
        let (resolved, tier) =
            resolve_repository_identity("hs", &aliases, &heads).expect("alias");
        assert_eq!(resolved, target);
        assert_eq!(tier, IdentityTier::Alias);
    }

    #[test]
    fn alias_target_without_live_head_is_unavailable() {
        let mut aliases = BTreeMap::new();
        aliases.insert("hs".into(), coord(&owner(), "missing"));
        let err = resolve_repository_identity("hs", &aliases, &[]).unwrap_err();
        assert_eq!(err, RouteFailure::AliasTargetUnavailable);
    }

    #[test]
    fn clone_and_display_name_are_used_only_after_stronger_tiers_miss() {
        let mut by_clone = repo("canonical-d", &owner());
        by_clone.clone_basenames = vec!["agentic-os-plan".into()];
        by_clone.name = Some("Plan Display".into());
        let heads = vec![by_clone];
        let (clone_coord, clone_tier) = resolve_repository_identity(
            "agentic-os-plan",
            &BTreeMap::new(),
            &heads,
        )
        .expect("clone tier resolves");
        assert_eq!(clone_coord, coord(&owner(), "canonical-d"));
        assert_eq!(clone_tier, IdentityTier::CloneBasename);
        let (_, display_tier) = resolve_repository_identity(
            "Plan Display",
            &BTreeMap::new(),
            &heads,
        )
        .expect("display tier resolves");
        assert_eq!(display_tier, IdentityTier::DisplayName);
    }

    #[test]
    fn duplicate_observations_of_one_coordinate_are_not_ambiguous() {
        let same = repo("agentic-os-plan", &owner());
        let (resolved, tier) = resolve_repository_identity(
            "agentic-os-plan",
            &BTreeMap::new(),
            &[same.clone(), same],
        )
        .expect("one distinct coordinate");
        assert_eq!(resolved, coord(&owner(), "agentic-os-plan"));
        assert_eq!(tier, IdentityTier::DTag);
    }

    #[test]
    fn unicode_is_not_normalized_between_tiers() {
        let mut head = repo("canonical", &owner());
        head.name = Some("Caf\u{00e9}".into());
        assert_eq!(
            resolve_repository_identity("Cafe\u{0301}", &BTreeMap::new(), &[head])
                .unwrap_err(),
            RouteFailure::RepositoryMissing
        );
    }

    #[test]
    fn two_claim_valid_projects_are_ambiguous() {
        let repository = repo("agentic-os-plan", &owner());
        let projects = vec![
            ProjectHead {
                coordinate: format!("30621:{}:gigo-harness", owner()),
                signer_hex: owner(),
                member_coordinates: vec![repository.coordinate.clone()],
                buzz_channel: Some("9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50".into()),
                listed: true,
            },
            ProjectHead {
                coordinate: format!("30621:{}:other", other()),
                signer_hex: owner(),
                member_coordinates: vec![repository.coordinate.clone()],
                buzz_channel: Some("11111111-1111-1111-1111-111111111111".into()),
                listed: true,
            },
        ];
        assert_eq!(
            authorize_unique_project_route(&repository, &projects).unwrap_err(),
            RouteFailure::ProjectAmbiguous
        );
    }

    #[test]
    fn unlisted_or_unauthorized_projects_do_not_count() {
        let repository = repo("agentic-os-plan", &owner());
        let projects = vec![
            ProjectHead {
                coordinate: format!("30621:{}:hidden", other()),
                signer_hex: other(),
                member_coordinates: vec![repository.coordinate.clone()],
                buzz_channel: Some("9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50".into()),
                listed: false,
            },
            ProjectHead {
                coordinate: format!("30621:{}:stranger", other()),
                signer_hex: other(),
                member_coordinates: vec![repository.coordinate.clone()],
                buzz_channel: Some("9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50".into()),
                listed: true,
            },
        ];
        assert_eq!(
            authorize_unique_project_route(&repository, &projects).unwrap_err(),
            RouteFailure::ProjectMissing
        );
    }

    #[test]
    fn maintainer_signer_counts_as_claim_valid() {
        let mut repository = repo("agentic-os-plan", &owner());
        repository.maintainers = vec![other()];
        let projects = vec![ProjectHead {
            coordinate: format!("30621:{}:gigo-harness", other()),
            signer_hex: other(),
            member_coordinates: vec![repository.coordinate.clone()],
            buzz_channel: Some("9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50".into()),
            listed: true,
        }];
        authorize_unique_project_route(&repository, &projects).expect("maintainer claim");
    }

    #[test]
    fn channel_quality_is_ignored_until_exactly_one_claim_valid_project() {
        let repository = repo("agentic-os-plan", &owner());
        let projects = vec![
            ProjectHead {
                coordinate: format!("30621:{}:a", owner()),
                signer_hex: owner(),
                member_coordinates: vec![repository.coordinate.clone()],
                buzz_channel: None,
                listed: true,
            },
            ProjectHead {
                coordinate: format!("30621:{}:b", owner()),
                signer_hex: owner(),
                member_coordinates: vec![repository.coordinate.clone()],
                buzz_channel: Some("9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50".into()),
                listed: true,
            },
        ];
        assert_eq!(
            authorize_unique_project_route(&repository, &projects).unwrap_err(),
            RouteFailure::ProjectAmbiguous
        );
    }
}
```

Also add this event-parsing coverage and a test that heads named `agentic-os-plan` and `harness-service` each resolve uniquely when they have distinct coordinates:

```rust
    #[test]
    fn repository_event_reads_first_name_and_all_clone_and_maintainer_values() {
        let keys = nostr::Keys::generate();
        let maintainer_a = "cd".repeat(32);
        let maintainer_b = "ef".repeat(32);
        let event = nostr::EventBuilder::new(nostr::Kind::Custom(30617), "")
            .tags(vec![
                nostr::Tag::parse(["d", "canonical-d"]).unwrap(),
                nostr::Tag::parse(["name", "First Name"]).unwrap(),
                nostr::Tag::parse(["name", "Second Name"]).unwrap(),
                nostr::Tag::parse([
                    "clone",
                    "https://github.com/acme/agentic-os-plan.git",
                    "ssh://git.example/harness-service.git",
                ])
                .unwrap(),
                nostr::Tag::parse([
                    "maintainers",
                    maintainer_a.as_str(),
                    maintainer_b.as_str(),
                ])
                .unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();
        let head = repository_head_from_event(&event).expect("repository head");
        assert_eq!(head.name.as_deref(), Some("First Name"));
        assert_eq!(
            head.clone_basenames,
            vec!["agentic-os-plan".to_string(), "harness-service".to_string()]
        );
        assert_eq!(head.maintainers, vec![maintainer_a, maintainer_b]);
    }

    #[test]
    fn project_event_visibility_and_channel_parse_fail_closed() {
        let keys = nostr::Keys::generate();
        let malformed = nostr::EventBuilder::new(nostr::Kind::Custom(30621), "")
            .tags(vec![
                nostr::Tag::parse(["d", "gigo-harness"]).unwrap(),
                nostr::Tag::parse(["buzz-visibility", "unexpected"]).unwrap(),
                nostr::Tag::parse(["buzz-channel", "first"]).unwrap(),
                nostr::Tag::parse(["buzz-channel", "second"]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();
        let parsed = project_head_from_event(&malformed).expect("project head");
        assert!(parsed.listed, "unknown visibility is listed");
        assert!(parsed.buzz_channel.is_none(), "duplicate channel is invalid");

        let unlisted = nostr::EventBuilder::new(nostr::Kind::Custom(30621), "")
            .tags(vec![
                nostr::Tag::parse(["d", "hidden"]).unwrap(),
                nostr::Tag::parse(["buzz-visibility", "unlisted"]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();
        assert!(!project_head_from_event(&unlisted).unwrap().listed);
    }
```

- [ ] **Step 2: Run tests and confirm they fail**

```bash
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_route -- --nocapture
```

Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement the pure resolver**

`resolve_repository_identity` must apply tiers in order:

1. Collect heads whose `d_tag == repository_name`.
2. If the set of coordinates has length 1, return that coordinate and `DTag`.
3. If length > 1, return `RepositoryAmbiguous`.
4. If the alias map contains `repository_name` as an exact key, parse the target, require a head with that exact coordinate, else `AliasTargetUnavailable`.
5. Collect heads whose `clone_basenames` contain `repository_name` exactly.
6. Same unique/ambiguous/missing rules with `CloneBasename`.
7. Collect heads whose `name` equals `repository_name`.
8. Same rules with `DisplayName`.
9. Otherwise `RepositoryMissing`.

`clone_basename`:

```rust
pub fn clone_basename(value: &str) -> Option<String> {
    let segment = if let Ok(url) = url::Url::parse(value) {
        let mut segments = url.path_segments()?;
        let last = segments.next_back()?;
        if last.is_empty() {
            return None;
        }
        last.to_string()
    } else if value.contains('/') {
        let last = value.rsplit('/').next().unwrap_or("");
        if last.is_empty() {
            return None;
        }
        last.to_string()
    } else {
        return None;
    };
    let stripped = segment.strip_suffix(".git").unwrap_or(&segment);
    if stripped.is_empty() {
        None
    } else {
        Some(stripped.to_string())
    }
}
```

Do not trim `value`.
Do not lowercase.

`repository_head_from_event` reads tags with `tag.as_slice()` and uses the first `d` value, matching the database's existing `extract_d_tag` replacement key.
The first `d` value must be non-empty.
`name` is the first `name` value.
Clone basenames come from every value after every `clone` tag name.
Maintainers come from every value after every `maintainers` tag name.
Owner is `event.pubkey.to_hex()`.
Coordinate is `format!("30617:{owner}:{d}")`.

`project_head_from_event` requires exactly one non-empty `d`, matching the enforced NIP-MP envelope.
Members are `a` tag values that `ProjectMemberCoord::parse_full` accepts.
`listed` is false only when the first `buzz-visibility` value is exactly `unlisted`.
Set `buzz_channel` only when exactly one `buzz-channel` tag exists and it has exactly two elements; otherwise leave it `None` so routing fails closed on malformed stored data.

`claim_valid_projects` keeps listed projects whose `member_coordinates` contain the repository coordinate exactly and whose `signer_hex` equals `owner_hex` or is present in `maintainers`.

`authorize_unique_project_route` counts that list and returns `ProjectMissing` for 0, `ProjectAmbiguous` for >1, and the unique project for 1.
It must not inspect `buzz_channel` while counting.

Declare `pub mod workflow_route;` in `crates/buzz-relay/src/lib.rs`.

- [ ] **Step 4: Run tests green and commit**

```bash
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_route -- --nocapture
```

```bash
. ./bin/activate-hermit && git add crates/buzz-relay/src/workflow_route.rs crates/buzz-relay/src/lib.rs && git commit -s -m "feat(relay): resolve webhook repository identity and project claims"
```

---

### Task 5: Save-time alias live-head validation

**Files:**

- Modify: `crates/buzz-relay/src/handlers/command_executor.rs` in `handle_workflow_def`
- Create: `crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs`

**Interfaces:**

- Consumes: `WorkflowDef::has_project_channel_routing`, typed `parse_repository_coordinate`, and `Db::get_latest_parameterized_head`.
- Produces: ingest rejection `invalid: routing alias '<key>' does not name a live repository in this community` when a target is missing.
- Produces: test-only `WorkflowSaveResult { accepted, message, workflow_id, webhook_secret }` and `define_webhook_workflow_raw(keys: &Keys, home_channel: Uuid, yaml: &str) -> WorkflowSaveResult`, where the two response fields are `None` on rejection.
- Does not grant claim or channel authority at save time.

**Acceptance:** An authenticated workflow owner can save aliases only when every target is a latest live `30617` head in the host community; a missing target rejects the save before secret injection without revealing its coordinate or any candidate list.

- [ ] **Step 1: Run impact for `handle_workflow_def`**

Report callers.
This is the workflow YAML save path.
Do not change webhook-secret injection order.

- [ ] **Step 2: Write the failing relay-backed alias-save test**

Create the E2E file with the shared `relay_url`, `relay_http_url`, `unique`, `create_channel`, `submit_event`, and `define_webhook_workflow_raw` helpers described in Task 8.
The workflow definition helper must generate a client workflow UUID, include it as the kind `30620` `d` tag, include the home channel as the `h` tag, and parse the JSON suffix after `response:` only for an accepted save.
Its raw result type and parser are exact:

```rust
struct WorkflowSaveResult {
    accepted: bool,
    message: String,
    workflow_id: Option<Uuid>,
    webhook_secret: Option<String>,
}
```

Implement `async fn define_webhook_workflow_raw(keys: &Keys, home_channel: Uuid, yaml: &str) -> WorkflowSaveResult` by publishing the client-UUID kind `30620` event through the authenticated test client.
Copy `accepted` and `message` from the publish result, strip `response:` and decode `workflow_id` plus `webhook_secret` only when `accepted` is true, and leave both optional fields `None` otherwise.

Add this test before editing `handle_workflow_def`:

```rust
#[tokio::test]
#[ignore = "requires a running relay"]
async fn alias_target_must_be_live_at_save() {
    let keys = nostr::Keys::generate();
    let home_channel = create_channel(&keys, "alias-home").await;
    let alias_key = "hs";
    let missing_target = format!(
        "30617:{}:missing-harness-service",
        keys.public_key().to_hex()
    );
    let yaml = format!(
        "name: Alias Save Rejection\n\
         trigger:\n  on: webhook\n\
         routing:\n  mode: project_channel_by_repository\n\
         \x20 aliases:\n    {alias_key}: {missing_target}\n\
         steps:\n  - id: notify\n    action: send_message\n    text: hi\n"
    );
    let result = define_webhook_workflow_raw(&keys, home_channel, &yaml).await;
    assert!(!result.accepted);
    assert!(result.workflow_id.is_none());
    assert!(result.webhook_secret.is_none());
    assert!(result.message.contains(alias_key));
    assert!(!result.message.contains("30617:"));
    assert!(!result.message.contains("missing-harness-service"));
}
```

Build and start the unchanged relay, run this exact test, and confirm it fails because the save is currently accepted:

```bash
. ./bin/activate-hermit && just setup
. ./bin/activate-hermit && cargo build --release -p buzz-relay
# In a separate Hermit-activated terminal: ./target/release/buzz-relay
. ./bin/activate-hermit && cargo test -p buzz-test-client --test e2e_workflow_project_channel_routing alias_target_must_be_live_at_save -- --exact --ignored --nocapture
```

- [ ] **Step 3: Implement after workflow ownership is established and before secret injection**

Keep the existing `get_workflow` owner/channel-conflict check ahead of alias target lookup.
This avoids doing repository existence work for a caller who is not allowed to update the workflow UUID.
Run the alias loop after that check and before `webhook_secret::inject_secret` and definition hashing.

```rust
    if let Some(routing) = def
        .routing
        .as_ref()
        .filter(|_| def.has_project_channel_routing())
    {
        for (alias_key, target) in &routing.aliases {
            let coordinate = buzz_workflow::routing::parse_repository_coordinate(target)
                .map_err(|_| {
                    IngestError::Rejected(
                        "invalid: routing alias target must be a live 30617 coordinate".into(),
                    )
                })?;
            let head = state
                .db
                .get_latest_parameterized_head(
                    tenant.community(),
                    buzz_core::kind::KIND_GIT_REPO_ANNOUNCEMENT as i32,
                    coordinate.owner_pubkey(),
                    coordinate.d_tag(),
                )
                .await
                .map_err(|e| IngestError::Internal(format!("error: alias target lookup: {e}")))?;
            if head.is_none() {
                return Err(IngestError::Rejected(format!(
                    "invalid: routing alias '{alias_key}' does not name a live repository in this community"
                )));
            }
        }
    }
```

Do not list other repositories in the rejection.
A lookup `Err` is `Internal`, not a 404-style generic string, because this is an authenticated owner save.

- [ ] **Step 4: Run focused tests, rebuild the relay, and commit**

```bash
. ./bin/activate-hermit && cargo test -p buzz-relay --lib handlers::command_executor::tests -- --nocapture
. ./bin/activate-hermit && cargo check -p buzz-relay
```

Rebuild the release relay and rerun `alias_target_must_be_live_at_save`; expected: PASS.

```bash
. ./bin/activate-hermit && cargo build --release -p buzz-relay
# Restart the relay in its separate Hermit-activated terminal: ./target/release/buzz-relay
. ./bin/activate-hermit && cargo test -p buzz-test-client --test e2e_workflow_project_channel_routing alias_target_must_be_live_at_save -- --exact --ignored --nocapture
```

```bash
. ./bin/activate-hermit && git add crates/buzz-relay/src/handlers/command_executor.rs crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs && git commit -s -m "feat(relay): reject workflow aliases whose repository targets are not live"
```

---

### Task 6: Webhook admission orchestration, kept dormant until Task 7

**Files:**

- Create: `crates/buzz-relay/src/workflow_admission.rs`
- Modify: `crates/buzz-relay/src/api/mod.rs`
- Modify: `crates/buzz-relay/src/lib.rs`

**Interfaces:**

- Consumes: host binding, `get_workflow`, webhook secret verification, `check_owner_authority`, `BeginWorkflowAdmission`, admission-guard head/channel/member reads, `resolve_repository_identity`, and `authorize_unique_project_route`.
- Produces: `pub(crate) async fn handle_workflow_webhook(state, id_str, query_secret, headers, body)` for Task 7 to call.
- Produces: private `enum ResolveRouteError { Deterministic(RouteFailure), Transient(buzz_db::DbError) }`.
- Produces: private `async fn resolve_dynamic_route(guard: &mut WorkflowAdmissionGuard, community_id: CommunityId, repository_name: &str, routing: &RoutingDef, workflow_owner: &[u8]) -> Result<WorkflowRouteSnapshot, ResolveRouteError>`.
- Produces: private `fn spawn_admitted_run(state: &Arc<AppState>, community_id: CommunityId, run_id: Uuid, definition: Value, trigger_ctx: TriggerContext)` containing the current parse, execute, and finalize task body.
- Produces: private `#[derive(Serialize)] struct AdmissionObservation { run_id: Option<Uuid>, workflow_id: Option<Uuid>, community_id: Option<Uuid>, outcome: &'static str, code: &'static str }`, the only value accepted by the admission logging helper.
- Produces: private `AdmissionObservation::for_run(run: &WorkflowRunRecord, outcome: &'static str, code: &'static str)`, `AdmissionObservation::without_run(workflow_id: Option<Uuid>, community_id: Option<Uuid>, outcome: &'static str, code: &'static str)`, and `record_admission_observation(observation: AdmissionObservation)`.
- Produces: private `fn route_failure_response(failure: RouteFailure) -> (StatusCode, Json<Value>)` using only the stable code, redacted message, and mapped status.
- Produces: HTTP `202` / `400` / `401` / `404` / `409` / `422` / `503` as specified.
- Spawns `execute_from_step` only for a newly created `pending` run.
- Does not modify `api/bridge.rs` or activate dynamic admission in this task.

**Acceptance:** The uncalled handler preserves the existing static parse/authority/response order, implements the dynamic authority and serialized admission order, performs no route lookup or spawn for duplicates, persists deterministic failures once, rolls transient failures back, passes its unit tests, and leaves the live `workflow_webhook` path unchanged until Task 7.

- [ ] **Step 1: Run impact for `workflow_webhook`**

This is the trust boundary.
Keep the existing generic `404` for unknown host, missing workflow, inactive/disabled workflow, and failed home-channel owner authority.

- [ ] **Step 2: Add `api_error_with_code` and failing admission unit tests**

In `api/mod.rs`:

```rust
pub(crate) fn api_error_with_code(
    status: StatusCode,
    msg: &str,
    code: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({ "error": msg, "code": code })),
    )
}
```

In `workflow_admission.rs`, add the exact response helper and cover its mapping from the adjacent test module:

```rust
fn route_failure_response(
    failure: RouteFailure,
) -> (StatusCode, Json<Value>) {
    let status = match failure.http_status() {
        409 => StatusCode::CONFLICT,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    api_error_with_code(
        status,
        failure.redacted_message(),
        failure.code(),
    )
}

#[test]
fn route_failure_conflict_is_409() {
    let (status, Json(body)) =
        route_failure_response(RouteFailure::IdempotencyConflict);
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "idempotency_conflict");
    assert!(body.get("run_id").is_none());
}

#[test]
fn deterministic_route_failure_is_redacted_422_without_run_id() {
    let (status, Json(body)) =
        route_failure_response(RouteFailure::RepositoryMissing);
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["code"], "repository_missing");
    assert_eq!(body["error"], "repository could not be resolved");
    assert!(body.get("run_id").is_none());
}

#[test]
fn admission_strips_key_before_trigger_context() {
    let body = serde_json::json!({
        "repository_name": "agentic-os-plan",
        "idempotency_key": "raw-secret",
        "title": "hello"
    });
    let fields = webhook_fields_from_body(strip_idempotency_key(body.clone()));
    assert!(!fields.contains_key("idempotency_key"));
    assert_eq!(fields.get("repository_name").map(String::as_str), Some("agentic-os-plan"));
    assert_eq!(fields.get("title").map(String::as_str), Some("hello"));
}
```

`webhook_fields_from_body` must copy the current conversion: strings clone, non-strings use `Value::to_string()`.
Add `accepted_run_response(workflow_id, &WorkflowRunRecord)` and test that it emits only `run_id`, `workflow_id`, and the run's current status.
Add `replayed_admission_failure(&WorkflowRunRecord) -> Option<(RouteFailure, (StatusCode, Json<Value>))>` and test that `repository_missing` returns its failure plus canonical redacted `422`, while `route_stale` and ordinary execution failures return `None` because those runs were already admitted.
Add `admission_observation_contains_only_safe_low_cardinality_fields` and serialize an `AdmissionObservation` to prove its field names and values contain no secret, raw idempotency key, callback body, repository candidate list, or project candidate list.

- [ ] **Step 3: Run those tests red, then implement the uncalled admission helper**

Do not edit `api/bridge.rs` in this task.
Implement `pub(crate) async fn handle_workflow_webhook(state: Arc<AppState>, id_str: String, query_secret: Option<String>, headers: HeaderMap, body: axum::body::Bytes) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)>` in `workflow_admission.rs`.

Port the current function body into `handle_workflow_webhook`, use `query_secret` where the bridge currently reads `query.secret`, and split static and dynamic behavior immediately after trigger validation and secret verification.
This deliberate one-task duplication keeps the production route unchanged while the new helper compiles and is unit-tested; Task 7 removes the old body when it activates this helper.
The static branch must retain the current order: parse optional JSON, build current trigger context, check lifecycle and home authority, call unchanged `create_workflow_run`, spawn, and return the current `202` body.
The static branch must not call any routing, hash, admission-lock, repository, project, or dynamic destination API.

The dynamic branch must use this exact order:

1. Bind host to community (already done).
2. Load workflow in that community; generic `404` on miss.
3. Require `TriggerDef::Webhook` (`400` as today if not).
4. Verify secret (`401` as today).
5. Reject disabled/inactive workflows, a missing home channel, and failed home-channel owner authority through the existing generic `404` before parsing the dynamic body.
6. Parse JSON syntax (`400` on invalid JSON), require an object, and require a non-empty string `idempotency_key` (`422 invalid_control_fields` with no run when the key is unusable).
7. Remove `idempotency_key`, hash its raw UTF-8 bytes, hash the sanitized parsed JSON, and build trigger context only from the sanitized object.
8. Call `begin_workflow_admission` before validating `repository_name` or loading repository/project/channel state.
9. On `Existing`, replay canonical `422` only when `status == Failed` and `RouteFailure::from_code(error_code)` returns an admission rejection; otherwise return `202` with the existing id and current status.
10. On `PayloadConflict`, return `409 idempotency_conflict` without updating the existing row.
11. On `Vacant(mut guard)`, parse the non-empty exact `repository_name`; if it is invalid, call `guard.reject` with `invalid_control_fields` and return canonical `422`.
12. Through the guard transaction, enumerate latest live global `30617` heads without a limit, parse them, and resolve the fixed identity tiers.
13. Through the same guard, enumerate latest live global `30621` heads without a limit, parse them, and require exactly one claim-valid project before channel inspection.
14. Require exactly one valid `buzz-channel` value, parse it as UUID, load the non-deleted channel through the guard, reject missing or archived as `project_channel_invalid`, and never inspect another project.
15. Load the workflow owner's active destination role through the guard; `None` is `route_unauthorized` even for an open channel.
16. Build a snapshot with the request community UUID, selected repository coordinate, selected project coordinate, destination UUID, and `IdentityTier::as_str()`.
17. Call `guard.accept`; only that successful insert produces a pending run.
18. Spawn execution once for that newly accepted run and return its `202` response.

Implement `resolve_dynamic_route` with this complete data flow:

```rust
async fn resolve_dynamic_route(
    guard: &mut WorkflowAdmissionGuard,
    community_id: CommunityId,
    repository_name: &str,
    routing: &RoutingDef,
    workflow_owner: &[u8],
) -> Result<WorkflowRouteSnapshot, ResolveRouteError> {
    let repository_events = guard
        .list_latest_parameterized_heads(KIND_GIT_REPO_ANNOUNCEMENT as i32)
        .await
        .map_err(ResolveRouteError::Transient)?;
    let repositories: Vec<RepositoryHead> = repository_events
        .iter()
        .filter_map(|stored| repository_head_from_event(&stored.event))
        .collect();
    let (repository_coordinate, tier) = resolve_repository_identity(
        repository_name,
        &routing.aliases,
        &repositories,
    )
    .map_err(ResolveRouteError::Deterministic)?;
    let repository = repositories
        .iter()
        .find(|head| head.coordinate == repository_coordinate)
        .ok_or(ResolveRouteError::Deterministic(
            RouteFailure::RepositoryMissing,
        ))?;

    let project_events = guard
        .list_latest_parameterized_heads(KIND_PROJECT as i32)
        .await
        .map_err(ResolveRouteError::Transient)?;
    let projects: Vec<ProjectHead> = project_events
        .iter()
        .filter_map(|stored| project_head_from_event(&stored.event))
        .collect();
    let project = authorize_unique_project_route(repository, &projects)
        .map_err(ResolveRouteError::Deterministic)?;
    let channel_id = project
        .buzz_channel
        .as_deref()
        .ok_or(ResolveRouteError::Deterministic(
            RouteFailure::ProjectChannelInvalid,
        ))?
        .parse::<Uuid>()
        .map_err(|_| {
            ResolveRouteError::Deterministic(RouteFailure::ProjectChannelInvalid)
        })?;
    let channel = guard
        .load_destination_channel(channel_id)
        .await
        .map_err(ResolveRouteError::Transient)?
        .ok_or(ResolveRouteError::Deterministic(
            RouteFailure::ProjectChannelInvalid,
        ))?;
    if channel.archived_at.is_some() {
        return Err(ResolveRouteError::Deterministic(
            RouteFailure::ProjectChannelInvalid,
        ));
    }
    let role = guard
        .destination_member_role(channel_id, workflow_owner)
        .await
        .map_err(ResolveRouteError::Transient)?;
    if role.is_none() {
        return Err(ResolveRouteError::Deterministic(
            RouteFailure::RouteUnauthorized,
        ));
    }

    Ok(WorkflowRouteSnapshot {
        community_id: *community_id.as_uuid(),
        repository_coordinate,
        project_coordinate: project.coordinate.clone(),
        channel_id,
        matched_identity_tier: tier.as_str().to_owned(),
    })
}
```

Use the existing `KIND_GIT_REPO_ANNOUNCEMENT` and `KIND_PROJECT` constants from `buzz_core::kind`; do not write raw kind integers in production code.

After the dynamic lifecycle and home-authority gate, the admission branch must have this control flow:

```rust
let Some(routing) = def.routing.as_ref() else {
    return Err(not_found("workflow not found"));
};
let body_value: Value = serde_json::from_slice(&body)
    .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid JSON body"))?;
if !body_value.is_object() {
    let failure = RouteFailure::InvalidControlFields;
    record_admission_observation(AdmissionObservation::without_run(
        Some(id),
        Some(*community_id.as_uuid()),
        "rejected",
        failure.code(),
    ));
    return Err(api_error_with_code(
        StatusCode::UNPROCESSABLE_ENTITY,
        failure.redacted_message(),
        failure.code(),
    ));
}
let raw_key = match parse_idempotency_key(&body_value) {
    Ok(raw_key) => raw_key,
    Err(failure) => {
        record_admission_observation(AdmissionObservation::without_run(
            Some(id),
            Some(*community_id.as_uuid()),
            "rejected",
            failure.code(),
        ));
        return Err(route_failure_response(failure));
    }
};
let sanitized_body = strip_idempotency_key(body_value);
let key_hash = hash_idempotency_key(&raw_key);
drop(raw_key);
let payload_hash = canonical_payload_hash(&sanitized_body)
    .map_err(|_| api_error(StatusCode::SERVICE_UNAVAILABLE, "service unavailable"))?;
let trigger_ctx = TriggerContext {
    channel_id: workflow
        .channel_id
        .map(|channel| channel.to_string())
        .unwrap_or_default(),
    webhook_fields: webhook_fields_from_body(sanitized_body.clone()),
    ..Default::default()
};
let trigger_ctx_json = serde_json::to_value(&trigger_ctx)
    .map_err(|_| api_error(StatusCode::SERVICE_UNAVAILABLE, "service unavailable"))?;

let begin = match state
    .db
    .begin_workflow_admission(
        community_id,
        id,
        &key_hash,
        &payload_hash,
    )
    .await
{
    Ok(begin) => begin,
    Err(_) => {
        record_admission_observation(AdmissionObservation::without_run(
            Some(id),
            Some(*community_id.as_uuid()),
            "unavailable",
            "none",
        ));
        return Err(api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "service unavailable",
        ));
    }
};
let mut guard = match begin {
    BeginWorkflowAdmission::Existing(run) => {
        if let Some((failure, response)) = replayed_admission_failure(&run) {
            record_admission_observation(AdmissionObservation::for_run(
                &run,
                "rejected",
                failure.code(),
            ));
            return Err(response);
        }
        record_admission_observation(AdmissionObservation::for_run(
            &run,
            "accepted",
            "none",
        ));
        return Ok(accepted_run_response(id, &run));
    }
    BeginWorkflowAdmission::PayloadConflict { existing } => {
        let failure = RouteFailure::IdempotencyConflict;
        record_admission_observation(AdmissionObservation::for_run(
            &existing,
            "conflict",
            failure.code(),
        ));
        return Err(api_error_with_code(
            StatusCode::CONFLICT,
            failure.redacted_message(),
            failure.code(),
        ));
    }
    BeginWorkflowAdmission::Vacant(guard) => guard,
};

let repository_name = match parse_repository_name(&sanitized_body) {
    Ok(name) => name,
    Err(failure) => {
        let run = guard
            .reject(
                &trigger_ctx_json,
                WorkflowRunFailure {
                    code: failure.code(),
                    message: failure.redacted_message(),
                },
            )
            .await
            .map_err(|_| api_error(StatusCode::SERVICE_UNAVAILABLE, "service unavailable"))?;
        record_admission_observation(AdmissionObservation::for_run(
            &run,
            "rejected",
            failure.code(),
        ));
        return Err(route_failure_response(failure));
    }
};

let snapshot = match resolve_dynamic_route(
    &mut guard,
    community_id,
    &repository_name,
    routing,
    &workflow.owner_pubkey,
)
.await
{
    Ok(snapshot) => snapshot,
    Err(ResolveRouteError::Deterministic(failure)) => {
        let run = guard
            .reject(
                &trigger_ctx_json,
                WorkflowRunFailure {
                    code: failure.code(),
                    message: failure.redacted_message(),
                },
            )
            .await
            .map_err(|_| api_error(StatusCode::SERVICE_UNAVAILABLE, "service unavailable"))?;
        record_admission_observation(AdmissionObservation::for_run(
            &run,
            "rejected",
            failure.code(),
        ));
        return Err(route_failure_response(failure));
    }
    Err(ResolveRouteError::Transient(_)) => {
        drop(guard);
        record_admission_observation(AdmissionObservation::without_run(
            Some(id),
            Some(*community_id.as_uuid()),
            "unavailable",
            "none",
        ));
        return Err(api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "service unavailable",
        ));
    }
};
let run = guard
    .accept(&trigger_ctx_json, &snapshot)
    .await
    .map_err(|_| api_error(StatusCode::SERVICE_UNAVAILABLE, "service unavailable"))?;
record_admission_observation(AdmissionObservation::for_run(
    &run,
    "accepted",
    "none",
));
spawn_admitted_run(
    &state,
    community_id,
    run.id,
    workflow.definition.clone(),
    trigger_ctx,
);
Ok(accepted_run_response(id, &run))
```

For every deterministic error after `Vacant`, call `guard.reject` with sanitized trigger context, the stable code, and `RouteFailure::redacted_message`, then return canonical `422` without a `run_id` field.
For every database or canonical-serialization error while the guard is vacant, drop the guard, return `503 {"error":"service unavailable"}`, and leave no run row.

If the database errors before a run exists, return `503` with `{ "error": "service unavailable" }` and no code that reveals internals.
Do not use `internal_error`'s `500` on that path.

Logs may include `run_id`, `workflow_id`, `community_id`, outcome, and failure code.
Logs must not include the raw key, secret, callback body, or candidate coordinate lists.
Route every admission outcome log through one helper that accepts only `AdmissionObservation`; do not invoke `tracing` with the request body, `provided_secret`, raw key, repository name, or resolver candidates anywhere in `workflow_admission.rs`.

Increment a low-cardinality counter:

```rust
metrics::counter!(
    "buzz_workflow_webhook_admission_total",
    "outcome" => outcome, // accepted | conflict | rejected | unavailable
    "code" => code        // "none" or RouteFailure::code()
).increment(1);
```

- [ ] **Step 4: Run unit tests and `cargo check`**

```bash
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_admission -- --nocapture && cargo check -p buzz-relay
```

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-relay/src/workflow_admission.rs crates/buzz-relay/src/api/mod.rs crates/buzz-relay/src/lib.rs && git commit -s -m "feat(relay): prepare idempotent webhook project routing"
```

---

### Task 7: Snapshot send_message, revalidation, provenance, and admission activation

**Files:**

- Modify: `crates/buzz-workflow/src/action_sink.rs`
- Modify: `crates/buzz-workflow/src/executor.rs`
- Modify: `crates/buzz-workflow/src/error.rs`
- Modify: `crates/buzz-relay/src/workflow_sink.rs`
- Modify: `crates/buzz-relay/src/workflow_route.rs`
- Modify: `crates/buzz-relay/src/api/bridge.rs` at `workflow_webhook`

**Interfaces:**

- Consumes: `WorkflowRunRecord.route_snapshot`, existing sink membership and kind `9` builder.
- Consumes: the dormant `workflow_admission::handle_workflow_webhook` from Task 6.
- Produces:
  - `pub struct WorkflowMessageRoute { pub run_id: Uuid, pub workflow_id: Uuid, pub home_channel_id: Uuid, pub repository_coordinate: String, pub project_coordinate: String }`
  - `ActionSink::send_message(&self, community_id: CommunityId, channel_id: &str, text: &str, author_pubkey: &str, route: Option<WorkflowMessageRoute>) -> Pin<Box<dyn Future<Output = Result<String, ActionSinkError>> + Send + '_>>`
  - Unit variant `ActionSinkError::RouteStale` mapped only to unit variant `WorkflowError::RouteStale`
  - `WorkflowError::RouteStale` with redacted display text `route is no longer valid` and `code() == "route_stale"`
  - `fn route_provenance_tags(route: &WorkflowMessageRoute) -> Result<[Tag; 3], ActionSinkError>`
  - Provenance tags on success: `a` repository, `a` project, `buzz:workflow-run` = run UUID
  - Test-only `ClaimSignerMode::{Owner, Maintainer}` and `DynamicRouteFixture { state, sink, community_id, home_channel_id, destination_channel_id, route, raw_test_key, baseline_counts }` with `new`, `send`, one mutation method per stale case, `assert_no_new_kind9`, and `load_sent_event`
  - Test-only `fn assert_exact_dynamic_provenance(event: &nostr::Event, route: &WorkflowMessageRoute)`

**Acceptance:** A run snapshot is used only when its community matches the executing run, any explicit dynamic channel field is rejected even when empty, every dynamic send revalidates the exact stored route immediately before event construction, stale authority inserts no event, successful roots carry exactly the required provenance, resolved action values are absent from executor logs, and only then does the live webhook route switch to the tested admission helper while static behavior remains unchanged.

- [ ] **Step 1: Run impact for `ActionSink::send_message`, `resolve_send_message_channel`, `RelayActionSink::send_message`, and `workflow_webhook`**

The only `ActionSink` implementor is `RelayActionSink`.
The only production caller is `dispatch_action` in `executor.rs`.

- [ ] **Step 2: Write failing executor tests**

Replace the channel helper usage with a wrapper that sees a snapshot:

```rust
    #[test]
    fn dynamic_send_message_uses_snapshot_channel_and_rejects_override() {
        let run_community = CommunityId::from_uuid(Uuid::new_v4());
        let snapshot_channel = Uuid::new_v4();
        let snapshot = buzz_db::workflow_admission::WorkflowRouteSnapshot {
            community_id: *run_community.as_uuid(),
            repository_coordinate: format!("30617:{}:agentic-os-plan", "ab".repeat(32)),
            project_coordinate: format!("30621:{}:gigo-harness", "cd".repeat(32)),
            channel_id: snapshot_channel,
            matched_identity_tier: "d_tag".into(),
        };
        let resolved = resolve_send_message_channel_for_run(
            run_community,
            None,
            "",
            Some(Uuid::new_v4()),
            Some(&snapshot),
        )
        .expect("snapshot wins");
        assert_eq!(resolved, snapshot_channel.to_string());

        let err = resolve_send_message_channel_for_run(
            run_community,
            Some(&Uuid::new_v4().to_string()),
            "",
            Some(Uuid::new_v4()),
            Some(&snapshot),
        )
        .unwrap_err();
        assert!(matches!(err, WorkflowError::InvalidDefinition(_)));

        let empty_field = resolve_send_message_channel_for_run(
            run_community,
            Some(""),
            "",
            Some(Uuid::new_v4()),
            Some(&snapshot),
        )
        .unwrap_err();
        assert!(matches!(empty_field, WorkflowError::InvalidDefinition(_)));

        let wrong_community = buzz_db::workflow_admission::WorkflowRouteSnapshot {
            community_id: Uuid::new_v4(),
            ..snapshot
        };
        assert!(matches!(
            resolve_send_message_channel_for_run(
                run_community,
                None,
                "",
                Some(Uuid::new_v4()),
                Some(&wrong_community),
            ),
            Err(WorkflowError::RouteStale)
        ));
    }

    #[test]
    fn static_send_message_channel_rules_remain_unchanged() {
        let run_community = CommunityId::from_uuid(Uuid::new_v4());
        let workflow_channel_id = Uuid::new_v4();
        let resolved = resolve_send_message_channel_for_run(
            run_community,
            None,
            "",
            Some(workflow_channel_id),
            None,
        )
        .expect("static bound channel");
        assert_eq!(resolved, workflow_channel_id.to_string());
    }
```

Keep the three existing `resolve_send_message_channel` tests passing by delegating the `None` snapshot path to that function.

Add this sink unit test:

```rust
    #[test]
    fn route_provenance_has_exact_coordinates_and_run_without_control_key() {
        let route = WorkflowMessageRoute {
            run_id: Uuid::new_v4(),
            workflow_id: Uuid::new_v4(),
            home_channel_id: Uuid::new_v4(),
            repository_coordinate: format!("30617:{}:agentic-os-plan", "ab".repeat(32)),
            project_coordinate: format!("30621:{}:gigo-harness", "cd".repeat(32)),
        };
        let tags = route_provenance_tags(&route).expect("build provenance tags");
        let parts: Vec<Vec<String>> = tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect();
        assert!(parts.contains(&vec!["a".into(), route.repository_coordinate.clone()]));
        assert!(parts.contains(&vec!["a".into(), route.project_coordinate.clone()]));
        assert!(parts.contains(&vec![
            "buzz:workflow-run".into(),
            route.run_id.to_string(),
        ]));
        assert!(!serde_json::to_string(&parts)
            .expect("serialize tag parts")
            .contains("idempotency_key"));
    }
```

- [ ] **Step 3: Run tests red**

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib executor::tests::dynamic_send_message_uses_snapshot_channel_and_rejects_override -- --exact --nocapture
```

Expected: FAIL on missing wrapper.

- [ ] **Step 4: Implement executor + sink**

```rust
fn resolve_send_message_channel_for_run(
    run_community_id: CommunityId,
    explicit_channel: Option<&str>,
    trigger_channel: &str,
    workflow_channel_id: Option<Uuid>,
    route_snapshot: Option<&buzz_db::workflow_admission::WorkflowRouteSnapshot>,
) -> Result<String, WorkflowError> {
    if let Some(snapshot) = route_snapshot {
        if snapshot.community_id != *run_community_id.as_uuid() {
            return Err(WorkflowError::RouteStale);
        }
        if explicit_channel.is_some() {
            return Err(WorkflowError::InvalidDefinition(
                "SendMessage: channel override is invalid for project_channel_by_repository routing"
                    .into(),
            ));
        }
        return Ok(snapshot.channel_id.to_string());
    }
    resolve_send_message_channel(explicit_channel, trigger_channel, workflow_channel_id)
}
```

In `dispatch_action` for `SendMessage`, after loading `wf_run` and `workflow`:

```rust
                    let channel_id = resolve_send_message_channel_for_run(
                        community_id,
                        channel.as_deref(),
                        &trigger_ctx.channel_id,
                        workflow.channel_id,
                        wf_run.route_snapshot.as_ref(),
                    )?;
                    let route = match (&wf_run.route_snapshot, workflow.channel_id) {
                        (Some(snapshot), Some(home_channel_id)) => Some(WorkflowMessageRoute {
                            run_id,
                            workflow_id: wf_run.workflow_id,
                            home_channel_id,
                            repository_coordinate: snapshot.repository_coordinate.clone(),
                            project_coordinate: snapshot.project_coordinate.clone(),
                        }),
                        (Some(_), None) => {
                            return Err(WorkflowError::InvalidDefinition(
                                "dynamic send_message requires a workflow home channel".into(),
                            ));
                        }
                        (None, _) => None,
                    };
                    let event_id = engine.action_sink()?.send_message(
                        community_id,
                        &channel_id,
                        text,
                        &owner_pubkey_hex,
                        route,
                    )
                    .await
                    .map_err(WorkflowError::from)?;
```

Replace executor action logs that interpolate resolved `text`, `to`, `emoji`, `url`, approval `message`, or `duration` with structural logs containing only `run_id`, `step`, the action name, and low-cardinality state.
For `SendMessage`, `channel` and `dynamic_route = route.is_some()` are allowed fields, but message text is not.
For `CallWebhook`, do not log the resolved URL, headers, or body.
For `RequestApproval`, do not log the resolved requester, message, or timeout.
For `Delay`, log the parsed bounded seconds only, not the original resolved duration string.
This log redaction applies to static and dynamic runs and does not alter action inputs, outputs, execution traces, or side effects.

Add to `WorkflowError`:

```rust
    /// Dynamic route lost authority between admission and the side effect.
    #[error("route is no longer valid")]
    RouteStale,
```

`code()` returns `"route_stale"`.
Add an assertion for that code to `error.rs::tests::workflow_error_codes_are_stable_and_separate_from_diagnostics`.

Map `ActionSinkError::RouteStale` to `WorkflowError::RouteStale` in `From`.
Do not map it through `WebhookError`.

In the sink, when `route` is `Some`, immediately before event construction call `async fn revalidate_dynamic_route(state: &AppState, community_id: CommunityId, channel_id: Uuid, author_pubkey: &[u8], route: &WorkflowMessageRoute) -> Result<(), ActionSinkError>` which must:

1. Reload the workflow; require `enabled` and `status == Active`.
2. Require the workflow's current `channel_id` to equal `home_channel_id` and its owner pubkey to equal the `author_pubkey` passed to the sink.
3. Parse the current definition and call `check_owner_authority` on `home_channel_id`.
4. Parse the stored repository coordinate and load that exact latest live global head in `community_id`; missing or malformed is stale.
5. Load latest project heads with no limit and rebuild `ProjectHead` values.
6. Recompute all claim-valid projects, require exactly one, and require its coordinate to equal the stored project coordinate; this equality prevents rerouting to a replacement project.
7. Require that exact project's one valid `buzz-channel` UUID to equal the send `channel_id`.
8. Require the channel to resolve in `community_id`, be non-deleted, and have `archived_at == None`.
9. Require `get_member_role` for the workflow owner on the destination to be `Some`, including open channels.

Any failed check returns the redacted unit variant `ActionSinkError::RouteStale` and must not call `insert_event_with_thread_metadata`.
Do not embed a coordinate, channel id, membership detail, or workflow detail in the stored route-stale diagnostic.

On success, keep existing tags and append:

```rust
Tag::parse(["a", &route.repository_coordinate])
Tag::parse(["a", &route.project_coordinate])
Tag::parse(["buzz:workflow-run", &route.run_id.to_string()])
```

Do not add the raw idempotency key, alias map, or callback body.

Place mention resolution before the final dynamic revalidation call.
After revalidation succeeds, do not perform another async lookup before `EventBuilder` constructs the event and `insert_event_with_thread_metadata` persists it.
After persistence, keep the existing `dispatch_persistent_event` call so the message still receives WebSocket fan-out, Redis publication, indexing, and the normal `EventCreated` hash-chain audit record.

For `route == None`, keep today's open-channel exception: non-members may post only when `visibility == "open"`.
Dynamic routes must not use that exception.
Update the existing `workflow_send_message_p_tags_mentioned_member` call to pass `None` and keep all of its current assertions green.

- [ ] **Step 5: Add database-backed stale-route tests before declaring the task green**

Extend the existing ignored `workflow_sink::integration_tests` fixtures to create a home channel, destination channel, active workflow, live repository head, one owner- or maintainer-signed listed project, and a `WorkflowMessageRoute`.
Give the fixture an explicit `ClaimSignerMode::{Owner, Maintainer}` input; use `Maintainer` only for the maintainer-removal case and `Owner` for the other cases.
Add these exact tests:

1. `dynamic_route_removed_destination_member_is_stale_and_inserts_no_event` removes the workflow owner from the destination after the fixture snapshot, calls `send_message`, asserts `ActionSinkError::RouteStale`, and asserts the destination kind `9` count did not change.
2. `dynamic_route_disabled_workflow_is_stale_and_inserts_no_event` calls `set_workflow_enabled(false)` before the sink and makes the same assertions.
3. `dynamic_route_deleted_repository_is_stale_and_inserts_no_event` soft-deletes the exact repository head before the sink and makes the same assertions.
4. `dynamic_route_deleted_project_is_stale_and_inserts_no_event` soft-deletes the exact project head before the sink and makes the same assertions.
5. `dynamic_route_project_signer_loses_maintainer_authority_is_stale` replaces the repository head without the stored project's signer in `maintainers`, asserts stale, and proves no event was inserted.
6. `dynamic_route_second_claim_is_stale_and_does_not_reroute` publishes a second listed claim-valid project before the sink, asserts stale, and asserts neither destination receives a new kind `9`.
7. `dynamic_route_project_channel_change_is_stale_and_inserts_no_event` replaces the stored project head with a different `buzz-channel` and makes the same assertions.
8. `dynamic_route_archived_channel_is_stale_and_inserts_no_event` archives the destination and makes the same assertions.
9. `dynamic_route_deleted_channel_is_stale_and_inserts_no_event` soft-deletes the destination and makes the same assertions.
10. `dynamic_route_removed_home_authority_is_stale_and_inserts_no_event` removes the workflow owner from the workflow home channel and makes the same assertions.
11. `dynamic_route_success_adds_provenance_and_no_control_key` keeps the fixture valid, sends once, loads the event, and asserts one repository `a`, one project `a`, one `buzz:workflow-run`, the existing `h`, `buzz:workflow`, and `buzz:workflow-owner` tags, and absence of the raw test key from tags and content.

Use this test shape so every listed mutation exercises the real sink and the common no-insert assertion:

```rust
macro_rules! stale_route_case {
    ($name:ident, $mode:expr, $mutation:ident) => {
        #[tokio::test]
        #[ignore = "requires Postgres"]
        async fn $name() {
            let mut fixture = DynamicRouteFixture::new($mode).await;
            fixture.$mutation().await;
            let error = fixture.send().await.expect_err("route must be stale");
            assert!(matches!(error, ActionSinkError::RouteStale));
            fixture.assert_no_new_kind9().await;
        }
    };
}

stale_route_case!(
    dynamic_route_removed_destination_member_is_stale_and_inserts_no_event,
    ClaimSignerMode::Owner,
    remove_destination_member
);
stale_route_case!(
    dynamic_route_disabled_workflow_is_stale_and_inserts_no_event,
    ClaimSignerMode::Owner,
    disable_workflow
);
stale_route_case!(
    dynamic_route_deleted_repository_is_stale_and_inserts_no_event,
    ClaimSignerMode::Owner,
    soft_delete_repository
);
stale_route_case!(
    dynamic_route_deleted_project_is_stale_and_inserts_no_event,
    ClaimSignerMode::Owner,
    soft_delete_project
);
stale_route_case!(
    dynamic_route_project_signer_loses_maintainer_authority_is_stale,
    ClaimSignerMode::Maintainer,
    replace_repository_without_maintainer
);
stale_route_case!(
    dynamic_route_second_claim_is_stale_and_does_not_reroute,
    ClaimSignerMode::Owner,
    publish_second_claim
);
stale_route_case!(
    dynamic_route_project_channel_change_is_stale_and_inserts_no_event,
    ClaimSignerMode::Owner,
    replace_project_channel
);
stale_route_case!(
    dynamic_route_archived_channel_is_stale_and_inserts_no_event,
    ClaimSignerMode::Owner,
    archive_destination
);
stale_route_case!(
    dynamic_route_deleted_channel_is_stale_and_inserts_no_event,
    ClaimSignerMode::Owner,
    soft_delete_destination
);
stale_route_case!(
    dynamic_route_removed_home_authority_is_stale_and_inserts_no_event,
    ClaimSignerMode::Owner,
    remove_home_member
);

#[tokio::test]
#[ignore = "requires Postgres"]
async fn dynamic_route_success_adds_provenance_and_no_control_key() {
    let fixture = DynamicRouteFixture::new(ClaimSignerMode::Owner).await;
    let event_id = fixture.send().await.expect("send through valid route");
    let event = fixture.load_sent_event(&event_id).await;
    assert_exact_dynamic_provenance(&event.event, &fixture.route);
    let serialized = serde_json::to_string(&event.event).expect("serialize event");
    assert!(!serialized.contains(&fixture.raw_test_key));
}
```

Define `assert_exact_dynamic_provenance` in the same test module to count tag slices, not substring-match serialized tags: exactly one repository `a`, one project `a`, one `buzz:workflow-run`, one destination `h`, one `buzz:workflow`, and one `buzz:workflow-owner`, with no root/parent `e` tag.

Use `state.db.get_event_by_id` for the positive event and an explicit `SELECT COUNT(*) FROM events WHERE community_id = $1 AND channel_id = $2 AND kind = 9 AND deleted_at IS NULL` baseline for negative cases.
Do not weaken the tests to call only a pure authorization helper; each negative test must exercise `RelayActionSink::send_message` and prove no row was inserted.

- [ ] **Step 6: Activate admission only after snapshot-aware execution is implemented**

Replace the current `workflow_webhook` body in `api/bridge.rs` with this thin wrapper and delete the temporary duplicated body rather than leaving dead logic behind:

```rust
pub async fn workflow_webhook(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
    Query(query): Query<WebhookQuery>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    crate::workflow_admission::handle_workflow_webhook(
        state,
        id_str,
        query.secret,
        headers,
        body,
    )
    .await
}
```

Do not reorder lifecycle checks, secret verification, JSON parsing, or static run creation while extracting the old body.

- [ ] **Step 7: Run tests green**

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_route -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_sink -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_sink::integration_tests::dynamic_route_ -- --ignored --nocapture
. ./bin/activate-hermit && ! rg -n 'SendMessage.*\{text\}|SendDm.*\{to\}|AddReaction.*\{emoji\}|CallWebhook.*\{url\}|RequestApproval.*\{message\}|Delay.*\{duration\}' crates/buzz-workflow/src/executor.rs
```

- [ ] **Step 8: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-workflow/src/action_sink.rs crates/buzz-workflow/src/executor.rs crates/buzz-workflow/src/error.rs crates/buzz-relay/src/workflow_sink.rs crates/buzz-relay/src/workflow_route.rs crates/buzz-relay/src/api/bridge.rs && git commit -s -m "feat(workflow): activate revalidated project-channel webhook routes"
```

---

### Task 8: Static compatibility, CI unit wiring, and relay-backed E2E

**Files:**

- Modify: `Justfile` (`test-unit`)
- Modify: `scripts/run-tests.sh` (`run_unit_tests`)
- Modify: `crates/buzz-test-client/Cargo.toml`
- Modify: `crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs`
- Modify: `crates/buzz-workflow/src/schema.rs`

**Interfaces:**

- Consumes: live relay as in [TESTING.md](../../../TESTING.md), kind `30620` workflow save, `POST /hooks/{id}`, kinds `30617`/`30621`/`9`.
- Produces: ignored E2E coverage for dual-repository routing, ambiguity, unauthorized membership, idempotency, and static non-routing webhooks.

**Acceptance:** The repository-wide unit gate executes `buzz-workflow`; a live migrated relay proves static compatibility, both repository routes, every destination denial class, deterministic failure replay, concurrent idempotency, stale-route refusal, provenance, and raw-key redaction from run and event data.

- [ ] **Step 1: Add buzz-workflow to unit CI**

In `Justfile` `test-unit`, after the `buzz-db --lib` nextest invocation, add:

```
        cargo nextest run -p buzz-workflow --lib
```

In the `else` branch, `scripts/run-tests.sh unit` must gain the same package.
In `run_unit_tests`, add:

```bash
  run_test_step "buzz-workflow tests" \
    cargo test -p buzz-workflow --lib -- --nocapture
```

This is required so the Task 1 tests actually gate PRs.
`just test-unit` does not currently run `buzz-workflow`.

- [ ] **Step 2: Write the static compatibility unit test**

```rust
    #[test]
    fn static_message_posted_workflow_does_not_grow_routing_fields() {
        let yaml = concat!(
            "name: 'Incident Alert'\n",
            "trigger:\n  on: message_posted\n",
            "steps:\n  - id: notify\n    action: send_message\n    text: 'P1 alert'\n",
        );
        let (def, json) = parse_yaml(yaml).expect("static parse");
        assert!(!def.has_project_channel_routing());
        assert!(!json.contains("\"routing\""));
        assert!(!json.contains("project_channel_by_repository"));
        assert!(!json.contains("idempotency_key"));
    }
```

Run `cargo test -p buzz-workflow --lib schema::tests::static_message_posted_workflow_does_not_grow_routing_fields -- --exact --nocapture` and require PASS.

- [ ] **Step 3: Write the ignored E2E spec**

Extend the E2E file created in Task 5.
Reuse event and channel command shapes from `e2e_project.rs` and `e2e_relay.rs`.
Do not copy the stale `conformance_multitenant.rs::define_workflow` comment that says the server generates the workflow UUID; current `handle_workflow_def` requires a client UUID in the kind `30620` `d` tag.

Helpers:

- `relay_url()` / `relay_http_url()` as in `e2e_relay.rs`.
- `unique(prefix)` for `d` tags.
- `define_webhook_workflow(http, keys, home_channel, yaml) -> (workflow_id, webhook_secret)` calls the Task 5 raw helper, requires `accepted == true`, and unwraps both accepted-only response fields with a diagnostic containing `message`.
- `post_hook(http, workflow_id, secret, body) -> (StatusCode, Value)` with `X-Webhook-Secret` and `Host` from the relay URL.
- `query_kind9(http, reader_keys, channel_id) -> Vec<Value>` uses NIP-98-authenticated `POST /query` with explicit `kinds:[9]`, `#h:[channel_id]`, and `limit:100`.
- `wait_for_kind9_count(http: &str, reader_keys: &Keys, channel_id: Uuid, expected: usize) -> Vec<Value>` polls every 50ms for at most 5 seconds and fails with the last response instead of sleeping a fixed interval.
- `test_pool()` uses `BUZZ_TEST_DATABASE_URL`, then `DATABASE_URL`, then `postgres://buzz:buzz_dev@localhost:5432/buzz`, because every E2E run in this file requires Postgres inspection.
- `community_id_for_http(pool, http)` normalizes the URL authority with `buzz_core::tenant::relay_url_authority` and selects the matching `communities.id`; every workflow-run query binds that community id and run id.
- `create_foreign_channel(pool, channel_id, keys)` uses a test-only `buzz_db::Db`, creates a uniquely hosted second community, ensures the key's user row there, and calls `create_channel_with_id` in that second community without sending requests to that host or starting another relay.
- `workflow_run_for_key(pool, community_id, workflow_id, raw_key)` computes SHA-256 of the raw UTF-8 key and selects one row with `WHERE community_id = $1 AND workflow_id = $2 AND idempotency_key_hash = $3`.
- `wait_for_run_status(pool, community_id, run_id, expected)` polls that exact scoped row every 50ms for at most 5 seconds and returns `trigger_context`, `route_snapshot`, `execution_trace`, `error_message`, `error_code`, and `status` for assertions.

Tests, each `#[tokio::test] #[ignore]`:

1. `static_webhook_without_routing_still_posts_to_home_channel`
   YAML has no `routing`.
   Body may omit `repository_name` and `idempotency_key`.
   Assert `202` and a kind `9` in the home channel with `buzz:workflow=true` and without `buzz:workflow-run`.

2. `one_workflow_routes_agentic_os_plan_and_harness_service`
   Create destination channel `gigo-harness-{unique}` and add the workflow owner as a member.
   Publish same-owner `30617` repos whose `d` tags are `agentic-os-plan-{unique}` and `harness-service-{unique}`.
   Publish one listed `30621` named `gigo-harness` whose signer is the repository owner, whose `a` tags are both coordinates, and whose `buzz-channel` is the destination UUID.
   Use a separate workflow owner who is a destination member; do not add the project signer to the destination, proving that project-signer membership is not required.
   Dynamic YAML uses `routing.mode: project_channel_by_repository`, one `send_message` without `channel`, and text `repo={{trigger.repository_name}} key={{trigger.idempotency_key}}` so the event proves repository substitution works while the unknown control-field placeholder cannot reveal the raw key.
   POST twice with those `repository_name` values and distinct `idempotency_key`s.
   Assert both relay-signed kind `9` messages land in the destination channel, not the home channel, have no root or parent `e` tag, and each event has `a` tags for the matching repo coordinate and the project coordinate plus `buzz:workflow-run`.
   Assert each run snapshot contains the host community UUID, its exact repository coordinate, the exact project coordinate, the destination UUID, and `matched_identity_tier == "d_tag"`.
   Poll both scoped runs to `completed` and assert each run's `send_message` execution-trace output names the exact event id carrying its `buzz:workflow-run` tag.

3. `ambiguous_claim_valid_projects_return_422_and_no_message`
   Two listed owner-signed projects both include the same repo.
   Assert `422` / `project_ambiguous` and zero new kind `9` events in either project channel or the home channel.

4. `open_destination_without_membership_is_unauthorized`
   Destination channel is `open` but the owner is not a member.
   Assert `422` / `route_unauthorized` and no message.

5. `same_key_same_payload_does_not_send_a_second_message`
   POST twice concurrently with identical body.
   Assert both `202` bodies return the same run id, one scoped workflow-run row exists, and exactly one kind `9` appears after polling.

6. `same_key_different_payload_is_409`
   Second body changes `title`.
   Assert `409` / `idempotency_conflict` and still one message.

7. `idempotency_key_never_appears_on_the_event_or_run_context`
   After success, fetch the kind `9` and the scoped workflow run.
   Assert the event content contains the substituted repository name and literal `{{trigger.idempotency_key}}`, but not the raw key string.
   Assert the raw key is also absent from event tags and from `trigger_context`, `route_snapshot`, `execution_trace`, `error_message`, and `error_code` serialized together.

8. `destination_channel_failure_matrix_posts_nothing`
   Use isolated fixtures for a project with no `buzz-channel` tag, a malformed `buzz-channel` UUID, an archived channel, and a channel UUID that exists only in another community.
   Add `buzz-db = { workspace = true }` under `buzz-test-client` dev-dependencies and use `create_foreign_channel` for the cross-community case instead of requiring a second relay process or host mapping.
   Assert every request returns `422 project_channel_invalid`, creates one failed run with `route_snapshot IS NULL` when the key is usable, and adds no kind `9` in the home or any candidate destination.

9. `deterministic_failure_replays_until_a_new_key_is_used`
   POST a valid repository with no claim-valid project and assert `422 project_missing` plus one failed row.
   Publish the valid project, retry the identical body and key, and assert the same canonical `422` and the same run row without resolution or a message.
   POST the same payload with a new key and assert `202` plus one destination message.

10. `membership_revoked_during_delay_marks_same_run_route_stale`
    Define a dynamic workflow whose first step is `delay` for `2s` and whose second step sends the message.
    Admit it, remove the workflow owner from the destination through kind `9001` while the delay is running, poll the scoped run to `failed`, assert `error_code == "route_stale"`, and assert no kind `9` was inserted.
    Retry the identical body and key, assert `202` with the same failed run id and status, wait beyond the original delay, and assert the run was not restarted and no message appeared.

11. `pre_admission_failures_create_no_run`
    Record the workflow's scoped run count, then send invalid JSON, a missing key, an empty key, a non-string key, and an invalid secret in isolated requests.
    Assert invalid JSON is `400`, the key failures are `422 invalid_control_fields`, invalid secret is `401`, and the scoped run count never changes.

12. `static_and_dynamic_lifecycle_parse_order_is_preserved`
    Save one static and one dynamic webhook, set both `workflows.enabled = FALSE` through community-and-workflow-scoped SQL, and POST malformed JSON with each valid secret.
    Assert the static webhook returns its current `400 invalid JSON body` before lifecycle rejection, while the dynamic webhook returns the generic `404 workflow not found` before body parsing, and assert neither workflow gains a run.

The Task 5 `alias_target_must_be_live_at_save` test remains in this file and must stay green.

Do not POST error text into a channel in any test.
Do not add GitHub or agent-specific fields.

- [ ] **Step 4: Run unit CI pieces green**

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_route -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_admission -- --nocapture
```

- [ ] **Step 5: Run the ignored E2E against a local relay**

```bash
. ./bin/activate-hermit && just setup
. ./bin/activate-hermit && cargo build --release -p buzz-relay
# In another Hermit-activated terminal: ./target/release/buzz-relay
. ./bin/activate-hermit && cargo test -p buzz-test-client --test e2e_workflow_project_channel_routing -- --ignored --nocapture
```

Expected: PASS on a migrated relay with the owner key able to create channels.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit && git add Justfile scripts/run-tests.sh crates/buzz-test-client/Cargo.toml crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs crates/buzz-workflow/src/schema.rs Cargo.lock && git commit -s -m "test(workflow): cover static compatibility and project-channel webhook routing"
```

---

## Acceptance Criteria Mapping

| Spec criterion | Task |
| --- | --- |
| 1. Static workflow regression | Tasks 1, 6, 7, 8 |
| 2. One workflow routes `agentic-os-plan` and `harness-service` | Tasks 4, 6, 8 |
| 3. Tier order, exact comparison, unique-first-tier stop | Task 4 |
| 4. Alias is live coordinate only and grants no authority | Tasks 1, 4, 5 |
| 5. Zero or multiple claim-valid projects reject | Tasks 4, 6, 8 |
| 6. Missing/malformed/archived/cross-community/unauthorized channel posts nothing | Tasks 6, 8 |
| 7. Stale route fails the existing run and does not reroute | Tasks 7, 8 |
| 8. Concurrent same key/payload is one run, one execution, one message | Tasks 2, 6, 8 |
| 9. Same key different payload is `409` | Tasks 2, 6, 8 |
| 10. Raw idempotency key absent everywhere listed | Tasks 1, 6, 7, 8 |
| 11. Successful root has repo, project, run, channel, workflow, owner provenance | Tasks 7, 8 |
| 12. Stable redacted codes, no fallback destination, no error posts | Tasks 1, 6, 8 |
| 13. No Archon/GitHub/agent binding | All tasks; grep gate below |
| 14. No outbound GitHub comment artifacts | File map and grep gate |

## Open Questions

The approved specification leaves these parsing or channel-type details unstated.
Use the safe provisional defaults below unless the product owner changes them before implementation.

1. Clone values that are not RFC 3986 URLs.
Default: contribute the exact final `/` segment only when a slash exists; otherwise contribute no identity.

2. Whether a project may route to a DM channel.
Default: allow stream, forum, or DM rows because the approved spec restricts only liveness, community, archive state, and membership, and the existing sink already accepts those rows.

3. Multiple repository `name` tags on legacy or non-Buzz announcements.
Default: use the first `name` value, matching existing first-tag metadata parsing.

## Validation Commands

Run from the repository root after Hermit activation:

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib project_heads::tests -- --ignored --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib workflow::tests::admission_ -- --ignored --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_route -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_admission -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_sink -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_sink::integration_tests::dynamic_route_ -- --ignored --nocapture
. ./bin/activate-hermit && cargo check -p buzz-relay -p buzz-workflow -p buzz-db
. ./bin/activate-hermit && ! rg -n 'SendMessage.*\{text\}|SendDm.*\{to\}|AddReaction.*\{emoji\}|CallWebhook.*\{url\}|RequestApproval.*\{message\}|Delay.*\{duration\}' crates/buzz-workflow/src/executor.rs
. ./bin/activate-hermit && just test-unit
```

Relay-backed acceptance:

```bash
. ./bin/activate-hermit && cargo test -p buzz-test-client --test e2e_workflow_project_channel_routing -- --ignored --nocapture
```

Repository gates after the targeted tests:

```bash
. ./bin/activate-hermit && just test
. ./bin/activate-hermit && just ci
. ./bin/activate-hermit && git diff --check
```

Scope grep before handoff.
These must find no outbound GitHub comment delivery, no new `/hooks` route, and no provider/agent binding in the new modules:

```bash
. ./bin/activate-hermit && ! rg -n "github.com/.*/issues/.*/comments|Hermes|Codex|Grok|Archon" crates/buzz-workflow/src/routing.rs crates/buzz-relay/src/workflow_route.rs crates/buzz-relay/src/workflow_admission.rs crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs
. ./bin/activate-hermit && test "$(rg -F -c '"/hooks/{id}"' crates/buzz-relay/src/router.rs)" -eq 1
. ./bin/activate-hermit && rg -n 'tracing::|metrics::' crates/buzz-relay/src/workflow_admission.rs
```

The router must still have exactly the existing `"/hooks/{id}"` registration.
Every `tracing` and `metrics` hit in `workflow_admission.rs` must be inside `record_admission_observation` and consume only `AdmissionObservation` fields.
Run `detect_changes({ scope: "compare", base_ref: "main" })` and require only the files and workflow execution flows in this plan's File Map.
If GitNexus is unavailable, record that fact and attach `git diff --stat`, `git diff --name-only`, and `git diff --check` output to the handoff.

## Spec Coverage Self-Review

- Repository identity tiers, exact comparison, and fail-closed ambiguity: Task 4.
- Alias save-time live head and admission-time revalidation only on alias match: Tasks 4 and 5.
- Claim-valid counting before channel quality: Task 4.
- Admission order, snapshot immutability, and idempotency: Tasks 2 and 6.
- Side-effect revalidation without reroute: Task 7.
- Provenance tags and redaction: Tasks 1, 6, 7, 8.
- Static compatibility: Tasks 1, 6, 7, 8.
- HTTP status table: Task 6.
- No new endpoint/service/UI/provider adapter: file map and grep gate.
