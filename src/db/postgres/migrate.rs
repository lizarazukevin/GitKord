//! Schema migration helpers.

use crate::error::AppError;
use sqlx::PgPool;
use std::path::Path;

/// Runs all pending migrations from the `migrations/` directory at runtime.
///
/// Uses the runtime [`sqlx::migrate::Migrator`] (rather than the compile-time
/// `sqlx::migrate!` macro) so that adding or editing migration files no longer
/// invalidates the crate's build cache and forces a full recompile/re-lint of
/// the entire binary crate.
pub(super) async fn run_migrations(pool: &PgPool) -> Result<(), AppError> {
	sqlx::migrate::Migrator::new(Path::new("src/db/postgres/migrations"))
		.await?
		.run(pool)
		.await?;
	Ok(())
}
