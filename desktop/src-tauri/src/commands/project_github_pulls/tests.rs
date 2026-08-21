use super::*;
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{GhRunner, GitHubRepoRef};
use serde_json::json;
use std::path::PathBuf;

fn error_code(error: &ProjectPullRequestMergeError) -> String {
    serde_json::to_value(error).expect("serialize error")["code"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn projected_pull(number: u64, draft: bool) -> serde_json::Value {
    json!({
        "number": number,
        "title": format!("Pull {number}"),
        "body": "PR body",
        "html_url": format!("https://github.com/acme/app/pull/{number}"),
        "draft": draft,
        "comments": 3,
        "created_at": "2026-01-02T03:04:05Z",
        "updated_at": "2026-01-03T03:04:05Z",
        "user": { "login": "ada", "avatar_url": "https://avatars.githubusercontent.com/u/1" },
        "head": {
            "ref": "feature",
            "sha": "dddddddddddddddddddddddddddddddddddddddd",
            "repo": { "full_name": "acme/app" }
        },
        "base": {
            "ref": "develop",
            "repo": { "full_name": "acme/app" }
        }
    })
}

#[cfg(unix)]
fn fake_gh_raw(output: &str, status: i32, stderr: &str) -> (tempfile::TempDir, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("create fake gh directory");
    let path = dir.path().join("gh");
    let script = format!(
        "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\n  *) printf '%s' '{}' ; printf '%s' '{}' >&2; exit {} ;;\nesac\n",
        output,
        stderr,
        status,
    );
    std::fs::write(&path, script).expect("write fake gh");
    let mut permissions = std::fs::metadata(&path)
        .expect("stat fake gh")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod fake gh");
    (dir, path)
}

#[cfg(unix)]
fn fake_gh(output: &serde_json::Value, status: i32, stderr: &str) -> (tempfile::TempDir, PathBuf) {
    fake_gh_raw(&output.to_string(), status, stderr)
}

#[cfg(unix)]
#[test]
fn maps_projected_open_and_draft_pull_fields() {
    let output = json!([projected_pull(42, false), projected_pull(7, true)]);
    let (dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect("list");
    assert_eq!(page.pulls.len(), 2);
    assert_eq!(page.pulls[0].number, 42);
    assert!(!page.pulls[0].draft);
    assert_eq!(page.pulls[0].user.login, "ada");
    assert_eq!(page.pulls[0].head.git_ref, "feature");
    assert_eq!(
        page.pulls[0].head.sha,
        "dddddddddddddddddddddddddddddddddddddddd"
    );
    assert_eq!(page.pulls[0].head.repo.full_name, "acme/app");
    assert_eq!(page.pulls[0].base.git_ref, "develop");
    assert!(page.pulls[1].draft);
    assert!(!page.has_more);
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(
        calls.contains("/repos/acme/app/pulls?state=open&per_page=100&sort=updated&direction=desc")
    );
    assert!(calls.contains("--jq"));
    assert!(!calls.contains("--paginate"));
}

#[cfg(unix)]
#[test]
fn inbound_fork_keeps_head_repo_full_name() {
    let mut item = projected_pull(42, false);
    item["head"]["repo"]["full_name"] = json!("other/app");
    item["head"]["ref"] = json!("fork-branch");
    let (_dir, path) = fake_gh(&json!([item]), 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect("list");
    assert_eq!(page.pulls[0].head.repo.full_name, "other/app");
    assert_eq!(page.pulls[0].head.git_ref, "fork-branch");
}

#[cfg(unix)]
#[test]
fn raw_projected_page_of_100_sets_has_more_before_filtering() {
    let output = serde_json::Value::Array(
        (1..=100)
            .map(|number| {
                let mut item = projected_pull(number, false);
                item["html_url"] = json!("https://evil.example/pull/1");
                item
            })
            .collect(),
    );
    let (_dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect("list");
    assert!(page.pulls.is_empty());
    assert!(page.has_more);
}

#[test]
fn accepts_only_repo_bound_pull_urls() {
    let repo = GitHubRepoRef::parse("https://github.com/Acme/App").expect("repo");
    for (raw, number, expected) in [
        ("https://github.com/acme/app/pull/42", 42, true),
        ("https://github.com/ACME/APP/pull/42", 42, true),
        ("https://github.com/acme/app/pull/43", 42, false),
        ("https://github.com/acme/other/pull/42", 42, false),
        ("https://github.com/acme/app/issues/42", 42, false),
        ("https://evil.example/acme/app/pull/42", 42, false),
        ("https://user@github.com/acme/app/pull/42", 42, false),
        ("https://github.com/acme/app/pull/42?x=1", 42, false),
        ("https://github.com/acme/app/pull/42#x", 42, false),
        ("https://github.com/acme/app/pull/42/", 42, false),
    ] {
        assert_eq!(is_pull_html_url(&repo, raw, number), expected, "{raw}");
    }
}

#[test]
fn rejects_non_github_clone_url_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
    let error = list_github_pull_requests_with(
        &gh,
        &format!("https://relay.example/git/{}/app", "ab".repeat(32)),
    )
    .expect_err("reject Buzz URL");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[cfg(unix)]
#[test]
fn malformed_or_truncated_json_is_an_error_not_an_empty_list() {
    let (_dir, path) = fake_gh_raw("[{", 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = list_github_pull_requests_with(&gh, "https://github.com/acme/app")
        .expect_err("malformed response");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[cfg(unix)]
#[test]
fn failed_auth_maps_to_auth_required() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("gh");
    std::fs::write(
        &path,
        "#!/bin/sh\ncase \"$*\" in *auth*status*) exit 1 ;; *) exit 1 ;; esac\n",
    )
    .expect("write fake gh");
    let mut permissions = std::fs::metadata(&path).expect("stat").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error =
        list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect_err("auth");
    assert_eq!(error_code(&error), "github_auth_required");
}

#[test]
fn remaps_repository_and_rate_failures_to_pull_codes() {
    let not_found = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 404");
    assert_eq!(
        error_code(&remap_pulls_error(
            not_found,
            "Not Found",
            PullsNotFound::Repo
        )),
        "github_repo_unavailable"
    );
    let forbidden = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
    assert_eq!(
        error_code(&remap_pulls_error(
            forbidden,
            "Forbidden",
            PullsNotFound::Repo
        )),
        "github_repo_unavailable"
    );
    let limited = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
    assert_eq!(
        error_code(&remap_pulls_error(
            limited,
            "API rate limit exceeded",
            PullsNotFound::Repo
        )),
        "github_pulls_failed"
    );
    let missing_pr = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 404");
    assert_eq!(
        error_code(&remap_pulls_error(
            missing_pr,
            "Not Found",
            PullsNotFound::PullRequest
        )),
        "github_pr_unavailable"
    );
}

#[test]
fn create_rejects_blank_title_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let error = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "   ",
        "body",
        "feature",
        "develop",
    )
    .expect_err("blank");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn create_rejects_more_than_256_unicode_scalars_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let title = "é".repeat(257);
    let error = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        &title,
        "",
        "feature",
        "develop",
    )
    .expect_err("long");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn create_rejects_empty_or_equal_branches_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let empty_head = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "Title",
        "",
        "  ",
        "develop",
    )
    .expect_err("empty head");
    assert_eq!(error_code(&empty_head), "github_pulls_failed");
    let empty_base = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "Title",
        "",
        "feature",
        "  ",
    )
    .expect_err("empty base");
    assert_eq!(error_code(&empty_base), "github_pulls_failed");
    let same = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "Title",
        "",
        "develop",
        "develop",
    )
    .expect_err("same");
    assert_eq!(error_code(&same), "github_pulls_failed");
}

