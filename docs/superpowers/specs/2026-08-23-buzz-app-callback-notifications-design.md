# Buzz App callback notifications

**Status:** Approved design; written specification awaiting final user review.
**Classification:** Architectural.
**Scope:** Community-managed Apps that receive authenticated callbacks and post repository-routed notifications under an App attribution.
**Initial App:** Archon.

## Summary

Buzz will add a first-class App model for external integrations.
An App is a community resource, not a human, agent, workflow, or channel member.
Community owners and admins can create and manage Apps through the Desktop client and Buzz CLI.

Archon will send a normalized callback to an App-specific endpoint.
Buzz will identify the App from the endpoint, authenticate the callback, resolve the repository to one project channel, and publish the notification under the Archon App attribution.
The callback cannot choose a channel.

The relay remains the cryptographic signer of App messages.
Clients will render the App as the visible actor only when both the message and App metadata are relay-attested in the same community.
This keeps the audit trail truthful without creating a Nostr user or agent keypair for the App.

## Superseded design boundary

This specification supersedes the App identity and inbound-delivery architecture in `2026-08-20-buzz-project-channel-routing-design.md`.
That design correctly established repository-to-project-channel routing, idempotency, community isolation, and fail-closed ambiguity handling.
It incorrectly placed Archon callbacks inside a workflow whose owner became the visible message author.

The implemented repository and project resolver remains reusable source.
The App path will move that source behind a neutral routing boundary instead of duplicating it.
The existing workflow webhook path remains supported for workflows and is not converted into the App path.

## Verified current behavior

The current webhook route loads a workflow and executes with the workflow owner's standing authority.
`WorkflowEngine` passes `workflow.owner_pubkey` into `ActionSink::send_message`.
`RelayActionSink` signs the message with the relay key and writes the workflow owner into the author-attributing `p` tag.
The Desktop author resolver trusts that delegated `p` tag only on relay-signed events.
The deployed Archon workflow belongs to Kevin, so the Desktop correctly renders Kevin under the current protocol.

The relevant current sources are:

- `crates/buzz-relay/src/workflow_admission.rs` for webhook authentication and workflow admission.
- `crates/buzz-workflow/src/action_sink.rs` for the workflow-owner author contract.
- `crates/buzz-relay/src/workflow_sink.rs` for relay signing and `p` attribution.
- `desktop/src/shared/lib/authors.ts` for delegated author resolution.
- `crates/buzz-relay/src/workflow_route.rs` for repository and project resolution.
- `crates/buzz-db/src/workflow_admission.rs` for idempotent workflow admission.

## Product contract and deliberate tension

Buzz Vision currently describes humans and agents through one portable Nostr identity model.
An App is deliberately different.
It is a community-local integration actor attested by the community relay.
It cannot authenticate as a user, join a channel, read messages, receive mentions, or build portable reputation.

This is a deliberate product extension for Slack-style integrations.
The App attribution states that the community relay accepted a callback from the configured App.
It does not claim that Archon signed the Nostr message.

## Goals

- Create and list Apps in the Desktop client.
- Manage the same App resources through Buzz CLI.
- Identify each callback as one configured App.
- Route callbacks from repository identity to the one authorized Buzz project channel.
- Publish notifications under App attribution without a human or workflow owner appearing as the author.
- Keep the destination server-derived and unavailable to callback input.
- Make provider retries idempotent.
- Preserve community isolation, auditability, and deterministic failure behavior.
- Reuse the existing routing and event-persistence boundaries.

## Non-goals

- No Buzz reply delivery to GitHub or Archon.
- No App keypair or agent identity.
- No App channel membership.
- No App access to channel reads, mentions, DMs, or presence.
- No callback-supplied channel or project identifier.
- No JSONata, Jolt, or notification template engine in Buzz.
- No App deletion in this version.
- No App management UI on Mobile.
- No conversion or removal of existing workflow webhooks.

## Considered approaches

### Approved: first-class App ingress

Buzz stores a community App, authenticates its callback directly, uses the shared project route resolver, and publishes a relay-signed App message.
This preserves the separation between external integrations and workflow owner authority.
It also gives the UI an explicit App type instead of pretending the App is a user.

### Rejected: App-triggered workflow

This approach would authenticate an App and then invoke a workflow for rendering and delivery.
It was rejected because the workflow owner would remain an authority participant and the design would continue mixing App and workflow semantics.

