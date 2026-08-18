//! Load a GitHub remote snapshot through `gh api` without cloning.

use crate::commands::project_git::{
    ProjectRepoCommitInfo, ProjectRepoFileInfo, ProjectRepoSnapshotInfo,
};
use crate::commands::project_git_exec::clean_branch;
use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use crate::commands::project_github_pull_request::{GhOutput, GhRunner, GitHubRepoRef};
use crate::commands::project_github_repository_state::{
    combined_cli_diagnostic, remap_state_error,
};
use base64::Engine as _;
use serde::Deserialize;
use std::ffi::OsString;

const GH_README_STREAM_LIMIT: usize = 256 * 1024;
const MAX_PREVIEW_BYTES: usize = 64 * 1024;
const MAX_TREE_ENTRIES: usize = 250;
const MAX_COMMITS: usize = 50;
const COMMITS_JQ: &str = "[.[] | {sha, tree: .commit.tree.sha, name: .commit.author.name, email: .commit.author.email, date: .commit.author.date, subject: (.commit.message | split(\"\\n\")[0])}]";
const TREE_JQ: &str =
    "{tree: [.tree[] | select(.type==\"blob\" or .type==\"commit\") | {path, type, size}][:250]}";

#[derive(Debug, Deserialize)]
struct GithubCommitRow {
    sha: String,
    tree: String,
    name: Option<String>,
    email: Option<String>,
    date: Option<String>,
    subject: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubTreePayload {
    tree: Vec<GithubTreeEntry>,
}

#[derive(Debug, Deserialize)]
struct GithubTreeEntry {
    path: String,
    #[serde(rename = "type")]
    kind: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GithubReadmePayload {
    path: String,
    content: Option<String>,
    encoding: Option<String>,
    size: Option<u64>,
}

pub(crate) fn github_repository_snapshot_with(
    gh: &GhRunner,
    clone_url: &str,
    git_ref: &str,
) -> Result<ProjectRepoSnapshotInfo, ProjectPullRequestMergeError> {
    let repo = GitHubRepoRef::parse(clone_url)
        .map_err(|message| ProjectPullRequestMergeError::new("github_state_failed", message))?;
    let branch = clean_github_branch(git_ref)?;
    gh.ensure_auth()
        .map_err(|error| remap_state_error(error, ""))?;

    let commit_rows = list_commit_rows(gh, &repo.slug(), &branch)?;
    let Some(head) = commit_rows.first() else {
        return Ok(empty_snapshot());
    };
    let tree_sha = head.tree.clone();
    let commits: Vec<ProjectRepoCommitInfo> = commit_rows
        .into_iter()
        .take(MAX_COMMITS)
        .map(map_commit)
        .collect();
    let files = list_tree(gh, &repo.slug(), &tree_sha)?;
    let readme = fetch_readme(gh, &repo.slug(), &branch)?;
    Ok(ProjectRepoSnapshotInfo {
        latest_commit: commits.first().cloned(),
        commits,
        files: attach_readme(files, readme),
        contributors: vec![],
    })
}

pub(crate) fn get_github_repository_snapshot_with_runner(
    clone_url: String,
    ref_name: String,
    gh: Result<GhRunner, ProjectPullRequestMergeError>,
) -> Result<ProjectRepoSnapshotInfo, ProjectPullRequestMergeError> {
    let gh = gh.map_err(|error| remap_state_error(error, ""))?;
    github_repository_snapshot_with(&gh, &clone_url, &ref_name)
}

/// Load README, file tree, and recent commits for a github.com clone URL.
#[tauri::command]
pub async fn get_github_repository_snapshot(
    clone_url: String,
    ref_name: String,
) -> Result<ProjectRepoSnapshotInfo, ProjectPullRequestMergeError> {
    tauri::async_runtime::spawn_blocking(move || {
        get_github_repository_snapshot_with_runner(
            clone_url,
            ref_name,
            GhRunner::discover(),
        )
    })
    .await
    .map_err(|error| {
        ProjectPullRequestMergeError::new("github_state_failed", error.to_string())
    })?
}

fn clean_github_branch(git_ref: &str) -> Result<String, ProjectPullRequestMergeError> {
    let trimmed = git_ref.trim();
    if trimmed.starts_with("refs/") && !trimmed.starts_with("refs/heads/") {
        return Err(ProjectPullRequestMergeError::new(
            "github_state_failed",
            "GitHub snapshot accepts a branch name, not a non-branch ref.",
        ));
    }
    clean_branch(Some(trimmed.to_string())).ok_or_else(|| {
        ProjectPullRequestMergeError::new("github_state_failed", "Invalid GitHub branch name.")
    })
}

fn list_commit_rows(
    gh: &GhRunner,
    slug: &str,
    branch: &str,
) -> Result<Vec<GithubCommitRow>, ProjectPullRequestMergeError> {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("sha", branch);
    serializer.append_pair("per_page", &MAX_COMMITS.to_string());
    let query = serializer.finish();
    let path = format!("/repos/{slug}/commits?{query}");
    let output = github_api_output(gh, &path, Some(COMMITS_JQ), 64 * 1024)?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        let lower = diagnostic.to_ascii_lowercase();
        // Empty GitHub repos return HTTP 409 ("Git Repository is empty."), not [].
        if lower.contains("409") || lower.contains("empty") {
            return Ok(vec![]);
        }
        return Err(remap_state_error(
            ProjectPullRequestMergeError::new("github_merge_failed", diagnostic.clone()),
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

fn list_tree(
    gh: &GhRunner,
    slug: &str,
    tree_sha: &str,
) -> Result<Vec<ProjectRepoFileInfo>, ProjectPullRequestMergeError> {
    let path = format!("/repos/{slug}/git/trees/{tree_sha}?recursive=1");
    let payload: GithubTreePayload = github_api_json(gh, &path, Some(TREE_JQ), 64 * 1024)?;
    Ok(payload
        .tree
        .into_iter()
        .filter(|entry| entry.kind == "blob" || entry.kind == "commit")
        .take(MAX_TREE_ENTRIES)
        .map(|entry| ProjectRepoFileInfo {
            path: entry.path,
            kind: entry.kind,
            size: entry.size,
            preview_content: None,
            last_changed_at: None,
            latest_commit: None,
        })
        .collect())
}

fn fetch_readme(
    gh: &GhRunner,
    slug: &str,
    branch: &str,
) -> Result<Option<GithubReadmePayload>, ProjectPullRequestMergeError> {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("ref", branch);
    let query = serializer.finish();
    let path = format!("/repos/{slug}/readme?{query}");
    let output = github_api_output(gh, &path, None, GH_README_STREAM_LIMIT)?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        let lower = diagnostic.to_ascii_lowercase();
        if lower.contains("404") || lower.contains("not found") {
            return Ok(None);
        }
        return Err(remap_state_error(
            ProjectPullRequestMergeError::new("github_merge_failed", diagnostic.clone()),
            &diagnostic,
        ));
    }
    if output.stdout.len() >= GH_README_STREAM_LIMIT {
        return Ok(None);
    }
    serde_json::from_str::<GithubReadmePayload>(&output.stdout)
        .map(Some)
        .or(Ok(None))
}

fn attach_readme(
    mut files: Vec<ProjectRepoFileInfo>,
    readme: Option<GithubReadmePayload>,
) -> Vec<ProjectRepoFileInfo> {
    let Some(readme) = readme else {
        return files;
    };
    let preview = decode_readme_preview(&readme);
    if let Some(existing) = files.iter_mut().find(|file| file.path == readme.path) {
        existing.preview_content = preview;
        return files;
    }
    files.insert(
        0,
        ProjectRepoFileInfo {
            path: readme.path,
            kind: "blob".to_string(),
            size: readme.size,
            preview_content: preview,
            last_changed_at: None,
            latest_commit: None,
        },
    );
    files
}

fn decode_readme_preview(readme: &GithubReadmePayload) -> Option<String> {
    if !readme
        .encoding
        .as_deref()
        .is_some_and(|encoding| encoding.eq_ignore_ascii_case("base64"))
    {
        return None;
    }
    let compact: String = readme
        .content
        .as_deref()?
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let mut bytes = base64::engine::general_purpose::STANDARD
        .decode(compact.as_bytes())
        .ok()?;
    if bytes.contains(&0) {
        return None;
    }
    if bytes.len() > MAX_PREVIEW_BYTES {
        bytes.truncate(MAX_PREVIEW_BYTES);
    }
    String::from_utf8(bytes).ok()
}

fn map_commit(row: GithubCommitRow) -> ProjectRepoCommitInfo {
    let timestamp = row
        .date
        .as_deref()
        .and_then(|date| chrono::DateTime::parse_from_rfc3339(date).ok())
        .map(|date| date.timestamp())
        .unwrap_or(0);
    ProjectRepoCommitInfo {
        short_hash: row.sha.chars().take(7).collect(),
        hash: row.sha,
        author_name: row.name.unwrap_or_default(),
        author_email: row.email.unwrap_or_default(),
        timestamp,
        subject: row.subject.unwrap_or_default(),
    }
}

fn empty_snapshot() -> ProjectRepoSnapshotInfo {
    ProjectRepoSnapshotInfo {
        latest_commit: None,
        commits: vec![],
        files: vec![],
        contributors: vec![],
    }
}

fn github_api_json<T: serde::de::DeserializeOwned>(
    gh: &GhRunner,
    path: &str,
    jq: Option<&str>,
    stdout_limit: usize,
) -> Result<T, ProjectPullRequestMergeError> {
    let output = github_api_output(gh, path, jq, stdout_limit)?;
    if !output.status.success() {
        let diagnostic = combined_cli_diagnostic(&output.stderr, &output.stdout);
        return Err(remap_state_error(
            ProjectPullRequestMergeError::new("github_merge_failed", diagnostic.clone()),
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

fn github_api_output(
    gh: &GhRunner,
    path: &str,
    jq: Option<&str>,
    stdout_limit: usize,
) -> Result<GhOutput, ProjectPullRequestMergeError> {
    let mut args = vec![
        OsString::from("api"),
        OsString::from("--hostname"),
        OsString::from("github.com"),
        OsString::from("--method"),
        OsString::from("GET"),
        OsString::from(path),
    ];
    if let Some(filter) = jq {
        args.push(OsString::from("--jq"));
        args.push(OsString::from(filter));
    }
    gh.run_with_limit(&args, stdout_limit)
        .map_err(|error| remap_state_error(error, ""))
}

#[cfg(test)]
impl std::fmt::Debug for ProjectRepoSnapshotInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProjectRepoSnapshotInfo")
            .field("commit_count", &self.commits.len())
            .field("file_count", &self.files.len())
            .finish()
    }
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
    fn maps_commits_tree_and_readme_preview() {
        let sha = "a".repeat(40);
        let tree = "b".repeat(40);
        let readme_b64 = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            b"# Hello develop\n",
        );
        let script = format!(
            r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[{{"sha":"{sha}","tree":"{tree}","name":"Ada","email":"ada@example.com","date":"2026-01-02T03:04:05Z","subject":"Seed readme"}}]'
    ;;
  *"/repos/acme/app/git/trees/"*)
    printf '%s' '{{"tree":[{{"path":"src/lib.rs","type":"blob","size":12}},{{"path":"README.md","type":"blob","size":16}}]}}'
    ;;
  *"/repos/acme/app/readme"*)
    printf '%s' '{{"path":"README.md","content":"{readme_b64}","encoding":"base64","size":16}}'
    ;;
  *) exit 1 ;;
esac
"#
        );
        let (_dir, path) = fake_gh(&script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let snapshot =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
                .expect("snapshot");
        assert_eq!(
            snapshot.latest_commit.as_ref().map(|c| c.hash.as_str()),
            Some(sha.as_str())
        );
        assert_eq!(snapshot.commits.len(), 1);
        assert_eq!(snapshot.contributors.len(), 0);
        let readme = snapshot
            .files
            .iter()
            .find(|file| file.path == "README.md")
            .expect("readme");
        assert_eq!(readme.preview_content.as_deref(), Some("# Hello develop\n"));
        assert!(snapshot
            .files
            .iter()
            .find(|file| file.path == "src/lib.rs")
            .and_then(|file| file.preview_content.as_ref())
            .is_none());
    }

    #[cfg(unix)]
    #[test]
    fn uses_jq_for_commits_and_tree_calls() {
        let sha = "a".repeat(40);
        let tree = "b".repeat(40);
        let script = format!(
            r#"
root=${{0%/gh}}
printf '%s\n' "$*" >> "$root/calls"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[{{"sha":"{sha}","tree":"{tree}","name":"Ada","email":"ada@example.com","date":"2026-01-02T03:04:05Z","subject":"Seed"}}]'
    ;;
  *"/repos/acme/app/git/trees/"*)
    printf '%s' '{{"tree":[]}}'
    ;;
  *"/repos/acme/app/readme"*)
    printf 'gh: HTTP 404\n' >&2
    exit 1
    ;;
  *) exit 1 ;;
esac
"#
        );
        let (dir, path) = fake_gh(&script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let _snapshot =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
                .expect("snapshot");
        let calls = std::fs::read_to_string(dir.path().join("calls")).expect("read calls");
        assert!(calls
            .lines()
            .any(|line| line.contains("/repos/acme/app/commits") && line.contains("--jq")));
        assert!(calls
            .lines()
            .any(|line| line.contains("/repos/acme/app/git/trees/") && line.contains("--jq")));
    }

