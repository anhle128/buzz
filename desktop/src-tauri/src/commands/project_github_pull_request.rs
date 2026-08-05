use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use regex::Regex;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use url::Url;

const GH_TIMEOUT: Duration = Duration::from_secs(60);
const GH_STREAM_LIMIT: usize = 64 * 1024;
const GH_DIAGNOSTIC_LIMIT: usize = 8 * 1024;

#[cfg(windows)]
type GhJob = crate::managed_agents::JobHandle;
#[cfg(not(windows))]
type GhJob = ();

pub(crate) struct GitHubRepoRef {
    owner: String,
    repo: String,
}

impl GitHubRepoRef {
    pub(crate) fn is_github_host(raw: &str) -> bool {
        Url::parse(raw)
            .ok()
            .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
            .is_some_and(|host| host == "github.com")
    }

    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        if raw.chars().any(char::is_whitespace) {
            return Err("GitHub repository URL must not contain whitespace.".to_string());
        }
        if !raw.starts_with("https://github.com/") {
            return Err("GitHub repository URL must use https://github.com/.".to_string());
        }
        let url = Url::parse(raw).map_err(|_| "Invalid GitHub repository URL.".to_string())?;
        if url.scheme() != "https"
            || url.host_str() != Some("github.com")
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("GitHub repository URL must be a plain github.com HTTPS URL.".to_string());
        }

        let segments = url
            .path_segments()
            .ok_or_else(|| "GitHub repository URL must name an owner and repository.".to_string())?
            .collect::<Vec<_>>();
        if segments.len() != 2 || segments.iter().any(|segment| segment.is_empty()) {
            return Err("GitHub repository URL must contain exactly owner/repository.".to_string());
        }

        let owner = segments[0];
        let repo = segments[1].strip_suffix(".git").unwrap_or(segments[1]);
        let owner_ok = !owner.is_empty()
            && owner
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-');
        let repo_ok = !repo.is_empty()
            && repo.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
            });
        if !owner_ok || !repo_ok {
            return Err("Invalid GitHub repository owner or name.".to_string());
        }

        Ok(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
        })
    }

    fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    fn pulls_url(&self) -> String {
        format!("https://github.com/{}/pulls", self.slug())
    }
}

pub(crate) struct GitHubMergeInput {
    pub(crate) target: GitHubRepoRef,
    pub(crate) source: GitHubRepoRef,
    pub(crate) target_branch: String,
    pub(crate) source_branch: String,
    pub(crate) expected_commit: String,
    pub(crate) title: String,
    pub(crate) body: String,
}

