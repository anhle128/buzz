# Buzz project channel routing for workflow webhooks

**Status:** Approved architecture and design sections; written specification awaiting user review.
**Classification:** Architectural.
**Scope:** Native inbound routing from an authenticated provider callback to the one authorized Buzz project channel for the callback repository.

## Summary

Buzz is the notification hub for repository automation.
One webhook workflow must be able to receive callbacks for many repositories and post each notification to the correct Buzz project channel.

The approved design resolves and authorizes the dynamic route during Buzz webhook admission, stores an immutable route snapshot on the workflow run, and revalidates that exact route immediately before each message side effect.
Only a webhook workflow that opts in to `project_channel_by_repository` uses this behavior.
Every existing workflow keeps its stored static channel and current behavior.

This is an inbound-only enhancement.
Mirroring a later Buzz thread reply to a GitHub comment is a separate excluded follow-up.

## Context and problem

The deployed workflow path at revision `408200fa69f0294b37d43eb8fdcdad0254b97c7e` sends webhook output to the channel stored on the workflow.
Archon can send receiver-specific JSON that includes `repository_name`, but the current Buzz webhook path does not resolve that identity through a repository and project to the project's `buzz-channel`.
The live `gigo-harness` project contains `agentic-os-plan` and `harness-service`, so a static workflow per project does not solve the multi-project routing problem.

The current source supports these findings:

