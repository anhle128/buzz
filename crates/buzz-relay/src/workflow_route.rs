//! Repository identity resolution and project claim authorization.
//!
//! Pure webhook routing helpers: map a callback `repository_name` onto a live
//! NIP-34 repository coordinate, then count listed owner-or-maintainer project
//! claims. No database access.

use std::collections::{BTreeMap, BTreeSet};

use buzz_sdk::ProjectMemberCoord;
use buzz_workflow::routing::parse_repository_coordinate;
use buzz_workflow::RouteFailure;
use uuid::Uuid;

/// Latest live kind `30617` repository identity used by webhook routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryHead {
    /// Canonical `30617:<owner>:<d>` coordinate.
    pub coordinate: String,
    /// Repository `d` tag.
    pub d_tag: String,
    /// First `name` tag value, if present.
    pub name: Option<String>,
    /// Basenames derived from every `clone` tag value.
    pub clone_basenames: Vec<String>,
    /// Repository owner pubkey hex.
    pub owner_hex: String,
    /// Every `maintainers` tag value.
    pub maintainers: Vec<String>,
}

/// Latest live kind `30621` project identity used by webhook routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectHead {
    /// Canonical `30621:<signer>:<d>` coordinate.
    pub coordinate: String,
    /// Project event signer pubkey hex.
    pub signer_hex: String,
    /// Member `a` tag coordinates accepted by [`ProjectMemberCoord::parse_full`].
    pub member_coordinates: Vec<String>,
    /// Destination channel id when the `buzz-channel` tag is well-formed.
    pub buzz_channel: Option<String>,
    /// False only when the first `buzz-visibility` value is exactly `unlisted`.
    pub listed: bool,
}

/// Identity tier that uniquely resolved a repository name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityTier {
    /// Exact `d` tag match.
    DTag,
    /// Exact alias key match.
    Alias,
    /// Exact clone-URL basename match.
    CloneBasename,
    /// Exact display-name match.
    DisplayName,
}

impl IdentityTier {
    /// Stable machine-readable tier name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DTag => "d_tag",
            Self::Alias => "alias",
            Self::CloneBasename => "clone_basename",
            Self::DisplayName => "display_name",
        }
    }
}

/// Last path segment of a clone URL, with at most one trailing `.git` stripped.
///
/// Does not trim or lowercase `value`. A trailing slash yields no basename.
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

/// Parse a kind `30617` event into a routing repository head.
///
/// Uses the first `d` value (same replacement key as `extract_d_tag`). Returns
/// `None` when that value is missing or empty.
pub fn repository_head_from_event(event: &nostr::Event) -> Option<RepositoryHead> {
    if event.kind.as_u16() != 30617 {
        return None;
    }
    let d_tag = first_tag_value(event, "d")
        .filter(|d| !d.is_empty())?
        .to_string();
    let name = first_tag_value(event, "name").map(str::to_string);
    let clone_basenames = all_tag_values(event, "clone")
        .into_iter()
        .filter_map(clone_basename)
        .collect();
    let maintainers = all_tag_values(event, "maintainers")
        .into_iter()
        .map(str::to_string)
        .collect();
    let owner_hex = event.pubkey.to_hex();
    let coordinate = format!("30617:{owner_hex}:{d_tag}");
    Some(RepositoryHead {
        coordinate,
        d_tag,
        name,
        clone_basenames,
        owner_hex,
        maintainers,
    })
}

