//! Audit‑log entries for PR threads.
//!
//! Every PR thread gets a running log of activity updates. The functions
//! here are thin wrappers that format the appropriate string and then
//! funnel through a shared helper so error handling stays consistent.

use crate::error::AppError;
use serenity::all::{ChannelId, CreateMessage, Http};
use std::sync::Arc;

/// Append a PR lifecycle change to the audit thread.
pub(crate) async fn post_lifecycle_pr_update(
    http: &Http,
    thread_id: u64,
    pr_number: u64,
    action: &str,
    merged: bool,
) -> Result<(), AppError> {
    let (emoji, verb) = match (action, merged) {
        ("closed", true) => ("🟣", "merged"),
        ("closed", _) => ("🔴", "closed"),
        ("reopened", _) => ("🟢", "reopened"),
        _ => ("⚪", action),
    };

    post_to_thread(
        http,
        thread_id,
        format!("{emoji} **PR #{pr_number}** was **{verb}**").as_ref(),
    )
    .await
}

/// Append a review verdict to the audit thread.
pub(crate) async fn post_review_verdict(
    http: &Http,
    thread_id: u64,
    reviewer: &str,
    state: &str,
    emoji: &str,
) -> Result<(), AppError> {
    let verb = match state {
        "approved" => "approved this review",
        "changes_requested" => "requested changes",
        "commented" => "published comments",
        _ => "submitted a review",
    };

    post_to_thread(
        http,
        thread_id,
        format!("{emoji} **{reviewer}** {verb}").as_ref(),
    )
    .await
}

/// Append a commit push notification to the audit thread.
pub(crate) async fn post_commit_push(
    http: &Http,
    thread_id: u64,
    pusher: &str,
    sha: &str,
) -> Result<(), AppError> {
    let short_sha = &sha[..7.min(sha.len())];

    post_to_thread(
        http,
        thread_id,
        format!("📬 **{pusher}** pushed commit `{short_sha}`").as_ref(),
    )
    .await
}

/// Send a message to any thread.
pub(crate) async fn post_to_thread(
    http: &Http,
    thread_id: u64,
    content: &str,
) -> Result<(), AppError> {
    ChannelId::new(thread_id)
        .send_message(http, CreateMessage::new().content(content))
        .await
        .map_err(|e| AppError::Discord(Arc::new(e)))?;
    Ok(())
}
