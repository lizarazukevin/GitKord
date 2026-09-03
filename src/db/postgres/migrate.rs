//! Schema migration helpers.

use crate::error::AppError;
use sqlx::PgPool;

/// Runs all pending migrations from the `migrations/` directory.
pub(super) async fn run_migrations(pool: &PgPool) -> Result<(), AppError> {
	sqlx::migrate!("src/db/postgres/migrations")
		.run(pool)
		.await
		.map_err(|e| AppError::Internal(e.into()))?;
	Ok(())
}
