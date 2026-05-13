//! SQLite-backed store implementations.
//!
//! All queries use `sqlx` with the async `SQLite` driver. The pool is shared
//! across all handler invocations via `Arc` — `SqlitePool` is already
//! internally reference-counted so wrapping it in `Arc` is not required,
//! but we do so to keep the ownership model consistent with `Http`.

use async_trait::async_trait;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::state::traits::{
    PrMessage, PrMessageStore, Subscription, SubscriptionStore, UserLink, UserLinkStore,
};

// ── Database initialisation ───────────────────────────────────────────────────

/// Connect to the `SQLite` database and run migrations.
///
/// Creates the database file if it does not exist. Call once at startup
/// before constructing any store.
///
/// # Errors
///
/// Returns [`AppError::Database`] if the connection or table creation fails.
pub async fn connect(database_url: &str) -> Result<SqlitePool, AppError> {
    let opts = database_url
        .parse::<SqliteConnectOptions>()
        .map_err(AppError::Database)?
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(opts)
        .await
        .map_err(AppError::Database)?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pr_messages (
            repo       TEXT NOT NULL,
            pr_number  INTEGER NOT NULL,
            channel_id INTEGER NOT NULL,
            message_id INTEGER NOT NULL,
            thread_id INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (repo, pr_number)
        )",
    )
    .execute(&pool)
    .await
    .map_err(AppError::Database)?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS subscriptions (
                repo        TEXT NOT NULL,
                guild_id    INTEGER NOT NULL,
                channel_id  INTEGER NOT NULL,
                PRIMARY KEY (repo, guild_id)
            )",
    )
    .execute(&pool)
    .await
    .map_err(AppError::Database)?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_links (
            discord_id   INTEGER PRIMARY KEY,
            github_login TEXT NOT NULL UNIQUE
        )",
    )
    .execute(&pool)
    .await
    .map_err(AppError::Database)?;

    Ok(pool)
}

// ── PrMessageStore ────────────────────────────────────────────────────────────

/// SQLite-backed implementation of [`PrMessageStore`].
pub struct SqlitePrMessageStore {
    pool: SqlitePool,
}

impl SqlitePrMessageStore {
    /// Create a new store backed by the given connection pool.
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PrMessageStore for SqlitePrMessageStore {
    async fn upsert(&self, record: PrMessage) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO pr_messages (repo, pr_number, channel_id, message_id, thread_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (repo, pr_number) DO UPDATE SET
                channel_id = excluded.channel_id,
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

    async fn get(&self, repo: &str, pr_number: u64) -> Result<Option<PrMessage>, AppError> {
        let row = sqlx::query_as::<_, PrMessageRow>(
            "SELECT repo, pr_number, channel_id, message_id, thread_id
             FROM pr_messages
             WHERE repo = ?1 AND pr_number = ?2",
        )
        .bind(repo)
        .bind(pr_number.cast_signed())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(row.map(PrMessage::from))
    }

    async fn delete(&self, repo: &str, pr_number: u64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM pr_messages WHERE repo = ?1 AND pr_number = ?2")
            .bind(repo)
            .bind(pr_number.cast_signed())
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok(())
    }
}

// ── SubscriptionStore ─────────────────────────────────────────────────────────

/// `SQLite`-backed implementation of [`SubscriptionStore`].
pub struct SqliteSubscriptionStore {
    pool: SqlitePool,
}

impl SqliteSubscriptionStore {
    /// Create a new store backed by the given connection pool.
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SubscriptionStore for SqliteSubscriptionStore {
    async fn upsert(&self, sub: Subscription) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO subscriptions (repo, guild_id, channel_id)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (repo, guild_id) DO UPDATE SET
                channel_id = excluded.channel_id",
        )
        .bind(&sub.repo)
        .bind(sub.guild_id.cast_signed())
        .bind(sub.channel_id.cast_signed())
        .execute(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(())
    }

    async fn get(&self, repo: &str, guild_id: u64) -> Result<Option<Subscription>, AppError> {
        let row = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT repo, guild_id, channel_id FROM subscriptions
             WHERE repo = ?1 AND guild_id = ?2",
        )
        .bind(repo)
        .bind(guild_id.cast_signed())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(row.map(Subscription::from))
    }

    async fn get_all_for_repo(&self, repo: &str) -> Result<Vec<Subscription>, AppError> {
        let rows = sqlx::query_as::<_, SubscriptionRow>(
            "SELECT repo, guild_id, channel_id FROM subscriptions WHERE repo = ?1",
        )
        .bind(repo)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(rows.into_iter().map(Subscription::from).collect())
    }

    async fn delete(&self, repo: &str, guild_id: u64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM subscriptions WHERE repo = ?1 AND guild_id = ?2")
            .bind(repo)
            .bind(guild_id.cast_signed())
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok(())
    }
}

// ── UserLinkStore ─────────────────────────────────────────────────────────────

/// SQLite-backed implementation of [`UserLinkStore`].
pub struct SqliteUserLinkStore {
    pool: SqlitePool,
}

impl SqliteUserLinkStore {
    /// Create a new store backed by the given connection pool.
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserLinkStore for SqliteUserLinkStore {
    async fn upsert(&self, link: UserLink) -> Result<(), AppError> {
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

    async fn get_by_discord(&self, discord_id: u64) -> Result<Option<UserLink>, AppError> {
        let row = sqlx::query_as::<_, UserLinkRow>(
            "SELECT discord_id, github_login FROM user_links WHERE discord_id = ?1",
        )
        .bind(discord_id.cast_signed())
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(row.map(UserLink::from))
    }

    async fn get_by_github(&self, github_login: &str) -> Result<Option<UserLink>, AppError> {
        let row = sqlx::query_as::<_, UserLinkRow>(
            "SELECT discord_id, github_login FROM user_links WHERE github_login = ?1",
        )
        .bind(github_login)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)?;

        Ok(row.map(UserLink::from))
    }

    async fn delete(&self, discord_id: u64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM user_links WHERE discord_id = ?1")
            .bind(discord_id.cast_signed())
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok(())
    }
}

// ── Row types (sqlx query_as targets) ────────────────────────────────────────
// SQLite has no u64 — we store as i64 and cast on the way out.

#[derive(sqlx::FromRow)]
struct PrMessageRow {
    repo: String,
    pr_number: i64,
    channel_id: i64,
    message_id: i64,
    thread_id: i64,
}

impl From<PrMessageRow> for PrMessage {
    fn from(row: PrMessageRow) -> Self {
        Self {
            repo: row.repo,
            pr_number: row.pr_number.cast_unsigned(),
            channel_id: row.channel_id.cast_unsigned(),
            message_id: row.message_id.cast_unsigned(),
            thread_id: row.thread_id.cast_unsigned(),
        }
    }
}

#[derive(sqlx::FromRow)]
struct SubscriptionRow {
    repo: String,
    guild_id: i64,
    channel_id: i64,
}

impl From<SubscriptionRow> for Subscription {
    fn from(row: SubscriptionRow) -> Self {
        Self {
            repo: row.repo,
            guild_id: row.guild_id.cast_unsigned(),
            channel_id: row.channel_id.cast_unsigned(),
        }
    }
}

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
