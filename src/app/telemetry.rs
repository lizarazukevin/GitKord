//! Global tracing subscriber initialization.

use crate::app::observability::loki;
use crate::app::observability::LogSink;
use crate::AppError;
use tracing::info;
use tracing::span;
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;

/// Default filter directive when `RUST_LOG` is unset.
///
/// Scopes noisy HTTP client internals down to `warn` to avoid flooding stdout and logs.
const DEFAULT_FILTER: &str = "info,hyper=warn,hyper_util=warn,reqwest=warn";

/// Name of the root span that carries the process-wide service metadata.
const SERVICE_SCOPE: &str = "service_scope";

/// Build the process-wide service scope span.
///
/// Carries `service.name`, `service.version`, and `environment` as
/// span fields. Every log event emitted inside `run(..).instrument(scope)`
/// inherits these fields because the formatters include the current span's fields.
///
/// This is the standard `tracing` mechanism for injecting static resource
/// metadata: a `Layer` cannot add fields to a span or event that wasn't
/// declared at creation, but a single root span entered for the process
/// lifetime guarantees every downstream span-less log shares the metadata.
pub fn service_scope(service_name: &str, environment: &str) -> tracing::Span {
	span!(
		Level::INFO,
		SERVICE_SCOPE,
		"service.name" = service_name,
		"service.version" = env!("CARGO_PKG_VERSION"),
		environment,
	)
}

/// Choose the stdout formatter based on the environment.
///
/// - Local development: human-readable log lines for quick scanning in the
///   terminal / Railway dev logs.
/// - Production: structured JSON with the current span's fields.
///
/// Note: Loki always receives full structure from the `tracing-loki` layer
/// regardless of this stdout format.
fn make_fmt_layer<S>(environment: &str) -> Box<dyn Layer<S> + Send + Sync>
where
	S: tracing::Subscriber + for<'a> LookupSpan<'a> + Send + Sync,
{
	match environment {
		"prod" => tracing_subscriber::fmt::layer()
			.json()
			.with_target(false)
			.with_current_span(true)
			.with_span_list(false)
			.boxed(),
		_ => tracing_subscriber::fmt::layer().boxed(),
	}
}

/// Initialize the global tracing subscriber.
///
/// Logs to stdout with an environment-appropriate format (human-readable in
/// dev, JSON in prod). If `log_endpoint` is present, also ships logs to the
/// configured backend (e.g. Loki) and returns a [`LogSink`] for clean
/// shutdown; if `None`, log export is skipped and `None` is returned.
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

	let subscriber = tracing_subscriber::registry().with(env_filter);

	let layer = make_fmt_layer(environment);
	let subscriber = subscriber.with(layer);

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
