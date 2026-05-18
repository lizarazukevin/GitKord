//! Discord message formatting and creation for PR events.
//!
//! Public functions map to webhook event types and produces
//! a formatted Discord message.

use serenity::all::{ChannelId, CreateMessage, CreateThread, EditMessage, Http, MessageId};
use tracing::info;

use crate::error::AppError;
use crate::error::Result;
use crate::github::types::{PullRequestPayload, PullRequestReviewPayload};

/// Post the main PR message to a channel and create an audit thread.
///
/// Returns `(message_id, thread_id)` — both must persist so future events
/// can edit the message and append to the thread.
pub async fn post_pull_request(
    http: &Http,
    channel_id: ChannelId,
    payload: &PullRequestPayload,
) -> Result<(u64, u64)> {
    let pr = &payload.pull_request;

    let message = channel_id
        .send_message(
            http,
            CreateMessage::new().content(format_pr_message(payload)),
        )
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    let thread_name = format!(
        "PR #{} — {} audit log",
        pr.number, payload.repository.full_name
    );
    let thread = channel_id
        .create_thread_from_message(
            http,
            message.id,
            CreateThread::new(thread_name)
                .auto_archive_duration(serenity::all::AutoArchiveDuration::OneWeek),
        )
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    info!(
        channel = %channel_id,
        message = %message.id,
        thread = %thread.id,
        pr      = pr.number,
        "posted PR message and created audit thread"
    );

    post_to_thread(
        http,
        thread.id.get(),
        &format!("🟢 **{}** opened this PR", pr.user.login),
    )
    .await?;

    Ok((message.id.get(), thread.id.get()))
}

/// Edit the main PR message in place and append an entry to the audit thread.
///
/// Used for `synchronize`, `closed`, `reopened` actions.
pub async fn update_pull_request(
    http: &Http,
    channel_id: ChannelId,
    message_id: u64,
    thread_id: u64,
    payload: &PullRequestPayload,
) -> Result<()> {
    channel_id
        .edit_message(
            http,
            MessageId::new(message_id),
            EditMessage::new().content(format_pr_message(payload)),
        )
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    let content = format!(
        "🔄 **{}** — PR #{} `{}`",
        timestamp(),
        payload.pull_request.number,
        payload.action,
    );
    post_to_thread(http, thread_id, &content).await?;

    info!(
        channel = %channel_id,
        message = message_id,
        thread = thread_id,
        pr      = payload.pull_request.number,
        action  = %payload.action,
        "updated PR message in Discord"
    );

    Ok(())
}

/// Post a review event to the PR audit thread.
pub async fn post_review(
    http: &Http,
    thread_id: u64,
    payload: &PullRequestReviewPayload,
) -> Result<()> {
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
    post_to_thread(http, thread_id, &content).await?;

    Ok(())
}

/// Post an audit entry to a thread. Used by commands (assign/unassign) and
/// event handlers alike so there is one place to change thread posting behavior.
pub async fn post_to_thread(http: &Http, thread_id: u64, content: &str) -> Result<()> {
    ChannelId::new(thread_id)
        .send_message(http, CreateMessage::new().content(content))
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;
    Ok(())
}

/// Format a PR payload into the main channel message.
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

/// Returns a short UTC timestamp string for audit entries.
fn timestamp() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string()
}
