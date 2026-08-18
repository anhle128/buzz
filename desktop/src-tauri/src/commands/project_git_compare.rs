//! Host-aware local/remote comparison for project git checkouts.

use super::project_git::{first_output_line, normalize_branch_option, ProjectRepoSyncStatusInfo};
use super::project_git_exec::{run_git, GitAuthConfig};
use super::project_github_pull_request::GitHubRepoRef;

fn short_hash(hash: &str) -> String {
    hash.chars().take(7).collect()
}

fn parse_count(output: &str) -> usize {
    output.trim().parse::<usize>().unwrap_or_default()
}

fn has_uncommitted_changes(output: &str) -> bool {
    output
        .lines()
        .any(|line| !line.starts_with("??") && !line.trim().is_empty())
}

fn has_untracked_files(output: &str) -> bool {
    output.lines().any(|line| line.starts_with("??"))
}

pub(crate) fn compare_local_remote_status(
    repo_dir: &std::path::Path,
    clone_url: &str,
    branch_name: Option<&str>,
    base_branch: Option<&str>,
    auth: &GitAuthConfig,
) -> Result<ProjectRepoSyncStatusInfo, String> {
    compare_local_remote_status_with_policy(
        repo_dir,
        clone_url,
        branch_name,
        base_branch,
        auth,
        GitHubRepoRef::parse(clone_url).is_ok(),
    )
}

