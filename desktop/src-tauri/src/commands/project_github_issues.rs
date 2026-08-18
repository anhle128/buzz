use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{redact_diagnostic, GhRunner, GitHubRepoRef};
use crate::commands::project_github_repository_state::combined_cli_diagnostic;
use chrono::DateTime;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::ffi::OsString;
use std::path::Path;
use url::Url;

const GH_ISSUE_STREAM_LIMIT: usize = 32 * 1024 * 1024;
const ISSUE_LIST_JQ: &str = "[.[] | {number, title, body: (.body // \"\"), state, html_url, comments, created_at, updated_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), labels: [(.labels // [])[] | if type == \"string\" then . else .name end], assignees: [(.assignees // [])[] | {login, avatar_url: (.avatar_url // \"\")}], has_pull_request: has(\"pull_request\")}]";
const ISSUE_ITEM_JQ: &str = "{number, title, body: (.body // \"\"), state, html_url, comments, created_at, updated_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), labels: [(.labels // [])[] | if type == \"string\" then . else .name end], assignees: [(.assignees // [])[] | {login, avatar_url: (.avatar_url // \"\")}], has_pull_request: has(\"pull_request\")}";
const ISSUE_COMMENTS_JQ: &str = "[.[] | {id, body: (.body // \"\"), created_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end)}]";

/// GitHub login identity returned to the desktop issue UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubIssueUserDto {
    pub login: String,
    pub avatar_url: String,
}

/// Bounded GitHub issue returned to the desktop issue UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubIssueDto {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub state: String,
    pub html_url: String,
    pub comments: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub user: GitHubIssueUserDto,
    pub labels: Vec<String>,
    pub assignees: Vec<GitHubIssueUserDto>,
}

/// One bounded GitHub issue page plus its first-page truncation signal.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubIssueListDto {
    pub issues: Vec<GitHubIssueDto>,
    pub has_more: bool,
}

/// One read-only GitHub issue comment returned to the desktop UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubIssueCommentDto {
    pub id: u64,
    pub body: String,
    pub created_at: i64,
    pub user: GitHubIssueUserDto,
}

#[derive(Debug, Deserialize)]
struct GitHubIssueUserWire {
    login: String,
    avatar_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubIssueWire {
    number: u64,
    title: String,
    body: String,
    state: String,
    html_url: String,
    comments: u64,
    created_at: String,
    updated_at: String,
    user: Option<GitHubIssueUserWire>,
    labels: Vec<String>,
    assignees: Vec<GitHubIssueUserWire>,
    has_pull_request: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubIssueCommentWire {
    id: u64,
    body: String,
    created_at: String,
    user: Option<GitHubIssueUserWire>,
}

fn github_api_json<T: DeserializeOwned>(
    gh: &GhRunner,
    method: &str,
    path: &str,
    jq: &str,
    input: Option<&Path>,
) -> Result<T, ProjectPullRequestMergeError> {
    let mut args = vec![
        OsString::from("api"),
        OsString::from("--hostname"),
        OsString::from("github.com"),
        OsString::from("--method"),
        OsString::from(method),
        OsString::from(path),
        OsString::from("--jq"),
        OsString::from(jq),
    ];
    if let Some(input) = input {
        args.push(OsString::from("--input"));
        args.push(input.as_os_str().to_os_string());
    }
    let output = gh
        .run_with_limit(&args, GH_ISSUE_STREAM_LIMIT)
        .map_err(|error| remap_issues_error(error, ""))?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        return Err(remap_issues_error(
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                redact_diagnostic(&diagnostic),
            ),
            &diagnostic,
        ));
    }
    serde_json::from_str(&output.stdout).map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub CLI returned an unexpected or truncated issue response. Update gh, then retry.",
        )
    })
}

