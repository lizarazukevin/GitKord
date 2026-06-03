use crate::db::models::Subscription;
use crate::db::SubscriptionStore;
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::PgPool;

/// `Postgres` row representation of a repository subscription.
///
/// Associates a GitHub repository with a Discord guild/channel pair.
/// `Postgres` uses signed `i64` BIGINT values, so Discord snowflakes
/// are converted at the persistence boundary.
#[derive(sqlx::FromRow)]
pub struct SubscriptionRow {
    pub repo: String,
    pub guild_id: i64,
    pub channel_id: i64,
    pub installation_id: i64,
}

impl From<SubscriptionRow> for Subscription {
    fn from(row: SubscriptionRow) -> Self {
        Self {
            repo: row.repo,
            guild_id: row.guild_id.cast_unsigned(),
            channel_id: row.channel_id.cast_unsigned(),
            installation_id: row.installation_id.cast_unsigned(),
        }
    }
}

/// `Postgres`-backed implementation of `SubscriptionStore`.
///
/// Stores the guild and channel to listen for repository events.
pub struct PostgresSubscriptionStore {
    pool: PgPool,
}

impl PostgresSubscriptionStore {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SubscriptionStore for PostgresSubscriptionStore {
    async fn upsert(&self, sub: Subscription) -> crate::error::Result<()> {
        sqlx::query(
            "INSERT INTO subscriptions (repo, guild_id, channel_id, installation_id)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT DO NOTHING",
        )
        .bind(&sub.repo)
        .bind(sub.guild_id.cast_signed())
        .bind(sub.channel_id.cast_signed())
        .bind(sub.installation_id.cast_signed())
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(())
    }

    async fn get_by_guild(
        &self,
        repo: &str,
        guild_id: u64,
    ) -> crate::error::Result<Option<Subscription>> {
        let row = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT repo, guild_id, channel_id, installation_id FROM subscriptions
             WHERE repo = $1 AND guild_id = $2",
        )
        .bind(repo)
        .bind(guild_id.cast_signed())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(row.map(Subscription::from))
    }

    async fn get_all_for_repo(&self, repo: &str) -> crate::error::Result<Vec<Subscription>> {
        let rows = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT repo, guild_id, channel_id, installation_id FROM subscriptions WHERE repo = $1",
        )
        .bind(repo)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(rows.into_iter().map(Subscription::from).collect())
    }

    async fn delete(&self, repo: &str, guild_id: u64, channel_id: u64) -> crate::error::Result<()> {
        sqlx::query(
            "DELETE FROM subscriptions
         WHERE repo = $1 AND guild_id = $2 AND channel_id = $3",
        )
        .bind(repo)
        .bind(guild_id.cast_signed())
        .bind(channel_id.cast_signed())
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(())
    }

    async fn get_installation_id(&self, repo: &str) -> crate::error::Result<Option<u64>> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT installation_id FROM subscriptions WHERE repo = $1 LIMIT 1")
                .bind(repo)
                .fetch_optional(&self.pool)
                .await
                .map_err(AppError::Database)?;

        Ok(row.map(|(id,)| id.cast_unsigned()))
    }

    async fn delete_all_for_repo(&self, repo: &str) -> crate::error::Result<()> {
        sqlx::query("DELETE FROM subscriptions WHERE repo = $1")
            .bind(repo)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok(())
    }
}
