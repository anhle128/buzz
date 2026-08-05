use crate::commands::project_git_merge_error::ProjectPullRequestMergeError;
use regex::Regex;
use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use url::Url;

const GH_TIMEOUT: Duration = Duration::from_secs(60);
const GH_STREAM_LIMIT: usize = 64 * 1024;
const GH_DIAGNOSTIC_LIMIT: usize = 8 * 1024;

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

    fn gh_repo(&self) -> String {
        format!("github.com/{}", self.slug())
    }

    fn pull_url(&self, number: u64) -> String {
        format!("https://github.com/{}/pull/{number}", self.slug())
    }

    fn pulls_url(&self) -> String {
        format!("https://github.com/{}/pulls", self.slug())
    }
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

    fn run(&self, args: &[OsString]) -> Result<GhOutput, ProjectPullRequestMergeError> {
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::util::configure_no_window(&mut command);

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
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout_thread = std::thread::spawn(move || read_pipe_bounded(stdout, GH_STREAM_LIMIT));
        let stderr_thread = std::thread::spawn(move || read_pipe_bounded(stderr, GH_STREAM_LIMIT));

        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_thread.join();
                    let _ = stderr_thread.join();
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
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_thread.join();
                    let _ = stderr_thread.join();
                    return Err(ProjectPullRequestMergeError::new(
                        "github_merge_failed",
                        redact_diagnostic(&format!("Failed to wait for GitHub CLI: {error}")),
                    ));
                }
            }
        };

        let stdout = stdout_thread.join().unwrap_or_default();
        let stderr = stderr_thread.join().unwrap_or_default();
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
            Regex::new(r"(?i)\b(token|bearer)\s+(?:[:=]\s*)?[^\s,;]+")
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
