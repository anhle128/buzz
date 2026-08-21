use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{redact_diagnostic, GhRunner, GitHubRepoRef};
use crate::commands::project_github_repository_state::combined_cli_diagnostic;
use chrono::DateTime;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::ffi::OsString;
use std::path::Path;
use url::Url;

const GH_PULL_STREAM_LIMIT: usize = 32 * 1024 * 1024;
const PULL_LIST_JQ: &str = "[.[] | {number, title, body: (.body // \"\"), html_url, draft: (.draft // false), comments, created_at, updated_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), head: {ref: .head.ref, sha: .head.sha, repo: {full_name: (.head.repo.full_name // \"\")}}, base: {ref: .base.ref, repo: {full_name: (.base.repo.full_name // \"\")}}}]";
const PULL_ITEM_JQ: &str = "{number, title, body: (.body // \"\"), html_url, draft: (.draft // false), comments, created_at, updated_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), head: {ref: .head.ref, sha: .head.sha, repo: {full_name: (.head.repo.full_name // \"\")}}, base: {ref: .base.ref, repo: {full_name: (.base.repo.full_name // \"\")}}}";
const PULL_COMMENTS_JQ: &str = "[.[] | {id, body: (.body // \"\"), html_url, created_at, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end)}]";

/// GitHub login identity returned to the desktop pull-request UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestUserDto {
    pub login: String,
    pub avatar_url: String,
}

/// Nested repository identity on a GitHub pull-request head or base.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestRepoDto {
    pub full_name: String,
}

/// Head ref returned to the desktop pull-request UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestHeadDto {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub sha: String,
    pub repo: GitHubPullRequestRepoDto,
}

/// Base ref returned to the desktop pull-request UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestBaseDto {
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub repo: GitHubPullRequestRepoDto,
}

/// Bounded GitHub pull request returned to the desktop UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestDto {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub html_url: String,
    pub draft: bool,
    pub comments: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub user: GitHubPullRequestUserDto,
    pub head: GitHubPullRequestHeadDto,
    pub base: GitHubPullRequestBaseDto,
}

/// One bounded GitHub pull-request page plus its first-page truncation signal.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestListDto {
    pub pulls: Vec<GitHubPullRequestDto>,
    pub has_more: bool,
}

/// One read-only GitHub pull-request conversation comment.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestCommentDto {
    pub id: u64,
    pub body: String,
    pub html_url: String,
    pub created_at: i64,
    pub user: GitHubPullRequestUserDto,
}

