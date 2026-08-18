use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_issues::{
    github_api_json, github_json_input, map_issue, map_issue_comment, GitHubIssueCommentDto,
    GitHubIssueCommentWire, GitHubIssueDto, GitHubIssueWire, ISSUE_ITEM_JQ,
};
use crate::commands::project_github_pull_request::{redact_diagnostic, GhRunner, GitHubRepoRef};

const ISSUE_COMMENT_ITEM_JQ: &str = "{id, body: (.body // \"\"), html_url, created_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end)}";

/// Close or reopen one GitHub issue for a github.com clone URL.
pub(crate) fn update_github_issue_state_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    state: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    if !matches!(state, "open" | "closed") {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub issue state must be open or closed.",
        ));
    }
    if number == 0 {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub issue number must be greater than zero.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_issue_write_error(error, ""))?;
    let input = github_json_input(&serde_json::json!({ "state": state }))?;
    let path = format!("/repos/{}/issues/{number}", repo.slug());
    let raw: GitHubIssueWire =
        github_api_json(gh, "PATCH", &path, ISSUE_ITEM_JQ, Some(input.path()))
            .map_err(|error| remap_issue_write_error(error, ""))?;
    map_issue(&repo, raw).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub returned an invalid updated issue.",
        )
    })
}

/// Create one GitHub issue comment for a github.com clone URL.
pub(crate) fn create_github_issue_comment_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    body: &str,
) -> Result<GitHubIssueCommentDto, ProjectPullRequestMergeError> {
    let body = body.trim();
    if body.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "Comment body is required.",
        ));
    }
    if number == 0 {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub issue number must be greater than zero.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_issue_write_error(error, ""))?;
    let input = github_json_input(&serde_json::json!({ "body": body }))?;
    let path = format!("/repos/{}/issues/{number}/comments", repo.slug());
    let raw: GitHubIssueCommentWire =
        github_api_json(gh, "POST", &path, ISSUE_COMMENT_ITEM_JQ, Some(input.path()))
            .map_err(|error| remap_issue_write_error(error, ""))?;
    map_issue_comment(&repo, number, raw).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub returned an invalid created comment.",
        )
    })
}

/// Inject a resolved `gh` runner for close/reopen discovery tests.
pub(crate) fn update_github_issue_state_with_runner(
    clone_url: String,
    number: u64,
    state: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issue_write_error(error, ""))?;
    update_github_issue_state_with(&gh, &clone_url, number, &state)
}

/// Inject a resolved `gh` runner for comment-create discovery tests.
pub(crate) fn create_github_issue_comment_with_runner(
    clone_url: String,
    number: u64,
    body: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueCommentDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issue_write_error(error, ""))?;
    create_github_issue_comment_with(&gh, &clone_url, number, &body)
}

