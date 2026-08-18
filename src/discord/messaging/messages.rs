//! Main PR message lifecycle.
//!
//! Handles posting the initial PR status message, editing it in place
//! when data changes, and the low‑level `post_to_thread` primitive used
//! by audit entries.

use crate::discord::messaging::audit::post_to_thread;
use crate::discord::messaging::renderer::format_pr_message;
use crate::discord::messaging::util::now_utc;
use crate::error::AppError;
use crate::service::github::pr_messages::PrMessageData;
use serenity::all::{ChannelId, CreateMessage, CreateThread, EditMessage, Http, MessageId};
use tracing::info;

/// Identifiers returned after posting a PR message.
pub struct DiscordMessage {
	pub message_id: u64,
	pub thread_id: u64,
}

/// Post the main PR message to a channel and create an audit thread on it.
///
/// Returns the message and thread IDs, both stored so future events
/// can edit the message and append entries to the thread.
pub async fn post_pull_request_message(
	http: &Http,
	channel_id: ChannelId,
	message_data: &PrMessageData,
) -> Result<DiscordMessage, AppError> {
	let formatted = format_pr_message(message_data, &now_utc());

	let message = channel_id
		.send_message(http, CreateMessage::new().content(formatted))
		.await?;

	let thread_name = format!("PR #{} — audit log", message_data.number);
	let thread = channel_id
		.create_thread_from_message(
			http,
			message.id,
			CreateThread::new(thread_name)
				.auto_archive_duration(serenity::all::AutoArchiveDuration::OneWeek),
		)
		.await?;

	info!(
		channel = %channel_id,
		message = %message.id,
		thread  = %thread.id,
		pr      = message_data.number,
		"posted PR message and created audit thread"
	);

	post_to_thread(
		http,
		thread.id.get(),
		&format!("🟢 **{}** opened a review", message_data.author),
	)
	.await?;

	Ok(DiscordMessage {
		message_id: message.id.get(),
		thread_id: thread.id.get(),
	})
}

/// Edit the main PR message in place with refreshed data.
///
/// Called on any event that changes visible PR state (e.g. review verdicts,
/// comment counts, lifecycle changes, etc.)
pub async fn update_pull_request_message(
	http: &Http,
	channel_id: ChannelId,
	message_id: u64,
	message_data: &PrMessageData,
) -> Result<(), AppError> {
	let formatted = format_pr_message(message_data, &now_utc());

	channel_id
		.edit_message(
			http,
			MessageId::new(message_id),
			EditMessage::new().content(formatted),
		)
		.await?;

	info!(
		channel = %channel_id,
		message = message_id,
		pr      = message_data.number,
		"updated PR message"
	);

	Ok(())
}
