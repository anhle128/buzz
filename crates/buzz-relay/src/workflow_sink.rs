//! Relay-side implementation of [`ActionSink`] for workflow actions.
//!
//! Builds Nostr events, persists them, and delegates post-persist side effects
//! (WebSocket fan-out, Redis pub/sub, search indexing, audit logging) to the
//! existing [`dispatch_persistent_event`] helper.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};

use buzz_core::kind::{KIND_GIT_REPO_ANNOUNCEMENT, KIND_PROJECT, KIND_STREAM_MESSAGE};
use buzz_core::tenant::CommunityId;
use buzz_db::workflow::WorkflowStatus;
use buzz_workflow::action_sink::{
    route_provenance_tags, ActionSink, ActionSinkError, WorkflowMessageRoute,
};
use buzz_workflow::routing::parse_repository_coordinate;
use buzz_workflow::WorkflowDef;
use chrono::Utc;
use nostr::{EventBuilder, Kind, Tag};
use tracing::info;
use uuid::Uuid;

use crate::handlers::event::dispatch_persistent_event;
use crate::state::AppState;
use crate::workflow_route::{
    project_head_from_event, repository_head_from_event, require_exact_stored_project_channel,
};

/// Resolves `@Name` mentions in workflow message text to the pubkeys of the
/// channel members they name, so the emitted kind:9 carries the `p` tags that
/// ACP agent-wake (`event_mentions_agent`) is gated on.
///
/// The client resolves mentions to `p` tags at compose time from an interactive
/// autocomplete pick; the workflow path has only free text, so this reverse-parse
/// *defines* the matching contract. It is deliberately conservative to avoid
/// waking the wrong agent:
///
/// - **Members only.** Candidates are the destination channel's members; global
///   users are never matched.
/// - **Exact display name.** No substring, prefix, or fuzzy matching. Names may
///   contain spaces/punctuation (`"Will Pfleger"`, `"Lep (Subagent)"`), so the
///   match is anchored on `@` and terminated by a non-name boundary rather than
///   whitespace.
/// - **Greedy-longest, non-overlapping.** Longer names are matched first and
///   consume their span, so `@Will Pfleger` binds *Pfleger* and a bare `@Will`
///   does not match the member `"Will Pfleger"`.
/// - **Ambiguous names wake no one.** If two or more members share the matched
///   display name, no `p` tag is emitted for it — arbitrary selection would
///   silently misroute and tagging all of them is a false-wake firehose.
///
/// Returns deduplicated pubkey hexes, in first-appearance order in `text`.
fn resolve_mention_pubkeys(text: &str, members: &[(String, String)]) -> Vec<String> {
    // Name → pubkey, folding case (client matches case-insensitively). A name
    // that maps to more than one distinct pubkey is ambiguous → wake no one.
    let mut by_name: std::collections::HashMap<String, Option<String>> =
        std::collections::HashMap::new();
    for (name, pubkey) in members {
        if name.trim().is_empty() {
            continue;
        }
        by_name
            .entry(name.to_lowercase())
            .and_modify(|slot| {
                if slot.as_deref() != Some(pubkey.as_str()) {
                    *slot = None; // ambiguous
                }
            })
            .or_insert_with(|| Some(pubkey.clone()));
    }

    // Match longest names first so a longer name consumes its span before a
    // shorter substring name can claim part of it.
    let mut names: Vec<&(String, String)> = members.iter().collect();
    names.sort_by_key(|(name, _)| std::cmp::Reverse(name.chars().count()));

    let chars: Vec<char> = text.chars().collect();
    let mut consumed = vec![false; chars.len()];

    // Case-insensitivity folds *both* sides through `char::to_lowercase`, which
    // can change length: `İ` (U+0130) lowercases to two code points (`i` +
    // U+0307 combining dot). Comparing a pre-lowercased copy of the whole text
    // against a lowercased name by index silently desyncs once any earlier char
    // expands. Instead, fold on the fly: walk the original `chars` at the
    // candidate `@`, folding each char, and match against the folded-name char
    // stream — tracking how many *original* chars were consumed so
    // boundary/`consumed` accounting stays in original coordinates. `None` = no
    // match; `Some(n)` = matched, consuming `n` original chars after the `@`.
    let match_name_len = |start: usize, folded_name: &[char]| -> Option<usize> {
        let mut ci = start;
        let mut ni = 0;
        while ni < folded_name.len() {
            let c = *chars.get(ci)?;
            for fc in c.to_lowercase() {
                if folded_name.get(ni) != Some(&fc) {
                    return None;
                }
                ni += 1;
            }
            ci += 1;
        }
        Some(ci - start)
    };

    // A mention is anchored on `@` at a left boundary (start / whitespace / `(`)
    // and the matched name must not be followed by a name-continuation char —
    // otherwise `@Will` would match inside `@Willow`. Combined with matching the
    // longest member name first, this is the whole rule: no punctuation allowlist
    // to get wrong, and it is unicode-safe (em-dash, emoji all terminate a name).
    let is_left_boundary = |i: usize| i == 0 || chars[i - 1].is_whitespace() || chars[i - 1] == '(';
    let extends_name = |c: char| c.is_alphanumeric() || c == '_';

    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut hits: Vec<(usize, String)> = Vec::new();

    for (name, _) in &names {
        let folded_name: Vec<char> = name.to_lowercase().chars().collect();
        if folded_name.is_empty() {
            continue;
        }
        let mut at = 0;
        while at < chars.len() {
            // Anchor on `@` at a left boundary and an unconsumed span; only then
            // attempt the fold-match. `name_len` is measured in *original* chars,
            // so `at + 1 + name_len` is the true position just past the name.
            let name_len = (chars[at] == '@' && is_left_boundary(at) && !consumed[at])
                .then(|| match_name_len(at + 1, &folded_name))
                .flatten()
                .filter(|&n| {
                    chars[at + 1 + n..]
                        .first()
                        .is_none_or(|&c| !extends_name(c))
                });
            if let Some(name_len) = name_len {
                let span = 1 + name_len;
                if let Some(Some(pubkey)) = by_name.get(&name.to_lowercase()) {
                    hits.push((at, pubkey.clone()));
                }
                for slot in consumed.iter_mut().skip(at).take(span) {
                    *slot = true;
                }
                at += span;
            } else {
                at += 1;
            }
        }
    }

    hits.sort_by_key(|(at, _)| *at);
    for (_, pubkey) in hits {
        if seen.insert(pubkey.clone()) {
            out.push(pubkey);
        }
    }
    out
}

