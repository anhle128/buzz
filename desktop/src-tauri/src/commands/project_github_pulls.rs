use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{redact_diagnostic, GhRunner, GitHubRepoRef};
use crate::commands::project_github_repository_state::combined_cli_diagnostic;
use chrono::DateTime;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::ffi::OsString;
use std::path::Path;
use url::Url;

const GH_PULL_STREAM_LIMIT: usize = 32 * 1024 * 1024;
const PULL_LIST_JQ: &str = "[.[] | {number, title, body: (.body // \"\"), html_url, draft, created_at, updated_at, comments, user: (if .user == null then null else {login: .user.login, avatar_url: (.user.avatar_url // \"\")} end), head: {ref: .head.ref, sha: .head.sha, repo: (if .head.repo == null then null else {full_name: .head.repo.full_name} end)}, base: {ref: .base.ref, repo: (if .base.repo == null then null else {full_name: .base.repo.full_name} end)}}]";

/// GitHub login identity returned to the desktop PR UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestUserDto {
    pub login: String,
    pub avatar_url: String,
}

/// GitHub repository identity returned to the desktop PR UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestRepoDto {
    pub full_name: String,
}

/// Bounded head metadata for one GitHub pull request.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestHeadDto {
    #[serde(rename = "ref")]
    pub branch: String,
    pub sha: String,
    pub repo: GitHubPullRequestRepoDto,
}

/// Bounded base metadata for one GitHub pull request.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestBaseDto {
    #[serde(rename = "ref")]
    pub branch: String,
    pub repo: GitHubPullRequestRepoDto,
}

/// One bounded GitHub pull request returned to the desktop UI.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestDto {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub html_url: String,
    pub draft: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub comments: u64,
    pub user: GitHubPullRequestUserDto,
    pub head: GitHubPullRequestHeadDto,
    pub base: GitHubPullRequestBaseDto,
}

/// One GitHub pull-request page plus its first-page truncation signal.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubPullRequestListDto {
    pub pulls: Vec<GitHubPullRequestDto>,
    pub has_more: bool,
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
    branch: String,
    sha: String,
    repo: Option<GitHubPullRequestRepoWire>,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestBaseWire {
    #[serde(rename = "ref")]
    branch: String,
    repo: Option<GitHubPullRequestRepoWire>,
}

#[derive(Debug, Deserialize)]
struct GitHubPullRequestWire {
    number: u64,
    title: String,
    body: String,
    html_url: String,
    draft: bool,
    created_at: String,
    updated_at: String,
    comments: u64,
    user: Option<GitHubPullRequestUserWire>,
    head: GitHubPullRequestHeadWire,
    base: GitHubPullRequestBaseWire,
}

/// Run `gh api` with optional JSON input and deserialize the jq-projected stdout.
pub(crate) fn github_pull_api_json<T: DeserializeOwned>(
    gh: &GhRunner,
    method: &str,
    path: &str,
    jq: &str,
    input: Option<&Path>,
    not_found_code: &str,
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
        .map_err(|error| remap_pulls_error(error, "", not_found_code))?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        return Err(remap_pulls_error(
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                redact_diagnostic(&diagnostic),
            ),
            &diagnostic,
            not_found_code,
        ));
    }
    serde_json::from_str(&output.stdout).map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_pulls_failed",
            "GitHub CLI returned an unexpected or truncated pull request response. Update gh, then retry.",
        )
    })
}

/// True when `raw` is a repo-bound GitHub pull request URL for `number`.
pub(crate) fn is_pull_html_url(repo: &GitHubRepoRef, raw: &str, number: u64) -> bool {
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

fn map_pull(repo: &GitHubRepoRef, value: serde_json::Value) -> Option<GitHubPullRequestDto> {
    let item = serde_json::from_value::<GitHubPullRequestWire>(value).ok()?;
    if item.number == 0 || item.title.trim().is_empty() {
        return None;
    }
    let user = item.user?;
    if user.login.trim().is_empty() {
        return None;
    }
    if item.head.branch.trim().is_empty()
        || item.head.sha.trim().is_empty()
        || item.base.branch.trim().is_empty()
    {
        return None;
    }
    let head_repo = item.head.repo?;
    let base_repo = item.base.repo?;
    if head_repo.full_name.trim().is_empty() || base_repo.full_name.trim().is_empty() {
        return None;
    }
    if !base_repo
        .full_name
        .eq_ignore_ascii_case(&repo.slug())
    {
        return None;
    }
    if !is_pull_html_url(repo, &item.html_url, item.number) {
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
        created_at,
        updated_at,
        comments: item.comments,
        user: GitHubPullRequestUserDto {
            login: user.login,
            avatar_url: user.avatar_url,
        },
        head: GitHubPullRequestHeadDto {
            branch: item.head.branch,
            sha: item.head.sha,
            repo: GitHubPullRequestRepoDto {
                full_name: head_repo.full_name,
            },
        },
        base: GitHubPullRequestBaseDto {
            branch: item.base.branch,
            repo: GitHubPullRequestRepoDto {
                full_name: base_repo.full_name,
            },
        },
    })
}

/// Remap GitHub CLI failures for pull-request operations.
pub(crate) fn remap_pulls_error(
    error: ProjectPullRequestMergeError,
    diagnostic: &str,
    not_found_code: &str,
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
    if lower.contains("403") && !lower.contains("rate") {
        return ProjectPullRequestMergeError::new("github_repo_unavailable", message);
    }
    if lower.contains("404") || lower.contains("not found") {
        return ProjectPullRequestMergeError::new(not_found_code, message);
    }
    ProjectPullRequestMergeError::new("github_pulls_failed", message)
}