fn remap_issue_write_error(
    error: ProjectPullRequestMergeError,
    diagnostic: &str,
) -> ProjectPullRequestMergeError {
    let value = serde_json::to_value(&error).unwrap_or_default();
    let code = value
        .get("code")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    if matches!(code, "github_cli_missing" | "github_auth_required") {
        return error;
    }
    let original = value
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let combined = format!("{diagnostic} {original}");
    let lower = combined.to_ascii_lowercase();
    let message = if diagnostic.trim().is_empty() {
        if original.is_empty() {
            "GitHub issue request failed.".to_string()
        } else {
            original.to_string()
        }
    } else {
        redact_diagnostic(diagnostic)
    };
    if lower.contains("rate limit") || lower.contains("abuse") {
        return ProjectPullRequestMergeError::new("github_issues_failed", message);
    }
    if lower.contains("404") || lower.contains("not found") {
        return ProjectPullRequestMergeError::new("github_issue_unavailable", message);
    }
    if lower.contains("403") && !lower.contains("rate") {
        return ProjectPullRequestMergeError::new("github_repo_unavailable", message);
    }
    ProjectPullRequestMergeError::new("github_issues_failed", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
    use crate::commands::project_github_pull_request::GhRunner;
    use serde_json::json;
    use std::path::PathBuf;

    fn error_code(error: &ProjectPullRequestMergeError) -> String {
        serde_json::to_value(error).expect("serialize error")["code"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    fn projected_issue(number: u64, state: &str) -> serde_json::Value {
        json!({
            "number": number,
            "title": format!("Issue {number}"),
            "body": "Steps",
            "state": state,
            "html_url": format!("https://github.com/acme/app/issues/{number}"),
            "comments": 3,
            "created_at": "2026-01-02T03:04:05Z",
            "updated_at": "2026-01-03T03:04:05Z",
            "user": { "login": "ada", "avatar_url": "https://avatars.githubusercontent.com/u/1" },
            "labels": ["bug"],
            "assignees": [{ "login": "linus", "avatar_url": "https://avatars.githubusercontent.com/u/2" }],
            "has_pull_request": false,
        })
    }

    #[cfg(unix)]
    fn fake_gh_input(
        output: &serde_json::Value,
        status: i32,
        stderr: &str,
    ) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("create fake gh directory");
        let path = dir.path().join("gh");
        let script = format!(
            "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\n  *)\n    previous=\"\"\n    for argument in \"$@\"; do\n      if [ \"$previous\" = \"--input\" ]; then cp \"$argument\" \"$root/input.json\"; fi\n      previous=\"$argument\"\n    done\n    printf '%s' '{}'\n    printf '%s' '{}' >&2\n    exit {}\n    ;;\nesac\n",
            output, stderr, status,
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
    #[test]
    fn state_rejects_unknown_and_zero_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        assert_eq!(
            error_code(
                &update_github_issue_state_with(&gh, "https://github.com/acme/app", 42, "done",)
                    .expect_err("state")
            ),
            "github_issues_failed",
        );
        assert_eq!(
            error_code(
                &update_github_issue_state_with(&gh, "https://github.com/acme/app", 0, "closed",)
                    .expect_err("number")
            ),
            "github_issues_failed",
        );
    }

    #[cfg(unix)]
    #[test]
    fn close_patches_state_only() {
        let output = projected_issue(42, "closed");
        let (dir, path) = fake_gh_input(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let issue =
            update_github_issue_state_with(&gh, "https://github.com/acme/app", 42, "closed")
                .expect("close");
        assert_eq!(issue.state, "closed");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
            )
            .expect("json"),
            json!({ "state": "closed" }),
        );
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
        assert!(
            calls.contains("api --hostname github.com --method PATCH /repos/acme/app/issues/42")
        );
    }

    #[cfg(unix)]
    #[test]
    fn comment_posts_trimmed_body_and_validates_html_url() {
        let output = json!({
            "id": 9,
            "body": "Looks good",
            "html_url": "https://github.com/acme/app/issues/42#issuecomment-9",
            "created_at": "2026-01-02T03:04:05Z",
            "user": { "login": "ada", "avatar_url": "" }
        });
        let (dir, path) = fake_gh_input(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let comment = create_github_issue_comment_with(
            &gh,
            "https://github.com/acme/app",
            42,
            "  Looks good  ",
        )
        .expect("comment");
        assert_eq!(comment.id, 9);
        let input: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
        )
        .expect("json");
        assert_eq!(input, json!({ "body": "Looks good" }));
    }

    #[cfg(unix)]
    #[test]
    fn comment_rejects_blank_body_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = create_github_issue_comment_with(&gh, "https://github.com/acme/app", 42, "   ")
            .expect_err("blank");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[cfg(unix)]
    #[test]
    fn comment_rejects_foreign_html_url() {
        let output = json!({
            "id": 9,
            "body": "Looks good",
            "html_url": "https://evil.example/acme/app/issues/42#issuecomment-9",
            "created_at": "2026-01-02T03:04:05Z",
            "user": { "login": "ada", "avatar_url": "" }
        });
        let (_dir, path) = fake_gh_input(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let error =
            create_github_issue_comment_with(&gh, "https://github.com/acme/app", 42, "Looks good")
                .expect_err("foreign URL");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[cfg(unix)]
    #[test]
    fn writes_reject_buzz_url_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = update_github_issue_state_with(
            &gh,
            "https://relay.example/git/owner/app",
            42,
            "closed",
        )
        .expect_err("host");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[test]
    fn write_remap_turns_issue_404_into_issue_unavailable() {
        let error = ProjectPullRequestMergeError::new("github_repo_unavailable", "gh: HTTP 404");
        assert_eq!(
            error_code(&remap_issue_write_error(error, "Not Found")),
            "github_issue_unavailable",
        );
    }

    #[test]
    fn state_wrapper_maps_missing_discovered_cli() {
        let error = update_github_issue_state_with_runner(
            "https://github.com/acme/app".to_string(),
            42,
            "closed".to_string(),
            GhRunner::from_resolved(None),
        )
        .expect_err("missing");
        assert_eq!(error_code(&error), "github_cli_missing");
    }

    #[test]
    fn comment_wrapper_maps_missing_discovered_cli() {
        let error = create_github_issue_comment_with_runner(
            "https://github.com/acme/app".to_string(),
            42,
            "Looks good".to_string(),
            GhRunner::from_resolved(None),
        )
        .expect_err("missing");
        assert_eq!(error_code(&error), "github_cli_missing");
    }
}
