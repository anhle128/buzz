//! Structured pull-request merge failures returned across the Tauri boundary.

use serde::Serialize;

/// Machine-readable recovery metadata for a failed pull-request merge.
#[derive(Debug, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ProjectPullRequestMergeRecovery {
    OpenTerminal {
        #[serde(rename = "targetBranch")]
        target_branch: String,
        #[serde(rename = "sourceBranch")]
        source_branch: String,
    },
    OpenUrl {
        url: String,
        reasons: Vec<String>,
    },
}

/// Structured pull-request merge failure returned across the Tauri boundary.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPullRequestMergeError {
    code: String,
    message: String,
    recovery: Option<ProjectPullRequestMergeRecovery>,
}

impl ProjectPullRequestMergeError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            recovery: None,
        }
    }

    fn conflict(target_branch: String, source_branch: String) -> Self {
        Self {
            code: "merge_conflict".to_string(),
            message: "Pull request has merge conflicts.".to_string(),
            recovery: Some(ProjectPullRequestMergeRecovery::OpenTerminal {
                target_branch,
                source_branch,
            }),
        }
    }

    pub(crate) fn open_url(
        code: &str,
        message: impl Into<String>,
        url: String,
        reasons: impl IntoIterator<Item = String>,
    ) -> Self {
        let reasons = reasons
            .into_iter()
            .take(20)
            .map(|reason| reason.chars().take(200).collect())
            .collect();
        Self {
            code: code.to_string(),
            message: message.into(),
            recovery: Some(ProjectPullRequestMergeRecovery::OpenUrl { url, reasons }),
        }
    }
}

impl From<String> for ProjectPullRequestMergeError {
    fn from(message: String) -> Self {
        // Relay push-policy denial for a repo with no `buzz-channel` binding.
        // The stable token is declared in `buzz-core::git_perms`
        // (GIT_NO_CHANNEL_BINDING_TOKEN); the relay guarantees the denial body
        // starts with it. Push failures reach this conversion as raw
        // stderr/`remote:` text, so match the token anywhere in the message.
        if message.contains(buzz_core_pkg::git_perms::GIT_NO_CHANNEL_BINDING_TOKEN) {
            return Self::new(
                buzz_core_pkg::git_perms::GIT_NO_CHANNEL_BINDING_TOKEN,
                "This repository is not bound to a channel, so the relay cannot \
                 authorize pushes. Bind it with: buzz repos bind --id <repo> \
                 --channel <channel-uuid>",
            );
        }
        Self::new("merge_failed", message)
    }
}

pub(crate) fn classify_merge_error(
    message: String,
    has_conflicts: bool,
    target_branch: &str,
    source_branch: &str,
) -> ProjectPullRequestMergeError {
    if has_conflicts {
        ProjectPullRequestMergeError::conflict(target_branch.to_string(), source_branch.to_string())
    } else {
        ProjectPullRequestMergeError::new(
            "merge_failed",
            format!("Pull request merge failed: {message}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_merge_error, ProjectPullRequestMergeError, ProjectPullRequestMergeRecovery,
    };

    #[test]
    fn merge_conflict_error_has_stable_recovery_metadata() {
        let error =
            ProjectPullRequestMergeError::conflict("main".to_string(), "feature/demo".to_string());

        assert_eq!(error.code, "merge_conflict");
        assert_eq!(error.message, "Pull request has merge conflicts.");
        let recovery = error.recovery.expect("conflict recovery");
        let ProjectPullRequestMergeRecovery::OpenTerminal {
            target_branch,
            source_branch,
        } = recovery
        else {
            panic!("expected open terminal recovery");
        };
        assert_eq!(target_branch, "main");
        assert_eq!(source_branch, "feature/demo");
    }

    #[test]
    fn merge_conflict_error_serializes_for_tauri_clients() {
        let error =
            ProjectPullRequestMergeError::conflict("main".to_string(), "feature/demo".to_string());
        let value = serde_json::to_value(error).expect("serialize merge conflict");

        assert_eq!(value["code"], "merge_conflict");
        assert_eq!(value["recovery"]["action"], "open_terminal");
        assert_eq!(value["recovery"]["targetBranch"], "main");
        assert_eq!(value["recovery"]["sourceBranch"], "feature/demo");
    }

    #[test]
    fn open_url_recovery_serializes_capped_reasons() {
        let reasons = (0..25)
            .map(|index| format!("{index}:{}", "x".repeat(250)))
            .collect::<Vec<_>>();
        let error = ProjectPullRequestMergeError::open_url(
            "github_pr_blocked",
            "Blocked.",
            "https://github.com/acme/buzz/pull/42".to_string(),
            reasons,
        );
        let value = serde_json::to_value(error).expect("serialize URL recovery");

        assert_eq!(value["recovery"]["action"], "open_url");
        assert_eq!(
            value["recovery"]["url"],
            "https://github.com/acme/buzz/pull/42"
        );
        let reasons = value["recovery"]["reasons"]
            .as_array()
            .expect("reasons array");
        assert_eq!(reasons.len(), 20);
        assert!(reasons
            .iter()
            .all(|reason| reason.as_str().unwrap().chars().count() == 200));
    }

    #[test]
    fn merge_error_classification_only_recovers_conflicts() {
        let conflict = classify_merge_error(
            "CONFLICT (content): Merge conflict in src/main.rs".to_string(),
            true,
            "main",
            "feature/demo",
        );
        assert_eq!(conflict.code, "merge_conflict");
        assert!(conflict.recovery.is_some());

        let other = classify_merge_error(
            "fatal: refusing to merge unrelated histories".to_string(),
            false,
            "main",
            "feature/demo",
        );
        assert_eq!(other.code, "merge_failed");
        assert!(other.recovery.is_none());
    }

    #[test]
    fn no_channel_binding_denial_converts_to_structured_code() {
        // The relay's push-policy denial arrives as raw git stderr with
        // `remote:` framing; the stable token must be recognized wherever it
        // sits in the message.
        let remote_stderr = format!(
            "remote: {}\nerror: failed to push some refs",
            buzz_core_pkg::git_perms::GIT_NO_CHANNEL_BINDING_BODY
        );
        let error = ProjectPullRequestMergeError::from(remote_stderr);

        assert_eq!(
            error.code,
            buzz_core_pkg::git_perms::GIT_NO_CHANNEL_BINDING_TOKEN
        );
        assert!(error.message.contains("buzz repos bind"));
        assert!(error.recovery.is_none());

        // Unrelated push failures keep the generic code and original text.
        let generic = ProjectPullRequestMergeError::from("connection reset".to_string());
        assert_eq!(generic.code, "merge_failed");
        assert_eq!(generic.message, "connection reset");
    }
}