/// List one bounded page of open GitHub pull requests with an injected runner.
pub(crate) fn list_github_pull_requests_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<GitHubPullRequestListDto, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_pulls_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_pulls_error(error, "", "github_repo_unavailable"))?;
    let path = format!(
        "/repos/{}/pulls?state=open&per_page=100&sort=updated&direction=desc",
        repo.slug(),
    );
    let raw: Vec<serde_json::Value> = github_pull_api_json(
        gh,
        "GET",
        &path,
        PULL_LIST_JQ,
        None,
        "github_repo_unavailable",
    )?;
    let has_more = raw.len() == 100;
    let pulls = raw
        .into_iter()
        .filter_map(|value| map_pull(&repo, value))
        .collect();
    Ok(GitHubPullRequestListDto { pulls, has_more })
}

#[cfg(test)]
mod tests {
    use super::*;
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
            "body": "Details",
            "html_url": format!("https://github.com/acme/app/pull/{number}"),
            "draft": draft,
            "created_at": "2026-01-02T03:04:05Z",
            "updated_at": "2026-01-03T03:04:05Z",
            "comments": 3,
            "user": { "login": "ada", "avatar_url": "https://avatars.githubusercontent.com/u/1" },
            "head": {
                "ref": "feature/readme",
                "sha": "1111111111111111111111111111111111111111",
                "repo": { "full_name": "acme/app" }
            },
            "base": {
                "ref": "main",
                "repo": { "full_name": "acme/app" }
            }
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
            output,
            stderr,
            status,
        );
        std::fs::write(&path, script).expect("write fake gh");
        let mut permissions = std::fs::metadata(&path).expect("stat fake gh").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("chmod fake gh");
        (dir, path)
    }

    #[cfg(unix)]
    #[test]
    fn list_maps_open_and_draft_pull_fields() {
        let output = json!([projected_pull(42, false), projected_pull(43, true)]);
        let (dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(
            &gh,
            "https://github.com/acme/app",
        )
        .expect("list");
        assert_eq!(page.pulls.len(), 2);
        assert_eq!(page.pulls[0].number, 42);
        assert!(!page.pulls[0].draft);
        assert!(page.pulls[1].draft);
        assert_eq!(page.pulls[0].user.login, "ada");
        assert_eq!(page.pulls[0].head.branch, "feature/readme");
        assert_eq!(page.pulls[0].head.repo.full_name, "acme/app");
        assert_eq!(page.pulls[0].base.branch, "main");
        assert_eq!(page.pulls[0].comments, 3);
        assert!(!page.has_more);
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("calls");
        assert!(calls.contains("/repos/acme/app/pulls?state=open&per_page=100&sort=updated&direction=desc"));
        assert!(calls.contains("--jq"));
        assert!(!calls.contains("--paginate"));
    }

    #[cfg(unix)]
    #[test]
    fn list_preserves_inbound_fork_metadata() {
        let mut item = projected_pull(42, false);
        item["head"]["repo"]["full_name"] = json!("fork-owner/app");
        let (_dir, path) = fake_gh(&json!([item]), 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "git@github.com:acme/app.git")
            .expect("list");
        assert_eq!(page.pulls[0].head.repo.full_name, "fork-owner/app");
        assert_eq!(page.pulls[0].head.sha, "1111111111111111111111111111111111111111");
    }

    #[cfg(unix)]
    #[test]
    fn raw_page_of_100_sets_has_more_before_invalid_rows_are_dropped() {
        let output = serde_json::Value::Array(
            (1..=100)
                .map(|number| {
                    let mut item = projected_pull(number, false);
                    item["html_url"] = json!(format!("https://evil.example/pull/{number}"));
                    item
                })
                .collect(),
        );
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app")
            .expect("list");
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
            ("https://evil.example/acme/app/pull/42", 42, false),
            ("https://user@github.com/acme/app/pull/42", 42, false),
            ("https://github.com/acme/app/pull/42?x=1", 42, false),
            ("https://github.com/acme/app/pull/42#x", 42, false),
            ("https://github.com/acme/app/pull/42/", 42, false),
        ] {
            assert_eq!(is_pull_html_url(&repo, raw, number), expected, "{raw}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn list_drops_a_pull_whose_base_repo_is_not_the_target() {
        let mut item = projected_pull(42, false);
        item["base"]["repo"]["full_name"] = json!("acme/other");
        let (_dir, path) = fake_gh(&json!([item]), 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app")
            .expect("list");
        assert!(page.pulls.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn list_drops_one_malformed_wire_row_without_losing_valid_rows() {
        let output = json!([
            { "number": "not-a-number", "title": 7 },
            projected_pull(42, false)
        ]);
        let (_dir, path) = fake_gh(&output, 0, "");
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let page = list_github_pull_requests_with(&gh, "https://github.com/acme/app")
            .expect("list");
        assert_eq!(page.pulls.len(), 1);
        assert_eq!(page.pulls[0].number, 42);
    }

    #[test]
    fn list_rejects_non_github_clone_url_before_running_gh() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false"))).expect("runner");
        let error = list_github_pull_requests_with(
            &gh,
            &format!("https://relay.example/git/{}/app", "ab".repeat(32)),
        )
        .expect_err("reject Buzz URL");
        assert_eq!(error_code(&error), "github_pulls_failed");
    }
}
