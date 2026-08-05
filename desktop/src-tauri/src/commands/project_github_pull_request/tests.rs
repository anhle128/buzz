use super::{
    decide_open_pull, encode_path_segment, find_or_create_pull_request_with,
    join_gh_readers_after_cleanup, merge_github_pull_request_with, redact_diagnostic, select_pull,
    GhRunner, GitHubCheck, GitHubMergeInput, GitHubOid, GitHubPullView, GitHubRepoRef, GitHubRule,
    PullDecision, PullSelection, GH_DIAGNOSTIC_LIMIT, GH_STREAM_LIMIT,
};
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const LIFECYCLE_FAKE_TIMEOUT: Duration = Duration::from_secs(15);
const PID_PUBLICATION_TIMEOUT: Duration = Duration::from_secs(3);

fn merge_input() -> GitHubMergeInput {
    GitHubMergeInput {
        target: GitHubRepoRef::parse("https://github.com/acme/buzz").expect("target repo"),
        source: GitHubRepoRef::parse("https://github.com/fork/buzz").expect("source repo"),
        target_branch: "main".to_string(),
        source_branch: "feature/pr".to_string(),
        expected_commit: "abcdef0123456789".to_string(),
        title: "Merge feature".to_string(),
        body: "Reviewed body".to_string(),
    }
}

fn pull_view(value: serde_json::Value) -> GitHubPullView {
    serde_json::from_value(value).expect("deserialize GitHub pull view fixture")
}

fn ready_pull_view() -> GitHubPullView {
    pull_view(serde_json::json!({
        "number": 42,
        "url": "https://github.com/acme/buzz/pull/42",
        "state": "OPEN",
        "headRefOid": "abcdef0123456789",
        "isDraft": false,
        "mergeable": "MERGEABLE",
        "mergeStateStatus": "CLEAN",
        "reviewDecision": "APPROVED",
        "autoMergeRequest": null,
        "mergeCommit": null,
        "mergedAt": null
    }))
}

fn passing_required_check(name: &str) -> GitHubCheck {
    GitHubCheck {
        name: name.to_string(),
        state: "SUCCESS".to_string(),
        bucket: "pass".to_string(),
        link: "https://github.com/acme/buzz/actions".to_string(),
    }
}

fn assert_ready(view: &GitHubPullView, checks: &[GitHubCheck], rules: Result<&[GitHubRule], &str>) {
    assert!(matches!(
        decide_open_pull(view, checks, rules).expect("decide pull"),
        PullDecision::Ready
    ));
}

fn assert_blocked_contains(
    view: &GitHubPullView,
    checks: &[GitHubCheck],
    rules: Result<&[GitHubRule], &str>,
    expected: &str,
) {
    let PullDecision::Blocked(reasons) =
        decide_open_pull(view, checks, rules).expect("decide pull")
    else {
        panic!("expected blocked decision");
    };
    assert!(
        reasons.iter().any(|reason| reason.contains(expected)),
        "missing {expected:?} in {reasons:?}"
    );
}

fn terminal_merge_commit(view: &GitHubPullView) -> Option<String> {
    if view.state == "MERGED" && view.merged_at.is_some() {
        view.merge_commit.clone().map(|commit| commit.oid)
    } else {
        None
    }
}

#[test]
fn github_pull_gate_state_table() {
    let pass = [passing_required_check("ci")];
    let empty_rules: [GitHubRule; 0] = [];
    let merge_queue_rules = [GitHubRule {
        kind: "merge_queue".to_string(),
    }];

    let mut head_changed = ready_pull_view();
    head_changed.head_ref_oid = "fedcba9876543210".to_string();
    assert_ne!(head_changed.head_ref_oid, merge_input().expected_commit);

    let mut merged = ready_pull_view();
    merged.state = "MERGED".to_string();
    merged.merged_at = Some("2026-08-05T00:00:00Z".to_string());
    merged.merge_commit = Some(GitHubOid {
        oid: "0011223344556677".to_string(),
    });
    assert_eq!(
        terminal_merge_commit(&merged).expect("verified merge commit"),
        "0011223344556677"
    );

    let mut unverifiable_merged = merged.clone();
    unverifiable_merged.merge_commit = None;
    assert!(terminal_merge_commit(&unverifiable_merged).is_none());
    unverifiable_merged.merge_commit = merged.merge_commit.clone();
    unverifiable_merged.merged_at = None;
    assert!(terminal_merge_commit(&unverifiable_merged).is_none());

    let mut closed = ready_pull_view();
    closed.state = "CLOSED".to_string();
    assert_blocked_contains(&closed, &pass, Ok(&empty_rules), "closed without merging");

    let mut draft = ready_pull_view();
    draft.is_draft = true;
    assert_blocked_contains(&draft, &pass, Ok(&empty_rules), "draft");

    let mut conflicting = ready_pull_view();
    conflicting.mergeable = "CONFLICTING".to_string();
    assert_blocked_contains(&conflicting, &pass, Ok(&empty_rules), "CONFLICTING");

    let mut unknown_mergeability = ready_pull_view();
    unknown_mergeability.mergeable = "UNKNOWN".to_string();
    assert_blocked_contains(&unknown_mergeability, &pass, Ok(&empty_rules), "UNKNOWN");

    let mut review_required = ready_pull_view();
    review_required.review_decision = "REVIEW_REQUIRED".to_string();
    assert_blocked_contains(&review_required, &pass, Ok(&empty_rules), "REVIEW_REQUIRED");

    let mut changes_requested = ready_pull_view();
    changes_requested.review_decision = "CHANGES_REQUESTED".to_string();
    assert_blocked_contains(
        &changes_requested,
        &pass,
        Ok(&empty_rules),
        "CHANGES_REQUESTED",
    );

    let failing_check = [GitHubCheck {
        name: "lint".to_string(),
        state: "FAILURE".to_string(),
        bucket: "fail".to_string(),
        link: "https://github.com/acme/buzz/actions/runs/1".to_string(),
    }];
    assert_blocked_contains(&ready_pull_view(), &failing_check, Ok(&empty_rules), "lint");
    assert_blocked_contains(
        &ready_pull_view(),
        &failing_check,
        Ok(&empty_rules),
        "FAILURE",
    );

    let mut auto_merge = ready_pull_view();
    auto_merge.auto_merge_request = Some(serde_json::json!({ "enabledBy": "octocat" }));
    assert_blocked_contains(
        &auto_merge,
        &pass,
        Ok(&empty_rules),
        "Auto-merge is already enabled",
    );

    assert_blocked_contains(
        &ready_pull_view(),
        &pass,
        Ok(&merge_queue_rules),
        "merge queue",
    );
    assert_blocked_contains(
        &ready_pull_view(),
        &pass,
        Err("rules endpoint failed"),
        "rules endpoint failed",
    );

    assert_ready(&ready_pull_view(), &pass, Ok(&empty_rules));

    let mut unstable = ready_pull_view();
    unstable.merge_state_status = "UNSTABLE".to_string();
    assert_ready(&unstable, &pass, Ok(&empty_rules));
    assert_blocked_contains(&unstable, &failing_check, Ok(&empty_rules), "lint");

    let mut blocked_state = ready_pull_view();
    blocked_state.merge_state_status = "BLOCKED".to_string();
    assert_blocked_contains(&blocked_state, &pass, Ok(&empty_rules), "BLOCKED");
}

