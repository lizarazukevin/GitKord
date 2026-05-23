//! Setup Discord bot to start interpreting event interactions.

use serenity::all::{Context, EventHandler, Interaction, Ready};
use tracing::info;

use crate::discord::commands;
use crate::discord::models::ReadyHandler;

#[serenity::async_trait]
impl EventHandler for ReadyHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Discord bot connected as {}", ready.user.name);

        if let Err(e) = commands::register(&ctx).await {
            tracing::error!(error = %e, "failed to register slash commands");
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        commands::dispatch(&ctx, &interaction, &self.app_state).await;
    }
}