/// Compare local and remote state with explicit strict-error policy.
/// GitHub uses strict results; tests exercise the preserved Buzz policy.
pub(super) fn compare_local_remote_status_with_policy(
    repo_dir: &std::path::Path,
    clone_url: &str,
    branch_name: Option<&str>,
    base_branch: Option<&str>,
    auth: &GitAuthConfig,
    strict_errors: bool,
) -> Result<ProjectRepoSyncStatusInfo, String> {
    let local_branch_result = run_git(&["branch", "--show-current"], Some(repo_dir), auth);
    let local_branch = if strict_errors {
        first_output_line(&local_branch_result?)
    } else {
        local_branch_result
            .ok()
            .and_then(|output| first_output_line(&output))
    };
    let local_branches_result = run_git(
        &[
            "for-each-ref",
            "--count=200",
            "--format=%(refname:short)",
            "refs/heads/",
        ],
        Some(repo_dir),
        auth,
    );
    let parse_local_branches = |output: String| {
        output
            .lines()
            .filter_map(|branch| normalize_branch_option(Some(branch)))
            .collect()
    };
    let local_branches: Vec<String> = if strict_errors {
        parse_local_branches(local_branches_result?)
    } else {
        local_branches_result
            .map(parse_local_branches)
            .unwrap_or_default()
    };
    // The local checkout's branch name is attacker-influencable (a hostile
    // remote can point HEAD at a flag-shaped refname), so it must pass the
    // same `clean_branch` validation as relay-supplied names before it is
    // ever handed to git as an argument.
    let branch = normalize_branch_option(branch_name)
        .or_else(|| normalize_branch_option(local_branch.as_deref()))
        .unwrap_or_else(|| "main".to_string());

    // Only rewrite the checkout's origin when it actually differs from the
    // project's clone URL — a read-only status poll must not silently
    // re-point the user's remote on every run.
    let current_origin_result = run_git(&["remote", "get-url", "origin"], Some(repo_dir), auth);
    let current_origin = if strict_errors {
        first_output_line(&current_origin_result?)
    } else {
        current_origin_result
            .ok()
            .and_then(|output| first_output_line(&output))
    };
    if current_origin.as_deref() != Some(clone_url) {
        let set_origin_result = run_git(
            &["remote", "set-url", "origin", clone_url],
            Some(repo_dir),
            auth,
        );
        if strict_errors {
            set_origin_result?;
        }
    }
    let base_branch =
        normalize_branch_option(base_branch).filter(|base_branch| *base_branch != branch);
    let mut fetch_args = vec![
        "fetch",
        "--quiet",
        "--depth=100",
        "--end-of-options",
        "origin",
        branch.as_str(),
    ];
    if let Some(base_branch) = base_branch.as_deref() {
        fetch_args.push(base_branch);
    }
    let fetch_result = run_git(&fetch_args, Some(repo_dir), auth);
    let remote_heads_result = run_git(
        &["ls-remote", "--heads", "--end-of-options", "origin"],
        Some(repo_dir),
        auth,
    );
    let remote_has_branches = if strict_errors {
        !remote_heads_result?.trim().is_empty()
    } else {
        remote_heads_result
            .map(|output| !output.trim().is_empty())
            .unwrap_or(true)
    };
    if let Err(error) = fetch_result {
        if strict_errors && remote_has_branches {
            return Err(error);
        }
    }

    let local_head_result = run_git(&["rev-parse", "HEAD"], Some(repo_dir), auth);
    let local_head = match local_head_result {
        Ok(output) => first_output_line(&output),
        Err(error) if strict_errors && !local_branches.is_empty() => return Err(error),
        Err(_) => None,
    };
    if strict_errors && local_head.is_none() && !local_branches.is_empty() {
        return Err("Could not resolve the local branch commit.".to_string());
    }
    let remote_ref = format!("refs/remotes/origin/{branch}");
    let remote_head = if strict_errors {
        first_output_line(&run_git(
            &[
                "for-each-ref",
                "--count=1",
                "--format=%(objectname)",
                remote_ref.as_str(),
            ],
            Some(repo_dir),
            auth,
        )?)
    } else {
        run_git(
            &["rev-parse", "--verify", "--quiet", remote_ref.as_str()],
            Some(repo_dir),
            auth,
        )
        .ok()
        .and_then(|output| first_output_line(&output))
    };
    // A legacy empty clone may have an unborn local `master` while the project
    // declares `main`. Permit that mismatch only when the remote has no branch
    // refs at all; any lookup failure is treated as non-empty (fail closed).
    let is_first_publish = remote_head.is_none() && !remote_has_branches;
    let merge_base = base_branch.as_deref().and_then(|base_branch| {
        run_git(
            &[
                "merge-base",
                "HEAD",
                format!("origin/{base_branch}").as_str(),
            ],
            Some(repo_dir),
            auth,
        )
        .ok()
        .and_then(|output| first_output_line(&output))
    });
    let status_result = run_git(&["status", "--porcelain"], Some(repo_dir), auth);
    let status = if strict_errors {
        status_result?
    } else {
        status_result.unwrap_or_default()
    };
    let has_uncommitted_changes = has_uncommitted_changes(&status);
    let has_untracked_files = has_untracked_files(&status);
    let ahead_count = match remote_head.as_deref() {
        Some(_) if strict_errors => run_git(
            &[
                "rev-list",
                "--count",
                format!("origin/{branch}..HEAD").as_str(),
            ],
            Some(repo_dir),
            auth,
        )?
        .trim()
        .parse::<usize>()
        .map_err(|_| "Git returned an invalid ahead count.".to_string())?,
        Some(_) => run_git(
            &[
                "rev-list",
                "--count",
                format!("origin/{branch}..HEAD").as_str(),
            ],
            Some(repo_dir),
            auth,
        )
        .map(|output| parse_count(&output))
        .unwrap_or_default(),
        None => usize::from(local_head.is_some()),
    };
    let behind_count = match remote_head.as_deref() {
        Some(_) if strict_errors => run_git(
            &[
                "rev-list",
                "--count",
                format!("HEAD..origin/{branch}").as_str(),
            ],
            Some(repo_dir),
            auth,
        )?
        .trim()
        .parse::<usize>()
        .map_err(|_| "Git returned an invalid behind count.".to_string())?,
        Some(_) => run_git(
            &[
                "rev-list",
                "--count",
                format!("HEAD..origin/{branch}").as_str(),
            ],
            Some(repo_dir),
            auth,
        )
        .map(|output| parse_count(&output))
        .unwrap_or_default(),
        None => 0,
    };

    let push_block_reason = if local_head.is_none() {
        Some("No local commits to push.".to_string())
    } else if local_branch.as_deref() != Some(branch.as_str()) && !is_first_publish {
        Some(format!(
            "Local checkout is on a different branch than {branch}."
        ))
    } else if has_uncommitted_changes || has_untracked_files {
        Some("Commit or discard local changes before pushing.".to_string())
    } else if behind_count > 0 {
        Some("Pull or reconcile remote commits before pushing.".to_string())
    } else if ahead_count == 0 {
        Some("Local branch is already pushed.".to_string())
    } else {
        None
    };

    // Pulling is a fast-forward only merge of origin/<branch> into the
    // current checkout, so it is blocked whenever that would not apply
    // cleanly (diverged history, dirty worktree, branch mismatch).
    let pull_block_reason = if local_head.is_none() {
        Some("No local commits yet — clone instead of pulling.".to_string())
    } else if remote_head.is_none() {
        Some("Remote branch not found.".to_string())
    } else if behind_count == 0 {
        Some("Local branch is up to date.".to_string())
    } else if local_branch.as_deref() != Some(branch.as_str()) {
        Some(format!(
            "Local checkout is on a different branch than {branch}."
        ))
    } else if has_uncommitted_changes {
        Some("Commit or stash local changes before pulling.".to_string())
    } else if ahead_count > 0 {
        Some("Local and remote have diverged — reconcile in a terminal.".to_string())
    } else {
        None
    };

    Ok(ProjectRepoSyncStatusInfo {
        local_path: Some(repo_dir.display().to_string()),
        local_branch,
        local_branches,
        local_head: local_head.clone(),
        local_short_head: local_head.as_deref().map(short_hash),
        remote_branch: Some(branch),
        remote_head: remote_head.clone(),
        remote_short_head: remote_head.as_deref().map(short_hash),
        merge_base,
        ahead_count,
        behind_count,
        has_uncommitted_changes,
        has_untracked_files,
        can_push: push_block_reason.is_none(),
        push_block_reason,
        can_pull: pull_block_reason.is_none(),
        pull_block_reason,
    })
}
