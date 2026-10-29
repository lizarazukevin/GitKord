//! `Postgres` implementation of `UserStore`.
use crate::error::AppError;
use crate::models::user_link::{UserLink, UserStore};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::info;

/// `Postgres` row representation of a Discord ↔ GitHub link.
#[derive(sqlx::FromRow)]
struct UserLinkRow {
	discord_id: i64,
	github_login: Option<String>,
	github_user_id: Option<i64>,
	github_verified_at: Option<DateTime<Utc>>,
	created_at: DateTime<Utc>,
	updated_at: DateTime<Utc>,
	created_by: Option<String>,
	updated_by: Option<String>,
}

impl From<UserLinkRow> for UserLink {
	fn from(row: UserLinkRow) -> Self {
		Self {
			discord_id: row.discord_id.cast_unsigned(),
			github_login: row.github_login,
			github_user_id: row.github_user_id,
			github_verified_at: row.github_verified_at,
			created_at: row.created_at,
			updated_at: row.updated_at,
			created_by: row.created_by,
			updated_by: row.updated_by,
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

#[async_trait]
impl UserStore for PgUserStore {
	async fn upsert(&self, link: UserLink) -> Result<(), AppError> {
		sqlx::query(
			"INSERT INTO user_links (discord_id, github_login, created_at, updated_at, created_by, updated_by)
             VALUES ($1, $2, NOW(), NOW(), $3, $3)
             ON CONFLICT (discord_id) DO UPDATE SET
                github_login = EXCLUDED.github_login,
                updated_at = NOW(),
                updated_by = EXCLUDED.updated_by",
		)
			.bind(link.discord_id.cast_signed())
			.bind(link.github_login.as_deref())
			.bind(link.updated_by.as_deref())
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	async fn upsert_verified(
		&self,
		discord_id: u64,
		github_user_id: i64,
		github_login: &str,
	) -> Result<(), AppError> {
		let mut tx = self.pool.begin().await?;

		// Drop every other unverified/legacy claim on this login. A verified
		// row that loses its login keeps its ID, so ID-based mentions still
		// resolve.
		let cleared = sqlx::query_scalar::<_, i64>(
			"UPDATE user_links
                SET github_login = NULL,
                    updated_at = NOW(),
                    updated_by = $2
              WHERE lower(github_login) = lower($1)
                AND discord_id <> $3
            RETURNING discord_id",
		)
		.bind(github_login)
		.bind(discord_id.to_string())
		.bind(discord_id.cast_signed())
		.fetch_all(&mut *tx)
		.await?;

		for cleared_discord_id in cleared {
			info!(
				discord_id,
				cleared_discord_id = cleared_discord_id.cast_unsigned(),
				github_login,
				"cleared competing github login"
			);
		}

		// Upsert by discord_id. A unique violation on github_user_id means
		// another Discord user already owns this GitHub account.
		let result = sqlx::query(
			"INSERT INTO user_links
                (discord_id, github_login, github_user_id, github_verified_at,
                 created_at, updated_at, created_by, updated_by)
             VALUES ($1, $2, $3, NOW(), NOW(), NOW(), $4, $4)
             ON CONFLICT (discord_id) DO UPDATE SET
                github_login = EXCLUDED.github_login,
                github_user_id = EXCLUDED.github_user_id,
                github_verified_at = EXCLUDED.github_verified_at,
                updated_at = NOW(),
                updated_by = EXCLUDED.updated_by",
		)
		.bind(discord_id.cast_signed())
		.bind(github_login)
		.bind(github_user_id)
		.bind(discord_id.to_string())
		.execute(&mut *tx)
		.await;

		match result {
			Ok(_) => {
				tx.commit().await?;
				Ok(())
			}
			Err(sqlx::Error::Database(db_err))
				if db_err.constraint() == Some("user_links_github_user_id_key") =>
			{
				Err(AppError::UserLinkConflict)
			}
			Err(e) => Err(e.into()),
		}
	}

	async fn fetch_by_discord_id(&self, discord_id: u64) -> Result<Option<UserLink>, AppError> {
		let row = sqlx::query_as::<_, UserLinkRow>(
			"SELECT discord_id, github_login, github_user_id, github_verified_at,
                    created_at, updated_at, created_by, updated_by
               FROM user_links
              WHERE discord_id = $1",
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
			"SELECT discord_id, github_login, github_user_id, github_verified_at,
                    created_at, updated_at, created_by, updated_by
               FROM user_links
              WHERE github_login = ANY($1)",
		)
		.bind(github_logins)
		.fetch_all(&self.pool)
		.await?;

		Ok(rows
			.into_iter()
			.filter_map(|row| {
				let link = UserLink::from(row);
				link.github_login.map(|login| (login, link.discord_id))
			})
			.collect())
	}

	async fn unlink_github(&self, discord_id: u64) -> Result<(), AppError> {
		sqlx::query(
			"UPDATE user_links
                SET github_login = NULL,
                    github_user_id = NULL,
                    github_verified_at = NULL,
                    updated_at = NOW(),
                    updated_by = $2
              WHERE discord_id = $1",
		)
		.bind(discord_id.cast_signed())
		.bind(discord_id.to_string())
		.execute(&self.pool)
		.await?;
		Ok(())
	}

	async fn delete(&self, discord_id: u64) -> Result<(), AppError> {
		sqlx::query("DELETE FROM user_links WHERE discord_id = $1")
			.bind(discord_id.cast_signed())
			.execute(&self.pool)
			.await?;
		Ok(())
	}

	async fn count_registered(&self) -> Result<u64, AppError> {
		let count: i64 = sqlx::query_scalar(
			"SELECT COUNT(*) FROM user_links
              WHERE github_login IS NOT NULL OR github_user_id IS NOT NULL",
		)
		.fetch_one(&self.pool)
		.await?;
		Ok(count.cast_unsigned())
	}
}
