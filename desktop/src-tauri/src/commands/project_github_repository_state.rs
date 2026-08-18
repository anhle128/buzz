//! Map GitHub `gh api` payloads into repository branch state.

use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{redact_diagnostic, GhRunner, GitHubRepoRef};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::time::{SystemTime, UNIX_EPOCH};

/// GitHub branch or tag tip returned to the desktop Projects picker.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubRepositoryBranch {
    pub name: String,
    pub commit: String,
}

/// Repository HEAD + branch list loaded from GitHub via `gh api`.
#[derive(Clone, Debug, Serialize)]
pub struct GitHubRepositoryState {
    pub head: String,
    pub branches: Vec<GitHubRepositoryBranch>,
    pub tags: Vec<GitHubRepositoryBranch>,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
struct GitHubRepoPayload {
    default_branch: String,
}

#[derive(Debug, Deserialize)]
struct GitHubBranchPayload {
    name: String,
    commit: GitHubBranchCommit,
}

#[derive(Debug, Deserialize)]
struct GitHubBranchCommit {
    sha: String,
}

/// Load default branch and branch tips for a GitHub clone URL using an injected runner.
pub(crate) fn github_repository_state_with(
    gh: &GhRunner,
    clone_url: &str,
) -> Result<GitHubRepositoryState, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_state_failed", message))?;
    gh.ensure_auth()
        .map_err(|error| remap_state_error(error, ""))?;
    let repo_json: GitHubRepoPayload = get_json(gh, &format!("/repos/{}", repo.slug()))?;
    let head = repo_json.default_branch.trim().to_string();
    if head.is_empty() {
        return Err(ProjectPullRequestMergeError::new(
            "github_state_failed",
            "GitHub did not return a default branch.",
        ));
    }
    let branches = list_branch_pages(gh, &repo.slug())?;
    Ok(GitHubRepositoryState {
        head,
        branches,
        tags: vec![],
        updated_at: unix_now(),
    })
}

pub(crate) fn get_github_repository_state_with_runner(
    clone_url: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GitHubRepositoryState, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_state_error(error, ""))?;
    github_repository_state_with(&gh, &clone_url)
}

#[tauri::command]
pub async fn get_github_repository_state(
    clone_url: String,
) -> Result<GitHubRepositoryState, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        get_github_repository_state_with_runner(clone_url, GhRunner::discover())
    })
    .await
    .map_err(|error| {
        ProjectPullRequestMergeError::new("github_state_failed", error.to_string())
    })?
}

fn get_json<T: serde::de::DeserializeOwned>(
    gh: &GhRunner,
    path: &str,
) -> Result<T, ProjectPullRequestMergeError> {
    let output = gh
        .run(&[
            OsString::from("api"),
            OsString::from("--hostname"),
            OsString::from("github.com"),
            OsString::from("--method"),
            OsString::from("GET"),
            OsString::from(path),
        ])
        .map_err(|error| remap_state_error(error, ""))?;
    if !output.status.success() {
        // `gh api` often puts the JSON error body on stdout and a short line on
        // stderr (e.g. "gh: HTTP 403"). Classify from both streams.
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        let redacted = redact_diagnostic(&diagnostic);
        return Err(remap_state_error(
            ProjectPullRequestMergeError::new("github_merge_failed", redacted),
            &diagnostic,
        ));
    }
    serde_json::from_str(&output.stdout).map_err(|_| {
        remap_state_error(
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "GitHub CLI returned an unexpected JSON response. Update gh, then retry.",
            ),
            &output.stderr,
        )
    })
}

fn combined_cli_diagnostic(stderr: &str, stdout: &str) -> String {
    match (stderr.trim().is_empty(), stdout.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => stderr.to_string(),
        (true, false) => stdout.to_string(),
        (false, false) => format!("{}\n{}", stderr.trim_end(), stdout.trim_end()),
    }
}

fn list_branch_pages(
    gh: &GhRunner,
    slug: &str,
) -> Result<Vec<GitHubRepositoryBranch>, ProjectPullRequestMergeError> {
    let mut branches = Vec::new();
    let mut page: u32 = 1;
    loop {
        let path = format!("/repos/{slug}/branches?per_page=100&page={page}");
        let page_branches: Vec<GitHubBranchPayload> = get_json(gh, &path)?;
        let page_len = page_branches.len();
        branches.extend(
            page_branches
                .into_iter()
                .map(|branch| GitHubRepositoryBranch {
                    name: branch.name,
                    commit: branch.commit.sha,
                }),
        );
        if page_len < 100 {
            break;
        }
        page = page.checked_add(1).ok_or_else(|| {
            ProjectPullRequestMergeError::new(
                "github_state_failed",
                "GitHub branch list pagination overflowed.",
            )
        })?;
    }
    Ok(branches)
}