### Rejected: App service account

This approach would give each App a Nostr keypair and require it to sign messages.
It was rejected because key management would turn the App into a user or agent-like service account.
The user explicitly requires an App model instead.

## App authority model

An App belongs to one community.
Only a current community owner or admin can create, update, rotate, enable, or disable an App.
The relay must enforce this rule even when a client hides unavailable controls.

An active App may post only through its callback endpoint.
It may reach any live channel in its community only when Buzz derives that channel from a valid repository and project route.
The App does not need channel membership because it cannot choose the destination or read channel content.

The callback body cannot contain an authoritative `channel_id`, project coordinate, community identifier, App identifier, or App display name.
If such fields are present, Buzz treats them as ordinary metadata or rejects reserved control-field collisions.
They never affect authorization or routing.

## App data model

Buzz adds a community-scoped `apps` table with these logical fields:

| Field | Contract |
| --- | --- |
| `community_id` | Host-derived tenant key and first part of the primary key. |
| `id` | Random UUID and second part of the primary key. |
| `name` | Required display name. |
| `description` | Optional public description. |
| `icon_url` | Optional URL produced through the existing Buzz media path. |
| `status` | `active` or `disabled`. |
| `secret_hash` | SHA-256 of the high-entropy callback secret. |
| `created_by` | Pubkey of the owner or admin who created the App. |
| `created_at` | Creation timestamp. |
| `updated_at` | Last metadata, state, or secret rotation timestamp. |

The composite primary key is `(community_id, id)`.
App names do not need to be unique.
The UUID is the stable callback and attribution identifier.

Buzz generates at least 256 bits of random secret material and returns the encoded secret only after create or rotate.
Buzz stores only its SHA-256 hash.
The raw secret must not appear in App metadata events, database projections, logs, traces, audit details, or later read responses.

## Public App metadata

Buzz adds relay-signed parameterized-replaceable App metadata kind `39007`.
The address is `(relay_pubkey, 39007, app_id)` within the host community.

The event contains:

- `d` with the App UUID.
- `name` with the current display name.
- `status` with `active` or `disabled`.
- Optional `picture` with the App icon URL.
- The public description in content.

The event contains no secret hash, creator authorization data, callback payload, or delivery history.
The relay publishes a replacement metadata event after create, metadata update, enable, or disable.
Secret rotation does not need a metadata replacement because it does not change public fields.

Disabled App metadata remains available.
Historical messages therefore retain a resolvable App actor.
Renaming an App or changing its icon updates historical rendering in the same way that profile updates affect existing messages.

## Management protocol

App management uses the existing signed Nostr command path instead of adding management HTTP endpoints.
Buzz reserves admin command kind `9038` as `KIND_APP_ADMIN_COMMAND`.
The command content is a validated JSON object with one action:

- `create` with name, optional description, and optional icon.
- `update` with App ID and changed public metadata.
- `rotate_secret` with App ID.
- `enable` with App ID.
- `disable` with App ID.

The relay binds the event to its host community and checks that the signer is a current owner or admin.
It executes the command transactionally and writes the existing audit trail.
Create and rotate return the new raw secret exactly once in the command result.
No command event or public metadata event stores that secret.

App listing uses a Nostr query for the latest live kind `39007` events signed by the active relay.
Clients do not need a new App listing HTTP endpoint.

## Desktop App management experience

Desktop adds an `Apps` section under Community Settings.
The section is visible only when the current membership snapshot says the user is an owner or admin.
The relay remains the authorization boundary when that snapshot is stale or maliciously modified.

The Apps page shows:

- App icon and name.
- Description when present.
- Active or disabled status.
- Callback URL derived from the current relay HTTP URL and App UUID.
- Created and updated times when available.
- An actions menu for edit, rotate secret, enable, and disable.

The empty state explains that Apps receive external callbacks and post notifications into project channels.
The primary action is `Create App`.

The create dialog contains name, optional description, and optional icon upload through the existing media path.
After successful creation, a one-time credentials dialog shows the callback URL and secret with separate copy controls.
Closing that dialog permanently removes the raw secret from client state.

Rotate secret requires confirmation because it immediately invalidates the previous secret.
The one-time credentials dialog then shows the replacement secret.
Disable requires confirmation and preserves metadata and delivery history.
This version has no delete action.