- Workflow definitions are YAML that Buzz stores as canonical JSON, and the current schema has no routing contract ([`schema.rs`](../../../crates/buzz-workflow/src/schema.rs#L1-L27)).
- The webhook endpoint binds the request to the host community, authenticates with the workflow secret, converts body fields into trigger context, checks owner authority, creates a run, and starts execution ([`bridge.rs`](../../../crates/buzz-relay/src/api/bridge.rs#L1862-L1999)).
- `send_message` currently prefers the workflow's stored channel and rejects a different explicit channel ([`executor.rs`](../../../crates/buzz-workflow/src/executor.rs#L468-L510)).
- The message sink validates the channel, checks owner access, creates a relay-signed top-level kind `9` message, and adds workflow attribution tags ([`workflow_sink.rs`](../../../crates/buzz-relay/src/workflow_sink.rs#L211-L340)).
- Workflow runs already store community scope, trigger context, trace, status, and structured failure information ([`workflow.rs`](../../../crates/buzz-db/src/workflow.rs#L190-L225), [`workflow.rs`](../../../crates/buzz-db/src/workflow.rs#L801-L848), [`workflow.rs`](../../../crates/buzz-db/src/workflow.rs#L904-L955)).
- NIP-MP defines exact full repository coordinates, non-exclusive project membership, claim authority, and exhaustive project/repository enumeration ([`NIP-MP.md`](../../nips/NIP-MP.md#member-coordinates), [`NIP-MP.md`](../../nips/NIP-MP.md#claim-authority), [`NIP-MP.md`](../../nips/NIP-MP.md#the-fold)).
- The current Projects fold already treats a project signer as authorized when it is the repository owner or a listed maintainer ([`projectModels.ts`](../../../desktop/src/features/projects/projectModels.ts#L438-L485)).

The external operational source is `/Users/dale/Desktop/obsidian-mind/work/active/Gigo Harness.md`, section “Archon-to-Buzz blocked handoff — 2026-08-19”.
This specification records only its routing conclusions and no secret material.

## Goals

- Let one opted-in webhook workflow route callbacks for multiple repositories and projects.
- Resolve repository identity and NIP-MP project membership deterministically.
- Enforce community, workflow-owner, repository-claim, and channel-membership boundaries before a run starts and before a message is created.
- Make provider retries idempotent at the workflow boundary.
- Preserve enough root-message provenance for a later outbound reply workflow without binding the inbound path to GitHub or to a fixed agent.
- Preserve all static workflow behavior when dynamic routing is absent.

## Non-goals

- No Buzz thread reply to GitHub comment delivery.
- No changes to Archon, provider configuration, GitHub, the vault, or production services.
- No new HTTP endpoint, service, routing projection, background reconciler, or provider-specific adapter.
- No workflow-agent selection or fixed binding to Hermes, Codex, Grok, or another agent.
- No general workflow crash-recovery or exactly-once framework beyond callback admission and run idempotency.
- No desktop, mobile, or web user-interface work.

## Repository identity decision and correction

An earlier exact-only proposal would have required `repository_name` to match only the repository `d` tag.
That proposal was corrected and is not part of the approved design.

Buzz will use one global resolver with this fixed tier order:

1. Repository `d` tag.
2. Workflow-configured alias.
3. Basename of a repository clone URL.
4. Repository display name.

At each tier, Buzz collapses all matches that identify the same full repository coordinate.
If the first tier with matches has one distinct repository coordinate, the resolver selects it and stops.
If that tier has more than one distinct repository coordinate, resolution fails closed.
Buzz does not inspect a weaker tier to break a tie or replace a stronger match.
If all tiers have zero matches, resolution fails closed.

The comparison contract is strict:

- `repository_name`, `d` tags, alias keys, clone basenames, and display names are case-sensitive.
- Buzz does not trim values or normalize Unicode, spaces, hyphens, or underscores.
- Each `clone` tag value contributes only its final URL path segment.
- Buzz removes exactly one terminal `.git` suffix from that segment.
- A clone value without a non-empty final path segment contributes no identity.
- The display-name tier reads the repository announcement's `name` tag.
- Any supported spelling variant must be an explicit alias.

An alias maps one exact alias key directly to one full coordinate in the form `30617:` plus the repository owner pubkey plus `:` plus the repository `d` tag.
An alias cannot target a bare `d` tag or a clone URL.
Duplicate alias keys are invalid, but distinct aliases can target the same coordinate.
Every target coordinate must have a latest live repository head in the webhook's host community when the workflow is saved.
During admission, Buzz revalidates the target only when `repository_name` matches that alias key at the alias tier.
An unused stale alias does not block a stronger match or an unrelated callback.
An alias helps only with identity recognition.
It grants no project, repository, or channel authority.

## Approaches

### Approved: admission resolution plus immutable snapshot and side-effect revalidation

Buzz resolves and authorizes the route in the webhook admission path.
It stores the selected community, repository, project, channel, and match tier as an immutable server-only run snapshot.
Immediately before a dynamic `send_message`, Buzz revalidates the current authority for that exact snapshot and never reroutes an existing run.

This approach gives the caller a deterministic admission result, gives the run a durable route identity, and prevents authority that is revoked after admission from reaching the message side effect.
It performs some checks twice, but those direct checks avoid a second source of truth and are suitable for the current community scale.

### Rejected: resolve only when `send_message` runs

This approach would keep admission small and use the freshest state at the side effect.
It was rejected because the admitted run would not have a durable route identity, different steps could observe different routes, and failures would move from the trust boundary into asynchronous execution.
It would also make deterministic provider retry responses and route auditing weaker.

### Rejected: materialized repository-to-project-channel routing projection

This approach could make lookups faster at a much larger scale.
It was rejected because it introduces another state model, invalidation rules, and eventual-consistency failure modes before measured load requires them.
NIP-MP already requires exhaustive, deterministic enumeration, and the current design can use direct community-scoped queries.
A projection is a later scaling option only if measured admission latency or database load shows that direct resolution is insufficient.

## User flow and workflow configuration

The workflow owner creates and manages a webhook workflow in its existing home channel.
The home channel remains the management and standing-authority boundary.
It is not a fallback notification destination.

Static workflow definitions remain unchanged.
A dynamic definition adds this optional contract:

```yaml
routing:
  mode: project_channel_by_repository
  aliases: {}
```

The `routing` block is valid only with `trigger.on: webhook`.
The only supported routing mode in this scope is `project_channel_by_repository`.
The `aliases` map is optional and defaults to empty.
Each alias key uses the exact comparison rules above, and each value is a full `30617` repository coordinate.

For a dynamic workflow, every `send_message.channel` field is invalid because the server-resolved run route is the only allowed message destination.
When `routing` is absent, `send_message` keeps the current stored-channel resolution and cross-channel override rules.

An admitted dynamic callback body must be a JSON object with:

- `repository_name`: a non-empty string used by the resolver.
- `idempotency_key`: a non-empty stable string scoped to this workflow and used only by the control plane.
- Any other callback fields needed by workflow conditions and templates.

Buzz removes `idempotency_key` before it builds template-visible webhook fields or stores trigger context.
All other callback fields keep the existing template conversion behavior.

The ideal flow is:

1. A provider sends one authenticated callback to the shared workflow endpoint.
2. Buzz binds the request host to one community and authenticates the workflow secret.
3. Buzz resolves `repository_name`, authorizes one project and channel, and creates or returns the idempotent run.
4. The workflow executes with the immutable route snapshot.
5. Buzz revalidates the exact route immediately before message creation.
6. Buzz posts the notification to the selected project channel or records a deterministic failure.

There is no fallback to the home channel, another project, a weaker identity tier after ambiguity, or a provider-supplied channel.

## Trust boundaries and authentication

The request host is the first tenant boundary.
Buzz must bind the host to a community before every workflow, repository, project, channel, and run lookup.
An unknown host and an unknown, inactive, disabled, or authority-revoked workflow use the existing generic not-found behavior.

The existing workflow secret authenticates the callback caller.
The `X-Webhook-Secret` header remains preferred, and the current query-parameter fallback remains for compatibility.
The secret authenticates the caller but does not grant channel authority.

The workflow executes with its owner's standing authority.
Buzz must verify that the owner still has the required authority in the workflow home channel before run creation.
For dynamic routing, the owner must also be a current member of the destination channel, including when that channel is open.
The project signer does not also need destination-channel membership.

Callback JSON is untrusted identity input.
Workflow alias configuration is owner-authored identity data, not an authorization grant.
Repository and project signatures, latest live heads, NIP-MP claim rules, current channel state, and current membership are the authorization sources.

NIP-MP defines a project's `buzz-channel` as metadata rather than repository authority or event-ingest routing ([`NIP-MP.md`](../../nips/NIP-MP.md#authority)).
This design preserves that boundary.
Buzz treats the tag as a destination reference only after separate project-claim, channel-state, and workflow-owner authorization checks.

## Webhook admission and route resolution

Dynamic routing belongs in the Buzz relay webhook admission path.
The generic workflow engine must receive only a server-produced route snapshot and must not resolve repository or project identity from callback fields.

Admission proceeds in this order:

1. Bind the request host to one community.
2. Load the workflow in that community and verify the secret, trigger type, lifecycle state, and owner home-channel authority.
3. Parse the body as a JSON object and validate the dynamic control fields.
4. Remove the raw `idempotency_key` from template-visible data.
5. Resolve the latest live `kind:30617` repository head with the fixed identity tiers.
6. Enumerate the latest live `kind:30621` projects without a fixed result limit.
7. Keep only relay-observable listed projects that contain an exact `a` tag for the selected repository coordinate and whose signer is the repository owner or a current repository maintainer.
8. Count claim-valid projects before inspecting channel quality.
9. Require exactly one claim-valid project.
10. Read its single `buzz-channel` value as a UUID and require a live, non-archived channel in the same community.
11. Require the workflow owner to be a current member of that channel, including for an open channel.
12. Create the idempotent run with the immutable route snapshot, or store the deterministic admission failure when a usable idempotency key is available.

Zero claim-valid projects fails closed.
More than one claim-valid project also fails closed because NIP-MP permits multiple project membership ([`NIP-MP.md`](../../nips/NIP-MP.md#multiple-membership)).
Buzz never selects one by order, creation time, display state, or channel quality.
An unauthorized or unlisted project does not count.
For server routing, listed means that the latest project head is live and does not declare `buzz-visibility` as `unlisted`.
Desktop-local hidden state is not server routing authority.
After the exactly-one check, a missing or invalid channel fails that selected route rather than causing Buzz to choose another project.

The immutable server-only route snapshot is:

```text
community_id
repository_coordinate
project_coordinate
channel_id
matched_identity_tier
```

Callback fields cannot set or override any snapshot field.
Configuration changes after admission do not redirect an existing run.

## Run schema and idempotency contract

`workflow_runs` remains the execution and idempotency ledger.
This design does not add a separate delivery subsystem.

The run needs these logical additions:

- `idempotency_key_hash`: SHA-256 of the raw control-plane key.
- `payload_hash`: SHA-256 of canonical parsed callback JSON after removal of `idempotency_key`.
- `route_snapshot`: the immutable server-produced route fields above, present only after complete successful resolution and authorization.
- A database uniqueness constraint on `(community_id, workflow_id, idempotency_key_hash)`.

The exact physical column layout can use typed columns or a structured route value, but it must preserve atomic uniqueness, community scoping, immutable route fields, and direct auditability.
The raw idempotency key must never be stored.

Canonical payload equality ignores JSON object-key order.
Array order, field presence, scalar type, and scalar value remain significant.
Buzz hashes the parsed JSON value rather than raw request bytes.

The idempotent admission rules are:

- A new key with a valid route atomically creates one pending run and returns `202 Accepted` with its run identity.
- The same key and same payload hash returns the existing run and its current status without starting another resolution, execution, or message creation.
- A duplicate of a stored deterministic admission failure reproduces its original redacted `422` outcome instead of changing or retrying the run.
- The same key and a different payload hash returns `409 Conflict` and does not change the existing run.
- Concurrent requests for the same key converge through the database uniqueness constraint to one run.
- A deterministic admission failure after authentication and validation of a usable key creates or returns one failed run with a stable failure code.
- A request that fails before authentication or has no usable idempotency key creates no run.
- A retry of an admitted run does not restart it, even if later execution or delivery failed.
- A configuration fix for a deterministic failed run requires a new idempotency key.

If a workflow has one notification message step, any number of Archon retries with the same key produces one run, one workflow execution, and one message.
More generally, retries do not add side effects beyond the steps declared in that single execution.

The raw key must not appear in workflow templates, stored trigger context, traces, logs, message content, message provenance, or error text.

## Side-effect revalidation and message emission

Immediately before each dynamic `send_message` side effect, Buzz must revalidate the exact stored route.
It must not rerun the identity tiers to find a replacement route.

The revalidation checks are:

- The workflow is active, enabled, and still authorized in its home channel.
- The stored repository coordinate still has a latest live head.
- The stored project still has a latest live listed head, contains the exact repository coordinate, and is signed by the repository owner or a current maintainer.
- Exactly one current claim-valid project exists for the repository.
- The stored project still names the stored channel.
- The channel still belongs to the run community and is live and not archived.
- The workflow owner is still a current destination-channel member, including for an open channel.

Any failed check marks the run failed before an event is created.
Buzz does not reroute to a different project, channel, or identity tier.

A successful notification uses the existing relay-signed top-level kind `9` workflow-message path.
It keeps these existing tags:

- `h` with the destination channel UUID.
- `p` with the workflow owner and any existing resolved mentions.
- `buzz:workflow` with value `true`.
- `buzz:workflow-owner` with the workflow owner pubkey.

It adds:

- One `a` tag for the full repository coordinate.
- One `a` tag for the full project coordinate.
- `buzz:workflow-run` with the workflow run UUID.

The message event ID is stored in the step result and execution trace through the existing workflow path.
The root message stays a standard Buzz thread root.
Replies remain standard kind `9` channel replies with normal root and parent references.
Replies do not need to copy the root provenance tags because a future router can resolve the root event.

The repository and project coordinates plus `buzz:workflow-run` provide durable root correlation.
The run links that root to the sanitized provider callback context without exposing the raw idempotency key.
These provenance fields identify context but grant no GitHub, repository, project, or channel authority.

Relay-signed workflow messages already avoid recursive workflow triggering through the workflow marker ([`event.rs`](../../../crates/buzz-relay/src/handlers/event.rs#L518-L548)).
Existing `p` mentions can wake eligible agents under normal Buzz rules, but this design does not select or require any agent implementation.

## Error behavior

The endpoint keeps deterministic, redacted external behavior:

| HTTP status | Condition |
| --- | --- |
| `202 Accepted` | A new run was admitted, or the same key and payload returned an existing admitted run. |
| `400 Bad Request` | The workflow UUID or JSON syntax is invalid. |
| `401 Unauthorized` | The webhook secret is missing or invalid. |
| `404 Not Found` | The host, workflow, lifecycle state, or home-channel authority is unavailable through the generic fail-closed boundary. |
| `409 Conflict` | The workflow-scoped idempotency key already exists with a different payload hash. |
| `422 Unprocessable Entity` | Dynamic control fields, repository resolution, project resolution, destination metadata, or route authorization fails deterministically. |
| `503 Service Unavailable` | A transient dependency failure prevents a reliable admission decision before a run exists. |

Stable machine-readable failure codes distinguish at least repository missing, repository ambiguous, alias target unavailable, project missing, project ambiguous, project channel invalid, route unauthorized, route stale at revalidation, and idempotency conflict.
External diagnostics do not reveal candidate coordinates, membership lists, project lists, stored secrets, raw callback bodies, or raw idempotency keys.

If a transient failure occurs before a run exists, the provider can retry with the same key.
If a transient execution or delivery failure occurs after run creation, Buzz marks that same run failed and does not start a second execution for the same key.

Buzz does not post an error message to the selected project channel or to the workflow home channel.
The synchronous HTTP response informs the provider about admission.
The workflow run and its stable error code inform the workflow owner about asynchronous execution.

## Audit and observability

The workflow run is the durable delivery record.
It records community and workflow identity, the two hashes, status, timestamps, sanitized trigger context, route snapshot when available, execution trace, event ID on success, and a stable error code with redacted detail on failure.

Successful message creation continues through the existing event-persistence and EventCreated hash-chain audit path ([`event.rs`](../../../crates/buzz-relay/src/handlers/event.rs#L344-L362)).
This design does not create a second webhook-specific hash chain.

Logs and metrics use low-cardinality outcome and failure-code dimensions plus run identifiers when needed for investigation.
They must not contain webhook secrets, raw idempotency keys, callback bodies, template expansions that expose control data, or ambiguous candidate lists.

Pre-authentication failures create no workflow run.
After authentication, a valid JSON object and usable idempotency key can produce a durable failed run for deterministic resolution or authorization failure.

## Provider-agnostic behavior and future routing

The inbound callback contract names no provider and no agent.
Archon is the initial sender, but any authenticated provider that supplies the same contract can use the route.
Notification content remains workflow-authored, and ordinary Buzz mention and reply behavior remains unchanged.

The current enhancement ends after the inbound root message reaches the correct Buzz project channel.
Buzz thread reply to GitHub comment delivery is an explicit, separately authorized future workflow and is excluded from this implementation.
That follow-up must design and approve:

- GitHub authorization for the target repository and conversation.
- Reply-delivery idempotency.
- Loop prevention between inbound notifications and outbound comments.
- Correlation from a Buzz reply through the root message provenance to the provider conversation.

The future workflow can use the root repository coordinate, project coordinate, workflow run UUID, and sanitized run context as correlation input.
It cannot treat those identifiers as delivery authority.
No GitHub token, provider adapter, outbound delivery step, or reply listener is part of the current scope.

## Static workflow compatibility

The absence of `routing` is the compatibility switch.
For every existing static workflow:

- `repository_name` and `idempotency_key` are not required.
- The current workflow channel remains the `send_message` destination.
- The current channel-override validation remains unchanged.
- The current webhook secret, trigger-field conversion, execution, message tags, and response behavior remain unchanged except for shared internal code that is behaviorally equivalent.
- No repository, project, alias, or dynamic channel query runs.

Dynamic-only rules must not alter message, reaction, schedule, diff, manual, approval-resume, or static webhook execution.
A focused regression test must prove this compatibility boundary.

## Implementation boundary

Implementation stays inside existing Buzz components:

| Component | Responsibility |
| --- | --- |
| `buzz-workflow` | Add the optional routing schema, validate its trigger and alias contract, and keep control-plane fields out of template context. |
| `buzz-db` | Add community-scoped repository/project resolution helpers and the workflow-run idempotency and route-snapshot persistence contract. |
| `buzz-relay` webhook admission | Authenticate, resolve, authorize, hash, create or return the idempotent run, and expose redacted outcomes. |
| `buzz-relay` workflow message boundary | Consume the immutable route, revalidate current authority, and add root provenance to the existing message event. |
| Tests | Cover resolver tiers, authorization, concurrency, revalidation, provenance, redaction, and static compatibility at the narrowest useful levels plus the relay-backed webhook flow. |

Direct queries must enumerate the relevant latest live repositories and projects without a fixed result limit, consistent with the NIP-MP completeness rule.
The implementation must reuse existing event, community, channel, membership, workflow-run, and audit concepts.
It must not add a new service, cache, projection, dependency, endpoint, user interface, or provider-specific routing abstraction.

## Acceptance criteria

1. A workflow without `routing` passes a regression test that proves its static channel behavior is unchanged.
2. One opted-in workflow can route callbacks for both `agentic-os-plan` and `harness-service` through their uniquely authorized project to `gigo-harness` when the live events and memberships support that result.
3. Repository resolution uses the approved tier order, exact case-sensitive comparisons, exact no-normalization rules, unique-first-tier stop rule, and fail-closed ambiguity rule.
4. An alias resolves only to a live full repository coordinate in the host community and never grants claim or channel authority.
5. Zero or multiple claim-valid projects reject the callback, and only a listed project signed by the repository owner or a current maintainer counts.
6. A missing, malformed, archived, cross-community, or unauthorized destination channel produces no message.
7. A route that loses repository, project, channel, workflow-owner, or membership authority before the side effect fails the existing run and does not reroute.
8. Concurrent same-key and same-payload callbacks create one run and one execution, and a single-message workflow creates one message.
9. The same key with a different payload is rejected without changing the existing run.
10. The raw idempotency key is absent from stored context, templates, traces, logs, errors, message content, and provenance.
11. A successful root message is a standard relay-signed Buzz message with repository, project, workflow-run, channel, workflow, and owner provenance.
12. Failures use stable redacted codes, never fall back to another destination, and never post an error message to a channel.
13. No behavior or schema binds the route to Archon, GitHub, Hermes, Codex, Grok, or another fixed provider or agent.
14. The excluded outbound Buzz-thread-to-GitHub-comment workflow has no implementation artifact in this scope.

## Design completion

This document consolidates the approved architecture approach and all seven approved design sections.
It is narrow enough for one implementation plan across the existing workflow, database, and relay boundaries.
Implementation planning must wait for explicit user review and approval of this written specification.
