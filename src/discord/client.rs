//! Serenity client setup for Discord.
//!
//! Builds the client, registers the slash commands on ready,
//! and exposes the `Http` handle so the webhook handler can
//! post messages without holding a reference to the full client.

use crate::discord::context::AppState;
use crate::discord::models::ReadyHandler;
use serenity::all::{GatewayIntents, Http};
use std::sync::Arc;

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
