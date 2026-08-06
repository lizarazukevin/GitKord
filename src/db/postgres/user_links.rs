//! `Postgres` implementation of `UserStore`.

use crate::error::AppError;
use crate::models::user_link::{UserLink, UserStore};
use async_trait::async_trait;
use sqlx::PgPool;
use std::collections::HashMap;

/// `Postgres` row representation of a Discord ↔ GitHub link.
#[derive(sqlx::FromRow)]
struct UserLinkRow {
	discord_id: i64,
	github_login: String,
}

impl From<UserLinkRow> for UserLink {
	fn from(row: UserLinkRow) -> Self {
		Self {
			discord_id: row.discord_id.cast_unsigned(),
			github_login: row.github_login,
		}
	}
}

pub(super) struct PgUserStore {
	pool: PgPool,
}

impl PgUserStore {
	pub(super) const fn new(pool: PgPool) -> Self {
		Self { pool }
	}
}

/// Create the `user_links` table if it does not already exist.
pub(super) async fn create_table(pool: &PgPool) -> Result<(), AppError> {
	sqlx::query(
		"CREATE TABLE IF NOT EXISTS user_links (
            discord_id   BIGINT PRIMARY KEY,
            github_login TEXT NOT NULL UNIQUE
        )",
	)
	.execute(pool)
	.await?;
	Ok(())
}

#[async_trait]
impl UserStore for PgUserStore {
	async fn upsert(&self, link: UserLink) -> Result<(), AppError> {
		sqlx::query(
			"INSERT INTO user_links (discord_id, github_login)
             VALUES ($1, $2)
             ON CONFLICT (discord_id) DO UPDATE SET
                github_login = EXCLUDED.github_login",
		)
		.bind(link.discord_id.cast_signed())
		.bind(&link.github_login)
		.execute(&self.pool)
		.await?;

		Ok(())
	}

	async fn fetch_by_discord_id(&self, discord_id: u64) -> Result<Option<UserLink>, AppError> {
		let row = sqlx::query_as::<_, UserLinkRow>(
			"SELECT discord_id, github_login FROM user_links WHERE discord_id = $1",
		)
		.bind(discord_id.cast_signed())
		.fetch_optional(&self.pool)
		.await?;

		Ok(row.map(UserLink::from))
	}

	async fn fetch_by_github_logins(
		&self,
		github_logins: &[String],
	) -> Result<HashMap<String, u64>, AppError> {
		if github_logins.is_empty() {
			return Ok(HashMap::new());
		}

		let rows = sqlx::query_as::<_, UserLinkRow>(
			"SELECT discord_id, github_login FROM user_links WHERE github_login = ANY($1)",
		)
		.bind(github_logins)
		.fetch_all(&self.pool)
		.await?;

		Ok(rows
			.into_iter()
			.map(|row| {
				let link = UserLink::from(row);
				(link.github_login, link.discord_id)
			})
			.collect())
	}

	async fn delete(&self, discord_id: u64) -> Result<(), AppError> {
		sqlx::query("DELETE FROM user_links WHERE discord_id = $1")
			.bind(discord_id.cast_signed())
			.execute(&self.pool)
			.await?;

		Ok(())
	}
}
