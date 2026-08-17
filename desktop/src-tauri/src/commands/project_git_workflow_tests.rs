use super::{
    align_unborn_head_branch, build_merged_status_event, build_pull_request_status_event,
    classify_pull_request_route, complete_pull_request_merge, normalize_commit,
    project_repo_merge_result, same_repository, validate_merge_status_metadata,
    ProjectRepoMergeGitResult, PullRequestRepoRoute,
};
use crate::commands::project_git_exec::{build_test_git_auth_config, run_git};
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use nostr::{Event, JsonUtil, Keys, Timestamp};

fn route_error(target: &str, source: &str) -> serde_json::Value {
    match classify_pull_request_route(target, source) {
        Ok(PullRequestRepoRoute::Buzz) => panic!("selected Buzz route"),
        Ok(PullRequestRepoRoute::GitHub { .. }) => panic!("selected GitHub route"),
        Err(error) => serde_json::to_value(error).expect("serialize error"),
    }
}

#[test]
fn empty_clone_uses_requested_default_branch() {
    let auth = build_test_git_auth_config().expect("build test git config");
    let repo = tempfile::tempdir().expect("create repository");
    run_git(&["init"], Some(repo.path()), &auth).expect("initialize repository");

    align_unborn_head_branch(repo.path(), Some("main"), &auth).expect("align unborn HEAD");

    assert_eq!(
        std::fs::read_to_string(repo.path().join(".git/HEAD"))
            .expect("read HEAD")
            .trim(),
        "ref: refs/heads/main"
    );
}

#[test]
fn normalize_commit_accepts_sha1_and_sha256_hex() {
    assert_eq!(normalize_commit(&"A".repeat(40)), Some("a".repeat(40)));
    assert_eq!(normalize_commit(&"B".repeat(64)), Some("b".repeat(64)));
}

#[test]
fn normalize_commit_rejects_invalid_values() {
    assert_eq!(normalize_commit("abc"), None);
    assert_eq!(normalize_commit(&"z".repeat(40)), None);
}

#[test]
fn repository_comparison_normalizes_git_suffix_and_trailing_slash() {
    assert!(same_repository(
        "https://relay.example/git/owner/repo.git",
        "https://relay.example/git/owner/repo/"
    ));
    assert!(!same_repository(
        "https://relay.example/git/owner/repo",
        "https://relay.example/git/fork/repo"
    ));
}

#[test]
fn strict_github_clone_urls_select_github_route() {
    match classify_pull_request_route(
        "https://github.com/block/buzz",
        "https://github.com/fork/buzz.git",
    )
    .expect("github route")
    {
        PullRequestRepoRoute::GitHub { .. } => {}
        PullRequestRepoRoute::Buzz => panic!("selected Buzz route"),
    }
}

#[test]
fn buzz_workspace_clone_urls_select_buzz_route() {
    match classify_pull_request_route(
            "https://relay.example/git/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/buzz",
            "https://relay.example/git/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/buzz",
        )
        .expect("buzz route")
        {
            PullRequestRepoRoute::Buzz => {}
            PullRequestRepoRoute::GitHub { .. } => panic!("selected GitHub route"),
        }
}

#[test]
fn mixed_github_and_buzz_urls_fail_structurally() {
    for (target, source) in [
            (
                "https://github.com/block/buzz",
                "https://relay.example/git/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/buzz",
            ),
            (
                "https://relay.example/git/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/buzz",
                "https://github.com/block/buzz",
            ),
        ] {
            let value = route_error(target, source);
            assert_eq!(value["code"], "github_merge_failed");
            assert_eq!(
                value["message"],
                "Source and target repositories must use the same supported host."
            );
        }
}

#[test]
fn malformed_github_urls_fail_instead_of_selecting_buzz() {
    for raw in [
        "https://github.com/block/buzz?tab=readme",
        "https://github.com/block/buzz#readme",
        "https://user@github.com/block/buzz",
        "https://github.com:443/block/buzz",
        "https://github.com/block/buzz/extra",
        "https://github.com:bad/acme/repo",
    ] {
        let value = route_error(raw, raw);
        assert_eq!(value["code"], "github_merge_failed");
    }
}

#[test]
fn github_errors_do_not_build_a_merged_status() {
    for (code, message) in [
        ("github_auth_required", "GitHub authentication is required."),
        ("github_pr_blocked", "GitHub blocked this pull request."),
        ("github_merge_failed", "GitHub did not verify the merge."),
    ] {
        let mut status_builder_called = false;
        let result = complete_pull_request_merge(
            Err(ProjectPullRequestMergeError::new(code, message)),
            |_| {
                status_builder_called = true;
                Ok("unexpected signed status".to_string())
            },
        );
        let error = result
            .err()
            .expect("GitHub error must stop before status construction");
        assert_eq!(serde_json::to_value(error).unwrap()["code"], code);
        assert!(!status_builder_called);
    }
}