fn pull(value: serde_json::Value) -> super::GitHubPullSummary {
    serde_json::from_value(value).expect("deserialize GitHub pull fixture")
}

fn exact_pull(state: &str, merged_at: Option<&str>) -> super::GitHubPullSummary {
    pull(serde_json::json!({
        "number": 42,
        "html_url": "https://github.com/acme/buzz/pull/42",
        "state": state,
        "merged_at": merged_at,
        "head": {
            "sha": "abcdef0123456789",
            "ref": "feature/pr",
            "repo": { "full_name": "fork/buzz" }
        },
        "base": {
            "ref": "main",
            "repo": { "full_name": "acme/buzz" }
        }
    }))
}

fn assert_error_code(error: ProjectPullRequestMergeError, code: &str, url: &str) {
    let value = error_value(error);
    assert_eq!(value["code"], code);
    assert_eq!(value["recovery"]["action"], "open_url");
    assert_eq!(value["recovery"]["url"], url);
}

#[test]
fn github_branch_names_are_encoded_as_one_path_segment() {
    assert_eq!(encode_path_segment("main"), "main");
    assert_eq!(encode_path_segment("release/2026.08"), "release%2F2026.08");
    assert_eq!(encode_path_segment("feature@review"), "feature%40review");
}

#[test]
fn select_pull_reuses_one_exact_open_pull() {
    let selection = select_pull(vec![exact_pull("open", None)], &merge_input()).expect("reuse");
    assert!(matches!(selection, PullSelection::Reuse(pull) if pull.number == 42));
}

#[test]
fn select_pull_rejects_ambiguous_open_pulls() {
    let mut second = exact_pull("open", None);
    second.number = 43;
    assert_error_code(
        select_pull(vec![exact_pull("open", None), second], &merge_input()).expect_err("ambiguous"),
        "github_pr_ambiguous",
        "https://github.com/acme/buzz/pulls",
    );
}

#[test]
fn select_pull_reuses_one_merged_pull_at_reviewed_head() {
    let selection = select_pull(
        vec![exact_pull("closed", Some("2026-08-05T00:00:00Z"))],
        &merge_input(),
    )
    .expect("reuse merged");
    assert!(matches!(selection, PullSelection::Reuse(pull) if pull.number == 42));
}

#[test]
fn select_pull_rejects_ambiguous_merged_pulls() {
    let mut second = exact_pull("closed", Some("2026-08-05T00:00:00Z"));
    second.number = 43;
    assert_error_code(
        select_pull(
            vec![exact_pull("closed", Some("2026-08-05T00:00:00Z")), second],
            &merge_input(),
        )
        .expect_err("ambiguous"),
        "github_pr_ambiguous",
        "https://github.com/acme/buzz/pulls",
    );
}

#[test]
fn select_pull_blocks_closed_unmerged_pull() {
    assert_error_code(
        select_pull(vec![exact_pull("closed", None)], &merge_input()).expect_err("blocked"),
        "github_pr_blocked",
        "https://github.com/acme/buzz/pull/42",
    );
}

#[test]
fn select_pull_ignores_near_matches() {
    let input = merge_input();
    let variants = [
        ("other/buzz", "feature/pr", "main", "acme/buzz"),
        ("fork/buzz", "FEATURE/pr", "main", "acme/buzz"),
        ("fork/buzz", "feature/pr", "Main", "acme/buzz"),
        ("fork/buzz", "feature/pr", "main", "other/buzz"),
    ];
    let pulls = variants.map(|(source_repo, source_branch, target_branch, target_repo)| {
        pull(serde_json::json!({
            "number": 42,
            "html_url": "https://github.com/acme/buzz/pull/42",
            "state": "open",
            "merged_at": null,
            "head": { "sha": "abcdef0123456789", "ref": source_branch, "repo": { "full_name": source_repo } },
            "base": { "ref": target_branch, "repo": { "full_name": target_repo } }
        }))
    });
    assert!(matches!(
        select_pull(Vec::from(pulls), &input),
        Ok(PullSelection::Create)
    ));
}

