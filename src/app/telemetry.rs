//! Global tracing subscriber initialization.

use crate::app::observability::loki;
use crate::AppError;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Initializes the global tracing subscriber, optionally shipping logs to Loki.
///
/// Verbosity is controlled by `RUST_LOG` environment variable.
/// Defaults to `info` when the variable is absent. When `loki_endpoint` is
/// `Some`, a Loki layer is appended to the subscriber and logs are shipped to
/// the given endpoint; otherwise logs are written to stdout only.
///
/// # Errors
///
/// Returns [`AppError::Loki`] if the endpoint URL is invalid or the Loki
/// tracing layer fails to build.
pub fn init_tracing(
	loki_endpoint: Option<&str>,
	service_name: &str,
	environment: &str,
) -> Result<(), AppError> {
	let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

	let subscriber = tracing_subscriber::registry()
		.with(env_filter)
		.with(tracing_subscriber::fmt::layer());

	if let Some(endpoint) = loki_endpoint {
		let (loki_layer, _task) = loki::init(endpoint, service_name, environment)?;
		subscriber.with(loki_layer).init();
	} else {
		subscriber.init();
	}

	Ok(())
}
