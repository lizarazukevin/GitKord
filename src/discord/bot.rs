//! Serenity client setup for Discord.
//!
//! Builds the client, registers the slash commands on ready,
//! and exposes the `Http` handle so the webhook handler can
//! post messages without holding a reference to the full client.

use serenity::all::{Context, EventHandler, GatewayIntents, Http, Interaction, Ready};
use std::sync::Arc;
use tracing::info;

use crate::discord::commands;
use crate::discord::context::AppState;

/// Event handler stored in the Serenity client.
struct ReadyHandler {
    app_state: AppState,
}

/// Build a Serenity client and return it alongside a shared `Http` handle.
///
/// The `Http` handle is cloned out before client moves into its task so
/// the webhook handler can post messages independently of the gateway connection.
pub async fn build(token: &str, app_state: AppState) -> (serenity::Client, Arc<Http>) {
    let intents = GatewayIntents::empty();

    let client = serenity::Client::builder(token, intents)
        .event_handler(ReadyHandler { app_state })
        .await
        .expect("failed to build Discord client");

    let http = Arc::clone(&client.http);
    (client, http)
}

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