#[test]
fn select_pull_creates_when_no_exact_pull_exists() {
    assert!(matches!(
        select_pull(Vec::new(), &merge_input()),
        Ok(PullSelection::Create)
    ));
}

#[cfg(unix)]
#[test]
fn github_pull_creation_uses_exact_api_shape_and_json_input() {
    let (dir, binary) = fake_lifecycle_gh("[[]]", &pull_response(), 0, "");
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };
    let input = merge_input();

    let url = find_or_create_pull_request_with(&runner, &input).expect("create and re-find pull");

    assert_eq!(url, "https://github.com/acme/buzz/pull/42");
    let calls = fake_gh_calls(&dir);
    let api_calls = calls
        .iter()
        .filter(|args| args.first().is_some_and(|arg| arg == "api"))
        .collect::<Vec<_>>();
    assert_eq!(api_calls.len(), 3);
    assert!(api_calls.iter().all(|args| {
        args.windows(2)
            .any(|pair| pair[0] == "--hostname" && pair[1] == "github.com")
    }));
    let list_args = [
        "api",
        "--method",
        "GET",
        "--hostname",
        "github.com",
        "--paginate",
        "--slurp",
        "repos/acme/buzz/pulls",
        "-f",
        "state=all",
        "-f",
        "head=fork:feature/pr",
        "-f",
        "base=main",
        "-f",
        "per_page=100",
    ]
    .map(str::to_string)
    .to_vec();
    assert_eq!(api_calls[0], &list_args);
    assert_eq!(api_calls[2], &list_args);
    let mut post_args = api_calls[1].clone();
    assert!(!std::path::Path::new(&post_args[7]).exists());
    post_args[7] = "<temp-json>".to_string();
    assert_eq!(
        post_args,
        [
            "api",
            "--method",
            "POST",
            "--hostname",
            "github.com",
            "repos/acme/buzz/pulls",
            "--input",
            "<temp-json>",
        ]
        .map(str::to_string)
        .to_vec()
    );
    assert!(api_calls[1]
        .iter()
        .all(|argument| !argument.contains(&input.title) && !argument.contains(&input.body)));

    let body: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("post.json")).expect("read POST JSON"),
    )
    .expect("parse POST JSON");
    assert_eq!(body["title"], input.title);
    assert_eq!(body["body"], input.body);
    assert_eq!(body["head"], "fork:feature/pr");
    assert_eq!(body["base"], "main");
    assert_eq!(body["head_repo"], "fork/buzz");
}

