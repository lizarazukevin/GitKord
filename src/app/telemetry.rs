//! Global tracing subscriber initialization.

use crate::app::observability::loki;
use crate::app::observability::LogSink;
use crate::AppError;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Default filter directive when `RUST_LOG` is unset.
///
/// Scopes noisy HTTP client internals down to `warn` to avoid flooding stdout and logs.
const DEFAULT_FILTER: &str = "info,hyper=warn,hyper_util=warn,reqwest=warn";

/// Initialize the global tracing subscriber.
///
/// Always logs to stdout. If `log_endpoint` is present, also ships logs to
/// the configured backend (e.g. Loki) and returns a [`LogSink`] for
/// clean shutdown; if `None`, log export is skipped and `None` is returned.
///
/// The returned sink is opaque (`impl LogSink`) but owned by value — no
/// heap allocation or indirection — since it has a single owner and is
/// consumed by exactly one `shutdown()` call at exit.
///
/// # Errors
/// Returns `AppError::Loki` if the endpoint is set but invalid, or the Loki
/// layer fails to build.
pub fn init_tracing(
	log_endpoint: Option<&str>,
	service_name: &str,
	environment: &str,
) -> Result<Option<impl LogSink>, AppError> {
	let env_filter =
		EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

	let subscriber = tracing_subscriber::registry()
		.with(env_filter)
		.with(tracing_subscriber::fmt::layer());

	let log_sink = if let Some(endpoint) = log_endpoint {
		let (loki_layer, handle) = loki::init(endpoint, service_name, environment)?;
		subscriber.with(loki_layer).init();
		info!(endpoint, "Log export enabled");
		Some(handle)
	} else {
		subscriber.init();
		info!("Log export disabled, no LOG_ENDPOINT set");
		None
	};

	Ok(log_sink)
}