Loading, empty, permission, relay error, validation error, and mutation-pending states must be explicit.
The UI must not optimistically claim that an App is active before the relay accepts the signed command.

## CLI management experience

Buzz CLI adds these commands:

```text
buzz apps list
buzz apps create --name <name> [--description <text>] [--icon-url <url>]
buzz apps update --app <uuid> [--name <name>] [--description <text>] [--icon-url <url>]
buzz apps rotate-secret --app <uuid>
buzz apps enable --app <uuid>
buzz apps disable --app <uuid>
```

All commands keep the existing JSON output and structured error conventions.
Create and rotate print the secret only in the successful JSON response.
Compact output must not print a secret unless the requested command created or rotated it.

## Callback endpoint

Each App receives this callback endpoint:

```text
POST /hooks/apps/{app_id}
Host: <community-host>
X-Webhook-Secret: <secret>
Content-Type: application/json
```

The path identifies a candidate App.
The secret authenticates the caller as that App.
The Host header selects the community before App lookup.

The endpoint does not accept the secret in a query parameter.
Unknown communities, cross-community App IDs, missing Apps, and disabled Apps fail closed without revealing cross-tenant state.

## Callback payload

The normalized callback object is:

```json
{
  "idempotency_key": "stable-provider-event-id",
  "repository_name": "harness-service",
  "event_type": "workflow.run.completed",
  "content": "✅ Archon completed speckit-feature for harness-service",
  "metadata": {}
}
```

`idempotency_key`, `repository_name`, `event_type`, and `content` are required non-empty strings.
`metadata` is an optional JSON object and defaults to an empty object.
Buzz rejects unknown reserved control fields and accepts non-reserved metadata only within configured size and nesting limits.

`content` uses the existing message content limits and validation.
The complete body uses the relay's bounded webhook body reader.
The endpoint rejects non-object JSON and unsupported content types.

Archon remains responsible for JSONata transformation and provider event filtering.
Buzz does not understand Archon workflow envelopes.
Buzz receives only the normalized notification contract.

Archon may continue to send `X-Webhook-Signature-V2` and `X-Webhook-Timestamp` headers.
The first App version authenticates with the high-entropy `X-Webhook-Secret` bearer value because Buzz can verify a stored hash without retaining an HMAC key.
The HMAC headers do not grant authority in this version.

## Authentication flow

Buzz hashes the provided secret and compares the digest with the stored digest in constant time.
Authentication runs before payload-specific diagnostics that could reveal App configuration.
Pre-authentication failures create no delivery record.

The callback route has rate limits by community, App ID, and source address.
Repeated authentication failures use low-cardinality security logs without secret material.
Disabling an App or rotating its secret affects the next callback lookup immediately.

## Repository and project routing

The App path reuses the existing pure repository and project route code after moving it from workflow-specific ownership into a neutral module.
Workflow routing and App routing call the same resolver.

The initial App configuration has no repository alias map.
The resolver therefore uses the current strict repository identity tiers with an empty alias set:

1. Exact repository `d` tag.
2. Exact clone URL basename.
3. Exact repository display name.

The first tier with matches wins only when it identifies one distinct full repository coordinate.
Zero matches or multiple distinct matches fail closed.
Comparison remains case-sensitive and performs no trimming or Unicode normalization.

Buzz enumerates the latest live listed projects that contain the repository coordinate.
Only a project signed by the repository owner or current maintainer is claim-valid.
Exactly one claim-valid project must exist.

Buzz reads the selected project's single valid `buzz-channel` UUID.
The channel must exist, be live, not archived, and belong to the same community.
The App does not need channel membership because the destination is server-derived and the App has no read capability.

Buzz never falls back to another project, a callback-supplied channel, an App default channel, or the old workflow home channel.

## App delivery ledger and idempotency

Buzz adds a community-scoped `app_callback_deliveries` table with these logical fields:

| Field | Contract |
| --- | --- |
| `community_id` | Host-derived tenant key. |
| `id` | Delivery UUID. |
| `app_id` | Authenticated App UUID. |
| `idempotency_key_hash` | SHA-256 of the raw provider key. |
| `payload_hash` | SHA-256 of canonical JSON after removing `idempotency_key`. |
| `event_type` | Sanitized callback event type. |
| `route_snapshot` | Repository coordinate, project coordinate, and channel UUID. |
| `status` | `delivered` or `rejected`. |
| `event_id` | Message event ID on success. |
| `failure_code` | Stable redacted code on deterministic rejection. |
| `created_at` | Admission time. |
| `completed_at` | Delivery or rejection time. |