#[cfg(unix)]
#[test]
fn github_pull_creation_recovers_lost_post_response_without_retrying_post() {
    let (dir, binary) = fake_lifecycle_gh("[[]]", &pull_response(), 1, "");
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let url = find_or_create_pull_request_with(&runner, &merge_input())
        .expect("relookup should find pull after failed POST");

    assert_eq!(url, "https://github.com/acme/buzz/pull/42");
    let calls = fake_gh_calls(&dir);
    assert_eq!(
        calls
            .iter()
            .filter(|args| {
                args.windows(2)
                    .any(|pair| pair[0] == "--method" && pair[1] == "POST")
            })
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn github_pull_creation_reports_missing_pull_after_successful_post_without_retrying() {
    let (dir, binary) = fake_lifecycle_gh("[[]]", "[[]]", 0, "");
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value = error_value(
        find_or_create_pull_request_with(&runner, &merge_input())
            .expect_err("missing pull after successful POST"),
    );

    assert_eq!(value["code"], "github_merge_failed");
    assert_eq!(
        value["message"],
        "GitHub created the pull request but it could not be found. Refresh and retry."
    );
    let calls = fake_gh_calls(&dir);
    assert_eq!(
        calls
            .iter()
            .filter(|args| {
                args.windows(2)
                    .any(|pair| pair[0] == "--method" && pair[1] == "POST")
            })
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn github_pull_creation_preserves_failed_post_error_when_relookup_is_empty() {
    let (dir, binary) = fake_lifecycle_gh("[[]]", "[[]]", 1, "connection lost\n");
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value = error_value(
        find_or_create_pull_request_with(&runner, &merge_input())
            .expect_err("failed POST should take precedence"),
    );

    assert_eq!(value["code"], "github_merge_failed");
    assert_eq!(value["message"], "connection lost\n");
    let calls = fake_gh_calls(&dir);
    assert_eq!(
        calls
            .iter()
            .filter(|args| {
                args.windows(2)
                    .any(|pair| pair[0] == "--method" && pair[1] == "POST")
            })
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn github_pull_creation_reuses_existing_open_pull_without_posting() {
    let (dir, binary) = fake_lifecycle_gh(&pull_response(), &pull_response(), 0, "");
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let url =
        find_or_create_pull_request_with(&runner, &merge_input()).expect("reuse existing pull");

    assert_eq!(url, "https://github.com/acme/buzz/pull/42");
    let calls = fake_gh_calls(&dir);
    assert!(calls.iter().all(|args| {
        !args
            .windows(2)
            .any(|pair| pair[0] == "--method" && pair[1] == "POST")
    }));
}

#[cfg(unix)]
#[test]
fn github_pull_creation_omits_head_repo_for_same_repository() {
    let same_repo_pull = pull_response().replace("fork/buzz", "acme/buzz");
    let (dir, binary) = fake_lifecycle_gh("[[]]", &same_repo_pull, 0, "");
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };
    let mut input = merge_input();
    input.source = GitHubRepoRef::parse("https://github.com/acme/buzz").expect("same source repo");

    let _ = find_or_create_pull_request_with(&runner, &input).expect("create same-repo pull");
    let body: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("post.json")).expect("read POST JSON"),
    )
    .expect("parse POST JSON");
    assert!(body.get("head_repo").is_none());
}

#[cfg(unix)]
#[test]
fn github_merge_stops_before_gates_when_head_changed() {
    let (dir, binary) = fake_merge_gh(MergeFake {
        initial_view: view_response("OPEN", "fedcba9876543210", None, None),
        ..MergeFake::ready()
    });
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value = error_value(
        merge_github_pull_request_with(&runner, merge_input()).expect_err("head changed"),
    );

    assert_eq!(value["code"], "github_branch_changed");
    let calls = fake_gh_calls(&dir);
    assert_no_gate_or_merge_put_calls(&calls);
}

#[cfg(unix)]
#[test]
fn github_merge_recovers_already_merged_without_querying_gates() {
    let (dir, binary) = fake_merge_gh(MergeFake {
        initial_view: view_response(
            "MERGED",
            "abcdef0123456789",
            Some("2026-08-05T00:00:00Z"),
            Some("0011223344556677"),
        ),
        ..MergeFake::ready()
    });
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let outcome = merge_github_pull_request_with(&runner, merge_input()).expect("already merged");

    assert_eq!(outcome.merge_commit, "0011223344556677");
    assert_eq!(
        outcome.message,
        "GitHub pull request #42 was already merged."
    );
    let calls = fake_gh_calls(&dir);
    assert_no_gate_or_merge_put_calls(&calls);
}

#[cfg(unix)]
#[test]
fn github_merge_stops_before_gates_when_closed_unmerged_after_lookup() {
    let (dir, binary) = fake_merge_gh(MergeFake {
        initial_view: view_response("CLOSED", "abcdef0123456789", None, None),
        ..MergeFake::ready()
    });
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value =
        error_value(merge_github_pull_request_with(&runner, merge_input()).expect_err("closed"));

    assert_eq!(value["code"], "github_pr_blocked");
    let calls = fake_gh_calls(&dir);
    assert_no_gate_or_merge_put_calls(&calls);
}

#[cfg(unix)]
#[test]
fn github_merge_skips_merge_for_each_blocked_open_gate() {
    let mut cases = Vec::new();
    let mut draft = ready_view_value();
    draft["isDraft"] = serde_json::json!(true);
    cases.push(("draft", draft, MergeFake::ready()));

    let mut conflicting = ready_view_value();
    conflicting["mergeable"] = serde_json::json!("CONFLICTING");
    cases.push(("conflicting", conflicting, MergeFake::ready()));

    let mut unknown_mergeable = ready_view_value();
    unknown_mergeable["mergeable"] = serde_json::json!("UNKNOWN");
    cases.push(("unknown mergeable", unknown_mergeable, MergeFake::ready()));

    let mut review_required = ready_view_value();
    review_required["reviewDecision"] = serde_json::json!("REVIEW_REQUIRED");
    cases.push(("review required", review_required, MergeFake::ready()));

    let mut changes_requested = ready_view_value();
    changes_requested["reviewDecision"] = serde_json::json!("CHANGES_REQUESTED");
    cases.push(("changes requested", changes_requested, MergeFake::ready()));

    let mut auto_merge = ready_view_value();
    auto_merge["autoMergeRequest"] = serde_json::json!({ "enabledBy": "octocat" });
    cases.push(("auto merge", auto_merge, MergeFake::ready()));

    let mut blocked_state = ready_view_value();
    blocked_state["mergeStateStatus"] = serde_json::json!("BLOCKED");
    cases.push(("blocked state", blocked_state, MergeFake::ready()));

    let mut failing_check = MergeFake::ready();
    failing_check.checks = r#"[{"name":"lint","state":"FAILURE","bucket":"fail","link":"https://github.com/acme/buzz/actions/runs/1"}]"#.to_string();
    cases.push(("failing check", ready_view_value(), failing_check));

    let mut queue = MergeFake::ready();
    queue.rules = r#"[[{"type":"merge_queue"}]]"#.to_string();
    cases.push(("merge queue", ready_view_value(), queue));

    let mut unknown_rules = MergeFake::ready();
    unknown_rules.rules_status = 1;
    unknown_rules.rules_stderr = "rules endpoint failed".to_string();
    cases.push(("rules unavailable", ready_view_value(), unknown_rules));

    for (name, view, mut fake) in cases {
        fake.initial_view = view.to_string();
        let (dir, binary) = fake_merge_gh(fake);
        let runner = GhRunner {
            binary,
            timeout: LIFECYCLE_FAKE_TIMEOUT,
        };

        let error = match merge_github_pull_request_with(&runner, merge_input()) {
            Ok(outcome) => panic!("expected {name} to block, got {outcome:?}"),
            Err(error) => error,
        };
        let value = error_value(error);

        assert_eq!(value["code"], "github_pr_blocked", "{name}");
        let calls = fake_gh_calls(&dir);
        assert_no_merge_put_call(&calls);
    }
}

#[cfg(unix)]
#[test]
fn github_merge_check_command_failure_keeps_pr_url_recovery() {
    let (dir, binary) = fake_merge_gh(MergeFake {
        checks_status: 1,
        checks_stderr: "checks endpoint failed".to_string(),
        ..MergeFake::ready()
    });
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value = error_value(
        merge_github_pull_request_with(&runner, merge_input()).expect_err("checks failure"),
    );

    assert_eq!(value["code"], "github_merge_failed");
    assert_eq!(value["message"], "checks endpoint failed");
    assert_eq!(value["recovery"]["action"], "open_url");
    assert_eq!(
        value["recovery"]["url"],
        "https://github.com/acme/buzz/pull/42"
    );
    assert_no_merge_put_call(&fake_gh_calls(&dir));
}

#[cfg(unix)]
#[test]
fn github_merge_accepts_documented_no_required_checks_message() {
    let (dir, binary) = fake_merge_gh(MergeFake {
        checks: String::new(),
        checks_status: 1,
        checks_stderr: "no required checks reported on the 'main' branch".to_string(),
        ..MergeFake::ready()
    });
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let outcome =
        merge_github_pull_request_with(&runner, merge_input()).expect("no required checks");

    assert_eq!(outcome.merge_commit, "deadbeef12345678");
    assert!(fake_gh_calls(&dir)
        .iter()
        .any(|args| is_merge_put_call(args)));
}

#[cfg(unix)]
#[test]
fn github_merge_uses_exact_put_json_and_verified_merge_commit() {
    let mut input = merge_input();
    input.target_branch = "release/2026.08".to_string();
    let (dir, binary) = fake_merge_gh(MergeFake {
        pulls: pull_response_for_base("release/2026.08"),
        verified_view: view_response(
            "MERGED",
            "abcdef0123456789",
            Some("2026-08-05T00:00:00Z"),
            Some("verifiedcafebabe"),
        ),
        ..MergeFake::ready()
    });
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let outcome = merge_github_pull_request_with(&runner, input).expect("merge");

    assert_eq!(outcome.merge_commit, "verifiedcafebabe");
    let calls = fake_gh_calls(&dir);
    assert!(calls.iter().any(|args| args
        .iter()
        .any(|arg| arg == "repos/acme/buzz/rules/branches/release%2F2026.08")));
    let mut merge_call = calls
        .iter()
        .find(|args| {
            args.iter()
                .any(|arg| arg == "repos/acme/buzz/pulls/42/merge")
        })
        .expect("merge PUT")
        .clone();
    let input_index = merge_call
        .iter()
        .position(|arg| arg == "--input")
        .expect("merge input flag")
        + 1;
    assert!(!std::path::Path::new(&merge_call[input_index]).exists());
    merge_call[input_index] = "<temp-json>".to_string();
    assert_eq!(
        merge_call,
        [
            "api",
            "--method",
            "PUT",
            "--hostname",
            "github.com",
            "repos/acme/buzz/pulls/42/merge",
            "--input",
            "<temp-json>",
        ]
        .map(str::to_string)
        .to_vec()
    );
    let body: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("merge.json")).expect("read merge JSON"),
    )
    .expect("parse merge JSON");
    assert_eq!(
        body,
        serde_json::json!({
            "sha": "abcdef0123456789",
            "merge_method": "merge"
        })
    );
    assert_forbidden_merge_commands_absent(&calls);
}

#[cfg(unix)]
#[test]
fn github_merge_requires_verified_post_query() {
    let (dir, binary) = fake_merge_gh(MergeFake {
        verified_view: view_response("OPEN", "abcdef0123456789", None, None),
        ..MergeFake::ready()
    });
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value = error_value(
        merge_github_pull_request_with(&runner, merge_input()).expect_err("unverified merge"),
    );

    assert_eq!(value["code"], "github_merge_failed");
    assert!(dir.path().join("merge.json").exists());
}

#[cfg(unix)]
#[test]
fn github_merge_maps_stale_put_rejection_to_branch_changed() {
    let (dir, binary) = fake_merge_gh(MergeFake {
        merge_status: 1,
        merge_stderr: "sha does not match pull request head".to_string(),
        ..MergeFake::ready()
    });
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value = error_value(
        merge_github_pull_request_with(&runner, merge_input()).expect_err("stale head"),
    );

    assert_eq!(value["code"], "github_branch_changed");
    assert!(dir.path().join("merge.json").exists());
}

#[cfg(unix)]
#[test]
fn github_merge_auth_error_cannot_return_merge_outcome() {
    let (dir, binary) = fake_merge_gh(MergeFake {
        auth_status: 1,
        ..MergeFake::ready()
    });
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value = error_value(
        merge_github_pull_request_with(&runner, merge_input()).expect_err("auth failure"),
    );

    assert_eq!(value["code"], "github_auth_required");
    assert_eq!(
        fake_gh_calls(&dir),
        [vec![
            "auth".to_string(),
            "status".to_string(),
            "--hostname".to_string(),
            "github.com".to_string(),
        ]]
    );
}

#[test]
fn github_repo_parser_accepts_only_strict_repository_urls() {
    let accepted = [
        ("https://github.com/anhle128/buzz", "anhle128", "buzz"),
        ("https://github.com/anhle128/buzz.git", "anhle128", "buzz"),
        (
            "https://github.com/Ocean-Labs/buzz_desktop",
            "Ocean-Labs",
            "buzz_desktop",
        ),
    ];
    for (raw, owner, repo) in accepted {
        let parsed = GitHubRepoRef::parse(raw).unwrap_or_else(|error| panic!("{raw}: {error}"));
        assert_eq!(parsed.owner, owner, "{raw}");
        assert_eq!(parsed.repo, repo, "{raw}");
    }

    let rejected = [
        "http://github.com/anhle128/buzz",
        "https://user@github.com/anhle128/buzz",
        "https://github.com:443/anhle128/buzz",
        "https://github.com/anhle128/buzz/extra",
        "https://github.com/anhle128/buzz?tab=readme",
        "https://github.com/anhle128/buzz#readme",
        "https://github.com/anhle128",
        "https://github.com/anh_le/buzz",
        "https://github.com/anhle128/buzz%2Freleases",
        "https://github.com/anhle128/buzz\n",
    ];
    for raw in rejected {
        assert!(GitHubRepoRef::parse(raw).is_err(), "accepted {raw:?}");
    }
}

#[test]
fn github_repo_host_detection_preserves_malformed_github_intent() {
    assert!(GitHubRepoRef::is_github_host(
        "https://github.com/anhle128/buzz?tab=readme"
    ));
}

fn error_value(error: ProjectPullRequestMergeError) -> serde_json::Value {
    serde_json::to_value(error).expect("serialize GitHub command error")
}

#[cfg(unix)]
fn fake_gh(script: &str) -> (tempfile::TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("create fake gh directory");
    let path = dir.path().join("gh");
    std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{script}\n")).expect("write fake gh");
    let mut permissions = std::fs::metadata(&path)
        .expect("stat fake gh")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod fake gh");
    (dir, path)
}

#[cfg(unix)]
fn fake_lifecycle_gh(
    first_list: &str,
    second_list: &str,
    post_status: i32,
    post_stderr: &str,
) -> (tempfile::TempDir, PathBuf) {
    let script = format!(
        r#"
root=${{0%/gh}}
printf '%s\t' "$@" >> "$root/calls"
printf '\036' >> "$root/calls"
method=
input=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --method) method=$2; shift 2 ;;
        --input) input=$2; shift 2 ;;
        *) shift ;;
    esac
