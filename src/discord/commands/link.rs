//! Link and unlink command handlers.
//! Uses GitHub app's device flow client to authorize users.

use crate::discord::commands::registry::CommandModule;
use crate::discord::commands::response::{
	defer_ephemeral, edit_ephemeral, ephemeral, require_guild,
};
use crate::error::format_error;
use crate::github::api::device_flow::DeviceFlowError;
use crate::service::discord::link::{
	DeviceCodeDisplay, LinkOutcome, StartLinkError, UserLinkAction, UserLinkRequest,
	UserLinkService,
};
use crate::AppError;
use anyhow::anyhow;
use async_trait::async_trait;
use serenity::all::{CommandInteraction, Context, CreateCommand};
use std::sync::Arc;

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
		if require_guild(ctx, cmd).await?.is_none() {
			return Ok(None);
		}
		let action = match cmd.data.name.as_str() {
			"link" => UserLinkAction::Link,
			"unlink" => UserLinkAction::Unlink,
			_ => return Ok(None),
		};
		Ok(Some(UserLinkRequest {
			discord_id: cmd.user.id.get(),
			action,
		}))
	}

	async fn handle_link(
		&self,
		ctx: &Context,
		cmd: &CommandInteraction,
		discord_id: u64,
	) -> Result<(), AppError> {
		defer_ephemeral(ctx, cmd).await?;

		let flow = match self.service.start_link().await {
			Ok(f) => f,
			Err(StartLinkError::Busy) => {
				edit_ephemeral(
					ctx,
					cmd,
					"Linking is busy right now. Try again in a minute.",
				)
				.await?;
				return Ok(());
			}
			Err(StartLinkError::ShuttingDown) => {
				edit_ephemeral(ctx, cmd, "GitKord is restarting. Try again in a minute.").await?;
				return Ok(());
			}
			Err(StartLinkError::Github(e)) => {
				let msg = if matches!(e, DeviceFlowError::Disabled) {
					"GitHub identity linking is not configured. Please open an issue if this is the case: <https://github.com/lizarazukevin/GitKord/issues/new>."
				} else {
					"Could not reach GitHub. Try again in a minute."
				};
				edit_ephemeral(ctx, cmd, msg).await?;
				return Err(AppError::Internal(anyhow!("github device flow start: {e}")));
			}
		};

		edit_ephemeral(ctx, cmd, &link_instruction_message(&flow.display)).await?;

		match self.service.complete_link(discord_id, flow).await {
			Ok(LinkOutcome::Success { login }) => {
				edit_ephemeral(ctx, cmd, &format!("✅ Linked to GitHub: `{login}`")).await
			}
			Ok(LinkOutcome::Expired) => {
				edit_ephemeral(ctx, cmd, "The GitHub code expired. Run `/link` again.").await
			}
			Ok(LinkOutcome::Denied) => {
				edit_ephemeral(ctx, cmd, "Authorization was denied on GitHub.").await
			}
			Ok(LinkOutcome::Timeout) => {
				edit_ephemeral(
					ctx,
					cmd,
					"Link request timed out. Run `/link` again to retry.",
				)
				.await
			}
			Ok(LinkOutcome::Shutdown) => Ok(()),
			Err(AppError::UserLinkConflict) => {
				edit_ephemeral(
					ctx,
					cmd,
					"That GitHub account is already linked to another Discord user.",
				)
				.await?;
				Err(AppError::UserLinkConflict)
			}
			Err(e) => {
				let user_msg = match &e {
					AppError::Message(_) => e.to_string(),
					_ => format_error("Something went wrong. Please try again.", None),
				};
				edit_ephemeral(ctx, cmd, &user_msg).await?;
				Err(e)
			}
		}
	}

	async fn handle_unlink(
		&self,
		ctx: &Context,
		cmd: &CommandInteraction,
		discord_id: u64,
	) -> Result<(), AppError> {
		match self.service.unlink(discord_id).await {
			Ok(()) => {
				ephemeral(
					ctx,
					cmd,
					"🔓 Your Discord to GitHub link has been **removed**.",
				)
				.await
			}
			Err(e) => {
				let user_msg = match &e {
					AppError::Message(_) => e.to_string(),
					_ => format_error("Something went wrong. Please try again.", None),
				};
				ephemeral(ctx, cmd, &user_msg).await?;
				Err(e)
			}
		}
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
		match req.action {
			UserLinkAction::Link => self.handle_link(ctx, cmd, req.discord_id).await,
			UserLinkAction::Unlink => self.handle_unlink(ctx, cmd, req.discord_id).await,
		}
	}
}

pub fn link_command() -> CreateCommand {
	CreateCommand::new("link").description("Link your Discord account to a GitHub account")
}

pub fn unlink_command() -> CreateCommand {
	CreateCommand::new("unlink").description("Remove your Discord to GitHub account link")
}

fn link_instruction_message(display: &DeviceCodeDisplay) -> String {
	format!(
		"To link your GitHub account (expires in **{minutes} minutes**):\n\n\
		 **1.** Open <{uri}>\n\
		 **2.** Enter code: `{code}`\n\n\
		 -# *⚠️ Only enter a code shown in this message. Never enter a code someone else sent you.*",
		uri = display.verification_uri,
		code = display.user_code,
		minutes = display.expires_in / 60,
	)
}
