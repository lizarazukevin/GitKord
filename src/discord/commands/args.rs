//! Command argument extraction utilities.

use serenity::all::CommandInteraction;

pub(crate) enum CommandOption<T> {
	Valid(T),
	Invalid,
	Missing,
}

/// Parse a string option from user input.
pub(super) fn string_option(cmd: &CommandInteraction, name: &str) -> CommandOption<String> {
	let Some(option) = cmd.data.options.iter().find(|o| o.name == name) else {
		return CommandOption::Missing;
	};

	let Some(value) = option.value.as_str() else {
		return CommandOption::Invalid;
	};

	CommandOption::Valid(value.to_owned())
}

/// Parse an integer option from user input.
pub(super) fn number_option(cmd: &CommandInteraction, name: &str) -> CommandOption<u64> {
	let Some(option) = cmd.data.options.iter().find(|o| o.name == name) else {
		return CommandOption::Missing;
	};

	let Some(value) = option.value.as_i64() else {
		return CommandOption::Invalid;
	};

	CommandOption::Valid(value as u64)
}

/// Parse a valid repository name from user input.
pub(super) fn repo_name_option(cmd: &CommandInteraction, name: &str) -> CommandOption<String> {
	let Some(option) = cmd.data.options.iter().find(|o| o.name == name) else {
		return CommandOption::Missing;
	};

	let Some(value) = option.value.as_str() else {
		return CommandOption::Invalid;
	};

	let value = value.to_lowercase();

	let mut parts = value.split('/');

	let (Some(owner), Some(repo), None) = (parts.next(), parts.next(), parts.next()) else {
		return CommandOption::Invalid;
	};

	if owner.is_empty() || repo.is_empty() {
		return CommandOption::Invalid;
	}

	CommandOption::Valid(value)
}

pub(crate) enum ReviewerInput {
	GitHubLogin(String),
	DiscordMention(u64),
}

/// Parse either a `GitHub` login or `Discord` mention from user input.
pub(super) fn resolve_reviewer_option(
	cmd: &CommandInteraction,
	name: &str,
) -> CommandOption<ReviewerInput> {
	let raw = match string_option(cmd, name) {
		CommandOption::Valid(s) => s,
		CommandOption::Invalid => return CommandOption::Invalid,
		CommandOption::Missing => return CommandOption::Missing,
	};

	if let Some(mention) = raw.strip_prefix("<@") {
		let id_str = mention.strip_prefix("!").unwrap_or(mention);
		let Some(discord_id) = id_str.strip_suffix(">").and_then(|s| s.parse::<u64>().ok()) else {
			return CommandOption::Invalid;
		};
		return CommandOption::Valid(ReviewerInput::DiscordMention(discord_id));
	}

	CommandOption::Valid(ReviewerInput::GitHubLogin(raw))
}