done
case "$method" in
    GET)
        if [ -e "$root/posted" ]; then
            printf '%s' {second_list}
        else
            printf '%s' {first_list}
        fi
        ;;
    POST)
        : > "$root/posted"
        IFS= read -r body < "$input" || true
        printf '%s' "$body" > "$root/post.json"
        printf '%s' {post_stderr} >&2
        exit {post_status}
        ;;
esac
"#,
        first_list = shell_quote(first_list),
        second_list = shell_quote(second_list),
        post_stderr = shell_quote(post_stderr),
    );
    fake_gh(&script)
}

#[cfg(unix)]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(unix)]
fn fake_gh_calls(dir: &tempfile::TempDir) -> Vec<Vec<String>> {
    std::fs::read_to_string(dir.path().join("calls"))
        .expect("read fake gh calls")
        .split('\x1e')
        .filter(|call| !call.is_empty())
        .map(|call| {
            call.split('\t')
                .filter(|argument| !argument.is_empty())
                .map(str::to_string)
                .collect()
        })
        .collect()
}

fn pull_response() -> String {
    pull_response_for_base("main")
}

fn pull_response_for_base(base: &str) -> String {
    serde_json::json!([[
        {
            "number": 42,
            "html_url": "https://github.com/acme/buzz/pull/42",
            "state": "open",
            "merged_at": null,
            "head": {
                "sha": "abcdef0123456789",
                "ref": "feature/pr",
                "repo": { "full_name": "fork/buzz" }
            },
            "base": {
                "ref": base,
                "repo": { "full_name": "acme/buzz" }
            }
        }
    ]])
    .to_string()
}