/// Relay-side action sink — executes workflow side-effects directly.
///
/// Holds a **weak** reference to `AppState` to avoid an `Arc` reference cycle:
/// `AppState` → `WorkflowEngine` → `ActionSink` → `AppState`. Using `Weak`
/// breaks the cycle so all structs can be dropped on shutdown.
///
/// Post-persist side effects are delegated to [`dispatch_persistent_event`]
/// for consistency with the REST/WebSocket paths.
pub struct RelayActionSink {
    state: Weak<AppState>,
}

impl RelayActionSink {
    /// Create a new `RelayActionSink` from the shared application state.
    pub fn new(state: &Arc<AppState>) -> Self {
        Self {
            state: Arc::downgrade(state),
        }
    }
}

impl ActionSink for RelayActionSink {
    fn send_message(
        &self,
        community_id: CommunityId,
        channel_id: &str,
        text: &str,
        author_pubkey: &str,
        route: Option<WorkflowMessageRoute>,
    ) -> Pin<Box<dyn Future<Output = Result<String, ActionSinkError>> + Send + '_>> {
        let channel_id = channel_id.to_owned();
        let text = text.to_owned();
        let author_pubkey = author_pubkey.to_owned();

        Box::pin(async move {
            // 0. Upgrade weak reference — fails only during shutdown.
            let state = self
                .state
                .upgrade()
                .ok_or_else(|| ActionSinkError::Database("relay is shutting down".into()))?;

            // The run carries its owning community (`community_id`); the
            // relay-signed kind:9 message belongs to *that* community, never the
            // deployment default. Re-deriving the tenant from `config.relay_url`
            // would post a community-B workflow's output into the deployment/
            // default community under N>1. Read the community's host back to
            // form a complete TenantContext (host is for labelling only — the
            // community is already fixed and is never re-derived from it). Fail
            // closed if the community no longer maps to a host.
            let host = state
                .db
                .lookup_community_host(community_id)
                .await
                .map_err(|e| ActionSinkError::Database(e.to_string()))?
                .ok_or_else(|| {
                    ActionSinkError::Database(format!(
                        "workflow run community {community_id} is not mapped to a host"
                    ))
                })?;
            let tenant = buzz_core::tenant::TenantContext::resolved(community_id, host);

            // 1. Validate content is not empty/whitespace-only
            if text.trim().is_empty() {
                return Err(ActionSinkError::EmptyContent);
            }

            // 2. Parse and validate channel — canonicalize UUID immediately
            let channel_uuid = Uuid::parse_str(&channel_id)
                .map_err(|e| ActionSinkError::InvalidInput(format!("invalid UUID: {e}")))?;
            let channel_id_canonical = channel_uuid.to_string();

            let author_pubkey = nostr::PublicKey::from_hex(&author_pubkey).map_err(|e| {
                ActionSinkError::InvalidInput(format!("invalid author pubkey: {e}"))
            })?;
            let author_pubkey_bytes = author_pubkey.to_bytes().to_vec();
            let author_pubkey_hex = author_pubkey.to_hex();

            if route.is_none() {
                let channel = state
                    .db
                    .get_channel(tenant.community(), channel_uuid)
                    .await
                    .map_err(|e| match &e {
                        buzz_db::DbError::ChannelNotFound(_) | buzz_db::DbError::NotFound(_) => {
                            ActionSinkError::ChannelNotFound(channel_id_canonical.clone())
                        }
                        _ => ActionSinkError::Database(e.to_string()),
                    })?;

                if channel.archived_at.is_some() {
                    return Err(ActionSinkError::ChannelArchived(
                        channel_id_canonical.clone(),
                    ));
                }

                let is_member = state
                    .is_member_cached(tenant.community(), channel_uuid, &author_pubkey_bytes)
                    .await
                    .map_err(|e| ActionSinkError::Database(e.to_string()))?;
                if !is_member && channel.visibility != "open" {
                    return Err(ActionSinkError::InvalidInput(
                        "workflow owner does not have access to destination channel".into(),
                    ));
                }
            }

            // Resolve `@Name` mentions before the final dynamic revalidation
            // so no async lookup sits between a successful revalidation and
            // EventBuilder / insert_event_with_thread_metadata.
            let members = state
                .db
                .get_members(tenant.community(), channel_uuid)
                .await
                .map_err(|e| ActionSinkError::Database(e.to_string()))?;
            let member_pubkeys: Vec<Vec<u8>> = members.iter().map(|m| m.pubkey.clone()).collect();
            let users = state
                .db
                .get_users_bulk(tenant.community(), &member_pubkeys)
                .await
                .map_err(|e| ActionSinkError::Database(e.to_string()))?;
            let named_members: Vec<(String, String)> = users
                .into_iter()
                .filter_map(|u| {
                    let name = u.display_name?;
                    Some((name, nostr::PublicKey::from_slice(&u.pubkey).ok()?.to_hex()))
                })
                .collect();
            let mut mention_tags = Vec::new();
            for mentioned in resolve_mention_pubkeys(&text, &named_members) {
                if mentioned == author_pubkey_hex {
                    continue;
                }
                mention_tags.push(
                    Tag::parse(["p", &mentioned])
                        .map_err(|e| ActionSinkError::EventBuild(format!("mention p tag: {e}")))?,
                );
            }

            if let Some(route) = &route {
                revalidate_dynamic_route(
                    &state,
                    community_id,
                    channel_uuid,
                    &author_pubkey_bytes,
                    route,
                )
                .await?;
            }

            // 3. Build kind:9 Nostr event
            //    - Signed by relay keypair (event.pubkey = relay pubkey)
            //    - `p` tag attributes the message to the workflow owner
            //    - `h` tag scopes to the channel (NIP-29, canonical UUID)
            //    - `buzz:workflow` tag prevents recursive workflow triggering
            //    - `buzz:workflow-owner` tag names the workflow owner explicitly,
            //      so consumers (e.g. the ACP inbound author gate) can attribute
            //      the message without inferring ownership from `p`-tag order
            //    - one `p` tag per `@Name` that resolves to a channel member,
            //      so mentioned agents are woken (wake is `p`-tag gated)
            //    - dynamic routes also carry repository/project `a` tags and
            //      `buzz:workflow-run` (no raw idempotency key)
            let mut tags = vec![
                Tag::parse(["p", &author_pubkey_hex])
                    .map_err(|e| ActionSinkError::EventBuild(format!("p tag: {e}")))?,
                Tag::parse(["h", &channel_id_canonical])
                    .map_err(|e| ActionSinkError::EventBuild(format!("h tag: {e}")))?,
                Tag::parse(["buzz:workflow", "true"])
                    .map_err(|e| ActionSinkError::EventBuild(format!("workflow tag: {e}")))?,
                Tag::parse(["buzz:workflow-owner", &author_pubkey_hex])
                    .map_err(|e| ActionSinkError::EventBuild(format!("workflow owner tag: {e}")))?,
            ];
            tags.extend(mention_tags);
            if let Some(route) = &route {
                tags.extend(route_provenance_tags(route)?);
            }

            let kind = Kind::from(KIND_STREAM_MESSAGE as u16);
            let event = EventBuilder::new(kind, &text)
                .tags(tags)
                .sign_with_keys(&state.relay_keypair)
                .map_err(|e| ActionSinkError::EventBuild(format!("signing: {e}")))?;

            let event_id_hex = event.id.to_hex();
            let event_id_bytes = event.id.as_bytes().to_vec();
            let kind_u32 = KIND_STREAM_MESSAGE;

            let event_created_at = {
                let ts = event.created_at.as_secs() as i64;
                chrono::DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now)
            };

            info!(
                event_id = %event_id_hex,
                channel_id = %channel_id_canonical,
                author = %author_pubkey,
                "Workflow SendMessage: posting kind {kind_u32} event"
            );

            // 4. Persist event with thread metadata (matches REST handler path).
            //    Workflow messages are always top-level: depth=0, no parent/root.
            let thread_meta = Some(buzz_db::event::ThreadMetadataParams {
                event_id: &event_id_bytes,
                event_created_at,
                channel_id: channel_uuid,
                parent_event_id: None,
                parent_event_created_at: None,
                root_event_id: None,
                root_event_created_at: None,
                depth: 0,
                broadcast: false,
            });

            let (stored_event, was_inserted) = state
                .db
                .insert_event_with_thread_metadata(
                    tenant.community(),
                    &event,
                    Some(channel_uuid),
                    thread_meta,
                )
                .await
                .map_err(|e| ActionSinkError::Database(e.to_string()))?;

            // 5. Post-persist side effects (fan-out, search, audit)
            //    Only if actually inserted (idempotency guard).
            if was_inserted {
                let _ = dispatch_persistent_event(
                    &tenant,
                    &state,
                    &stored_event,
                    kind_u32,
                    &author_pubkey_hex,
                    None,
                )
                .await;
            }

            Ok(event_id_hex)
        })
    }
}

