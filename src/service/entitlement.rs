//! Service layer for the entitlements tied to purchased plans.

use crate::models::entitlement::EntitlementStore;
use crate::models::plan::Plan;
use crate::models::user_link::UserStore;
use crate::AppError;
use std::sync::Arc;

pub struct EntitlementService {
	entitlement_store: Arc<dyn EntitlementStore>,
	user_store: Arc<dyn UserStore>,
}

impl EntitlementService {
	pub const fn new(
		entitlement_store: Arc<dyn EntitlementStore>,
		user_store: Arc<dyn UserStore>,
	) -> Self {
		Self {
			entitlement_store,
			user_store,
		}
	}

	/// Follows a Discord ID to the respective plan we have recorded.
	pub async fn resolve_plan(&self, discord_id: u64) -> Result<Plan, AppError> {
		let Some(link) = self.user_store.fetch_by_discord_id(discord_id).await? else {
			return Ok(Plan::Basic);
		};
		let Some(github_user_id) = link.github_user_id else {
			return Ok(Plan::Basic);
		};
		let Some(ent) = self.entitlement_store.fetch_active(github_user_id).await? else {
			return Ok(Plan::Basic);
		};
		Ok(ent.plan)
	}
}