#[cfg(unix)]
struct MergeFake {
    initial_view: String,
    verified_view: String,
    checks: String,
    checks_status: i32,
    checks_stderr: String,
    rules: String,
    rules_status: i32,
    rules_stderr: String,
    merge_stdout: String,
    merge_status: i32,
    merge_stderr: String,
    auth_status: i32,
    pulls: String,
}

#[cfg(unix)]
impl MergeFake {
    fn ready() -> Self {
        Self {
            initial_view: view_response("OPEN", "abcdef0123456789", None, None),
            verified_view: view_response(
                "MERGED",
                "abcdef0123456789",
                Some("2026-08-05T00:00:00Z"),
                Some("deadbeef12345678"),
            ),
            checks: r#"[{"name":"ci","state":"SUCCESS","bucket":"pass","link":"https://github.com/acme/buzz/actions"}]"#.to_string(),
            checks_status: 0,
            checks_stderr: String::new(),
            rules: "[[]]".to_string(),
            rules_status: 0,
            rules_stderr: String::new(),
            merge_stdout: r#"{"sha":"unverified123","merged":true,"message":"Pull Request successfully merged"}"#.to_string(),
            merge_status: 0,
            merge_stderr: String::new(),
            auth_status: 0,
            pulls: pull_response(),
        }
    }
}

#[cfg(unix)]
fn view_response(
    state: &str,
    head: &str,
    merged_at: Option<&str>,
    merge_commit: Option<&str>,
) -> String {
    let mut value = ready_view_value();
    value["state"] = serde_json::json!(state);
    value["headRefOid"] = serde_json::json!(head);
    value["mergedAt"] = serde_json::json!(merged_at);
    value["mergeCommit"] = merge_commit
        .map(|oid| serde_json::json!({ "oid": oid }))
        .unwrap_or(serde_json::Value::Null);
    value.to_string()
}

#[cfg(unix)]
fn ready_view_value() -> serde_json::Value {
    serde_json::json!({
        "number": 42,
        "url": "https://github.com/acme/buzz/pull/42",
        "state": "OPEN",
        "headRefOid": "abcdef0123456789",
        "isDraft": false,
        "mergeable": "MERGEABLE",
        "mergeStateStatus": "CLEAN",
        "reviewDecision": "APPROVED",
        "autoMergeRequest": null,
        "mergeCommit": null,
        "mergedAt": null
    })
}

