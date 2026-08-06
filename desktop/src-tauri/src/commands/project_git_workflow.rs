//! Local clone and pull-request merge commands for the Projects workflow.

use super::project_git::{first_output_line, normalize_branch_option};
use super::project_git_diff::clean_commit;
use super::project_git_exec::{
    build_git_auth_config_for_keys, build_git_clone_auth_config, clone_url_owner, run_git,
    validate_local_clone_url, validate_local_clone_url_for_workspace, validate_workspace_clone_url,
    GitAuthConfig,
};
use super::project_github_pull_request::{
    merge_github_pull_request, GitHubMergeInput, GitHubRepoRef,
};
use super::project_repo_paths::{
    canonical_repos_roots, canonicalize_repos_root, default_repos_root_candidates,
    find_local_repo_dir, local_repo_candidates,
};
use crate::app_state::AppState;
use crate::managed_agents::{load_managed_agents, spawn_key_refusal};
use crate::relay::submit_signed_event_with_keys;
use nostr::{Event, EventBuilder, JsonUtil, Keys, Kind, Tag, Timestamp};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

#[derive(Serialize)]
pub struct ProjectRepoCloneResult {
    pub path: String,
    pub cloned: bool,
    pub message: String,
}

#[derive(Serialize)]
pub struct ProjectRepoMergeResult {
    pub message: String,
    pub merge_commit: String,
    pub status_event: String,
    pub status_publication_error: Option<String>,
}

/// Machine-readable pull-request merge failure types live in
/// [`super::project_git_merge_error`]; re-imported here for the merge
/// workflow below.
use super::project_git_merge_error::{classify_merge_error, ProjectPullRequestMergeError};

struct ProjectRepoMergeGitResult {
    message: String,
    merge_commit: String,
}

fn complete_pull_request_merge(
    git_result: Result<ProjectRepoMergeGitResult, ProjectPullRequestMergeError>,
    build_status: impl FnOnce(
        &ProjectRepoMergeGitResult,
    ) -> Result<String, ProjectPullRequestMergeError>,
) -> Result<(ProjectRepoMergeGitResult, String), ProjectPullRequestMergeError> {
    let git_result = git_result?;
    let status_event = build_status(&git_result)?;
    Ok((git_result, status_event))
}

fn project_repo_merge_result(
    git_result: ProjectRepoMergeGitResult,
    status_event: String,
    status_publication_error: Option<String>,
) -> ProjectRepoMergeResult {
    ProjectRepoMergeResult {
        message: git_result.message,
        merge_commit: git_result.merge_commit,
        status_event,
        status_publication_error,
    }
}

enum PullRequestRepoRoute {
    Buzz,
    GitHub {
        target: GitHubRepoRef,
        source: GitHubRepoRef,
    },
}

/// Validated repository and pull-request metadata for a native merge.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPullRequestMergeInput {
    target_clone_url: String,
    source_clone_url: String,
    target_owner: String,
    repo_address: String,
    pull_request_id: String,
    pull_request_author: String,
    status_created_at: u64,
    target_branch: String,
    source_branch: String,
    expected_commit: String,
    title: String,
    body: String,
}

/// Repository-scoped metadata for an agent-signed review request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPullRequestReviewRequestInput {
    target_owner: String,
    repo_address: String,
    pull_request_id: String,
    reviewers: Vec<String>,
    reviewer_label: String,
}

/// Repository-scoped metadata for an agent-signed lifecycle status.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPullRequestStatusInput {
    target_owner: String,
    repo_address: String,
    pull_request_id: String,
    pull_request_author: String,
    status: String,
    created_at: u64,
}

/// A previously signed merged-status event that needs publishing again.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPullRequestMergedStatusInput {
    target_owner: String,
    status_event: String,
}

fn normalize_commit(value: &str) -> Option<String> {
    clean_commit(Some(value.trim().to_ascii_lowercase()))
}

fn normalize_event_id(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    (value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())).then_some(value)
}