    #[cfg(unix)]
    #[test]
    fn caps_tree_entries_and_readme_preview() {
        let sha = "a".repeat(40);
        let tree = "b".repeat(40);
        let tree_entries = (0..=MAX_TREE_ENTRIES)
            .map(|index| serde_json::json!({"path": format!("src/file{index}.rs"), "type": "blob", "size": 1}))
            .collect::<Vec<_>>();
        let tree_json = serde_json::json!({ "tree": tree_entries }).to_string();
        let readme_content = vec![b'x'; MAX_PREVIEW_BYTES + 10];
        let readme_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, readme_content);
        let script = format!(
            r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[{{"sha":"{sha}","tree":"{tree}","name":"Ada","email":"ada@example.com","date":"2026-01-02T03:04:05Z","subject":"Seed"}}]'
    ;;
  *"/repos/acme/app/git/trees/"*)
    printf '%s' '{tree_json}'
    ;;
  *"/repos/acme/app/readme"*)
    printf '%s' '{{"path":"README.md","content":"{readme_b64}","encoding":"base64","size":70000}}'
    ;;
  *) exit 1 ;;
esac
"#
        );
        let (_dir, path) = fake_gh(&script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let snapshot =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
                .expect("snapshot");
        assert_eq!(snapshot.files.len(), MAX_TREE_ENTRIES + 1);
        let readme = snapshot
            .files
            .iter()
            .find(|file| file.path == "README.md")
            .expect("inserted readme");
        let preview = readme.preview_content.as_deref().expect("preview");
        assert_eq!(preview.len(), MAX_PREVIEW_BYTES);
        assert!(preview.bytes().all(|byte| byte == b'x'));
    }

    #[cfg(unix)]
    #[test]
    fn readme_http_404_still_returns_tree() {
        let sha = "a".repeat(40);
        let tree = "b".repeat(40);
        let script = format!(
            r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[{{"sha":"{sha}","tree":"{tree}","name":"Ada","email":"ada@example.com","date":"2026-01-02T03:04:05Z","subject":"Seed"}}]'
    ;;
  *"/repos/acme/app/git/trees/"*)
    printf '%s' '{{"tree":[{{"path":"src/lib.rs","type":"blob","size":12}}]}}'
    ;;
  *"/repos/acme/app/readme"*)
    printf 'gh: HTTP 404\n' >&2
    printf '%s' '{{"message":"Not Found"}}'
    exit 1
    ;;
  *) exit 1 ;;