#[derive(Debug)]
pub(crate) struct GitHubMergeOutcome {
    pub(crate) message: String,
    pub(crate) merge_commit: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubPullSummary {
    number: u64,
    html_url: String,
    state: String,
    merged_at: Option<String>,
    head: GitHubPullHead,
    base: GitHubPullBase,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubPullHead {
    sha: String,
    #[serde(rename = "ref")]
    branch: String,
    repo: GitHubNamedRepo,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubPullBase {
    #[serde(rename = "ref")]
    branch: String,
    repo: GitHubNamedRepo,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubNamedRepo {
    full_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GitHubPullView {
    number: u64,
    url: String,
    state: String,
    head_ref_oid: String,
    is_draft: bool,
    mergeable: String,
    merge_state_status: String,
    review_decision: String,
    auto_merge_request: Option<serde_json::Value>,
    merge_commit: Option<GitHubOid>,
    merged_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubOid {
    oid: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubCheck {
    name: String,
    state: String,
    bucket: String,
    link: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubRule {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug)]
enum PullDecision {
    Ready,
    Blocked(Vec<String>),
}

#[derive(Debug)]
enum PullSelection {
    Reuse(GitHubPullSummary),
    Create,
}

#[derive(Serialize)]
struct MergePullRequest<'a> {
    sha: &'a str,
    merge_method: &'static str,
}

#[derive(Deserialize)]
struct GitHubMergeResponse {
    sha: String,
    merged: bool,
    message: String,
}

fn select_pull(
    pulls: Vec<GitHubPullSummary>,
    input: &GitHubMergeInput,
) -> Result<PullSelection, ProjectPullRequestMergeError> {
    let mut open = Vec::new();
    let mut merged = Vec::new();
    let mut closed = Vec::new();
    for pull in pulls.into_iter().filter(|pull| {
        pull.base
            .repo
            .full_name
            .eq_ignore_ascii_case(&input.target.slug())
            && pull
                .head
                .repo
                .full_name
                .eq_ignore_ascii_case(&input.source.slug())
            && pull.base.branch == input.target_branch
            && pull.head.branch == input.source_branch
    }) {
        if pull.state.eq_ignore_ascii_case("open") {
            open.push(pull);
        } else if pull.merged_at.is_some()
            && pull.head.sha.eq_ignore_ascii_case(&input.expected_commit)
        {
            merged.push(pull);
        } else if pull.merged_at.is_none() {
            closed.push(pull);
        }
    }

    match open.len() {
        1 => return Ok(PullSelection::Reuse(open.remove(0))),
        2.. => {
            return Err(ProjectPullRequestMergeError::open_url(
                "github_pr_ambiguous",
                "More than one open GitHub pull request matched.",
                input.target.pulls_url(),
                Vec::new(),
            ));
        }
        0 => {}
    }
    match merged.len() {
        1 => return Ok(PullSelection::Reuse(merged.remove(0))),
        2.. => {
            return Err(ProjectPullRequestMergeError::open_url(
                "github_pr_ambiguous",
                "More than one merged GitHub pull request matched the reviewed commit.",
                input.target.pulls_url(),
                Vec::new(),
            ));
        }
        0 => {}
    }
    if let Some(pull) = closed.into_iter().min_by_key(|pull| pull.number) {
        return Err(ProjectPullRequestMergeError::open_url(
            "github_pr_blocked",
            "The matching GitHub pull request was closed without merging.",
            pull.html_url,
            vec!["Reopen or replace the pull request on GitHub.".to_string()],
        ));
    }
    Ok(PullSelection::Create)
}

#[derive(Debug)]
struct GhRunner {
    binary: PathBuf,
    timeout: Duration,
}

#[derive(Debug)]
struct GhOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl GhRunner {
    fn discover() -> Result<Self, ProjectPullRequestMergeError> {
        Self::from_resolved(crate::managed_agents::resolve_command("gh"))
    }

    fn from_resolved(binary: Option<PathBuf>) -> Result<Self, ProjectPullRequestMergeError> {
        let binary = binary.ok_or_else(missing_cli_error)?;
        Ok(Self {
            binary,
            timeout: GH_TIMEOUT,
        })
    }

    fn ensure_auth(&self) -> Result<(), ProjectPullRequestMergeError> {
        let output = self.run(&[
            OsString::from("auth"),
            OsString::from("status"),
            OsString::from("--hostname"),
            OsString::from("github.com"),
        ])?;
        if output.status.success() {
            return Ok(());
        }
        Err(ProjectPullRequestMergeError::new(
            "github_auth_required",
            "Authenticate GitHub CLI with: gh auth login --hostname github.com",
        ))
    }

    fn run_json<T: DeserializeOwned>(
        &self,
        args: &[OsString],
        accepted_codes: &[i32],
    ) -> Result<T, ProjectPullRequestMergeError> {
        let output = self.run(args)?;
        let code = output.status.code().unwrap_or(-1);
        if !output.status.success() && !accepted_codes.contains(&code) {
            return Err(ProjectPullRequestMergeError::new(
                "github_merge_failed",
                redact_diagnostic(&output.stderr),
            ));
        }
        serde_json::from_str(&output.stdout).map_err(|_| {
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "GitHub CLI returned an unexpected JSON response. Update gh, then retry.",
            )
        })
    }

    fn run(&self, args: &[OsString]) -> Result<GhOutput, ProjectPullRequestMergeError> {
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::util::configure_no_window(&mut command);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let mut child = command.spawn().map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                missing_cli_error()
            } else {
                ProjectPullRequestMergeError::new(
                    "github_merge_failed",
                    redact_diagnostic(&format!("Failed to run GitHub CLI: {error}")),
                )
            }
        })?;
        let pid = child.id();
        #[cfg(windows)]
        let mut job = match create_windows_job(pid) {
            Ok(job) => Some(job),
            Err(error) => {
                let _ = crate::managed_agents::terminate_process(pid);
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        #[cfg(not(windows))]
        let mut job = None::<GhJob>;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout_thread = std::thread::spawn(move || read_pipe_bounded(stdout, GH_STREAM_LIMIT));
        let stderr_thread = std::thread::spawn(move || read_pipe_bounded(stderr, GH_STREAM_LIMIT));

        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= self.timeout => {
                    let _ = join_gh_readers_after_cleanup(
                        || {
                            release_gh_tree(pid, &mut job);
                            let _ = child.kill();
                            let _ = child.wait();
                        },
                        stdout_thread,
                        stderr_thread,
                    );
                    return Err(ProjectPullRequestMergeError::new(
                        "github_merge_failed",
                        format!(
                            "GitHub CLI timed out after {} seconds.",
                            self.timeout.as_secs_f64()
                        ),
                    ));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    let _ = join_gh_readers_after_cleanup(
                        || {
                            release_gh_tree(pid, &mut job);
                            let _ = child.kill();
                            let _ = child.wait();
                        },
                        stdout_thread,
                        stderr_thread,
                    );
                    return Err(ProjectPullRequestMergeError::new(
                        "github_merge_failed",
                        redact_diagnostic(&format!("Failed to wait for GitHub CLI: {error}")),
                    ));
                }
            }
        };

        let (stdout, stderr) = join_gh_readers_after_cleanup(
            || release_gh_tree(pid, &mut job),
            stdout_thread,
            stderr_thread,
        );
        if status.code() == Some(127) {
            return Err(missing_cli_error());
        }
        Ok(GhOutput {
            status,
            stdout,
            stderr,
        })
    }
}

fn list_pulls(
    gh: &GhRunner,
    input: &GitHubMergeInput,
) -> Result<PullSelection, ProjectPullRequestMergeError> {
    let pages: Vec<Vec<GitHubPullSummary>> = gh.run_json(
        &[
            "api".into(),
            "--method".into(),
            "GET".into(),
            "--hostname".into(),
            "github.com".into(),
            "--paginate".into(),
            "--slurp".into(),
            format!("repos/{}/pulls", input.target.slug()).into(),
            "-f".into(),
            "state=all".into(),
            "-f".into(),
            format!("head={}:{}", input.source.owner, input.source_branch).into(),
            "-f".into(),
            format!("base={}", input.target_branch).into(),
            "-f".into(),
            "per_page=100".into(),
        ],
        &[],
    )?;
    select_pull(pages.into_iter().flatten().collect(), input)
}

fn load_pull_view(
    gh: &GhRunner,
    repo: &GitHubRepoRef,
    number: u64,
) -> Result<GitHubPullView, ProjectPullRequestMergeError> {
    gh.run_json(
        &[
            "pr".into(),
            "view".into(),
            number.to_string().into(),
            "--repo".into(),
            format!("github.com/{}", repo.slug()).into(),
            "--json".into(),
            "number,url,state,headRefOid,isDraft,mergeable,mergeStateStatus,reviewDecision,autoMergeRequest,mergeCommit,mergedAt".into(),
        ],
        &[],
    )
}

fn load_required_checks(
    gh: &GhRunner,
    repo: &GitHubRepoRef,
    number: u64,
    url: &str,
) -> Result<Vec<GitHubCheck>, ProjectPullRequestMergeError> {
    let output = gh.run(&[
        "pr".into(),
        "checks".into(),
        number.to_string().into(),
        "--repo".into(),
        format!("github.com/{}", repo.slug()).into(),
        "--required".into(),
        "--json".into(),
        "name,state,bucket,link".into(),
    ])?;
    let code = output.status.code().unwrap_or(-1);
    if !output.status.success() && code != 8 {
        return if is_no_required_checks_stderr(&output.stderr) {
            Ok(Vec::new())
        } else {
            Err(ProjectPullRequestMergeError::open_url(
                "github_merge_failed",
                redact_diagnostic(&output.stderr),
                url.to_string(),
                Vec::new(),
            ))
        };
    }
    if output.stdout.trim().is_empty() && is_no_required_checks_stderr(&output.stderr) {
        return Ok(Vec::new());
    }
    serde_json::from_str(&output.stdout).map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "GitHub CLI returned an unexpected required-check response. Update gh, then retry.",
        )
    })
}

