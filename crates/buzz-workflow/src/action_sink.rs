//! Action sink trait — interface for workflow side-effects.
//!
//! The relay implements [`ActionSink`] to provide direct DB access to the
//! executor, replacing the HTTP loopback pattern.

use std::future::Future;
use std::pin::Pin;

use buzz_core::tenant::CommunityId;
use nostr::Tag;
use uuid::Uuid;

/// Server-resolved dynamic destination used for a workflow `send_message`.
///
/// Captured from the admitted run snapshot. The sink revalidates this exact
/// route immediately before constructing the kind `9` event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowMessageRoute {
    /// Workflow run that produced this side effect.
    pub run_id: Uuid,
    /// Workflow definition that owns the run.
    pub workflow_id: Uuid,
    /// Workflow home channel used as the standing-authority boundary.
    pub home_channel_id: Uuid,
    /// Canonical `30617:<owner>:<d>` repository coordinate.
    pub repository_coordinate: String,
    /// Canonical `30621:<signer>:<d>` project coordinate.
    pub project_coordinate: String,
}

/// Errors from action sink operations.
#[derive(Debug, thiserror::Error)]
pub enum ActionSinkError {
    /// An input parameter is malformed (e.g. invalid UUID).
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// The target channel does not exist.
    #[error("channel not found: {0}")]
    ChannelNotFound(String),
    /// The target channel is archived.
    #[error("channel is archived: {0}")]
    ChannelArchived(String),
    /// Nostr event construction or signing failed.
    #[error("event construction failed: {0}")]
    EventBuild(String),
    /// A database operation failed.
    #[error("database error: {0}")]
    Database(String),
    /// Message content is empty or whitespace-only.
    #[error("empty message content")]
    EmptyContent,
    /// Dynamic route lost authority between admission and the side effect.
    #[error("route is no longer valid")]
    RouteStale,
}

impl From<ActionSinkError> for crate::WorkflowError {
    fn from(e: ActionSinkError) -> Self {
        match e {
            ActionSinkError::RouteStale => crate::WorkflowError::RouteStale,
            other => crate::WorkflowError::WebhookError(other.to_string()),
        }
    }
}

/// Provenance tags for a successful dynamic `send_message`.
///
/// Exactly one repository `a`, one project `a`, and one `buzz:workflow-run`.
/// Does not include the raw idempotency key, alias map, or callback body.
pub fn route_provenance_tags(route: &WorkflowMessageRoute) -> Result<[Tag; 3], ActionSinkError> {
    Ok([
        Tag::parse(["a", &route.repository_coordinate])
            .map_err(|e| ActionSinkError::EventBuild(format!("repository a tag: {e}")))?,
        Tag::parse(["a", &route.project_coordinate])
            .map_err(|e| ActionSinkError::EventBuild(format!("project a tag: {e}")))?,
        Tag::parse(["buzz:workflow-run", &route.run_id.to_string()])
            .map_err(|e| ActionSinkError::EventBuild(format!("workflow-run tag: {e}")))?,
    ])
}

/// Interface for workflow actions that produce side effects.
///
/// Implemented by the relay to provide direct DB/event access to the executor.
/// This replaces the HTTP loopback where the executor POSTed to the relay's
/// REST API (which failed with 401 auth errors).
///
/// Returns `Pin<Box<dyn Future>>` for dyn-compatibility — required because
/// `WorkflowEngine` stores `Arc<dyn ActionSink>`.
pub trait ActionSink: Send + Sync {
    /// Post a message to a channel on behalf of a workflow owner.
    ///
    /// - `community_id`: the server-resolved community that owns the workflow
    ///   run driving this side effect. The relay-signed message is published
    ///   under *this* community, never the deployment/default tenant — the run
    ///   carries its owning community so a workflow in community B posts into B
    ///   even though the side effect has no inbound connection to bind.
    /// - `channel_id`: UUID string of the target channel
    /// - `text`: message body (must not be empty/whitespace-only)
    /// - `author_pubkey`: hex-encoded pubkey of the workflow owner (used for
    ///   the `p` attribution tag; the relay keypair signs the event)
    /// - `route`: admitted dynamic destination, when the run was routed by
    ///   `project_channel_by_repository`. `None` keeps static send behavior.
    ///
    /// Returns the event ID hex string on success.
    fn send_message(
        &self,
        community_id: CommunityId,
        channel_id: &str,
        text: &str,
        author_pubkey: &str,
        route: Option<WorkflowMessageRoute>,
    ) -> Pin<Box<dyn Future<Output = Result<String, ActionSinkError>> + Send + '_>>;
}
