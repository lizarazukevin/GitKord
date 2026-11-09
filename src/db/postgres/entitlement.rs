//! `Postgres` implementation of [`EntitlementStore`].

use crate::models::entitlement::{Entitlement, EntitlementStatus, EntitlementStore, UpsertOutcome};
use crate::models::plan::Plan;
use crate::AppError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Row};

#[derive(FromRow)]
struct EntitlementRow {
	github_account_id: i64,
	plan: String,
	status: String,
	effective_until: Option<DateTime<Utc>>,
	received_at: DateTime<Utc>,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
	created_by: String,
	updated_by: String,
}

impl From<EntitlementRow> for Entitlement {
	fn from(row: EntitlementRow) -> Self {
		Self {
			github_account_id: row.github_account_id,
			plan: Plan::from_stored(&row.plan),
			status: match row.status.as_str() {
				"active" => EntitlementStatus::Active,
				_ => EntitlementStatus::Cancelled,
			},
			effective_until: row.effective_until,
			received_at: row.received_at,
			created_at: row.created_at,
			updated_at: row.updated_at,
			created_by: row.created_by,
			updated_by: row.updated_by,
		}
	}
}

pub(super) struct PgEntitlementStore {
	pool: PgPool,
}

impl PgEntitlementStore {
	pub const fn new(pool: PgPool) -> Self {
		Self { pool }
	}
}

#[async_trait]
impl EntitlementStore for PgEntitlementStore {
	async fn upsert(&self, e: Entitlement) -> Result<UpsertOutcome, AppError> {
		let row = sqlx::query(
			"INSERT INTO entitlements (
                 github_account_id, plan, status, effective_until,
                 received_at, created_by, updated_by
             ) VALUES ($1, $2, $3, $4, NOW(), $5, $5)
             ON CONFLICT (github_account_id) DO UPDATE SET
                 plan            = EXCLUDED.plan,
                 status          = EXCLUDED.status,
                 effective_until = EXCLUDED.effective_until,
                 received_at     = EXCLUDED.received_at,
                 updated_at      = NOW(),
                 updated_by      = EXCLUDED.updated_by
             RETURNING (xmax = 0) AS inserted",
		)
		.bind(e.github_account_id)
		.bind(e.plan.as_str())
		.bind(e.status.as_str())
		.bind(e.effective_until)
		.bind(&e.created_by)
		.fetch_one(&self.pool)
		.await?;

		let inserted: bool = row.try_get("inserted")?;
		Ok(if inserted {
			UpsertOutcome::Inserted
		} else {
			UpsertOutcome::Updated
		})
	}

	async fn fetch_active(&self, github_account_id: i64) -> Result<Option<Entitlement>, AppError> {
		let row = sqlx::query_as::<_, EntitlementRow>(
			"SELECT github_account_id, plan, status, effective_until,
                    received_at, created_at, updated_at, created_by, updated_by
             FROM entitlements
             WHERE github_account_id = $1
               AND status = 'active'
               AND (effective_until IS NULL OR effective_until > NOW())",
		)
		.bind(github_account_id)
		.fetch_optional(&self.pool)
		.await?;

		Ok(row.map(Entitlement::from))
	}
}
