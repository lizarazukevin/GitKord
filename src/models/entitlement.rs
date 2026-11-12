//! Domain model and traits for entitlement plans.

use crate::models::plan::Plan;
use crate::AppError;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntitlementStatus {
	/// User is entitled to `plan` until `effective_until` (if set).
	Active,
	/// Explicitly canceled, no entitlement.
	Cancelled,
}

impl EntitlementStatus {
	pub const fn as_str(self) -> &'static str {
		match self {
			Self::Active => "active",
			Self::Cancelled => "cancelled",
		}
	}
}

#[derive(Debug, Clone)]
pub struct Entitlement {
	pub github_account_id: i64,
	pub plan: Plan,
	pub status: EntitlementStatus,
	pub effective_until: Option<DateTime<Utc>>,
	#[allow(dead_code)]
	pub received_at: DateTime<Utc>,
	#[allow(dead_code)]
	pub created_at: DateTime<Utc>,
	#[allow(dead_code)]
	pub updated_at: DateTime<Utc>,
	#[allow(dead_code)]
	pub created_by: String,
	#[allow(dead_code)]
	pub updated_by: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
	Inserted,
	Updated,
}

#[async_trait::async_trait]
pub trait EntitlementStore: Send + Sync {
	async fn upsert(&self, e: Entitlement) -> Result<UpsertOutcome, AppError>;

	/// Returns a record if the associated `github_account_id` has an active plan.
	async fn fetch_active(&self, github_account_id: i64) -> Result<Option<Entitlement>, AppError>;
}
