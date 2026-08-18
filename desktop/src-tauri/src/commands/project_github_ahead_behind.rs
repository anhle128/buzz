//! Compare a local HEAD to a GitHub branch tip without git fetch.

use crate::commands::project_git_exec::clean_branch;
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{GhOutput, GhRunner, GitHubRepoRef};
use crate::commands::project_github_repository_state::{
    combined_cli_diagnostic, remap_state_error,
};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;

/// GitHub comparison result for a local HEAD against a remote branch tip.
#[derive(Clone, Debug, Serialize)]
pub struct GithubAheadBehind {
    /// `compared` when counts are available, or `unpushed` when GitHub does not know the local SHA.
    pub status: String,
    /// Commits the local checkout is ahead of the GitHub branch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ahead: Option<u32>,
    /// Commits the local checkout is behind the GitHub branch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GithubComparePayload {
    ahead_by: u32,
    behind_by: u32,
}

pub(crate) fn github_ahead_behind_with(
    gh: &GhRunner,
    clone_url: &str,
    branch: &str,
    local_sha: &str,
    remote_sha: &str,
) -> Result<GithubAheadBehind, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_state_failed", message))?;
    clean_github_compare_branch(branch)?;
    let local_sha = parse_oid(local_sha)?;
    let remote_sha = parse_oid(remote_sha)?;
    gh.ensure_auth()
        .map_err(|error| remap_state_error(error, ""))?;
    if local_sha.eq_ignore_ascii_case(&remote_sha) {
        return Ok(GithubAheadBehind {
            status: "compared".into(),
            ahead: Some(0),
            behind: Some(0),
        });
    }
    let path = format!(
        "/repos/{}/compare/{}...{}",
        repo.slug(),
        remote_sha,
        local_sha
    );
    let output = github_api_output(gh, &path)?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        let lower = diagnostic.to_ascii_lowercase();
        if lower.contains("404") || lower.contains("not found") {
            return Ok(GithubAheadBehind {
                status: "unpushed".into(),
                ahead: None,
                behind: None,
            });
        }
        return Err(remap_state_error(
            ProjectPullRequestMergeError::new("github_merge_failed", diagnostic.clone()),
            &diagnostic,
        ));
    }
    let payload: GithubComparePayload = serde_json::from_str(&output.stdout).map_err(|_| {
        remap_state_error(
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "GitHub CLI returned an unexpected JSON response. Update gh, then retry.",
            ),
            &output.stderr,
        )
    })?;
    Ok(GithubAheadBehind {
        status: "compared".into(),
        ahead: Some(payload.ahead_by),
        behind: Some(payload.behind_by),
    })
}

fn clean_github_compare_branch(branch: &str) -> Result<String, ProjectPullRequestMergeError> {
    let trimmed = branch.trim();
    if trimmed.starts_with("refs/") && !trimmed.starts_with("refs/heads/") {
        return Err(ProjectPullRequestMergeError::new(
            "github_state_failed",
            "GitHub compare accepts a branch name, not a non-branch ref.",
        ));
    }
    clean_branch(Some(trimmed.to_string())).ok_or_else(|| {
        ProjectPullRequestMergeError::new("github_state_failed", "Invalid GitHub branch name.")
    })
}

fn parse_oid(value: &str) -> Result<String, ProjectPullRequestMergeError> {
    let value = value.trim().to_ascii_lowercase();
    if matches!(value.len(), 40 | 64)
        && value.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Ok(value);
    }
    Err(ProjectPullRequestMergeError::new(
        "github_state_failed",
        "GitHub compare requires a full commit SHA.",
    ))
}

fn github_api_output(gh: &GhRunner, path: &str) -> Result<GhOutput, ProjectPullRequestMergeError> {
    gh.run(&[
        OsString::from("api"),
        OsString::from("--hostname"),
        OsString::from("github.com"),
        OsString::from("--method"),
        OsString::from("GET"),
        OsString::from(path),
    ])
    .map_err(|error| remap_state_error(error, ""))
}

pub(crate) fn get_github_ahead_behind_with_runner(
    clone_url: String,
    branch: String,
    local_sha: String,
    remote_sha: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<GithubAheadBehind, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_state_error(error, ""))?;
    github_ahead_behind_with(&gh, &clone_url, &branch, &local_sha, &remote_sha)
}

