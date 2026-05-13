//! Serenity Discord client setup.
//!
//! Builds the client, registers the ready handler,
//! and exposes the `Http` handle so the application
//! can post messages without holding reference to the full client.

use octocrab::Octocrab;
use serenity::all::{Context, EventHandler, GatewayIntents, Http, Interaction, Ready};
use std::sync::Arc;
use tracing::info;

use crate::discord::commands;
use crate::state::traits::{SubscriptionStore, UserLinkStore};

// ── Client builder ────────────────────────────────────────────────────────────

/// Build a Serenity client and return it alongside a shared `Http` handle.
///
/// The `Http` handle is cloned out before `client.start()` is called so
/// the webhook handler can post messages independently of the gateway task.
pub async fn build(
    token: &str,
    sub_store: Arc<dyn SubscriptionStore>,
    user_store: Arc<dyn UserLinkStore>,
    github: Arc<Octocrab>,
    webhook_url: String,
    webhook_secret: String,
) -> (serenity::Client, Arc<Http>) {
    let intents = GatewayIntents::empty();

    let client = serenity::Client::builder(token, intents)
        .event_handler(ReadyHandler {
            sub_store,
            user_store,
            github,
            webhook_url,
            webhook_secret,
        })
        .await
        .expect("failed to build Discord client");

    // Clone the Http handle out before we move the client into its task.
    let http = Arc::clone(&client.http);

    (client, http)
}

// ── Event handler ─────────────────────────────────────────────────────────────

struct ReadyHandler {
    sub_store: Arc<dyn SubscriptionStore>,
    user_store: Arc<dyn UserLinkStore>,
    github: Arc<Octocrab>,
    webhook_url: String,
    webhook_secret: String,
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
        commands::dispatch(
            &ctx,
            &interaction,
            self.sub_store.as_ref(),
            self.user_store.as_ref(),
            &self.github,
            &self.webhook_url,
            &self.webhook_secret,
        )
        .await;
    }
}
