//! Link and unlink command handlers.

use crate::discord::commands::args::{string_option, CommandOption};
use crate::discord::commands::registry::CommandModule;
use crate::discord::commands::response::{ephemeral, require_guild};
use crate::error::format_error;
use crate::service::discord::link::{LinkAction, UserLinkRequest, UserLinkService};
use crate::AppError;
use async_trait::async_trait;
use serenity::all::{
	CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
};
use std::sync::Arc;
use tracing::error;

pub(super) struct UserLinkModule {
	service: Arc<UserLinkService>,
}

impl UserLinkModule {
	pub(super) const fn new(service: Arc<UserLinkService>) -> Self {
		Self { service }
	}

	async fn parse_link_input(
		&self,
		ctx: &Context,
		cmd: &CommandInteraction,
	) -> Result<Option<UserLinkRequest>, AppError> {
		require_guild(ctx, cmd).await?;

		let action = match cmd.data.name.as_str() {
			"link" => LinkAction::Link,
			"unlink" => LinkAction::Unlink,
			_ => return Ok(None),
		};

		let github_login = match action {
			LinkAction::Link => {
				if let CommandOption::Valid(login) = string_option(cmd, "username") {
					login
				} else {
					ephemeral(ctx, cmd, "Provide a valid GitHub username.").await?;
					return Ok(None);
				}
			}
			LinkAction::Unlink => String::new(),
		};

		Ok(Some(UserLinkRequest {
			github_login,
			discord_id: cmd.user.id.get(),
			action,
		}))
	}
}

#[async_trait]
impl CommandModule for UserLinkModule {
	fn commands(&self) -> Vec<CreateCommand> {
		vec![link_command(), unlink_command()]
	}

	fn names(&self) -> &'static [&'static str] {
		&["link", "unlink"]
	}

	async fn execute(&self, ctx: &Context, cmd: &CommandInteraction) -> Result<(), AppError> {
		let Some(req) = self.parse_link_input(ctx, cmd).await? else {
			return Ok(());
		};

		match self.service.handle(req).await {
			Ok(msg) => ephemeral(ctx, cmd, &msg).await,
			Err(e) => {
				error!(error = %e, "link service failed");
				let user_msg = match &e {
					AppError::Message(_) => e.to_string(),
					_ => format_error("Something went wrong. Please try again.", None),
				};
				ephemeral(ctx, cmd, &user_msg).await
			}
		}
	}
}

pub fn link_command() -> CreateCommand {
	CreateCommand::new("link")
		.description("Link your Discord account to a GitHub username")
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"username",
				"Your GitHub username",
			)
			.required(true),
		)
}

pub fn unlink_command() -> CreateCommand {
	CreateCommand::new("unlink").description("Remove your Discord to GitHub account link")
}