#[cfg(unix)]
fn fake_merge_gh(fake: MergeFake) -> (tempfile::TempDir, PathBuf) {
    let script = format!(
        r#"
root=${{0%/gh}}
printf '%s\t' "$@" >> "$root/calls"
printf '\036' >> "$root/calls"
if [ "${{1:-}}" = "auth" ]; then
    exit {auth_status}
fi
if [ "${{1:-}}" = "pr" ] && [ "${{2:-}}" = "view" ]; then
    if [ -e "$root/merged" ]; then
        printf '%s' {verified_view}
    else
        printf '%s' {initial_view}
    fi
    exit 0
fi
if [ "${{1:-}}" = "pr" ] && [ "${{2:-}}" = "checks" ]; then
    printf '%s' {checks}
    printf '%s' {checks_stderr} >&2
    exit {checks_status}
fi
method=
input=
endpoint=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --method) method=$2; shift 2 ;;
        --input) input=$2; shift 2 ;;
        repos/*) endpoint=$1; shift ;;
        *) shift ;;
    esac
done
case "$method:$endpoint" in
    GET:repos/acme/buzz/pulls)
        printf '%s' {pulls}
        ;;
    GET:repos/acme/buzz/rules/branches/*)
        printf '%s' {rules}
        printf '%s' {rules_stderr} >&2
        exit {rules_status}
        ;;
    PUT:repos/acme/buzz/pulls/42/merge)
        cp "$input" "$root/merge.json"
        : > "$root/merged"
        printf '%s' {merge_stdout}
        printf '%s' {merge_stderr} >&2
        exit {merge_status}
        ;;
    *)
        printf 'unexpected gh call: %s %s\n' "$method" "$endpoint" >&2
        exit 2
        ;;
esac
"#,
        auth_status = fake.auth_status,
        initial_view = shell_quote(&fake.initial_view),
        verified_view = shell_quote(&fake.verified_view),
        checks = shell_quote(&fake.checks),
        checks_stderr = shell_quote(&fake.checks_stderr),
        checks_status = fake.checks_status,
        pulls = shell_quote(&fake.pulls),
        rules = shell_quote(&fake.rules),
        rules_stderr = shell_quote(&fake.rules_stderr),
        rules_status = fake.rules_status,
        merge_stdout = shell_quote(&fake.merge_stdout),
        merge_stderr = shell_quote(&fake.merge_stderr),
        merge_status = fake.merge_status,
    );
    fake_gh(&script)
}

#[cfg(unix)]
fn assert_no_gate_or_merge_put_calls(calls: &[Vec<String>]) {
    assert_no_merge_put_call(calls);
    assert!(
        calls.iter().all(|args| !is_pr_checks_call(args)
            && !args.iter().any(|arg| arg.contains("/rules/branches/"))),
        "unexpected gate call: {calls:?}"
    );
}

#[cfg(unix)]
fn assert_no_merge_put_call(calls: &[Vec<String>]) {
    assert!(
        calls.iter().all(|args| !is_merge_put_call(args)),
        "unexpected merge PUT call: {calls:?}"
    );
}

#[cfg(unix)]
fn is_pr_checks_call(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "pr" && pair[1] == "checks")
}

#[cfg(unix)]
fn is_merge_put_call(args: &[String]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "--method" && pair[1] == "PUT")
        && args
            .iter()
            .any(|arg| arg == "repos/acme/buzz/pulls/42/merge")
}

#[cfg(unix)]
fn assert_forbidden_merge_commands_absent(calls: &[Vec<String>]) {
    assert!(calls.iter().all(|args| {
        !args
            .windows(2)
            .any(|pair| pair[0] == "pr" && pair[1] == "merge")
            && args
                .iter()
                .all(|arg| !matches!(arg.as_str(), "--admin" | "--auto" | "--queue"))
            && args
                .iter()
                .all(|arg| !arg.contains("Merge feature") && !arg.contains("Reviewed body"))
    }));
}

#[cfg(unix)]
fn recorded_pid(path: &std::path::Path) -> libc::pid_t {
    std::fs::read_to_string(path)
        .expect("read recorded pid")
        .trim()
        .parse()
        .expect("fake gh recorded a numeric pid")
}

#[cfg(unix)]
fn process_is_alive(pid: libc::pid_t) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(unix)]
fn kill_if_alive(pid: libc::pid_t) -> bool {
    let alive = process_is_alive(pid);
    if alive {
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    alive
}

#[cfg(unix)]
fn wait_for_recorded_pid(path: &std::path::Path) -> libc::pid_t {
    let deadline = Instant::now() + PID_PUBLICATION_TIMEOUT;
    while Instant::now() < deadline {
        if let Ok(value) = std::fs::read_to_string(path) {
            if let Ok(pid) = value.trim().parse() {
                return pid;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    recorded_pid(path)
}

#[test]
fn gh_runner_releases_tree_before_joining_readers() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let released = Arc::new(AtomicBool::new(false));
    let reader = |released: Arc<AtomicBool>| {
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_millis(250);
            while Instant::now() < deadline {
                if released.load(Ordering::SeqCst) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            false
        })
    };
    let stdout = reader(Arc::clone(&released));
    let stderr = reader(Arc::clone(&released));

    let (stdout_saw_cleanup, stderr_saw_cleanup) =
        join_gh_readers_after_cleanup(|| released.store(true, Ordering::SeqCst), stdout, stderr);

    assert!(stdout_saw_cleanup);
    assert!(stderr_saw_cleanup);
}

#[cfg(windows)]
#[test]
fn gh_windows_job_setup_returns_a_retained_handle() {
    let _: fn(
        u32,
    ) -> Result<
        crate::managed_agents::JobHandle,
        crate::commands::project_git_merge_error::ProjectPullRequestMergeError,
    > = super::create_windows_job;
}

#[cfg(unix)]
#[test]
fn gh_runner_preserves_direct_argv_and_closes_stdin() {
    let (_dir, binary) = fake_gh(
        r#"
test -z "$(cat)"
printf '<%s>\n' "$@"
"#,
    );
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(2),
    };

    let output = runner
        .run(&[
            OsString::from("value with spaces"),
            OsString::from("$(printf shell-injection)"),
            OsString::from("semi;colon"),
        ])
        .expect("run fake gh");

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        "<value with spaces>\n<$(printf shell-injection)>\n<semi;colon>\n"
    );
}

#[cfg(unix)]
#[test]
fn gh_runner_drains_both_streams_and_bounds_retention() {
    let (_dir, binary) = fake_gh(
        r#"
(head -c 70000 /dev/zero | tr '\0' x) &
(head -c 70000 /dev/zero | tr '\0' y >&2) &
wait
"#,
    );
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(5),
    };

    let output = runner.run(&[]).expect("drain fake gh output");

    assert!(output.status.success());
    assert_eq!(output.stdout.len(), GH_STREAM_LIMIT);
    assert_eq!(output.stderr.len(), GH_STREAM_LIMIT);
    assert!(output.stdout.bytes().all(|byte| byte == b'x'));
    assert!(output.stderr.bytes().all(|byte| byte == b'y'));
}

#[cfg(unix)]
#[test]
fn gh_runner_kills_and_reaps_a_timed_out_child() {
    let (_dir, binary) = fake_gh(
        r#"
/bin/sleep 8 &
printf '%s' "$!" > "$1"
while :; do :; done
"#,
    );
    let pid_file = tempfile::NamedTempFile::new().expect("create pid file");
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(2),
    };
    let started = Instant::now();
    let pid_path = pid_file.path().as_os_str().to_owned();
    let run = std::thread::spawn(move || runner.run(&[pid_path]));
    let descendant = wait_for_recorded_pid(pid_file.path());

    let error = run
        .join()
        .expect("timed-out fake gh runner thread should not panic")
        .expect_err("fake gh should time out");

    assert!(started.elapsed() < Duration::from_secs(4));
    let value = error_value(error);
    assert_eq!(value["code"], "github_merge_failed");
    assert!(value["message"].as_str().unwrap().contains("timed out"));
    std::thread::sleep(Duration::from_millis(200));
    let descendant_alive = kill_if_alive(descendant);
    assert!(
        !descendant_alive,
        "descendant PID {descendant} survived timeout cleanup"
    );
}

#[cfg(unix)]
#[test]
fn gh_runner_cleans_descendants_before_joining_after_normal_exit() {
    let (_dir, binary) = fake_gh(
        r#"
/bin/sleep 6 &
printf '%s' "$!" > "$1"
exit 0
"#,
    );
    let pid_file = tempfile::NamedTempFile::new().expect("create descendant pid file");
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(2),
    };
    let started = Instant::now();

    let output = runner
        .run(&[pid_file.path().as_os_str().to_owned()])
        .expect("fake gh leader exits successfully");
    let elapsed = started.elapsed();
    let descendant = recorded_pid(pid_file.path());
    std::thread::sleep(Duration::from_millis(200));
    let descendant_alive = kill_if_alive(descendant);

    assert!(output.status.success());
    assert!(
        elapsed < Duration::from_secs(4),
        "inherited output handles blocked runner for {elapsed:?}"
    );
    assert!(
        !descendant_alive,
        "descendant PID {descendant} survived normal-exit cleanup"
    );
}

#[test]
fn gh_runner_redacts_secrets_and_caps_user_diagnostics() {
    let secrets = [
        "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ123456",
        "github_pat_11AA22BB33CC44DD55EE66FF",
        "header-secret",
        "token-secret",
        "bearer-secret",
        "url-password",
        "basic-secret",
    ];
    let raw = format!(
        "{}\n{}\nAuthorization: Basic {} {}\ntoken {}\nBearer {}\nhttps://alice:{}@github.com/owner/repo\n{}",
        secrets[0],
        secrets[1],
        secrets[2],
        secrets[6],
        secrets[3],
        secrets[4],
        secrets[5],
        "A".repeat(GH_DIAGNOSTIC_LIMIT * 2),
    );

    let diagnostic = redact_diagnostic(&raw);

    assert!(diagnostic.contains("[REDACTED]"));
    assert!(secrets.iter().all(|secret| !diagnostic.contains(secret)));
    assert!(diagnostic.len() <= GH_DIAGNOSTIC_LIMIT);
}

#[test]
fn gh_runner_redacts_delimiter_adjacent_credentials() {
    let cases = [
        ("token: TOKEN_COLON_SECRET", "TOKEN_COLON_SECRET"),
        ("token=TOKEN_EQUALS_SECRET", "TOKEN_EQUALS_SECRET"),
        ("Bearer: BEARER_COLON_SECRET", "BEARER_COLON_SECRET"),
        ("Bearer=BEARER_EQUALS_SECRET", "BEARER_EQUALS_SECRET"),
    ];

    for (raw, secret) in cases {
        let diagnostic = redact_diagnostic(raw);
        assert!(diagnostic.contains("[REDACTED]"), "{raw}");
        assert!(!diagnostic.contains(secret), "leaked {secret}");
    }
}

#[test]
fn gh_runner_maps_resolution_failure_to_stable_missing_cli_code() {
    let error = GhRunner::from_resolved(None).expect_err("missing gh should fail");
    let value = error_value(error);

    assert_eq!(value["code"], "github_cli_missing");
    assert!(value["message"].as_str().unwrap().contains("Install gh"));
}

#[cfg(unix)]
#[test]
fn gh_runner_maps_exit_127_to_stable_missing_cli_code() {
    let (_dir, binary) = fake_gh("exit 127");
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value = error_value(runner.run(&[]).expect_err("exit 127 should fail"));

    assert_eq!(value["code"], "github_cli_missing");
}

#[cfg(unix)]
#[test]
fn gh_runner_maps_failed_auth_without_exposing_output() {
    let (_dir, binary) = fake_gh(
        r#"
printf '%s\n' 'ghp_AUTH_SECRET Authorization: Bearer AUTH_SECRET' >&2
exit 1
"#,
    );
    let runner = GhRunner {
        binary,
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };

    let value = error_value(runner.ensure_auth().expect_err("auth should fail"));

    assert_eq!(value["code"], "github_auth_required");
    assert_eq!(
        value["message"],
        "Authenticate GitHub CLI with: gh auth login --hostname github.com"
    );
    assert!(!value.to_string().contains("AUTH_SECRET"));
}