fn remap_state_error(
    error: ProjectPullRequestMergeError,
    diagnostic: &str,
) -> ProjectPullRequestMergeError {
    let value = serde_json::to_value(&error).unwrap_or_default();
    let code = value.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let original_message = value.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let blob = format!("{diagnostic} {original_message}");
    let lower = blob.to_ascii_lowercase();
    if code == "github_cli_missing" || code == "github_auth_required" {
        return error;
    }
    let message = if diagnostic.trim().is_empty() {
        if original_message.is_empty() {
            "GitHub repository state failed.".to_string()
        } else {
            original_message.to_string()
        }
    } else {
        redact_diagnostic(diagnostic)
    };
    if lower.contains("rate limit") || lower.contains("abuse") {
        return ProjectPullRequestMergeError::new("github_state_failed", message);
    }
    if lower.contains("404")
        || lower.contains("not found")
        || (lower.contains("403") && !lower.contains("rate"))
    {
        return ProjectPullRequestMergeError::new("github_repo_unavailable", message);
    }
    ProjectPullRequestMergeError::new("github_state_failed", message)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
    #[test]
    fn maps_repo_and_branch_payloads_to_develop_head() {
        let develop = "d".repeat(40);
        let main = "m".repeat(40);
        let script = format!(
            r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/branches"*)
    printf '%s' '[{{"name":"develop","commit":{{"sha":"{develop}"}}}},{{"name":"main","commit":{{"sha":"{main}"}}}}]'
    ;;
  *"/repos/acme/app"*)
    printf '%s' '{{"default_branch":"develop"}}'
    ;;
  *) exit 1 ;;
esac
"#
        );
        let (_dir, path) = fake_gh(&script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let state =
            github_repository_state_with(&gh, "https://github.com/acme/app").expect("state");
        assert_eq!(state.head, "develop");
        assert_eq!(state.tags.len(), 0);
        assert_eq!(state.branches.len(), 2);
    }

    #[test]
    fn rejects_non_github_clone_url_before_runner() {
        let gh = GhRunner::from_resolved(Some(PathBuf::from("/bin/false")))
            .expect("dummy runner unused");
        let err =
            github_repository_state_with(&gh, "https://gitlab.com/acme/app").expect_err("gitlab");
        let value = serde_json::to_value(err).expect("json");
        assert_eq!(value["code"], "github_state_failed");
    }

    #[test]
    fn wrapper_maps_discover_failure() {
        let err = get_github_repository_state_with_runner(
            "https://github.com/acme/app".into(),
            GhRunner::from_resolved(None),
        )
        .expect_err("missing");
        let value = serde_json::to_value(err).expect("json");
        assert_eq!(value["code"], "github_cli_missing");
    }

    #[cfg(unix)]
    #[test]
    fn missing_gh_binary_is_cli_missing() {
        let err = GhRunner::from_resolved(None).expect_err("missing");
        let value = serde_json::to_value(err).expect("json");
        assert_eq!(value["code"], "github_cli_missing");
    }

    #[cfg(unix)]
    #[test]
    fn rate_limit_in_stdout_body_is_state_failed() {
        let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app"*)
    printf '%s' '{"message":"API rate limit exceeded for the user"}'
    printf 'gh: HTTP 403\n' >&2
    exit 1
    ;;
  *) exit 1 ;;
esac
"#;
        let (_dir, path) = fake_gh(script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let err =
            github_repository_state_with(&gh, "https://github.com/acme/app").expect_err("rate");
        let value = serde_json::to_value(err).expect("json");
        assert_eq!(value["code"], "github_state_failed");
    }

    #[cfg(unix)]
    #[test]
    fn not_found_in_stdout_body_is_repo_unavailable() {
        let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app"*)
    printf '%s' '{"message":"Not Found"}'
    exit 1
    ;;
  *) exit 1 ;;
esac
"#;
        let (_dir, path) = fake_gh(script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let err =
            github_repository_state_with(&gh, "https://github.com/acme/app").expect_err("missing");
        let value = serde_json::to_value(err).expect("json");
        assert_eq!(value["code"], "github_repo_unavailable");
    }
}
