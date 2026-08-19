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
    fn fake_gh(
        output: &serde_json::Value,
        status: i32,
        stderr: &str,
    ) -> (tempfile::TempDir, PathBuf) {
        fake_gh_raw(&output.to_string(), status, stderr)
    }

    #[cfg(unix)]
    #[test]
    fn maps_projected_open_and_draft_pull_fields() {
        let output = json!([projected_pull(42, false), projected_pull(7, true)]);
        let (dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page =
            list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect("list");
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
        assert!(calls
            .contains("/repos/acme/app/pulls?state=open&per_page=100&sort=updated&direction=desc"));
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
        let page =
            list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect("list");
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
        let page =
            list_github_pull_requests_with(&gh, "https://github.com/acme/app").expect("list");
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
        let gh =
            GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
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
        let gh =
            GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
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
        let gh =
            GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
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
        let gh =
            GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
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
        let gh =
            GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false"))).expect("runner");
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
        let comments =
            list_github_pull_request_comments_with(&gh, "https://github.com/acme/app", 42)
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
}
