//! Serenity Discord client setup.
//!
//! Builds the client, registers the ready handler,
//! and exposes the `Http` handle so the application
//! can post messages without holding reference to the full client.

use std::sync::Arc;

use serenity::all::{Context, EventHandler, GatewayIntents, Http, Interaction, Ready};
use tracing::info;

use crate::discord::commands;
use crate::state::traits::UserLinkStore;

// ── Client builder ────────────────────────────────────────────────────────────

/// Build a Serenity client and return it alongside a shared `Http` handle.
///
/// The `Http` handle is cloned out before `client.start()` is called so
/// the webhook handler can post messages independently of the gateway task.
pub async fn build(
    token: &str,
    user_store: Arc<dyn UserLinkStore>,
) -> (serenity::Client, Arc<Http>) {
    let intents = GatewayIntents::empty();

    let client = serenity::Client::builder(token, intents)
        .event_handler(ReadyHandler { user_store })
        .await
        .expect("failed to build Discord client");

    // Clone the Http handle out before we move the client into its task.
    let http = Arc::clone(&client.http);

    (client, http)
}

// ── Event handler ─────────────────────────────────────────────────────────────

struct ReadyHandler {
    user_store: Arc<dyn UserLinkStore>,
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
        commands::dispatch(&ctx, &interaction, self.user_store.as_ref()).await;
    }
}