pub(crate) fn is_issue_html_url(repo: &GitHubRepoRef, raw: &str, number: u64) -> bool {
    if number == 0 || raw != raw.trim() || raw.contains('\\') || raw.contains('%') {
        return false;
    }
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    let Some(segments) = url
        .path_segments()
        .map(|segments| segments.collect::<Vec<_>>())
    else {
        return false;
    };
    segments.len() == 4
        && segments[0].eq_ignore_ascii_case(&repo.owner)
        && segments[1].eq_ignore_ascii_case(&repo.repo)
        && segments[2] == "issues"
        && segments[3].parse::<u64>().ok() == Some(number)
}

fn map_issue(repo: &GitHubRepoRef, item: GitHubIssueWire) -> Option<GitHubIssueDto> {
    if item.has_pull_request
        || item.number == 0
        || !matches!(item.state.as_str(), "open" | "closed")
    {
        return None;
    }
    let user = item.user?;
    if user.login.trim().is_empty() || !is_issue_html_url(repo, &item.html_url, item.number) {
        return None;
    }
    let created_at = DateTime::parse_from_rfc3339(&item.created_at)
        .ok()?
        .timestamp();
    let updated_at = DateTime::parse_from_rfc3339(&item.updated_at)
        .ok()?
        .timestamp();
    let assignees = item
        .assignees
        .into_iter()
        .filter(|assignee| !assignee.login.trim().is_empty())
        .map(|assignee| GitHubIssueUserDto {
            login: assignee.login,
            avatar_url: assignee.avatar_url,
        })
        .collect();
    Some(GitHubIssueDto {
        number: item.number,
        title: item.title,
        body: item.body,
        state: item.state,
        html_url: item.html_url,
        comments: item.comments,
        created_at,
        updated_at,
        user: GitHubIssueUserDto {
            login: user.login,
            avatar_url: user.avatar_url,
        },
        labels: item.labels,
        assignees,
    })
}

pub(crate) fn remap_issues_error(
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
    if lower.contains("404")
        || lower.contains("not found")
        || (lower.contains("403") && !lower.contains("rate"))
    {
        return ProjectPullRequestMergeError::new("github_repo_unavailable", message);
    }
    ProjectPullRequestMergeError::new("github_issues_failed", message)
}

pub(crate) fn list_github_issues_with(
    gh: &GhRunner,
    clone_url: &str,
    state: &str,
) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError> {
    if !matches!(state, "open" | "closed") {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub issue list state must be open or closed.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_issues_error(error, ""))?;
    let path = format!(
        "/repos/{}/issues?state={state}&per_page=100&sort=updated&direction=desc",
        repo.slug(),
    );
    let raw: Vec<GitHubIssueWire> = github_api_json(gh, "GET", &path, ISSUE_LIST_JQ, None)?;
    let has_more = raw.len() == 100;
    let issues = raw
        .into_iter()
        .filter_map(|item| map_issue(&repo, item))
        .collect();
    Ok(GitHubIssueListDto { issues, has_more })
}

fn issue_json_input(
    title: &str,
    body: &str,
) -> Result<tempfile::NamedTempFile, ProjectPullRequestMergeError> {
    use std::io::Write;
    let mut file = tempfile::Builder::new()
        .prefix("buzz-gh-")
        .tempfile()
        .map_err(|error| {
            ProjectPullRequestMergeError::new("github_issues_failed", error.to_string())
        })?;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({ "title": title, "body": body }),
    )
    .map_err(|error| {
        ProjectPullRequestMergeError::new("github_issues_failed", error.to_string())
    })?;
    file.flush().map_err(|error| {
        ProjectPullRequestMergeError::new("github_issues_failed", error.to_string())
    })?;
    Ok(file)
}

pub(crate) fn create_github_issue_with(
    gh: &GhRunner,
    clone_url: &str,
    title: &str,
    body: &str,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "Issue title is required.",
        ));
    }
    if title.chars().count() > 256 {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "Issue title must be 256 characters or fewer.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_issues_error(error, ""))?;
    let input = issue_json_input(title, body)?;
    let path = format!("/repos/{}/issues", repo.slug());
    let raw: GitHubIssueWire =
        github_api_json(gh, "POST", &path, ISSUE_ITEM_JQ, Some(input.path()))?;
    map_issue(&repo, raw).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub returned an invalid created issue.",
        )
    })
}