fn is_no_required_checks_stderr(stderr: &str) -> bool {
    let normalized = stderr.trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "no required checks reported")
        || (normalized.starts_with("no required checks reported on the '")
            && normalized.ends_with("' branch"))
}

fn load_active_rules(
    gh: &GhRunner,
    repo: &GitHubRepoRef,
    branch: &str,
) -> Result<Vec<GitHubRule>, String> {
    let output = gh
        .run(&[
            "api".into(),
            "--method".into(),
            "GET".into(),
            "--hostname".into(),
            "github.com".into(),
            "--paginate".into(),
            "--slurp".into(),
            format!(
                "repos/{}/rules/branches/{}",
                repo.slug(),
                encode_path_segment(branch)
            )
            .into(),
            "-f".into(),
            "per_page=100".into(),
        ])
        .map_err(|error| {
            serde_json::to_value(error)
                .ok()
                .and_then(|value| {
                    value
                        .get("message")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "GitHub CLI failed.".to_string())
        })?;
    if !output.status.success() {
        return Err(redact_diagnostic(&output.stderr));
    }
    let pages: Vec<Vec<GitHubRule>> = serde_json::from_str(&output.stdout)
        .map_err(|_| "GitHub returned unexpected branch-rule JSON.".to_string())?;
    Ok(pages.into_iter().flatten().collect())
}

fn encode_path_segment(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            write!(&mut encoded, "%{byte:02X}").expect("write to string");
        }
    }
    encoded
}

