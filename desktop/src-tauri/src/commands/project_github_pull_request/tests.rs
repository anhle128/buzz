use super::{redact_diagnostic, GhRunner, GitHubRepoRef, GH_DIAGNOSTIC_LIMIT, GH_STREAM_LIMIT};
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, Instant};

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

    let error = runner
        .run(&[pid_file.path().as_os_str().to_owned()])
        .expect_err("fake gh should time out");

    assert!(started.elapsed() < Duration::from_secs(4));
    let value = error_value(error);
    assert_eq!(value["code"], "github_merge_failed");
    assert!(value["message"].as_str().unwrap().contains("timed out"));
    let descendant = recorded_pid(pid_file.path());
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
