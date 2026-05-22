use crate::error::AppError;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::SqlitePool;

/// Connect to `SQLite` and create all tables if they do not exist.
///
/// Call once at startup before constructing any store. Fails fast if
/// the database file cannot be created or the schema cannot be applied.
///
/// # Errors
///
/// Returns [`AppError::Database`] if the connection or any table creation fails.
pub async fn connect(database_url: &str) -> crate::error::Result<SqlitePool> {
    let opts = database_url
        .parse::<SqliteConnectOptions>()
        .map_err(AppError::Database)?
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(opts)
        .await
        .map_err(AppError::Database)?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pr_channel_messages (
            repo       TEXT NOT NULL,
            pr_number  INTEGER NOT NULL,
            channel_id INTEGER NOT NULL,
            message_id INTEGER NOT NULL,
            thread_id INTEGER NOT NULL,
            PRIMARY KEY (repo, pr_number, channel_id)
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
                PRIMARY KEY (repo, guild_id, channel_id)
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