fn decide_open_pull(
    view: &GitHubPullView,
    checks: &[GitHubCheck],
    rules: Result<&[GitHubRule], &str>,
) -> Result<PullDecision, ProjectPullRequestMergeError> {
    let mut reasons = Vec::new();
    if view.state != "OPEN" {
        reasons.push("The pull request is closed without merging.".to_string());
    }
    if view.is_draft {
        reasons.push("The pull request is still a draft.".to_string());
    }
    if matches!(view.mergeable.as_str(), "CONFLICTING" | "UNKNOWN") {
        reasons.push(format!("Mergeability is {}.", view.mergeable));
    }
    if matches!(
        view.review_decision.as_str(),
        "REVIEW_REQUIRED" | "CHANGES_REQUESTED"
    ) {
        reasons.push(format!("Review state is {}.", view.review_decision));
    }
    if view.auto_merge_request.is_some() {
        reasons.push("Auto-merge is already enabled on GitHub.".to_string());
    }
    for check in checks.iter().filter(|check| check.bucket != "pass") {
        let _ = &check.link;
        reasons.push(format!("Required check {} is {}.", check.name, check.state));
    }
    match rules {
        Ok(rules) if rules.iter().any(|rule| rule.kind == "merge_queue") => {
            reasons.push("The target branch requires GitHub's merge queue.".to_string());
        }
        Err(message) => reasons.push(format!("Branch rules could not be verified: {message}")),
        Ok(_) => {}
    }
    let merge_state_ready = view.merge_state_status == "CLEAN"
        || (view.merge_state_status == "UNSTABLE"
            && checks.iter().all(|check| check.bucket == "pass"));
    if !merge_state_ready {
        reasons.push(format!("Merge state is {}.", view.merge_state_status));
    }

    if reasons.is_empty() {
        Ok(PullDecision::Ready)
    } else {
        Ok(PullDecision::Blocked(reasons))
    }
}

