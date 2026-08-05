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

#[derive(Debug)]
enum PullSelection {
    Reuse(GitHubPullSummary),
    Create,
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
    match list_pulls(gh, input)? {
        PullSelection::Reuse(pull) => return Ok(pull.html_url),
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
        PullSelection::Reuse(pull) => Ok(pull.html_url),
        PullSelection::Create => match post_error {
            Some(error) => Err(error),
            None => Err(ProjectPullRequestMergeError::new(
                "github_merge_failed",
                "GitHub created the pull request but it could not be found. Refresh and retry.",
            )),
        },
    }
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
