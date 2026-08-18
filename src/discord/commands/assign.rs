//! Assign and unassign command handlers.

pub use crate::discord::commands::args::CommandOption;
use crate::discord::commands::args::{number_option, repo_name_option, resolve_reviewer_option};
use crate::discord::commands::registry::CommandModule;
use crate::discord::commands::response::{deferred_ephemeral, ephemeral, require_guild};
use crate::error::format_error;
use crate::service::discord::assign::{AssignAction, AssignRequest, AssignService};
use crate::AppError;
use anyhow::Result;
use async_trait::async_trait;
use serenity::all::{
	CommandInteraction, CommandOptionType, Context, CreateCommand, CreateCommandOption,
};
use std::sync::Arc;
use tracing::error;

pub(super) struct AssignModule {
	service: Arc<AssignService>,
}

impl AssignModule {
	pub(super) const fn new(service: Arc<AssignService>) -> Self {
		Self { service }
	}

	async fn parse_assign_input(
		&self,
		ctx: &Context,
		cmd: &CommandInteraction,
	) -> Result<Option<AssignRequest>, AppError> {
		require_guild(ctx, cmd).await?;

		let action = match cmd.data.name.as_str() {
			"assign" => AssignAction::Assign,
			"unassign" => AssignAction::Unassign,
			_ => return Ok(None),
		};

		let repo = match repo_name_option(cmd, "repository") {
			CommandOption::Valid(r) => Some(r),
			CommandOption::Invalid => {
				ephemeral(ctx, cmd, "Repository must be in `owner/name` format.").await?;
				return Ok(None);
			}
			CommandOption::Missing => None,
		};

		let pr = match number_option(cmd, "pr") {
			CommandOption::Valid(n) => Some(n),
			CommandOption::Invalid => {
				ephemeral(ctx, cmd, "Pull request number is invalid.").await?;
				return Ok(None);
			}
			CommandOption::Missing => None,
		};

		let CommandOption::Valid(reviewer) = resolve_reviewer_option(cmd, "reviewer") else {
			ephemeral(
				ctx,
				cmd,
				"Provide a valid GitHub username or Discord mention.",
			)
			.await?;
			return Ok(None);
		};

		Ok(Some(AssignRequest {
			reviewer,
			action,
			actor: cmd.user.name.clone(),
			repo,
			pr,
			channel_id: cmd.channel_id.get(),
		}))
	}
}

#[async_trait]
impl CommandModule for AssignModule {
	fn commands(&self) -> Vec<CreateCommand> {
		vec![assign_command(), unassign_command()]
	}

	fn names(&self) -> &'static [&'static str] {
		&["assign", "unassign"]
	}

	async fn execute(&self, ctx: &Context, cmd: &CommandInteraction) -> Result<(), AppError> {
		let Some(req) = self.parse_assign_input(ctx, cmd).await? else {
			return Ok(());
		};

		cmd.defer_ephemeral(ctx)
			.await
			.map_err(|e| AppError::Discord(Arc::new(e)))?;

		match self.service.handle(req, &ctx.http).await {
			Ok(msg) => deferred_ephemeral(ctx, cmd, &msg).await,
			Err(e) => {
				error!(error = %e, "assign service failed");
				let user_msg = match &e {
					AppError::Message(_) => e.to_string(),
					_ => format_error("Something went wrong. Please try again.", None),
				};
				deferred_ephemeral(ctx, cmd, &user_msg).await
			}
		}
	}
}

fn assign_command() -> CreateCommand {
	CreateCommand::new("assign")
		.description(
			"Request review on a PR. Run inside a PR thread to skip `repository` and `pr`.",
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"reviewer",
				"GitHub username or @Discord mention",
			)
			.required(true),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"repository",
				"Repository in owner/name format (not needed inside a PR thread)",
			)
			.required(false),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::Integer,
				"pr",
				"PR number (not needed inside a PR thread)",
			)
			.required(false),
		)
}

fn unassign_command() -> CreateCommand {
	CreateCommand::new("unassign")
		.description(
			"Remove a review request. Run inside a PR thread to skip `repository` and `pr`.",
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"reviewer",
				"Github username or @Discord mention",
			)
			.required(true),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::String,
				"repository",
				"Repository in owner/name format (not needed inside a PR thread)",
			)
			.required(false),
		)
		.add_option(
			CreateCommandOption::new(
				CommandOptionType::Integer,
				"pr",
				"PR number (not needed inside a PR thread)",
			)
			.required(false),
		)
}
