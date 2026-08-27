//! Response utilities for our application.

use crate::AppError;
use serenity::all::{
	CommandInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
	GuildId,
};

/// Send an ephemeral response visible only to the users who ran a slash command.
/// Limited to operation taking 3 seconds, otherwise use [`deferred_ephemeral`].
pub(super) async fn ephemeral(
	ctx: &Context,
	cmd: &CommandInteraction,
	content: &str,
) -> Result<(), AppError> {
	cmd.create_response(
		ctx,
		CreateInteractionResponse::Message(
			CreateInteractionResponseMessage::new()
				.content(content)
				.ephemeral(true),
		),
	)
	.await?;
	Ok(())
}

/// Send an ephemeral follow-up after the interaction has been deferred.
/// Interaction token is valid for 15 minutes.
/// Ref: <https://docs.discord.com/developers/interactions/receiving-and-responding#interaction-callback>
pub(super) async fn deferred_ephemeral(
	ctx: &Context,
	cmd: &CommandInteraction,
	content: &str,
) -> Result<(), AppError> {
	cmd.create_followup(
		ctx,
		serenity::builder::CreateInteractionResponseFollowup::new()
			.content(content)
			.ephemeral(true),
	)
	.await
	.map(|_| ())?;
	Ok(())
}

/// Command must be invoked in a guild/server.
pub(super) async fn require_guild(
	ctx: &Context,
	cmd: &CommandInteraction,
) -> Result<Option<GuildId>, AppError> {
	if let Some(guild_id) = cmd.guild_id {
		Ok(Some(guild_id))
	} else {
		ephemeral(ctx, cmd, "This command only works inside a server.").await?;
		Ok(None)
	}
}

/// If the command was invoked inside a thread, send an ephemeral rejection
/// and return `true`.
pub(super) async fn reject_if_thread(
	ctx: &Context,
	cmd: &CommandInteraction,
	reason: &str,
) -> Result<bool, AppError> {
	let channel = cmd.channel_id.to_channel(ctx).await?;
	if let Some(guild_channel) = channel.guild() {
		if guild_channel.thread_metadata.is_some() {
			ephemeral(ctx, cmd, reason).await?;
			return Ok(true);
		}
	}
	Ok(false)
}