#[derive(Clone, Copy)]
enum PullsNotFound {
    Repo,
    PullRequest,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestUserWire {
    login: String,
    avatar_url: String,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestRepoWire {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestHeadWire {
    #[serde(rename = "ref")]
    git_ref: String,
    sha: String,
    repo: GitHubPullRequestRepoWire,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestBaseWire {
    #[serde(rename = "ref")]
    git_ref: String,
    repo: GitHubPullRequestRepoWire,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestWire {
    number: u64,
    title: String,
    body: String,
    html_url: String,
    draft: bool,
    comments: u64,
    created_at: String,
    updated_at: String,
    user: Option<GitHubPullRequestUserWire>,
    head: GitHubPullRequestHeadWire,
    base: GitHubPullRequestBaseWire,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestCommentWire {
    id: u64,
    body: String,
    html_url: String,
    created_at: String,
    user: Option<GitHubPullRequestUserWire>,
}

fn github_api_json<T: DeserializeOwned>(
    gh: &GhRunner,
    method: &str,
    path: &str,
    jq: &str,
    input: Option<&Path>,
    not_found: PullsNotFound,
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
        .run_with_limit(&args, GH_PULL_STREAM_LIMIT)
        .map_err(|error| remap_pulls_error(error, "", not_found))?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        return Err(remap_pulls_error(
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                redact_diagnostic(&diagnostic),
            ),
            &diagnostic,
            not_found,
        ));
    }
    serde_json::from_str(&output.stdout).map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "GitHub CLI returned an unexpected or truncated pull request response. Update gh, then retry.",
        )
    })
}

fn is_pull_html_url(repo: &GitHubRepoRef, raw: &str, number: u64) -> bool {
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
        && segments[2] == "pull"
        && segments[3].parse::<u64>().ok() == Some(number)
}

fn is_pull_comment_html_url(repo: &GitHubRepoRef, raw: &str, number: u64, comment_id: u64) -> bool {
    if number == 0
        || comment_id == 0
        || raw != raw.trim()
        || raw.contains('\\')
        || raw.contains('%')
    {
        return false;
    }
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    let expected_fragment = format!("issuecomment-{comment_id}");
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment() != Some(expected_fragment.as_str())
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
        && matches!(segments[2], "issues" | "pull")
        && segments[3].parse::<u64>().ok() == Some(number)
}

fn map_pull(repo: &GitHubRepoRef, item: GitHubPullRequestWire) -> Option<GitHubPullRequestDto> {
    let user = item.user?;
    if item.number == 0
        || user.login.trim().is_empty()
        || item.head.git_ref.trim().is_empty()
        || item.base.git_ref.trim().is_empty()
        || item.head.sha.trim().is_empty()
        || !is_pull_html_url(repo, &item.html_url, item.number)
    {
        return None;
    }
    let created_at = DateTime::parse_from_rfc3339(&item.created_at)
        .ok()?
        .timestamp();
    let updated_at = DateTime::parse_from_rfc3339(&item.updated_at)
        .ok()?
        .timestamp();
    Some(GitHubPullRequestDto {
        number: item.number,
        title: item.title,
        body: item.body,
        html_url: item.html_url,
        draft: item.draft,
        comments: item.comments,
        created_at,
        updated_at,
        user: GitHubPullRequestUserDto {
            login: user.login,
            avatar_url: user.avatar_url,
        },
        head: GitHubPullRequestHeadDto {
            git_ref: item.head.git_ref,
            sha: item.head.sha,
            repo: GitHubPullRequestRepoDto {
                full_name: item.head.repo.full_name,
            },
        },
        base: GitHubPullRequestBaseDto {
            git_ref: item.base.git_ref,
            repo: GitHubPullRequestRepoDto {
                full_name: item.base.repo.full_name,
            },
        },
    })
}

fn remap_pulls_error(
    error: ProjectPullRequestMergeError,
    diagnostic: &str,
    not_found: PullsNotFound,
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
            "GitHub pull request request failed.".to_string()
        } else {
            original.to_string()
        }
    } else {
        redact_diagnostic(diagnostic)
    };
    if lower.contains("rate limit") || lower.contains("abuse") {
        return ProjectPullRequestMergeError::new("github_pulls_failed", message);
    }
    if lower.contains("404")
        || lower.contains("not found")
        || (lower.contains("403") && !lower.contains("rate"))
    {
        let code = match not_found {
            PullsNotFound::Repo => "github_repo_unavailable",
            PullsNotFound::PullRequest => "github_pr_unavailable",
        };
        return ProjectPullRequestMergeError::new(code, message);
    }
    ProjectPullRequestMergeError::new("github_pulls_failed", message)
}

fn list_github_pull_requests_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<GitHubPullRequestListDto, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_pulls_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_pulls_error(error, "", PullsNotFound::Repo))?;
    let path = format!(
        "/repos/{}/pulls?state=open&per_page=100&sort=updated&direction=desc",
        repo.slug(),
    );
    let raw: Vec<GitHubPullRequestWire> =
        github_api_json(gh, "GET", &path, PULL_LIST_JQ, None, PullsNotFound::Repo)?;
    let has_more = raw.len() == 100;
    let pulls = raw
        .into_iter()
        .filter_map(|item| map_pull(&repo, item))
        .collect();
    Ok(GitHubPullRequestListDto { pulls, has_more })
}

fn pull_json_input(
    title: &str,
    body: &str,
    head: &str,
    base: &str,
) -> Result<tempfile::NamedTempFile, ProjectPullRequestMergeError> {
    use std::io::Write;
    let mut file = tempfile::Builder::new()
        .prefix("buzz-gh-")
        .tempfile()
        .map_err(|error| {
            ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string())
        })?;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({ "title": title, "body": body, "head": head, "base": base }),
    )
    .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?;
    file.flush().map_err(|error| {
        ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string())
    })?;
    Ok(file)
}