fn classify_pull_request_route(
    target_clone_url: &str,
    source_clone_url: &str,
) -> Result<PullRequestRepoRoute, ProjectPullRequestMergeError> {
    let target_is_github = has_github_intent(target_clone_url);
    let source_is_github = has_github_intent(source_clone_url);
    match (target_is_github, source_is_github) {
        (true, true) => Ok(PullRequestRepoRoute::GitHub {
            target: GitHubRepoRef::parse(target_clone_url).map_err(|message| {
                ProjectPullRequestMergeError::new("github_merge_failed", message)
            })?,
            source: GitHubRepoRef::parse(source_clone_url).map_err(|message| {
                ProjectPullRequestMergeError::new("github_merge_failed", message)
            })?,
        }),
        (false, false) => Ok(PullRequestRepoRoute::Buzz),
        _ => Err(ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "Source and target repositories must use the same supported host.",
        )),
    }
}

fn has_github_intent(raw: &str) -> bool {
    if GitHubRepoRef::is_github_host(raw) {
        return true;
    }
    let raw = raw.to_ascii_lowercase();
    raw.strip_prefix("https://github.com")
        .is_some_and(|suffix| {
            suffix.is_empty()
                || matches!(suffix.as_bytes().first(), Some(b'/' | b':' | b'?' | b'#'))
        })
}

struct ProjectOwnerIdentity {
    keys: Keys,
    auth_tag: Option<String>,
}

fn project_owner_identity(
    app: &AppHandle,
    state: &AppState,
    target_owner: &str,
) -> Result<ProjectOwnerIdentity, String> {
    let viewer_keys = state.signing_keys()?;
    if viewer_keys.public_key().to_hex() == target_owner {
        return Ok(ProjectOwnerIdentity {
            keys: viewer_keys,
            auth_tag: None,
        });
    }

    let _store_guard = state
        .managed_agents_store_lock
        .lock()
        .map_err(|error| error.to_string())?;
    let records = load_managed_agents(app)?;
    let record = records
        .iter()
        .find(|record| record.pubkey.eq_ignore_ascii_case(target_owner))
        .ok_or_else(|| {
            "Only the repository owner or the owner of its managed agent can merge pull requests."
                .to_string()
        })?;
    if let Some(error) = spawn_key_refusal(record) {
        return Err(error);
    }
    let keys = Keys::parse(&record.private_key_nsec)
        .map_err(|error| format!("managed agent signing key is invalid: {error}"))?;
    if keys.public_key().to_hex() != target_owner {
        return Err("Managed agent key does not match the repository owner.".to_string());
    }
    Ok(ProjectOwnerIdentity {
        keys,
        auth_tag: record.auth_tag.clone(),
    })
}

fn validate_repo_address(repo_address: &str, owner: &str) -> Result<(), String> {
    let prefix = format!("30617:{owner}:");
    if repo_address.strip_prefix(&prefix).is_none_or(str::is_empty) {
        return Err("Repository address does not match the repository owner.".to_string());
    }
    Ok(())
}

fn validate_merge_status_metadata(
    repo_address: &str,
    owner: &str,
    pull_request_id: &str,
    pull_request_author: &str,
) -> Result<(String, String), String> {
    validate_repo_address(repo_address, owner)?;
    let pull_request_id = normalize_event_id(pull_request_id)
        .ok_or_else(|| "Invalid pull request event ID.".to_string())?;
    let pull_request_author = normalize_event_id(pull_request_author)
        .ok_or_else(|| "Invalid pull request author.".to_string())?;
    Ok((pull_request_id, pull_request_author))
}

fn build_merged_status_event(
    keys: &Keys,
    repo_address: &str,
    pull_request_id: &str,
    pull_request_author: &str,
    merge_commit: &str,
    created_at: u64,
) -> Result<String, String> {
    let owner = keys.public_key().to_hex();
    let (pull_request_id, pull_request_author) =
        validate_merge_status_metadata(repo_address, &owner, pull_request_id, pull_request_author)?;
    let merge_commit =
        normalize_commit(merge_commit).ok_or_else(|| "Invalid merge commit.".to_string())?;
    let created_at = created_at.max(Timestamp::now().as_secs());

    let mut raw_tags = vec![
        vec!["e", pull_request_id.as_str(), "", "root"],
        vec!["a", repo_address],
        vec!["p", owner.as_str()],
    ];
    if pull_request_author != owner {
        raw_tags.push(vec!["p", pull_request_author.as_str()]);
    }
    raw_tags.extend([
        vec!["merge-commit", merge_commit.as_str()],
        vec!["r", merge_commit.as_str()],
    ]);
    let tags = raw_tags
        .into_iter()
        .map(Tag::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("build merged status tags: {error}"))?;
    EventBuilder::new(Kind::Custom(1631), "")
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at))
        .sign_with_keys(keys)
        .map(|event| event.as_json())
        .map_err(|error| format!("sign merged pull request status: {error}"))
}

