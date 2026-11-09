//! `GitHub` Marketplace webhook event handler.
//!
//! GitHub delivers Marketplace lifecycle events (purchase, cancellation,
//! plan changes, etc.) to the App's webhook with an
//! `X-GitHub-Event: marketplace_purchase` header.

use crate::error::AppError;
use crate::github::webhook::events::models::GitHubEvent;
use crate::github::webhook::router::WebhookEventHandler;
use crate::service::github::marketplace::{MarketplaceRequest, MarketplaceService};
use async_trait::async_trait;
use axum::body::Bytes;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct MarketplacePurchasePayload {
	/// The lifecycle action: `purchased`, `cancelled`, `pending_change`,
	/// `pending_change_cancelled`, or `changed`.
	pub action: String,
	/// RFC 3339 timestamp (e.g. `2017-10-05T00:00:00+00:00`) for when the
	/// change takes effect.
	pub effective_date: String,
	/// The GitHub user who triggered the change.
	pub sender: MarketplaceSender,
	/// The purchase that the action refers to.
	pub marketplace_purchase: MarketplacePurchase,
}

/// The GitHub user who triggered a Marketplace event.
#[derive(Debug, Deserialize)]
pub struct MarketplaceSender {
	pub login: String,
}

/// The current state of a subscription to the App on the Marketplace.
#[derive(Debug, Clone, Deserialize)]
pub struct MarketplacePurchase {
	pub account: MarketplaceAccount,
	#[allow(dead_code)]
	pub billing_cycle: String,
	#[allow(dead_code)]
	pub unit_count: u64,
	#[allow(dead_code)]
	pub on_free_trial: bool,
	#[allow(dead_code)]
	pub free_trial_ends_on: Option<String>,
	pub plan: MarketplacePlan,
}

/// The account the Marketplace subscription belongs to.
#[derive(Debug, Clone, Deserialize)]
pub struct MarketplaceAccount {
	pub login: String,
	pub id: i64,
}

/// The selected Marketplace plan.
#[derive(Debug, Clone, Deserialize)]
pub struct MarketplacePlan {
	#[allow(dead_code)]
	pub id: u64,
	pub name: String,
	#[allow(dead_code)]
	pub description: String,
	#[allow(dead_code)]
	pub price_model: String,
	#[allow(dead_code)]
	pub monthly_price_in_cents: u64,
	#[allow(dead_code)]
	pub yearly_price_in_cents: u64,
}

/// Acknowledges Marketplace events. Lightweight receiver (not fully implemented),
/// validates the signature and payload, records the purchase change to the
/// logs, and returns. This is enough for GitHub to verify the webhook when
/// listing the App, and gives a clear place to attach billing logic later.
pub struct MarketplacePurchaseEventHandler {
	service: Arc<MarketplaceService>,
}

impl MarketplacePurchaseEventHandler {
	pub const fn new(service: Arc<MarketplaceService>) -> Self {
		Self { service }
	}
}

#[async_trait]
impl WebhookEventHandler for MarketplacePurchaseEventHandler {
	fn event_type(&self) -> GitHubEvent {
		GitHubEvent::MarketplacePurchase
	}

	async fn execute(&self, body: Bytes) -> Result<(), AppError> {
		let payload: MarketplacePurchasePayload = serde_json::from_slice(&body)?;
		let req = MarketplaceRequest::from_payload(payload)?;
		self.service.handle(req).await
	}
}