async fn revalidate_dynamic_route(
    state: &AppState,
    community_id: CommunityId,
    channel_id: Uuid,
    author_pubkey: &[u8],
    route: &WorkflowMessageRoute,
) -> Result<(), ActionSinkError> {
    let workflow = state
        .db
        .get_workflow(community_id, route.workflow_id)
        .await
        .map_err(|_| ActionSinkError::RouteStale)?;
    if !workflow.enabled || workflow.status != WorkflowStatus::Active {
        return Err(ActionSinkError::RouteStale);
    }
    if workflow.channel_id != Some(route.home_channel_id)
        || workflow.owner_pubkey.as_slice() != author_pubkey
    {
        return Err(ActionSinkError::RouteStale);
    }

    let def: WorkflowDef =
        serde_json::from_value(workflow.definition).map_err(|_| ActionSinkError::RouteStale)?;
    state
        .workflow_engine
        .check_owner_authority(
            community_id,
            route.home_channel_id,
            &workflow.owner_pubkey,
            &def,
        )
        .await
        .map_err(|_| ActionSinkError::RouteStale)?;

    let parsed = parse_repository_coordinate(&route.repository_coordinate)
        .map_err(|_| ActionSinkError::RouteStale)?;
    let repository_event = state
        .db
        .get_latest_parameterized_head(
            community_id,
            KIND_GIT_REPO_ANNOUNCEMENT as i32,
            parsed.owner_pubkey().as_slice(),
            parsed.d_tag(),
        )
        .await
        .map_err(|_| ActionSinkError::RouteStale)?
        .ok_or(ActionSinkError::RouteStale)?;
    let repository =
        repository_head_from_event(&repository_event.event).ok_or(ActionSinkError::RouteStale)?;
    if repository.coordinate != route.repository_coordinate {
        return Err(ActionSinkError::RouteStale);
    }

    let project_events = state
        .db
        .list_latest_parameterized_heads(community_id, KIND_PROJECT as i32)
        .await
        .map_err(|_| ActionSinkError::RouteStale)?;
    let projects: Vec<_> = project_events
        .iter()
        .filter_map(|stored| project_head_from_event(&stored.event))
        .collect();
    require_exact_stored_project_channel(
        &repository,
        &projects,
        &route.project_coordinate,
        channel_id,
    )
    .map_err(|_| ActionSinkError::RouteStale)?;

    let channel = state
        .db
        .get_channel(community_id, channel_id)
        .await
        .map_err(|_| ActionSinkError::RouteStale)?;
    if channel.archived_at.is_some() {
        return Err(ActionSinkError::RouteStale);
    }

    let role = state
        .db
        .get_member_role(community_id, channel_id, author_pubkey)
        .await
        .map_err(|_| ActionSinkError::RouteStale)?;
    if role.is_none() {
        return Err(ActionSinkError::RouteStale);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(name: &str, pubkey: &str) -> (String, String) {
        (name.to_string(), pubkey.to_string())
    }

    // A 64-char hex pubkey built from a single repeated nibble, for readable tests.
    fn pk(nibble: char) -> String {
        std::iter::repeat_n(nibble, 64).collect()
    }

    #[test]
    fn resolves_exact_member_name() {
        let members = vec![m("Robby", &pk('a'))];
        assert_eq!(
            resolve_mention_pubkeys("heads up @Robby — please take a look", &members),
            vec![pk('a')]
        );
    }

    #[test]
    fn matches_case_insensitively() {
        let members = vec![m("Robby", &pk('a'))];
        assert_eq!(
            resolve_mention_pubkeys("ping @robby", &members),
            vec![pk('a')]
        );
    }

    #[test]
    fn ignores_non_member_and_bare_at() {
        let members = vec![m("Robby", &pk('a'))];
        assert!(resolve_mention_pubkeys("hey @Stranger and @", &members).is_empty());
    }

    #[test]
    fn greedy_longest_binds_full_name_not_prefix() {
        // Both "Will" and "Will Pfleger" are members. `@Will Pfleger` must bind
        // Pfleger's key only; a bare `@Will` binds Will.
        let members = vec![m("Will", &pk('1')), m("Will Pfleger", &pk('2'))];
        assert_eq!(
            resolve_mention_pubkeys("cc @Will Pfleger on this", &members),
            vec![pk('2')]
        );
        assert_eq!(
            resolve_mention_pubkeys("cc @Will on this", &members),
            vec![pk('1')]
        );
    }

    #[test]
    fn at_mid_token_does_not_match() {
        // `@` must sit at a left boundary (start / whitespace / `(`). An email-ish
        // or mid-token `@` (`alice@Robby`) must not wake Robby.
        let members = vec![m("Robby", &pk('a'))];
        assert!(resolve_mention_pubkeys("alice@Robby", &members).is_empty());
    }

    #[test]
    fn prefix_member_does_not_match_inside_longer_word() {
        // "Sam" is a member; `@Sami` (no "Sami" member) must not wake Sam.
        let members = vec![m("Sam", &pk('3'))];
        assert!(resolve_mention_pubkeys("hi @Sami", &members).is_empty());
    }

    #[test]
    fn name_with_spaces_and_punctuation() {
        let members = vec![m("Lep (Subagent)", &pk('4'))];
        assert_eq!(
            resolve_mention_pubkeys("@Lep (Subagent) take it", &members),
            vec![pk('4')]
        );
    }

    #[test]
    fn em_dash_terminates_name() {
        // Generated prose often writes `@Name—text` with no space.
        let members = vec![m("Robby", &pk('a'))];
        assert_eq!(
            resolve_mention_pubkeys("@Robby—please look", &members),
            vec![pk('a')]
        );
    }

    #[test]
    fn non_ascii_member_name() {
        let members = vec![m("Zoë", &pk('5'))];
        assert_eq!(
            resolve_mention_pubkeys("welcome @Zoë!", &members),
            vec![pk('5')]
        );
    }

    #[test]
    fn lowercase_expansion_does_not_shift_later_mentions() {
        // Regression (Wren's redteam counterexample): `İ` (U+0130) lowercases to
        // TWO code points (`i` + U+0307). A design that pre-lowercases the whole
        // text and indexes it in parallel with the original chars desyncs after
        // the expansion, dropping every later valid mention. `@İ @Robby` must
        // resolve BOTH members, in order.
        let members = vec![m("İ", &pk('c')), m("Robby", &pk('a'))];
        assert_eq!(
            resolve_mention_pubkeys("@İ @Robby", &members),
            vec![pk('c'), pk('a')]
        );
    }

    #[test]
    fn sharp_s_matches_case_insensitively() {
        // `ẞ` (U+1E9E capital sharp s) lowercases to `ß` (U+00DF) — a single
        // char, NOT `ss` (that's uppercase/full-case-fold behavior, not
        // `char::to_lowercase`). Covers non-ASCII case-insensitive matching, and
        // that a later mention still resolves after it.
        let members = vec![m("ẞ", &pk('d')), m("Max", &pk('b'))];
        assert_eq!(
            resolve_mention_pubkeys("@ẞ and @Max", &members),
            vec![pk('d'), pk('b')]
        );
    }

    // Adversarial rows from Quinn's re-review (the two `ẞ→ss`-premised ones were
    // dropped as vacuous — `ẞ` lowercases to `ß`, one char, so it never inverts
    // original-vs-folded length; only `İ` does).

    #[test]
    fn combining_mark_in_name_matches() {
        // A name carrying a combining mark (`é` as `e` + U+0301) matches the same
        // sequence in text (1:1 folding) and terminates cleanly.
        let members = vec![m("Jos\u{0065}\u{0301}", &pk('4'))]; // "José" decomposed
        assert_eq!(
            resolve_mention_pubkeys("hi @Jos\u{0065}\u{0301}!", &members),
            vec![pk('4')]
        );
    }

    #[test]
    fn expanding_name_at_trailing_boundary() {
        // Expansion at the very end: `@İ` with nothing after must match, and
        // `@İx` (x extends the name, no `İx` member) must NOT match `İ`.
        let members = vec![m("İ", &pk('5'))];
        assert_eq!(resolve_mention_pubkeys("@İ", &members), vec![pk('5')]);
        assert!(resolve_mention_pubkeys("@İx", &members).is_empty());
    }

    #[test]
    fn back_to_back_at_is_one_mention() {
        // `@İ@Robby`: the second `@` is preceded by a name char (`İ`), so it is
        // NOT at a left boundary — same rule as `alice@Robby`. Back-to-back
        // `@a@b` is intentionally one mention; a separator is required to wake
        // both. The expanding first name (`İ` → 2 folded chars) also proves the
        // span accounting stays in original coordinates.
        let members = vec![m("İ", &pk('5')), m("Robby", &pk('a'))];
        assert_eq!(resolve_mention_pubkeys("@İ@Robby", &members), vec![pk('5')]);
        // ASCII control: same shape, same outcome — it's the boundary rule, not
        // a Unicode span-accounting bug.
        let ascii = vec![m("Sam", &pk('6')), m("Robby", &pk('a'))];
        assert_eq!(resolve_mention_pubkeys("@Sam@Robby", &ascii), vec![pk('6')]);
        // With a separator, both wake.
        assert_eq!(
            resolve_mention_pubkeys("@İ @Robby", &members),
            vec![pk('5'), pk('a')]
        );
    }

    #[test]
    fn ambiguous_name_wakes_no_one() {
        // Six "Fizz" agents (real team case) with distinct pubkeys → tag none.
        let members = vec![
            m("Fizz", &pk('6')),
            m("Fizz", &pk('7')),
            m("Fizz", &pk('8')),
        ];
        assert!(resolve_mention_pubkeys("@Fizz status?", &members).is_empty());
    }

    #[test]
    fn duplicate_name_same_pubkey_is_not_ambiguous() {
        // Same identity listed twice (e.g. two channels) is not a conflict.
        let members = vec![m("Fizz", &pk('6')), m("Fizz", &pk('6'))];
        assert_eq!(resolve_mention_pubkeys("@Fizz go", &members), vec![pk('6')]);
    }

    #[test]
    fn dedupes_repeated_mentions_in_first_appearance_order() {
        let members = vec![m("Robby", &pk('a')), m("Max", &pk('b'))];
        assert_eq!(
            resolve_mention_pubkeys("@Max then @Robby then @Max again", &members),
            vec![pk('b'), pk('a')]
        );
    }

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
        let parts: Vec<Vec<String>> = tags.iter().map(|tag| tag.as_slice().to_vec()).collect();
        assert!(parts.contains(&vec!["a".into(), route.repository_coordinate.clone()]));
        assert!(parts.contains(&vec!["a".into(), route.project_coordinate.clone()]));
        assert!(parts.contains(&vec!["buzz:workflow-run".into(), route.run_id.to_string(),]));
        assert!(!serde_json::to_string(&parts)
            .expect("serialize tag parts")
            .contains("idempotency_key"));
    }
}

