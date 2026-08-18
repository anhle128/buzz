use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_issues::{
    github_api_json, github_json_input, map_issue, map_issue_comment, GitHubIssueCommentDto,
    GitHubIssueCommentWire, GitHubIssueDto, GitHubIssueUserDto, GitHubIssueWire, ISSUE_ITEM_JQ,
};
use crate::commands::project_github_pull_request::{redact_diagnostic, GhRunner, GitHubRepoRef};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};

const ISSUE_COMMENT_ITEM_JQ: &str = "{id, body: (.body // \"\"), html_url, created_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end)}";
const REPO_LABELS_JQ: &str = "[.[] | {name, color: (.color // \"\")}]";
const REPO_ASSIGNEES_JQ: &str = "[.[] | {login, avatar_url: (.avatar_url // \"\")}]";
const USER_JQ: &str = "{login: (.login // \"\"), avatar_url: (.avatar_url // \"\")}";

/// One repository label from the GitHub catalog.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubRepoLabelDto {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Deserialize)]
struct GitHubRepoLabelWire {
    name: String,
    color: String,
}

#[derive(Debug, Deserialize)]
struct GitHubIssueUserWire {
    login: String,
    avatar_url: String,
}

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

/// List repository labels for a github.com clone URL.
pub(crate) fn list_github_repo_labels_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<Vec<GitHubRepoLabelDto>, ProjectPullRequestMergeError> {
    let repo = parse_github_repo(clone_url)?;
    gh.ensure_auth()
        .map_err(|error| remap_issue_write_error(error, ""))?;
    let path = format!("/repos/{}/labels?per_page=100", repo.slug());
    let raw: Vec<GitHubRepoLabelWire> = github_api_json(gh, "GET", &path, REPO_LABELS_JQ, None)?;
    Ok(map_repo_labels(raw))
}

/// Add one label to a GitHub issue for a github.com clone URL.
pub(crate) fn add_github_issue_labels_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    name: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let name = require_label_name(name)?;
    require_issue_number(number)?;
    let repo = parse_github_repo(clone_url)?;
    gh.ensure_auth()
        .map_err(|error| remap_issue_write_error(error, ""))?;
    let input = github_json_input(&serde_json::json!({ "labels": [name] }))?;
    let path = format!("/repos/{}/issues/{number}/labels", repo.slug());
    let _: serde_json::Value =
        github_api_json(gh, "POST", &path, REPO_LABELS_JQ, Some(input.path()))
            .map_err(|error| remap_label_write_error(error, ""))?;
    fetch_updated_issue(gh, &repo, number)
}

/// Remove one label from a GitHub issue for a github.com clone URL.
pub(crate) fn remove_github_issue_label_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    name: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let name = require_label_name(name)?;
    require_issue_number(number)?;
    let repo = parse_github_repo(clone_url)?;
    gh.ensure_auth()
        .map_err(|error| remap_issue_write_error(error, ""))?;
    let encoded = utf8_percent_encode(name, NON_ALPHANUMERIC);
    let path = format!("/repos/{}/issues/{number}/labels/{encoded}", repo.slug());
    let _: serde_json::Value = github_api_json(gh, "DELETE", &path, REPO_LABELS_JQ, None)
        .map_err(|error| remap_label_write_error(error, ""))?;
    fetch_updated_issue(gh, &repo, number)
}

/// List assignable users for a github.com clone URL.
pub(crate) fn list_github_repo_assignees_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<Vec<GitHubIssueUserDto>, ProjectPullRequestMergeError> {
    let repo = parse_github_repo(clone_url)?;
    gh.ensure_auth()
        .map_err(|error| remap_issue_write_error(error, ""))?;
    let path = format!("/repos/{}/assignees?per_page=100", repo.slug());
    let raw: Vec<GitHubIssueUserWire> = github_api_json(gh, "GET", &path, REPO_ASSIGNEES_JQ, None)?;
    Ok(map_repo_assignees(raw))
}

/// Add one assignee to a GitHub issue for a github.com clone URL.
pub(crate) fn add_github_issue_assignees_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    login: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    mutate_github_issue_assignees(gh, clone_url, number, login, "POST")
}

/// Remove one assignee from a GitHub issue for a github.com clone URL.
pub(crate) fn remove_github_issue_assignee_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    login: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    mutate_github_issue_assignees(gh, clone_url, number, login, "DELETE")
}