pub(crate) fn list_github_issue_comments_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
) -> Result<Vec<GitHubIssueCommentDto>, ProjectPullRequestMergeError> {
    if number == 0 {
        return Err(ProjectPullRequestMergeError::new(
            "github_issues_failed",
            "GitHub issue number must be greater than zero.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_issues_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_issues_error(error, ""))?;
    let path = format!(
        "/repos/{}/issues/{number}/comments?per_page=100",
        repo.slug()
    );
    let raw: Vec<GitHubIssueCommentWire> =
        github_api_json(gh, "GET", &path, ISSUE_COMMENTS_JQ, None)?;
    Ok(raw
        .into_iter()
        .filter_map(|comment| {
            let user = comment.user?;
            if comment.id == 0 || user.login.trim().is_empty() {
                return None;
            }
            let created_at = DateTime::parse_from_rfc3339(&comment.created_at)
                .ok()?
                .timestamp();
            Some(GitHubIssueCommentDto {
                id: comment.id,
                body: comment.body,
                created_at,
                user: GitHubIssueUserDto {
                    login: user.login,
                    avatar_url: user.avatar_url,
                },
            })
        })
        .collect())
}

pub(crate) fn list_github_issues_with_runner(
    clone_url: String,
    state: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issues_error(error, ""))?;
    list_github_issues_with(&gh, &clone_url, &state)
}

pub(crate) fn create_github_issue_with_runner(
    clone_url: String,
    title: String,
    body: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issues_error(error, ""))?;
    create_github_issue_with(&gh, &clone_url, &title, &body)
}

pub(crate) fn list_github_issue_comments_with_runner(
    clone_url: String,
    number: u64,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<Vec<GitHubIssueCommentDto>, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_issues_error(error, ""))?;
    list_github_issue_comments_with(&gh, &clone_url, number)
}

