//! `Postgres` implementation of `NewsletterSignupStore`.

use crate::models::newsletter_signup::{Email, NewsletterSignup, NewsletterSignupStore};
use crate::AppError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

const MAX_PAGE_SIZE: u32 = 100;

#[derive(sqlx::FromRow)]
struct NewsletterSignupRow {
	id: i64,
	email: String,
	subscribed: bool,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
}

impl From<NewsletterSignupRow> for NewsletterSignup {
	fn from(row: NewsletterSignupRow) -> Self {
		Self {
			id: row.id,
			email: row.email,
			subscribed: row.subscribed,
			created_at: row.created_at,
			updated_at: row.updated_at,
		}
	}
}

pub(super) struct PgNewsletterSignupStore {
	pool: PgPool,
}

impl PgNewsletterSignupStore {
	pub(super) const fn new(pool: PgPool) -> Self {
		Self { pool }
	}
}

#[async_trait]
impl NewsletterSignupStore for PgNewsletterSignupStore {
	async fn upsert(&self, email: &Email) -> Result<(), AppError> {
		sqlx::query(
			"INSERT INTO newsletter_signups (email, created_at, updated_at)
             VALUES ($1, NOW(), NOW())
             ON CONFLICT (email) DO UPDATE SET
                subscribed = TRUE,
                updated_at = NOW()",
		)
		.bind(email.as_str())
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	async fn unsubscribe(&self, email: &Email) -> Result<(), AppError> {
		sqlx::query(
			"UPDATE newsletter_signups
             SET subscribed = FALSE, updated_at = NOW()
             WHERE email = $1 AND subscribed",
		)
		.bind(email.as_str())
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	async fn fetch_all_newsletter_signup_emails(
		&self,
		max_results: u32,
		next_token: Option<i64>,
		is_subscribed: Option<bool>,
	) -> Result<Vec<NewsletterSignup>, AppError> {
		let rows = sqlx::query_as::<_, NewsletterSignupRow>(
			"SELECT id, email, subscribed, created_at, updated_at
             FROM newsletter_signups
             WHERE id > $1 AND ($2::boolean IS NULL OR subscribed = $2)
             ORDER BY id
             LIMIT $3",
		)
		.bind(next_token.unwrap_or(0))
		.bind(is_subscribed)
		.bind(i64::from(max_results.clamp(1, MAX_PAGE_SIZE)))
		.fetch_all(&self.pool)
		.await?;

		Ok(rows.into_iter().map(NewsletterSignup::from).collect())
	}

	async fn count_subscribed(&self) -> Result<u64, AppError> {
		let count: i64 =
			sqlx::query_scalar("SELECT COUNT(*) FROM newsletter_signups WHERE subscribed")
				.fetch_one(&self.pool)
				.await?;

		Ok(count.cast_unsigned())
	}
}