/// Compare a local HEAD against the loaded GitHub branch tip.
#[tauri::command]
pub async fn get_github_ahead_behind(
    clone_url: String,
    branch: String,
    local_sha: String,
    remote_sha: String,
) -> Result<GithubAheadBehind, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        get_github_ahead_behind_with_runner(
            clone_url,
            branch,
            local_sha,
            remote_sha,
            GhRunner::discover(),
        )
    })
    .await
    .map_err(|error| ProjectPullRequestMergeError::new("github_state_failed", error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn fake_gh(script: &str) -> (tempfile::TempDir, std::path::PathBuf) {
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

    fn error_code(error: &ProjectPullRequestMergeError) -> String {
        serde_json::to_value(error).expect("json")["code"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    #[cfg(unix)]
    #[test]
    fn equal_shas_skip_compare() {
        let sha = "d".repeat(40);
        let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/compare"*) exit 1 ;;
  *) exit 1 ;;
esac
"#;
        let (_dir, path) = fake_gh(script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let result =
            github_ahead_behind_with(&gh, "https://github.com/acme/app", "develop", &sha, &sha)
                .expect("compared");
        assert_eq!(result.status, "compared");
        assert_eq!(result.ahead, Some(0));
        assert_eq!(result.behind, Some(0));
    }

    #[cfg(unix)]
    #[test]
    fn maps_ahead_by_and_behind_by() {
        let local = "e".repeat(40);
        let remote = "d".repeat(40);
        let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/compare"*)
    printf '%s' '{"ahead_by":2,"behind_by":1}'
    ;;
  *) exit 1 ;;
esac
"#;
        let (_dir, path) = fake_gh(script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let result = github_ahead_behind_with(
            &gh,
            "https://github.com/acme/app",
            "develop",
            &local,
            &remote,
        )
        .expect("compared");
        assert_eq!(result.status, "compared");
        assert_eq!(result.ahead, Some(2));
        assert_eq!(result.behind, Some(1));
    }

    #[cfg(unix)]
    #[test]
    fn unknown_local_sha_is_unpushed_not_zero() {
        let local = "f".repeat(40);
        let remote = "d".repeat(40);
        let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/compare"*)
    printf 'gh: HTTP 404\n' >&2
    printf '%s' '{"message":"Not Found"}'
    exit 1
    ;;
  *) exit 1 ;;
esac
"#;
        let (_dir, path) = fake_gh(script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let result = github_ahead_behind_with(
            &gh,
            "https://github.com/acme/app",
            "develop",
            &local,
            &remote,
        )
        .expect("unpushed");
        assert_eq!(result.status, "unpushed");
        assert_eq!(result.ahead, None);
        assert_eq!(result.behind, None);
    }

    #[test]
    fn rejects_non_github_clone_url_before_runner() {
        let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
            .expect("dummy runner unused");
        let err = github_ahead_behind_with(
            &gh,
            "https://gitlab.com/acme/app",
            "main",
            &"a".repeat(40),
            &"b".repeat(40),
        )
        .expect_err("gitlab");
        assert_eq!(error_code(&err), "github_state_failed");
    }

    #[test]
    fn rejects_nostr_and_tag_refs_before_runner() {
        let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
            .expect("dummy runner unused");
        let local = "a".repeat(40);
        let remote = "b".repeat(40);
        let nostr = github_ahead_behind_with(
            &gh,
            "https://github.com/acme/app",
            "refs/nostr/abc",
            &local,
            &remote,
        )
        .expect_err("nostr");
        let tag = github_ahead_behind_with(
            &gh,
            "https://github.com/acme/app",
            "refs/tags/v1",
            &local,
            &remote,
        )
        .expect_err("tag");
        assert_eq!(error_code(&nostr), "github_state_failed");
        assert_eq!(error_code(&tag), "github_state_failed");
    }

    #[test]
    fn rejects_non_branch_refs_before_runner() {
        let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
            .expect("dummy runner unused");
        let err = github_ahead_behind_with(
            &gh,
            "https://github.com/acme/app",
            "refs/pull/1/head",
            &"a".repeat(40),
            &"b".repeat(40),
        )
        .expect_err("pull ref");
        assert_eq!(error_code(&err), "github_state_failed");
    }

    #[cfg(unix)]
    #[test]
    fn failed_auth_is_auth_required() {
        let script = r#"
case "$*" in
  *auth*status*) exit 1 ;;
  *) exit 1 ;;
esac
"#;
        let (_dir, path) = fake_gh(script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let err = github_ahead_behind_with(
            &gh,
            "https://github.com/acme/app",
            "develop",
            &"a".repeat(40),
            &"b".repeat(40),
        )
        .expect_err("auth");
        assert_eq!(error_code(&err), "github_auth_required");
    }

    #[test]
    fn wrapper_maps_discover_failure() {
        let err = get_github_ahead_behind_with_runner(
            "https://github.com/acme/app".into(),
            "develop".into(),
            "a".repeat(40),
            "b".repeat(40),
            GhRunner::from_resolved(None),
        )
        .expect_err("missing");
        assert_eq!(error_code(&err), "github_cli_missing");
    }
}
