use super::{
    find_or_create_pull_request_with, join_gh_readers_after_cleanup, redact_diagnostic,
    select_pull, GhRunner, GitHubMergeInput, GitHubRepoRef, PullSelection, GH_DIAGNOSTIC_LIMIT,
    GH_STREAM_LIMIT,
};
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
    let (dir, binary) = fake_lifecycle_gh("[[]]", &pull_response(), 0);
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(2),
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
    assert!(api_calls[1]
        .windows(2)
        .any(|pair| pair[0] == "--method" && pair[1] == "POST"));
    assert!(api_calls[1].iter().any(|arg| arg == "--input"));
    assert!(!api_calls[1].contains(&input.title));
    assert!(!api_calls[1].contains(&input.body));

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
    let (dir, binary) = fake_lifecycle_gh("[[]]", &pull_response(), 1);
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(2),
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
fn github_pull_creation_reuses_existing_open_pull_without_posting() {
    let (dir, binary) = fake_lifecycle_gh(&pull_response(), &pull_response(), 0);
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(2),
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
    let (dir, binary) = fake_lifecycle_gh("[[]]", &same_repo_pull, 0);
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(2),
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
) -> (tempfile::TempDir, PathBuf) {
    let (dir, binary) = fake_gh(
        r#"
root=$(dirname "$0")
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
            cat "$root/list-second.json"
        else
            cat "$root/list-first.json"
        fi
        ;;
    POST)
        : > "$root/posted"
        cat "$input" > "$root/post.json"
        exit "$(cat "$root/post-status")"
        ;;
esac
"#,
    );
    std::fs::write(dir.path().join("list-first.json"), first_list).expect("write first list");
    std::fs::write(dir.path().join("list-second.json"), second_list).expect("write second list");
    std::fs::write(dir.path().join("post-status"), post_status.to_string())
        .expect("write post status");
    (dir, binary)
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
                "ref": "main",
                "repo": { "full_name": "acme/buzz" }
            }
        }
    ]])
    .to_string()
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
    let deadline = Instant::now() + Duration::from_secs(1);
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
        timeout: Duration::from_secs(2),
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
        timeout: Duration::from_secs(2),
    };

    let value = error_value(runner.ensure_auth().expect_err("auth should fail"));

    assert_eq!(value["code"], "github_auth_required");
    assert_eq!(
        value["message"],
        "Authenticate GitHub CLI with: gh auth login --hostname github.com"
    );
    assert!(!value.to_string().contains("AUTH_SECRET"));
}