The table has a unique constraint on `(community_id, app_id, idempotency_key_hash)`.
The raw idempotency key is never stored.

Buzz canonicalizes the parsed JSON object after removing `idempotency_key` and hashes that value.
Object key order does not affect equality.
Array order, field presence, scalar type, and scalar value remain significant.

The idempotency behavior is:

- A new key and valid route creates one message and one delivered record.
- The same key and payload returns the existing result without creating another message.
- The same key and different payload returns `409 Conflict` without changing the record.
- Concurrent identical callbacks converge on one record and one message.
- A deterministic route rejection stores one rejected record and reproduces the same redacted rejection on retry.
- Authentication failure or a missing usable idempotency key creates no record.
- A transient database failure creates no committed record and allows a normal retry.

The delivered record and message event persistence must commit atomically or not at all.
A process crash after commit but before the HTTP response is therefore safe because the provider retry returns the existing delivery.
Post-persist fan-out, search, and audit use the existing event side-effect path.

## Message emission and provenance

A successful callback creates a standard top-level kind `9` channel message signed by the community relay.
It carries these tags:

- `h` with the destination channel UUID.
- `buzz:app` with the App UUID.
- One `a` tag for the repository coordinate.
- One `a` tag for the project coordinate.
- `buzz:app-delivery` with the delivery UUID.
- `buzz:app-event` with the normalized event type.

The message does not contain an author-attributing `p` tag for Kevin, the App creator, or a workflow owner.
Any `p` tags produced by resolved `@Name` mentions are mention recipients only.
The App attribution path takes precedence over legacy relay-signed `p` attribution when clients resolve the visible actor.

Replies remain normal human or agent channel replies.
They reference the App message as their thread root and do not copy App provenance.

## Client author resolution

Clients first inspect a relay-signed channel message for `buzz:app`.
They accept the App attribution only when all these conditions hold:

- The event has a valid signature from the active community relay.
- The App UUID is valid.
- The latest kind `39007` metadata for that UUID is signed by the same relay in the same community.

If any condition fails, the client renders the actual relay signer.
It must not fall through to the first mention `p` tag and mislabel a mentioned user as the author.

Desktop renders the App name, icon, and an `App` badge in the timeline, thread roots, quoted messages, search results, link previews, and desktop notifications.
Mobile renders the same App actor in message and thread surfaces.
App management remains Desktop-only in this version.

## Error contract

| HTTP status | Condition |
| --- | --- |
| `202 Accepted` | A new delivery succeeded or an identical delivery already exists. |
| `400 Bad Request` | App UUID, content type, JSON syntax, or object shape is invalid. |
| `401 Unauthorized` | The callback secret is missing or invalid. |
| `404 Not Found` | The community or App is unavailable, or the App is disabled. |
| `409 Conflict` | The App-scoped idempotency key exists with a different payload. |
| `422 Unprocessable Entity` | Required control fields, repository, project, or channel routing fails deterministically. |
| `429 Too Many Requests` | Callback rate limit is exceeded. |
| `503 Service Unavailable` | A transient dependency failure prevents a reliable decision. |

External errors use stable machine-readable failure codes and redacted messages.
They never reveal candidate repositories, project membership, channel membership, secret hashes, raw callback bodies, or raw idempotency keys.

## Audit and observability

App create, update, rotate, enable, and disable operations write the existing community audit chain with the admin actor and App UUID.
Successful message creation continues through the existing EventCreated audit path.
The delivery ledger provides callback-specific correlation without adding another background service.

Logs and metrics use App UUID, delivery UUID, outcome, latency, and low-cardinality failure code.
They do not include secret values, secret hashes, callback content, raw metadata, raw idempotency keys, or ambiguous routing candidates.

## Compatibility

Existing workflow definitions, webhook URLs, workflow secrets, run history, and workflow message attribution remain unchanged.
The App route does not intercept `/hooks/{workflow_id}`.
Workflow routing continues to use workflow owner authority.

Existing human and agent messages continue through current author resolution.
Only a valid relay-signed `buzz:app` marker selects the App actor path.
Regular members and agents cannot spoof App attribution with a self-signed message.