fn build_pull_request_status_event(
    keys: &Keys,
    repo_address: &str,
    pull_request_id: &str,
    pull_request_author: &str,
    status: &str,
    created_at: u64,
) -> Result<String, String> {
    let owner = keys.public_key().to_hex();
    let (pull_request_id, pull_request_author) =
        validate_merge_status_metadata(repo_address, &owner, pull_request_id, pull_request_author)?;
    let kind = match status {
        "open" => Kind::Custom(1630),
        "closed" => Kind::Custom(1632),
        "draft" => Kind::Custom(1633),
        _ => return Err("Invalid pull request lifecycle status.".to_string()),
    };
    let mut raw_tags = vec![
        vec!["e", pull_request_id.as_str(), "", "root"],
        vec!["a", repo_address],
        vec!["p", owner.as_str()],
    ];
    if pull_request_author != owner {
        raw_tags.push(vec!["p", pull_request_author.as_str()]);
    }
    let tags = raw_tags
        .into_iter()
        .map(Tag::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("build pull request status tags: {error}"))?;
    EventBuilder::new(kind, "")
        .tags(tags)
        .custom_created_at(Timestamp::from(created_at.max(Timestamp::now().as_secs())))
        .sign_with_keys(keys)
        .map(|event| event.as_json())
        .map_err(|error| format!("sign pull request status: {error}"))
}