#[test]
fn create_rejects_cross_repository_head_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let error = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "Title",
        "",
        "fork-owner:feature",
        "develop",
    )
    .expect_err("cross-repository head");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[cfg(unix)]
#[test]
fn create_posts_trimmed_title_and_exact_body() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("gh");
    let output = projected_pull(43, false).to_string();
    let script = format!(
        "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\n  *--method*POST*)\n    previous=\"\"\n    for argument in \"$@\"; do\n      if [ \"$previous\" = \"--input\" ]; then cp \"$argument\" \"$root/input.json\"; fi\n      previous=\"$argument\"\n    done\n    printf '%s' '{}'\n    ;;\n  *) exit 1 ;;\nesac\n",
        output,
    );
    std::fs::write(&path, script).expect("write fake gh");
    let mut permissions = std::fs::metadata(&path).expect("stat").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("chmod");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let pull = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "  Pull 43  ",
        " body with surrounding space ",
        "  feature  ",
        "  develop  ",
    )
    .expect("create");
    assert_eq!(pull.number, 43);
    let input: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
    )
    .expect("json");
    assert_eq!(
        input,
        serde_json::json!({
            "title": "Pull 43",
            "body": " body with surrounding space ",
            "head": "feature",
            "base": "develop",
        })
    );
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("--method POST"));
    assert!(calls.contains("/repos/acme/app/pulls"));
    assert!(calls.contains("--input"));
    assert!(!calls.contains("/pulls/"));
}