esac
"#
        );
        let (_dir, path) = fake_gh(&script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let snapshot =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
                .expect("snapshot");
        assert_eq!(snapshot.files.len(), 1);
        assert!(snapshot.files[0].preview_content.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn empty_commit_list_skips_tree_and_readme() {
        let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[]'
    ;;
  *) exit 1 ;;
esac
"#;
        let (_dir, path) = fake_gh(script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let snapshot =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
                .expect("empty");
        assert!(snapshot.latest_commit.is_none());
        assert!(snapshot.commits.is_empty());
        assert!(snapshot.files.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn empty_repo_http_409_skips_tree_and_readme() {
        let script = r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf 'gh: HTTP 409: Git Repository is empty.\n' >&2
    printf '%s' '{"message":"Git Repository is empty."}'
    exit 1
    ;;
  *) exit 1 ;;
esac
"#;
        let (_dir, path) = fake_gh(script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let snapshot =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
                .expect("empty repo");
        assert!(snapshot.latest_commit.is_none());
        assert!(snapshot.commits.is_empty());
        assert!(snapshot.files.is_empty());
        assert!(snapshot.contributors.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn tree_entries_keep_blobs_and_commits_only() {
        let sha = "a".repeat(40);
        let tree = "b".repeat(40);
        let script = format!(
            r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[{{"sha":"{sha}","tree":"{tree}","name":"Ada","email":"ada@example.com","date":"2026-01-02T03:04:05Z","subject":"Seed"}}]'
    ;;
  *"/repos/acme/app/git/trees/"*)
    printf '%s' '{{"tree":[{{"path":"src","type":"tree","size":null}},{{"path":"src/lib.rs","type":"blob","size":12}},{{"path":"vendor/dep","type":"commit","size":null}}]}}'
    ;;
  *"/repos/acme/app/readme"*)
    printf 'gh: HTTP 404\n' >&2
    exit 1
    ;;
  *) exit 1 ;;
esac
"#
        );
        let (_dir, path) = fake_gh(&script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let snapshot =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
                .expect("snapshot");
        let paths: Vec<&str> = snapshot.files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, vec!["src/lib.rs", "vendor/dep"]);
        assert!(!paths.iter().any(|path| *path == "src"));
    }

    #[test]
    fn wrapper_maps_discover_failure() {
        let err = get_github_repository_snapshot_with_runner(
            "https://github.com/acme/app".into(),
            "develop".into(),
            GhRunner::from_resolved(None),
        )
        .expect_err("missing");
        let value = serde_json::to_value(err).expect("json");
        assert_eq!(value["code"], "github_cli_missing");
    }

    #[test]
    fn rejects_non_github_clone_url_before_runner() {
        let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
            .expect("dummy runner unused");
        let err = github_repository_snapshot_with(&gh, "https://gitlab.com/acme/app", "main")
            .expect_err("gitlab");
        assert_eq!(error_code(&err), "github_state_failed");
    }

    #[test]
    fn rejects_nostr_and_tag_refs() {
        let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
            .expect("dummy runner unused");
        let nostr =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "refs/nostr/abc")
                .expect_err("nostr");
        let tag =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "refs/tags/v1")
                .expect_err("tag");
        assert_eq!(error_code(&nostr), "github_state_failed");
        assert_eq!(error_code(&tag), "github_state_failed");
    }

    #[test]
    fn rejects_non_branch_refs() {
        let gh = GhRunner::from_resolved(Some(std::path::PathBuf::from("/bin/false")))
            .expect("dummy runner unused");
        let err =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "refs/pull/1/head")
                .expect_err("pull ref");
        assert_eq!(error_code(&err), "github_state_failed");
    }

    #[cfg(unix)]
    #[test]
    fn missing_gh_binary_is_cli_missing() {
        let err = GhRunner::from_resolved(None).expect_err("missing");
        assert_eq!(error_code(&err), "github_cli_missing");
    }

    #[cfg(unix)]
    #[test]
    fn failed_auth_is_auth_required() {
        let (_dir, path) = fake_gh(
            r#"
case "$*" in
  *auth*status*) exit 1 ;;
  *) exit 1 ;;
