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
- Do not change Archon, GitHub, the vault, desktop, mobile, web, or CLI.
- Do not implement Buzz-thread-to-GitHub-comment delivery or any GitHub token/outbound step.
- Do not bind routing to Archon, Hermes, Codex, Grok, or any other named provider or agent.
- Absence of `routing` is the compatibility switch for every existing workflow.
- Dynamic routing is valid only with `trigger.on: webhook` and `routing.mode: project_channel_by_repository`.
- For a dynamic workflow, every `send_message.channel` field is invalid.
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
- Activate Hermit in every shell with `. ./bin/activate-hermit && ...`.
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
- Store the route snapshot only after complete successful resolution and authorization.
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
- Clone values that parse as `url::Url` contribute only the last non-empty path segment.
- Clone values that do not parse as a URL contribute the last `/` segment when a `/` exists, otherwise no identity.
- Exactly one terminal `.git` suffix is removed from that segment.
- A missing, empty, or `.git`-only segment contributes no identity.
- Trailing-slash URLs whose last path segment is empty contribute no identity.
- Display name is the first `name` tag value only.
- Every `clone` tag contributes every value after the tag name.
- Every `maintainers` tag contributes every value after the tag name, compared exactly with no case folding.
- `buzz-visibility` is unlisted only when the first value is exactly `unlisted`.
- Absent or any other visibility token is listed.
- Destination channels may be stream, forum, or DM as long as they are live, not archived, in the run community, and the workflow owner is a current member.
- YAML duplicate alias keys follow serde last-wins because `BTreeMap` cannot retain duplicates after parse.
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
| Modify `crates/buzz-db/src/workflow.rs` | Run record fields and `admit_idempotent_workflow_run` |
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
| Create `crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs` | Relay-backed webhook routing flow |

## Out of Scope

- Desktop Open Discussion / Channels-tab routing.
- New kind integers.
- Materialized repository-to-channel projections.
- General exactly-once execution beyond webhook admission and run idempotency.
- Recursive workflow triggering changes beyond the existing `buzz:workflow` marker.
- Agent wake policy beyond ordinary `p` mentions.

---

### Task 1: Routing schema, control-plane hashing, and failure codes

**Files:**

- Create: `crates/buzz-workflow/src/routing.rs`
- Modify: `crates/buzz-workflow/src/schema.rs`
- Modify: `crates/buzz-workflow/src/error.rs`
- Modify: `crates/buzz-workflow/src/lib.rs`
- Modify: `crates/buzz-workflow/Cargo.toml`

**Interfaces:**

- Consumes: existing `WorkflowDef`, `TriggerDef`, `ActionDef`, `WorkflowError`, `parse_yaml`.
- Produces:
  - `pub struct RoutingDef { pub mode: RoutingMode, pub aliases: BTreeMap<String, String> }`
  - `pub enum RoutingMode { ProjectChannelByRepository }` serialized as `project_channel_by_repository`
  - `pub enum RouteFailure` with `code()`, `http_status()`, and `redacted_message()`
  - `pub struct DynamicControlFields { pub repository_name: String, pub idempotency_key: String }`
  - `pub fn parse_repository_coordinate(coord: &str) -> Result<(), WorkflowError>`
  - `pub fn parse_dynamic_control_fields(body: &serde_json::Value) -> Result<DynamicControlFields, RouteFailure>`
  - `pub fn strip_idempotency_key(body: serde_json::Value) -> serde_json::Value`
  - `pub fn hash_idempotency_key(raw: &str) -> [u8; 32]`
  - `pub fn canonical_payload_hash(body: &serde_json::Value) -> Result<[u8; 32], WorkflowError>`
  - `impl WorkflowDef { pub fn has_project_channel_routing(&self) -> bool }`

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `WorkflowDef`, `WorkflowDef::validate`, and `parse_yaml`.
Report callers and risk.
The expected callers are YAML parse sites and webhook/event execution.
Stop only if impact shows git, CLI, or UI writers.

- [ ] **Step 2: Add `sha2` and the module skeleton, then write failing tests**