fn build_review_request_event(
    keys: &Keys,
    repo_address: &str,
    pull_request_id: &str,
    reviewers: &[String],
    reviewer_label: &str,
) -> Result<String, String> {
    let owner = keys.public_key().to_hex();
    validate_repo_address(repo_address, &owner)?;
    let pull_request_id = normalize_event_id(pull_request_id)
        .ok_or_else(|| "Invalid pull request event ID.".to_string())?;
    if reviewers.is_empty() || reviewers.len() > 50 {
        return Err("Select between 1 and 50 reviewers.".to_string());
    }
    let mut reviewers = reviewers
        .iter()
        .map(|reviewer| {
            normalize_event_id(reviewer).ok_or_else(|| "Invalid reviewer pubkey.".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    reviewers.sort();
    reviewers.dedup();
    let reviewer_label = reviewer_label.trim();
    if reviewer_label.is_empty() || reviewer_label.chars().count() > 128 {
        return Err("Reviewer label must be between 1 and 128 characters.".to_string());
    }

    let mut raw_tags = vec![
        vec![
            "e".to_string(),
            pull_request_id,
            String::new(),
            "root".to_string(),
        ],
        vec!["a".to_string(), repo_address.to_string()],
    ];
    raw_tags.extend(
        reviewers
            .into_iter()
            .map(|reviewer| vec!["p".to_string(), reviewer]),
    );
    raw_tags.push(vec!["t".to_string(), "review-request".to_string()]);
    let tags = raw_tags
        .into_iter()
        .map(Tag::parse)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("build review request tags: {error}"))?;
    EventBuilder::new(
        Kind::TextNote,
        format!("Requested a review from {reviewer_label}"),
    )
    .tags(tags)
    .sign_with_keys(keys)
    .map(|event| event.as_json())
    .map_err(|error| format!("sign pull request review request: {error}"))
}

fn same_repository(left: &str, right: &str) -> bool {
    left.trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .eq_ignore_ascii_case(right.trim().trim_end_matches('/').trim_end_matches(".git"))
}

fn clone_destination_root(repos_dir: Option<&str>) -> Result<std::path::PathBuf, String> {
    match canonical_repos_roots(repos_dir) {
        Ok(roots) => roots
            .into_iter()
            .next()
            .ok_or_else(|| "reposDir is not accessible".to_string()),
        Err(error) => {
            if repos_dir.is_some() {
                return Err(error);
            }
            let root = default_repos_root_candidates()
                .into_iter()
                .next()
                .ok_or(error)?;
            std::fs::create_dir_all(&root).map_err(|error| format!("create repos dir: {error}"))?;
            canonicalize_repos_root(root)
        }
    }
}

fn align_unborn_head_branch(
    repo_dir: &std::path::Path,
    branch: Option<&str>,
    auth: &GitAuthConfig,
) -> Result<(), String> {
    let Some(branch) = branch else {
        return Ok(());
    };
    if run_git(&["rev-parse", "--verify", "HEAD"], Some(repo_dir), auth).is_ok() {
        return Ok(());
    }
    let target = format!("refs/heads/{branch}");
    run_git(
        &["symbolic-ref", "HEAD", target.as_str()],
        Some(repo_dir),
        auth,
    )
    .map(|_| ())
}

pub(crate) fn clone_project_repository_blocking(
    repos_dir: Option<&str>,
    project_dtag: &str,
    clone_url: &str,
    default_branch: Option<&str>,
    auth: &GitAuthConfig,
) -> Result<ProjectRepoCloneResult, String> {
    validate_local_clone_url(clone_url)?;
    let branch = normalize_branch_option(default_branch);
    if let Some(repo_dir) = find_local_repo_dir(repos_dir, project_dtag, Some(clone_url))? {
        return Ok(ProjectRepoCloneResult {
            path: repo_dir.display().to_string(),
            cloned: false,
            message: "Repository is already cloned.".to_string(),
        });
    }

    let repos_root = clone_destination_root(repos_dir)?;
    let repo_name = local_repo_candidates(project_dtag, Some(clone_url))
        .into_iter()
        .next()
        .ok_or_else(|| "Could not derive a directory name for the repository.".to_string())?;
    let repo_dir = repos_root.join(repo_name);
    if repo_dir.exists() {
        return Err(format!(
            "{} already exists but is not a git checkout.",
            repo_dir.display()
        ));
    }
    let repo_path = repo_dir
        .to_str()
        .ok_or_else(|| "repository path is not UTF-8".to_string())?;

    let mut clone_args = vec!["clone"];
    if let Some(ref branch) = branch {
        clone_args.extend(["--branch", branch.as_str()]);
    }
    clone_args.extend(["--end-of-options", clone_url, repo_path]);
    if let Err(error) = run_git(&clone_args, None, auth) {
        if branch.is_none() {
            return Err(error);
        }
        run_git(
            &["clone", "--end-of-options", clone_url, repo_path],
            None,
            auth,
        )?;
    }
    align_unborn_head_branch(&repo_dir, branch.as_deref(), auth)?;

    Ok(ProjectRepoCloneResult {
        path: repo_dir.display().to_string(),
        cloned: true,
        message: format!("Cloned repository to {}.", repo_dir.display()),
    })
}

fn merge_buzz_pull_request(
    target_clone_url: String,
    source_clone_url: String,
    target_branch: String,
    source_branch: String,
    expected_commit: String,
    merger_pubkey: String,
    auth: GitAuthConfig,
) -> Result<ProjectRepoMergeGitResult, ProjectPullRequestMergeError> {
    let temp_dir = tempfile::tempdir().map_err(|error| format!("create temp dir: {error}"))?;
    let repo_dir = temp_dir.path().join("repo");
    let repo_path = repo_dir
        .to_str()
        .ok_or_else(|| "temporary repository path is not UTF-8".to_string())?;
    run_git(
        &[
            "clone",
            "--filter=blob:none",
            "--no-tags",
            "--branch",
            target_branch.as_str(),
            "--single-branch",
            "--end-of-options",
            target_clone_url.as_str(),
            repo_path,
        ],
        None,
        &auth,
    )?;
    run_git(
        &[
            "fetch",
            "--quiet",
            "--end-of-options",
            source_clone_url.as_str(),
            source_branch.as_str(),
        ],
        Some(&repo_dir),
        &auth,
    )?;
    let source_head = run_git(&["rev-parse", "FETCH_HEAD"], Some(&repo_dir), &auth)
        .ok()
        .and_then(|output| first_output_line(&output))
        .ok_or_else(|| "Could not resolve the pull request branch.".to_string())?;
    if source_head.to_ascii_lowercase() != expected_commit {
        return Err(ProjectPullRequestMergeError::new(
            "branch_changed",
            "The pull request branch changed. Refresh the pull request before merging.",
        ));
    }

    let merge_email = format!("{merger_pubkey}@users.noreply.buzz");
    let merge_result = run_git(
        &[
            "-c",
            "user.name=Buzz User",
            "-c",
            format!("user.email={merge_email}").as_str(),
            "merge",
            "--no-edit",
            "--end-of-options",
            expected_commit.as_str(),
        ],
        Some(&repo_dir),
        &auth,
    );
    if let Err(error) = merge_result {
        let has_conflicts = run_git(
            &["diff", "--name-only", "--diff-filter=U"],
            Some(&repo_dir),
            &auth,
        )
        .is_ok_and(|output| !output.trim().is_empty());
        return Err(classify_merge_error(
            error,
            has_conflicts,
            &target_branch,
            &source_branch,
        ));
    }
    let merge_commit = run_git(&["rev-parse", "HEAD"], Some(&repo_dir), &auth)
        .ok()
        .and_then(|output| first_output_line(&output))
        .ok_or_else(|| "Could not resolve the merge commit.".to_string())?;
    run_git(
        &[
            "push",
            "--end-of-options",
            "origin",
            format!("HEAD:{target_branch}").as_str(),
        ],
        Some(&repo_dir),
        &auth,
    )?;

    Ok(ProjectRepoMergeGitResult {
        message: format!("Merged {source_branch} into {target_branch}."),
        merge_commit,
    })
}

#[tauri::command]
pub async fn clone_project_repository(
    repos_dir: Option<String>,
    project_dtag: String,
    clone_url: String,
    default_branch: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProjectRepoCloneResult, String> {
    validate_local_clone_url_for_workspace(&clone_url, &state)?;
    let auth = build_git_clone_auth_config(&clone_url, &state)?;
    tauri::async_runtime::spawn_blocking(move || {
        clone_project_repository_blocking(
            repos_dir.as_deref(),
            &project_dtag,
            &clone_url,
            default_branch.as_deref(),
            &auth,
        )
    })
    .await
    .map_err(|error| format!("repo clone task failed: {error}"))?
}

#[tauri::command]
pub async fn sign_project_pull_request_status(
    input: ProjectPullRequestStatusInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target_owner = input.target_owner.trim().to_ascii_lowercase();
    if normalize_event_id(&target_owner).is_none() {
        return Err("Invalid target repository owner.".to_string());
    }
    let identity = project_owner_identity(&app, &state, &target_owner)?;
    let event = Event::from_json(build_pull_request_status_event(
        &identity.keys,
        &input.repo_address,
        &input.pull_request_id,
        &input.pull_request_author,
        &input.status,
        input.created_at,
    )?)
    .map_err(|error| format!("parse signed pull request status: {error}"))?;
    submit_signed_event_with_keys(&event, &state, &identity.keys, identity.auth_tag.as_deref())
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn sign_project_pull_request_review_request(
    input: ProjectPullRequestReviewRequestInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target_owner = input.target_owner.trim().to_ascii_lowercase();
    if normalize_event_id(&target_owner).is_none() {
        return Err("Invalid target repository owner.".to_string());
    }
    let identity = project_owner_identity(&app, &state, &target_owner)?;
    let event = Event::from_json(build_review_request_event(
        &identity.keys,
        &input.repo_address,
        &input.pull_request_id,
        &input.reviewers,
        &input.reviewer_label,
    )?)
    .map_err(|error| format!("parse signed review request: {error}"))?;
    submit_signed_event_with_keys(&event, &state, &identity.keys, identity.auth_tag.as_deref())
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn publish_project_pull_request_merged_status(
    input: ProjectPullRequestMergedStatusInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target_owner = input.target_owner.trim().to_ascii_lowercase();
    if normalize_event_id(&target_owner).is_none() {
        return Err("Invalid target repository owner.".to_string());
    }
    let event = Event::from_json(input.status_event)
        .map_err(|error| format!("parse merged status event: {error}"))?;
    if event.kind != Kind::Custom(1631)
        || event.pubkey.to_hex() != target_owner
        || event.verify().is_err()
    {
        return Err("Invalid merged pull request status event.".to_string());
    }
    let identity = project_owner_identity(&app, &state, &target_owner)?;
    submit_signed_event_with_keys(&event, &state, &identity.keys, identity.auth_tag.as_deref())
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn merge_project_pull_request(
    input: ProjectPullRequestMergeInput,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ProjectRepoMergeResult, ProjectPullRequestMergeError> {
    let ProjectPullRequestMergeInput {
        target_clone_url,
        source_clone_url,
        target_owner,
        repo_address,
        pull_request_id,
        pull_request_author,
        status_created_at,
        target_branch,
        source_branch,
        expected_commit,
        title,
        body,
    } = input;
    let target_owner = target_owner.trim().to_ascii_lowercase();
    if target_owner.len() != 64 || !target_owner.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid target repository owner.".to_string().into());
    }
    let owner_identity = project_owner_identity(&app, &state, &target_owner)?;
    let merger_pubkey = owner_identity.keys.public_key().to_hex();
    let target_branch = normalize_branch_option(Some(&target_branch))
        .ok_or_else(|| "Invalid target branch.".to_string())?;
    let source_branch = normalize_branch_option(Some(&source_branch))
        .ok_or_else(|| "Invalid source branch.".to_string())?;
    let expected_commit = normalize_commit(&expected_commit)
        .ok_or_else(|| "Invalid pull request commit.".to_string())?;
    let (pull_request_id, pull_request_author) = validate_merge_status_metadata(
        &repo_address,
        &merger_pubkey,
        &pull_request_id,
        &pull_request_author,
    )?;
    let route = classify_pull_request_route(&target_clone_url, &source_clone_url)?;

    let git_result = match route {
        PullRequestRepoRoute::GitHub { target, source } => {
            if title.trim().is_empty() {
                Err(ProjectPullRequestMergeError::new(
                    "github_merge_failed",
                    "GitHub pull request title cannot be empty.",
                ))
            } else {
                let github_input = GitHubMergeInput {
                    target,
                    source,
                    target_branch,
                    source_branch,
                    expected_commit,
                    title,
                    body,
                };
                tauri::async_runtime::spawn_blocking(move || {
                    merge_github_pull_request(github_input)
                })
                .await
                .map_err(|error| {
                    ProjectPullRequestMergeError::new(
                        "merge_task_failed",
                        format!("GitHub merge task failed: {error}"),
                    )
                })
                .and_then(|outcome| outcome)
                .map(|outcome| ProjectRepoMergeGitResult {
                    message: outcome.message,
                    merge_commit: outcome.merge_commit,
                })
            }
        }
        PullRequestRepoRoute::Buzz => {
            validate_workspace_clone_url(&target_clone_url, &state)?;
            validate_workspace_clone_url(&source_clone_url, &state)?;
            if clone_url_owner(&target_clone_url).as_deref() != Some(target_owner.as_str()) {
                return Err("Target clone URL does not match the repository owner."
                    .to_string()
                    .into());
            }
            if target_branch == source_branch
                && same_repository(&target_clone_url, &source_clone_url)
            {
                return Err("Source and target branches must be different."
                    .to_string()
                    .into());
            }
            let auth = build_git_auth_config_for_keys(&owner_identity.keys)?;
            Ok(tauri::async_runtime::spawn_blocking(move || {
                merge_buzz_pull_request(
                    target_clone_url,
                    source_clone_url,
                    target_branch,
                    source_branch,
                    expected_commit,
                    merger_pubkey,
                    auth,
                )
            })
            .await
            .map_err(|error| {
                ProjectPullRequestMergeError::new(
                    "merge_task_failed",
                    format!("pull request merge task failed: {error}"),
                )
            })??)
        }
    };
    let (git_result, status_event) = complete_pull_request_merge(git_result, |git_result| {
        build_merged_status_event(
            &owner_identity.keys,
            &repo_address,
            &pull_request_id,
            &pull_request_author,
            &git_result.merge_commit,
            status_created_at,
        )
        .map_err(ProjectPullRequestMergeError::from)
    })?;
    let signed_status = Event::from_json(&status_event)
        .map_err(|error| format!("parse signed merged status: {error}"))?;
    let status_publication_error = submit_signed_event_with_keys(
        &signed_status,
        &state,
        &owner_identity.keys,
        owner_identity.auth_tag.as_deref(),
    )
    .await
    .err();
    Ok(project_repo_merge_result(
        git_result,
        status_event,
        status_publication_error,
    ))
}

#[cfg(test)]
#[path = "project_git_workflow_tests.rs"]
mod tests;
