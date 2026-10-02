//! `Postgres` implementation of `EmailSignupStore`.

#![allow(dead_code)]

use crate::models::email_signup::{Email, EmailSignup, EmailSignupStore};
use crate::AppError;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

const MAX_PAGE_SIZE: u32 = 100;

#[derive(sqlx::FromRow)]
struct EmailSignupRow {
	id: i64,
	email: String,
	subscribed: bool,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
}

impl From<EmailSignupRow> for EmailSignup {
	fn from(row: EmailSignupRow) -> Self {
		Self {
			id: row.id,
			email: row.email,
			subscribed: row.subscribed,
			created_at: row.created_at,
			updated_at: row.updated_at,
		}
	}
}

pub(super) struct PgEmailSignupStore {
	pool: PgPool,
}

impl PgEmailSignupStore {
	pub(super) const fn new(pool: PgPool) -> Self {
		Self { pool }
	}
}

#[async_trait]
impl EmailSignupStore for PgEmailSignupStore {
	async fn upsert(&self, email: &Email) -> Result<(), AppError> {
		sqlx::query(
			"INSERT INTO email_signups (email, created_at, updated_at)
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
			"UPDATE email_signups
             SET subscribed = FALSE, updated_at = NOW()
             WHERE email = $1 AND subscribed",
		)
		.bind(email.as_str())
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	async fn fetch_all_email_signups(
		&self,
		next_token: Option<i64>,
		max_results: u32,
		is_subscribed: Option<bool>,
	) -> Result<Vec<EmailSignup>, AppError> {
		let rows = sqlx::query_as::<_, EmailSignupRow>(
			"SELECT id, email, subscribed, created_at, updated_at
             FROM email_signups
             WHERE id > $1 AND ($2::boolean IS NULL OR subscribed = $2)
             ORDER BY id
             LIMIT $3",
		)
		.bind(next_token.unwrap_or(0))
		.bind(is_subscribed)
		.bind(i64::from(max_results.clamp(1, MAX_PAGE_SIZE)))
		.fetch_all(&self.pool)
		.await?;

		Ok(rows.into_iter().map(EmailSignup::from).collect())
	}
}