fn json_input<T: Serialize>(
    value: &T,
) -> Result<tempfile::NamedTempFile, ProjectPullRequestMergeError> {
    let mut file = tempfile::Builder::new()
        .prefix("buzz-gh-")
        .tempfile()
        .map_err(|_| {
            ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "Buzz could not prepare the GitHub request.",
            )
        })?;
    serde_json::to_writer(file.as_file_mut(), value).map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "Buzz could not encode the GitHub request.",
        )
    })?;
    file.as_file_mut().flush().map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "Buzz could not finish the GitHub request.",
        )
    })?;
    Ok(file)
}

#[derive(Serialize)]
struct CreatePullRequest<'a> {
    title: &'a str,
    body: &'a str,
    head: String,
    base: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    head_repo: Option<String>,
}

pub(crate) fn find_or_create_pull_request(
    input: &GitHubMergeInput,
) -> Result<String, ProjectPullRequestMergeError> {
    let gh = GhRunner::discover()?;
    find_or_create_pull_request_with(&gh, input)
}

fn find_or_create_pull_request_with(
    gh: &GhRunner,
    input: &GitHubMergeInput,
) -> Result<String, ProjectPullRequestMergeError> {
    gh.ensure_auth()?;
    ensure_pull(gh, input).map(|pull| pull.html_url)
}

fn ensure_pull(
    gh: &GhRunner,
    input: &GitHubMergeInput,
) -> Result<GitHubPullSummary, ProjectPullRequestMergeError> {
    match list_pulls(gh, input)? {
        PullSelection::Reuse(pull) => return Ok(pull),
        PullSelection::Create => {}
    }

    let request = CreatePullRequest {
        title: &input.title,
        body: &input.body,
        head: format!("{}:{}", input.source.owner, input.source_branch),
        base: &input.target_branch,
        head_repo: (!input
            .source
            .slug()
            .eq_ignore_ascii_case(&input.target.slug()))
        .then(|| input.source.slug()),
    };
    let body = json_input(&request)?;
    let post = gh.run(&[
        "api".into(),
        "--method".into(),
        "POST".into(),
        "--hostname".into(),
        "github.com".into(),
        format!("repos/{}/pulls", input.target.slug()).into(),
        "--input".into(),
        body.path().as_os_str().to_owned(),
    ]);
    drop(body);
    let post_error = match post {
        Ok(output) if output.status.success() => None,
        Ok(output) => Some(ProjectPullRequestMergeError::new(
            "github_merge_failed",
            redact_diagnostic(&output.stderr),
        )),
        Err(error) => Some(error),
    };

    match list_pulls(gh, input)? {
        PullSelection::Reuse(pull) => Ok(pull),
        PullSelection::Create => match post_error {
            Some(error) => Err(error),
            None => Err(ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "GitHub created the pull request but it could not be found. Refresh and retry.",
            )),
        },
    }
}

pub(crate) fn merge_github_pull_request(
    input: GitHubMergeInput,
) -> Result<GitHubMergeOutcome, ProjectPullRequestMergeError> {
    let gh = GhRunner::discover()?;
    merge_github_pull_request_with(&gh, input)
}

