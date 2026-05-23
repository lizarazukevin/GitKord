use crate::error::AppError;
use crate::state::models::UserLink;
use crate::state::UserLinkStore;
use async_trait::async_trait;
use sqlx::SqlitePool;

/// `SQLite` row representation of a Discord ↔ GitHub user link.
///
/// Maps a Discord user snowflake to a GitHub login for reviewer
/// mentions and attribution. `SQLite` uses signed `i64` INTEGER
/// values, so Discord snowflakes are converted at the persistence boundary.
#[derive(sqlx::FromRow)]
pub struct UserLinkRow {
    pub discord_id: i64,
    pub github_login: String,
}

impl From<UserLinkRow> for UserLink {
    fn from(row: UserLinkRow) -> Self {
        Self {
            discord_id: row.discord_id.cast_unsigned(),
            github_login: row.github_login,
        }
    }
}

/// `SQLite`-backed implementation of `UserLinkStore`.
///
/// Stores the Discord tag and GitHub login of a user when
/// the link command is invoked, at any time this should only be one pairing.
pub struct SqliteUserLinkStore {
    pool: SqlitePool,
}

impl SqliteUserLinkStore {
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserLinkStore for SqliteUserLinkStore {
    async fn upsert(&self, link: UserLink) -> crate::error::Result<()> {
        sqlx::query(
            "INSERT INTO user_links (discord_id, github_login)
             VALUES (?1, ?2)
             ON CONFLICT (discord_id) DO UPDATE SET
                github_login = excluded.github_login",
        )
        .bind(link.discord_id.cast_signed())
        .bind(&link.github_login)
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(())
    }

    async fn get_by_discord(&self, discord_id: u64) -> crate::error::Result<Option<UserLink>> {
        let row = sqlx::query_as::<_, UserLinkRow>(
            "SELECT discord_id, github_login FROM user_links WHERE discord_id = ?1",
        )
        .bind(discord_id.cast_signed())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(row.map(UserLink::from))
    }

    async fn get_by_github(&self, github_login: &str) -> crate::error::Result<Option<UserLink>> {
        let row = sqlx::query_as::<_, UserLinkRow>(
            "SELECT discord_id, github_login FROM user_links WHERE github_login = ?1",
        )
        .bind(github_login)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(row.map(UserLink::from))
    }

    async fn delete(&self, discord_id: u64) -> crate::error::Result<()> {
        sqlx::query("DELETE FROM user_links WHERE discord_id = ?1")
            .bind(discord_id.cast_signed())
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok(())
    }
}