/// Parse a kind `30621` event into a routing project head.
///
/// Requires exactly one non-empty `d` tag. Malformed `buzz-channel` data is
/// stored as `None` so later routing fails closed.
pub fn project_head_from_event(event: &nostr::Event) -> Option<ProjectHead> {
    if event.kind.as_u16() != 30621 {
        return None;
    }
    let mut d_tags = tags_named(event, "d");
    let d_tag = d_tags
        .next()
        .filter(|tag| tag.len() == 2 && !tag[1].is_empty())?;
    if d_tags.next().is_some() {
        return None;
    }
    let d_tag = d_tag[1].clone();
    let signer_hex = event.pubkey.to_hex();
    let member_coordinates = tags_named(event, "a")
        .filter_map(|tag| tag.get(1))
        .filter_map(|value| ProjectMemberCoord::parse_full(value).ok().map(|m| m.coord))
        .collect();
    let listed = first_tag_value(event, "buzz-visibility") != Some("unlisted");
    let mut channel_tags = tags_named(event, "buzz-channel");
    let buzz_channel = match (channel_tags.next(), channel_tags.next()) {
        (Some(tag), None) if tag.len() == 2 => Some(tag[1].clone()),
        _ => None,
    };
    Some(ProjectHead {
        coordinate: format!("30621:{signer_hex}:{d_tag}"),
        signer_hex,
        member_coordinates,
        buzz_channel,
        listed,
    })
}

/// Resolve `repository_name` against live heads using the fixed identity tiers.
///
/// Order is `d_tag`, then `alias`, then `clone_basename`, then `display_name`.
/// The first tier with any match stops. Duplicate observations of one
/// coordinate are not ambiguous.
pub fn resolve_repository_identity(
    repository_name: &str,
    aliases: &BTreeMap<String, String>,
    heads: &[RepositoryHead],
) -> Result<(String, IdentityTier), RouteFailure> {
    let d_tag_coords = unique_coordinates(heads, |head| head.d_tag == repository_name);
    match d_tag_coords.as_slice() {
        [only] => return Ok(((*only).to_string(), IdentityTier::DTag)),
        [] => {}
        _ => return Err(RouteFailure::RepositoryAmbiguous),
    }

    if let Some(target) = aliases.get(repository_name) {
        let parsed = parse_repository_coordinate(target)
            .map_err(|_| RouteFailure::AliasTargetUnavailable)?;
        let coordinate = parsed.as_coordinate();
        if heads.iter().any(|head| head.coordinate == coordinate) {
            return Ok((coordinate, IdentityTier::Alias));
        }
        return Err(RouteFailure::AliasTargetUnavailable);
    }

    let clone_coords = unique_coordinates(heads, |head| {
        head.clone_basenames
            .iter()
            .any(|basename| basename == repository_name)
    });
    match clone_coords.as_slice() {
        [only] => return Ok(((*only).to_string(), IdentityTier::CloneBasename)),
        [] => {}
        _ => return Err(RouteFailure::RepositoryAmbiguous),
    }

    let name_coords =
        unique_coordinates(heads, |head| head.name.as_deref() == Some(repository_name));
    match name_coords.as_slice() {
        [only] => return Ok(((*only).to_string(), IdentityTier::DisplayName)),
        [] => {}
        _ => return Err(RouteFailure::RepositoryAmbiguous),
    }

    Err(RouteFailure::RepositoryMissing)
}

/// Listed projects that name this repository and are signed by its owner or a maintainer.
pub fn claim_valid_projects<'a>(
    repository: &RepositoryHead,
    projects: &'a [ProjectHead],
) -> Vec<&'a ProjectHead> {
    projects
        .iter()
        .filter(|project| {
            project.listed
                && project
                    .member_coordinates
                    .iter()
                    .any(|coord| coord == &repository.coordinate)
                && (project.signer_hex == repository.owner_hex
                    || repository
                        .maintainers
                        .iter()
                        .any(|maintainer| maintainer == &project.signer_hex))
        })
        .collect()
}

/// Require exactly one claim-valid project before inspecting channel quality.
pub fn authorize_unique_project_route<'a>(
    repository: &RepositoryHead,
    projects: &'a [ProjectHead],
) -> Result<&'a ProjectHead, RouteFailure> {
    let valid = claim_valid_projects(repository, projects);
    match valid.as_slice() {
        [] => Err(RouteFailure::ProjectMissing),
        [only] => Ok(*only),
        _ => Err(RouteFailure::ProjectAmbiguous),
    }
}