/// List one page of GitHub issues for a github.com clone URL.
#[tauri::command]
pub async fn list_github_issues(
    clone_url: String,
    state: String,
) -> Result<GitHubIssueListDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_issues_with_runner(clone_url, state, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// Create one GitHub issue for a github.com clone URL.
#[tauri::command]
pub async fn create_github_issue(
    clone_url: String,
    title: String,
    body: String,
) -> Result<GitHubIssueDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        create_github_issue_with_runner(clone_url, title, body, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

/// List the first page of comments for one GitHub issue.
#[tauri::command]
pub async fn list_github_issue_comments(
    clone_url: String,
    number: u64,
) -> Result<Vec<GitHubIssueCommentDto>, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_issue_comments_with_runner(clone_url, number, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_issues_failed", error.to_string()))?
}

#[cfg(test)]
mod tests {
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

    fn projected_issue(number: u64, has_pull_request: bool) -> serde_json::Value {
        json!({
            "number": number,
            "title": format!("Issue {number}"),
            "body": "Steps",
            "state": "open",
            "html_url": format!("https://github.com/acme/app/issues/{number}"),
            "comments": 3,
            "created_at": "2026-01-02T03:04:05Z",
            "updated_at": "2026-01-03T03:04:05Z",
            "user": { "login": "ada", "avatar_url": "https://avatars.githubusercontent.com/u/1" },
            "labels": ["bug"],
            "assignees": [{ "login": "linus", "avatar_url": "https://avatars.githubusercontent.com/u/2" }],
            "has_pull_request": has_pull_request,
        })
    }

    #[cfg(unix)]
    fn fake_gh(
        output: &serde_json::Value,
        status: i32,
        stderr: &str,
    ) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("create fake gh directory");
        let path = dir.path().join("gh");
        let script = format!(
            "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\n  *) printf '%s' '{}' ; printf '%s' '{}' >&2; exit {} ;;\nesac\n",
            output.to_string(),
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
    #[test]
    fn maps_projected_open_issue_fields() {
        let output = json!([projected_issue(42, false)]);
        let (dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page =
            list_github_issues_with(&gh, "https://github.com/acme/app", "open").expect("list");
        assert_eq!(page.issues.len(), 1);
        assert_eq!(page.issues[0].number, 42);
        assert_eq!(page.issues[0].state, "open");
        assert_eq!(page.issues[0].user.login, "ada");
        assert_eq!(page.issues[0].labels, vec!["bug"]);
        assert_eq!(page.issues[0].assignees[0].login, "linus");
        assert!(!page.has_more);
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
        assert!(calls.contains(
            "/repos/acme/app/issues?state=open&per_page=100&sort=updated&direction=desc"
        ));
        assert!(calls.contains("--jq"));
        assert!(calls.contains("has_pull_request"));
    }

    #[cfg(unix)]
    #[test]
    fn drops_projected_pull_request_items() {
        let output = json!([projected_issue(7, true), projected_issue(42, false)]);
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page =
            list_github_issues_with(&gh, "https://github.com/acme/app", "open").expect("list");
        assert_eq!(
            page.issues
                .iter()
                .map(|issue| issue.number)
                .collect::<Vec<_>>(),
            vec![42]
        );
    }

    #[cfg(unix)]
    #[test]
    fn raw_projected_page_of_100_sets_has_more_before_filtering() {
        let output = serde_json::Value::Array(
            (1..=100)
                .map(|number| projected_issue(number, true))
                .collect(),
        );
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page =
            list_github_issues_with(&gh, "https://github.com/acme/app", "open").expect("list");
        assert!(page.issues.is_empty());
        assert!(page.has_more);
    }

    #[test]
    fn accepts_only_repo_bound_issue_urls() {
        let repo = GitHubRepoRef::parse("https://github.com/Acme/App").expect("repo");
        for (raw, number, expected) in [
            ("https://github.com/acme/app/issues/42", 42, true),
            ("https://github.com/ACME/APP/issues/42", 42, true),
            ("https://github.com/acme/app/issues/43", 42, false),
            ("https://github.com/acme/other/issues/42", 42, false),
            ("https://evil.example/acme/app/issues/42", 42, false),
            ("https://user@github.com/acme/app/issues/42", 42, false),
            ("https://github.com/acme/app/issues/42?x=1", 42, false),
            ("https://github.com/acme/app/issues/42#x", 42, false),
            ("https://github.com/acme/app/issues/42/", 42, false),
        ] {
            assert_eq!(is_issue_html_url(&repo, raw, number), expected, "{raw}");
        }
    }

    #[test]
    fn rejects_non_github_clone_url_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = list_github_issues_with(
            &gh,
            &format!("https://relay.example/git/{}/app", "ab".repeat(32)),
            "open",
        )
        .expect_err("reject Buzz URL");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[test]
    fn rejects_state_all_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = list_github_issues_with(&gh, "https://github.com/acme/app", "all")
            .expect_err("reject all");
        assert_eq!(error_code(&error), "github_issues_failed");
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
            list_github_issues_with(&gh, "https://github.com/acme/app", "open").expect_err("auth");
        assert_eq!(error_code(&error), "github_auth_required");
    }

    #[test]
    fn remaps_repository_and_rate_failures_to_issue_codes() {
        let not_found = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 404");
        assert_eq!(
            error_code(&remap_issues_error(not_found, "Not Found")),
            "github_repo_unavailable"
        );
        let forbidden = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
        assert_eq!(
            error_code(&remap_issues_error(forbidden, "Forbidden")),
            "github_repo_unavailable"
        );
        let limited = ProjectPullRequestMergeError::new("github_merge_failed", "gh: HTTP 403");
        assert_eq!(
            error_code(&remap_issues_error(limited, "API rate limit exceeded")),
            "github_issues_failed"
        );
    }

    #[test]
    fn create_rejects_blank_title_before_running_gh() {
        let gh =
            GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
        let error = create_github_issue_with(&gh, "https://github.com/acme/app", "   ", "body")
            .expect_err("blank");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[test]
    fn create_rejects_more_than_256_unicode_scalars_before_running_gh() {
        let gh =
            GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
        let title = "é".repeat(257);
        let error = create_github_issue_with(&gh, "https://github.com/acme/app", &title, "")
            .expect_err("long");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[cfg(unix)]
    #[test]
    fn create_posts_trimmed_title_and_exact_body() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("gh");
        let output = projected_issue(43, false).to_string();
        let script = format!(
            "#!/bin/sh\nset -eu\nroot=${{0%/gh}}\nprintf '%s\\n' \"$*\" >> \"$root/calls\"\ncase \"$*\" in\n  *auth*status*) exit 0 ;;\n  *--method*POST*)\n    previous=\"\"\n    for argument in \"$@\"; do\n      if [ \"$previous\" = \"--input\" ]; then cp \"$argument\" \"$root/input.json\"; fi\n      previous=\"$argument\"\n    done\n    printf '%s' '{}'\n    ;;\n  *) exit 1 ;;\nesac\n",
            output,
        );
        std::fs::write(&path, script).expect("write fake gh");
        let mut permissions = std::fs::metadata(&path).expect("stat").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("chmod");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let issue = create_github_issue_with(
            &gh,
            "https://github.com/acme/app",
            "  Issue 43  ",
            " body with surrounding space ",
        )
        .expect("create");
        assert_eq!(issue.number, 43);
        let input: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("input.json")).expect("input"),
        )
        .expect("json");
        assert_eq!(
            input,
            serde_json::json!({
                "title": "Issue 43",
                "body": " body with surrounding space ",
            })
        );
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
        assert!(calls.contains("--method POST"));
        assert!(calls.contains("/repos/acme/app/issues"));
        assert!(calls.contains("--input"));
        assert!(calls.contains("--jq"));
    }

    #[cfg(unix)]
    #[test]
    fn create_rejects_response_with_foreign_html_url() {
        let mut output = projected_issue(43, false);
        output["html_url"] =
            serde_json::Value::String("https://evil.example/issues/43".to_string());
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let error = create_github_issue_with(&gh, "https://github.com/acme/app", "Issue 43", "")
            .expect_err("foreign URL");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[test]
    fn comments_reject_number_zero_before_running_gh() {
        let gh =
            GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
        let error = list_github_issue_comments_with(&gh, "https://github.com/acme/app", 0)
            .expect_err("zero");
        assert_eq!(error_code(&error), "github_issues_failed");
    }

    #[cfg(unix)]
    #[test]
    fn comments_keep_projected_github_order() {
        let output = serde_json::json!([
            { "id": 1, "body": "first", "created_at": "2026-01-02T03:04:05Z", "user": { "login": "ada", "avatar_url": "https://example.com/a" } },
            { "id": 2, "body": "second", "created_at": "2026-01-02T04:04:05Z", "user": { "login": "linus", "avatar_url": "https://example.com/b" } }
        ]);
        let (dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let comments = list_github_issue_comments_with(&gh, "https://github.com/acme/app", 42)
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
    }

    #[test]
    fn list_wrapper_maps_missing_discovered_cli() {
        let error = list_github_issues_with_runner(
            "https://github.com/acme/app".to_string(),
            "open".to_string(),
            GhRunner::from_resolved(None),
        )
        .expect_err("missing");
        assert_eq!(error_code(&error), "github_cli_missing");
    }

    #[test]
    fn create_wrapper_maps_missing_discovered_cli() {
        let error = create_github_issue_with_runner(
            "https://github.com/acme/app".to_string(),
            "title".to_string(),
            "body".to_string(),
            GhRunner::from_resolved(None),
        )
        .expect_err("missing");
        assert_eq!(error_code(&error), "github_cli_missing");
    }

    #[test]
    fn comments_wrapper_maps_missing_discovered_cli() {
        let error = list_github_issue_comments_with_runner(
            "https://github.com/acme/app".to_string(),
            42,
            GhRunner::from_resolved(None),
        )
        .expect_err("missing");
        assert_eq!(error_code(&error), "github_cli_missing");
    }
}
