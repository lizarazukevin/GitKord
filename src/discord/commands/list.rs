//! `/list` slash command: subcommand router and per-resource query composition root.

use async_trait::async_trait;
use serenity::all::{
	CommandDataOption, CommandDataOptionValue, CommandInteraction, CommandOptionType, Context,
	CreateCommand, CreateCommandOption,
};

use crate::discord::commands::registry::CommandModule;
use crate::discord::commands::response::ephemeral;
use crate::models::subscription::Subscription;
use crate::service::discord::list::ListSubscriptionsQuery;
use crate::AppError;

pub(super) struct ListModule {
	subscriptions: ListSubscriptionsQuery,
}

impl ListModule {
	pub(super) const fn new(subscriptions: ListSubscriptionsQuery) -> Self {
		Self { subscriptions }
	}
}

#[async_trait]
impl CommandModule for ListModule {
	fn commands(&self) -> Vec<CreateCommand> {
		vec![list_command()]
	}

	fn names(&self) -> &'static [&'static str] {
		&["list"]
	}

	async fn execute(&self, ctx: &Context, cmd: &CommandInteraction) -> Result<(), AppError> {
		let (sub, _opts) = match cmd.data.options.first() {
			Some(CommandDataOption {
				name,
				value: CommandDataOptionValue::SubCommand(options),
				..
			}) => (name.as_str(), options.as_slice()),
			_ => return Err(AppError::Message("expected a /list subcommand".into())),
		};

		match sub {
			"subscriptions" => {
				let subs = self.subscriptions.run(cmd.channel_id.get()).await?;
				reply_with_subs(ctx, cmd, subs).await
			}
			other => Err(AppError::Message(format!(
				"unknown /list subcommand: {other}"
			))),
		}
	}
}

/// Render a subscription list and reply ephemerally.
async fn reply_with_subs(
	ctx: &Context,
	cmd: &CommandInteraction,
	subs: Vec<Subscription>,
) -> Result<(), AppError> {
	ephemeral(ctx, cmd, &format_subscriptions(&subs)).await
}

/// Format channel subscriptions into a user-facing `Discord` message.
fn format_subscriptions(subs: &[Subscription]) -> String {
	if subs.is_empty() {
		return "No repository subscriptions in this channel.".into();
	}

	let repos = subs
		.iter()
		.map(|sub| format!("- `{}/{}`", sub.owner, sub.project))
		.collect::<Vec<_>>()
		.join("\n");

	format!("**Repository subscriptions in this channel**\n{repos}")
}

fn list_command() -> CreateCommand {
	CreateCommand::new("list")
		.description("Choose which resource to list")
		.add_option(CreateCommandOption::new(
			CommandOptionType::SubCommand,
			"subscriptions",
			"List repositories this channel is subscribed to",
		))
}
