#![warn(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(clippy::multiple_crate_versions)] // transitive deps — not in our control

//! `GitKord` entrypoint.
//!
//! Two tasks run concurrently for the lifetime of the process:
//! - Axum HTTP server — receives GitHub webhook payloads
//! - Serenity Discord client — handles gateway events and slash registry
//!
//! Both are spawned as Tokio tasks so neither blocks the other.
//! If either task exits unexpectedly, the process exits with a non-zero code
//! so Railway (or any supervisor) knows to restart it.

use git_kord::{
	init_tracing, run, service_scope, AppError, EnvConfig, Environment, LogSink, APP_NAME,
};
use tracing::Instrument;

#[tokio::main]
async fn main() -> Result<(), AppError> {
	let env_config = EnvConfig::from_env()?;
	let environment = Environment::from(env_config.local_dev).to_string();

	let log_sink = init_tracing(env_config.log_endpoint.as_deref(), APP_NAME, &environment)?;

	let result = run(env_config)
		.instrument(service_scope(APP_NAME, &environment))
		.await;

	if let Some(sink) = log_sink {
		sink.shutdown().await;
	}

	result
}
