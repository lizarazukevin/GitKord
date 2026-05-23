//! Health check command.
//!
//! Simple liveness probe so users can confirm the bot is online
//! without triggering any external API calls.

use crate::discord::commands::shared::ephemeral;
use serenity::all::{CommandInteraction, Context};

/// Respond with a simple online confirmation message.
pub async fn handle_health(ctx: &Context, cmd: &CommandInteraction) -> Result<(), serenity::Error> {
    ephemeral(ctx, cmd, "`GitKord` is online and healthy.").await
}
