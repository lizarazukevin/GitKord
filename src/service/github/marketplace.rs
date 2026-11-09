//! Service layer for marketplace events for the GitHub app.

use crate::github::webhook::events::marketplace_purchase::MarketplacePurchasePayload;
use crate::models::entitlement::{Entitlement, EntitlementStatus, EntitlementStore};
use crate::models::plan::Plan;
use crate::AppError;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplaceAction {
	Purchased,
	Changed,
	Cancelled,
	PendingChange,
	PendingChangeCancelled,
	Other,
}

pub struct MarketplaceRequest {
	pub action: MarketplaceAction,
	pub account_id: i64,
	pub account_login: String,
	pub plan_name: String,
	pub effective_date: DateTime<Utc>,
	pub sender: String,
}

impl MarketplaceRequest {
	pub fn from_payload(payload: MarketplacePurchasePayload) -> Result<Self, AppError> {
		let account = payload.marketplace_purchase.account;

		let effective_date = DateTime::parse_from_rfc3339(&payload.effective_date)
			.map(|dt| dt.with_timezone(&Utc))
			.map_err(|e| {
				AppError::Internal(anyhow::anyhow!(
					"invalid effective_date {:?}: {e}",
					payload.effective_date
				))
			})?;

		let action = match payload.action.as_str() {
			"purchased" => MarketplaceAction::Purchased,
			"changed" => MarketplaceAction::Changed,
			"cancelled" => MarketplaceAction::Cancelled,
			"pending_change" => MarketplaceAction::PendingChange,
			"pending_change_cancelled" => MarketplaceAction::PendingChangeCancelled,
			other => {
				warn!(action = other, "unknown marketplace action, will ignore");
				MarketplaceAction::Other
			}
		};

		Ok(Self {
			action,
			account_id: account.id,
			account_login: account.login,
			plan_name: payload.marketplace_purchase.plan.name,
			effective_date,
			sender: payload.sender.login,
		})
	}
}

pub struct MarketplaceService {
	entitlement_store: Arc<dyn EntitlementStore>,
}

impl MarketplaceService {
	pub const fn new(entitlement_store: Arc<dyn EntitlementStore>) -> Self {
		Self { entitlement_store }
	}

	pub async fn handle(&self, req: MarketplaceRequest) -> Result<(), AppError> {
		match req.action {
			MarketplaceAction::Purchased | MarketplaceAction::Changed => {
				self.record_active(req).await
			}
			MarketplaceAction::Cancelled => self.record_cancellation(req).await,
			MarketplaceAction::PendingChange
			| MarketplaceAction::PendingChangeCancelled
			| MarketplaceAction::Other => Ok(()),
		}
	}

	/// New entry or update to active plan has been made.
	async fn record_active(&self, req: MarketplaceRequest) -> Result<(), AppError> {
		let Some(plan) = Self::marketplace_plan(&req.plan_name) else {
			warn!(
				plan_name = %req.plan_name,
				account_id = req.account_id,
				"unknown marketplace plan name, ignoring event"
			);
			return Ok(());
		};

		self.entitlement_store
			.upsert(Self::build(&req, plan, EntitlementStatus::Active, None))
			.await?;

		info!(
			account_id = req.account_id,
			account = %req.account_login,
			plan = plan.as_str(),
			"marketplace purchase recorded"
		);

		Ok(())
	}

	/// User decides to cancel their plan, updating with effective until dates.
	async fn record_cancellation(&self, req: MarketplaceRequest) -> Result<(), AppError> {
		let plan = match Self::marketplace_plan(&req.plan_name) {
			Some(plan) => plan,
			None => {
				if let Some(existing) = self.entitlement_store.fetch_active(req.account_id).await? {
					existing.plan
				} else {
					warn!(
						plan_name = %req.plan_name,
						account_id = req.account_id,
						"cancellation for unrecognized plan with no prior entitlement, ignoring"
					);
					return Ok(());
				}
			}
		};

		// Future-dated cancellation: the user is entitled until that date.
		// `fetch_active` filters on `effective_until > NOW()`, so this row
		// naturally stops being active when the date passes.
		let (status, effective_until) = if req.effective_date > Utc::now() {
			(EntitlementStatus::Active, Some(req.effective_date))
		} else {
			(EntitlementStatus::Cancelled, None)
		};

		self.entitlement_store
			.upsert(Self::build(&req, plan, status, effective_until))
			.await?;

		info!(
			account_id = req.account_id,
			account = %req.account_login,
			plan = plan.as_str(),
			status = status.as_str(),
			effective_until = ?effective_until,
			"marketplace cancellation recorded"
		);
		Ok(())
	}

	fn marketplace_plan(name: &str) -> Option<Plan> {
		match name.to_ascii_lowercase().as_str() {
			"basic" => Some(Plan::Basic),
			"premium" => Some(Plan::Premium),
			_ => None,
		}
	}

	fn build(
		req: &MarketplaceRequest,
		plan: Plan,
		status: EntitlementStatus,
		effective_until: Option<DateTime<Utc>>,
	) -> Entitlement {
		let now = Utc::now();
		Entitlement {
			github_account_id: req.account_id,
			plan,
			status,
			effective_until,
			received_at: now,
			created_at: now,
			updated_at: now,
			created_by: req.sender.clone(),
			updated_by: req.sender.clone(),
		}
	}
}
