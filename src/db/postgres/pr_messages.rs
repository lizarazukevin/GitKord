//! `Postgres` implementation of `PrStore`.

use crate::error::AppError;
use crate::models::pr_message::{PrMessage, PrStore};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

/// `Postgres` row representation of a pull request channel message mapping.
#[derive(sqlx::FromRow)]
struct PrMessageRow {
	repository: String,
	pr: i64,
	channel_id: i64,
	message_id: i64,
	thread_id: i64,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
	created_by: Option<String>,
	updated_by: Option<String>,
}

impl From<PrMessageRow> for PrMessage {
	fn from(row: PrMessageRow) -> Self {
		Self {
			repository: row.repository,
			pr: row.pr.cast_unsigned(),
			channel_id: row.channel_id.cast_unsigned(),
			message_id: row.message_id.cast_unsigned(),
			thread_id: row.thread_id.cast_unsigned(),
			created_at: row.created_at,
			updated_at: row.updated_at,
			created_by: row.created_by,
			updated_by: row.updated_by,
		}
	}
}

pub(super) struct PgPrMessageStore {
	pool: PgPool,
}

impl PgPrMessageStore {
	pub(super) const fn new(pool: PgPool) -> Self {
		Self { pool }
	}
}

#[async_trait]
impl PrStore for PgPrMessageStore {
	async fn upsert(&self, record: PrMessage) -> Result<(), AppError> {
		sqlx::query(
			"INSERT INTO pr_messages (repository, pr, channel_id, message_id, thread_id)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (repository, pr, channel_id) DO UPDATE SET
                message_id = EXCLUDED.message_id,
                thread_id = EXCLUDED.thread_id",
		)
		.bind(&record.repository)
		.bind(record.pr.cast_signed())
		.bind(record.channel_id.cast_signed())
		.bind(record.message_id.cast_signed())
		.bind(record.thread_id.cast_signed())
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	async fn fetch_by_thread_id(&self, thread_id: u64) -> Result<Option<PrMessage>, AppError> {
		let row = sqlx::query_as::<_, PrMessageRow>(
			"SELECT repository, pr, channel_id, message_id, thread_id, created_at, updated_at, created_by, updated_by
                 FROM pr_messages
                 WHERE thread_id = $1",
		)
		.bind(thread_id.cast_signed())
		.fetch_optional(&self.pool)
		.await?;

		Ok(row.map(PrMessage::from))
	}

	async fn fetch_all_by_repo_and_pr(
		&self,
		repo: &str,
		pr_number: u64,
	) -> Result<Vec<PrMessage>, AppError> {
		let rows = sqlx::query_as::<_, PrMessageRow>(
            "SELECT repository, pr, channel_id, message_id, thread_id, created_at, updated_at, created_by, updated_by FROM pr_messages WHERE repository = $1 AND pr = $2"
        )
            .bind(repo)
            .bind(pr_number.cast_signed())
            .fetch_all(&self.pool)
            .await?;

		Ok(rows.into_iter().map(PrMessage::from).collect())
	}
}
