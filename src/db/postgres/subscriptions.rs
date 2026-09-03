//! `Postgres` implementation of `SubscriptionStore`.

use crate::error::AppError;
use crate::models::subscription::{Subscription, SubscriptionStore};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// `Postgres` row representation of a subscription.
#[derive(sqlx::FromRow)]
struct SubscriptionRow {
	repository: String,
	guild_id: i64,
	channel_id: i64,
	installation_id: i64,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
	created_by: Option<String>,
	updated_by: Option<String>,
}

impl From<SubscriptionRow> for Subscription {
	fn from(row: SubscriptionRow) -> Self {
		Self {
			repository: row.repository,
			guild_id: row.guild_id.cast_unsigned(),
			channel_id: row.channel_id.cast_unsigned(),
			installation_id: row.installation_id.cast_unsigned(),
			created_at: row.created_at,
			updated_at: row.updated_at,
			created_by: row.created_by,
			updated_by: row.updated_by,
		}
	}
}

pub(super) struct PgSubscriptionStore {
	pool: PgPool,
}

impl PgSubscriptionStore {
	pub(super) const fn new(pool: PgPool) -> Self {
		Self { pool }
	}
}

#[async_trait]
impl SubscriptionStore for PgSubscriptionStore {
	async fn upsert(&self, sub: Subscription) -> Result<(), AppError> {
		sqlx::query(
			"INSERT INTO subscriptions (repository, guild_id, channel_id, installation_id, created_at, updated_at, created_by, updated_by)
             VALUES ($1, $2, $3, $4, NOW(), NOW(), $5, $5)
             ON CONFLICT (repository, guild_id, channel_id) DO UPDATE SET
                installation_id = EXCLUDED.installation_id,
                updated_at = NOW(),
                updated_by = EXCLUDED.updated_by",
		)
		.bind(&sub.repository)
		.bind(sub.guild_id.cast_signed())
		.bind(sub.channel_id.cast_signed())
		.bind(sub.installation_id.cast_signed())
		.bind(sub.updated_by.as_deref())
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	async fn fetch_installation_id_by_repo(&self, repo: &str) -> Result<Option<u64>, AppError> {
		let id: Option<i64> = sqlx::query_scalar(
			"SELECT installation_id
         FROM subscriptions
         WHERE repository = $1
         LIMIT 1",
		)
		.bind(repo)
		.fetch_optional(&self.pool)
		.await?;

		Ok(id.map(i64::cast_unsigned))
	}

	async fn fetch_all_by_repo(&self, repo: &str) -> Result<Vec<Subscription>, AppError> {
		let rows = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT repository, guild_id, channel_id, installation_id, created_at, updated_at, created_by, updated_by FROM subscriptions WHERE repository = $1",
        )
        .bind(repo)
        .fetch_all(&self.pool)
        .await?;

		Ok(rows.into_iter().map(Subscription::from).collect())
	}

	async fn delete(&self, repo: &str, guild_id: u64, channel_id: u64) -> Result<(), AppError> {
		sqlx::query(
			"DELETE FROM subscriptions
         WHERE repository = $1 AND guild_id = $2 AND channel_id = $3",
		)
		.bind(repo)
		.bind(guild_id.cast_signed())
		.bind(channel_id.cast_signed())
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	async fn delete_all_by_repo(&self, repo: &str) -> Result<(), AppError> {
		sqlx::query("DELETE FROM subscriptions WHERE repository = $1")
			.bind(repo)
			.execute(&self.pool)
			.await?;

		Ok(())
	}
}