In `crates/buzz-workflow/Cargo.toml` add `sha2 = { workspace = true }` next to the other workspace crates.
In `crates/buzz-workflow/src/lib.rs` add `pub mod routing;` and `pub use routing::{DynamicControlFields, RouteFailure, RoutingDef, RoutingMode};`.
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
```

Add these tests in `routing.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn control_fields_require_non_empty_strings() {
        let err = parse_dynamic_control_fields(&json!({"repository_name": "", "idempotency_key": "k"}))
            .unwrap_err();
        assert_eq!(err, RouteFailure::InvalidControlFields);
        assert_eq!(err.code(), "invalid_control_fields");
        assert_eq!(err.http_status(), 422);
        assert!(!err.redacted_message().contains("agentic-os-plan"));
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
        let ok = format!("30617:{}:agentic-os-plan", "ab".repeat(32));
        parse_repository_coordinate(&ok).expect("valid coordinate");
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
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib parse_project_channel_routing_with_empty_aliases -- --exact --nocapture
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
    #[serde(default)]
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
    #[serde(default)]
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
}
```

`parse_repository_coordinate` must require literal `30617`, 64 lowercase hex owner, and non-empty `d` after the first two colons.
Uppercase hex is rejected, never folded.

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

### Task 2: Idempotent workflow-run persistence

**Files:**

- Create: `migrations/0032_workflow_run_route_idempotency.sql`
- Modify: `schema/schema.sql`
- Modify: `crates/buzz-db/src/workflow.rs`
- Modify: `crates/buzz-db/src/lib.rs`

**Interfaces:**

- Consumes: existing `create_workflow_run`, `WorkflowRunRecord`, `WorkflowRunFailure`, `RunStatus`.
- Produces:
  - `pub struct WorkflowRouteSnapshot { pub community_id: Uuid, pub repository_coordinate: String, pub project_coordinate: String, pub channel_id: Uuid, pub matched_identity_tier: String }`
  - `pub enum AdmitWorkflowRun { Created(WorkflowRunRecord), Existing(WorkflowRunRecord), PayloadConflict { existing: WorkflowRunRecord } }`
  - `pub async fn admit_idempotent_workflow_run(pool, community_id, workflow_id, idempotency_key_hash: &[u8], payload_hash: &[u8], trigger_context: Option<&Value>, route_snapshot: Option<&WorkflowRouteSnapshot>, status: RunStatus, failure: Option<WorkflowRunFailure<'_>>) -> Result<AdmitWorkflowRun>`
- `create_workflow_run` signature stays unchanged for static, event, and cron paths.

- [ ] **Step 1: Run impact checks**

Run GitNexus upstream impact for `create_workflow_run`, `get_workflow_run`, `row_to_run_record`, and `WorkflowRunRecord`.
Event, cron, and webhook callers must keep compiling against the old `create_workflow_run` signature.

- [ ] **Step 2: Write failing unit tests for snapshot parsing and admit outcome types**

In `workflow.rs` tests, add:

```rust
    #[test]
    fn route_snapshot_round_trips_json() {
        let channel_id = Uuid::parse_str("9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50").unwrap();
        let snapshot = WorkflowRouteSnapshot {
            community_id: channel_id,
            repository_coordinate: format!("30617:{}:agentic-os-plan", "ab".repeat(32)),
            project_coordinate: format!("30621:{}:gigo-harness", "cd".repeat(32)),
            channel_id,
            matched_identity_tier: "d_tag".into(),
        };
        let value = serde_json::to_value(&snapshot).unwrap();
        let parsed: WorkflowRouteSnapshot = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.repository_coordinate, snapshot.repository_coordinate);
        assert_eq!(parsed.matched_identity_tier, "d_tag");
    }
```

This fails until `WorkflowRouteSnapshot` exists.

- [ ] **Step 3: Write the migration and schema.sql, then add ignored Postgres tests**

Create `migrations/0032_workflow_run_route_idempotency.sql`:

```sql
-- Immutable webhook route snapshot and control-plane idempotency hashes.
-- Raw idempotency keys are never stored. NULL hashes keep static runs unrestricted.
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
        CHECK ((idempotency_key_hash IS NULL) = (payload_hash IS NULL));

CREATE UNIQUE INDEX idx_workflow_runs_idempotency
    ON workflow_runs (community_id, workflow_id, idempotency_key_hash)
    WHERE idempotency_key_hash IS NOT NULL;
```

Copy the same columns, checks, and unique index into `schema/schema.sql` on the `workflow_runs` table next to `error_code`.

Add ignored Postgres tests in the existing `workflow.rs` `mod tests`, reusing `setup_pool`, `make_community`, and `make_workflow_in`.
Do not add a second `TEST_DB_URL` or `setup_pool`.

```rust
    async fn count_runs(pool: &PgPool, community: CommunityId, workflow_id: Uuid) -> i64 {
        let row = sqlx::query(
            "SELECT COUNT(*)::bigint AS n FROM workflow_runs \
             WHERE community_id = $1 AND workflow_id = $2",
        )
        .bind(community.as_uuid())
        .bind(workflow_id)
        .fetch_one(pool)
        .await
        .expect("count runs");
        row.get("n")
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn admit_same_key_same_payload_returns_existing_without_second_row() {
        let pool = setup_pool().await;
        let community = make_community(&pool).await;
        let (workflow_id, _) = make_workflow_in(&pool, community).await;
        let key = [1u8; 32];
        let payload = [9u8; 32];
        let first = admit_idempotent_workflow_run(
            &pool,
            community,
            workflow_id,
            &key,
            &payload,
            None,
            None,
            RunStatus::Pending,
            None,
        )
        .await
        .expect("first admit");
        let AdmitWorkflowRun::Created(created) = first else {
            panic!("first admit must create");
        };
        let second = admit_idempotent_workflow_run(
            &pool,
            community,
            workflow_id,
            &key,
            &payload,
            None,
            None,
            RunStatus::Pending,
            None,
        )
        .await
        .expect("second admit");
        let AdmitWorkflowRun::Existing(existing) = second else {
            panic!("second admit must reuse");
        };
        assert_eq!(created.id, existing.id);
        assert_eq!(count_runs(&pool, community, workflow_id).await, 1);
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn admit_same_key_different_payload_is_conflict() {
        let pool = setup_pool().await;
        let community = make_community(&pool).await;
        let (workflow_id, _) = make_workflow_in(&pool, community).await;
        let key = [2u8; 32];
        let first = admit_idempotent_workflow_run(
            &pool,
            community,
            workflow_id,
            &key,
            &[9u8; 32],
            None,
            None,
            RunStatus::Pending,
            None,
        )
        .await
        .expect("first admit");
        let created_id = match first {
            AdmitWorkflowRun::Created(run) => run.id,
            other => panic!("expected Created, got {other:?}"),
        };
        let second = admit_idempotent_workflow_run(
            &pool,
            community,
            workflow_id,
            &key,
            &[8u8; 32],
            None,
            None,
            RunStatus::Pending,
            None,
        )
        .await
        .expect("conflict admit");
        match second {
            AdmitWorkflowRun::PayloadConflict { existing } => {
                assert_eq!(existing.id, created_id);
                assert_eq!(existing.payload_hash.as_deref(), Some(&[9u8; 32][..]));
            }
            other => panic!("expected PayloadConflict, got {other:?}"),
        }
        assert_eq!(count_runs(&pool, community, workflow_id).await, 1);
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn concurrent_same_key_inserts_converge_on_one_row() {
        let pool = setup_pool().await;
        let community = make_community(&pool).await;
        let (workflow_id, _) = make_workflow_in(&pool, community).await;
        let key = [3u8; 32];
        let payload = [7u8; 32];
        let left = {
            let pool = pool.clone();
            async move {
                admit_idempotent_workflow_run(
                    &pool,
                    community,
                    workflow_id,
                    &key,
                    &payload,
                    None,
                    None,
                    RunStatus::Pending,
                    None,
                )
                .await
            }
        };
        let right = {
            let pool = pool.clone();
            async move {
                admit_idempotent_workflow_run(
                    &pool,
                    community,
                    workflow_id,
                    &key,
                    &payload,
                    None,
                    None,
                    RunStatus::Pending,
                    None,
                )
                .await
            }
        };
        let (a, b) = tokio::join!(left, right);
        let a = a.expect("left admit");
        let b = b.expect("right admit");
        let id_a = match &a {
            AdmitWorkflowRun::Created(run) | AdmitWorkflowRun::Existing(run) => run.id,
            AdmitWorkflowRun::PayloadConflict { existing } => existing.id,
        };
        let id_b = match &b {
            AdmitWorkflowRun::Created(run) | AdmitWorkflowRun::Existing(run) => run.id,
            AdmitWorkflowRun::PayloadConflict { existing } => existing.id,
        };
        assert_eq!(id_a, id_b);
        assert_eq!(count_runs(&pool, community, workflow_id).await, 1);
    }
```

- [ ] **Step 4: Run the ignored tests and confirm they fail**

```bash
. ./bin/activate-hermit && cargo test -p buzz-db --lib admit_same_key_same_payload_returns_existing_without_second_row -- --exact --ignored --nocapture
```

Expected: FAIL on missing columns/function until the migration is applied and the function exists.
If the local database has not applied `0032`, the insert errors with `column "idempotency_key_hash" does not exist`.

- [ ] **Step 5: Implement record fields and admit**

Extend `WorkflowRunRecord` with:

```rust
    /// SHA-256 of the raw control-plane idempotency key. Never the raw key.
    pub idempotency_key_hash: Option<Vec<u8>>,
    /// SHA-256 of canonical callback JSON after removing `idempotency_key`.
    pub payload_hash: Option<Vec<u8>>,
    /// Immutable server-produced route. Present only after authorized admission.
    pub route_snapshot: Option<WorkflowRouteSnapshot>,
```

Update every `SELECT` that builds a run record, including `get_workflow_run` and `list_workflow_runs_page`, to include `idempotency_key_hash`, `payload_hash`, and `route_snapshot`.
Update `row_to_run_record` to deserialize `route_snapshot` with `serde_json::from_value` when the JSONB is not NULL.

Leave `create_workflow_run` inserting NULLs for the new columns.

Implement `admit_idempotent_workflow_run` as:

1. `INSERT INTO workflow_runs (community_id, id, workflow_id, status, current_step, execution_trace, trigger_context, idempotency_key_hash, payload_hash, route_snapshot, error_code, error_message, completed_at) VALUES ($1, $2, $3, $4::run_status, 0, '[]', $5, $6, $7, $8, $9, $10, CASE WHEN $4 IN ('failed','completed','cancelled') THEN NOW() ELSE NULL END) ON CONFLICT (community_id, workflow_id, idempotency_key_hash) WHERE idempotency_key_hash IS NOT NULL DO NOTHING RETURNING id`.
2. If a row is returned, `get_workflow_run` and return `AdmitWorkflowRun::Created`.
3. If not, `SELECT` the existing row by `(community_id, workflow_id, idempotency_key_hash)`.
4. If `existing.payload_hash == payload_hash`, return `Existing`.
5. Otherwise return `PayloadConflict` without `UPDATE`.

Bind hashes as `&[u8]` of length 32.
Serialize `route_snapshot` with `serde_json::to_value`.

Add matching `Db::admit_idempotent_workflow_run` in `lib.rs` with `#[datastore_span(name = "admit_idempotent_workflow_run", system = "postgresql")]`.

- [ ] **Step 6: Apply migration locally and run tests green**

```bash
. ./bin/activate-hermit && cargo test -p buzz-db --lib route_snapshot_round_trips_json -- --exact --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib admit_same_key_same_payload_returns_existing_without_second_row admit_same_key_different_payload_is_conflict concurrent_same_key_inserts_converge_on_one_row -- --ignored --nocapture
```

Expected: PASS after `just setup` or the relay migrator has applied `0032`.

- [ ] **Step 7: Commit**

```bash
. ./bin/activate-hermit && git add migrations/0032_workflow_run_route_idempotency.sql schema/schema.sql crates/buzz-db/src/workflow.rs crates/buzz-db/src/lib.rs && git commit -s -m "feat(db): persist workflow route snapshots and idempotency hashes"
```

---

### Task 3: Unlimited latest live repository and project heads

**Files:**

- Create: `crates/buzz-db/src/project_heads.rs`
- Modify: `crates/buzz-db/src/lib.rs`

**Interfaces:**

- Consumes: `CommunityId`, `StoredEvent`, `row_to_stored_event`, kinds `30617` and `30621`.
- Produces:
  - `pub async fn list_latest_parameterized_heads(pool, community_id, kind: i32) -> Result<Vec<StoredEvent>>`
  - `pub async fn get_latest_parameterized_head(pool, community_id, kind: i32, pubkey: &[u8], d_tag: &str) -> Result<Option<StoredEvent>>`
- No `LIMIT` clause on the list query.
- Latest head ordering is `created_at DESC, id ASC` via `DISTINCT ON (pubkey, d_tag)`.
- Live means `deleted_at IS NULL`.
- Scope to `channel_id IS NULL` because these kinds are global-only.

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

    fn repo_event(keys: &Keys, d: &str, name: &str, created_at: u64) -> nostr::Event {
        EventBuilder::new(Kind::from(30617), "")
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
        let older = repo_event(&keys_a, "agentic-os-plan", "old", base);
        let newer = repo_event(&keys_a, "agentic-os-plan", "new", base + 5);
        let harness = repo_event(&keys_b, "harness-service", "harness-service", base + 1);
        let extra = repo_event(&keys_c, "extra-repo", "extra-repo", base + 2);
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
    async fn list_heads_excludes_deleted_rows() {
        let pool = setup_pool().await;
        let community = make_community(&pool).await;
        let keys = Keys::generate();
        let event = repo_event(&keys, "agentic-os-plan", "agentic-os-plan", chrono::Utc::now().timestamp() as u64);
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
        let event = repo_event(&keys, "agentic-os-plan", "agentic-os-plan", chrono::Utc::now().timestamp() as u64);
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
ORDER BY created_at DESC, id ASC
LIMIT 1
```

Reuse `event::row_to_stored_event` if it is `pub(crate)`.
If it is private, add a `pub(crate)` re-export rather than duplicating decode.

Declare `pub mod project_heads;` in `lib.rs` and wrap both functions on `Db` with datastore spans `list_latest_parameterized_heads` and `get_latest_parameterized_head`.

Do not call `query_events` here.
`EventQuery` applies `DEFAULT_MAX_PAGE_LIMIT` of 1000, which violates NIP-MP completeness.

- [ ] **Step 4: Run tests green and commit**

```bash
. ./bin/activate-hermit && cargo test -p buzz-db --lib project_heads -- --ignored --nocapture
```

```bash
. ./bin/activate-hermit && git add crates/buzz-db/src/project_heads.rs crates/buzz-db/src/lib.rs && git commit -s -m "feat(db): list latest live repository and project heads"
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
  - `pub fn authorize_unique_project_route(repository: &RepositoryHead, projects: &[ProjectHead]) -> Result<&'a ProjectHead, RouteFailure>` counting claim-valid projects before inspecting channel quality

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

Also add a test that one workflow-style pair of heads named `agentic-os-plan` and `harness-service` can each uniquely resolve when they share no ambiguous `d` tag.

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

`repository_head_from_event` reads tags with `tag.as_slice()`.
`d` is required.
`name` is the first `name` value.
Clone basenames come from every value after every `clone` tag name.
Maintainers come from every value after every `maintainers` tag name.
Owner is `event.pubkey.to_hex()`.
Coordinate is `format!("30617:{owner}:{d}")`.

`project_head_from_event` requires `d`.
Members are `a` tag values that `ProjectMemberCoord::parse_full` accepts.
`listed` is false only when the first `buzz-visibility` value is exactly `unlisted`.
`buzz_channel` is the first `buzz-channel` value.

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

- Modify: `crates/buzz-relay/src/handlers/command_executor.rs` around the workflow YAML parse in `handle_workflow_def` (currently after `parse_yaml` near lines 743-746)

**Interfaces:**

- Consumes: `WorkflowDef::has_project_channel_routing`, `parse_repository_coordinate`, `Db::get_latest_parameterized_head`.
- Produces: ingest rejection `invalid: routing alias '<key>' does not name a live repository in this community` when a target is missing.
- Does not grant claim or channel authority at save time.

- [ ] **Step 1: Run impact for `handle_workflow_def`**

Report callers.
This is the workflow YAML save path.
Do not change webhook-secret injection order.

- [ ] **Step 2: Extract save-time validation so the error text can be unit-tested**

Add `pub(crate) fn missing_alias_target_error(alias_key: &str) -> String` in `command_executor.rs` and test:

```rust
    #[test]
    fn missing_alias_error_names_the_key_without_candidate_lists() {
        let msg = missing_alias_target_error("hs");
        assert!(msg.contains("hs"));
        assert!(!msg.contains("30617:"));
        assert!(!msg.contains("agentic-os-plan"));
    }
```

The live-head lookup stays in `handle_workflow_def` and is covered by Task 8 if an E2E save-rejection is added.
Add that E2E as `alias_target_must_be_live_at_save` in Task 8.

- [ ] **Step 3: Implement after successful `parse_yaml` and before secret injection**

```rust
    if def.has_project_channel_routing() {
        let Some(routing) = def.routing.as_ref() else {
            unreachable!("has_project_channel_routing implies routing");
        };
        for (alias_key, target) in &routing.aliases {
            let (owner_hex, d_tag) = match parse_alias_target(target) {
                Ok(parts) => parts,
                Err(_) => {
                    return Err(IngestError::Rejected(
                        "invalid: routing alias target must be a live 30617 coordinate".into(),
                    ));
                }
            };
            let owner_bytes = hex::decode(&owner_hex).map_err(|_| {
                IngestError::Rejected("invalid: routing alias target owner is not hex".into())
            })?;
            let head = state
                .db
                .get_latest_parameterized_head(
                    tenant.community(),
                    buzz_core::kind::KIND_GIT_REPO_ANNOUNCEMENT as i32,
                    &owner_bytes,
                    &d_tag,
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

- [ ] **Step 4: Compile and commit**

```bash
. ./bin/activate-hermit && cargo test -p buzz-relay --lib -- --nocapture
```

Relay `--lib` may include Redis-backed bridge tests.
If those fail for missing Redis, run the compiler gate instead:

```bash
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_route -- --nocapture && cargo check -p buzz-relay
```

```bash
. ./bin/activate-hermit && git add crates/buzz-relay/src/handlers/command_executor.rs && git commit -s -m "feat(relay): reject workflow aliases whose repository targets are not live"
```

---

### Task 6: Webhook admission

**Files:**

- Create: `crates/buzz-relay/src/workflow_admission.rs`
- Modify: `crates/buzz-relay/src/api/bridge.rs` (`workflow_webhook` near lines 1873-2046)
- Modify: `crates/buzz-relay/src/api/mod.rs`
- Modify: `crates/buzz-relay/src/lib.rs`

**Interfaces:**

- Consumes: host binding, `get_workflow`, webhook secret verify, `check_owner_authority`, `list_latest_parameterized_heads`, `resolve_repository_identity`, `authorize_unique_project_route`, `admit_idempotent_workflow_run`, `get_channel`, `get_member_role`.
- Produces: HTTP `202` / `400` / `401` / `404` / `409` / `422` / `503` as specified.
- Spawns `execute_from_step` only for a newly created `pending` run.

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

In `workflow_admission.rs` tests, cover the pure mapping helpers:

```rust
#[test]
fn route_failure_conflict_is_409() {
    assert_eq!(RouteFailure::IdempotencyConflict.http_status(), 409);
    assert_eq!(RouteFailure::IdempotencyConflict.code(), "idempotency_conflict");
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

- [ ] **Step 3: Run those tests red, then implement admission**

Keep `pub async fn workflow_webhook` in `bridge.rs` as:

```rust
pub async fn workflow_webhook(
    State(state): State<Arc<AppState>>,
    Path(id_str): Path<String>,
    Query(query): Query<WebhookQuery>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    crate::workflow_admission::handle_workflow_webhook(state, id_str, query, headers, body).await
}
```

Move the current function body into `handle_workflow_webhook` and add the dynamic branch after secret verification and owner home-channel authority, matching this order:

1. Bind host to community (already done).
2. Load workflow in that community; generic `404` on miss.
3. Require `TriggerDef::Webhook` (`400` as today if not).
4. Verify secret (`401` as today).
5. Parse JSON: empty body stays `None` for static; invalid JSON is `400`.
6. If `!def.has_project_channel_routing()`, keep the current trigger-context + `create_workflow_run` + spawn path with no repository or project queries.
7. If dynamic:
   1. Require `body` to be a JSON object (`422` `invalid_control_fields`, no run).
   2. `parse_dynamic_control_fields` (`422`, no run if unusable key).
   3. Strip `idempotency_key` and build `trigger_ctx.webhook_fields` from the stripped object only.
   4. Hash the raw key and the stripped payload.
   5. Load latest live `30617` heads with `list_latest_parameterized_heads`.
   6. Map them through `repository_head_from_event`.
   7. `resolve_repository_identity`.
   8. Load latest live `30621` heads with no limit.
   9. Map through `project_head_from_event`.
   10. `authorize_unique_project_route`.
   11. Parse `buzz_channel` as UUID; `get_channel` in the request community; reject if missing, `archived_at.is_some()`, or `deleted_at` already excluded.
   12. `get_member_role` for the workflow owner on that destination channel; `None` is `route_unauthorized` even when `visibility == "open"`.
   13. Build `WorkflowRouteSnapshot`.
   14. `admit_idempotent_workflow_run` with `Pending` and the snapshot.
   15. On `Created`, spawn execution exactly as today.
   16. On `Existing` whose `status` is `failed` and whose `error_code` is one of the admission `422` codes, replay `422` with that code and its redacted message and do not spawn.
   17. On any other `Existing`, return `202` with the existing `run_id` / `status` and do not spawn.
   18. On `PayloadConflict`, return `409` `idempotency_conflict` and do not `UPDATE`.

Map deterministic route errors after a usable key to `admit_idempotent_workflow_run` with `RunStatus::Failed`, `route_snapshot: None`, and `WorkflowRunFailure { code, message: redacted }`.
Then return `422`.

If the database errors before a run exists, return `503` with `{ "error": "service unavailable" }` and no code that reveals internals.
Do not use `internal_error`'s `500` on that path.

Logs may include `run_id`, `workflow_id`, `community_id`, outcome, and failure code.
Logs must not include the raw key, secret, callback body, or candidate coordinate lists.

Increment a low-cardinality counter:

```rust
metrics::counter!(
    "buzz_workflow_webhook_admission_total",
    "outcome" => outcome, // accepted | conflict | rejected | unavailable
    "code" => code        // none | invalid_control_fields | repository_missing | ...
).increment(1);
```

- [ ] **Step 4: Run unit tests and `cargo check`**

```bash
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_admission -- --nocapture && cargo check -p buzz-relay
```

- [ ] **Step 5: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-relay/src/workflow_admission.rs crates/buzz-relay/src/api/bridge.rs crates/buzz-relay/src/api/mod.rs crates/buzz-relay/src/lib.rs && git commit -s -m "feat(relay): admit webhook callbacks onto an immutable project channel route"
```

---

### Task 7: Snapshot send_message, revalidation, and provenance

**Files:**

- Modify: `crates/buzz-workflow/src/action_sink.rs`
- Modify: `crates/buzz-workflow/src/executor.rs`
- Modify: `crates/buzz-workflow/src/error.rs`
- Modify: `crates/buzz-relay/src/workflow_sink.rs`
- Modify: `crates/buzz-relay/src/workflow_route.rs`

**Interfaces:**

- Consumes: `WorkflowRunRecord.route_snapshot`, existing sink membership and kind `9` builder.
- Produces:
  - `pub struct WorkflowMessageRoute { pub run_id: Uuid, pub workflow_id: Uuid, pub home_channel_id: Uuid, pub repository_coordinate: String, pub project_coordinate: String }`
  - `ActionSink::send_message(..., route: Option<WorkflowMessageRoute>)`
  - `WorkflowError::RouteStale` with `code() == "route_stale"`
  - Provenance tags on success: `a` repository, `a` project, `buzz:workflow-run` = run UUID

- [ ] **Step 1: Run impact for `ActionSink::send_message`, `resolve_send_message_channel`, and `RelayActionSink::send_message`**

The only `ActionSink` implementor is `RelayActionSink`.
The only production caller is `dispatch_action` in `executor.rs`.

- [ ] **Step 2: Write failing executor tests**

Replace the channel helper usage with a wrapper that sees a snapshot:

```rust
    #[test]
    fn dynamic_send_message_uses_snapshot_channel_and_rejects_override() {
        let snapshot_channel = Uuid::new_v4();
        let snapshot = buzz_db::workflow::WorkflowRouteSnapshot {
            community_id: Uuid::new_v4(),
            repository_coordinate: format!("30617:{}:agentic-os-plan", "ab".repeat(32)),
            project_coordinate: format!("30621:{}:gigo-harness", "cd".repeat(32)),
            channel_id: snapshot_channel,
            matched_identity_tier: "d_tag".into(),
        };
        let resolved = resolve_send_message_channel_for_run(
            None,
            "",
            Some(Uuid::new_v4()),
            Some(&snapshot),
        )
        .expect("snapshot wins");
        assert_eq!(resolved, snapshot_channel.to_string());

        let err = resolve_send_message_channel_for_run(
            Some(&Uuid::new_v4().to_string()),
            "",
            Some(Uuid::new_v4()),
            Some(&snapshot),
        )
        .unwrap_err();
        assert!(matches!(err, WorkflowError::InvalidDefinition(_)));
    }

    #[test]
    fn static_send_message_channel_rules_remain_unchanged() {
        let workflow_channel_id = Uuid::new_v4();
        let resolved = resolve_send_message_channel_for_run(
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

Add a sink unit test that a tag list helper emits the three provenance tags and never includes an `idempotency_key` string.

- [ ] **Step 3: Run tests red**

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib dynamic_send_message_uses_snapshot_channel_and_rejects_override -- --exact --nocapture
```

Expected: FAIL on missing wrapper.

- [ ] **Step 4: Implement executor + sink**

```rust
fn resolve_send_message_channel_for_run(
    explicit_channel: Option<&str>,
    trigger_channel: &str,
    workflow_channel_id: Option<Uuid>,
    route_snapshot: Option<&buzz_db::workflow::WorkflowRouteSnapshot>,
) -> Result<String, WorkflowError> {
    if let Some(snapshot) = route_snapshot {
        let explicit_channel = explicit_channel
            .map(str::trim)
            .filter(|value| !value.is_empty());
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

Add to `WorkflowError`:

```rust
    /// Dynamic route lost authority between admission and the side effect.
    #[error("route stale: {0}")]
    RouteStale(String),
```

`code()` returns `"route_stale"`.

Map `ActionSinkError::RouteStale(msg)` to `WorkflowError::RouteStale(msg)` in `From`.
Do not map it through `WebhookError`.

In the sink, when `route` is `Some`, immediately before event construction call `revalidate_dynamic_route(...)` which must:

1. Reload the workflow; require `enabled` and `status == Active`.
2. `check_owner_authority` on `home_channel_id`.
3. Load the stored repository head by coordinate; missing is stale.
4. Load latest project heads with no limit; rebuild `ProjectHead` values.
5. Require the stored project coordinate to still be the unique claim-valid project.
6. Require that project's `buzz-channel` still equals the send `channel_id`.
7. Require the channel still belongs to `community_id`, is live, and is not archived.
8. Require `get_member_role` for the workflow owner on the destination is `Some`, including open channels.

Any failed check returns `ActionSinkError::RouteStale` and must not call `insert_event_with_thread_metadata`.

On success, keep existing tags and append:

```rust
Tag::parse(["a", &route.repository_coordinate])
Tag::parse(["a", &route.project_coordinate])
Tag::parse(["buzz:workflow-run", &route.run_id.to_string()])
```

Do not add the raw idempotency key, alias map, or callback body.

For `route == None`, keep today's open-channel exception: non-members may post only when `visibility == "open"`.
Dynamic routes must not use that exception.

- [ ] **Step 5: Run tests green**

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_route -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_sink -- --nocapture
```

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit && git add crates/buzz-workflow/src/action_sink.rs crates/buzz-workflow/src/executor.rs crates/buzz-workflow/src/error.rs crates/buzz-relay/src/workflow_sink.rs crates/buzz-relay/src/workflow_route.rs && git commit -s -m "feat(workflow): revalidate snapshot routes and tag project provenance"
```

---

### Task 8: Static compatibility, CI unit wiring, and relay-backed E2E

**Files:**

- Modify: `Justfile` (`test-unit`)
- Modify: `scripts/run-tests.sh` (`run_unit_tests`)
- Create: `crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs`
- Modify: `crates/buzz-workflow/src/schema.rs` if any extra static regression test is still missing

**Interfaces:**

- Consumes: live relay as in [TESTING.md](../../../TESTING.md), kind `30620` workflow save, `POST /hooks/{id}`, kinds `30617`/`30621`/`9`.
- Produces: ignored E2E coverage for dual-repository routing, ambiguity, unauthorized membership, idempotency, and static non-routing webhooks.

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

- [ ] **Step 2: Write the static compatibility unit test if not already present**

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
        assert!(!json.contains("project_channel_by_repository"));
        assert!(!json.contains("idempotency_key"));
    }
```

Run it red if needed, then green.

- [ ] **Step 3: Write the ignored E2E spec**

Create `crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs` modeled on `e2e_project.rs` and the webhook save/response parsing in `conformance_multitenant.rs`.

Helpers:

- `relay_url()` / `relay_http_url()` as in `e2e_relay.rs`.
- `unique(prefix)` for `d` tags.
- `define_webhook_workflow(http, keys, home_channel, yaml) -> (workflow_id, webhook_secret)` by publishing kind `30620` and parsing `response:{...}`.
- `post_hook(http, workflow_id, secret, body) -> (StatusCode, Value)` with `X-Webhook-Secret` and `Host` from the relay URL.

Tests, each `#[tokio::test] #[ignore]`:

1. `static_webhook_without_routing_still_posts_to_home_channel`
   YAML has no `routing`.
   Body may omit `repository_name` and `idempotency_key`.
   Assert `202` and a kind `9` in the home channel with `buzz:workflow=true` and without `buzz:workflow-run`.

2. `one_workflow_routes_agentic_os_plan_and_harness_service`
   Create destination channel `gigo-harness-{unique}` and add the workflow owner as a member.
   Publish `30617` repos whose `d` tags are `agentic-os-plan-{unique}` and `harness-service-{unique}`.
   Publish one listed `30621` named `gigo-harness` whose signer is the repo owner and whose `a` tags are both coordinates and whose `buzz-channel` is the destination UUID.
   Dynamic YAML uses `routing.mode: project_channel_by_repository` and one `send_message` without `channel`.
   POST twice with those `repository_name` values and distinct `idempotency_key`s.
   Assert both messages land in the destination channel, not the home channel, and each event has `a` tags for the matching repo coordinate and the project coordinate plus `buzz:workflow-run`.

3. `ambiguous_claim_valid_projects_return_422_and_no_message`
   Two listed owner-signed projects both include the same repo.
   Assert `422` / `project_ambiguous` and zero new kind `9` events in either project channel or the home channel.

4. `open_destination_without_membership_is_unauthorized`
   Destination channel is `open` but the owner is not a member.
   Assert `422` / `route_unauthorized` and no message.

5. `same_key_same_payload_does_not_send_a_second_message`
   POST twice concurrently with identical body.
   Assert one run id and exactly one kind `9`.

6. `same_key_different_payload_is_409`
   Second body changes `title`.
   Assert `409` / `idempotency_conflict` and still one message.

7. `idempotency_key_never_appears_on_the_event_or_run_context`
   After success, fetch the kind `9` tags and, if `DATABASE_URL` is available, the `workflow_runs.trigger_context` JSON as `e2e_relay.rs` does.
   Assert the raw key string is absent from tags, content, and `trigger_context`.

8. `alias_target_must_be_live_at_save`
   Define a dynamic workflow whose alias target coordinate has no live `30617` head.
   Assert the kind `30620` save is rejected and the OK message names the alias key without other repository coordinates.

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
# In another terminal: buzz-relay
. ./bin/activate-hermit && cargo test -p buzz-test-client --test e2e_workflow_project_channel_routing -- --ignored --nocapture
```

Expected: PASS on a migrated relay with the owner key able to create channels.

- [ ] **Step 6: Commit**

```bash
. ./bin/activate-hermit && git add Justfile scripts/run-tests.sh crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs crates/buzz-workflow/src/schema.rs && git commit -s -m "test(workflow): cover static compatibility and project-channel webhook routing"
```

---

## Acceptance Criteria Mapping

| Spec criterion | Task |
| --- | --- |
| 1. Static workflow regression | Tasks 1, 7, 8 |
| 2. One workflow routes `agentic-os-plan` and `harness-service` | Tasks 4, 6, 8 |
| 3. Tier order, exact comparison, unique-first-tier stop | Task 4 |
| 4. Alias is live coordinate only and grants no authority | Tasks 1, 4, 5 |
| 5. Zero or multiple claim-valid projects reject | Tasks 4, 6, 8 |
| 6. Missing/malformed/archived/cross-community/unauthorized channel posts nothing | Tasks 6, 8 |
| 7. Stale route fails the existing run and does not reroute | Task 7 |
| 8. Concurrent same key/payload is one run, one execution, one message | Tasks 2, 6, 8 |
| 9. Same key different payload is `409` | Tasks 2, 6, 8 |
| 10. Raw idempotency key absent everywhere listed | Tasks 1, 6, 8 |
| 11. Successful root has repo, project, run, channel, workflow, owner provenance | Task 7, 8 |
| 12. Stable redacted codes, no fallback destination, no error posts | Tasks 1, 6, 8 |
| 13. No Archon/GitHub/agent binding | All tasks; grep gate below |
| 14. No outbound GitHub comment artifacts | File map and grep gate |

## Open Questions

The following choices are not fully spelled as types in the spec.
The plan uses the defaults in Resolved Implementation Decisions.

1. Physical run columns versus a single structured type.
Default: two `BYTEA` hashes plus `JSONB route_snapshot` and a partial unique index.

2. Project coordinate spelling.
Default: `30621:<lowercase-hex>:<d>`.

3. Clone values that are not RFC 3986 URLs.
Default: last `/` segment only; otherwise no identity.

4. YAML duplicate alias keys.
Default: serde last-wins after parse.

5. Whether `422` bodies include a `code` field.
Default: yes, for dynamic webhook errors only.

6. Whether DM destinations are allowed.
Default: yes, if live, not archived, same community, and the owner is a member.

7. Multiple `name` tags.
Default: first value only.

8. Exact E2E `d` tags on a shared local relay.
Default: unique suffixes; unit tests keep the production names.

## Validation Commands

Run from the repository root after Hermit activation:

```bash
. ./bin/activate-hermit && cargo test -p buzz-workflow --lib -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-db --lib project_heads admit_same_key_same_payload_returns_existing_without_second_row admit_same_key_different_payload_is_conflict concurrent_same_key_inserts_converge_on_one_row -- --ignored --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_route -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_admission -- --nocapture
. ./bin/activate-hermit && cargo test -p buzz-relay --lib workflow_sink -- --nocapture
. ./bin/activate-hermit && cargo check -p buzz-relay -p buzz-workflow -p buzz-db
```

Relay-backed acceptance:

```bash
. ./bin/activate-hermit && cargo test -p buzz-test-client --test e2e_workflow_project_channel_routing -- --ignored --nocapture
```

Scope grep before handoff.
These must find no outbound GitHub comment delivery, no new `/hooks` route, and no provider/agent binding in the new modules:

```bash
. ./bin/activate-hermit && rg -n "github.com/.*/issues/.*/comments|Hermes|Codex|Grok|Archon" crates/buzz-workflow/src/routing.rs crates/buzz-relay/src/workflow_route.rs crates/buzz-relay/src/workflow_admission.rs crates/buzz-test-client/tests/e2e_workflow_project_channel_routing.rs
. ./bin/activate-hermit && rg -n "route\\(\"/hooks" crates/buzz-relay/src/router.rs
```

The router must still have exactly the existing `"/hooks/{id}"` registration.

## Spec Coverage Self-Review

- Repository identity tiers, exact comparison, and fail-closed ambiguity: Task 4.
- Alias save-time live head and admission-time revalidation only on alias match: Tasks 4 and 5.
- Claim-valid counting before channel quality: Task 4.
- Admission order, snapshot immutability, and idempotency: Tasks 2 and 6.
- Side-effect revalidation without reroute: Task 7.
- Provenance tags and redaction: Tasks 1, 6, 7, 8.
- Static compatibility: Tasks 1, 7, 8.
- HTTP status table: Task 6.
- No new endpoint/service/UI/provider adapter: file map and grep gate.
