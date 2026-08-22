//! Log shipping to Grafana Loki via `tracing-loki`.

pub mod error;
pub use error::LokiError;

use crate::app::observability::LogSink;
use async_trait::async_trait;
use url::Url;

/// Alias used for clarity.
/// Ref: <https://docs.rs/tracing-loki/latest/tracing_loki/index.html>
pub type LokiTracingLayer = tracing_loki::Layer;

/// Handle to the running Loki log-shipping task.
///
/// Owns the [`tracing_loki::BackgroundTaskController`] and the task's
/// [`tokio::task::JoinHandle`], enabling a graceful drain on shutdown.
pub struct LokiHandle {
	controller: tracing_loki::BackgroundTaskController,
	join_handle: tokio::task::JoinHandle<()>,
}

impl LokiHandle {
	/// Signal the background task to stop, then wait for it to finish
	/// flushing any buffered log lines to Loki.
	///
	/// This stops the process's only log sink, so call it LAST during
	/// shutdown, after every other subsystem (HTTP server, Discord client)
	/// has stopped producing logs. Events logged while it drains, or after it
	/// returns are best-effort and may be silently dropped.
	pub async fn shutdown(self) {
		self.controller.shutdown().await;
		if let Err(e) = self.join_handle.await {
			eprintln!("Loki background task panicked during shutdown: {e}");
		}
	}
}

#[async_trait]
impl LogSink for LokiHandle {
	async fn shutdown(self) {
		self.shutdown().await;
	}
}

/// Builds the Loki tracing layer and spawns its background task.
///
/// Labels are indexed by Loki — **keep this set small and low-cardinality**.
/// The returned `LokiTracingLayer` should be added to the tracing subscriber.
/// The returned `LokiHandle` is already running in the background; it must be
/// kept alive (not dropped) for the duration of the program so logs keep
/// shipping, then drained via [`LogSink::shutdown`] last during shutdown.
///
/// # Errors
/// Returns `LokiError` if the URL is invalid or the layer fails to build.
///
/// # Panics
/// Must be called from within a running Tokio runtime (`tokio::spawn` requires
/// an active runtime context). Cannot be called more than once.
pub fn init(
	endpoint: &str,
	service_name: &str,
	environment: &str,
) -> Result<(LokiTracingLayer, LokiHandle), LokiError> {
	let url = Url::parse(endpoint).map_err(LokiError::InvalidUrl)?;

	let (layer, controller, task) = tracing_loki::builder()
		.label("service_name", service_name)
		.map_err(LokiError::BuildError)?
		.label("env", environment)
		.map_err(LokiError::BuildError)?
		.build_controller_url(url)
		.map_err(LokiError::BuildError)?;

	let join_handle = tokio::spawn(task);

	Ok((
		layer,
		LokiHandle {
			controller,
			join_handle,
		},
	))
}