/// Return the GitHub user authenticated to `gh`.
pub(crate) fn get_github_authenticated_user_with(
    gh: &GhRunner,
) -> Result<GitHubIssueUserDto, ProjectPullRequestMergeError> {
    gh.ensure_auth()
        .map_err(|error| remap_issue_write_error(error, ""))?;
    let raw: GitHubIssueUserWire = github_api_json(gh, "GET", "/user", USER_JQ, None)?;
    if raw.login.trim().is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub returned an invalid authenticated user.",
        ));
    }
    Ok(GitHubIssueUserDto {
        login: raw.login,
        avatar_url: raw.avatar_url,
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

/// Inject a resolved `gh` runner for label catalog discovery tests.
pub(crate) fn list_github_repo_labels_with_runner(
    clone_url: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<Vec<GitHubRepoLabelDto>, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issue_write_error(error, ""))?;
    list_github_repo_labels_with(&gh, &clone_url)
}

/// Inject a resolved `gh` runner for label-add discovery tests.
pub(crate) fn add_github_issue_labels_with_runner(
    clone_url: String,
    number: u64,
    name: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issue_write_error(error, ""))?;
    add_github_issue_labels_with(&gh, &clone_url, number, &name)
}

/// Inject a resolved `gh` runner for label-remove discovery tests.
pub(crate) fn remove_github_issue_label_with_runner(
    clone_url: String,
    number: u64,
    name: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issue_write_error(error, ""))?;
    remove_github_issue_label_with(&gh, &clone_url, number, &name)
}

/// Inject a resolved `gh` runner for assignee catalog discovery tests.
pub(crate) fn list_github_repo_assignees_with_runner(
    clone_url: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<Vec<GitHubIssueUserDto>, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issue_write_error(error, ""))?;
    list_github_repo_assignees_with(&gh, &clone_url)
}

/// Inject a resolved `gh` runner for assignee-add discovery tests.
pub(crate) fn add_github_issue_assignees_with_runner(
    clone_url: String,
    number: u64,
    login: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issue_write_error(error, ""))?;
    add_github_issue_assignees_with(&gh, &clone_url, number, &login)
}

/// Inject a resolved `gh` runner for assignee-remove discovery tests.
pub(crate) fn remove_github_issue_assignee_with_runner(
    clone_url: String,
    number: u64,
    login: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issue_write_error(error, ""))?;
    remove_github_issue_assignee_with(&gh, &clone_url, number, &login)
}

/// Inject a resolved `gh` runner for authenticated-user discovery tests.
pub(crate) fn get_github_authenticated_user_with_runner(
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueUserDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issue_write_error(error, ""))?;
    get_github_authenticated_user_with(&gh)
}

/// Close or reopen one GitHub issue for a github.com clone URL.
#[tauri::command]
pub async fn update_github_issue_state(
    clone_url: String,
    number: u64,
    state: String,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        update_github_issue_state_with_runner(clone_url, number, state, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// Create one GitHub issue comment for a github.com clone URL.
#[tauri::command]
pub async fn create_github_issue_comment(
    clone_url: String,
    number: u64,
    body: String,
) -> Result<GitHubIssueCommentDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        create_github_issue_comment_with_runner(clone_url, number, body, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// List repository labels for a github.com clone URL.
#[tauri::command]
pub async fn list_github_repo_labels(
    clone_url: String,
) -> Result<Vec<GitHubRepoLabelDto>, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_repo_labels_with_runner(clone_url, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// Add one label to a GitHub issue for a github.com clone URL.
#[tauri::command]
pub async fn add_github_issue_labels(
    clone_url: String,
    number: u64,
    name: String,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        add_github_issue_labels_with_runner(clone_url, number, name, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// Remove one label from a GitHub issue for a github.com clone URL.
#[tauri::command]
pub async fn remove_github_issue_label(
    clone_url: String,
    number: u64,
    name: String,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        remove_github_issue_label_with_runner(clone_url, number, name, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// List assignable users for a github.com clone URL.
#[tauri::command]
pub async fn list_github_repo_assignees(
    clone_url: String,
) -> Result<Vec<GitHubIssueUserDto>, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_repo_assignees_with_runner(clone_url, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// Add one assignee to a GitHub issue for a github.com clone URL.
#[tauri::command]
pub async fn add_github_issue_assignees(
    clone_url: String,
    number: u64,
    login: String,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        add_github_issue_assignees_with_runner(clone_url, number, login, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// Remove one assignee from a GitHub issue for a github.com clone URL.
#[tauri::command]
pub async fn remove_github_issue_assignee(
    clone_url: String,
    number: u64,
    login: String,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        remove_github_issue_assignee_with_runner(clone_url, number, login, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// Return the GitHub user authenticated to `gh`.
#[tauri::command]
pub async fn get_github_authenticated_user(
) -> Result<GitHubIssueUserDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        get_github_authenticated_user_with_runner(GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

fn parse_github_repo(clone_url: &str) -> Result<GitHubRepoRef, ProjectPullRequestMergeError> {
    GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))
}

fn require_label_name(name: &str) -> Result<&str, ProjectPullRequestMergeError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "Label name is required.",
        ));
    }
    Ok(name)
}

fn require_assignee_login(login: &str) -> Result<&str, ProjectPullRequestMergeError> {
    let login = login.trim();
    if login.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "Assignee login is required.",
        ));
    }
    Ok(login)
}

fn require_issue_number(number: u64) -> Result<(), ProjectPullRequestMergeError> {
    if number == 0 {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub issue number must be greater than zero.",
        ));
    }
    Ok(())
}

fn fetch_updated_issue(
    gh: &GhRunner,
    repo: &GitHubRepoRef,
    number: u64,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let path = format!("/repos/{}/issues/{number}", repo.slug());
    let raw: GitHubIssueWire = github_api_json(gh, "GET", &path, ISSUE_ITEM_JQ, None)
        .map_err(|error| remap_issue_write_error(error, ""))?;
    map_issue(repo, raw).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub returned an invalid updated issue.",
        )
    })
}

