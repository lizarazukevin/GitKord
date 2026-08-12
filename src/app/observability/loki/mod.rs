//! Log shipping to Grafana Loki via `tracing-loki`.

pub mod error;
pub use error::LokiError;

use std::collections::HashMap;
use std::future::Future;
use url::Url;

/// Alias used for clarity.
/// Ref: <https://docs.rs/tracing-loki/latest/tracing_loki/index.html>
pub type LokiTracingLayer = tracing_loki::Layer;

/// Wraps the spawned background task that ships logs to Loki.
pub struct LokiTask {
	_handle: tokio::task::JoinHandle<()>,
}

impl LokiTask {
	fn new(task: impl Future<Output = ()> + Send + 'static) -> Self {
		Self {
			_handle: tokio::spawn(task),
		}
	}
}

/// Builds the Loki tracing layer and spawns its background task.
///
/// The returned `LokiTracingLayer` should be added to the tracing subscriber.
/// The `LokiTask` is already running in the background, it must be kept
/// alive (not dropped) for the duration of the program.
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
) -> Result<(LokiTracingLayer, LokiTask), LokiError> {
	let url = Url::parse(endpoint).map_err(LokiError::InvalidUrl)?;

	// Labels are indexed in Loki, **keep this set small and low-cardinality**
	let mut labels = HashMap::new();
	labels.insert("service_name".to_owned(), service_name.to_owned());
	labels.insert("environment".to_owned(), environment.to_owned());

	let (layer, task) =
		tracing_loki::layer(url, labels, HashMap::new()).map_err(LokiError::BuildError)?;

	Ok((layer, LokiTask::new(task)))
}
