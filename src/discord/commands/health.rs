//! Health check command handler.

use crate::config::APP_NAME;
use crate::discord::commands::registry::CommandModule;
use crate::discord::commands::response::{ephemeral, require_guild};
use crate::service::discord::health::HealthService;
use crate::AppError;
use anyhow::Result;
use async_trait::async_trait;
use serenity::all::{CommandInteraction, Context, CreateCommand};
use std::sync::Arc;

pub(super) struct HealthModule {
	service: Arc<HealthService>,
}

impl HealthModule {
	pub(super) const fn new(service: Arc<HealthService>) -> Self {
		Self { service }
	}
}

#[async_trait]
impl CommandModule for HealthModule {
	fn commands(&self) -> Vec<CreateCommand> {
		vec![health_command()]
	}

	fn names(&self) -> &'static [&'static str] {
		&["health"]
	}

	async fn execute(&self, ctx: &Context, cmd: &CommandInteraction) -> Result<(), AppError> {
		require_guild(ctx, cmd).await?;

		let msg = self.service.handle().await;
		ephemeral(ctx, cmd, &msg).await
	}
}

fn health_command() -> CreateCommand {
	CreateCommand::new("health").description(format!("Check if {APP_NAME} is running"))
}
