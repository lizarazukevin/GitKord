//! One-store-wide read query for subscriptions.

use std::sync::Arc;

use crate::models::subscription::{Subscription, SubscriptionStore};
use crate::AppError;

pub struct ListSubscriptionsQuery {
	store: Arc<dyn SubscriptionStore>,
}

impl ListSubscriptionsQuery {
	pub const fn new(store: Arc<dyn SubscriptionStore>) -> Self {
		Self { store }
	}

	pub async fn run(&self, channel_id: u64) -> Result<Vec<Subscription>, AppError> {
		self.store
			.as_ref()
			.fetch_all_by_channel_id(channel_id)
			.await
	}
}
