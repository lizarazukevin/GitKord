//! Application lifecycle orchestration.

mod bootstrap;
pub mod observability;
mod server;
mod shutdown;
mod telemetry;

use crate::error::AppError;
use crate::EnvConfig;
pub use telemetry::init_tracing;

/// Build the application and run it until a shutdown signal.
///
/// Spawns the HTTP webhook server and Discord client as concurrent tasks.
/// On a shutdown signal, both are stopped gracefully and `Ok(())` is returned.
///
/// # Errors
///
/// Returns [`AppError`] if application construction fails — e.g. invalid
/// environment configuration, GitHub client setup, database connection, or
/// Discord client build. Returns an error if the HTTP webhook server or
/// Discord client exits unexpectedly.
pub async fn run(env_config: EnvConfig) -> Result<(), AppError> {
	bootstrap::Application::build(env_config).await?.run().await
}