fn merge_github_pull_request_with(
    gh: &GhRunner,
    input: GitHubMergeInput,
) -> Result<GitHubMergeOutcome, ProjectPullRequestMergeError> {
    gh.ensure_auth()?;
    let pull = ensure_pull(gh, &input)?;
    let view = load_pull_view(gh, &input.target, pull.number)?;
    if !view
        .head_ref_oid
        .eq_ignore_ascii_case(&input.expected_commit)
    {
        return Err(branch_changed_error());
    }
    if view.state == "MERGED" {
        let merge_commit = view
            .merge_commit
            .filter(|_| view.merged_at.is_some())
            .map(|commit| commit.oid)
            .ok_or_else(|| {
                ProjectPullRequestMergeError::new(
                    "github_merge_failed",
                    "GitHub reported a merge without a verifiable merge commit.",
                )
            })?;
        return Ok(GitHubMergeOutcome {
            message: format!("GitHub pull request #{} was already merged.", pull.number),
            merge_commit,
        });
    }
    if view.state != "OPEN" {
        return Err(ProjectPullRequestMergeError::open_url(
            "github_pr_blocked",
            "The matching GitHub pull request is closed without merging.",
            view.url,
            vec!["Reopen or replace the pull request on GitHub.".to_string()],
        ));
    }
    let checks = load_required_checks(gh, &input.target, pull.number, &view.url)?;
    let rules = load_active_rules(gh, &input.target, &input.target_branch);
    match decide_open_pull(&view, &checks, rules.as_deref().map_err(String::as_str))? {
        PullDecision::Blocked(reasons) => Err(ProjectPullRequestMergeError::open_url(
            "github_pr_blocked",
            "GitHub does not allow an immediate merge yet.",
            view.url,
            reasons,
        )),
        PullDecision::Ready => merge_and_verify(gh, &input, &view),
    }
}

fn merge_and_verify(
    gh: &GhRunner,
    input: &GitHubMergeInput,
    view: &GitHubPullView,
) -> Result<GitHubMergeOutcome, ProjectPullRequestMergeError> {
    let request = MergePullRequest {
        sha: &input.expected_commit,
        merge_method: "merge",
    };
    let body = json_input(&request)?;
    let output = gh.run(&[
        "api".into(),
        "--method".into(),
        "PUT".into(),
        "--hostname".into(),
        "github.com".into(),
        format!("repos/{}/pulls/{}/merge", input.target.slug(), view.number).into(),
        "--input".into(),
        body.path().as_os_str().to_owned(),
    ]);
    drop(body);
    let output = output?;
    if !output.status.success() {
        let diagnostic = format!("{}\n{}", output.stdout, output.stderr);
        return if is_stale_head_rejection(&diagnostic) {
            Err(branch_changed_error())
        } else {
            Err(ProjectPullRequestMergeError::open_url(
                "github_merge_failed",
                redact_diagnostic(&output.stderr),
                view.url.clone(),
                Vec::new(),
            ))
        };
    }
    let response: GitHubMergeResponse = serde_json::from_str(&output.stdout).map_err(|_| {
        ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "GitHub returned an unexpected merge response. Update gh, then retry.",
        )
    })?;
    let _ = response.sha;
    if !response.merged {
        return Err(ProjectPullRequestMergeError::open_url(
            "github_merge_failed",
            response.message,
            view.url.clone(),
            Vec::new(),
        ));
    }
    let verified = load_pull_view(gh, &input.target, view.number)?;
    if !verified
        .head_ref_oid
        .eq_ignore_ascii_case(&input.expected_commit)
    {
        return Err(branch_changed_error());
    }
    let merge_commit = verified
        .merge_commit
        .filter(|_| verified.state == "MERGED" && verified.merged_at.is_some())
        .map(|commit| commit.oid)
        .ok_or_else(|| {
            ProjectPullRequestMergeError::open_url(
                "github_merge_failed",
                "GitHub did not verify the pull request merge.",
                view.url.clone(),
                Vec::new(),
            )
        })?;
    Ok(GitHubMergeOutcome {
        message: format!("Merged GitHub pull request #{}.", view.number),
        merge_commit,
    })
}