/// Require the unique claim-valid project to still be the stored coordinate
/// whose one valid `buzz-channel` UUID equals `channel_id`.
///
/// A replacement project or a changed destination is stale: the side effect
/// must not reroute.
pub fn require_exact_stored_project_channel<'a>(
    repository: &RepositoryHead,
    projects: &'a [ProjectHead],
    stored_project_coordinate: &str,
    channel_id: Uuid,
) -> Result<&'a ProjectHead, RouteFailure> {
    let project = authorize_unique_project_route(repository, projects)?;
    if project.coordinate != stored_project_coordinate {
        return Err(RouteFailure::RouteStale);
    }
    match project
        .buzz_channel
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok())
    {
        Some(project_channel) if project_channel == channel_id => Ok(project),
        _ => Err(RouteFailure::RouteStale),
    }
}

fn unique_coordinates<F>(heads: &[RepositoryHead], pred: F) -> Vec<&str>
where
    F: Fn(&RepositoryHead) -> bool,
{
    let mut seen = BTreeSet::new();
    let mut coords = Vec::new();
    for head in heads {
        if pred(head) && seen.insert(head.coordinate.as_str()) {
            coords.push(head.coordinate.as_str());
        }
    }
    coords
}

fn tags_named<'a>(event: &'a nostr::Event, name: &'a str) -> impl Iterator<Item = &'a [String]> {
    event.tags.iter().filter_map(move |tag| {
        let parts = tag.as_slice();
        if parts.first().map(String::as_str) == Some(name) {
            Some(parts)
        } else {
            None
        }
    })
}

fn first_tag_value<'a>(event: &'a nostr::Event, name: &str) -> Option<&'a str> {
    event.tags.iter().find_map(|tag| {
        let parts = tag.as_slice();
        if parts.first().map(String::as_str) == Some(name) {
            parts.get(1).map(String::as_str)
        } else {
            None
        }
    })
}