#[cfg(test)]
mod integration_tests {
    //! Regression test for `e3661764` / `7899c1a8`: a workflow `send_message`
    //! that mentions a channel member by name (`@Name`) must emit a `p` tag for
    //! that member so ACP agent wake (`event_mentions_agent`, p-tag gated) fires.
    //!
    //! Postgres-gated like the other DB-backed relay tests. Run with:
    //!   `cargo test -p buzz-relay --lib workflow_sink -- --ignored`
    use super::*;
    use buzz_core::channel::{ChannelType, ChannelVisibility, MemberRole};
    use buzz_db::CreateCommunityWithOwnerResult;
    use std::sync::Arc;

    /// Real-PG state mirroring `handlers::event::tests::test_state_with_redis_url`.
    async fn test_state() -> Arc<AppState> {
        let mut config = crate::config::Config::from_env().expect("default config loads");
        config.require_relay_membership = false;
        config.redis_url = "redis://127.0.0.1:1".to_string();
        let pool = sqlx::PgPool::connect_lazy(&config.database_url).expect("lazy pg pool");
        let db = buzz_db::Db::from_pool(pool.clone());
        let redis_pool = deadpool_redis::Config::from_url(&config.redis_url)
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .expect("redis pool");
        let pubsub = Arc::new(
            buzz_pubsub::PubSubManager::new(&config.redis_url, redis_pool.clone())
                .await
                .expect("pubsub manager"),
        );
        let audit = buzz_audit::AuditService::new(pool.clone());
        let auth = buzz_auth::AuthService::new(config.auth.clone());
        let search = buzz_search::SearchService::new(pool.clone());
        let workflow_engine = Arc::new(buzz_workflow::WorkflowEngine::new(
            db.clone(),
            buzz_workflow::WorkflowConfig::default(),
        ));
        let media_storage = buzz_media::MediaStorage::new(&config.media).expect("media storage");
        let (state, _audit_shutdown) = AppState::new(
            config,
            db,
            redis_pool,
            audit,
            pubsub,
            auth,
            search,
            workflow_engine,
            nostr::Keys::generate(),
            media_storage,
        );
        Arc::new(state)
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn workflow_send_message_p_tags_mentioned_member() {
        let state = test_state().await;

        let author = nostr::Keys::generate();
        let author_hex = author.public_key().to_hex();
        let agent = nostr::Keys::generate();
        let agent_hex = agent.public_key().to_hex();
        let agent_bytes = agent.public_key().to_bytes().to_vec();

        let host = format!("wf-ptag-{}.example", uuid::Uuid::new_v4().simple());
        let community = match state
            .db
            .create_community_with_owner(&host, &author_hex)
            .await
            .expect("create community")
        {
            CreateCommunityWithOwnerResult::Created(rec) => rec.id,
            other => panic!("expected fresh community, got {other:?}"),
        };

        // Open channel; the creator (author) is bootstrapped as an owner-member.
        let channel = state
            .db
            .create_channel(
                community,
                "wf-ptag",
                ChannelType::Stream,
                ChannelVisibility::Open,
                None,
                &author.public_key().to_bytes(),
                None,
            )
            .await
            .expect("create channel");

        // The mentioned agent is a real member with a resolvable display name.
        state
            .db
            .ensure_user(community, &agent_bytes)
            .await
            .expect("ensure agent user row");
        state
            .db
            .update_user_profile(community, &agent_bytes, Some("Robby"), None, None, None)
            .await
            .expect("set agent display name");
        state
            .db
            .add_member(
                community,
                channel.id,
                &agent_bytes,
                MemberRole::Bot,
                Some(&author.public_key().to_bytes()),
            )
            .await
            .expect("add agent member");

        let sink = RelayActionSink::new(&state);
        let event_id_hex = sink
            .send_message(
                community,
                &channel.id.to_string(),
                "heads up @Robby — please take a look",
                &author_hex,
                None,
            )
            .await
            .expect("send_message");

        let id_bytes = nostr::EventId::from_hex(&event_id_hex)
            .expect("event id")
            .as_bytes()
            .to_vec();
        let stored = state
            .db
            .get_event_by_id(community, &id_bytes)
            .await
            .expect("query event")
            .expect("event persisted");

        let p_tag_targets: Vec<&str> = stored
            .event
            .tags
            .iter()
            .filter(|t| t.as_slice().first().map(|s| s.as_str()) == Some("p"))
            .filter_map(|t| t.as_slice().get(1).map(|s| s.as_str()))
            .collect();

        assert!(
            p_tag_targets.contains(&author_hex.as_str()),
            "author should still be attributed via p tag; got {p_tag_targets:?}"
        );
        assert!(
            p_tag_targets.contains(&agent_hex.as_str()),
            "mentioned member {agent_hex} must be p-tagged so it wakes; got {p_tag_targets:?}"
        );

        let owner_tag = stored
            .event
            .tags
            .iter()
            .find(|t| t.as_slice().first().map(|s| s.as_str()) == Some("buzz:workflow-owner"))
            .and_then(|t| t.as_slice().get(1).map(|s| s.as_str()));
        assert_eq!(
            owner_tag,
            Some(author_hex.as_str()),
            "workflow owner must be named explicitly via buzz:workflow-owner \
             so consumers never infer ownership from p-tag order"
        );
    }

    #[derive(Clone, Copy)]
    enum ClaimSignerMode {
        Owner,
        Maintainer,
    }

    struct DynamicRouteFixture {
        state: Arc<AppState>,
        sink: RelayActionSink,
        community_id: CommunityId,
        home_channel_id: Uuid,
        destination_channel_id: Uuid,
        route: WorkflowMessageRoute,
        raw_test_key: String,
        baseline_counts: std::collections::HashMap<Uuid, i64>,
        pool: sqlx::PgPool,
        owner_hex: String,
        owner_bytes: Vec<u8>,
        admin_bytes: Vec<u8>,
        repo_keys: nostr::Keys,
        project_keys: nostr::Keys,
        repo_d: String,
        project_d: String,
        repo_event_id: Vec<u8>,
        project_event_id: Vec<u8>,
        repo_created_at: u64,
        project_created_at: u64,
    }

    fn signed_repo_event(
        keys: &nostr::Keys,
        d: &str,
        maintainers: &[String],
        created_at: u64,
    ) -> nostr::Event {
        let mut tags = vec![
            Tag::parse(["d", d]).expect("d tag"),
            Tag::parse(["name", d]).expect("name tag"),
        ];
        for maintainer in maintainers {
            tags.push(Tag::parse(["maintainers", maintainer]).expect("maintainers tag"));
        }
        EventBuilder::new(Kind::from(30617u16), "")
            .tags(tags)
            .custom_created_at(nostr::Timestamp::from(created_at))
            .sign_with_keys(keys)
            .expect("sign repo")
    }

    fn signed_project_event(
        keys: &nostr::Keys,
        d: &str,
        repo_coord: &str,
        channel_id: Uuid,
        created_at: u64,
    ) -> nostr::Event {
        EventBuilder::new(Kind::from(30621u16), "")
            .tags(vec![
                Tag::parse(["d", d]).expect("d tag"),
                Tag::parse(["a", repo_coord]).expect("a tag"),
                Tag::parse(["buzz-channel", &channel_id.to_string()]).expect("buzz-channel"),
            ])
            .custom_created_at(nostr::Timestamp::from(created_at))
            .sign_with_keys(keys)
            .expect("sign project")
    }

    fn assert_exact_dynamic_provenance(event: &nostr::Event, route: &WorkflowMessageRoute) {
        let tags: Vec<Vec<String>> = event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect();
        let named = |name: &str| {
            tags.iter()
                .filter(|tag| tag.first().map(String::as_str) == Some(name))
                .count()
        };
        let exact = |slice: &[String]| tags.iter().filter(|tag| tag.as_slice() == slice).count();
        let run_id = route.run_id.to_string();
        assert_eq!(
            exact(&["a".into(), route.repository_coordinate.clone()]),
            1,
            "exactly one repository a tag"
        );
        assert_eq!(
            exact(&["a".into(), route.project_coordinate.clone()]),
            1,
            "exactly one project a tag"
        );
        assert_eq!(named("a"), 2, "exactly two a tags");
        assert_eq!(
            exact(&["buzz:workflow-run".into(), run_id]),
            1,
            "exactly one buzz:workflow-run tag"
        );
        assert_eq!(named("h"), 1, "exactly one destination h tag");
        assert_eq!(named("buzz:workflow"), 1, "exactly one buzz:workflow tag");
        assert_eq!(
            named("buzz:workflow-owner"),
            1,
            "exactly one buzz:workflow-owner tag"
        );
        assert_eq!(named("e"), 0, "no root/parent e tag");
    }

    impl DynamicRouteFixture {
        async fn new(mode: ClaimSignerMode) -> Self {
            let state = test_state().await;
            let pool = sqlx::PgPool::connect(&state.config.database_url)
                .await
                .expect("connect fixture pool");

            let admin = nostr::Keys::generate();
            let owner = nostr::Keys::generate();
            let repo_keys = nostr::Keys::generate();
            let project_keys = match mode {
                ClaimSignerMode::Owner => repo_keys.clone(),
                ClaimSignerMode::Maintainer => nostr::Keys::generate(),
            };
            let admin_hex = admin.public_key().to_hex();
            let admin_bytes = admin.public_key().to_bytes().to_vec();
            let owner_hex = owner.public_key().to_hex();
            let owner_bytes = owner.public_key().to_bytes().to_vec();

            let host = format!("wf-dyn-{}.example", Uuid::new_v4().simple());
            let community = match state
                .db
                .create_community_with_owner(&host, &admin_hex)
                .await
                .expect("create community")
            {
                CreateCommunityWithOwnerResult::Created(rec) => rec.id,
                other => panic!("expected fresh community, got {other:?}"),
            };

            let home = state
                .db
                .create_channel(
                    community,
                    "wf-home",
                    ChannelType::Stream,
                    ChannelVisibility::Private,
                    None,
                    &admin_bytes,
                    None,
                )
                .await
                .expect("create home channel");
            let destination = state
                .db
                .create_channel(
                    community,
                    "wf-dest",
                    ChannelType::Stream,
                    ChannelVisibility::Open,
                    None,
                    &admin_bytes,
                    None,
                )
                .await
                .expect("create destination channel");

            state
                .db
                .ensure_user(community, &owner_bytes)
                .await
                .expect("ensure workflow owner");
            state
                .db
                .add_member(
                    community,
                    home.id,
                    &owner_bytes,
                    MemberRole::Member,
                    Some(&admin_bytes),
                )
                .await
                .expect("add owner to home");
            state
                .db
                .add_member(
                    community,
                    destination.id,
                    &owner_bytes,
                    MemberRole::Member,
                    Some(&admin_bytes),
                )
                .await
                .expect("add owner to destination");

            let definition = serde_json::json!({
                "name": "dynamic-fixture",
                "trigger": {"on": "webhook"},
                "routing": {"mode": "project_channel_by_repository"},
                "steps": [{"id": "notify", "action": "send_message", "text": "hello"}],
                "enabled": true,
            })
            .to_string();
            let workflow_id = state
                .db
                .create_workflow(
                    community,
                    Some(home.id),
                    &owner_bytes,
                    "dynamic-fixture",
                    &definition,
                    &[0u8; 32],
                )
                .await
                .expect("create workflow");

            let repo_d = "agentic-os-plan".to_string();
            let project_d = "gigo-harness".to_string();
            let repo_hex = repo_keys.public_key().to_hex();
            let project_hex = project_keys.public_key().to_hex();
            let repository_coordinate = format!("30617:{repo_hex}:{repo_d}");
            let project_coordinate = format!("30621:{project_hex}:{project_d}");
            let now = chrono::Utc::now().timestamp() as u64;
            let repo_created_at = now;
            let project_created_at = now + 1;
            let maintainers = match mode {
                ClaimSignerMode::Owner => Vec::new(),
                ClaimSignerMode::Maintainer => vec![project_hex.clone()],
            };
            let repo_event = signed_repo_event(&repo_keys, &repo_d, &maintainers, repo_created_at);
            let project_event = signed_project_event(
                &project_keys,
                &project_d,
                &repository_coordinate,
                destination.id,
                project_created_at,
            );
            state
                .db
                .insert_event(community, &repo_event, None)
                .await
                .expect("insert repo");
            state
                .db
                .insert_event(community, &project_event, None)
                .await
                .expect("insert project");

            let route = WorkflowMessageRoute {
                run_id: Uuid::new_v4(),
                workflow_id,
                home_channel_id: home.id,
                repository_coordinate,
                project_coordinate,
            };
            let sink = RelayActionSink::new(&state);
            let mut fixture = Self {
                state,
                sink,
                community_id: community,
                home_channel_id: home.id,
                destination_channel_id: destination.id,
                route,
                raw_test_key: format!("idempotency_key-{}", Uuid::new_v4()),
                baseline_counts: std::collections::HashMap::new(),
                pool,
                owner_hex,
                owner_bytes,
                admin_bytes,
                repo_keys,
                project_keys,
                repo_d,
                project_d,
                repo_event_id: repo_event.id.as_bytes().to_vec(),
                project_event_id: project_event.id.as_bytes().to_vec(),
                repo_created_at,
                project_created_at,
            };
            fixture.track_channel(home.id).await;
            fixture.track_channel(destination.id).await;
            fixture
        }

        async fn kind9_count(&self, channel_id: Uuid) -> i64 {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM events WHERE community_id = $1 AND channel_id = $2 AND kind = 9 AND deleted_at IS NULL",
            )
            .bind(self.community_id.as_uuid())
            .bind(channel_id)
            .fetch_one(&self.pool)
            .await
            .expect("count kind 9")
        }

        async fn track_channel(&mut self, channel_id: Uuid) {
            let count = self.kind9_count(channel_id).await;
            self.baseline_counts.insert(channel_id, count);
        }

        async fn send(&self) -> Result<String, ActionSinkError> {
            self.sink
                .send_message(
                    self.community_id,
                    &self.destination_channel_id.to_string(),
                    "routed workflow message",
                    &self.owner_hex,
                    Some(self.route.clone()),
                )
                .await
        }

        async fn assert_no_new_kind9(&self) {
            for (channel_id, expected) in &self.baseline_counts {
                let actual = self.kind9_count(*channel_id).await;
                assert_eq!(
                    actual, *expected,
                    "kind 9 count changed for channel {channel_id}"
                );
            }
        }

        async fn load_sent_event(&self, event_id: &str) -> buzz_core::StoredEvent {
            let id_bytes = nostr::EventId::from_hex(event_id)
                .expect("event id")
                .as_bytes()
                .to_vec();
            self.state
                .db
                .get_event_by_id(self.community_id, &id_bytes)
                .await
                .expect("query event")
                .expect("event persisted")
        }

        async fn extra_channel(&mut self, name: &str) -> Uuid {
            let channel = self
                .state
                .db
                .create_channel(
                    self.community_id,
                    name,
                    ChannelType::Stream,
                    ChannelVisibility::Open,
                    None,
                    &self.admin_bytes,
                    None,
                )
                .await
                .expect("create extra channel");
            self.track_channel(channel.id).await;
            channel.id
        }

        async fn remove_destination_member(&mut self) {
            self.state
                .db
                .remove_member(
                    self.community_id,
                    self.destination_channel_id,
                    &self.owner_bytes,
                    &self.admin_bytes,
                )
                .await
                .expect("remove destination member");
        }

        async fn disable_workflow(&mut self) {
            self.state
                .db
                .set_workflow_enabled(self.community_id, self.route.workflow_id, false)
                .await
                .expect("disable workflow");
        }

        async fn soft_delete_repository(&mut self) {
            self.state
                .db
                .soft_delete_event(self.community_id, &self.repo_event_id)
                .await
                .expect("soft-delete repository");
        }

        async fn soft_delete_project(&mut self) {
            self.state
                .db
                .soft_delete_event(self.community_id, &self.project_event_id)
                .await
                .expect("soft-delete project");
        }

        async fn replace_repository_without_maintainer(&mut self) {
            let created_at = self.repo_created_at + 10;
            let event = signed_repo_event(&self.repo_keys, &self.repo_d, &[], created_at);
            self.state
                .db
                .insert_event(self.community_id, &event, None)
                .await
                .expect("replace repository without maintainer");
            self.repo_event_id = event.id.as_bytes().to_vec();
            self.repo_created_at = created_at;
        }

        async fn publish_second_claim(&mut self) {
            let extra = self.extra_channel("wf-second-dest").await;
            let created_at = self.project_created_at + 10;
            let event = signed_project_event(
                &self.project_keys,
                "second-claim",
                &self.route.repository_coordinate,
                extra,
                created_at,
            );
            self.state
                .db
                .insert_event(self.community_id, &event, None)
                .await
                .expect("publish second claim");
        }

        async fn replace_project_channel(&mut self) {
            let extra = self.extra_channel("wf-changed-dest").await;
            let created_at = self.project_created_at + 10;
            let event = signed_project_event(
                &self.project_keys,
                &self.project_d,
                &self.route.repository_coordinate,
                extra,
                created_at,
            );
            self.state
                .db
                .insert_event(self.community_id, &event, None)
                .await
                .expect("replace project channel");
            self.project_event_id = event.id.as_bytes().to_vec();
            self.project_created_at = created_at;
        }

        async fn archive_destination(&mut self) {
            self.state
                .db
                .archive_channel(self.community_id, self.destination_channel_id)
                .await
                .expect("archive destination");
        }

        async fn soft_delete_destination(&mut self) {
            self.state
                .db
                .soft_delete_channel(self.community_id, self.destination_channel_id)
                .await
                .expect("soft-delete destination");
        }

        async fn remove_home_member(&mut self) {
            self.state
                .db
                .remove_member(
                    self.community_id,
                    self.home_channel_id,
                    &self.owner_bytes,
                    &self.admin_bytes,
                )
                .await
                .expect("remove home member");
        }
    }

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
}