#[cfg(unix)]
#[test]
fn create_rejects_response_with_foreign_html_url() {
    let mut output = projected_pull(43, false);
    output["html_url"] = serde_json::Value::String("https://evil.example/pull/43".to_string());
    let (_dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = create_github_pull_request_with(
        &gh,
        "https://github.com/acme/app",
        "Pull 43",
        "",
        "feature",
        "develop",
    )
    .expect_err("foreign URL");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn comments_reject_number_zero_before_running_gh() {
    let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
    let error = list_github_pull_request_comments_with(&gh, "https://github.com/acme/app", 0)
        .expect_err("zero");
    assert_eq!(error_code(&error), "github_pulls_failed");
}

#[test]
fn accepts_issue_and_pull_comment_urls_for_the_same_number() {
    let repo = GitHubRepoRef::parse("https://github.com/acme/app").expect("repo");
    assert!(is_pull_comment_html_url(
        &repo,
        "https://github.com/acme/app/issues/42#issuecomment-9",
        42,
        9,
    ));
    assert!(is_pull_comment_html_url(
        &repo,
        "https://github.com/acme/app/pull/42#issuecomment-9",
        42,
        9,
    ));
    assert!(!is_pull_comment_html_url(
        &repo,
        "https://github.com/acme/app/pull/42#discussion_r9",
        42,
        9,
    ));
    assert!(!is_pull_comment_html_url(
        &repo,
        "https://github.com/acme/other/issues/42#issuecomment-9",
        42,
        9,
    ));
}

#[cfg(unix)]
#[test]
fn comments_keep_projected_github_order_and_drop_invalid_urls() {
    let output = serde_json::json!([
        { "id": 1, "body": "first", "html_url": "https://github.com/acme/app/issues/42#issuecomment-1", "created_at": "2026-01-02T03:04:05Z", "user": { "login": "ada", "avatar_url": "https://example.com/a" } },
        { "id": 2, "body": "bad", "html_url": "https://evil.example/comment/2", "created_at": "2026-01-02T04:04:05Z", "user": { "login": "eve", "avatar_url": "https://example.com/e" } },
        { "id": 3, "body": "second", "html_url": "https://github.com/acme/app/pull/42#issuecomment-3", "created_at": "2026-01-02T05:04:05Z", "user": { "login": "linus", "avatar_url": "https://example.com/b" } }
    ]);
    let (dir, path) = fake_gh(&output, 0, "");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let comments = list_github_pull_request_comments_with(&gh, "https://github.com/acme/app", 42)
        .expect("comments");
    assert_eq!(
        comments
            .iter()
            .map(|comment| comment.body.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(comments[1].user.login, "linus");
    let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("/repos/acme/app/issues/42/comments?per_page=100"));
    assert!(!calls.contains("/pulls/42/comments"));
}

#[cfg(unix)]
#[test]
fn comments_map_http_404_to_pr_unavailable() {
    let (_dir, path) = fake_gh(&serde_json::json!({}), 1, "gh: HTTP 404 Not Found");
    let gh = GhRunner::from_resolved(Some(path)).expect("runner");
    let error = list_github_pull_request_comments_with(&gh, "https://github.com/acme/app", 42)
        .expect_err("missing");
    assert_eq!(error_code(&error), "github_pr_unavailable");
}

#[test]
fn list_wrapper_maps_missing_discovered_cli() {
    let error = list_github_pull_requests_with_runner(
        "https://github.com/acme/app".to_string(),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}

#[test]
fn create_wrapper_maps_missing_discovered_cli() {
    let error = create_github_pull_request_with_runner(
        "https://github.com/acme/app".to_string(),
        "title".to_string(),
        "body".to_string(),
        "feature".to_string(),
        "develop".to_string(),
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}

#[test]
fn comments_wrapper_maps_missing_discovered_cli() {
    let error = list_github_pull_request_comments_with_runner(
        "https://github.com/acme/app".to_string(),
        42,
        GhRunner::from_resolved(None),
    )
    .expect_err("missing");
    assert_eq!(error_code(&error), "github_cli_missing");
}
