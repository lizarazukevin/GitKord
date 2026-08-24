//! Structured log context: classification, context, and builder.
#![allow(dead_code)]

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

/// Fluent builder for a `(EventKind, name, LogContext)` triple.
#[derive(Debug, Clone)]
pub struct LogBuilder {
	kind: EventKind,
	name: String,
	context: LogContext,
}

impl LogContext {
	/// Start a builder for a webhook event.
	#[must_use]
	pub fn webhook(event: &str) -> LogBuilder {
		LogBuilder::new(EventKind::Webhook, event)
	}

	/// Start a builder for a Discord command.
	#[must_use]
	pub fn command(name: &str) -> LogBuilder {
		LogBuilder::new(EventKind::Command, name)
	}

	/// Start a builder for an HTTP request.
	#[must_use]
	pub fn http_request(method: &str, path: &str) -> LogBuilder {
		LogBuilder::new(EventKind::HttpRequest, &format!("{method} {path}"))
	}
}

impl LogBuilder {
	fn new(kind: EventKind, name: &str) -> Self {
		Self {
			kind,
			name: name.to_owned(),
			context: LogContext::default(),
		}
	}

	/// Set the repository as `owner/name`.
	#[must_use]
	pub fn repo(mut self, owner: &str, name: &str) -> Self {
		self.context.repository = Some(format!("{owner}/{name}"));
		self
	}

	/// Set the pull request number.
	#[must_use]
	pub const fn pr(mut self, number: u64) -> Self {
		self.context.pr_number = Some(number);
		self
	}

	/// Set the GitHub user login (sender, reviewer, or actor).
	#[must_use]
	pub fn sender(mut self, login: &str) -> Self {
		self.context.github_user = Some(login.to_owned());
		self
	}

	/// Set the Discord user snowflake ID.
	#[must_use]
	pub const fn user_id(mut self, id: u64) -> Self {
		self.context.discord_user_id = Some(id);
		self
	}

	/// Set the Discord channel ID.
	#[must_use]
	pub const fn channel(mut self, id: u64) -> Self {
		self.context.channel_id = Some(id);
		self
	}

	/// Set the Discord guild (server) ID.
	#[must_use]
	pub const fn guild(mut self, id: u64) -> Self {
		self.context.guild_id = Some(id);
		self
	}

	/// Set the Discord thread ID (PR audit thread).
	#[must_use]
	pub const fn thread(mut self, id: u64) -> Self {
		self.context.thread_id = Some(id);
		self
	}

	/// Set the GitHub App installation ID.
	#[must_use]
	pub const fn installation(mut self, id: u64) -> Self {
		self.context.installation_id = Some(id);
		self
	}

	/// Set the raw slash command arguments.
	#[must_use]
	pub fn command_args(mut self, args: &str) -> Self {
		self.context.command_args = Some(args.to_owned());
		self
	}

	/// Finalize the builder into the `(kind, name, context)` triple.
	#[must_use]
	pub fn build(self) -> (EventKind, String, LogContext) {
		(self.kind, self.name, self.context)
	}
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
