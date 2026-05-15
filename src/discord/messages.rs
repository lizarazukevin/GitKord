//! Discord message formatting and creation for PR events.
//!
//! Each public function corresponds to a webhook event and produces
//! a formatted Discord message. Editing existing messages (for updates)
//! will be added once state persistence is in place.

use serenity::all::{ChannelId, CreateMessage, CreateThread, EditMessage, Http, MessageId};
use tracing::info;

use crate::error::AppError;
use crate::github::types::{PullRequestPayload, PullRequestReviewPayload};

// ── Message creation ──────────────────────────────────────────────────────────

/// Post a new PR message to a Discord channel when a pull request is opened.
///
/// Returns `(message_id, thread_id)` — both must persist so future events
/// can edit the message and append to the thread respectively.
pub async fn post_pull_request(
    http: &Http,
    channel_id: ChannelId,
    payload: &PullRequestPayload,
) -> Result<(u64, u64), AppError> {
    let pr = &payload.pull_request;
    let content = format_pr_message(payload);

    let message = channel_id
        .send_message(http, CreateMessage::new().content(content))
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
        "posted PR message to Discord"
    );

    let opening_entry = format!("🟢 **{}** opened this PR", pr.user.login);
    ChannelId::new(thread.id.get())
        .send_message(http, CreateMessage::new().content(opening_entry))
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    Ok((message.id.get(), thread.id.get()))
}

/// Edit an existing PR message in place when a pull request is updated.
///
/// Used for `synchronize`, `closed`, `reopened` actions — keeps one
/// message per PR rather than flooding the channel.
pub async fn update_pull_request(
    http: &Http,
    channel_id: ChannelId,
    message_id: u64,
    thread_id: u64,
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

    let audit_entry = format!(
        "🔄 **{}** — PR #{} `{}`",
        chrono_now(),
        payload.pull_request.number,
        payload.action,
    );
    ChannelId::new(thread_id)
        .send_message(http, CreateMessage::new().content(audit_entry))
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

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

/// Post a review event as a reply in the PR's audit thread.
pub async fn post_review(
    http: &Http,
    thread_id: u64,
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

    ChannelId::new(thread_id)
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

/// Returns a short UTC timestamp string for audit entries.
fn chrono_now() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string()
}