fn mutate_github_issue_assignees(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
    login: &str,
    method: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let login = require_assignee_login(login)?;
    require_issue_number(number)?;
    let repo = parse_github_repo(clone_url)?;
    gh.ensure_auth()
        .map_err(|error| remap_issue_write_error(error, ""))?;
    let input = github_json_input(&serde_json::json!({ "assignees": [login] }))?;
    let path = format!("/repos/{}/issues/{number}/assignees", repo.slug());
    let raw: GitHubIssueWire =
        github_api_json(gh, method, &path, ISSUE_ITEM_JQ, Some(input.path()))
            .map_err(|error| remap_issue_write_error(error, ""))?;
    map_issue(&repo, raw).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub returned an invalid updated issue.",
        )
    })
}

fn map_repo_labels(labels: Vec<GitHubRepoLabelWire>) -> Vec<GitHubRepoLabelDto> {
    labels
        .into_iter()
        .filter_map(|label| {
            if label.name.trim().is_empty() || !is_six_ascii_hex_color(&label.color) {
                return None;
            }
            Some(GitHubRepoLabelDto {
                name: label.name,
                color: label.color,
            })
        })
        .collect()
}

fn map_repo_assignees(users: Vec<GitHubIssueUserWire>) -> Vec<GitHubIssueUserDto> {
    users
        .into_iter()
        .filter(|user| !user.login.trim().is_empty())
        .map(|user| GitHubIssueUserDto {
            login: user.login,
            avatar_url: user.avatar_url,
        })
        .collect()
}