fn is_stale_head_rejection(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("sha")
        && (lower.contains("does not match")
            || lower.contains("head branch was modified")
            || lower.contains("head sha"))
}

fn branch_changed_error() -> ProjectPullRequestMergeError {
    ProjectPullRequestMergeError::new(
        "github_branch_changed",
        "The GitHub pull request branch changed. Refresh before merging.",
    )
}

#[cfg(windows)]
fn create_windows_job(pid: u32) -> Result<GhJob, ProjectPullRequestMergeError> {
    crate::managed_agents::create_job_for_child(pid).ok_or_else(|| {
        ProjectPullRequestMergeError::new(
            "github_merge_failed",
            "GitHub CLI could not be isolated in a Windows process job.",
        )
    })
}

fn release_gh_tree(pid: u32, job: &mut Option<GhJob>) {
    #[cfg(unix)]
    unsafe {
        // The child owns this process group. ESRCH after leader exit is a no-op;
        // never fall back to +pid because that PID may already have been reused.
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        drop(job.take());
        let _ = pid;
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
    }
    #[cfg(not(windows))]
    let _ = job;
}

fn join_gh_readers_after_cleanup<T: Default, U: Default>(
    cleanup: impl FnOnce(),
    stdout_thread: std::thread::JoinHandle<T>,
    stderr_thread: std::thread::JoinHandle<U>,
) -> (T, U) {
    cleanup();
    (
        stdout_thread.join().unwrap_or_default(),
        stderr_thread.join().unwrap_or_default(),
    )
}

fn missing_cli_error() -> ProjectPullRequestMergeError {
    ProjectPullRequestMergeError::new(
        "github_cli_missing",
        "GitHub CLI is required. Install gh, then retry.",
    )
}

fn read_pipe_bounded(pipe: Option<impl Read>, limit: usize) -> String {
    let Some(mut pipe) = pipe else {
        return String::new();
    };
    let mut retained = Vec::with_capacity(limit);
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                let keep = count.min(limit.saturating_sub(retained.len()));
                retained.extend_from_slice(&chunk[..keep]);
            }
        }
    }
    let output = String::from_utf8_lossy(&retained);
    truncate_utf8_bytes(&output, limit)
}

fn redact_diagnostic(raw: &str) -> String {
    static TOKEN: OnceLock<Regex> = OnceLock::new();
    static AUTHORIZATION: OnceLock<Regex> = OnceLock::new();
    static CREDENTIAL: OnceLock<Regex> = OnceLock::new();
    static USERINFO: OnceLock<Regex> = OnceLock::new();

    let value = TOKEN
        .get_or_init(|| {
            Regex::new(r"(?i)\b(?:gh[pousr]_[a-z0-9]+|github_pat_[a-z0-9_]+)\b")
                .expect("valid GitHub token regex")
        })
        .replace_all(raw, "[REDACTED]");
    let value = AUTHORIZATION
        .get_or_init(|| {
            Regex::new(r"(?im)(authorization[^\S\r\n]*:[^\S\r\n]*)[^\r\n]*")
                .expect("valid authorization regex")
        })
        .replace_all(&value, "$1[REDACTED]");
    let value = CREDENTIAL
        .get_or_init(|| {
            Regex::new(r"(?i)\b(token|bearer)(?:\s*[:=]\s*|\s+)[^\s,;]+")
                .expect("valid credential regex")
        })
        .replace_all(&value, "$1 [REDACTED]");
    let value = USERINFO
        .get_or_init(|| Regex::new(r"(?i)(https?://)[^/\s@]*@").expect("valid URL userinfo regex"))
        .replace_all(&value, "$1[REDACTED]@");
    truncate_utf8_bytes(&value, GH_DIAGNOSTIC_LIMIT)
}

fn truncate_utf8_bytes(value: &str, limit: usize) -> String {
    let mut end = value.len().min(limit);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests;
