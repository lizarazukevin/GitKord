//! Discord message formatting and creation for PR events.
//!
//! Each public function corresponds to a webhook event and produces
//! a formatted Discord message. Editing existing messages (for updates)
//! will be added once state persistence is in place.

use serenity::all::{ChannelId, CreateMessage, EditMessage, Http, MessageId};
use tracing::info;

use crate::error::AppError;
use crate::github::types::{PullRequestPayload, PullRequestReviewPayload};

// ── Message creation ──────────────────────────────────────────────────────────

/// Post a new PR message to a Discord channel when a pull request is opened.
///
/// Returns the ID of the created message — the caller should persist this
/// so future events can edit the message in place.
pub async fn post_pull_request(
    http: &Http,
    channel_id: ChannelId,
    payload: &PullRequestPayload,
) -> Result<u64, AppError> {
    let pr = &payload.pull_request;
    let content = format_pr_message(payload);

    let message = channel_id
        .send_message(http, CreateMessage::new().content(content))
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    info!(
        channel = %channel_id,
        message = %message.id,
        pr      = pr.number,
        "posted PR message to Discord"
    );

    Ok(message.id.get())
}

/// Edit an existing PR message in place when a pull request is updated.
///
/// Used for `synchronize`, `closed`, `reopened` actions — keeps one
/// message per PR rather than flooding the channel.
pub async fn update_pull_request(
    http: &Http,
    channel_id: ChannelId,
    message_id: u64,
    payload: &PullRequestPayload,
) -> Result<(), AppError> {
    let content = format_pr_message(payload);

    channel_id
        .edit_message(
            http,
            MessageId::new(message_id),
            EditMessage::new().content(content),
        )
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    info!(
        channel = %channel_id,
        message = message_id,
        pr      = payload.pull_request.number,
        action  = %payload.action,
        "updated PR message in Discord"
    );

    Ok(())
}

/// Post a review event as a reply in the PR's audit thread.
pub async fn post_review(
    http: &Http,
    channel_id: ChannelId,
    payload: &PullRequestReviewPayload,
) -> Result<(), AppError> {
    let review = &payload.review;
    let pr = &payload.pull_request;

    let verb = match review.state.to_lowercase().as_str() {
        "approved" => "approved",
        "changes_requested" => "requested changes on",
        _ => "commented on",
    };

    let content = format!(
        "{emoji} **{reviewer}** {verb} PR #{number}",
        emoji = review.verdict_emoji(),
        reviewer = review.user.login,
        number = pr.number,
    );

    channel_id
        .send_message(http, CreateMessage::new().content(content))
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    Ok(())
}

// ── Formatting ────────────────────────────────────────────────────────────────

/// Format a PR payload into a Discord message string.
///
/// Plain string for now — will move to embeds once the core flow is
/// proven end-to-end.
fn format_pr_message(payload: &PullRequestPayload) -> String {
    let pr = &payload.pull_request;

    format!(
        "{emoji} **PR #{number}** — {title}\n\
         👤 {author} → `{base}` from `{head}`\n\
         🔗 {url}",
        emoji = pr.status_emoji(),
        number = pr.number,
        title = pr.title,
        author = pr.user.login,
        base = pr.base.branch,
        head = pr.head.branch,
        url = pr.html_url,
    )
}
