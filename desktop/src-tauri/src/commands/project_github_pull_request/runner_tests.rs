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
        timeout: LIFECYCLE_FAKE_TIMEOUT,
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
fn gh_runner_run_with_limit_keeps_more_than_default_stream_cap() {
    let (_dir, binary) = fake_gh(
        r#"
head -c 200000 /dev/zero | tr '\0' r
"#,
    );
    let runner = GhRunner {
        binary,
        timeout: Duration::from_secs(5),
    };
    let output = runner
        .run_with_limit(&[], 256 * 1024)
        .expect("drain fake gh output");
    assert!(output.status.success());
    assert_eq!(output.stdout.len(), 200_000);
    assert!(output.stdout.bytes().all(|byte| byte == b'r'));
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
        timeout: LIFECYCLE_FAKE_TIMEOUT,
    };
    let started = Instant::now();
    let pid_path = pid_file.path().as_os_str().to_owned();
    let run = std::thread::spawn(move || runner.run(&[pid_path]));
    let descendant = wait_for_recorded_pid(pid_file.path());

    let error = run
        .join()
        .expect("timed-out fake gh runner thread should not panic")
        .expect_err("fake gh should time out");

    assert!(started.elapsed() < LIFECYCLE_FAKE_TIMEOUT + Duration::from_secs(2));
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
        timeout: LIFECYCLE_FAKE_TIMEOUT,
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
