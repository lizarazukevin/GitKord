//! Serenity Discord client setup.
//!
//! Builds the client, registers the ready handler,
//! and exposes the `Http` handle so the application
//! can post messages without holding reference to the full client.

use std::sync::Arc;

use serenity::all::{Context, EventHandler, GatewayIntents, Http, Ready};
use tracing::info;

// ── Client builder ────────────────────────────────────────────────────────────

/// Build a Serenity client and return it alongside a shared `Http` handle.
///
/// The `Http` handle is cloned out before `client.start()` is called so
/// the webhook handler can post messages independently of the gateway task.
pub async fn build(token: &str) -> (serenity::Client, Arc<Http>) {
    let intents = GatewayIntents::empty();

    let client = serenity::Client::builder(token, intents)
        .event_handler(ReadyHandler)
        .await
        .expect("failed to build Discord client");

    // Clone the Http handle out before we move the client into its task.
    let http = Arc::clone(&client.http);

    (client, http)
}

// ── Event handler ─────────────────────────────────────────────────────────────

struct ReadyHandler;

#[serenity::async_trait]
impl EventHandler for ReadyHandler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("Discord bot connected as {}", ready.user.name);
    }
}
