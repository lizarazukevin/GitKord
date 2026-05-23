//! Discord message transport.
//!
//! Handles all direct Discord API calls — posting, editing, and threading.
//! Formatting is handled entirely by renderer.rs so this module stays focused
//! on the mechanics of getting content into Discord.

use serenity::all::{ChannelId, CreateMessage, CreateThread, EditMessage, Http, MessageId};
use tracing::info;

use crate::discord::messages::renderer::format_pr_message;
use crate::discord::models::PostedPullRequest;
use crate::error::{AppError, Result};
use crate::github::models::PrMessageData;

/// Post the main PR message to a channel and create an audit thread on it.
///
/// Returns message and thread IDs — both must be stored so future events
/// can edit the message and append entries to the thread.
pub async fn post_pull_request_message(
    http: &Http,
    channel_id: ChannelId,
    message_data: &PrMessageData,
) -> Result<PostedPullRequest> {
    let message = channel_id
        .send_message(
            http,
            CreateMessage::new().content(format_pr_message(message_data)),
        )
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    let thread_name = format!("PR #{} — audit log", message_data.number);
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
        thread  = %thread.id,
        pr      = message_data.number,
        "posted PR message and created audit thread"
    );

    // First audit entry — records who opened the PR.
    post_to_thread(
        http,
        thread.id.get(),
        &format!("🟢 **{}** opened this PR", message_data.author),
    )
    .await?;

    Ok(PostedPullRequest {
        message_id: message.id.get(),
        thread_id: thread.id.get(),
    })
}

/// Edit the main PR message in place with refreshed data.
///
/// Called on any event that changes visible PR state — review verdicts,
/// comment counts, lifecycle changes, etc.
pub async fn update_pull_request_message(
    http: &Http,
    channel_id: ChannelId,
    message_id: u64,
    message_data: &PrMessageData,
) -> Result<()> {
    channel_id
        .edit_message(
            http,
            MessageId::new(message_id),
            EditMessage::new().content(format_pr_message(message_data)),
        )
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;

    info!(
        channel   = %channel_id,
        message   = message_id,
        pr        = message_data.number,
        "updated PR message"
    );

    Ok(())
}

/// Send a message to a PR audit thread.
///
/// Used by audit.rs and commands (assign/unassign) so all thread posting
/// goes through one place.
pub async fn post_to_thread(http: &Http, thread_id: u64, content: &str) -> Result<()> {
    ChannelId::new(thread_id)
        .send_message(http, CreateMessage::new().content(content))
        .await
        .map_err(|e| AppError::Discord(Box::new(e)))?;
    Ok(())
}
