//! Response utilities for our application.

use serenity::all::{
    CommandInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
    GuildId,
};
use serenity::Error;

/// Send an ephemeral response visible only to the users who ran a slash command.
pub(super) async fn ephemeral(
    ctx: &Context,
    cmd: &CommandInteraction,
    content: &str,
) -> Result<(), Error> {
    cmd.create_response(
        ctx,
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(content)
                .ephemeral(true),
        ),
    )
    .await
}

/// Send an ephemeral follow‑up after the interaction has been deferred.
pub(super) async fn deferred_ephemeral(
    ctx: &Context,
    cmd: &CommandInteraction,
    content: &str,
) -> Result<(), Error> {
    cmd.create_followup(
        ctx,
        serenity::builder::CreateInteractionResponseFollowup::new()
            .content(content)
            .ephemeral(true),
    )
    .await
    .map(|_| ())
}

/// Command must be invoked in a guild/server.
pub(super) async fn require_guild(
    ctx: &Context,
    cmd: &CommandInteraction,
) -> Result<Option<GuildId>, Error> {
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
) -> Result<bool, Error> {
    let channel = cmd.channel_id.to_channel(ctx).await?;
    if let Some(guild_channel) = channel.guild() {
        if guild_channel.thread_metadata.is_some() {
            ephemeral(ctx, cmd, reason).await?;
            return Ok(true);
        }
    }
    Ok(false)
}