The old Archon workflow may coexist during rollout.
Operators disable it only after the new App callback passes live verification.

## Testing strategy

### Unit tests

- Validate App command payloads and state transitions.
- Verify secret generation, hashing, and constant-time comparison.
- Validate callback shape, bounds, and reserved fields.
- Verify canonical payload hashing and idempotency conflict behavior.
- Exercise repository tiers and ambiguous project routing through the shared resolver.
- Verify App actor resolution and fail-safe relay fallback.

### Database integration tests

- Prove App and delivery isolation across communities with the same UUID inputs.
- Prove owner/admin mutations and member denial.
- Prove unique concurrent delivery admission.
- Prove atomic message and delivered-record persistence.
- Prove disabled Apps and rotated secrets take effect immediately.

### Relay end-to-end tests

- Create an App through the signed admin command.
- Query its relay-signed metadata.
- Deliver callbacks for two repositories in one project to the same project channel.
- Reject missing and invalid secrets.
- Reject disabled, missing, and cross-community Apps.
- Reject missing and ambiguous routes.
- Return one message for repeated and concurrent identical callbacks.
- Reject an idempotency key reused with different content.
- Verify the message signer, App marker, channel, repository, project, event type, and delivery provenance.
- Prove existing static and dynamically routed workflow webhooks remain unchanged.

### Desktop end-to-end tests

- Show Apps only to owner/admin users.
- Render loading, empty, list, failure, and disabled states.
- Create an App and show callback credentials once.
- Edit metadata and observe list updates.
- Rotate the secret and require confirmation.
- Disable and enable the App.
- Render a channel message as `Archon` with an `App` badge and no Kevin attribution.

### Mobile tests

- Resolve a valid relay-attested App message.
- Reject spoofed App metadata or a non-relay message marker.
- Preserve normal human and agent author rendering.

### Live acceptance test

The final live test uses the deployed Mac mini relay and Archon service.
It creates an `Archon` App in the `gigo-harness` community, configures the current Archon provider bindings with the App callback URL and secret, and invokes the existing Archon manual callback endpoint.

One test uses `harness-service` and one uses `agentic-os-plan`.
Both messages must arrive in the `gigo-harness` channel under `Archon · App`.
Provider retry must not add a second message.
The old `Archon Notifications` workflow is disabled only after both callbacks pass.

## Rollout

1. Apply the App and delivery migrations with the relay release.
2. Deploy relay, Desktop, and Mobile support for App metadata and attribution.
3. Create the `Archon` App through Desktop or CLI.
4. Copy its callback URL and one-time secret into the two Archon provider bindings.
5. Keep Archon JSONata transforms responsible for the normalized callback object.
6. Run the manual callback for both registered repositories.
7. Verify channel, App attribution, provenance, idempotency, search, and audit evidence.
8. Disable the old workflow callback after the new path passes.

## Acceptance criteria

1. A community owner or admin can create and list Apps in Desktop and CLI.
2. A normal member cannot create, update, rotate, enable, or disable an App.
3. Create and rotate return a high-entropy secret once, and Buzz stores only its hash.
4. App metadata is a relay-signed community-local kind `39007` event with no private fields.
5. One Archon App endpoint accepts callbacks for multiple repositories.
6. The callback cannot select a channel or project.
7. Buzz routes each repository through exactly one claim-valid project to its live `buzz-channel`.
8. Missing or ambiguous repository, project, or channel state creates no message.
9. The App does not need channel membership and has no channel read capability.
10. Identical provider retries and concurrent callbacks create exactly one delivery and one message.
11. Reusing an idempotency key with a different payload returns `409` and changes nothing.
12. A successful message is relay-signed and displays as `Archon · App` without Kevin or workflow-owner attribution.
13. A self-signed or cross-community App marker cannot spoof App attribution.
14. Disabled Apps reject callbacks immediately while historical metadata remains resolvable.
15. Existing workflow webhook and human/agent author behavior passes regression tests.
16. Live callbacks for `harness-service` and `agentic-os-plan` reach `gigo-harness` under the Archon App attribution.

## Design completion

This specification records all approved brainstorming decisions.
It is narrow enough for one implementation plan across relay, database, CLI, Desktop, Mobile, and integration tests.
Implementation planning must wait for final user review of this written specification.
