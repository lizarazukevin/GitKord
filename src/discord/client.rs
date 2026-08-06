//! Setup `Discord` bot gateway connection and event handling.

use crate::discord::commands::registry::CommandRegistry;
use crate::error::AppError;
use serenity::all::{Context, EventHandler, GatewayIntents, Http, Interaction, Ready};
use serenity::Client;
use std::sync::Arc;
use tracing::info;

struct BotEventHandler {
	registry: CommandRegistry,
}

#[serenity::async_trait]
impl EventHandler for BotEventHandler {
	async fn ready(&self, ctx: Context, ready: Ready) {
		info!("{} is connected!", ready.user.name);

		if let Err(e) = self.registry.register_all(&ctx).await {
			tracing::error!(error = %e, "failed to register slash registry");
		}
	}

	async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
		self.registry.dispatch(&ctx, &interaction).await;
	}
}

/// Build a `Serenity` client and returns its shared Http client.
///
/// The returned `Arc<Http>` is used by `GitHub` webhook handlers to send and
/// edit `Discord` messages without needing access to the running gateway client.
pub async fn build(
	token: &str,
	commands: CommandRegistry,
) -> Result<(Client, Arc<Http>), AppError> {
	let intents = GatewayIntents::empty();

	let client = Client::builder(token, intents)
		.event_handler(BotEventHandler { registry: commands })
		.await
		.map_err(|e| AppError::Discord(Arc::new(e)))?;

	let http = Arc::clone(&client.http);
	Ok((client, http))
}