fn all_tag_values<'a>(event: &'a nostr::Event, name: &str) -> Vec<&'a str> {
    let mut values = Vec::new();
    for tag in event.tags.iter() {
        let parts = tag.as_slice();
        if parts.first().map(String::as_str) == Some(name) {
            values.extend(parts.iter().skip(1).map(String::as_str));
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn owner() -> String {
        "ab".repeat(32)
    }
    fn other() -> String {
        "cd".repeat(32)
    }
    fn coord(owner: &str, d: &str) -> String {
        format!("30617:{owner}:{d}")
    }
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
        aliases.insert("agentic-os-plan".into(), coord(&other(), "other-repo"));
        let (resolved, tier) =
            resolve_repository_identity("agentic-os-plan", &aliases, &heads).expect("unique d tag");
        assert_eq!(resolved, coord(&owner, "agentic-os-plan"));
        assert_eq!(tier, IdentityTier::DTag);
    }

    #[test]
    fn d_tag_ambiguous_does_not_consult_weaker_tiers() {
        let heads = vec![
            repo("agentic-os-plan", &owner()),
            repo("agentic-os-plan", &other()),
        ];
        let err =
            resolve_repository_identity("agentic-os-plan", &BTreeMap::new(), &heads).unwrap_err();
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
        assert_eq!(
            clone_basename("https://github.com/acme/agentic-os-plan.git/"),
            None
        );
        assert_eq!(clone_basename("https://github.com/"), None);
    }

    #[test]
    fn alias_unique_coordinate_does_not_grant_authority() {
        let target = coord(&owner(), "harness-service");
        let heads = vec![repo("harness-service", &owner())];
        let mut aliases = BTreeMap::new();
        aliases.insert("hs".into(), target.clone());
        let (resolved, tier) = resolve_repository_identity("hs", &aliases, &heads).expect("alias");
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
        let (clone_coord, clone_tier) =
            resolve_repository_identity("agentic-os-plan", &BTreeMap::new(), &heads)
                .expect("clone tier resolves");
        assert_eq!(clone_coord, coord(&owner(), "canonical-d"));
        assert_eq!(clone_tier, IdentityTier::CloneBasename);
        let (_, display_tier) =
            resolve_repository_identity("Plan Display", &BTreeMap::new(), &heads)
                .expect("display tier resolves");
        assert_eq!(display_tier, IdentityTier::DisplayName);
    }

    #[test]
    fn duplicate_observations_of_one_coordinate_are_not_ambiguous() {
        let same = repo("agentic-os-plan", &owner());
        let (resolved, tier) =
            resolve_repository_identity("agentic-os-plan", &BTreeMap::new(), &[same.clone(), same])
                .expect("one distinct coordinate");
        assert_eq!(resolved, coord(&owner(), "agentic-os-plan"));
        assert_eq!(tier, IdentityTier::DTag);
    }

    #[test]
    fn unicode_is_not_normalized_between_tiers() {
        let mut head = repo("canonical", &owner());
        head.name = Some("Caf\u{00e9}".into());
        assert_eq!(
            resolve_repository_identity("Cafe\u{0301}", &BTreeMap::new(), &[head]).unwrap_err(),
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
    fn stored_project_mismatch_or_channel_change_is_stale() {
        let repository = repo("agentic-os-plan", &owner());
        let dest = Uuid::parse_str("9a1657ac-f7aa-5db0-b632-d8bbeb6dfb50").expect("uuid");
        let project = ProjectHead {
            coordinate: format!("30621:{}:gigo-harness", owner()),
            signer_hex: owner(),
            member_coordinates: vec![repository.coordinate.clone()],
            buzz_channel: Some(dest.to_string()),
            listed: true,
        };
        require_exact_stored_project_channel(
            &repository,
            std::slice::from_ref(&project),
            &project.coordinate,
            dest,
        )
        .expect("exact stored route");

        assert_eq!(
            require_exact_stored_project_channel(
                &repository,
                std::slice::from_ref(&project),
                &format!("30621:{}:replacement", owner()),
                dest,
            )
            .unwrap_err(),
            RouteFailure::RouteStale
        );

        let other_channel = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");
        assert_eq!(
            require_exact_stored_project_channel(
                &repository,
                std::slice::from_ref(&project),
                &format!("30621:{}:gigo-harness", owner()),
                other_channel,
            )
            .unwrap_err(),
            RouteFailure::RouteStale
        );
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
                nostr::Tag::parse(["maintainers", maintainer_a.as_str(), maintainer_b.as_str()])
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
        assert!(
            parsed.buzz_channel.is_none(),
            "duplicate channel is invalid"
        );

        let unlisted = nostr::EventBuilder::new(nostr::Kind::Custom(30621), "")
            .tags(vec![
                nostr::Tag::parse(["d", "hidden"]).unwrap(),
                nostr::Tag::parse(["buzz-visibility", "unlisted"]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();
        assert!(!project_head_from_event(&unlisted).unwrap().listed);
    }

    #[test]
    fn project_event_ignores_relay_hint_that_looks_like_a_member_coordinate() {
        let keys = nostr::Keys::generate();
        let actual_member = coord(&owner(), "actual-member");
        let relay_hint = coord(&other(), "hint-only");
        let event = nostr::EventBuilder::new(nostr::Kind::Custom(30621), "")
            .tags(vec![
                nostr::Tag::parse(["d", "gigo-harness"]).unwrap(),
                nostr::Tag::parse(["a", actual_member.as_str(), relay_hint.as_str()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();

        let parsed = project_head_from_event(&event).expect("project head");
        assert_eq!(parsed.member_coordinates, vec![actual_member]);
    }

    #[test]
    fn distinct_named_heads_resolve_uniquely() {
        let owner = owner();
        let heads = vec![
            repo("agentic-os-plan", &owner),
            repo("harness-service", &owner),
        ];
        let (plan, plan_tier) =
            resolve_repository_identity("agentic-os-plan", &BTreeMap::new(), &heads)
                .expect("agentic-os-plan unique");
        let (harness, harness_tier) =
            resolve_repository_identity("harness-service", &BTreeMap::new(), &heads)
                .expect("harness-service unique");
        assert_eq!(plan, coord(&owner, "agentic-os-plan"));
        assert_eq!(plan_tier, IdentityTier::DTag);
        assert_eq!(harness, coord(&owner, "harness-service"));
        assert_eq!(harness_tier, IdentityTier::DTag);
    }
}
