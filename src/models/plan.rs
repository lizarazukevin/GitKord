//! Subscription plans and their repository caps for a Discord user.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
	Basic,
	Premium,
	Admin,
}

impl Plan {
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Basic => "basic",
			Self::Premium => "premium",
			Self::Admin => "admin",
		}
	}

	#[must_use]
	pub const fn max_subscriptions(self, basic_cap: u32, premium_cap: u32) -> Option<u32> {
		match self {
			Self::Basic => Some(basic_cap),
			Self::Premium => Some(premium_cap),
			Self::Admin => None,
		}
	}

	#[must_use]
	pub fn from_stored(s: &str) -> Self {
		match s.to_ascii_lowercase().as_str() {
			"premium" => Self::Premium,
			"admin" => Self::Admin,
			_ => Self::Basic,
		}
	}
}