fn create_github_pull_request_with(
    gh: &GhRunner,
    clone_url: &str,
    title: &str,
    body: &str,
    head: &str,
    base: &str,
) -> Result<GitHubPullRequestDto, ProjectPullRequestMergeError> {
    let title = title.trim();
    let head = head.trim();
    let base = base.trim();
    if title.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "Pull request title is required.",
        ));
    }
    if title.chars().count() > 256 {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "Pull request title must be 256 characters or fewer.",
        ));
    }
    if head.is_empty() || base.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "Pull request branches are required.",
        ));
    }
    if head.contains(':') {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "The compare branch must belong to this repository.",
        ));
    }
    if head == base {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "The base and compare branches must be different.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_pulls_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_pulls_error(error, "", PullsNotFound::Repo))?;
    let input = pull_json_input(title, body, head, base)?;
    let path = format!("/repos/{}/pulls", repo.slug());
    let raw: GitHubPullRequestWire = github_api_json(
        gh,
        "POST",
        &path,
        PULL_ITEM_JQ,
        Some(input.path()),
        PullsNotFound::Repo,
    )?;
    map_pull(&repo, raw).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "GitHub returned an invalid created pull request.",
        )
    })
}

fn list_github_pull_request_comments_with(
    gh: &GhRunner,
    clone_url: &str,
    number: u64,
) -> Result<Vec<GitHubPullRequestCommentDto>, ProjectPullRequestMergeError> {
    if number == 0 {
        return Err(ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "GitHub pull request number must be greater than zero.",
        ));
    }
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_pulls_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_pulls_error(error, "", PullsNotFound::PullRequest))?;
    let path = format!(
        "/repos/{}/issues/{number}/comments?per_page=100",
        repo.slug()
    );
    let raw: Vec<GitHubPullRequestCommentWire> = github_api_json(
        gh,
        "GET",
        &path,
        PULL_COMMENTS_JQ,
        None,
        PullsNotFound::PullRequest,
    )?;
    Ok(raw
        .into_iter()
        .filter_map(|comment| {
            let user = comment.user?;
            if comment.id == 0
                || user.login.trim().is_empty()
                || !is_pull_comment_html_url(&repo, &comment.html_url, number, comment.id)
            {
                return None;
            }
            let created_at = DateTime::parse_from_rfc3339(&comment.created_at)
                .ok()?
                .timestamp();
            Some(GitHubPullRequestCommentDto {
                id: comment.id,
                body: comment.body,
                html_url: comment.html_url,
                created_at,
                user: GitHubPullRequestUserDto {
                    login: user.login,
                    avatar_url: user.avatar_url,
                },
            })
        })
        .collect())
}

fn list_github_pull_requests_with_runner(
    clone_url: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubPullRequestListDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_pulls_error(error, "", PullsNotFound::Repo))?;
    list_github_pull_requests_with(&gh, &clone_url)
}

fn create_github_pull_request_with_runner(
    clone_url: String,
    title: String,
    body: String,
    head: String,
    base: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubPullRequestDto, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_pulls_error(error, "", PullsNotFound::Repo))?;
    create_github_pull_request_with(&gh, &clone_url, &title, &body, &head, &base)
}

fn list_github_pull_request_comments_with_runner(
    clone_url: String,
    number: u64,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<Vec<GitHubPullRequestCommentDto>, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_pulls_error(error, "", PullsNotFound::PullRequest))?;
    list_github_pull_request_comments_with(&gh, &clone_url, number)
}

/// List one page of open GitHub pull requests for a github.com clone URL.
#[tauri::command]
pub async fn list_github_pull_requests(
    clone_url: String,
) -> Result<GitHubPullRequestListDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_pull_requests_with_runner(clone_url, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?
}

/// Create one ready GitHub pull request for a github.com clone URL.
#[tauri::command]
pub async fn create_github_pull_request(
    clone_url: String,
    title: String,
    body: String,
    head: String,
    base: String,
) -> Result<GitHubPullRequestDto, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        create_github_pull_request_with_runner(
            clone_url,
            title,
            body,
            head,
            base,
            GhRunner::discover(),
        )
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?
}

/// List the first page of conversation comments for one GitHub pull request.
#[tauri::command]
pub async fn list_github_pull_request_comments(
    clone_url: String,
    number: u64,
) -> Result<Vec<GitHubPullRequestCommentDto>, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        list_github_pull_request_comments_with_runner(clone_url, number, GhRunner::discover())
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_pulls_failed", error.to_string()))?
}

#[cfg(test)]
mod tests;
