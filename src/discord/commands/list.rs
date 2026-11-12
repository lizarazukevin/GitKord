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

/// Cap on rows rendered in one ephemeral reply.
/// Keep message well under Discord's 2000-character content limit.
const MAX_SUBSCRIPTIONS_TO_LIST: usize = 15;

/// Render a subscription list and reply ephemerally.
async fn reply_with_subs(
	ctx: &Context,
	cmd: &CommandInteraction,
	subs: Vec<Subscription>,
) -> Result<(), AppError> {
	ephemeral(ctx, cmd, &format_subscriptions(cmd.channel_id.get(), &subs)).await
}

/// Format the channel's subscriptions as a single `Discord` message.
fn format_subscriptions(channel_id: u64, subs: &[Subscription]) -> String {
	if subs.is_empty() {
		return format!("No repository subscriptions in <#{channel_id}> yet.");
	}

	// Ordering is a presentation concern; the query stays an unfiltered read.
	let mut sorted = subs.iter().collect::<Vec<_>>();
	sorted.sort_by_key(|sub| (&sub.owner, &sub.project));

	let body = sorted
		.iter()
		.take(MAX_SUBSCRIPTIONS_TO_LIST)
		.map(|sub| format_line(sub))
		.collect::<Vec<_>>()
		.join("\n");

	let hidden = sorted.len().saturating_sub(MAX_SUBSCRIPTIONS_TO_LIST);
	let footer = if hidden == 0 {
		String::new()
	} else {
		format!("\n\n-# *…and {hidden} more*")
	};

	format!("**Subscribed repositories in:  <#{channel_id}>**\n{body}{footer}")
}

/// One bullet row: repo, optional subscriber mention, and creation date.
fn format_line(sub: &Subscription) -> String {
	let mut parts = vec![format!(
		"[{}/{}](<https://github.com/{}/{}>)",
		sub.owner, sub.project, sub.owner, sub.project
	)];

	if let Some(mention) = subscriber_mention(sub.created_by.as_deref()) {
		parts.push(mention);
	}

	parts.push(sub.created_at.format("*%d %b %Y*").to_string());
	format!("- {}", parts.join("  **·**  "))
}

/// Render a `Discord` snowflake as a mention, or `None` when absent or not a
/// valid snowflake (pre-audit rows have no creator).
fn subscriber_mention(created_by: Option<&str>) -> Option<String> {
	created_by
		.and_then(|id| id.parse::<u64>().ok())
		.map(|id| format!("<@{id}>"))
}

fn list_command() -> CreateCommand {
	CreateCommand::new("list")
		.description("Choose which resource to list")
		.add_option(CreateCommandOption::new(
			CommandOptionType::SubCommand,
			"subscriptions",
			"List repositories you are subscribed to",
		))
}
