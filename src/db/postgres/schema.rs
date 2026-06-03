use crate::error::AppError;
use sqlx::PgPool;

/// Connect to `Postgres` and create all tables if they do not exist.
///
/// Call once at startup before constructing any store. Fails fast if
/// the database file cannot be created or the schema cannot be applied.
///
/// # Errors
///
/// Returns [`AppError::Database`] if the connection or any table creation fails.
pub async fn connect(database_url: &str) -> crate::error::Result<PgPool> {
    let pool = PgPool::connect(database_url)
        .await
        .map_err(AppError::Database)?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pr_channel_messages (
            repo       TEXT NOT NULL,
            pr_number  BIGINT NOT NULL,
            channel_id BIGINT NOT NULL,
            message_id BIGINT NOT NULL,
            thread_id BIGINT NOT NULL,
            PRIMARY KEY (repo, pr_number, channel_id)
        )",
    )
    .execute(&pool)
    .await
    .map_err(AppError::Database)?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS subscriptions (
                repo        TEXT NOT NULL,
                guild_id    BIGINT NOT NULL,
                channel_id  BIGINT NOT NULL,
                installation_id BIGINT NOT NULL,
                PRIMARY KEY (repo, guild_id, channel_id)
            )",
    )
    .execute(&pool)
    .await
    .map_err(AppError::Database)?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_links (
            discord_id   BIGINT PRIMARY KEY,
            github_login TEXT NOT NULL UNIQUE
        )",
    )
    .execute(&pool)
    .await
    .map_err(AppError::Database)?;

    Ok(pool)
}
