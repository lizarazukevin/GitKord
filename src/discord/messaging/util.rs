//! Small utility functions shared across the messaging module.

/// Current UTC timestamp formatted for `Discord` message footers.
pub fn now_utc() -> String {
	chrono::Utc::now()
		.format("%d %b %Y at %H:%M UTC")
		.to_string()
}