fn is_six_ascii_hex_color(color: &str) -> bool {
    color.len() == 6 && color.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn remap_label_write_error(
    error: ProjectPullRequestMergeError,
    diagnostic: &str,
) -> ProjectPullRequestMergeError {
    let remapped = remap_issue_write_error(error, diagnostic);
    let value = serde_json::to_value(&remapped).unwrap_or_default();
    if value.get("code").and_then(|value| value.as_str()) == Some("github_issue_unavailable") {
        let message = value
            .get("message")
            .and_then(|value| value.as_str())
            .filter(|message| !message.is_empty())
            .unwrap_or("GitHub issue request failed.");
        return ProjectPullRequestMergeError::new("github_issues_failed", message);
    }
    remapped
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
    fn fake_gh_dispatch() -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("create fake gh directory");
        let path = dir.path().join("gh");
        let issue = projected_issue(42, "open");
        let script = format!(
            "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\nesac\nprevious=\"\"\nfor argument in \"$@\"; do\n  if [ \"$previous\" = \"--input\" ]; then cp \"$argument\" \"$root/input.json\"; fi\n  previous=\"$argument\"\ndone\ncase \"$*\" in\n  */labels*)\n    printf '%s' '[]'\n    ;;\n  *)\n    printf '%s' '{}'\n    ;;\nesac\n",
            issue,
        );
        std::fs::write(&path, script).expect("write fake gh");
        let mut permissions = std::fs::metadata(&path)
            .expect("stat fake gh")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("chmod fake gh");
        (dir, path)
    }

    fn label_wire(name: &str, color: &str) -> GitHubRepoLabelWire {
        GitHubRepoLabelWire {
            name: name.to_string(),
            color: color.to_string(),
        }
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

    #[cfg(unix)]
    #[test]
    fn label_delete_percent_encodes_path_and_refetches_issue() {
        let (dir, path) = fake_gh_dispatch();
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let issue = remove_github_issue_label_with(
            &gh,
            "https://github.com/acme/app",
            42,
            "good first #issue",
        )
        .expect("remove label");
        assert_eq!(issue.number, 42);
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
        assert!(calls.contains("/repos/acme/app/issues/42/labels/good%20first%20%23issue"));
        assert!(calls.contains("--method GET /repos/acme/app/issues/42"));
        assert!(!calls.contains("labels/good first"));
    }

    #[cfg(unix)]
    #[test]
    fn assignee_add_sends_login_in_json_body() {
        let (dir, path) = fake_gh_dispatch();
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        add_github_issue_assignees_with(&gh, "https://github.com/acme/app", 42, "linus")
            .expect("assign");
        let input: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
        )
        .expect("json");
        assert_eq!(input, json!({ "assignees": ["linus"] }));
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
        assert!(calls.contains("/repos/acme/app/issues/42/assignees"));
        assert!(!calls.contains("/assignees/linus"));
    }

    #[test]
    fn label_catalog_accepts_only_nonempty_names_and_six_hex_colors() {
        let labels = map_repo_labels(vec![
            label_wire("bug", "d73a4a"),
            label_wire("docs", "0075ca"),
            label_wire("bad", "red"),
            label_wire("hash", "#d73a4a"),
            label_wire("   ", "ffffff"),
        ]);
        assert_eq!(
            labels
                .into_iter()
                .map(|label| (label.name, label.color))
                .collect::<Vec<_>>(),
            vec![
                ("bug".into(), "d73a4a".into()),
                ("docs".into(), "0075ca".into())
            ],
        );
    }

    #[cfg(unix)]
    #[test]
    fn label_rejects_blank_name_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = add_github_issue_labels_with(&gh, "https://github.com/acme/app", 42, "   ")
            .expect_err("blank");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[cfg(unix)]
    #[test]
    fn assignee_rejects_blank_login_and_zero_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let blank = add_github_issue_assignees_with(&gh, "https://github.com/acme/app", 42, "   ")
            .expect_err("blank");
        let zero =
            remove_github_issue_assignee_with(&gh, "https://github.com/acme/app", 0, "linus")
                .expect_err("zero");
        assert_eq!(error_code(&blank), "github_issues_failed");
        assert_eq!(error_code(&zero), "github_issues_failed");
    }

    #[cfg(unix)]
    #[test]
    fn user_maps_login_and_avatar() {
        let output = json!({ "login": "ada", "avatar_url": "https://example.com/ada" });
        let (_dir, path) = fake_gh_input(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let user = get_github_authenticated_user_with(&gh).expect("user");
        assert_eq!(user.login, "ada");
        assert_eq!(user.avatar_url, "https://example.com/ada");
    }

    #[cfg(unix)]
    #[test]
    fn catalog_404_is_repo_unavailable() {
        let (_dir, path) = fake_gh_input(&json!([]), 1, "gh: HTTP 404");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let error = list_github_repo_labels_with(&gh, "https://github.com/acme/app")
            .expect_err("missing repo");
        assert_eq!(error_code(&error), "github_repo_unavailable");
    }

    #[cfg(unix)]
    #[test]
    fn label_write_404_is_issues_failed() {
        let (_dir, path) = fake_gh_input(&json!([]), 1, "gh: HTTP 404");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let error =
            remove_github_issue_label_with(&gh, "https://github.com/acme/app", 42, "missing")
                .expect_err("missing label");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[cfg(unix)]
    #[test]
    fn assignee_remove_sends_login_in_json_body() {
        let output = projected_issue(42, "open");
        let (dir, path) = fake_gh_input(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        remove_github_issue_assignee_with(&gh, "https://github.com/acme/app", 42, "linus")
            .expect("unassign");
        let input: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
        )
        .expect("json");
        assert_eq!(input, json!({ "assignees": ["linus"] }));
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
        assert!(calls.contains("--method DELETE /repos/acme/app/issues/42/assignees"));
        assert!(!calls.contains("/assignees/linus"));
    }

    #[test]
    fn user_wrapper_maps_missing_discovered_cli() {
        let error = get_github_authenticated_user_with_runner(GhRunner::from_resolved(None))
            .expect_err("missing");
        assert_eq!(error_code(&error), "github_cli_missing");
    }

    #[test]
    fn catalog_and_mutation_wrappers_map_missing_discovered_cli() {
        let clone_url = "https://github.com/acme/app".to_string();
        let missing = || GhRunner::from_resolved(None);
        let errors = [
            list_github_repo_labels_with_runner(clone_url.clone(), missing()).expect_err("labels"),
            add_github_issue_labels_with_runner(clone_url.clone(), 42, "bug".into(), missing())
                .expect_err("add label"),
            remove_github_issue_label_with_runner(clone_url.clone(), 42, "bug".into(), missing())
                .expect_err("remove label"),
            list_github_repo_assignees_with_runner(clone_url.clone(), missing())
                .expect_err("assignees"),
            add_github_issue_assignees_with_runner(
                clone_url.clone(),
                42,
                "linus".into(),
                missing(),
            )
            .expect_err("add assignee"),
            remove_github_issue_assignee_with_runner(clone_url, 42, "linus".into(), missing())
                .expect_err("remove assignee"),
        ];
        for error in errors {
            assert_eq!(error_code(&error), "github_cli_missing");
        }
    }
}
