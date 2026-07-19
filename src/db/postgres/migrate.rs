//! Schema migration helpers.

use crate::error::AppError;
use sqlx::PgPool;

/// Ensures all the required tables exist.
pub(super) async fn run_migrations(pool: &PgPool) -> Result<(), AppError> {
    super::pr_messages::create_table(pool).await?;
    super::subscriptions::create_table(pool).await?;
    super::user_links::create_table(pool).await?;
    Ok(())
}
