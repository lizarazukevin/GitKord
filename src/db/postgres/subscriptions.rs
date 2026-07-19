//! `Postgres` implementation of `SubscriptionStore`.

use crate::error::AppError;
use crate::models::subscription::{Subscription, SubscriptionStore};
use async_trait::async_trait;
use sqlx::PgPool;

/// `Postgres` row representation of a subscription.
#[derive(sqlx::FromRow)]
struct SubscriptionRow {
    repository: String,
    guild_id: i64,
    channel_id: i64,
    installation_id: i64,
}

impl From<SubscriptionRow> for Subscription {
    fn from(row: SubscriptionRow) -> Self {
        Self {
            repository: row.repository,
            guild_id: row.guild_id.cast_unsigned(),
            channel_id: row.channel_id.cast_unsigned(),
            installation_id: row.installation_id.cast_unsigned(),
        }
    }
}

pub(super) struct PgSubscriptionStore {
    pool: PgPool,
}

impl PgSubscriptionStore {
    pub(super) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Create the `subscriptions` table if it does not already exist.
pub(super) async fn create_table(pool: &PgPool) -> Result<(), AppError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS subscriptions (
                repository        TEXT NOT NULL,
                guild_id    BIGINT NOT NULL,
                channel_id  BIGINT NOT NULL,
                installation_id BIGINT NOT NULL,
                PRIMARY KEY (repository, guild_id, channel_id)
            )",
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[async_trait]
impl SubscriptionStore for PgSubscriptionStore {
    async fn upsert(&self, sub: Subscription) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO subscriptions (repository, guild_id, channel_id, installation_id)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (repository, guild_id, channel_id) DO UPDATE SET
                installation_id = EXCLUDED.installation_id",
        )
        .bind(&sub.repository)
        .bind(sub.guild_id.cast_signed())
        .bind(sub.channel_id.cast_signed())
        .bind(sub.installation_id.cast_signed())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn fetch_installation_id_by_repo(&self, repo: &str) -> Result<u64, AppError> {
        let (id,): (i64,) = sqlx::query_as(
            "SELECT installation_id FROM subscriptions WHERE repository = $1 LIMIT 1",
        )
        .bind(repo)
        .fetch_one(&self.pool)
        .await?;

        Ok(id.cast_unsigned())
    }

    async fn fetch_all_by_repo(&self, repo: &str) -> Result<Vec<Subscription>, AppError> {
        let rows = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT repository, guild_id, channel_id, installation_id FROM subscriptions WHERE repository = $1",
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
