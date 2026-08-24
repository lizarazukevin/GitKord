//! Structured log context: classification, context, and builder.

use serenity::all::CommandDataOption;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
	/// A Discord slash command (e.g. `/assign`, `/subscribe`).
	Command,
	/// A GitHub webhook event (e.g. `pull_request`, `review`).
	Webhook,
	/// An HTTP request handled by the Axum server.
	HttpRequest,
}

impl EventKind {
	#[must_use]
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Command => "command",
			Self::Webhook => "webhook",
			Self::HttpRequest => "http_request",
		}
	}
}

/// Domain context attached to a log span.
///
/// All fields are optional so a handler can attach only what it knows.
/// `Default` yields an empty context, useful for cases like deserialization
/// failures where only the event name is available.
#[derive(Debug, Clone, Default)]
pub struct LogContext {
	/// Repository in `owner/name` form.
	pub repository: Option<String>,
	/// Pull request number.
	pub pr_number: Option<u64>,
	/// GitHub user login (sender, reviewer, or actor).
	pub github_user: Option<String>,
	/// Discord user snowflake ID (stable; usernames can change).
	pub discord_user_id: Option<u64>,
	/// Discord channel ID.
	pub channel_id: Option<u64>,
	/// Discord guild (server) ID.
	pub guild_id: Option<u64>,
	/// Discord thread ID (PR audit thread).
	pub thread_id: Option<u64>,
	/// GitHub App installation ID.
	pub installation_id: Option<u64>,
	/// Raw slash command arguments (e.g. `repository=owner/name, pr=42`).
	pub command_args: Option<String>,
}

/// Format slash command options as a compact `name=value` string for logging.
///
/// Uses each option's `Debug` representation, so complex values (mentions,
/// sub-commands, etc.) remain distinguishable without manual formatting.
/// Sub-command/group options are flattened inline. Returns an empty string
/// when the command has no options.
pub fn format_command_args(options: &[CommandDataOption]) -> String {
	options
		.iter()
		.map(|opt| format!("{}={:?}", opt.name, opt.value))
		.collect::<Vec<_>>()
		.join(", ")
}
