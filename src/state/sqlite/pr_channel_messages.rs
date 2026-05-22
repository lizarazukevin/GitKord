use crate::error::AppError;
use crate::state::models::PrChannelMessage;
use crate::state::PrChannelMessageStore;
use async_trait::async_trait;
use sqlx::SqlitePool;

/// `SQLite` row representation of a pull request channel message mapping.
///
/// Stores the Discord message and audit thread associated with a PR
/// in a subscribed channel. `SQLite` uses signed `i64` INTEGER values,
/// so Discord snowflakes are converted at the persistence boundary.
#[derive(sqlx::FromRow)]
pub struct PrChannelMessageRow {
    pub repo: String,
    pub pr_number: i64,
    pub channel_id: i64,
    pub message_id: i64,
    pub thread_id: i64,
}

impl From<PrChannelMessageRow> for PrChannelMessage {
    fn from(row: PrChannelMessageRow) -> Self {
        Self {
            repo: row.repo,
            pr_number: row.pr_number.cast_unsigned(),
            channel_id: row.channel_id.cast_unsigned(),
            message_id: row.message_id.cast_unsigned(),
            thread_id: row.thread_id.cast_unsigned(),
        }
    }
}

/// `SQLite`-backed implementation of `PrChannelMessageStore`.
///
/// Stores the Discord message + audit thread associated with a PR
/// for each subscribed channel.
pub struct SqlitePrChannelMessageStore {
    pool: SqlitePool,
}

impl SqlitePrChannelMessageStore {
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PrChannelMessageStore for SqlitePrChannelMessageStore {
    async fn upsert(&self, record: PrChannelMessage) -> crate::error::Result<()> {
        sqlx::query(
            "INSERT INTO pr_channel_messages (repo, pr_number, channel_id, message_id, thread_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (repo, pr_number, channel_id) DO UPDATE SET
                message_id = excluded.message_id,
                thread_id = excluded.thread_id",
        )
        .bind(&record.repo)
        .bind(record.pr_number.cast_signed())
        .bind(record.channel_id.cast_signed())
        .bind(record.message_id.cast_signed())
        .bind(record.thread_id.cast_signed())
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(())
    }

    async fn get(
        &self,
        repo: &str,
        pr_number: u64,
    ) -> crate::error::Result<Option<PrChannelMessage>> {
        let row = sqlx::query_as::<_, PrChannelMessageRow>(
            "SELECT repo, pr_number, channel_id, message_id, thread_id
             FROM pr_channel_messages WHERE repo = ?1 AND pr_number = ?2",
        )
        .bind(repo)
        .bind(pr_number.cast_signed())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(row.map(PrChannelMessage::from))
    }

    async fn get_all_by_repo_and_pr(
        &self,
        repo: &str,
        pr_number: u64,
    ) -> crate::error::Result<Vec<PrChannelMessage>> {
        let rows = sqlx::query_as::<_, PrChannelMessageRow>(
            "SELECT repo, pr_number, channel_id, message_id, thread_id FROM pr_channel_messages WHERE repo = ?1, pr_number = ?2)",
        )
            .bind(repo)
            .bind(pr_number.cast_signed())
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok(rows.into_iter().map(PrChannelMessage::from).collect())
    }

    async fn delete(
        &self,
        repo: &str,
        pr_number: u64,
        channel_id: u64,
    ) -> crate::error::Result<()> {
        sqlx::query(
            "DELETE FROM pr_channel_messages WHERE repo = ?1 AND pr_number = ?2 AND channel_id = ?3",
        )
            .bind(repo)
            .bind(pr_number.cast_signed())
            .bind(channel_id.cast_signed())
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok(())
    }

    async fn delete_all_for_pr(&self, repo: &str, pr_number: u64) -> crate::error::Result<()> {
        sqlx::query("DELETE FROM pr_channel_messages WHERE repo = ?1 AND pr_number = ?2")
            .bind(repo)
            .bind(pr_number.cast_signed())
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok(())
    }

    async fn get_by_thread_id(
        &self,
        thread_id: u64,
    ) -> crate::error::Result<Option<PrChannelMessage>> {
        let row = sqlx::query_as::<_, PrChannelMessageRow>(
            "SELECT repo, pr_number, channel_id, message_id, thread_id
                 FROM pr_channel_messages
                 WHERE thread_id = ?1",
        )
        .bind(thread_id.cast_signed())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(row.map(PrChannelMessage::from))
    }
}