esac
"#,
        );
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let err = github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
            .expect_err("auth");
        assert_eq!(error_code(&err), "github_auth_required");
    }

    #[cfg(unix)]
    #[test]
    fn inserts_readme_when_missing_from_tree_window() {
        let sha = "a".repeat(40);
        let tree = "b".repeat(40);
        let readme_b64 =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"# Deep\n");
        let script = format!(
            r#"
case "$*" in
  *auth*status*) exit 0 ;;
  *"/repos/acme/app/commits"*)
    printf '%s' '[{{"sha":"{sha}","tree":"{tree}","name":"Ada","email":"ada@example.com","date":"2026-01-02T03:04:05Z","subject":"Seed"}}]'
    ;;
  *"/repos/acme/app/git/trees/"*)
    printf '%s' '{{"tree":[{{"path":"src/lib.rs","type":"blob","size":12}}]}}'
    ;;
  *"/repos/acme/app/readme"*)
    printf '%s' '{{"path":"docs/README.md","content":"{readme_b64}","encoding":"base64","size":7}}'
    ;;
  *) exit 1 ;;
esac
"#
        );
        let (_dir, path) = fake_gh(&script);
        let gh = GhRunner::from_resolved(Some(path)).expect("runner");
        let snapshot =
            github_repository_snapshot_with(&gh, "https://github.com/acme/app", "develop")
                .expect("snapshot");
        assert!(snapshot
            .files
            .iter()
            .any(|file| file.path == "docs/README.md"
                && file.preview_content.as_deref() == Some("# Deep\n")));
    }
}