#[test]
fn verified_github_merge_builds_status_with_verified_commit() {
    let verified_commit = "a".repeat(40);
    let (git_result, status_event) = complete_pull_request_merge(
        Ok(ProjectRepoMergeGitResult {
            message: "Merged GitHub pull request #42.".to_string(),
            merge_commit: verified_commit.clone(),
        }),
        |git_result| {
            assert_eq!(git_result.merge_commit, verified_commit);
            Ok(format!("signed:{}", git_result.merge_commit))
        },
    )
    .expect("verified merge builds status");
    assert_eq!(git_result.merge_commit, verified_commit);
    assert_eq!(status_event, format!("signed:{verified_commit}"));
}

#[test]
fn publication_failure_keeps_the_signed_merged_status() {
    let result = project_repo_merge_result(
        ProjectRepoMergeGitResult {
            message: "Merged GitHub pull request #42.".to_string(),
            merge_commit: "a".repeat(40),
        },
        "signed merged status".to_string(),
        Some("relay unavailable".to_string()),
    );
    assert_eq!(result.status_event, "signed merged status");
    assert_eq!(
        result.status_publication_error.as_deref(),
        Some("relay unavailable")
    );
}

#[test]
fn merged_status_is_signed_by_repository_owner() {
    let keys = Keys::generate();
    let owner = keys.public_key().to_hex();
    let pull_request_id = "d".repeat(64);
    let pull_request_author = "b".repeat(64);
    let merge_commit = "e".repeat(40);
    let repo_address = format!("30617:{owner}:buzz");
    let before = Timestamp::now().as_secs();
    let event = Event::from_json(
        build_merged_status_event(
            &keys,
            &repo_address,
            &pull_request_id,
            &pull_request_author,
            &merge_commit,
            123,
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(event.pubkey, keys.public_key());
    assert_eq!(event.kind.as_u16(), 1631);
    assert!(event.created_at.as_secs() >= before);
    assert!(event
        .tags
        .iter()
        .any(|tag| tag.as_slice() == ["merge-commit", merge_commit.as_str()]));
    assert!(event.verify().is_ok());
}

#[test]
fn merged_status_preserves_a_newer_requested_timestamp() {
    let keys = Keys::generate();
    let owner = keys.public_key().to_hex();
    let requested = Timestamp::now().as_secs() + 10;
    let event = Event::from_json(
        build_merged_status_event(
            &keys,
            &format!("30617:{owner}:buzz"),
            &"d".repeat(64),
            &"b".repeat(64),
            &"e".repeat(40),
            requested,
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(event.created_at.as_secs(), requested);
}

#[test]
fn merge_status_metadata_is_rejected_before_git_work() {
    let owner = "a".repeat(64);
    assert!(validate_merge_status_metadata(
        &format!("30617:{}:buzz", "b".repeat(64)),
        &owner,
        &"d".repeat(64),
        &"e".repeat(64),
    )
    .is_err());
    assert!(validate_merge_status_metadata(
        &format!("30617:{owner}:buzz"),
        &owner,
        "not-an-event-id",
        &"e".repeat(64),
    )
    .is_err());
    assert!(validate_merge_status_metadata(
        &format!("30617:{owner}:buzz"),
        &owner,
        &"d".repeat(64),
        "not-an-author",
    )
    .is_err());
}

#[test]
fn lifecycle_status_is_signed_by_repository_owner() {
    let keys = Keys::generate();
    let owner = keys.public_key().to_hex();
    let author = "b".repeat(64);
    let event = Event::from_json(
        build_pull_request_status_event(
            &keys,
            &format!("30617:{owner}:buzz"),
            &"d".repeat(64),
            &author,
            "closed",
            Timestamp::now().as_secs(),
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(event.pubkey, keys.public_key());
    assert_eq!(event.kind.as_u16(), 1632);
    assert!(event
        .tags
        .iter()
        .any(|tag| tag.as_slice() == ["p", author.as_str()]));
    assert!(event.verify().is_ok());
}

#[test]
fn lifecycle_status_rejects_merged_alias() {
    let keys = Keys::generate();
    let owner = keys.public_key().to_hex();

    assert!(build_pull_request_status_event(
        &keys,
        &format!("30617:{owner}:buzz"),
        &"d".repeat(64),
        &"b".repeat(64),
        "merged",
        Timestamp::now().as_secs(),
    )
    .is_err());
}
