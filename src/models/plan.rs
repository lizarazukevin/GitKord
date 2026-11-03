//! Subscription plans and their repository caps for a Discord user.

pub enum Plan {
	Basic,
}

impl Plan {
	#[must_use]
	pub const fn max_subscriptions(self, basic_cap: u32) -> u32 {
		match self {
			Self::Basic => basic_cap,
		}
	}
}
