//! Application lifecycle orchestration.

mod bootstrap;
mod server;
mod shutdown;
mod telemetry;

pub use telemetry::init_tracing;

use crate::error::AppError;

/// Build the application and run it until a shutdown signal or fatal error.
pub async fn run() -> Result<(), AppError> {
    bootstrap::Application::build().await?.run().await
}
